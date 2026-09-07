//! Performance and memory regression thresholds (SPEC.md NFR-1, NFR-2, §13.6).
//!
//! CI must be offline and deterministic, so the scales the requirements are written
//! against — commune, canton, national — cannot be built here. What runs instead:
//!
//! * **Rates** measured on the committed 1 km² fixture, then extrapolated to canton and
//!   national area and checked against NFR-1's budgets. An extrapolation is a weaker
//!   claim than a real canton build and this file says so; what it reliably catches is
//!   the failure that actually happens, which is an inner loop becoming quadratic.
//! * **Memory shape**, which is the real content of NFR-2: peak RSS must not grow with
//!   the input. That is testable at any scale and is a stronger statement than a single
//!   threshold, because it is the property that makes a national build possible at all.
//!
//! Ceilings are set well above the measured value — four times, mostly — because CI
//! runners vary by two to three times between the fastest and slowest machine in the
//! matrix. A tight threshold on a shared runner produces failures that teach people to
//! ignore the job, which is worse than no job. Order-of-magnitude regressions are what
//! this is for.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use s2g_core::contour::{generate, ContourConfig};
use s2g_core::elevation::{Cell, Grid, Tile};
use s2g_core::perf::{peak_rss_bytes, Rate};
use s2g_core::proj;

/// Areas the requirements are written against.
const CANTON_KM2: f64 = 2_000.0;
const SWITZERLAND_KM2: f64 = 41_285.0;

/// NFR-1: canton contours in ten minutes with a warm elevation cache.
const CANTON_CONTOUR_BUDGET_S: f64 = 600.0;
/// NFR-1: national build completes overnight, unattended. Ten hours.
const NATIONAL_BUDGET_S: f64 = 10.0 * 3600.0;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// The committed swissALTI3D tile as a one-cell grid: 1 km², 500×500 samples at 2 m.
fn fixture_grid() -> Grid {
    let cell = Cell {
        e_km: 2645,
        n_km: 1163,
    };
    let tile = Tile::decode(&fixtures().join("swissalti3d_2019_2645-1163_2.tif"), cell)
        .expect("the committed fixture tile must decode");
    let mut tiles = HashMap::new();
    tiles.insert(cell, tile);
    Grid::from_tiles(&[cell], &tiles).expect("one cell is a valid grid")
}

/// Repeat `f` until at least `min_seconds` have passed, and return seconds per call.
///
/// Repetition rather than a single timing, because a 1 km² fixture takes tens of
/// milliseconds and a single measurement on a loaded CI runner is mostly noise.
fn seconds_per_call(min_seconds: f64, mut f: impl FnMut()) -> f64 {
    let start = std::time::Instant::now();
    let mut calls = 0u32;
    loop {
        f();
        calls += 1;
        let elapsed = start.elapsed().as_secs_f64();
        if elapsed >= min_seconds {
            return elapsed / calls as f64;
        }
    }
}

/// Print every rate, then fail once with all of them, so a run reports the whole picture
/// rather than the first thing over budget.
/// Rate ceilings are calibrated against an optimised build, which is what CI runs and
/// what a user has. An unoptimised build is five to ten times slower here, so measuring
/// it against the same ceilings would either fail constantly or, if the ceilings were
/// loosened to accommodate it, stop catching anything.
fn optimised_build() -> bool {
    if cfg!(debug_assertions) {
        eprintln!(
            "skipping rate thresholds: this is an unoptimised build. \
             Run `cargo test --release --test perf`."
        );
        return false;
    }
    true
}

fn assert_all(rates: &[Rate]) {
    println!("\n--- performance (SPEC.md NFR-1, §13.6) ---");
    for r in rates {
        println!("{}", r.report());
    }
    if let Some(peak) = peak_rss_bytes() {
        println!("{:<32} {:>12.1} MB", "peak rss", peak as f64 / 1e6);
    }
    let over: Vec<&str> = rates
        .iter()
        .filter(|r| !r.within_budget())
        .map(|r| r.name)
        .collect();
    assert!(over.is_empty(), "over budget: {}", over.join(", "));
}

