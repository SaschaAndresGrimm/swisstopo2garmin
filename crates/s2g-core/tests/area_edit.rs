//! Editing a drawn area (SPEC.md FR-31).
//!
//! Corner ordering and drag anchors are the kind of thing that looks fine in a
//! screenshot and feels like the map fighting you in the hand, so they are pinned here
//! rather than checked by eye.

use s2g_core::area_edit::{apply, handles, outline, AreaEdit, HandleRole};
use s2g_core::recipe::AreaSelection;

/// A 10 km square near Grindelwald.
fn rect() -> AreaSelection {
    AreaSelection::BBox {
        min_e: 2_640_000.0,
        min_n: 1_160_000.0,
        max_e: 2_650_000.0,
        max_n: 1_170_000.0,
    }
}

fn triangle() -> AreaSelection {
    AreaSelection::Polygon {
        points: vec![
            [2_640_000.0, 1_160_000.0],
            [2_650_000.0, 1_160_000.0],
            [2_645_000.0, 1_170_000.0],
        ],
    }
}

fn circle() -> AreaSelection {
    AreaSelection::Circle {
        easting: 2_645_000.0,
        northing: 1_165_000.0,
        radius_km: 5.0,
    }
}

fn bbox_of(area: &AreaSelection) -> (f64, f64, f64, f64) {
    let b = area.bbox();
    (b.min_e, b.min_n, b.max_e, b.max_n)
}

// ---- outlines ------------------------------------------------------------------

/// The defect this module exists for: every selection was drawn as its bounding
/// rectangle, so a polygon disappeared the moment it was finished and was replaced by a
/// box the user had not drawn.
#[test]
fn a_polygon_is_outlined_as_itself_not_as_its_bounding_box() {
    let o = outline(&triangle());
    assert_eq!(o.rings.len(), 1);
    // Three corners plus the repeated closing point.
    assert_eq!(o.rings[0].len(), 4, "{:?}", o.rings[0]);
    assert_eq!(
        o.rings[0].first(),
        o.rings[0].last(),
        "the ring must close for MapLibre to fill it"
    );
}

#[test]
fn a_circle_is_outlined_as_a_circle() {
    let o = outline(&circle());
    assert_eq!(o.rings.len(), 1);
    assert!(
        o.rings[0].len() > 32,
        "a circle needs enough segments to read as one"
    );

    // Every point should be about 5 km from the centre once projected back.
    let (clon, clat) = s2g_core::proj::lv95_to_wgs84(2_645_000.0, 1_165_000.0);
    for p in &o.rings[0] {
        let (e, n) = s2g_core::proj::wgs84_to_lv95(p[0], p[1]);
        let (ce, cn) = s2g_core::proj::wgs84_to_lv95(clon, clat);
        let r = ((e - ce).powi(2) + (n - cn).powi(2)).sqrt();
        assert!((r - 5_000.0).abs() < 5.0, "radius {r} is not 5 km");
    }
}

#[test]
fn a_composite_is_outlined_part_by_part() {
    let area = AreaSelection::Composite {
        parts: vec![rect(), circle()],
    };
    assert_eq!(outline(&area).rings.len(), 2);
}

// ---- handles -------------------------------------------------------------------

/// Corner order is south-west, south-east, north-east, north-west. `apply` indexes it,
/// so a change here without a change there resizes from the wrong anchor.
#[test]
fn rectangle_handles_are_the_four_corners_anticlockwise_plus_a_move_handle() {
    let h = handles(&rect());
    let corners: Vec<_> = h.iter().filter(|x| x.role == HandleRole::Vertex).collect();
    assert_eq!(corners.len(), 4);

    // Checked in LV95, where the geometry is exact. Comparing latitudes would be wrong:
    // LV95 is an oblique Mercator, so a line of constant northing is *not* a line of
    // constant latitude, and two corners of the same edge differ by metres of latitude.
    let back: Vec<(f64, f64)> = corners
        .iter()
        .map(|h| s2g_core::proj::wgs84_to_lv95(h.lon, h.lat))
        .collect();
    let near = |a: f64, b: f64| (a - b).abs() < 2.0;
    assert!(
        near(back[0].0, 2_640_000.0) && near(back[0].1, 1_160_000.0),
        "0 is not SW: {:?}",
        back[0]
    );
    assert!(
        near(back[1].0, 2_650_000.0) && near(back[1].1, 1_160_000.0),
        "1 is not SE: {:?}",
        back[1]
    );
    assert!(
        near(back[2].0, 2_650_000.0) && near(back[2].1, 1_170_000.0),
        "2 is not NE: {:?}",
        back[2]
    );
    assert!(
        near(back[3].0, 2_640_000.0) && near(back[3].1, 1_170_000.0),
        "3 is not NW: {:?}",
        back[3]
    );

    assert_eq!(h.iter().filter(|x| x.role == HandleRole::Centre).count(), 1);
}

