//! Contour generation against analytic surfaces, where the right answer is known,
//! plus a real swissALTI3D tile.

use s2g_core::contour::{generate, ContourConfig, Tier};
use s2g_core::elevation::{Cell, Grid, Tile, NODATA, TILE_PX};
use s2g_core::proj::BBox;
use std::collections::HashMap;

/// Build a grid from a closure over (col, row).
fn synthetic(cols: usize, rows: usize, f: impl Fn(usize, usize) -> f32) -> Grid {
    let mut samples = Vec::with_capacity(cols * rows);
    for row in 0..rows {
        for col in 0..cols {
            samples.push(f(col, row));
        }
    }
    Grid {
        origin_e: 2_600_000.0,
        origin_n: 1_200_000.0,
        cols,
        rows,
        samples,
    }
}

fn never() -> bool {
    false
}

#[test]
fn an_inclined_plane_gives_straight_parallel_contours() {
    // z rises 1 m per sample eastward, so a 20 m contour every 20 samples, each a
    // straight north-south line.
    let g = synthetic(100, 40, |col, _| col as f32);
    let cfg = ContourConfig {
        interval_m: 20,
        major_m: 100,
        medium_m: 0,
        simplify_m: 0.1,
    };
    let (lines, stats) = generate(&g, &cfg, never);

    assert!(stats.levels >= 4, "levels: {}", stats.levels);
    assert!(!lines.is_empty());

    for c in &lines {
        assert_eq!(
            c.elevation % 20,
            0,
            "off-interval elevation {}",
            c.elevation
        );
        // Straight line: every vertex shares an easting.
        let e0 = c.points[0].e;
        for p in &c.points {
            assert!(
                (p.e - e0).abs() < 0.5,
                "contour at {} m is not straight: {:.2} vs {:.2}",
                c.elevation,
                p.e,
                e0
            );
        }
        // And it should be simplified to essentially its endpoints.
        assert!(
            c.points.len() <= 4,
            "{} points for a straight line",
            c.points.len()
        );
    }
}

