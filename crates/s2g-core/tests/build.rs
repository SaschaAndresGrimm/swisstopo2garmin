//! The whole pipeline in Rust: GeoPackage region -> PBF -> splitter -> mkgmap ->
//! verified gmapsupp.img.
//!
//! Skipped when the toolchain has not been vendored, so a clean checkout still passes.

use std::path::{Path, PathBuf};

use s2g_core::contour::{generate, ContourConfig};
use s2g_core::download::Cancel;
use s2g_core::elevation::{Cell, Grid, Tile};
use s2g_core::extract::{RegionBuilder, DEFAULT_LAYERS};
use s2g_core::garmin::{compile, split, BuildOptions, MapIdentity, Toolchain};
use s2g_core::gpkg::Gpkg;
use s2g_core::img;
use s2g_core::proj::BBox;
use std::collections::HashMap;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixture_bbox() -> BBox {
    BBox::new(2_645_000.0, 1_163_000.0, 2_647_000.0, 1_165_000.0)
}

fn toolchain() -> Option<Toolchain> {
    Toolchain::discover(&repo_root()).ok()
}

#[test]
fn map_identity_is_deterministic_and_collision_resistant() {
    let a = MapIdentity::for_recipe("valais:hiking:2026-02", "Valais");
    let b = MapIdentity::for_recipe("valais:hiking:2026-02", "Valais");
    let c = MapIdentity::for_recipe("bern:cycling:2026-02", "Bern");

    assert_eq!(
        a.family_id, b.family_id,
        "same recipe must keep its identity"
    );
    assert_ne!(
        a.family_id, c.family_id,
        "different recipes must not collide"
    );
    assert!(
        a.family_id >= 6000,
        "stay clear of Garmin's own low family ids"
    );

    // The overview map number must differ from every tile number, or two of our own
    // maps collide on a device (docs/m0-findings.md §4.6).
    assert_eq!(a.overview_mapnumber() % 10_000, 0);
    for n in 0..5 {
        assert_ne!(a.tile_mapnumber(n), a.overview_mapnumber());
    }

    // splitter rejects a map id above 99_999_999 outright, so the allocation must stay
    // inside that range even with the maximum tile index.
    for key in ["a", "valais", "whole-switzerland:full-topo:2026-02", ""] {
        let id = MapIdentity::for_recipe(key, "x");
        assert!(
            id.overview_mapnumber() <= MapIdentity::MAX_MAP_NUMBER,
            "{key}: overview {} out of range",
            id.overview_mapnumber()
        );
        assert!(
            id.tile_mapnumber(9_998) <= MapIdentity::MAX_MAP_NUMBER,
            "{key}: tile {} out of range",
            id.tile_mapnumber(9_998)
        );
        assert!(id.overview_mapnumber() >= 10_000_000, "8 digits expected");
    }
}

#[test]
fn verifier_rejects_a_gmapsupp_without_an_overview_map() {
    // This is the exact shape a single-pass `--gmapsupp` build produces: one detail
    // tile, no overview map. The device lists it and draws nothing.
    let info = img::ImgInfo {
        file_bytes: 1_000_000,
        block_size: 1024,
        description: "test".into(),
        subfiles: vec![
            img::SubFile {
                name: "63260001".into(),
                kind: "TRE".into(),
                bytes: 10_000,
            },
            img::SubFile {
                name: "63260001".into(),
                kind: "RGN".into(),
                bytes: 900_000,
            },
            img::SubFile {
                name: "63260001".into(),
                kind: "LBL".into(),
                bytes: 30_000,
            },
            img::SubFile {
                name: "SWISSTOP".into(),
                kind: "TYP".into(),
                bytes: 1_700,
            },
        ],
    };
    let v = img::verify_gmapsupp(&info, false);
    assert!(!v.ok(), "a missing overview map must be fatal");
    assert!(
        v.problems.iter().any(|p| p.contains("overview")),
        "{:?}",
        v.problems
    );
}

#[test]
fn verifier_rejects_an_empty_map() {
    let info = img::ImgInfo {
        file_bytes: 5_000,
        block_size: 512,
        description: String::new(),
        subfiles: vec![
            img::SubFile {
                name: "63260000".into(),
                kind: "TRE".into(),
                bytes: 650,
            },
            img::SubFile {
                name: "63260000".into(),
                kind: "RGN".into(),
                bytes: 0,
            },
            img::SubFile {
                name: "63260001".into(),
                kind: "TRE".into(),
                bytes: 100,
            },
        ],
    };
    let v = img::verify_gmapsupp(&info, false);
    assert!(!v.ok());
    assert!(v
        .problems
        .iter()
        .any(|p| p.contains("empty") || p.contains("LBL")));
}

#[test]
fn verifier_enforces_device_limits() {
    let info = img::ImgInfo {
        file_bytes: 5 * 1024 * 1024 * 1024,
        block_size: 1024,
        description: String::new(),
        subfiles: vec![],
    };
    let v = img::verify_for_device(&info, 5000, 4_000_000_000, 4096);
    assert!(!v.ok());
    assert!(
        v.problems.iter().any(|p| p.contains("FAT32")),
        "{:?}",
        v.problems
    );
    assert!(
        v.problems.iter().any(|p| p.contains("tiles")),
        "{:?}",
        v.problems
    );
}

