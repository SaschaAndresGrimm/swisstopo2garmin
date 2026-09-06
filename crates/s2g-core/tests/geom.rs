//! Clipping, simplification and point-in-polygon, in LV95 metres.

use s2g_core::geom::{
    clip, clip_line, clip_ring, point_in_polygon, point_in_ring, signed_area, simplify, Coord,
    Geometry,
};
use s2g_core::proj::BBox;

fn c(e: f64, n: f64) -> Coord {
    Coord::new(e, n)
}
fn box_10km() -> BBox {
    BBox::new(0.0, 0.0, 10_000.0, 10_000.0)
}

#[test]
fn simplify_removes_collinear_points_and_keeps_the_shape() {
    let line: Vec<Coord> = (0..=100).map(|i| c(i as f64 * 10.0, 0.0)).collect();
    let out = simplify(&line, 1.0);
    assert_eq!(out.len(), 2, "a straight line needs only its endpoints");
    assert_eq!(out[0], line[0]);
    assert_eq!(out[1], *line.last().unwrap());
}

#[test]
fn simplify_respects_the_tolerance() {
    // A single spike of 5 m: kept at a 1 m tolerance, dropped at 10 m.
    let line = vec![c(0.0, 0.0), c(50.0, 5.0), c(100.0, 0.0)];
    assert_eq!(simplify(&line, 1.0).len(), 3);
    assert_eq!(simplify(&line, 10.0).len(), 2);
}

#[test]
fn simplify_never_drops_endpoints_or_returns_fewer_than_two() {
    let line = vec![c(0.0, 0.0), c(1.0, 0.0), c(2.0, 0.0)];
    let out = simplify(&line, 1000.0);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], line[0]);
    assert_eq!(out[1], line[2]);

    // Degenerate inputs pass through untouched.
    assert_eq!(simplify(&[], 5.0).len(), 0);
    assert_eq!(simplify(&[c(1.0, 1.0)], 5.0).len(), 1);
    assert_eq!(
        simplify(&line, 0.0).len(),
        3,
        "zero tolerance disables simplification"
    );
}

#[test]
fn simplify_achieves_the_reduction_measured_in_milestone_0() {
    // Contour-like geometry: dense vertices along a smooth curve. Milestone 0
    // measured a 97% reduction at 8 m on real swissALTI3D contours; a synthetic
    // curve should be in the same ballpark, and certainly not worse than 80%.
    let curve: Vec<Coord> = (0..2000)
        .map(|i| {
            let t = i as f64 * 2.0;
            c(t, (t / 400.0).sin() * 300.0)
        })
        .collect();
    let out = simplify(&curve, 8.0);
    let reduction = 1.0 - out.len() as f64 / curve.len() as f64;
    assert!(reduction > 0.8, "only {:.0}% reduction", reduction * 100.0);
}

#[test]
fn clip_line_cuts_at_the_boundary_instead_of_dropping() {
    // Dropping a line that leaves the box would leave a visible gap at the edge.
    let line = vec![c(-5_000.0, 5_000.0), c(15_000.0, 5_000.0)];
    let parts = clip_line(&line, &box_10km());
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].first().unwrap().e, 0.0);
    assert_eq!(parts[0].last().unwrap().e, 10_000.0);
}

#[test]
fn clip_line_splits_a_line_that_re_enters_the_box() {
    // Out and back in: two separate pieces, not one line bridging the gap.
    let line = vec![
        c(1_000.0, 5_000.0),
        c(-1_000.0, 5_000.0),
        c(-1_000.0, 6_000.0),
        c(1_000.0, 6_000.0),
    ];
    let parts = clip_line(&line, &box_10km());
    assert_eq!(parts.len(), 2, "expected two pieces, got {parts:?}");
    for p in &parts {
        for v in p {
            assert!(v.e >= -0.001, "piece escaped the box: {v:?}");
        }
    }
}

#[test]
fn clip_line_keeps_a_fully_contained_line_intact() {
    let line = vec![c(100.0, 100.0), c(200.0, 300.0), c(400.0, 100.0)];
    let parts = clip_line(&line, &box_10km());
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].len(), 3);
}

#[test]
fn clip_line_drops_a_line_entirely_outside() {
    let line = vec![c(-100.0, -100.0), c(-200.0, -300.0)];
    assert!(clip_line(&line, &box_10km()).is_empty());
}

#[test]
fn clip_ring_stays_closed() {
    // An area must remain a closed ring; clipping it as a line would leave an open
    // outline that renders as a stroke rather than a fill.
    let ring = vec![
        c(-1_000.0, -1_000.0),
        c(5_000.0, -1_000.0),
        c(5_000.0, 5_000.0),
        c(-1_000.0, 5_000.0),
        c(-1_000.0, -1_000.0),
    ];
    let out = clip_ring(&ring, &box_10km());
    assert!(out.len() >= 4, "got {out:?}");
    assert_eq!(out.first(), out.last(), "clipped ring is not closed");
    for v in &out {
        assert!(v.e >= -0.001 && v.n >= -0.001, "vertex escaped: {v:?}");
    }
}