/// Midpoints must wrap, including the closing edge. A non-wrapping loop leaves the one
/// edge people most often want to pull out with no handle on it.
#[test]
fn a_polygon_has_a_midpoint_on_every_edge_including_the_closing_one() {
    let h = handles(&triangle());
    let mids: Vec<_> = h
        .iter()
        .filter(|x| x.role == HandleRole::Midpoint)
        .collect();
    assert_eq!(mids.len(), 3, "a triangle has three edges");
    let mut idx: Vec<usize> = mids.iter().map(|m| m.index).collect();
    idx.sort();
    assert_eq!(idx, vec![0, 1, 2]);
}

/// The move handle must sit *inside* the shape. A vertex mean lands wherever the
/// vertices are densest, which on a polygon traced along one detailed edge puts it
/// outside the polygon entirely and makes the handle unusable.
#[test]
fn the_move_handle_is_an_area_centroid_not_a_vertex_average() {
    // A square with a densely sampled bottom edge: the vertex mean is dragged far south.
    let mut points = vec![
        [0.0, 0.0],
        [10_000.0, 0.0],
        [10_000.0, 10_000.0],
        [0.0, 10_000.0],
    ];
    for i in 1..20 {
        points.insert(1, [i as f64 * 500.0, 0.0]);
    }
    let offset: Vec<[f64; 2]> = points
        .iter()
        .map(|p| [2_640_000.0 + p[0], 1_160_000.0 + p[1]])
        .collect();
    let area = AreaSelection::Polygon { points: offset };

    let centre = handles(&area)
        .into_iter()
        .find(|h| h.role == HandleRole::Centre)
        .expect("a polygon needs a move handle");
    let (e, n) = s2g_core::proj::wgs84_to_lv95(centre.lon, centre.lat);
    // The true centroid of the square is its middle.
    assert!((e - 2_645_000.0).abs() < 200.0, "easting {e}");
    assert!(
        (n - 1_165_000.0).abs() < 200.0,
        "northing {n} -- a vertex average would sit near the bottom edge"
    );
}

#[test]
fn a_circle_has_a_centre_and_a_radius_handle() {
    let h = handles(&circle());
    assert_eq!(h.len(), 2);
    assert_eq!(h[0].role, HandleRole::Centre);
    assert_eq!(h[1].role, HandleRole::Radius);
    // Due east of the centre in LV95, which is where the cursor expects to find it.
    // Not due east in *latitude*: LV95 is an oblique Mercator and the two differ.
    let c = s2g_core::proj::wgs84_to_lv95(h[0].lon, h[0].lat);
    let r = s2g_core::proj::wgs84_to_lv95(h[1].lon, h[1].lat);
    assert!(
        (r.0 - c.0 - 5_000.0).abs() < 2.0,
        "radius handle at {r:?} from {c:?}"
    );
    assert!((r.1 - c.1).abs() < 2.0, "the radius handle is not due east");
}

// ---- editing -------------------------------------------------------------------

#[test]
fn dragging_a_corner_keeps_the_opposite_corner_fixed() {
    // Drag the south-west corner (index 0) north-east by 1 km.
    let edited = apply(
        &rect(),
        AreaEdit::MoveVertex {
            index: 0,
            easting: 2_641_000.0,
            northing: 1_161_000.0,
        },
    )
    .unwrap();
    assert_eq!(
        bbox_of(&edited),
        (2_641_000.0, 1_161_000.0, 2_650_000.0, 1_170_000.0),
        "the north-east corner should not have moved"
    );

    // And the north-east corner (index 2) anchors on the south-west.
    let edited = apply(
        &rect(),
        AreaEdit::MoveVertex {
            index: 2,
            easting: 2_648_000.0,
            northing: 1_168_000.0,
        },
    )
    .unwrap();
    assert_eq!(
        bbox_of(&edited),
        (2_640_000.0, 1_160_000.0, 2_648_000.0, 1_168_000.0)
    );
}

/// Every drawing program lets a corner cross its opposite and flips the shape. Refusing
/// would make a fast drag feel broken; inverting the rectangle would build nothing.
#[test]
fn dragging_a_corner_past_its_opposite_flips_rather_than_inverting() {
    let edited = apply(
        &rect(),
        AreaEdit::MoveVertex {
            index: 0,
            easting: 2_655_000.0,
            northing: 1_175_000.0,
        },
    )
    .unwrap();
    let (min_e, min_n, max_e, max_n) = bbox_of(&edited);
    assert!(min_e < max_e && min_n < max_n, "the rectangle inverted");
    assert_eq!(
        (min_e, min_n, max_e, max_n),
        (2_650_000.0, 1_170_000.0, 2_655_000.0, 1_175_000.0)
    );
}

