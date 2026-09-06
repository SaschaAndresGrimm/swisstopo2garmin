//! Geometry types and operations in LV95 metres.
//!
//! Coordinates are projected metres throughout the pipeline; reprojection to WGS84
//! happens once, at the point of writing OSM output. Doing geometry in metres keeps
//! tolerances (simplification, buffering) meaningful and avoids latitude-dependent
//! distortion.

use crate::proj::BBox;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord {
    pub e: f64,
    pub n: f64,
}

impl Coord {
    pub fn new(e: f64, n: f64) -> Self {
        Self { e, n }
    }
    pub fn distance(&self, other: &Coord) -> f64 {
        (self.e - other.e).hypot(self.n - other.n)
    }
}

pub type Ring = Vec<Coord>;

#[derive(Debug, Clone, PartialEq)]
pub enum Geometry {
    Point(Coord),
    MultiPoint(Vec<Coord>),
    LineString(Vec<Coord>),
    MultiLineString(Vec<Vec<Coord>>),
    /// Exterior ring first, then interior rings (holes).
    Polygon(Vec<Ring>),
    MultiPolygon(Vec<Vec<Ring>>),
}

impl Geometry {
    pub fn bbox(&self) -> Option<BBox> {
        let mut it = self.coords();
        let first = it.next()?;
        let (mut min_e, mut min_n, mut max_e, mut max_n) = (first.e, first.n, first.e, first.n);
        for c in it {
            min_e = min_e.min(c.e);
            min_n = min_n.min(c.n);
            max_e = max_e.max(c.e);
            max_n = max_n.max(c.n);
        }
        Some(BBox::new(min_e, min_n, max_e, max_n))
    }

    pub fn coords(&self) -> Box<dyn Iterator<Item = Coord> + '_> {
        match self {
            Geometry::Point(c) => Box::new(std::iter::once(*c)),
            Geometry::MultiPoint(v) => Box::new(v.iter().copied()),
            Geometry::LineString(v) => Box::new(v.iter().copied()),
            Geometry::MultiLineString(v) => Box::new(v.iter().flatten().copied()),
            Geometry::Polygon(r) => Box::new(r.iter().flatten().copied()),
            Geometry::MultiPolygon(p) => Box::new(p.iter().flatten().flatten().copied()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.coords().next().is_none()
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Geometry::Point(_) => "Point",
            Geometry::MultiPoint(_) => "MultiPoint",
            Geometry::LineString(_) => "LineString",
            Geometry::MultiLineString(_) => "MultiLineString",
            Geometry::Polygon(_) => "Polygon",
            Geometry::MultiPolygon(_) => "MultiPolygon",
        }
    }
}

/// Douglas-Peucker simplification.
///
/// This is the single biggest lever on output size: at an 8 m tolerance it removed
/// 97% of contour vertices with no visible change at Garmin's ~2.4 m grid
/// (docs/m0-findings.md §5).
pub fn simplify(points: &[Coord], tolerance: f64) -> Vec<Coord> {
    if tolerance <= 0.0 || points.len() < 3 {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut stack = vec![(0usize, points.len() - 1)];

    while let Some((i, j)) = stack.pop() {
        if j <= i + 1 {
            continue;
        }
        let (a, b) = (points[i], points[j]);
        let (dx, dy) = (b.e - a.e, b.n - a.n);
        let den = dx * dx + dy * dy;
        let mut best = -1.0f64;
        let mut best_idx = 0usize;
        for (k, p) in points.iter().enumerate().take(j).skip(i + 1) {
            let d = if den == 0.0 {
                p.distance(&a)
            } else {
                let t = (((p.e - a.e) * dx + (p.n - a.n) * dy) / den).clamp(0.0, 1.0);
                p.distance(&Coord::new(a.e + t * dx, a.n + t * dy))
            };
            if d > best {
                best = d;
                best_idx = k;
            }
        }
        if best > tolerance {
            keep[best_idx] = true;
            stack.push((i, best_idx));
            stack.push((best_idx, j));
        }
    }
    points
        .iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(*p))
        .collect()
}

/// Signed area of a ring; positive means counter-clockwise.
pub fn signed_area(ring: &[Coord]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        sum += a.e * b.n - b.e * a.n;
    }
    sum / 2.0
}

