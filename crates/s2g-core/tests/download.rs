//! Acquisition behaviour: zip member location, streaming inflate, checksum
//! verification, mid-stream resume, and cancellation.
//!
//! These cover the acquisition path for swissTLM3D, which ships as a ZIP64 archive
//! holding one DEFLATE member (4.80 GB -> 10.78 GB). See docs/m0-findings.md §1.

use std::io::Write;
use std::sync::atomic::Ordering;

use s2g_core::download::{download, download_zip_member_inflated, Cancel, Progress};
use s2g_core::stac::Digest;
use s2g_core::testing::FakeHttp;
use s2g_core::zip;
use sha2::{Digest as _, Sha256};

/// Build a single-member ZIP with a DEFLATE payload, optionally in ZIP64 form.
fn build_zip(name: &str, payload: &[u8], zip64: bool) -> Vec<u8> {
    let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(payload).unwrap();
    let deflated = enc.finish().unwrap();

    let mut out = Vec::new();
    let nb = name.as_bytes();

    // ---- local file header
    let lfh_off = out.len() as u64;
    out.extend(b"PK\x03\x04");
    out.extend(20u16.to_le_bytes()); // version needed
    out.extend(0u16.to_le_bytes()); // flags
    out.extend(8u16.to_le_bytes()); // method: deflate
    out.extend(0u16.to_le_bytes()); // time
    out.extend(0u16.to_le_bytes()); // date
    out.extend(0u32.to_le_bytes()); // crc (unused by us)
    out.extend((deflated.len() as u32).to_le_bytes());
    out.extend((payload.len() as u32).to_le_bytes());
    out.extend((nb.len() as u16).to_le_bytes());
    out.extend(0u16.to_le_bytes()); // extra len
    out.extend(nb);
    out.extend(&deflated);

    // ---- central directory
    let cd_off = out.len() as u64;
    let zip64_extra: Vec<u8> = if zip64 {
        let mut e = Vec::new();
        e.extend(0x0001u16.to_le_bytes());
        e.extend(24u16.to_le_bytes());
        e.extend((payload.len() as u64).to_le_bytes());
        e.extend((deflated.len() as u64).to_le_bytes());
        e.extend(lfh_off.to_le_bytes());
        e
    } else {
        Vec::new()
    };

    out.extend(b"PK\x01\x02");
    out.extend(45u16.to_le_bytes());
    out.extend(45u16.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out.extend(8u16.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    if zip64 {
        out.extend(u32::MAX.to_le_bytes()); // csize -> in extra
        out.extend(u32::MAX.to_le_bytes()); // usize -> in extra
    } else {
        out.extend((deflated.len() as u32).to_le_bytes());
        out.extend((payload.len() as u32).to_le_bytes());
    }
    out.extend((nb.len() as u16).to_le_bytes());
    out.extend((zip64_extra.len() as u16).to_le_bytes());
    out.extend(0u16.to_le_bytes()); // comment len
    out.extend(0u16.to_le_bytes()); // disk
    out.extend(0u16.to_le_bytes()); // internal attrs
    out.extend(0u32.to_le_bytes()); // external attrs
    if zip64 {
        out.extend(u32::MAX.to_le_bytes()); // lfh offset -> in extra
    } else {
        out.extend((lfh_off as u32).to_le_bytes());
    }
    out.extend(nb);
    out.extend(&zip64_extra);
    let cd_size = out.len() as u64 - cd_off;

    if zip64 {
        let eocd64_off = out.len() as u64;
        out.extend(b"PK\x06\x06");
        out.extend(44u64.to_le_bytes());
        out.extend(45u16.to_le_bytes());
        out.extend(45u16.to_le_bytes());
        out.extend(0u32.to_le_bytes());
        out.extend(0u32.to_le_bytes());
        out.extend(1u64.to_le_bytes());
        out.extend(1u64.to_le_bytes());
        out.extend(cd_size.to_le_bytes());
        out.extend(cd_off.to_le_bytes());

        out.extend(b"PK\x06\x07");
        out.extend(0u32.to_le_bytes());
        out.extend(eocd64_off.to_le_bytes());
        out.extend(1u32.to_le_bytes());
    }

    // ---- end of central directory
    out.extend(b"PK\x05\x06");
    out.extend(0u16.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    if zip64 {
        out.extend(u32::MAX.to_le_bytes());
        out.extend(u32::MAX.to_le_bytes());
    } else {
        out.extend((cd_size as u32).to_le_bytes());
        out.extend((cd_off as u32).to_le_bytes());
    }
    out.extend(0u16.to_le_bytes());
    out
}

fn payload(n: usize) -> Vec<u8> {
    // compressible but not trivially so, and large enough to span many chunks
    (0..n).map(|i| ((i * 31 + i / 7) % 251) as u8).collect()
}

/// Pseudo-random bytes that DEFLATE cannot shrink, so the archive stays large enough
/// for tests that need many chunks on the wire.
fn incompressible(n: usize) -> Vec<u8> {
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    (0..n)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}

fn sha256_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

fn multihash_of(b: &[u8]) -> String {
    format!("1220{}", sha256_hex(b))
}

#[tokio::test]
async fn locates_the_member_in_a_zip64_archive() {
    let body = payload(400_000);
    let archive = build_zip("SWISSTLM3D_TEST.gpkg", &body, true);
    let http = FakeHttp::new().with_body("http://x/a.zip", archive.clone());

    let m = zip::first_member(&http, "http://x/a.zip", archive.len() as u64)
        .await
        .unwrap();
    assert_eq!(m.name, "SWISSTLM3D_TEST.gpkg");
    assert_eq!(m.uncompressed_size, body.len() as u64);
    assert!(m.is_deflate());
    assert!(m.data_end() <= archive.len() as u64);
}

#[tokio::test]
async fn locates_the_member_in_a_plain_zip() {
    let body = payload(50_000);
    let archive = build_zip("plain.bin", &body, false);
    let http = FakeHttp::new().with_body("http://x/a.zip", archive.clone());
    let m = zip::first_member(&http, "http://x/a.zip", archive.len() as u64)
        .await
        .unwrap();
    assert_eq!(m.name, "plain.bin");
    assert_eq!(m.uncompressed_size, body.len() as u64);
}

#[tokio::test]
async fn inflates_while_downloading_and_verifies_the_archive_checksum() {
    let body = payload(500_000);
    let archive = build_zip("data.gpkg", &body, true);
    let digest = Digest::from_multihash(&multihash_of(&archive)).unwrap();
    let http = FakeHttp::new().with_body("http://x/a.zip", archive);

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("data.gpkg");
    let mut seen = Progress::default();
    let (path, member) = download_zip_member_inflated(
        &http,
        "http://x/a.zip",
        &dest,
        Some(&digest),
        &Cancel::new(),
        &mut |p| seen = p,
    )
    .await
    .unwrap();

    assert_eq!(
        std::fs::read(&path).unwrap(),
        body,
        "inflated bytes must match"
    );
    assert_eq!(member.uncompressed_size, body.len() as u64);
    assert_eq!(seen.written, body.len() as u64);
    // the archive is never stored alongside the inflated output
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(names, vec!["data.gpkg"], "no .part or .zip left behind");
}

#[tokio::test]
async fn resumes_mid_stream_without_restarting_the_transfer() {
    let body = incompressible(600_000);
    let archive = build_zip("data.gpkg", &body, true);
    let digest = Digest::from_multihash(&multihash_of(&archive)).unwrap();
    // fail once, a third of the way in
    let http = FakeHttp::new()
        .with_body("http://x/a.zip", archive.clone())
        .failing_after(archive.len() / 3);

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("data.gpkg");

    // clear the injected failure once it has fired, so the retry succeeds
    let failures = http.failures.clone();
    let calls = http.stream_calls.clone();
    let handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            if failures.load(Ordering::SeqCst) > 0 {
                break;
            }
        }
    });

    // give the watcher a chance, then run with the failure cleared after first hit
    let http_ref = &http;
    let run = async {
        let mut p = Progress::default();
        let r = download_zip_member_inflated(
            http_ref,
            "http://x/a.zip",
            &dest,
            Some(&digest),
            &Cancel::new(),
            &mut |x| p = x,
        )
        .await;
        (r, p)
    };
    let clearer = async {
        while http.failures.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        http.clear_failures();
    };
    let ((res, prog), _) = tokio::join!(run, clearer);
    handle.abort();

    let (path, _m) = res.expect("download should recover from a mid-stream failure");
    assert_eq!(std::fs::read(&path).unwrap(), body);
    assert!(prog.retries >= 1, "a retry should have been recorded");
    assert!(
        calls.load(Ordering::SeqCst) >= 2,
        "it should have reconnected at least once"
    );
}

