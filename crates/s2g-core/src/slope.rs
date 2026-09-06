//! Slope classes over 30°, derived from the elevation grid (SPEC.md FR-CART12).
//!
//! swisstopo publishes `ch.swisstopo.hangneigung-ueber_30` as a rendered WMTS layer only
//! — there is no downloadable dataset — so the classes are computed here from the same
//! swissALTI3D data a build already fetches for contours. The class colours *are* taken
//! from that layer, measured rather than chosen (see `docs/palette.md`).
//!
//! Two decisions worth stating:
//!
//! **Computed at 10 m, not at the grid's native 2 m.** Slope over a 2 m baseline is
//! dominated by boulders, road cuttings and tree-canopy artefacts; every scree field
//! comes out mottled with false 45° pixels. Ski-touring practice, and swisstopo's own
//! product, work at around 10 m, where the number means what a skier reads it to mean.
//!
//! **The field is padded with a ring of zero before tracing.** Marching squares emits
//! open chains wherever a region meets the edge of the grid, and closing those by walking
//! the border is fiddly and easy to get subtly wrong. Padding makes every region closed
//! by construction, at the cost of clipping bands to just inside the edge — which is
//! where the map ends anyway.

use crate::elevation::{Grid, NODATA};
use crate::geom::{simplify, Coord};

/// Class boundaries in degrees, matching swisstopo's Hangneigung classes.
pub const DEFAULT_BANDS: [i32; 5] = [30, 35, 40, 45, 50];

#[derive(Debug, Clone)]
pub struct SlopeConfig {
    /// Baseline the gradient is measured over. 10 m by default.
    pub cell_m: f64,
    pub bands: Vec<i32>,
    /// Douglas-Peucker tolerance for the traced rings.
    pub simplify_m: f64,
    /// Rings enclosing fewer than this many cells are dropped, so a single steep sample
    /// does not become a feature. At 10 m cells, 12 is about a tenth of a hectare.
    pub min_cells: usize,
}

impl Default for SlopeConfig {
    fn default() -> Self {
        Self {
            cell_m: 10.0,
            bands: DEFAULT_BANDS.to_vec(),
            simplify_m: 8.0,
            min_cells: 12,
        }
    }
}

/// One traced region: everything inside is at least `min_deg` steep.
#[derive(Debug, Clone)]
pub struct SlopeArea {
    pub min_deg: i32,
    pub ring: Vec<Coord>,
}

#[derive(Debug, Clone, Default)]
pub struct SlopeStats {
    pub areas: usize,
    pub points_before_simplify: usize,
    pub points_after_simplify: usize,
    pub dropped_small: usize,
    /// Cells with no elevation data, treated as flat.
    pub nodata_cells: usize,
    pub cols: usize,
    pub rows: usize,
}

/// A slope field in degrees, aggregated to `cell_m`.
pub struct SlopeField {
    pub origin_e: f64,
    pub origin_n: f64,
    pub cell_m: f64,
    pub cols: usize,
    pub rows: usize,
    /// Row-major from the south-west, like [`Grid`].
    pub degrees: Vec<f32>,
}

impl SlopeField {
    pub fn at(&self, col: usize, row: usize) -> f32 {
        self.degrees[row * self.cols + col]
    }

