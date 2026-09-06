//! DEM generation: cell naming, geometry, void handling and the .hgt format.

use s2g_core::dem::{dem_dists, write_hgt, HgtCell, Resolution, VOID};
use s2g_core::elevation::{Cell, Grid, Tile, NODATA, TILE_PX};
use s2g_core::proj::BBox;
use std::collections::HashMap;

fn grindelwald_grid() -> (Grid, BBox) {
    let cell = Cell {
        e_km: 2645,
        n_km: 1163,
    };
    let tile = Tile::decode(
        std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/swissalti3d_2019_2645-1163_2.tif"
        )),
        cell,
    )
    .expect("fixture tile");
    let mut tiles = HashMap::new();
    tiles.insert(cell, tile);
    let grid = Grid::from_tiles(&[cell], &tiles).unwrap();
    let bbox = grid.bbox();
    (grid, bbox)
}

#[test]
fn cell_names_follow_the_srtm_convention() {
    assert_eq!(HgtCell { lat: 46, lon: 8 }.name(), "N46E008");
    assert_eq!(HgtCell { lat: 47, lon: 7 }.name(), "N47E007");
    assert_eq!(HgtCell { lat: -34, lon: -59 }.name(), "S34W059");
    assert_eq!(HgtCell { lat: 0, lon: 0 }.name(), "N00E000");
}

#[test]
fn cells_cover_a_bbox_using_all_four_corners() {
    // The LV95 grid is rotated relative to the graticule, so a two-corner conversion
    // can miss a cell along an edge.
    let bbox = BBox::new(2_600_000.0, 1_150_000.0, 2_700_000.0, 1_250_000.0);
    let cells = HgtCell::covering(&bbox);
    assert!(!cells.is_empty());
    let [w, s, e, n] = bbox.to_wgs84();
    for lat in s.floor() as i32..=n.floor() as i32 {
        for lon in w.floor() as i32..=e.floor() as i32 {
            assert!(
                cells.contains(&HgtCell { lat, lon }),
                "missing cell {lat}/{lon}"
            );
        }
    }
}

#[test]
fn file_size_is_exactly_the_srtm_format_size() {
    // mkgmap identifies the resolution from the file size, so this is not negotiable.
    assert_eq!(Resolution::ArcSecond1.file_bytes(), 25_934_402);
    assert_eq!(Resolution::ArcSecond3.file_bytes(), 2_884_802);
}

#[test]
fn writes_valid_hgt_cells_from_a_real_tile() {
    let (grid, bbox) = grindelwald_grid();
    let dir = tempfile::tempdir().unwrap();

    // 3 arc-second keeps the test fast; the geometry logic is identical.
    let (files, stats) =
        write_hgt(&grid, &bbox, Resolution::ArcSecond3, dir.path(), |_| {}).unwrap();

    assert_eq!(files.len(), stats.cells);
    assert!(!files.is_empty());
    for f in &files {
        let meta = std::fs::metadata(f).unwrap();
        assert_eq!(
            meta.len() as usize,
            Resolution::ArcSecond3.file_bytes(),
            "{} has the wrong size for its resolution",
            f.display()
        );
        let name = f.file_stem().unwrap().to_string_lossy();
        assert!(
            name.starts_with('N') && name.contains('E'),
            "bad name {name}"
        );
    }

    // A 1 km tile inside a 1-degree cell covers a small fraction, so most samples are
    // void by design — but some must be real, or the resampling is broken.
    assert!(stats.samples_filled > 0, "no samples were filled");
    assert!(
        stats.samples_void > stats.samples_filled,
        "a 1 km tile cannot fill a degree"
    );
    println!(
        "{} cell(s), {:.3}% covered, {} bytes",
        stats.cells,
        stats.coverage() * 100.0,
        stats.bytes
    );
}

#[test]
fn filled_samples_are_plausible_elevations_and_voids_are_marked() {
    let (grid, bbox) = grindelwald_grid();
    let dir = tempfile::tempdir().unwrap();
    let (files, _) = write_hgt(&grid, &bbox, Resolution::ArcSecond3, dir.path(), |_| {}).unwrap();

    let data = std::fs::read(&files[0]).unwrap();
    let mut real = Vec::new();
    let mut voids = 0usize;
    for chunk in data.chunks_exact(2) {
        // .hgt is big-endian signed 16-bit.
        let v = i16::from_be_bytes([chunk[0], chunk[1]]);
        if v == VOID {
            voids += 1;
        } else {
            real.push(v);
        }
    }
    assert!(voids > 0, "expected voids outside the tile");
    assert!(!real.is_empty(), "expected real elevations inside the tile");

    let lo = *real.iter().min().unwrap();
    let hi = *real.iter().max().unwrap();
    // The fixture tile is the Grindelwald valley floor and the slope above it.
    assert!(
        (900..=1200).contains(&lo) && (900..=1200).contains(&hi),
        "implausible range {lo}..{hi} for Grindelwald"
    );
}

#[test]
fn voids_are_not_interpolated_into_the_terrain() {
    // A grid with a hole must produce voids there, not a slope down to -9999.
    let cell = Cell {
        e_km: 2600,
        n_km: 1200,
    };
    let samples: Vec<f32> = (0..TILE_PX * TILE_PX)
        .map(|i| {
            let (col, row) = (i % TILE_PX, i / TILE_PX);
            if (100..400).contains(&col) && (100..400).contains(&row) {
                NODATA
            } else {
                1500.0
            }
        })
        .collect();
    let mut tiles = HashMap::new();
    tiles.insert(cell, Tile { cell, samples });
    let grid = Grid::from_tiles(&[cell], &tiles).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let (files, _) = write_hgt(
        &grid,
        &grid.bbox(),
        Resolution::ArcSecond3,
        dir.path(),
        |_| {},
    )
    .unwrap();
    let data = std::fs::read(&files[0]).unwrap();
    for chunk in data.chunks_exact(2) {
        let v = i16::from_be_bytes([chunk[0], chunk[1]]);
        assert!(
            v == VOID || (1400..=1600).contains(&v),
            "value {v} suggests a void was interpolated into the terrain"
        );
    }
}

#[test]
fn dem_dists_match_the_style_level_count_and_resolution() {
    // mkgmap aborts with "More dem-dist values than levels" on a mismatch, and the
    // base differs between 1 and 3 arc-second data.
    let d1 = dem_dists(Resolution::ArcSecond1, 5);
    assert_eq!(d1, vec![3314, 6628, 13256, 26512, 53024]);
    let d3 = dem_dists(Resolution::ArcSecond3, 4);
    assert_eq!(d3, vec![9942, 19884, 39768, 79536]);
    assert_eq!(dem_dists(Resolution::ArcSecond1, 0).len(), 0);
}

#[test]
fn projection_round_trip_is_accurate_enough_for_resampling() {
    // Every .hgt sample is placed by inverse-projecting a WGS84 position, so a
    // round-trip error would smear the terrain.
    for (e, n) in [
        (2_600_000.0, 1_200_000.0),
        (2_645_000.0, 1_163_000.0),
        (2_760_000.0, 1_180_000.0),
    ] {
        let err = s2g_core::dem::projection_round_trip_error(e, n);
        assert!(err < 2.0, "round-trip error {err:.2} m at {e}/{n}");
    }
}
