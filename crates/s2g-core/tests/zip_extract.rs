//! Local zip extraction, used for the multi-file ASTRA route archives.
//!
//! Archives are built here with the system `zip`, so the reader is tested against real
//! archives rather than against bytes this project also wrote.

use std::path::Path;
use std::process::Command;

use s2g_core::zip::extract_all;

fn have_zip() -> bool {
    Command::new("zip")
        .arg("-v")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Build an archive from files written into a temporary source directory.
fn make_archive(dir: &Path, files: &[(&str, &[u8])], extra_args: &[&str]) -> std::path::PathBuf {
    let src = dir.join("src");
    std::fs::create_dir_all(src.join("nested")).unwrap();
    for (name, body) in files {
        std::fs::write(src.join(name), body).unwrap();
    }
    let archive = dir.join("a.zip");
    let mut cmd = Command::new("zip");
    cmd.arg("-r").args(extra_args).arg(&archive).arg(".");
    cmd.current_dir(&src);
    let out = cmd.output().unwrap();
    assert!(out.status.success(), "zip failed: {out:?}");
    archive
}

#[test]
fn extracts_every_member_of_a_multi_file_archive() {
    if !have_zip() {
        eprintln!("skipping: no zip command");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    // Something incompressible and something very compressible, so both stored and
    // deflate paths are exercised: zip stores a member it cannot shrink.
    let random: Vec<u8> = (0..4096u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
    let repetitive = vec![b'A'; 8192];
    let archive = make_archive(
        dir.path(),
        &[
            ("Route.shp", &random),
            ("Route.dbf", &repetitive),
            ("Route.prj", b"PROJCS[\"CH1903+\"]"),
        ],
        &[],
    );

    let dest = dir.path().join("out");
    let mut files = extract_all(&archive, &dest).unwrap();
    files.sort();
    let names: Vec<String> = files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(names.contains(&"Route.shp".to_string()), "{names:?}");
    assert!(names.contains(&"Route.dbf".to_string()));
    assert!(names.contains(&"Route.prj".to_string()));

    assert_eq!(std::fs::read(dest.join("Route.shp")).unwrap(), random);
    assert_eq!(std::fs::read(dest.join("Route.dbf")).unwrap(), repetitive);
}

/// The ASTRA archives nest their shapefiles in a dated directory.
#[test]
fn nested_paths_are_flattened_into_the_destination() {
    if !have_zip() {
        eprintln!("skipping: no zip command");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir_all(src.join("2026_shape_veloland")).unwrap();
    std::fs::write(src.join("2026_shape_veloland").join("VeloWeg.shp"), b"body").unwrap();
    let archive = dir.path().join("a.zip");
    assert!(Command::new("zip")
        .arg("-r")
        .arg(&archive)
        .arg(".")
        .current_dir(&src)
        .output()
        .unwrap()
        .status
        .success());

    let dest = dir.path().join("out");
    let files = extract_all(&archive, &dest).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(std::fs::read(dest.join("VeloWeg.shp")).unwrap(), b"body");
}

/// A member path must never escape the destination directory.
#[test]
fn a_traversal_path_cannot_write_outside_the_destination() {
    if !have_zip() {
        eprintln!("skipping: no zip command");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("evil"), b"pwned").unwrap();
    let archive = dir.path().join("a.zip");
    // Explicit member name containing a traversal.
    assert!(Command::new("zip")
        .arg(&archive)
        .arg("evil")
        .current_dir(&src)
        .output()
        .unwrap()
        .status
        .success());
    // Rewrite the stored name to ../../escaped.
    let mut bytes = std::fs::read(&archive).unwrap();
    let target = b"evil";
    let replacement = b"../e";
    let mut i = 0;
    while let Some(pos) = bytes[i..]
        .windows(target.len())
        .position(|w| w == target)
        .map(|p| p + i)
    {
        bytes[pos..pos + target.len()].copy_from_slice(replacement);
        i = pos + target.len();
    }
    std::fs::write(&archive, &bytes).unwrap();

    let dest = dir.path().join("out");
    let files = extract_all(&archive, &dest).unwrap();
    for f in &files {
        assert!(
            f.starts_with(&dest),
            "{f:?} escaped the destination directory"
        );
    }
    assert!(!dir.path().join("e").exists());
}

#[test]
fn a_truncated_archive_is_an_error_not_a_partial_lie() {
    if !have_zip() {
        eprintln!("skipping: no zip command");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let archive = make_archive(dir.path(), &[("a.bin", &vec![b'x'; 40_000])], &[]);
    let mut bytes = std::fs::read(&archive).unwrap();
    bytes.truncate(bytes.len() / 2);
    let cut = dir.path().join("cut.zip");
    std::fs::write(&cut, bytes).unwrap();

    assert!(extract_all(&cut, &dir.path().join("out")).is_err());
}

/// Streamed archives leave the local header's sizes zero and put them in a trailing
/// data descriptor. The ASTRA route archives are written this way, and a reader that
/// takes its sizes from local headers fails on them.
#[test]
fn local_header_sizes_are_ignored_in_favour_of_the_central_directory() {
    if !have_zip() {
        eprintln!("skipping: no zip command");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let body = vec![b'Z'; 20_000];
    let archive = make_archive(
        dir.path(),
        &[("VeloWeg.dbf", &body), ("VeloWeg.cpg", b"UTF-8")],
        &[],
    );

    // Rewrite every local header the way a streaming writer leaves it: bit 3 of the
    // flags set, sizes zero. The real values stay in the central directory, which is
    // where a correct reader takes them from.
    let mut raw = std::fs::read(&archive).unwrap();
    let mut i = 0;
    let mut rewritten = 0;
    while i + 30 <= raw.len() {
        if &raw[i..i + 4] == b"PK\x03\x04" {
            raw[i + 6] |= 0x08;
            raw[i + 18..i + 26].fill(0);
            rewritten += 1;
            let name_len = u16::from_le_bytes([raw[i + 26], raw[i + 27]]) as usize;
            let extra_len = u16::from_le_bytes([raw[i + 28], raw[i + 29]]) as usize;
            i += 30 + name_len + extra_len;
        } else {
            i += 1;
        }
    }
    assert!(rewritten >= 2, "expected to rewrite both local headers");
    let streamed = dir.path().join("streamed.zip");
    std::fs::write(&streamed, &raw).unwrap();

    let dest = dir.path().join("out");
    let files = extract_all(&streamed, &dest).unwrap();
    assert_eq!(files.len(), 2, "{files:?}");
    assert_eq!(std::fs::read(dest.join("VeloWeg.dbf")).unwrap(), body);
    assert_eq!(std::fs::read(dest.join("VeloWeg.cpg")).unwrap(), b"UTF-8");
}

#[test]
fn a_file_that_is_not_an_archive_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("not.zip");
    std::fs::write(&path, b"this is not a zip archive at all, not even close").unwrap();
    assert!(extract_all(&path, &dir.path().join("out")).is_err());

    std::fs::write(&path, b"tiny").unwrap();
    assert!(extract_all(&path, &dir.path().join("out")).is_err());
}
