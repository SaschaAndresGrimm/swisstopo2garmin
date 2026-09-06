//! Area masking by an arbitrary shape (SPEC.md FR-34, FR-39).
//!
//! The extractor clips to a bounding box, which is all a rectangle or a place radius
//! needs. An administrative unit or a route corridor is not a rectangle: the bounding
//! box of canton Valais is more than twice its area, and the bounding box of a
//! transalpine route is most of Switzerland. A mask filters what survives the bbox
//! clip, so the box stays the cheap first cut and the shape does the rest.
//!
//! Both mask kinds answer one question — "is this coordinate inside?" — through a
//! uniform grid built once per build. A linear scan over canton boundary segments per
//! feature would be far too slow at 21.7 M features.

use crate::geom::{point_in_polygon, Coord, Geometry, Ring};
use crate::proj::BBox;

/// Grid cell size. 500 m keeps the index small for a national extent (720 × 460
/// cells) while leaving few segments per cell in dense boundary geometry.
const CELL_M: f64 = 500.0;

/// A shape that features must intersect to be kept.
pub enum Mask {
    /// Union of polygons, each as exterior ring plus holes. Administrative units.
    Polygons(PolygonMask),
    /// Everything within `radius_m` of any polyline. Route corridors.
    Corridor(CorridorMask),
}

impl Mask {
    pub fn polygons(polygons: Vec<Vec<Ring>>) -> Self {
        Mask::Polygons(PolygonMask::new(polygons))
    }

    pub fn corridor(lines: Vec<Vec<Coord>>, radius_m: f64) -> Self {
        Mask::Corridor(CorridorMask::new(lines, radius_m))
    }

    pub fn contains(&self, p: Coord) -> bool {
        match self {
            Mask::Polygons(m) => m.contains(p),
            Mask::Corridor(m) => m.contains(p),
        }
    }

    /// The mask's own extent, which is the bounding box a build should clip to.
    pub fn bbox(&self) -> BBox {
        match self {
            Mask::Polygons(m) => m.bbox,
            Mask::Corridor(m) => m.bbox,
        }
    }

    /// Whether any part of a geometry is inside.
    ///
    /// Tests vertices rather than exact geometric intersection. A polygon larger than
    /// the mask whose vertices all lie outside would be dropped; that is only reachable
    /// with features far bigger than the selected area, where dropping them is right
    /// anyway. Checking every vertex, rather than a representative point, is what keeps
    /// a long road that merely passes through the corridor.
    pub fn intersects(&self, geom: &Geometry) -> bool {
        geom.coords().any(|c| self.contains(c))
    }
}

/// Grid of cell -> indices, keyed by integer cell coordinates.
struct Grid {
    bbox: BBox,
    /// Cell size in metres. Grown from [`CELL_M`] rather than capping coverage, so a
    /// larger extent stays fully indexed and only loses resolution.
    cell_m: f64,
    cols: usize,
    rows: usize,
    cells: Vec<Vec<u32>>,
}

impl Grid {
    fn new(bbox: BBox) -> Self {
        // Cap the grid at 4 M cells so a huge or non-finite extent cannot allocate the
        // world, and enlarge the cells to keep covering it.
        const MAX_PER_AXIS: f64 = 2_000.0;
        let (w, h) = (bbox.max_e - bbox.min_e, bbox.max_n - bbox.min_n);
        let cell_m = if w.is_finite() && h.is_finite() {
            CELL_M.max(w.max(h) / MAX_PER_AXIS)
        } else {
            CELL_M
        };
        let span = |extent: f64| {
            let n = (extent / cell_m).ceil();
            if n.is_finite() {
                n.clamp(1.0, MAX_PER_AXIS) as usize
            } else {
                1
            }
        };
        let cols = span(w);
        let rows = span(h);
        Self {
            bbox,
            cell_m,
            cols,
            rows,
            cells: vec![Vec::new(); cols * rows],
        }
    }

    fn cell_of(&self, p: Coord) -> Option<(usize, usize)> {
        if p.e < self.bbox.min_e || p.e > self.bbox.max_e || p.n < self.bbox.min_n || p.n > self.bbox.max_n {
            return None;
        }
        let cx = (((p.e - self.bbox.min_e) / self.cell_m) as usize).min(self.cols - 1);
        let cy = (((p.n - self.bbox.min_n) / self.cell_m) as usize).min(self.rows - 1);
        Some((cx, cy))
    }

