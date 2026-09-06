//! Contour generation by marching squares (SPEC.md §7.4, FR-P6..FR-P8).
//!
//! Contours are the largest single contributor to output size, so simplification is
//! not optional: Milestone 0 measured a 97% vertex reduction at an 8 m tolerance with
//! no visible change at Garmin's ~2.4 m grid.
//!
//! Continuity across tile boundaries (FR-P7) comes for free here, because contouring
//! runs over a whole mosaic rather than per tile. There is no seam to stitch.

use std::collections::HashMap;

use crate::elevation::{Grid, NODATA};
use crate::geom::{simplify, Coord};

/// Which weight a contour is drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Minor,
    Medium,
    /// Index contour: heavier, and the only tier that gets labelled.
    Major,
}

impl Tier {
    /// The tagging convention mkgmap styles already expect, so third-party styles
    /// remain compatible (FR-P6).
    pub fn contour_ext(&self) -> &'static str {
        match self {
            Tier::Minor => "elevation_minor",
            Tier::Medium => "elevation_medium",
            Tier::Major => "elevation_major",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ContourConfig {
    pub interval_m: i32,
    /// Index contour spacing. The Landeskarte uses 100 m over a 20 m interval.
    pub major_m: i32,
    /// Intermediate tier. 0 disables it.
    ///
    /// Milestone 0 shipped interval=20 / medium=50 / major=100, where the medium tier
    /// could never occur: every multiple of 50 that is also a multiple of 20 is a
    /// multiple of 100, so it was always classified major first. Default to disabled
    /// rather than silently dead.
    pub medium_m: i32,
    /// Douglas-Peucker tolerance in metres.
    pub simplify_m: f64,
}

impl Default for ContourConfig {
    fn default() -> Self {
        Self {
            interval_m: 20,
            major_m: 100,
            medium_m: 0,
            simplify_m: 8.0,
        }
    }
}

impl ContourConfig {
    pub fn tier(&self, ele: i32) -> Tier {
        if self.major_m > 0 && ele % self.major_m == 0 {
            Tier::Major
        } else if self.medium_m > 0 && ele % self.medium_m == 0 {
            Tier::Medium
        } else {
            Tier::Minor
        }
    }

    /// Warn about a configuration whose medium tier can never be reached.
    pub fn medium_is_reachable(&self) -> bool {
        if self.medium_m <= 0 {
            return true; // disabled on purpose
        }
        // Reachable if some multiple of the interval hits medium but not major.
        (1..=1000)
            .map(|k| k * self.interval_m)
            .any(|e| e % self.medium_m == 0 && (self.major_m <= 0 || e % self.major_m != 0))
    }
}

#[derive(Debug, Clone)]
pub struct Contour {
    pub elevation: i32,
    pub tier: Tier,
    pub points: Vec<Coord>,
}

#[derive(Debug, Clone, Default)]
pub struct ContourStats {
    pub lines: usize,
    pub points_before_simplify: usize,
    pub points_after_simplify: usize,
    pub levels: usize,
    pub skipped_nodata_cells: usize,
}

impl ContourStats {
    pub fn simplification_ratio(&self) -> f64 {
        if self.points_before_simplify == 0 {
            return 0.0;
        }
        1.0 - self.points_after_simplify as f64 / self.points_before_simplify as f64
    }
}

/// Quantised endpoint key, used to join marching-squares segments into polylines.
/// 1 mm resolution is far finer than the 2 m sample spacing, so distinct crossings
/// never collide, while identical ones always match.
type Key = (i64, i64);

fn key(c: Coord) -> Key {
    ((c.e * 1000.0).round() as i64, (c.n * 1000.0).round() as i64)
}

/// Generate contours over the whole grid.
pub fn generate(
    grid: &Grid,
    cfg: &ContourConfig,
    mut cancel: impl FnMut() -> bool,
) -> (Vec<Contour>, ContourStats) {
    let mut out = Vec::new();
    let mut stats = ContourStats::default();

    let Some((lo, hi)) = grid.range() else {
        return (out, stats);
    };
    let step = cfg.interval_m.max(1);
    let first = (lo as i32).div_euclid(step) * step;
    let last = (hi as i32).div_euclid(step) * step;

    let mut level = first;
    while level <= last {
        if cancel() {
            break;
        }
        stats.levels += 1;
        let segments = march(grid, level as f32, &mut stats);
        for mut points in join(segments) {
            stats.points_before_simplify += points.len();
            points = simplify(&points, cfg.simplify_m);
            stats.points_after_simplify += points.len();
            if points.len() < 2 {
                continue;
            }
            stats.lines += 1;
            out.push(Contour {
                elevation: level,
                tier: cfg.tier(level),
                points,
            });
        }
        level += step;
    }

    (out, stats)
}

/// Marching squares over one level, producing unordered segments.
fn march(grid: &Grid, level: f32, stats: &mut ContourStats) -> Vec<(Coord, Coord)> {
    let mut segs = Vec::new();
    if grid.cols < 2 || grid.rows < 2 {
        return segs;
    }

    for row in 0..grid.rows - 1 {
        for col in 0..grid.cols - 1 {
            let sw = grid.at(col, row);
            let se = grid.at(col + 1, row);
            let nw = grid.at(col, row + 1);
            let ne = grid.at(col + 1, row + 1);

            // A cell touching a void produces no contour: interpolating against
            // -9999 would draw a wall around every gap.
            if sw == NODATA || se == NODATA || nw == NODATA || ne == NODATA {
                stats.skipped_nodata_cells += 1;
                continue;
            }

            let mut idx = 0u8;
            if sw >= level {
                idx |= 1;
            }
            if se >= level {
                idx |= 2;
            }
            if ne >= level {
                idx |= 4;
            }
            if nw >= level {
                idx |= 8;
            }
            if idx == 0 || idx == 15 {
                continue;
            }

            let (x0, y0) = grid.coord(col, row);
            let (x1, y1) = grid.coord(col + 1, row + 1);

            // Crossing points on each edge, by linear interpolation.
            let bottom = || Coord::new(x0 + (x1 - x0) * frac(sw, se, level), y0);
            let right = || Coord::new(x1, y0 + (y1 - y0) * frac(se, ne, level));
            let top = || Coord::new(x0 + (x1 - x0) * frac(nw, ne, level), y1);
            let left = || Coord::new(x0, y0 + (y1 - y0) * frac(sw, nw, level));

            // Saddle cases (5 and 10) are resolved with the cell average, which is
            // the standard disambiguation and keeps lines from crossing.
            let avg = (sw + se + ne + nw) / 4.0;

            match idx {
                1 | 14 => segs.push((left(), bottom())),
                2 | 13 => segs.push((bottom(), right())),
                3 | 12 => segs.push((left(), right())),
                4 | 11 => segs.push((right(), top())),
                6 | 9 => segs.push((bottom(), top())),
                7 | 8 => segs.push((left(), top())),
                5 => {
                    if avg >= level {
                        segs.push((left(), top()));
                        segs.push((bottom(), right()));
                    } else {
                        segs.push((left(), bottom()));
                        segs.push((right(), top()));
                    }
                }
                10 => {
                    if avg >= level {
                        segs.push((left(), bottom()));
                        segs.push((right(), top()));
                    } else {
                        segs.push((left(), top()));
                        segs.push((bottom(), right()));
                    }
                }
                _ => {}
            }
        }
    }
    segs
}

fn frac(a: f32, b: f32, level: f32) -> f64 {
    let d = (b - a) as f64;
    if d.abs() < f64::EPSILON {
        0.5
    } else {
        (((level - a) as f64) / d).clamp(0.0, 1.0)
    }
}

/// Chain segments into polylines by matching endpoints.
fn join(segments: Vec<(Coord, Coord)>) -> Vec<Vec<Coord>> {
    let mut adjacency: HashMap<Key, Vec<usize>> = HashMap::new();
    for (i, (a, b)) in segments.iter().enumerate() {
        adjacency.entry(key(*a)).or_default().push(i);
        adjacency.entry(key(*b)).or_default().push(i);
    }

    let mut used = vec![false; segments.len()];
    let mut out = Vec::new();

    for start in 0..segments.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let (a, b) = segments[start];
        let mut line = vec![a, b];

        // Extend forward, then backward, consuming segments as they are attached.
        for direction in 0..2 {
            loop {
                let end = if direction == 0 {
                    *line.last().unwrap()
                } else {
                    line[0]
                };
                let Some(candidates) = adjacency.get(&key(end)) else {
                    break;
                };
                let Some(&next) = candidates.iter().find(|&&i| !used[i]) else {
                    break;
                };
                used[next] = true;
                let (na, nb) = segments[next];
                let far = if key(na) == key(end) { nb } else { na };
                if direction == 0 {
                    line.push(far);
                } else {
                    line.insert(0, far);
                }
            }
        }
        out.push(line);
    }
    out
}