/// Collapsing a rectangle onto a line would build nothing, and "nothing" is far more
/// confusing than a refused drag.
#[test]
fn a_corner_drag_that_would_collapse_the_area_is_refused_with_a_reason() {
    let err = apply(
        &rect(),
        AreaEdit::MoveVertex {
            index: 0,
            easting: 2_649_999.0,
            northing: 1_169_999.0,
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("would build nothing"), "{err}");
}

#[test]
fn a_rectangle_that_does_not_have_that_corner_says_so() {
    let err = apply(
        &rect(),
        AreaEdit::MoveVertex {
            index: 7,
            easting: 2_645_000.0,
            northing: 1_165_000.0,
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("four corners"), "{err}");
}

#[test]
fn translating_moves_every_shape_without_changing_its_size() {
    for area in [rect(), triangle(), circle()] {
        let before = area.bbox();
        let moved = apply(
            &area,
            AreaEdit::Translate {
                d_easting: 2_000.0,
                d_northing: -1_000.0,
            },
        )
        .unwrap();
        let after = moved.bbox();
        assert!((after.min_e - before.min_e - 2_000.0).abs() < 1e-6);
        assert!((after.min_n - before.min_n + 1_000.0).abs() < 1e-6);
        assert!(
            (after.area_km2() - before.area_km2()).abs() < 1e-6,
            "moving changed the area"
        );
    }
}

#[test]
fn dragging_a_polygon_vertex_moves_only_that_vertex() {
    let edited = apply(
        &triangle(),
        AreaEdit::MoveVertex {
            index: 2,
            easting: 2_646_000.0,
            northing: 1_180_000.0,
        },
    )
    .unwrap();
    let AreaSelection::Polygon { points } = edited else {
        panic!("still a polygon")
    };
    assert_eq!(points[0], [2_640_000.0, 1_160_000.0]);
    assert_eq!(points[1], [2_650_000.0, 1_160_000.0]);
    assert_eq!(points[2], [2_646_000.0, 1_180_000.0]);
}

#[test]
fn dragging_a_midpoint_inserts_a_vertex_in_the_right_place() {
    let edited = apply(
        &triangle(),
        AreaEdit::InsertVertex {
            after: 0,
            easting: 2_645_000.0,
            northing: 1_158_000.0,
        },
    )
    .unwrap();
    let AreaSelection::Polygon { points } = edited else {
        panic!("still a polygon")
    };
    assert_eq!(points.len(), 4);
    assert_eq!(
        points[1],
        [2_645_000.0, 1_158_000.0],
        "inserted in the wrong place"
    );
    assert_eq!(
        points[2],
        [2_650_000.0, 1_160_000.0],
        "the following vertex moved"
    );
}

/// Inserting after the last vertex is what dragging the closing edge's midpoint does.
#[test]
fn a_vertex_can_be_inserted_on_the_closing_edge() {
    let edited = apply(
        &triangle(),
        AreaEdit::InsertVertex {
            after: 2,
            easting: 2_638_000.0,
            northing: 1_165_000.0,
        },
    )
    .unwrap();
    let AreaSelection::Polygon { points } = edited else {
        panic!()
    };
    assert_eq!(points.len(), 4);
    assert_eq!(points[3], [2_638_000.0, 1_165_000.0]);
}

#[test]
fn removing_a_vertex_from_a_triangle_is_refused_with_a_reason() {
    let err = apply(&triangle(), AreaEdit::RemoveVertex { index: 0 }).unwrap_err();
    assert!(err.to_string().contains("at least three"), "{err}");
}

#[test]
fn a_vertex_can_be_removed_once_there_are_four() {
    let square = AreaSelection::Polygon {
        points: vec![
            [2_640_000.0, 1_160_000.0],
            [2_650_000.0, 1_160_000.0],
            [2_650_000.0, 1_170_000.0],
            [2_640_000.0, 1_170_000.0],
        ],
    };
    let edited = apply(&square, AreaEdit::RemoveVertex { index: 1 }).unwrap();
    let AreaSelection::Polygon { points } = edited else {
        panic!()
    };
    assert_eq!(points.len(), 3);
    assert_eq!(points[1], [2_650_000.0, 1_170_000.0]);
}

#[test]
fn a_circles_radius_can_be_dragged() {
    let edited = apply(&circle(), AreaEdit::SetRadiusKm { radius_km: 8.0 }).unwrap();
    match edited {
        AreaSelection::Circle {
            radius_km,
            easting,
            northing,
        } => {
            assert_eq!(radius_km, 8.0);
            assert_eq!(
                (easting, northing),
                (2_645_000.0, 1_165_000.0),
                "the centre moved"
            );
        }
        other => panic!("expected a circle, got {other:?}"),
    }
}

#[test]
fn a_radius_dragged_to_nothing_is_refused() {
    let err = apply(&circle(), AreaEdit::SetRadiusKm { radius_km: 0.01 }).unwrap_err();
    assert!(err.to_string().contains("would build nothing"), "{err}");
}

/// A named place that has been dragged somewhere else is not that place any more.
/// Keeping the name would leave the recipe claiming an area centred where it is not.
#[test]
fn moving_a_named_place_turns_it_into_a_circle() {
    let place = AreaSelection::Place {
        name: "Grindelwald".into(),
        radius_km: 6.0,
        easting: 2_645_000.0,
        northing: 1_165_000.0,
    };
    let moved = apply(
        &place,
        AreaEdit::Translate {
            d_easting: 20_000.0,
            d_northing: 0.0,
        },
    )
    .unwrap();
    match moved {
        AreaSelection::Circle {
            easting, radius_km, ..
        } => {
            assert_eq!(easting, 2_665_000.0);
            assert_eq!(radius_km, 6.0);
        }
        other => panic!("expected a circle, got {other:?}"),
    }
}

// ---- what cannot be edited ------------------------------------------------------

/// These three have *derived* shapes. Dragging a canton's boundary would either lie
/// about what gets built or silently turn the selection into a polygon.
#[test]
fn derived_selections_are_not_editable_and_say_what_to_do_instead() {
    let admin = AreaSelection::AdminUnits {
        level: s2g_core::boundaries::AdminLevel::Canton,
        numbers: vec![2],
        names: vec!["Bern".into()],
        buffer_km: 0.0,
        min_e: 2_560_000.0,
        min_n: 1_120_000.0,
        max_e: 2_680_000.0,
        max_n: 1_230_000.0,
    };
    let corridor = AreaSelection::Corridor {
        name: "a track".into(),
        buffer_km: 3.0,
        points: vec![[2_640_000.0, 1_160_000.0], [2_650_000.0, 1_170_000.0]],
    };
    let composite = AreaSelection::Composite {
        parts: vec![rect(), circle()],
    };

    for (area, expect) in [
        (admin, "swissBOUNDARIES3D"),
        (corridor, "imported track"),
        (composite, "one part at a time"),
    ] {
        let o = outline(&area);
        assert!(!o.editable, "{} should not be editable", expect);
        assert!(o.handles.is_empty(), "handles were offered for {expect}");
        let reason = o.not_editable_because.expect("a reason is required");
        assert!(reason.contains(expect), "{reason}");

        // And the edit itself is refused with the same explanation, so a stale UI
        // cannot get one through.
        let err = apply(
            &area,
            AreaEdit::Translate {
                d_easting: 1.0,
                d_northing: 1.0,
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains(expect), "{err}");
    }
}

/// An editable shape must still refuse an operation that makes no sense for it, rather
/// than doing something arbitrary.
#[test]
fn an_operation_that_does_not_apply_to_the_shape_is_refused() {
    let err = apply(&rect(), AreaEdit::SetRadiusKm { radius_km: 4.0 }).unwrap_err();
    assert!(err.to_string().contains("rectangle"), "{err}");
    let err = apply(&triangle(), AreaEdit::SetRadiusKm { radius_km: 4.0 }).unwrap_err();
    assert!(err.to_string().contains("polygon"), "{err}");
}

/// Handles are produced in WGS84 and edits are consumed in LV95, so a handle dragged
/// nowhere must leave the shape where it was. This catches a projection applied twice
/// or in the wrong direction — which would silently move every edited area.
#[test]
fn a_handle_dragged_nowhere_leaves_the_shape_exactly_where_it_was() {
    for area in [rect(), triangle(), circle()] {
        for h in handles(&area) {
            let (e, n) = s2g_core::proj::wgs84_to_lv95(h.lon, h.lat);
            let edit = match h.role {
                HandleRole::Vertex => AreaEdit::MoveVertex {
                    index: h.index,
                    easting: e,
                    northing: n,
                },
                HandleRole::Centre => AreaEdit::Translate {
                    d_easting: 0.0,
                    d_northing: 0.0,
                },
                HandleRole::Radius => continue,
                HandleRole::Midpoint => continue,
            };
            let after = apply(&area, edit).expect("dragging nowhere must be allowed");
            let (a, b) = (area.bbox(), after.bbox());
            for (x, y) in [
                (a.min_e, b.min_e),
                (a.min_n, b.min_n),
                (a.max_e, b.max_e),
                (a.max_n, b.max_n),
            ] {
                // 2 m, from the projection's own envelope: swisstopo's approximate
                // formulas are accurate to about 1 m, and a round trip applies them
                // twice. That is still below the ~2.4 m resolution of the Garmin IMG
                // format, so an error this size cannot reach the output.
                assert!(
                    (x - y).abs() < 2.0,
                    "a null drag of {:?} moved the shape by {} m",
                    h.role,
                    (x - y).abs()
                );
            }
        }
    }
}