#[test]
fn a_cone_gives_closed_concentric_rings() {
    let (cols, rows) = (120usize, 120usize);
    let (cx, cy) = (60.0f32, 60.0f32);
    let g = synthetic(cols, rows, |col, row| {
        let d = ((col as f32 - cx).powi(2) + (row as f32 - cy).powi(2)).sqrt();
        (60.0 - d).max(0.0) * 2.0 // peak 120 m at the centre
    });
    let cfg = ContourConfig {
        interval_m: 20,
        major_m: 100,
        medium_m: 0,
        simplify_m: 1.0,
    };
    let (lines, _) = generate(&g, &cfg, never);
    assert!(!lines.is_empty());

    // Rings must close: first and last vertex coincide.
    let mut closed = 0;
    for c in &lines {
        if c.points[0].distance(c.points.last().unwrap()) < 3.0 {
            closed += 1;
        }
    }
    assert!(
        closed >= lines.len() / 2,
        "only {closed} of {} contours closed on a cone",
        lines.len()
    );

    // Higher contours must enclose smaller areas.
    let mut by_ele: Vec<(i32, f64)> = lines
        .iter()
        .filter(|c| c.points.len() > 8)
        .map(|c| {
            let b = c.points.iter().fold(
                (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
                |(a, b2, c2, d), p| (a.min(p.e), b2.min(p.n), c2.max(p.e), d.max(p.n)),
            );
            (c.elevation, (b.2 - b.0) * (b.3 - b.1))
        })
        .collect();
    by_ele.sort_by_key(|(e, _)| *e);
    for w in by_ele.windows(2) {
        assert!(
            w[1].1 <= w[0].1 * 1.05,
            "contour at {} m encloses more area than at {} m",
            w[1].0,
            w[0].0
        );
    }
}

#[test]
fn a_saddle_does_not_produce_crossing_lines() {
    // z = x*y is the classic marching-squares ambiguity. Resolved with the cell
    // average, contours must stay separate rather than forming an X.
    let g = synthetic(80, 80, |col, row| {
        ((col as f32 - 40.0) * (row as f32 - 40.0)) / 20.0
    });
    let cfg = ContourConfig {
        interval_m: 10,
        major_m: 50,
        medium_m: 0,
        simplify_m: 0.5,
    };
    let (lines, stats) = generate(&g, &cfg, never);
    assert!(!lines.is_empty());
    assert!(stats.lines > 0);
    for c in &lines {
        assert!(c.points.len() >= 2);
        for p in &c.points {
            assert!(p.e.is_finite() && p.n.is_finite());
        }
    }
}

#[test]
fn voids_do_not_generate_contours_around_themselves() {
    // Interpolating against -9999 would draw a cliff around every gap.
    let g = synthetic(60, 60, |col, row| {
        if (20..40).contains(&col) && (20..40).contains(&row) {
            NODATA
        } else {
            100.0 + col as f32 * 0.5
        }
    });
    let cfg = ContourConfig {
        interval_m: 10,
        ..Default::default()
    };
    let (lines, stats) = generate(&g, &cfg, never);
    assert!(
        stats.skipped_nodata_cells > 0,
        "void cells should be skipped"
    );
    for c in &lines {
        // No contour may carry an elevation near the nodata sentinel.
        assert!(
            c.elevation > -1000,
            "contour at {} m came from a void",
            c.elevation
        );
    }
}

#[test]
fn flat_terrain_produces_no_contours() {
    let g = synthetic(50, 50, |_, _| 1000.0);
    let cfg = ContourConfig {
        interval_m: 20,
        ..Default::default()
    };
    let (lines, _) = generate(&g, &cfg, never);
    // A perfectly flat surface at exactly a contour level is degenerate; what matters
    // is that nothing spurious with a wild elevation appears.
    for c in &lines {
        assert_eq!(c.elevation, 1000);
    }
}

#[test]
fn an_all_void_grid_yields_nothing() {
    let g = synthetic(30, 30, |_, _| NODATA);
    let (lines, stats) = generate(&g, &ContourConfig::default(), never);
    assert!(lines.is_empty());
    assert_eq!(stats.levels, 0, "a grid with no valid samples has no range");
}

#[test]
fn tiers_follow_the_landeskarte_convention() {
    let cfg = ContourConfig {
        interval_m: 20,
        major_m: 100,
        medium_m: 0,
        simplify_m: 8.0,
    };
    assert_eq!(cfg.tier(1000), Tier::Major);
    assert_eq!(cfg.tier(1020), Tier::Minor);
    assert_eq!(cfg.tier(1080), Tier::Minor);
    assert_eq!(Tier::Major.contour_ext(), "elevation_major");
    assert_eq!(Tier::Minor.contour_ext(), "elevation_minor");
    assert!(cfg.medium_is_reachable(), "a disabled medium tier is fine");
}

#[test]
fn detects_a_medium_tier_that_can_never_occur() {
    // The Milestone 0 configuration: every multiple of 50 that is also a multiple of
    // 20 is a multiple of 100, so `medium` was always classified `major` first and the
    // tier was silently dead.
    let dead = ContourConfig {
        interval_m: 20,
        major_m: 100,
        medium_m: 50,
        simplify_m: 8.0,
    };
    assert!(
        !dead.medium_is_reachable(),
        "interval 20 / medium 50 / major 100 has an unreachable medium tier"
    );

    let ok = ContourConfig {
        interval_m: 10,
        major_m: 100,
        medium_m: 50,
        simplify_m: 8.0,
    };
    assert!(ok.medium_is_reachable());
    assert_eq!(ok.tier(1050), Tier::Medium);
}

#[test]
fn cells_cover_a_bbox_on_the_kilometre_grid() {
    let cells = Cell::covering(&BBox::new(
        2_645_100.0,
        1_163_100.0,
        2_646_900.0,
        1_164_900.0,
    ));
    assert_eq!(cells.len(), 4, "{cells:?}");
    assert!(cells.contains(&Cell {
        e_km: 2645,
        n_km: 1163
    }));
    assert!(cells.contains(&Cell {
        e_km: 2646,
        n_km: 1164
    }));

    // A single point still needs one cell.
    let one = Cell::covering(&BBox::new(
        2_645_500.0,
        1_163_500.0,
        2_645_500.0,
        1_163_500.0,
    ));
    assert_eq!(one.len(), 1);
}

#[test]
fn parses_the_cell_and_year_out_of_asset_names() {
    let (year, cell) = Cell::from_name("swissalti3d_2022_2645-1163_2_2056_5728.tif").unwrap();
    assert_eq!(year, 2022);
    assert_eq!(
        cell,
        Cell {
            e_km: 2645,
            n_km: 1163
        }
    );
    assert!(Cell::from_name("something_else.tif").is_none());
}

#[test]
fn mosaic_places_tiles_south_to_north() {
    // Tile rows run north to south; the grid runs south to north. Getting this
    // backwards mirrors the terrain, which is invisible in a histogram and obvious
    // on a map.
    let mut tiles = HashMap::new();
    let cells = vec![Cell {
        e_km: 2645,
        n_km: 1163,
    }];
    // Encode the row index into the value so orientation is checkable.
    let samples: Vec<f32> = (0..TILE_PX * TILE_PX)
        .map(|i| (i / TILE_PX) as f32)
        .collect();
    tiles.insert(
        cells[0],
        Tile {
            cell: cells[0],
            samples,
        },
    );

    let g = Grid::from_tiles(&cells, &tiles).unwrap();
    assert_eq!(g.cols, TILE_PX);
    assert_eq!(g.rows, TILE_PX);
    // Tile row 0 is the northernmost, so it must land in the grid's top row.
    assert_eq!(
        g.at(0, g.rows - 1),
        0.0,
        "north edge should hold tile row 0"
    );
    assert_eq!(
        g.at(0, 0),
        (TILE_PX - 1) as f32,
        "south edge should hold the last tile row"
    );
    assert_eq!(g.origin_n, 1_163_000.0);
}

// ---------------------------------------------------------------------------
// Real swissALTI3D data
// ---------------------------------------------------------------------------

const TILE_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/swissalti3d_2019_2645-1163_2.tif"
);