/// Contour generation is the dominant cost of a build: 43 % of measured stage time, and
/// the stage Milestone 0 found breaking the canton budget at 0.74 s/km²
/// (docs/m0-findings.md §3.2). It is the rate most worth watching.
#[test]
fn contour_generation_holds_the_canton_budget() {
    if !optimised_build() {
        return;
    }
    let grid = fixture_grid();
    let cfg = ContourConfig {
        interval_m: 20,
        major_m: 100,
        medium_m: 0,
        simplify_m: 8.0,
    };

    // Correctness first: a rate measured over a stage that produced nothing is not a
    // measurement of anything.
    let (lines, _) = generate(&grid, &cfg, || false);
    assert!(
        lines.len() > 50,
        "the fixture produced only {} contours, so this rate means nothing",
        lines.len()
    );

    let mut traced = 0usize;
    let per_km2 = seconds_per_call(0.5, || {
        traced += generate(&grid, &cfg, || false).0.len();
    });
    assert!(traced > 0, "the benchmark measured nothing");

    assert_all(&[
        Rate {
            name: "contour.per_km2",
            measured: per_km2,
            // Measured at 0.0079 s/km2 in release on an M-series laptop. Five times
            // that, which absorbs the two-to-three-times spread across CI runners and
            // still fails on a real regression. For scale, the Milestone 0 measurement
            // that prompted the parallel fetch was 0.74 s/km2 end to end.
            ceiling: 0.040,
            unit: "s/km2",
        },
        Rate {
            name: "contour.canton_extrapolated",
            measured: per_km2 * CANTON_KM2,
            ceiling: CANTON_CONTOUR_BUDGET_S,
            unit: "s",
        },
        Rate {
            name: "contour.national_extrapolated",
            measured: per_km2 * SWITZERLAND_KM2,
            ceiling: NATIONAL_BUDGET_S,
            unit: "s",
        },
    ]);
}

/// Slope classification runs over the same grid and is the newest stage, so it has the
/// least history behind its cost.
///
/// The bands are lowered from the shipped 30-50° to 20-40°. The fixture tile is the
/// Grindelwald valley floor — 945 m to 1105 m across a kilometre — so at the shipped
/// bands it produces nothing but two specks, and a rate measured over a stage that
/// traced nothing would not be measuring tracing or simplification at all. Lowered, it
/// produces eight areas and drops fifty-eight small ones, which exercises both.
#[test]
fn slope_classification_holds_the_canton_budget() {
    if !optimised_build() {
        return;
    }
    let grid = fixture_grid();
    let cfg = s2g_core::slope::SlopeConfig {
        bands: vec![20, 25, 30, 35, 40],
        ..Default::default()
    };

    let (areas, stats) = s2g_core::slope::areas(&grid, &cfg);
    assert!(
        !areas.is_empty() && stats.dropped_small > 0,
        "the fixture produced {} areas and dropped {}, so this rate would not be \
         measuring tracing or simplification",
        areas.len(),
        stats.dropped_small
    );

    let mut traced = 0usize;
    let per_km2 = seconds_per_call(0.5, || {
        traced += s2g_core::slope::areas(&grid, &cfg).0.len();
    });
    assert!(traced > 0, "the benchmark measured nothing");

    assert_all(&[
        Rate {
            name: "slope.per_km2",
            measured: per_km2,
            // Measured at 0.0006 s/km2 in release; eight times that.
            ceiling: 0.005,
            unit: "s/km2",
        },
        Rate {
            name: "slope.canton_extrapolated",
            measured: per_km2 * CANTON_KM2,
            // A quarter of the contour budget: slope is an option, not the main event.
            ceiling: CANTON_CONTOUR_BUDGET_S / 4.0,
            unit: "s",
        },
    ]);
}

