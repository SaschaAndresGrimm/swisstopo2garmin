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
    /// Several masks combined (SPEC.md FR-41).
    ///
    /// A point is inside if any part contains it. Kept as a list rather than merged
    /// into one geometry: merging polygons is real work to get right, and a union
    /// answers the only question a mask is ever asked.
    Union(Vec<Mask>),
    /// A polygon set grown outwards by a distance (SPEC.md FR-34).
    ///
    /// Exact Minkowski growth by a disc, not an approximation: a point is inside if it
    /// is inside the polygons *or* within the distance of their boundary. That is
    /// precisely what the two masks above already answer between them, so buffering
    /// needs no polygon offsetting — which is fiddly to get right on self-touching
    /// rings, and cantonal boundaries have plenty of those.
    BufferedPolygons {
        inside: PolygonMask,
        edge: CorridorMask,
    },
}

impl Mask {
    pub fn polygons(polygons: Vec<Vec<Ring>>) -> Self {
        Mask::Polygons(PolygonMask::new(polygons))
    }

    pub fn corridor(lines: Vec<Vec<Coord>>, radius_m: f64) -> Self {
        Mask::Corridor(CorridorMask::new(lines, radius_m))
    }

    /// Polygons grown outwards by `buffer_m`. Zero buffer gives a plain polygon mask,
    /// so a caller need not special-case "no buffer".
    pub fn polygons_buffered(polygons: Vec<Vec<Ring>>, buffer_m: f64) -> Self {
        if buffer_m <= 0.0 {
            return Mask::polygons(polygons);
        }
        // Every ring of every polygon becomes a polyline; the corridor around them is
        // the band straddling the boundary, and the polygon mask fills the interior.
        let rings: Vec<Vec<Coord>> = polygons.iter().flatten().cloned().collect();
        Mask::BufferedPolygons {
            inside: PolygonMask::new(polygons),
            edge: CorridorMask::new(rings, buffer_m),
        }
    }

    /// Combine masks. A single part is returned as itself rather than wrapped.
    pub fn union(mut parts: Vec<Mask>) -> Self {
        if parts.len() == 1 {
            return parts.remove(0);
        }
        Mask::Union(parts)
    }

    pub fn contains(&self, p: Coord) -> bool {
        match self {
            Mask::Union(parts) => parts.iter().any(|m| m.contains(p)),
            Mask::Polygons(m) => m.contains(p),
            Mask::Corridor(m) => m.contains(p),
            Mask::BufferedPolygons { inside, edge } => inside.contains(p) || edge.contains(p),
        }
    }

