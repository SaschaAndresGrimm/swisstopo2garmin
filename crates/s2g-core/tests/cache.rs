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

/// Every byte under the root must land in exactly one bucket.
///
/// The elevation tile cache is loose `.tif` files, so `list()` never saw it and the
/// reported usage omitted the fastest-growing directory. Eighteen calibration builds
/// filled a disk that way.
#[test]
fn usage_accounts_for_elevation_tiles_and_build_files() {
    use s2g_core::cache::Cache;

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    let write = |rel: &str, bytes: usize| {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, vec![b'x'; bytes]).unwrap();
    };

    // A dataset, in the app layout.
    write("ch.swisstopo.swisstlm3d/swisstlm3d_2026-02/x.gpkg", 5_000);
    // Elevation tiles: loose files, one level down. This is the part that was invisible.
    write(
        "ch.swisstopo.swissalti3d/swissalti3d_2019_2600-1199.tif",
        3_000,
    );
    write(
        "ch.swisstopo.swissalti3d/swissalti3d_2019_2601-1199.tif",
        3_000,
    );
    write("ch.swisstopo.swissaltiregio/regio.tif", 1_000);
    // Build intermediates.
    write("builds/grindelwald/region.osm.pbf", 7_000);
    write("builds/grindelwald/img/gmapsupp.img", 2_000);
    // A saved recipe, and a quarantined download.
    write("recipes/valais.json", 100);
    write(".quarantine/broken__item__1/x.gpkg", 400);
    // A stranded partial download at the root.
    write("stray.part", 50);

    let u = Cache::new(root).usage();
    assert_eq!(u.datasets, 5_000);
    assert_eq!(
        u.elevation, 7_000,
        "elevation must include both collections"
    );
    assert_eq!(u.builds, 9_000);
    assert_eq!(u.recipes, 100);
    assert_eq!(u.quarantine, 400);
    assert_eq!(u.other, 50);
    assert_eq!(u.total(), 21_550);

    // And the total must equal what the filesystem says, or a bucket is missing.
    let mut walked = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            if e.file_type().unwrap().is_dir() {
                stack.push(e.path());
            } else {
                walked += e.metadata().unwrap().len();
            }
        }
    }
    assert_eq!(u.total(), walked, "usage must account for every byte");
}

#[test]
fn the_spike_era_flat_directories_count_as_datasets() {
    use s2g_core::cache::Cache;

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("winter")).unwrap();
    std::fs::write(root.join("winter/ski_routes_2056.gpkg"), vec![b'x'; 800]).unwrap();
    std::fs::create_dir_all(root.join("routes/veloland/shp")).unwrap();
    std::fs::write(
        root.join("routes/veloland/shp/VeloWeg.shp"),
        vec![b'x'; 200],
    )
    .unwrap();

    let u = Cache::new(root).usage();
    assert_eq!(u.datasets, 1_000);
    assert_eq!(u.other, 0);
}

#[test]
fn clearing_elevation_and_builds_frees_exactly_those_bytes() {
    use s2g_core::cache::Cache;

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let write = |rel: &str, bytes: usize| {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, vec![b'x'; bytes]).unwrap();
    };
    write("ch.swisstopo.swisstlm3d/item/x.gpkg", 5_000);
    write("ch.swisstopo.swissalti3d/a.tif", 3_000);
    write("builds/x/region.osm.pbf", 7_000);
    write("recipes/keep.json", 100);

    let cache = Cache::new(root);
    assert_eq!(cache.clear_elevation().unwrap(), 3_000);
    assert_eq!(cache.clear_builds().unwrap(), 7_000);

    let u = cache.usage();
    assert_eq!(u.elevation, 0);
    assert_eq!(u.builds, 0);
    // The datasets and the recipe must survive: one is expensive, the other is not
    // re-derivable at all.
    assert_eq!(u.datasets, 5_000);
    assert_eq!(u.recipes, 100);

    // Clearing again is not an error.
    assert_eq!(cache.clear_elevation().unwrap(), 0);
    assert_eq!(cache.clear_builds().unwrap(), 0);
}