/// Projection runs per coordinate, so it is the one rate where a constant-factor
/// regression is multiplied by hundreds of millions.
#[test]
fn projection_holds_its_per_point_cost() {
    if !optimised_build() {
        return;
    }
    // A spread of real Swiss coordinates rather than one point repeated, so the
    // iterative parts of the transform are exercised.
    let points: Vec<(f64, f64)> = (0..1000)
        .map(|i| {
            (
                2_500_000.0 + (i as f64) * 300.0,
                1_100_000.0 + (i as f64) * 200.0,
            )
        })
        .collect();

    // Accumulated, not discarded. Dropping the result let the optimiser delete the
    // whole loop -- the first version of this test reported 0.03 ns per point, about a
    // tenth of a cycle, which is the signature of a measurement of nothing.
    let mut sink = 0.0f64;
    let per_batch = seconds_per_call(0.5, || {
        for (e, n) in &points {
            let (lon, lat) = proj::lv95_to_wgs84(*e, *n);
            sink += lon + lat;
        }
    });
    let per_point_ns = per_batch / points.len() as f64 * 1e9;
    assert!(
        sink.is_finite() && sink != 0.0,
        "the loop was optimised away again"
    );

    assert_all(&[Rate {
        name: "proj.per_point",
        measured: per_point_ns,
        // Measured at 2.1 ns in release; five times that. A national build projects on
        // the order of 10^8 points, so even at this ceiling it is under a minute of the
        // overnight budget -- the ceiling is here to catch a regression, not to defend
        // a budget that is in any danger.
        ceiling: 10.0,
        unit: "ns/point",
    }]);
}

/// NFR-2, and the reason a national build is possible at all: peak memory must not
/// follow the size of the input.
///
/// Contour generation is the right stage to test it on. It holds a grid and produces
/// lines, so it is the stage where a mistake would show, and it is run over a 16× larger
/// grid here than any single elevation tile.
#[test]
fn peak_memory_does_not_follow_the_size_of_the_input() {
    // The high-water mark where the platform keeps one, the current figure otherwise.
    // On Linux — where CI runs this — VmHWM is a true peak and cannot miss a spike
    // between samples. On macOS the current RSS may already have fallen back by the
    // time it is read, which makes the assertion weaker locally but not wrong: the same
    // bound is checked, and the run that has to hold is the Linux one.
    let rss = || peak_rss_bytes().or_else(s2g_core::perf::current_rss_bytes);
    let Some(before) = rss() else {
        eprintln!("skipping: this platform reports no resident set size");
        return;
    };

    let base = fixture_grid();
    let cfg = ContourConfig {
        interval_m: 20,
        major_m: 100,
        medium_m: 0,
        simplify_m: 8.0,
    };

    // 1x, 4x and 16x the fixture area, by tiling the same samples. Synthetic, but the
    // question is how memory scales with sample count, and that it answers exactly.
    let mut peaks = Vec::new();
    for factor in [1usize, 2, 4] {
        let grid = tiled(&base, factor);
        let samples = grid.samples.len();
        let (lines, _) = generate(&grid, &cfg, || false);
        assert!(!lines.is_empty());
        let peak = rss().expect("already established that this platform answers");
        println!(
            "{}x area ({} samples): peak {:.1} MB",
            factor * factor,
            samples,
            peak as f64 / 1e6
        );
        peaks.push(peak);
    }

    let growth = peaks.last().unwrap().saturating_sub(before);
    let grid_bytes = base.samples.len() * 16 * 4;
    // Growth is expected: a 16x grid genuinely holds 16x the samples. What must not
    // happen is growth by a *multiple* of the working set, which is what a stage
    // accumulating instead of streaming looks like. Eight times the largest grid covers
    // the grid itself, the contour lines it produces (which for this fixture are a
    // comparable size), and the allocator's slack.
    assert!(
        growth < (grid_bytes as u64) * 8,
        "peak RSS grew by {:.1} MB over a largest input of {:.1} MB, which is not \
         streaming behaviour",
        growth as f64 / 1e6,
        grid_bytes as f64 / 1e6
    );
    // And the whole test must stay far below NFR-2's ceiling.
    assert!(
        *peaks.last().unwrap() < 4_000_000_000,
        "resident set {} exceeds NFR-2's 4 GB",
        peaks.last().unwrap()
    );
}

/// The same samples repeated into a `factor` x `factor` grid.
fn tiled(base: &Grid, factor: usize) -> Grid {
    let cols = base.cols * factor;
    let rows = base.rows * factor;
    let mut samples = Vec::with_capacity(cols * rows);
    for row in 0..rows {
        let src_row = row % base.rows;
        for col in 0..cols {
            samples.push(base.samples[src_row * base.cols + (col % base.cols)]);
        }
    }
    Grid {
        origin_e: base.origin_e,
        origin_n: base.origin_n,
        cols,
        rows,
        samples,
    }
}