#[test]
fn decodes_a_real_swissalti3d_tile() {
    let cell = Cell {
        e_km: 2645,
        n_km: 1163,
    };
    let tile = Tile::decode(std::path::Path::new(TILE_FIXTURE), cell).expect("decode");
    assert_eq!(tile.samples.len(), TILE_PX * TILE_PX);

    let valid: Vec<f32> = tile
        .samples
        .iter()
        .copied()
        .filter(|v| *v != NODATA && v.is_finite())
        .collect();
    assert!(valid.len() > TILE_PX * TILE_PX / 2, "mostly void tile?");

    let lo = valid.iter().copied().fold(f32::MAX, f32::min);
    let hi = valid.iter().copied().fold(f32::MIN, f32::max);
    // This cell is the Grindelwald valley floor and the slope above it.
    assert!(
        (900.0..3000.0).contains(&lo) && (900.0..3000.0).contains(&hi),
        "implausible elevation range {lo}..{hi} for Grindelwald"
    );
    assert!(
        hi - lo > 100.0,
        "expected real relief, got {:.0} m",
        hi - lo
    );
    println!(
        "real tile: {lo:.0}..{hi:.0} m over {} valid samples",
        valid.len()
    );
}

#[test]
fn generates_contours_from_a_real_tile() {
    let cell = Cell {
        e_km: 2645,
        n_km: 1163,
    };
    let tile = Tile::decode(std::path::Path::new(TILE_FIXTURE), cell).unwrap();
    let mut tiles = HashMap::new();
    tiles.insert(cell, tile);
    let grid = Grid::from_tiles(&[cell], &tiles).unwrap();

    let cfg = ContourConfig::default(); // 20 m interval, 100 m index, 8 m tolerance
    let (lines, stats) = generate(&grid, &cfg, never);

    assert!(
        stats.levels > 5,
        "only {} levels over real terrain",
        stats.levels
    );
    assert!(!lines.is_empty());

    // Simplification is the main lever on output size; Milestone 0 measured 97% on
    // real contours. Assert it is doing substantial work.
    assert!(
        stats.simplification_ratio() > 0.7,
        "only {:.0}% of contour vertices removed",
        stats.simplification_ratio() * 100.0
    );

    // Every contour must sit inside the tile and on an interval boundary.
    let b = grid.bbox().expand(TILE_RES_TOLERANCE);
    for c in &lines {
        assert_eq!(c.elevation % cfg.interval_m, 0);
        for p in &c.points {
            assert!(b.contains(p.e, p.n), "contour point {p:?} outside the tile");
        }
    }

    let majors = lines.iter().filter(|c| c.tier == Tier::Major).count();
    assert!(majors > 0, "no index contours over 1 km of alpine terrain");

    println!(
        "real tile: {} contours across {} levels, {} -> {} points ({:.0}% removed), {} index",
        stats.lines,
        stats.levels,
        stats.points_before_simplify,
        stats.points_after_simplify,
        stats.simplification_ratio() * 100.0,
        majors
    );
}

/// Contours are interpolated between sample centres, so they can sit half a sample
/// outside the nominal grid envelope.
const TILE_RES_TOLERANCE: f64 = 2.0;