    /// Mean elevation over each `cell_m` block, then Horn's 3x3 gradient.
    ///
    /// Horn rather than a simple central difference: it weights the diagonal neighbours
    /// and is what GDAL, ArcGIS and swisstopo all use, so a number here is comparable
    /// with a number from those.
    pub fn from_grid(grid: &Grid, cell_m: f64) -> (SlopeField, usize) {
        let step = (cell_m / crate::elevation::TILE_RES_M).round().max(1.0) as usize;
        let cols = grid.cols / step;
        let rows = grid.rows / step;
        let mut nodata = 0usize;

        // Aggregate first: averaging suppresses the sample-scale noise that makes a 2 m
        // slope raster unusable.
        let mut mean = vec![f32::NAN; cols.max(1) * rows.max(1)];
        for r in 0..rows {
            for c in 0..cols {
                let mut sum = 0.0f64;
                let mut n = 0u32;
                for dr in 0..step {
                    for dc in 0..step {
                        let v = grid.at(c * step + dc, r * step + dr);
                        if v > NODATA {
                            sum += v as f64;
                            n += 1;
                        }
                    }
                }
                mean[r * cols + c] = if n > 0 {
                    (sum / n as f64) as f32
                } else {
                    nodata += 1;
                    f32::NAN
                };
            }
        }

        let mut degrees = vec![0.0f32; cols.max(1) * rows.max(1)];
        let get = |c: isize, r: isize| -> f32 {
            let c = c.clamp(0, cols as isize - 1) as usize;
            let r = r.clamp(0, rows as isize - 1) as usize;
            mean[r * cols + c]
        };
        for r in 0..rows as isize {
            for c in 0..cols as isize {
                let z = [
                    get(c - 1, r + 1), get(c, r + 1), get(c + 1, r + 1),
                    get(c - 1, r),     get(c, r),     get(c + 1, r),
                    get(c - 1, r - 1), get(c, r - 1), get(c + 1, r - 1),
                ];
                if z.iter().any(|v| v.is_nan()) {
                    // No data anywhere in the window: report flat rather than a cliff.
                    continue;
                }
                let dzdx = ((z[2] + 2.0 * z[5] + z[8]) - (z[0] + 2.0 * z[3] + z[6])) as f64
                    / (8.0 * cell_m);
                let dzdy = ((z[0] + 2.0 * z[1] + z[2]) - (z[6] + 2.0 * z[7] + z[8])) as f64
                    / (8.0 * cell_m);
                degrees[r as usize * cols + c as usize] =
                    (dzdx * dzdx + dzdy * dzdy).sqrt().atan().to_degrees() as f32;
            }
        }

        (
            SlopeField {
                origin_e: grid.origin_e,
                origin_n: grid.origin_n,
                cell_m,
                cols,
                rows,
                degrees,
            },
            nodata,
        )
    }

    fn coord(&self, col: f64, row: f64) -> Coord {
        Coord::new(
            self.origin_e + (col + 0.5) * self.cell_m,
            self.origin_n + (row + 0.5) * self.cell_m,
        )
    }
}

/// Trace closed rings around every region at or above each band.
pub fn areas(grid: &Grid, cfg: &SlopeConfig) -> (Vec<SlopeArea>, SlopeStats) {
    let (field, nodata) = SlopeField::from_grid(grid, cfg.cell_m);
    let mut out = Vec::new();
    let mut stats = SlopeStats {
        nodata_cells: nodata,
        cols: field.cols,
        rows: field.rows,
        ..Default::default()
    };
    if field.cols < 3 || field.rows < 3 {
        return (out, stats);
    }

    for &band in &cfg.bands {
        for ring in trace(&field, band as f32) {
            // Shoelace area in cells, to drop specks.
            let area_cells = (shoelace(&ring) / (cfg.cell_m * cfg.cell_m)).abs();
            if area_cells < cfg.min_cells as f64 {
                stats.dropped_small += 1;
                continue;
            }
            stats.points_before_simplify += ring.len();
            let mut simplified = simplify(&ring, cfg.simplify_m);
            // Simplification can collapse a ring below a usable polygon.
            if simplified.len() < 4 {
                stats.dropped_small += 1;
                continue;
            }
            // A polygon way must close.
            if simplified.first() != simplified.last() {
                simplified.push(simplified[0]);
            }
            stats.points_after_simplify += simplified.len();
            out.push(SlopeArea {
                min_deg: band,
                ring: simplified,
            });
        }
    }
    stats.areas = out.len();
    (out, stats)
}

fn shoelace(ring: &[Coord]) -> f64 {
    let mut sum = 0.0;
    for w in ring.windows(2) {
        sum += w[0].e * w[1].n - w[1].e * w[0].n;
    }
    sum / 2.0
}