#[tokio::test]
async fn rejects_a_corrupt_archive_and_leaves_nothing_behind() {
    let body = payload(80_000);
    let archive = build_zip("data.gpkg", &body, true);
    let wrong = Digest::from_multihash(&format!("1220{}", "00".repeat(32))).unwrap();
    let http = FakeHttp::new().with_body("http://x/a.zip", archive);

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("data.gpkg");
    let err = download_zip_member_inflated(
        &http,
        "http://x/a.zip",
        &dest,
        Some(&wrong),
        &Cancel::new(),
        &mut |_| {},
    )
    .await
    .unwrap_err();

    assert!(
        matches!(err, s2g_core::Error::ChecksumMismatch { .. }),
        "{err}"
    );
    assert!(!dest.exists(), "verified-bad output must not be published");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn cancellation_is_prompt_and_leaves_no_partial_file() {
    // incompressible, so the archive really is ~2 MB on the wire and cancellation
    // lands mid-transfer rather than after it has already finished
    let body = incompressible(2_000_000);
    let archive = build_zip("data.gpkg", &body, true);
    let http = FakeHttp::new().with_body("http://x/a.zip", archive);
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("data.gpkg");

    let cancel = Cancel::new();
    let c2 = cancel.clone();
    let err = download_zip_member_inflated(
        &http,
        "http://x/a.zip",
        &dest,
        None,
        &cancel,
        &mut move |p| {
            if p.read > 100_000 {
                c2.cancel();
            }
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(err, s2g_core::Error::Cancelled), "{err}");
    assert!(!dest.exists());
    // The earlier version of this test only checked `dest`, so it passed while a
    // multi-gigabyte `.part` file was left behind. Assert the directory is empty.
    let leftover: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        leftover.is_empty(),
        "cancel left files behind: {leftover:?}"
    );
}

#[tokio::test]
async fn plain_download_verifies_and_publishes_atomically() {
    let body = payload(120_000);
    let digest = Digest::from_multihash(&multihash_of(&body)).unwrap();
    let http = FakeHttp::new().with_body("http://x/f.bin", body.clone());
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("f.bin");

    let path = download(
        &http,
        "http://x/f.bin",
        &dest,
        Some(&digest),
        &Cancel::new(),
        &mut |_| {},
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read(path).unwrap(), body);
}