/// The real thing, end to end.
#[test]
fn builds_a_verified_gmapsupp_from_the_fixture() {
    let Some(tc) = toolchain() else {
        eprintln!("skipping: toolchain not vendored (run vendor/fetch_tools.py)");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let bbox = fixture_bbox();
    let cancel = Cancel::new();

    // ---- vectors + contours into one PBF ----
    let pbf = dir.path().join("region.osm.pbf");
    let mut builder = RegionBuilder::create(&pbf, &bbox).unwrap();
    let g = Gpkg::open(fixtures().join("grindelwald.gpkg")).unwrap();
    builder
        .add_vectors(&g, DEFAULT_LAYERS, &cancel, |_, _| {})
        .unwrap();

    let cell = Cell {
        e_km: 2645,
        n_km: 1163,
    };
    let tile = Tile::decode(&fixtures().join("swissalti3d_2019_2645-1163_2.tif"), cell).unwrap();
    let mut tiles = HashMap::new();
    tiles.insert(cell, tile);
    let grid = Grid::from_tiles(&[cell], &tiles).unwrap();
    let (contours, cstats) = generate(&grid, &ContourConfig::default(), || false);
    let n_contours = builder.add_contours(&contours, |_| false, &cancel).unwrap();
    assert!(n_contours > 0, "expected contours from a real tile");

    let stats = builder.finish().unwrap();
    assert!(stats.nodes > 0 && stats.ways > 0);

    // ---- split ----
    let identity = MapIdentity::for_recipe("test:fixture", "swissTLM3D test");
    let tile_pbfs = split(
        &tc,
        &pbf,
        &dir.path().join("tiles"),
        &identity,
        200_000,
        2048,
        &Cancel::new(),
    )
    .unwrap();
    assert!(!tile_pbfs.is_empty());

    // ---- compile ----
    let opts = BuildOptions::new(
        identity.clone(),
        repo_root().join("style/swisstopo"),
        repo_root().join("typ/swisstopo.txt"),
    );
    let out = compile(
        &tc,
        &tile_pbfs,
        &dir.path().join("img"),
        &opts,
        &Cancel::new(),
    )
    .unwrap();
    assert!(
        out.overview_img.is_some(),
        "the two-pass build must write an overview map"
    );
    assert!(out.bytes > 20_000, "gmapsupp is only {} bytes", out.bytes);

    // ---- verify ----
    let info = img::read(&out.gmapsupp).unwrap();
    let verdict = img::verify_gmapsupp(&info, false);
    assert!(verdict.ok(), "verification failed: {:?}", verdict.problems);

    let maps = info.maps();
    assert!(
        maps.iter().any(|m| m.ends_with("0000")),
        "no overview map among {maps:?}"
    );
    assert!(maps.len() >= 2, "expected overview + tiles, got {maps:?}");
    assert!(info.bytes_of_kind("RGN") > 10_000);

    println!(
        "built {} bytes: {} maps, {} tiles, RGN {} B, LBL {} B, contours {} ({:.0}% points removed)\n  kinds: {:?}\n  warnings: {:?}",
        out.bytes,
        maps.len(),
        out.tile_count,
        info.bytes_of_kind("RGN"),
        info.bytes_of_kind("LBL"),
        n_contours,
        cstats.simplification_ratio() * 100.0,
        info.kind_counts(),
        verdict.warnings
    );
}

#[test]
fn dem_dists_are_derived_per_style_and_mismatches_are_caught() {
    use s2g_core::garmin::style_level_count;

    // mkgmap aborts with "More dem-dist values than levels" if these disagree, and the
    // handlebar and wrist styles deliberately have different level counts.
    let handlebar = repo_root().join("style/swisstopo");
    let wrist = repo_root().join("style/swisstopo-wrist");
    let hl = style_level_count(&handlebar).unwrap();
    let wl = style_level_count(&wrist).unwrap();
    assert!(
        hl >= 4,
        "handlebar style should have several levels, got {hl}"
    );
    assert!(
        wl < hl,
        "the wrist style should have fewer levels ({wl} vs {hl})"
    );

    let id = MapIdentity::for_recipe("k", "n");
    for (dir, want) in [(handlebar, hl), (wrist, wl)] {
        let opts = BuildOptions::new(id.clone(), dir, repo_root().join("typ/swisstopo.txt"))
            .with_dem(PathBuf::from("/tmp/dem"))
            .unwrap();
        assert_eq!(opts.dem_dists.len(), want, "one dem-dist per level");
        // Each level halves the resolution of the previous one.
        for w in opts.dem_dists.windows(2) {
            assert_eq!(w[1], w[0] * 2);
        }
    }

    assert!(style_level_count(Path::new("/definitely/not/a/style")).is_err());
}