/// Marching squares over the padded field, chained into closed rings.
fn trace(field: &SlopeField, threshold: f32) -> Vec<Vec<Coord>> {
    // Padding by one cell of zero puts every region strictly inside, so every chain
    // closes and no border walking is needed.
    let cols = field.cols + 2;
    let rows = field.rows + 2;
    let value = |c: isize, r: isize| -> f32 {
        if c < 1 || r < 1 || c > field.cols as isize || r > field.rows as isize {
            0.0
        } else {
            field.at((c - 1) as usize, (r - 1) as usize)
        }
    };

    // Segments keyed by quantised endpoints, so chaining is a hash join.
    let q = |p: (f64, f64)| ((p.0 * 64.0).round() as i64, (p.1 * 64.0).round() as i64);
    let mut segments: Vec<((f64, f64), (f64, f64))> = Vec::new();

    for r in 0..rows as isize - 1 {
        for c in 0..cols as isize - 1 {
            // Corner values, counter-clockwise from the south-west.
            let (sw, se, ne, nw) = (
                value(c, r),
                value(c + 1, r),
                value(c + 1, r + 1),
                value(c, r + 1),
            );
            let case = (sw >= threshold) as u8
                | (((se >= threshold) as u8) << 1)
                | (((ne >= threshold) as u8) << 2)
                | (((nw >= threshold) as u8) << 3);
            if case == 0 || case == 15 {
                continue;
            }

            // Interpolated crossings on each edge, in grid coordinates.
            let lerp = |a: f32, b: f32| -> f64 {
                let d = (b - a) as f64;
                if d.abs() < 1e-9 {
                    0.5
                } else {
                    ((threshold - a) as f64 / d).clamp(0.0, 1.0)
                }
            };
            let cf = c as f64;
            let rf = r as f64;
            let south = (cf + lerp(sw, se), rf);
            let east = (cf + 1.0, rf + lerp(se, ne));
            let north = (cf + lerp(nw, ne), rf + 1.0);
            let west = (cf, rf + lerp(sw, nw));

            // Segments are oriented so the inside (>= threshold) is on the left, which
            // makes every traced ring wind consistently.
            let mut push = |a: (f64, f64), b: (f64, f64)| segments.push((a, b));
            match case {
                1 => push(west, south),
                2 => push(south, east),
                3 => push(west, east),
                4 => push(east, north),
                5 => {
                    // Saddle: resolved by the cell average, as the contour tracer does.
                    if (sw + se + ne + nw) / 4.0 >= threshold {
                        push(west, north);
                        push(east, south);
                    } else {
                        push(west, south);
                        push(east, north);
                    }
                }
                6 => push(south, north),
                7 => push(west, north),
                8 => push(north, west),
                9 => push(north, south),
                10 => {
                    if (sw + se + ne + nw) / 4.0 >= threshold {
                        push(north, east);
                        push(south, west);
                    } else {
                        push(north, west);
                        push(south, east);
                    }
                }
                11 => push(north, east),
                12 => push(east, west),
                13 => push(east, south),
                14 => push(south, west),
                _ => {}
            }
        }
    }

    // Chain: follow each segment's end to the segment starting there.
    let mut by_start: std::collections::HashMap<(i64, i64), Vec<usize>> = Default::default();
    for (i, (a, _)) in segments.iter().enumerate() {
        by_start.entry(q(*a)).or_default().push(i);
    }
    let mut used = vec![false; segments.len()];
    let mut rings = Vec::new();

    for start in 0..segments.len() {
        if used[start] {
            continue;
        }
        let mut ring = vec![segments[start].0, segments[start].1];
        used[start] = true;
        let first = q(segments[start].0);
        let mut cursor = q(segments[start].1);

        while cursor != first {
            let Some(next) = by_start
                .get(&cursor)
                .and_then(|ids| ids.iter().find(|i| !used[**i]).copied())
            else {
                break; // Should not happen with a padded field, but never loop forever.
            };
            used[next] = true;
            ring.push(segments[next].1);
            cursor = q(segments[next].1);
        }

        if cursor == first && ring.len() >= 4 {
            ring.push(ring[0]);
            rings.push(
                ring.iter()
                    .map(|(c, r)| field.coord(*c - 1.0, *r - 1.0))
                    .collect(),
            );
        }
    }
    rings
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grid tilted by a known angle, so the computed slope can be checked against
    /// trigonometry rather than against itself.
    fn ramp(cols: usize, rows: usize, degrees: f64) -> Grid {
        let rise = degrees.to_radians().tan() * crate::elevation::TILE_RES_M;
        Grid {
            origin_e: 2_600_000.0,
            origin_n: 1_200_000.0,
            cols,
            rows,
            samples: (0..rows)
                .flat_map(|_r| (0..cols).map(move |c| (c as f64 * rise) as f32))
                .collect(),
        }
    }

    #[test]
    fn slope_of_a_known_ramp_matches_trigonometry() {
        for expect in [10.0, 30.0, 45.0, 60.0] {
            let grid = ramp(200, 200, expect);
            let (field, nodata) = SlopeField::from_grid(&grid, 10.0);
            assert_eq!(nodata, 0);
            // Away from the clamped edges, every cell is the ramp angle.
            let mid = field.at(field.cols / 2, field.rows / 2);
            assert!(
                (mid as f64 - expect).abs() < 0.5,
                "expected {expect}°, computed {mid}°"
            );
        }
    }

    #[test]
    fn flat_ground_has_no_slope_and_no_areas() {
        let grid = ramp(120, 120, 0.0);
        let (field, _) = SlopeField::from_grid(&grid, 10.0);
        assert!(field.degrees.iter().all(|d| *d < 0.01));

        let (areas, stats) = areas(&grid, &SlopeConfig::default());
        assert!(areas.is_empty(), "flat ground produced {} areas", areas.len());
        assert_eq!(stats.dropped_small, 0);
    }

    #[test]
    fn aggregation_reduces_the_grid_to_the_requested_cell_size() {
        let grid = ramp(500, 500, 20.0);
        let (field, _) = SlopeField::from_grid(&grid, 10.0);
        // 500 samples at 2 m is 1000 m, which is 100 cells at 10 m.
        assert_eq!(field.cols, 100);
        assert_eq!(field.rows, 100);
        assert_eq!(field.cell_m, 10.0);
    }

    /// A steep circular cone inside flat ground: exactly one ring per band it reaches.
    #[test]
    fn a_cone_produces_one_closed_ring_per_band() {
        let (cols, rows) = (200usize, 200usize);
        let (cx, cy) = (100.0f64, 100.0f64);
        let samples: Vec<f32> = (0..rows)
            .flat_map(|r| {
                (0..cols).map(move |c| {
                    let d = ((c as f64 - cx).powi(2) + (r as f64 - cy).powi(2)).sqrt();
                    // A cone: constant slope inside the radius, flat outside.
                    let radius = 60.0;
                    if d < radius {
                        ((radius - d) * crate::elevation::TILE_RES_M * 45f64.to_radians().tan())
                            as f32
                    } else {
                        0.0
                    }
                })
            })
            .collect();
        let grid = Grid {
            origin_e: 2_600_000.0,
            origin_n: 1_200_000.0,
            cols,
            rows,
            samples,
        };

        let (found, stats) = areas(&grid, &SlopeConfig::default());
        assert!(!found.is_empty(), "a 45° cone must produce slope areas");
        assert!(stats.cols > 0 && stats.rows > 0);

        for a in &found {
            assert!(
                a.ring.len() >= 4,
                "a polygon needs at least four points, got {}",
                a.ring.len()
            );
            assert_eq!(
                a.ring.first(),
                a.ring.last(),
                "every ring must close, band {}",
                a.min_deg
            );
        }
        // The 30° and 40° bands are both reached; 50° is not, since the cone is 45°.
        let bands: std::collections::BTreeSet<i32> = found.iter().map(|a| a.min_deg).collect();
        assert!(bands.contains(&30), "{bands:?}");
        assert!(bands.contains(&40), "{bands:?}");
        assert!(!bands.contains(&50), "a 45° cone must not reach the 50° band: {bands:?}");
    }

    #[test]
    fn no_data_is_treated_as_flat_rather_than_as_a_cliff() {
        let mut grid = ramp(120, 120, 0.0);
        // A hole in the middle: without care, the step from NODATA to 0 reads as
        // vertical and paints a false 90° patch.
        for r in 40..60 {
            for c in 40..60 {
                grid.samples[r * 120 + c] = NODATA;
            }
        }
        let (field, nodata) = SlopeField::from_grid(&grid, 10.0);
        assert!(nodata > 0, "the hole must be counted");
        assert!(
            field.degrees.iter().all(|d| *d < 1.0),
            "no-data produced a false slope of {}°",
            field.degrees.iter().cloned().fold(0.0, f32::max)
        );
    }

    /// A single wild sample must not paint a steep patch.
    ///
    /// Aggregation is what removes it, before the size filter is ever consulted: one
    /// 500 m spike averaged over a 5x5 block of 2 m samples is a 20 m rise across a
    /// 10 m cell, which Horn reads as about 27° — under the first band. That is the
    /// reason to compute at 10 m rather than at 2 m, stated as a test.
    #[test]
    fn a_single_wild_sample_does_not_become_a_steep_patch() {
        let mut grid = ramp(120, 120, 0.0);
        grid.samples[60 * 120 + 60] = 500.0;

        let (found, _) = areas(&grid, &SlopeConfig::default());
        assert!(found.is_empty(), "a single spike became {} areas", found.len());

        // At 2 m the same spike is a cliff, which is what makes a native-resolution
        // slope raster unusable.
        let (native, _) = SlopeField::from_grid(&grid, 2.0);
        let steepest = native.degrees.iter().cloned().fold(0.0f32, f32::max);
        assert!(steepest > 80.0, "expected a false cliff at 2 m, got {steepest}°");
    }

    /// A genuinely steep but tiny area is dropped by the size filter.
    #[test]
    fn a_region_smaller_than_the_minimum_is_dropped() {
        // A 40 m square at 45°, which is 16 cells of 10 m: real, but far too small to
        // be worth a feature on a device screen.
        let (cols, rows) = (120usize, 120usize);
        let rise = 45f64.to_radians().tan() * crate::elevation::TILE_RES_M;
        let samples: Vec<f32> = (0..rows)
            .flat_map(|r| {
                (0..cols).map(move |c| {
                    let inside = (55..75).contains(&c) && (55..75).contains(&r);
                    if inside {
                        ((c - 55) as f64 * rise) as f32
                    } else {
                        0.0
                    }
                })
            })
            .collect();
        let grid = Grid {
            origin_e: 2_600_000.0,
            origin_n: 1_200_000.0,
            cols,
            rows,
            samples,
        };

        let strict = SlopeConfig {
            min_cells: 10_000,
            ..Default::default()
        };
        let (found, stats) = areas(&grid, &strict);
        assert!(found.is_empty(), "{} areas survived a huge minimum", found.len());
        assert!(stats.dropped_small > 0, "nothing was reported as dropped");

        // With the default minimum the same patch is kept.
        let (kept, _) = areas(&grid, &SlopeConfig::default());
        assert!(!kept.is_empty(), "the patch should survive the default minimum");
    }

    #[test]
    fn a_grid_too_small_to_aggregate_yields_nothing_rather_than_panicking() {
        let grid = ramp(4, 4, 40.0);
        let (found, stats) = areas(&grid, &SlopeConfig::default());
        assert!(found.is_empty());
        assert!(stats.cols < 3 || stats.rows < 3);
    }
}