    /// Cell range covering a box, clamped to the grid.
    fn range_of(&self, b: &BBox) -> Option<(usize, usize, usize, usize)> {
        if b.max_e < self.bbox.min_e
            || b.min_e > self.bbox.max_e
            || b.max_n < self.bbox.min_n
            || b.min_n > self.bbox.max_n
        {
            return None;
        }
        let cell = self.cell_m;
        let x0 = (((b.min_e - self.bbox.min_e) / cell).floor().max(0.0) as usize).min(self.cols - 1);
        let x1 = (((b.max_e - self.bbox.min_e) / cell).floor().max(0.0) as usize).min(self.cols - 1);
        let y0 = (((b.min_n - self.bbox.min_n) / cell).floor().max(0.0) as usize).min(self.rows - 1);
        let y1 = (((b.max_n - self.bbox.min_n) / cell).floor().max(0.0) as usize).min(self.rows - 1);
        Some((x0, y0, x1, y1))
    }

    fn insert(&mut self, b: &BBox, index: u32) {
        if let Some((x0, y0, x1, y1)) = self.range_of(b) {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    self.cells[y * self.cols + x].push(index);
                }
            }
        }
    }

    fn at(&self, cx: usize, cy: usize) -> &[u32] {
        &self.cells[cy * self.cols + cx]
    }
}

/// Union of polygons with a grid over their bounding boxes.
pub struct PolygonMask {
    polygons: Vec<Vec<Ring>>,
    bbox: BBox,
    grid: Grid,
}

impl PolygonMask {
    pub fn new(polygons: Vec<Vec<Ring>>) -> Self {
        let mut min_e = f64::INFINITY;
        let mut min_n = f64::INFINITY;
        let mut max_e = f64::NEG_INFINITY;
        let mut max_n = f64::NEG_INFINITY;
        // Plain accumulators: BBox::new normalises min against max, which would turn
        // infinite seed values into an infinite box rather than an empty one.
        let boxes: Vec<BBox> = polygons
            .iter()
            .map(|rings| {
                let (mut e0, mut n0, mut e1, mut n1) =
                    (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
                for c in rings.iter().flatten() {
                    e0 = e0.min(c.e);
                    n0 = n0.min(c.n);
                    e1 = e1.max(c.e);
                    n1 = n1.max(c.n);
                }
                min_e = min_e.min(e0);
                min_n = min_n.min(n0);
                max_e = max_e.max(e1);
                max_n = max_n.max(n1);
                BBox::new(e0, n0, e1, n1)
            })
            .collect();

        // An empty mask must not produce a NaN extent.
        let bbox = if polygons.is_empty() {
            BBox::new(0.0, 0.0, 0.0, 0.0)
        } else {
            BBox::new(min_e, min_n, max_e, max_n)
        };
        let mut grid = Grid::new(bbox);
        for (i, b) in boxes.iter().enumerate() {
            grid.insert(b, i as u32);
        }
        Self {
            polygons,
            bbox,
            grid,
        }
    }

    pub fn contains(&self, p: Coord) -> bool {
        let Some((cx, cy)) = self.grid.cell_of(p) else {
            return false;
        };
        self.grid
            .at(cx, cy)
            .iter()
            .any(|i| point_in_polygon(p, &self.polygons[*i as usize]))
    }

    pub fn bbox(&self) -> BBox {
        self.bbox
    }
}

/// Everything within a radius of a set of polylines.
pub struct CorridorMask {
    /// Flattened segments as (a, b), indexed by the grid.
    segments: Vec<(Coord, Coord)>,
    radius_m: f64,
    bbox: BBox,
    grid: Grid,
}

impl CorridorMask {
    pub fn new(lines: Vec<Vec<Coord>>, radius_m: f64) -> Self {
        let mut segments = Vec::new();
        for line in &lines {
            for w in line.windows(2) {
                segments.push((w[0], w[1]));
            }
            // A single-point track is still a valid corridor centre.
            if line.len() == 1 {
                segments.push((line[0], line[0]));
            }
        }

        let mut min_e = f64::INFINITY;
        let mut min_n = f64::INFINITY;
        let mut max_e = f64::NEG_INFINITY;
        let mut max_n = f64::NEG_INFINITY;
        for (a, b) in &segments {
            for c in [a, b] {
                min_e = min_e.min(c.e);
                min_n = min_n.min(c.n);
                max_e = max_e.max(c.e);
                max_n = max_n.max(c.n);
            }
        }
        let bbox = if segments.is_empty() {
            BBox::new(0.0, 0.0, 0.0, 0.0)
        } else {
            // The corridor extends a radius beyond the track itself.
            BBox::new(
                min_e - radius_m,
                min_n - radius_m,
                max_e + radius_m,
                max_n + radius_m,
            )
        };

        let mut grid = Grid::new(bbox);
        for (i, (a, b)) in segments.iter().enumerate() {
            // Index each segment over its own box grown by the radius, so a lookup
            // only has to search the cell the query point falls in.
            let sb = BBox::new(
                a.e.min(b.e) - radius_m,
                a.n.min(b.n) - radius_m,
                a.e.max(b.e) + radius_m,
                a.n.max(b.n) + radius_m,
            );
            grid.insert(&sb, i as u32);
        }

        Self {
            segments,
            radius_m,
            bbox,
            grid,
        }
    }