    /// The mask's own extent, which is the bounding box a build should clip to.
    pub fn bbox(&self) -> BBox {
        match self {
            Mask::Union(parts) => {
                let mut it = parts.iter().map(|m| m.bbox());
                match it.next() {
                    None => BBox::new(0.0, 0.0, 0.0, 0.0),
                    Some(first) => it.fold(first, |a, b| {
                        BBox::new(
                            a.min_e.min(b.min_e),
                            a.min_n.min(b.min_n),
                            a.max_e.max(b.max_e),
                            a.max_n.max(b.max_n),
                        )
                    }),
                }
            }
            Mask::Polygons(m) => m.bbox,
            Mask::Corridor(m) => m.bbox,
            // The edge corridor already includes the buffer on all sides.
            Mask::BufferedPolygons { edge, .. } => edge.bbox,
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
        if p.e < self.bbox.min_e
            || p.e > self.bbox.max_e
            || p.n < self.bbox.min_n
            || p.n > self.bbox.max_n
        {
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
        let x0 =
            (((b.min_e - self.bbox.min_e) / cell).floor().max(0.0) as usize).min(self.cols - 1);
        let x1 =
            (((b.max_e - self.bbox.min_e) / cell).floor().max(0.0) as usize).min(self.cols - 1);
        let y0 =
            (((b.min_n - self.bbox.min_n) / cell).floor().max(0.0) as usize).min(self.rows - 1);
        let y1 =
            (((b.max_n - self.bbox.min_n) / cell).floor().max(0.0) as usize).min(self.rows - 1);
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
///
/// The grid narrows a lookup to the polygons whose box covers the point, but a canton is
/// *one* polygon with 14,000 vertices, so that alone still walks every edge on every
/// lookup — 15 µs each, and a build tests every vertex of every feature.
///
/// So a second grid indexes the ring segments, and a cell that no segment crosses is
/// entirely inside or entirely outside. Its verdict is computed once, by the same full
/// test, and cached; only cells the boundary actually crosses pay the full price. The
/// answer is identical either way, which `the_edge_cache_agrees_with_the_slow_path`
/// checks exhaustively.
pub struct PolygonMask {
    polygons: Vec<Vec<Ring>>,
    bbox: BBox,
    grid: Grid,
    /// Cells crossed by a ring segment; those cannot be answered from the cache.
    edges: Grid,
    /// Per-cell verdict for edge-free cells: 0 unknown, 1 inside, 2 outside. Atomic so
    /// the mask stays `Sync`, which the build future requires.
    verdict: Vec<std::sync::atomic::AtomicU8>,
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
                let (mut e0, mut n0, mut e1, mut n1) = (
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                );
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

        // Index the ring segments, so an edge-free cell can be recognised.
        let mut edges = Grid::new(bbox);
        for (i, rings) in polygons.iter().enumerate() {
            for ring in rings {
                for w in ring.windows(2) {
                    let sb = BBox::new(
                        w[0].e.min(w[1].e),
                        w[0].n.min(w[1].n),
                        w[0].e.max(w[1].e),
                        w[0].n.max(w[1].n),
                    );
                    edges.insert(&sb, i as u32);
                }
            }
        }
        let verdict = (0..edges.cols * edges.rows)
            .map(|_| std::sync::atomic::AtomicU8::new(0))
            .collect();

        Self {
            polygons,
            bbox,
            grid,
            edges,
            verdict,
        }
    }

    pub fn contains(&self, p: Coord) -> bool {
        use std::sync::atomic::Ordering;

        let Some((cx, cy)) = self.grid.cell_of(p) else {
            return false;
        };
        let cell = cy * self.edges.cols + cx;
        let boundary_here = !self.edges.at(cx, cy).is_empty();

        if !boundary_here {
            match self.verdict[cell].load(Ordering::Relaxed) {
                1 => return true,
                2 => return false,
                _ => {}
            }
        }

        let answer = self
            .grid
            .at(cx, cy)
            .iter()
            .any(|i| point_in_polygon(p, &self.polygons[*i as usize]));

        // Only an edge-free cell is uniform, so only that verdict may be cached.
        if !boundary_here {
            self.verdict[cell].store(if answer { 1 } else { 2 }, Ordering::Relaxed);
        }
        answer
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
        assert!(
            !m.contains(Coord::new(2_605_000.0, 1_205_000.0)),
            "the hole is inside"
        );
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
            let p = Coord::new(
                2_580_500.0 + i as f64 * 1_000.0,
                1_140_500.0 + i as f64 * 1_000.0,
            );
            assert!(m.contains(p), "{p:?} should be inside");
        }
        assert!(!m.contains(Coord::new(2_579_000.0, 1_170_000.0)));
        assert!(!m.contains(Coord::new(2_641_000.0, 1_170_000.0)));
    }
}

#[cfg(test)]
mod buffer_tests {
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
    fn a_buffer_extends_the_shape_outwards_by_the_distance() {
        let poly = square(2_600_000.0, 1_200_000.0, 10_000.0);
        let m = Mask::polygons_buffered(vec![poly], 2_000.0);

        // Deep inside stays inside.
        assert!(m.contains(Coord::new(2_605_000.0, 1_205_000.0)));
        // Just outside the edge, within the buffer.
        assert!(m.contains(Coord::new(2_611_900.0, 1_205_000.0)));
        // Beyond the buffer.
        assert!(!m.contains(Coord::new(2_612_100.0, 1_205_000.0)));
        // Diagonally past a corner: the growth is a disc, so the corner is rounded and
        // 2 km diagonally out is further than 2 km.
        assert!(!m.contains(Coord::new(2_611_600.0, 1_211_600.0)));
        assert!(m.contains(Coord::new(2_611_200.0, 1_200_000.0)));
    }

    #[test]
    fn a_zero_buffer_is_a_plain_polygon_mask() {
        let m = Mask::polygons_buffered(vec![square(2_600_000.0, 1_200_000.0, 5_000.0)], 0.0);
        assert!(matches!(m, Mask::Polygons(_)));
        assert!(m.contains(Coord::new(2_602_000.0, 1_202_000.0)));
        assert!(!m.contains(Coord::new(2_606_000.0, 1_202_000.0)));
    }

    #[test]
    fn the_buffered_extent_includes_the_buffer() {
        let m = Mask::polygons_buffered(vec![square(2_600_000.0, 1_200_000.0, 5_000.0)], 1_500.0);
        let b = m.bbox();
        assert_eq!(b.min_e, 2_598_500.0);
        assert_eq!(b.max_e, 2_606_500.0);
        assert_eq!(b.min_n, 1_198_500.0);
        assert_eq!(b.max_n, 1_206_500.0);
    }

    /// A hole must stay a hole, but its rim gets the buffer too — the buffer grows the
    /// shape outwards everywhere, which around a hole means inwards.
    #[test]
    fn a_hole_shrinks_by_the_buffer_rather_than_vanishing() {
        let mut rings = square(2_600_000.0, 1_200_000.0, 20_000.0);
        rings.push(vec![
            Coord::new(2_605_000.0, 1_205_000.0),
            Coord::new(2_605_000.0, 1_215_000.0),
            Coord::new(2_615_000.0, 1_215_000.0),
            Coord::new(2_615_000.0, 1_205_000.0),
            Coord::new(2_605_000.0, 1_205_000.0),
        ]);
        let m = Mask::polygons_buffered(vec![rings], 1_000.0);

        // The middle of the hole is still out.
        assert!(!m.contains(Coord::new(2_610_000.0, 1_210_000.0)));
        // Just inside the hole's rim is within the buffer of the boundary, so in.
        assert!(m.contains(Coord::new(2_605_500.0, 1_210_000.0)));
    }

    /// Two disjoint units selected together must both be kept, with their buffers.
    #[test]
    fn several_polygons_are_buffered_as_one_union() {
        let m = Mask::polygons_buffered(
            vec![
                square(2_600_000.0, 1_200_000.0, 5_000.0),
                square(2_620_000.0, 1_200_000.0, 5_000.0),
            ],
            1_000.0,
        );
        assert!(m.contains(Coord::new(2_602_000.0, 1_202_000.0)));
        assert!(m.contains(Coord::new(2_622_000.0, 1_202_000.0)));
        // Between them, outside both buffers.
        assert!(!m.contains(Coord::new(2_612_000.0, 1_202_000.0)));
        // Just outside the first, inside its buffer.
        assert!(m.contains(Coord::new(2_605_500.0, 1_202_000.0)));
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    /// The per-cell cache must never change an answer, only the time it takes.
    ///
    /// Checked exhaustively against a from-scratch mask on a shape with a concave notch
    /// and a hole, so cells that are inside the bounding box but outside the polygon,
    /// and cells inside the hole, are both covered.
    #[test]
    fn the_edge_cache_agrees_with_the_slow_path() {
        let outer = vec![
            Coord::new(2_600_000.0, 1_200_000.0),
            Coord::new(2_620_000.0, 1_200_000.0),
            Coord::new(2_620_000.0, 1_220_000.0),
            // A deep notch, so part of the bounding box is outside the shape.
            Coord::new(2_612_000.0, 1_220_000.0),
            Coord::new(2_612_000.0, 1_206_000.0),
            Coord::new(2_608_000.0, 1_206_000.0),
            Coord::new(2_608_000.0, 1_220_000.0),
            Coord::new(2_600_000.0, 1_220_000.0),
            Coord::new(2_600_000.0, 1_200_000.0),
        ];
        let hole = vec![
            Coord::new(2_602_000.0, 1_202_000.0),
            Coord::new(2_602_000.0, 1_205_000.0),
            Coord::new(2_605_000.0, 1_205_000.0),
            Coord::new(2_605_000.0, 1_202_000.0),
            Coord::new(2_602_000.0, 1_202_000.0),
        ];
        let polygon = vec![outer, hole];

        let cached = PolygonMask::new(vec![polygon.clone()]);
        let reference = PolygonMask::new(vec![polygon]);

        // Sample finely enough to land inside cells, on boundaries, and outside.
        let mut checked = 0;
        let mut n = 1_199_000.0;
        while n <= 1_221_000.0 {
            let mut e = 2_599_000.0;
            while e <= 2_621_000.0 {
                let p = Coord::new(e, n);
                // Ask the cached mask twice: the second call takes the cached path.
                let first = cached.contains(p);
                let second = cached.contains(p);
                let slow = {
                    // A fresh mask has an empty cache, so this is always the full test.
                    PolygonMask::new(reference.polygons.clone()).contains(p)
                };
                assert_eq!(first, slow, "at {e},{n}");
                assert_eq!(second, slow, "cached answer differs at {e},{n}");
                checked += 1;
                e += 700.0;
            }
            n += 700.0;
        }
        assert!(checked > 900, "only {checked} points checked");
    }

    /// The second pass over the same points must be much faster than the first.
    #[test]
    fn caching_actually_saves_work_on_a_dense_polygon() {
        // A circle approximated by many segments, like a real boundary.
        let ring: Vec<Coord> = (0..=4_000)
            .map(|i| {
                let a = i as f64 / 4_000.0 * std::f64::consts::TAU;
                Coord::new(
                    2_610_000.0 + 9_000.0 * a.cos(),
                    1_210_000.0 + 9_000.0 * a.sin(),
                )
            })
            .collect();
        let mask = PolygonMask::new(vec![vec![ring]]);

        let probe = |mask: &PolygonMask| {
            let started = std::time::Instant::now();
            let mut inside = 0;
            let mut n = 1_201_000.0;
            while n < 1_219_000.0 {
                let mut e = 2_601_000.0;
                while e < 2_619_000.0 {
                    if mask.contains(Coord::new(e, n)) {
                        inside += 1;
                    }
                    e += 250.0;
                }
                n += 250.0;
            }
            (inside, started.elapsed())
        };

        let (first_inside, cold) = probe(&mask);
        let (second_inside, warm) = probe(&mask);

        assert_eq!(first_inside, second_inside, "the answer must not change");
        assert!(
            first_inside > 1_000,
            "expected many points inside the circle"
        );
        assert!(
            warm < cold / 2,
            "cache saved nothing: cold {cold:?}, warm {warm:?}"
        );
    }
}