/// Ray-casting point-in-ring test.
pub fn point_in_ring(p: Coord, ring: &[Coord]) -> bool {
    let mut inside = false;
    let mut j = ring.len().wrapping_sub(1);
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[j]);
        if (a.n > p.n) != (b.n > p.n) {
            let x = (b.e - a.e) * (p.n - a.n) / (b.n - a.n) + a.e;
            if p.e < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// True if the point lies inside the polygon's exterior ring and outside its holes.
pub fn point_in_polygon(p: Coord, rings: &[Ring]) -> bool {
    let Some(exterior) = rings.first() else {
        return false;
    };
    if !point_in_ring(p, exterior) {
        return false;
    }
    !rings[1..].iter().any(|hole| point_in_ring(p, hole))
}

// ---------------------------------------------------------------------------
// Clipping
// ---------------------------------------------------------------------------

/// Clip a polyline to a rectangle, returning the pieces that fall inside.
///
/// A line crossing the boundary is cut, not dropped: dropping it leaves visible
/// gaps at tile edges (SPEC.md §7.3).
pub fn clip_line(points: &[Coord], bbox: &BBox) -> Vec<Vec<Coord>> {
    let mut out = Vec::new();
    let mut current: Vec<Coord> = Vec::new();

    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        match clip_segment(a, b, bbox) {
            None => {
                if current.len() > 1 {
                    out.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
            }
            Some((ca, cb)) => {
                if current.is_empty() {
                    current.push(ca);
                } else if current.last() != Some(&ca) {
                    // the previous segment left the box and we have re-entered
                    if current.len() > 1 {
                        out.push(std::mem::take(&mut current));
                    } else {
                        current.clear();
                    }
                    current.push(ca);
                }
                current.push(cb);
            }
        }
    }
    if current.len() > 1 {
        out.push(current);
    }
    out
}

/// Liang-Barsky segment clip.
fn clip_segment(a: Coord, b: Coord, r: &BBox) -> Option<(Coord, Coord)> {
    let (dx, dy) = (b.e - a.e, b.n - a.n);
    let mut t0 = 0.0f64;
    let mut t1 = 1.0f64;
    let checks = [
        (-dx, a.e - r.min_e),
        (dx, r.max_e - a.e),
        (-dy, a.n - r.min_n),
        (dy, r.max_n - a.n),
    ];
    for (p, q) in checks {
        if p == 0.0 {
            if q < 0.0 {
                return None; // parallel and outside
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                if t > t1 {
                    return None;
                }
                t0 = t0.max(t);
            } else {
                if t < t0 {
                    return None;
                }
                t1 = t1.min(t);
            }
        }
    }
    Some((
        Coord::new(a.e + t0 * dx, a.n + t0 * dy),
        Coord::new(a.e + t1 * dx, a.n + t1 * dy),
    ))
}

/// Clip a polygon ring to a rectangle (Sutherland-Hodgman).
///
/// The result stays a closed ring, which is what a filled area needs; clipping it as
/// a line would leave an unclosed outline that renders as a stroke, not a fill.
pub fn clip_ring(ring: &[Coord], bbox: &BBox) -> Vec<Coord> {
    if ring.is_empty() {
        return Vec::new();
    }
    let mut output = ring.to_vec();
    if output.first() == output.last() && output.len() > 1 {
        output.pop();
    }

    for edge in 0..4 {
        if output.is_empty() {
            break;
        }
        let input = std::mem::take(&mut output);
        let inside = |c: &Coord| match edge {
            0 => c.e >= bbox.min_e,
            1 => c.e <= bbox.max_e,
            2 => c.n >= bbox.min_n,
            _ => c.n <= bbox.max_n,
        };
        let intersect = |a: Coord, b: Coord| -> Coord {
            let (dx, dy) = (b.e - a.e, b.n - a.n);
            match edge {
                0 => Coord::new(bbox.min_e, a.n + dy * (bbox.min_e - a.e) / dx),
                1 => Coord::new(bbox.max_e, a.n + dy * (bbox.max_e - a.e) / dx),
                2 => Coord::new(a.e + dx * (bbox.min_n - a.n) / dy, bbox.min_n),
                _ => Coord::new(a.e + dx * (bbox.max_n - a.n) / dy, bbox.max_n),
            }
        };
        for i in 0..input.len() {
            let cur = input[i];
            let prev = input[(i + input.len() - 1) % input.len()];
            let (cur_in, prev_in) = (inside(&cur), inside(&prev));
            if cur_in {
                if !prev_in {
                    output.push(intersect(prev, cur));
                }
                output.push(cur);
            } else if prev_in {
                output.push(intersect(prev, cur));
            }
        }
    }

    if output.len() >= 3 {
        output.push(output[0]); // re-close
    } else {
        output.clear();
    }
    output
}

/// Clip a whole geometry, dropping anything that falls entirely outside.
pub fn clip(geom: &Geometry, bbox: &BBox) -> Option<Geometry> {
    match geom {
        Geometry::Point(c) => bbox.contains(c.e, c.n).then_some(Geometry::Point(*c)),
        Geometry::MultiPoint(v) => {
            let kept: Vec<_> = v
                .iter()
                .copied()
                .filter(|c| bbox.contains(c.e, c.n))
                .collect();
            (!kept.is_empty()).then_some(Geometry::MultiPoint(kept))
        }
        Geometry::LineString(v) => {
            let parts = clip_line(v, bbox);
            match parts.len() {
                0 => None,
                1 => Some(Geometry::LineString(parts.into_iter().next().unwrap())),
                _ => Some(Geometry::MultiLineString(parts)),
            }
        }
        Geometry::MultiLineString(v) => {
            let parts: Vec<_> = v.iter().flat_map(|l| clip_line(l, bbox)).collect();
            (!parts.is_empty()).then_some(Geometry::MultiLineString(parts))
        }
        Geometry::Polygon(rings) => {
            let clipped: Vec<Ring> = rings
                .iter()
                .map(|r| clip_ring(r, bbox))
                .filter(|r| r.len() >= 4)
                .collect();
            // A polygon whose exterior clipped away has nothing to fill.
            (!clipped.is_empty()).then_some(Geometry::Polygon(clipped))
        }
        Geometry::MultiPolygon(polys) => {
            let kept: Vec<Vec<Ring>> = polys
                .iter()
                .map(|rings| {
                    rings
                        .iter()
                        .map(|r| clip_ring(r, bbox))
                        .filter(|r| r.len() >= 4)
                        .collect::<Vec<Ring>>()
                })
                .filter(|rings: &Vec<Ring>| !rings.is_empty())
                .collect();
            (!kept.is_empty()).then_some(Geometry::MultiPolygon(kept))
        }
    }
}