    pub fn contains(&self, p: Coord) -> bool {
        let Some((cx, cy)) = self.grid.cell_of(p) else {
            return false;
        };
        let r2 = self.radius_m * self.radius_m;
        self.grid.at(cx, cy).iter().any(|i| {
            let (a, b) = self.segments[*i as usize];
            point_segment_distance_sq(p, a, b) <= r2
        })
    }

    pub fn bbox(&self) -> BBox {
        self.bbox
    }

    /// Total track length in metres, for the route readout (FR-40).
    pub fn track_length_m(&self) -> f64 {
        self.segments.iter().map(|(a, b)| a.distance(b)).sum()
    }
}

/// Squared distance from a point to a segment. Squared to avoid a square root per test.
fn point_segment_distance_sq(p: Coord, a: Coord, b: Coord) -> f64 {
    let (dx, dy) = (b.e - a.e, b.n - a.n);
    let len2 = dx * dx + dy * dy;
    let (cx, cy) = if len2 <= f64::EPSILON {
        (a.e, a.n)
    } else {
        let t = (((p.e - a.e) * dx + (p.n - a.n) * dy) / len2).clamp(0.0, 1.0);
        (a.e + t * dx, a.n + t * dy)
    };
    let (ex, en) = (p.e - cx, p.n - cy);
    ex * ex + en * en
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(e: f64, n: f64, size: f64) -> Vec<Ring> {
        vec![vec![
            Coord::new(e, n),
            Coord::new(e + size, n),
            Coord::new(e + size, n + size),
            Coord::new(e, n + size),
            Coord::new(e, n),
        ]]
    }

    #[test]
    fn a_polygon_mask_accepts_inside_and_rejects_outside() {
        let m = Mask::polygons(vec![square(2_600_000.0, 1_200_000.0, 10_000.0)]);
        assert!(m.contains(Coord::new(2_605_000.0, 1_205_000.0)));
        assert!(!m.contains(Coord::new(2_615_000.0, 1_205_000.0)));
        // Outside the grid entirely.
        assert!(!m.contains(Coord::new(2_500_000.0, 1_100_000.0)));
    }

    #[test]
    fn a_hole_is_not_inside() {
        let mut rings = square(2_600_000.0, 1_200_000.0, 10_000.0);
        // Inner ring, wound the other way.
        rings.push(vec![
            Coord::new(2_603_000.0, 1_203_000.0),
            Coord::new(2_603_000.0, 1_207_000.0),
            Coord::new(2_607_000.0, 1_207_000.0),
            Coord::new(2_607_000.0, 1_203_000.0),
            Coord::new(2_603_000.0, 1_203_000.0),
        ]);
        let m = Mask::polygons(vec![rings]);
        assert!(m.contains(Coord::new(2_601_000.0, 1_201_000.0)));
        assert!(!m.contains(Coord::new(2_605_000.0, 1_205_000.0)), "the hole is inside");
    }

    #[test]
    fn disjoint_polygons_are_a_union() {
        let m = Mask::polygons(vec![
            square(2_600_000.0, 1_200_000.0, 5_000.0),
            square(2_650_000.0, 1_250_000.0, 5_000.0),
        ]);
        assert!(m.contains(Coord::new(2_602_000.0, 1_202_000.0)));
        assert!(m.contains(Coord::new(2_652_000.0, 1_252_000.0)));
        assert!(!m.contains(Coord::new(2_630_000.0, 1_230_000.0)));
        // The union's extent spans both.
        let b = m.bbox();
        assert_eq!((b.min_e, b.max_e), (2_600_000.0, 2_655_000.0));
    }

    #[test]
    fn a_corridor_accepts_within_the_radius_only() {
        let line = vec![
            Coord::new(2_600_000.0, 1_200_000.0),
            Coord::new(2_610_000.0, 1_200_000.0),
        ];
        let m = Mask::corridor(vec![line], 1_000.0);
        // Beside the middle of the segment.
        assert!(m.contains(Coord::new(2_605_000.0, 1_200_900.0)));
        assert!(!m.contains(Coord::new(2_605_000.0, 1_201_100.0)));
        // Beyond the end, inside the radius: the cap is round, not square.
        assert!(m.contains(Coord::new(2_610_900.0, 1_200_000.0)));
        assert!(!m.contains(Coord::new(2_610_800.0, 1_200_800.0)));
    }

    #[test]
    fn the_corridor_extent_includes_the_radius() {
        let m = Mask::corridor(
            vec![vec![
                Coord::new(2_600_000.0, 1_200_000.0),
                Coord::new(2_601_000.0, 1_200_000.0),
            ]],
            2_500.0,
        );
        let b = m.bbox();
        assert_eq!(b.min_e, 2_597_500.0);
        assert_eq!(b.max_e, 2_603_500.0);
        assert_eq!(b.min_n, 1_197_500.0);
    }

    #[test]
    fn corridor_length_is_the_track_length() {
        let m = CorridorMask::new(
            vec![vec![
                Coord::new(2_600_000.0, 1_200_000.0),
                Coord::new(2_603_000.0, 1_200_000.0),
                Coord::new(2_603_000.0, 1_204_000.0),
            ]],
            500.0,
        );
        assert_eq!(m.track_length_m(), 7_000.0);
    }

    /// A road crossing the corridor must be kept even though most of it is outside.
    #[test]
    fn a_line_that_merely_crosses_the_corridor_is_kept() {
        let m = Mask::corridor(
            vec![vec![
                Coord::new(2_600_000.0, 1_200_000.0),
                Coord::new(2_620_000.0, 1_200_000.0),
            ]],
            500.0,
        );
        let road = Geometry::LineString(vec![
            Coord::new(2_610_000.0, 1_180_000.0),
            Coord::new(2_610_000.0, 1_200_000.0),
            Coord::new(2_610_000.0, 1_220_000.0),
        ]);
        assert!(m.intersects(&road));

        let far = Geometry::LineString(vec![
            Coord::new(2_700_000.0, 1_180_000.0),
            Coord::new(2_700_000.0, 1_220_000.0),
        ]);
        assert!(!m.intersects(&far));
    }

    #[test]
    fn an_empty_mask_contains_nothing_and_has_a_finite_extent() {
        let m = Mask::polygons(vec![]);
        assert!(!m.contains(Coord::new(2_600_000.0, 1_200_000.0)));
        assert!(m.bbox().min_e.is_finite());

        let c = Mask::corridor(vec![], 1_000.0);
        assert!(!c.contains(Coord::new(2_600_000.0, 1_200_000.0)));
        assert!(c.bbox().max_n.is_finite());
    }

    #[test]
    fn a_single_point_track_is_a_disc() {
        let m = Mask::corridor(vec![vec![Coord::new(2_600_000.0, 1_200_000.0)]], 1_000.0);
        assert!(m.contains(Coord::new(2_600_500.0, 1_200_500.0)));
        assert!(!m.contains(Coord::new(2_601_000.0, 1_200_900.0)));
    }

    /// A national extent must stay fully indexed, with larger cells rather than
    /// partial coverage.
    #[test]
    fn a_national_extent_indexes_its_far_corner() {
        let b = crate::proj::LV95_BOUNDS;
        let m = Mask::polygons(vec![square(b.0, b.1, b.2 - b.0)]);
        // A point near the far corner of the square must still be found.
        assert!(m.contains(Coord::new(b.2 - 1_000.0, b.1 + 1_000.0)));
        assert!(!m.contains(Coord::new(b.2 + 1_000.0, b.1 + 1_000.0)));
    }

    /// The grid must not change the answer, only the speed.
    #[test]
    fn a_mask_spanning_many_grid_cells_stays_correct() {
        // 60 km square: 120 x 120 cells at 500 m.
        let m = Mask::polygons(vec![square(2_580_000.0, 1_140_000.0, 60_000.0)]);
        for i in 0..60 {
            let p = Coord::new(2_580_500.0 + i as f64 * 1_000.0, 1_140_500.0 + i as f64 * 1_000.0);
            assert!(m.contains(p), "{p:?} should be inside");
        }
        assert!(!m.contains(Coord::new(2_579_000.0, 1_170_000.0)));
        assert!(!m.contains(Coord::new(2_641_000.0, 1_170_000.0)));
    }
}
