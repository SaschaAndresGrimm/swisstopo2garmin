use s2g_core::cache::{available_bytes, precheck_space, Cache, Provenance};
use s2g_core::stac::Digest;

fn prov(collection: &str, item: &str, file: &str, bytes: u64) -> Provenance {
    Provenance {
        collection: collection.into(),
        item: item.into(),
        datetime: Some("2026-02-24T00:00:00Z".into()),
        asset: format!("{item}.gpkg.zip"),
        href: "https://example.test/x.zip".into(),
        checksum: Digest::from_multihash(&format!("1220{}", "ab".repeat(32))).ok(),
        file: file.into(),
        bytes,
        fetched_at: "2026-09-06T00:00:00Z".into(),
        inflated: true,
    }
}

#[tokio::test]
async fn lists_entries_with_provenance_and_totals() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());

    let d = cache
        .ensure_dir("ch.swisstopo.swisstlm3d", "swisstlm3d_2026-02")
        .await
        .unwrap();
    std::fs::write(d.join("SWISSTLM3D.gpkg"), vec![7u8; 2048]).unwrap();
    cache
        .write_provenance(&prov(
            "ch.swisstopo.swisstlm3d",
            "swisstlm3d_2026-02",
            "SWISSTLM3D.gpkg",
            2048,
        ))
        .await
        .unwrap();

    let entries = cache.list().await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].bytes, 2048);
    assert_eq!(entries[0].item, "swisstlm3d_2026-02");
    assert!(entries[0].provenance.as_ref().unwrap().inflated);
    assert_eq!(cache.total_bytes().await.unwrap(), 2048);
}

#[tokio::test]
async fn partial_downloads_are_not_listed_as_datasets() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let d = cache.ensure_dir("coll", "item").await.unwrap();
    std::fs::write(d.join("real.gpkg"), b"ok").unwrap();
    std::fs::write(d.join("half.gpkg.part"), b"nope").unwrap();

    let entries = cache.list().await.unwrap();
    assert_eq!(
        entries.len(),
        1,
        "a .part file must never look like a dataset"
    );
    assert!(entries[0].path.ends_with("real.gpkg"));
}

#[tokio::test]
async fn quarantine_moves_aside_rather_than_deleting() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let d = cache.ensure_dir("coll", "bad").await.unwrap();
    std::fs::write(d.join("x.bin"), b"corrupt").unwrap();

    let moved = cache.quarantine("coll", "bad").await.unwrap();
    assert!(moved.exists(), "quarantined data must still be inspectable");
    assert!(!d.exists());
    assert!(cache.list().await.unwrap().is_empty());
}

#[tokio::test]
async fn missing_cache_root_is_empty_not_an_error() {
    let cache = Cache::new("/definitely/does/not/exist/s2g");
    assert!(cache.list().await.unwrap().is_empty());
}

#[test]
fn reports_free_space_and_rejects_impossible_requests() {
    let dir = tempfile::tempdir().unwrap();
    let free = available_bytes(dir.path()).expect("free space should be readable");
    assert!(free > 0);

    precheck_space(dir.path(), 1024).expect("1 KiB should fit");
    let err = precheck_space(dir.path(), u64::MAX).unwrap_err();
    assert!(
        matches!(err, s2g_core::Error::InsufficientSpace { .. }),
        "{err}"
    );
}

#[test]
fn free_space_works_for_a_directory_that_does_not_exist_yet() {
    // the cache dir is created lazily, so the precheck must walk up to a real ancestor
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("not").join("created").join("yet");
    assert!(available_bytes(&nested).is_some());
}

#[tokio::test]
async fn reads_provenance_written_by_the_milestone_0_spike() {
    // spikes/s0/fetch_tlm3d.py used different field names and omitted several fields.
    // The real cache on a developer machine contains files in this shape.
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let d = cache
        .ensure_dir("ch.swisstopo.swisstlm3d", "swisstlm3d_2026-02")
        .await
        .unwrap();
    std::fs::write(d.join("SWISSTLM3D_2026_LV95_LN02.gpkg"), b"x").unwrap();
    std::fs::write(
        d.join("provenance.json"),
        br#"{
          "collection": "ch.swisstopo.swisstlm3d",
          "item": "swisstlm3d_2026-02",
          "datetime": "2026-02-24T00:00:00Z",
          "asset": "swisstlm3d_2026-02_2056_5728.gpkg.zip",
          "href": "https://data.geo.admin.ch/x.zip",
          "checksum_multihash": "12202218054B21C23E1683FB14A80A9C33D811C7E02A891AB4AEDAF761B68750206A",
          "member": "SWISSTLM3D_2026_LV95_LN02.gpkg",
          "fetched_at": "2026-09-06T08:55:16Z"
        }"#,
    )
    .unwrap();

    let entries = cache.list().await.unwrap();
    assert_eq!(entries.len(), 1, "the .gpkg is the only dataset");
    let p = entries[0]
        .provenance
        .as_ref()
        .expect("spike provenance must be readable");
    assert_eq!(
        p.file, "SWISSTLM3D_2026_LV95_LN02.gpkg",
        "`member` aliases to `file`"
    );
    assert_eq!(p.fetched_at, "2026-09-06T08:55:16Z");
    let d = p
        .checksum
        .as_ref()
        .expect("bare multihash string must decode");
    assert_eq!(d.algo, "sha2-256");
}

#[tokio::test]
async fn sidecar_files_are_not_datasets() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let d = cache.ensure_dir("coll", "item").await.unwrap();
    for f in [
        "real.gpkg",
        "status.json",
        "provenance.json",
        "x.part",
        "y.tmp",
        ".DS_Store",
    ] {
        std::fs::write(d.join(f), b"{}").unwrap();
    }
    let entries = cache.list().await.unwrap();
    assert_eq!(
        entries.len(),
        1,
        "only real.gpkg is a dataset, got {entries:?}"
    );
    assert!(entries[0].path.ends_with("real.gpkg"));
}

#[tokio::test]
async fn sweeps_orphaned_partials_left_by_a_killed_process() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let d = cache.ensure_dir("coll", "item").await.unwrap();
    std::fs::write(d.join("keep.gpkg"), vec![0u8; 10]).unwrap();
    std::fs::write(d.join("orphan.gpkg.part"), vec![0u8; 4096]).unwrap();
    std::fs::write(d.join("provenance.json.tmp"), b"{}").unwrap();

    let (freed, removed) = cache.sweep_partials().await.unwrap();
    assert_eq!(freed, 4096 + 2);
    assert_eq!(removed.len(), 2);
    assert!(
        d.join("keep.gpkg").exists(),
        "real datasets must survive the sweep"
    );
    assert_eq!(cache.list().await.unwrap().len(), 1);
}