#[test]
fn clip_ring_covering_the_whole_box_becomes_the_box() {
    let ring = vec![
        c(-1e6, -1e6),
        c(1e6, -1e6),
        c(1e6, 1e6),
        c(-1e6, 1e6),
        c(-1e6, -1e6),
    ];
    let out = clip_ring(&ring, &box_10km());
    let bbox = Geometry::Polygon(vec![out.clone()]).bbox().unwrap();
    assert!((bbox.min_e - 0.0).abs() < 0.001);
    assert!((bbox.max_e - 10_000.0).abs() < 0.001);
    assert!((bbox.max_n - 10_000.0).abs() < 0.001);
}

#[test]
fn clip_geometry_dispatches_by_type() {
    let b = box_10km();

    assert!(clip(&Geometry::Point(c(5_000.0, 5_000.0)), &b).is_some());
    assert!(clip(&Geometry::Point(c(50_000.0, 5_000.0)), &b).is_none());

    let mp = Geometry::MultiPoint(vec![c(1.0, 1.0), c(-9.0, -9.0)]);
    match clip(&mp, &b) {
        Some(Geometry::MultiPoint(v)) => assert_eq!(v.len(), 1),
        other => panic!("expected MultiPoint, got {other:?}"),
    }

    // A line that leaves and re-enters must be promoted to a MultiLineString.
    let split = Geometry::LineString(vec![
        c(1_000.0, 500.0),
        c(-1_000.0, 500.0),
        c(-1_000.0, 900.0),
        c(1_000.0, 900.0),
    ]);
    assert!(matches!(clip(&split, &b), Some(Geometry::MultiLineString(v)) if v.len() == 2));

    assert!(clip(
        &Geometry::LineString(vec![c(-5.0, -5.0), c(-9.0, -9.0)]),
        &b
    )
    .is_none());
}

#[test]
fn clip_polygon_preserves_holes_that_survive() {
    let exterior = vec![
        c(1_000.0, 1_000.0),
        c(9_000.0, 1_000.0),
        c(9_000.0, 9_000.0),
        c(1_000.0, 9_000.0),
        c(1_000.0, 1_000.0),
    ];
    let hole = vec![
        c(4_000.0, 4_000.0),
        c(6_000.0, 4_000.0),
        c(6_000.0, 6_000.0),
        c(4_000.0, 6_000.0),
        c(4_000.0, 4_000.0),
    ];
    let poly = Geometry::Polygon(vec![exterior, hole]);
    match clip(&poly, &box_10km()) {
        Some(Geometry::Polygon(rings)) => {
            assert_eq!(rings.len(), 2, "the hole must survive an untouched clip");
        }
        other => panic!("expected Polygon, got {other:?}"),
    }
}

#[test]
fn point_in_polygon_respects_holes() {
    let exterior = vec![c(0.0, 0.0), c(10.0, 0.0), c(10.0, 10.0), c(0.0, 10.0)];
    let hole = vec![c(4.0, 4.0), c(6.0, 4.0), c(6.0, 6.0), c(4.0, 6.0)];
    let rings = vec![exterior, hole];

    assert!(
        point_in_polygon(c(1.0, 1.0), &rings),
        "inside, outside the hole"
    );
    assert!(
        !point_in_polygon(c(5.0, 5.0), &rings),
        "inside the hole is outside"
    );
    assert!(!point_in_polygon(c(-1.0, 5.0), &rings), "outside entirely");
    assert!(
        !point_in_polygon(c(1.0, 1.0), &[]),
        "no rings means no interior"
    );
}

#[test]
fn point_in_ring_handles_a_simple_square() {
    let sq = vec![c(0.0, 0.0), c(10.0, 0.0), c(10.0, 10.0), c(0.0, 10.0)];
    assert!(point_in_ring(c(5.0, 5.0), &sq));
    assert!(!point_in_ring(c(15.0, 5.0), &sq));
}

#[test]
fn signed_area_reports_winding() {
    let ccw = vec![c(0.0, 0.0), c(10.0, 0.0), c(10.0, 10.0), c(0.0, 10.0)];
    let mut cw = ccw.clone();
    cw.reverse();
    assert!(signed_area(&ccw) > 0.0);
    assert!(signed_area(&cw) < 0.0);
    assert_eq!(signed_area(&ccw).abs(), 100.0);
    assert_eq!(signed_area(&[c(0.0, 0.0), c(1.0, 1.0)]), 0.0);
}

#[test]
fn geometry_bbox_and_coords() {
    let g = Geometry::MultiLineString(vec![
        vec![c(0.0, 0.0), c(10.0, 5.0)],
        vec![c(-5.0, 20.0), c(3.0, 1.0)],
    ]);
    let b = g.bbox().unwrap();
    assert_eq!(
        (b.min_e, b.min_n, b.max_e, b.max_n),
        (-5.0, 0.0, 10.0, 20.0)
    );
    assert_eq!(g.coords().count(), 4);
    assert!(!g.is_empty());
    assert!(Geometry::MultiPoint(vec![]).is_empty());
}