#[test]
fn usage_of_a_missing_root_is_zero_rather_than_an_error() {
    use s2g_core::cache::Cache;
    let u = Cache::new("/nonexistent/s2g-data").usage();
    assert_eq!(u, Default::default());
    assert_eq!(u.total(), 0);
}

/// SPEC.md §12: "Corrupt cache detected — quarantine, offer re-download, never crash."
///
/// The mechanism existed before this and was called from nowhere: corruption is noticed
/// by whoever reads the file, and that code has a path rather than a
/// `(collection, item)` pair.
#[tokio::test]
async fn a_corrupt_file_quarantines_the_dataset_it_belongs_to() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let item = cache
        .ensure_dir("ch.swisstopo.swisstlm3d", "swisstlm3d_2026-02")
        .await
        .unwrap();
    let gpkg = item.join("swisstlm3d.gpkg");
    std::fs::write(&gpkg, b"this is not a geopackage").unwrap();

    let moved = cache
        .quarantine_containing(&gpkg)
        .await
        .unwrap()
        .expect("a file inside the cache should be quarantined");

    assert!(
        !item.exists(),
        "the damaged dataset is still where a build would find it"
    );
    assert!(
        moved.join("swisstlm3d.gpkg").is_file(),
        "the data was deleted, not moved aside"
    );
    // And the dataset no longer lists as usable, so the Data screen offers it again.
    assert!(cache.list().await.unwrap().is_empty());
}

/// A user's own GeoPackage, somewhere else on disk, must not be moved.
#[tokio::test]
async fn a_file_outside_the_cache_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let mine = elsewhere.path().join("my-own.gpkg");
    std::fs::write(&mine, b"mine").unwrap();

    let cache = Cache::new(dir.path());
    assert_eq!(cache.quarantine_containing(&mine).await.unwrap(), None);
    assert!(mine.is_file(), "somebody else's file was moved");
}

/// A loose file directly under the cache root belongs to no dataset, and treating its
/// name as a collection would quarantine a directory that does not exist.
#[tokio::test]
async fn a_loose_file_at_the_cache_root_is_not_a_dataset() {
    let dir = tempfile::tempdir().unwrap();
    let loose = dir.path().join("calibration.jsonl");
    std::fs::write(&loose, b"{}").unwrap();
    let cache = Cache::new(dir.path());
    assert_eq!(cache.quarantine_containing(&loose).await.unwrap(), None);
    assert!(loose.is_file());
}

/// SPEC.md §12: "Insufficient disk space — precheck before download and before build;
/// report required vs. available."
///
/// The download side had no precheck at all, and the interesting part is that the wire
/// size is the wrong number to check: swissTLM3D arrives as 4.5 GB and lands as 10.0.
#[test]
fn a_download_is_measured_by_what_it_leaves_on_disk_not_what_crosses_the_wire() {
    use s2g_core::cache::download_need;

    const WIRE: u64 = 4_500_000_000;
    const ON_DISK: u64 = 10_000_000_000;

    // Streamed and inflated as it arrives: only the result is ever on disk.
    assert_eq!(
        download_need(Some(WIRE), Some(ON_DISK), true),
        Some(ON_DISK)
    );
    // Downloaded whole, then extracted: both exist at the same moment.
    assert_eq!(
        download_need(Some(WIRE), Some(ON_DISK), false),
        Some(WIRE + ON_DISK)
    );
    // A bare GeoPackage is its own member, so the two numbers agree.
    assert_eq!(download_need(Some(WIRE), Some(WIRE), true), Some(WIRE));
}

/// A precheck cannot be invented from no information, and refusing a download because
/// a HEAD request failed would be a worse failure than not checking.
#[test]
fn an_unknown_size_produces_no_precheck_rather_than_a_guess() {
    use s2g_core::cache::download_need;
    assert_eq!(download_need(None, None, true), None);
    assert_eq!(download_need(None, None, false), None);
    // One number known is better than none.
    assert_eq!(download_need(Some(100), None, false), Some(100));
    assert_eq!(download_need(None, Some(200), false), Some(200));
}
