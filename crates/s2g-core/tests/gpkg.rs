//! GeoPackage reader, against a committed 2x2 km swissTLM3D extract.
//!
//! The fixture is real swisstopo data (see spikes/s0/make_fixture.sh), so these tests
//! exercise the actual schema, the actual R-tree layout and the actual sentinel
//! values — not a synthetic approximation of them.

use s2g_core::geom::Geometry;
use s2g_core::gpkg::{parse_gpkg_geometry, Gpkg};
use s2g_core::proj::BBox;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/grindelwald.gpkg"
);
/// The extent the fixture was cut to.
fn fixture_bbox() -> BBox {
    BBox::new(2_645_000.0, 1_163_000.0, 2_647_000.0, 1_165_000.0)
}

fn open() -> Gpkg {
    Gpkg::open(FIXTURE).expect("fixture should open")
}

#[test]
fn lists_layers_with_geometry_and_index_information() {
    let layers = open().layers().unwrap();
    assert_eq!(
        layers.len(),
        5,
        "{:?}",
        layers.iter().map(|l| &l.name).collect::<Vec<_>>()
    );

    let roads = layers
        .iter()
        .find(|l| l.name == "tlm_strassen_strasse")
        .expect("road layer");
    assert!(roads.is_spatial());
    assert_eq!(roads.geometry_column.as_deref(), Some("geom"));
    assert_eq!(roads.srs_id, 2056, "swissTLM3D is LV95");
    assert!(roads.columns.iter().any(|c| c == "wanderwege"));

    // Every swissTLM3D spatial layer carries an R-tree. This is what makes regional
    // extraction fast enough that no tiled intermediate is needed.
    assert!(
        layers.iter().all(|l| l.has_rtree),
        "all layers should report an R-tree: {:?}",
        layers
            .iter()
            .map(|l| (&l.name, l.has_rtree))
            .collect::<Vec<_>>()
    );
}

#[test]
fn rtree_presence_is_read_from_gpkg_extensions_not_guessed_from_names() {
    // Milestone 0 inferred index presence by splitting `rtree_<table>_geom` on '_',
    // which silently reported "no index" because layer names contain underscores.
    let layers = open().layers().unwrap();
    let underscored = layers
        .iter()
        .filter(|l| l.name.matches('_').count() >= 3)
        .count();
    assert!(
        underscored > 0,
        "fixture should contain deeply underscored names"
    );
    assert!(layers.iter().filter(|l| l.has_rtree).count() == layers.len());
}

#[test]
fn counts_in_bbox_without_reading_geometry() {
    let g = open();
    let all = g.count("tlm_strassen_strasse").unwrap();
    let inside = g
        .count_in_bbox("tlm_strassen_strasse", &fixture_bbox())
        .unwrap();
    assert_eq!(inside, all, "the fixture was cut to this exact extent");

    // A box in the corner must select strictly fewer features.
    let corner = BBox::new(2_645_000.0, 1_163_000.0, 2_645_500.0, 1_163_500.0);
    let few = g.count_in_bbox("tlm_strassen_strasse", &corner).unwrap();
    assert!(few > 0 && few < all, "corner selected {few} of {all}");

    // Somewhere else entirely.
    let elsewhere = BBox::new(2_700_000.0, 1_200_000.0, 2_701_000.0, 1_201_000.0);
    assert_eq!(
        g.count_in_bbox("tlm_strassen_strasse", &elsewhere).unwrap(),
        0
    );
}

#[test]
fn streams_features_with_parsed_geometry_and_attributes() {
    let g = open();
    let mut n = 0usize;
    let mut linestrings = 0usize;
    let mut with_class = 0usize;

    g.for_each_in_bbox(
        "tlm_strassen_strasse",
        &fixture_bbox(),
        &["objektart", "wanderwege", "belagsart"],
        |f| {
            n += 1;
            if matches!(
                f.geometry,
                Geometry::LineString(_) | Geometry::MultiLineString(_)
            ) {
                linestrings += 1;
            }
            assert!(
                !f.geometry.is_empty(),
                "feature {} has empty geometry",
                f.id
            );
            if f.attr("objektart").is_some() {
                with_class += 1;
            }
            true
        },
    )
    .unwrap();

    assert_eq!(n, 716, "fixture road count");
    assert_eq!(linestrings, n, "the road layer is entirely linear");
    assert_eq!(with_class, n, "every road carries an objektart");
}

#[test]
fn filters_the_k_w_no_data_sentinel() {
    // `k_W` appears across many swissTLM3D columns and is not a value. A style rule
    // matching it would paint features that carry no information.
    let g = open();
    let mut raw_k_w = 0usize;
    let mut leaked = 0usize;

    g.for_each_in_bbox(
        "tlm_strassen_strasse",
        &fixture_bbox(),
        &["belagsart", "verkehrsbedeutung"],
        |f| {
            for key in ["belagsart", "verkehrsbedeutung"] {
                if f.attributes.get(key).and_then(|v| v.as_str()) == Some("k_W") {
                    raw_k_w += 1;
                    if f.attr(key).is_some() {
                        leaked += 1;
                    }
                }
            }
            true
        },
    )
    .unwrap();

    assert!(raw_k_w > 0, "fixture should contain k_W values to filter");
    assert_eq!(
        leaked, 0,
        "k_W must never be returned as a meaningful value"
    );
}

#[test]
fn reads_the_swiss_hiking_classification() {
    // The flagship feature: the classification is an attribute on the road layer,
    // not a separate layer.
    let g = open();
    let mut yellow = 0;
    let mut red = 0;
    g.for_each_in_bbox(
        "tlm_strassen_strasse",
        &fixture_bbox(),
        &["wanderwege"],
        |f| {
            match f.attr("wanderwege") {
                Some("Wanderweg") => yellow += 1,
                Some("Bergwanderweg") => red += 1,
                _ => {}
            }
            true
        },
    )
    .unwrap();
    assert_eq!(yellow, 198);
    assert_eq!(red, 3);
}

#[test]
fn parses_polygon_layers_with_rings() {
    let g = open();
    let mut polygons = 0usize;
    let mut total_rings = 0usize;
    g.for_each_in_bbox(
        "tlm_bb_bodenbedeckung",
        &fixture_bbox(),
        &["objektart"],
        |f| {
            match &f.geometry {
                Geometry::Polygon(rings) => {
                    assert!(!rings.is_empty());
                    // A GeoPackage ring is closed: first point equals last.
                    for r in rings {
                        assert!(r.len() >= 4, "ring too short: {}", r.len());
                        assert_eq!(r.first(), r.last(), "ring is not closed");
                    }
                    total_rings += rings.len();
                    polygons += 1;
                }
                Geometry::MultiPolygon(ps) => {
                    total_rings += ps.iter().map(|r| r.len()).sum::<usize>();
                    polygons += 1;
                }
                other => panic!("land cover should be areal, got {}", other.kind()),
            }
            true
        },
    )
    .unwrap();
    assert_eq!(polygons, 99);
    assert!(total_rings >= polygons);
}

#[test]
fn geometry_lands_in_the_expected_place() {
    let g = open();
    let bbox = fixture_bbox();
    let mut checked = 0;
    g.for_each_in_bbox("tlm_namen_flurname", &bbox, &["name"], |f| {
        // Points come from an R-tree query, so they must fall inside the query box.
        if let Geometry::Point(c) = f.geometry {
            assert!(
                bbox.expand(1.0).contains(c.e, c.n),
                "point {:?} outside the query box",
                c
            );
            checked += 1;
        }
        true
    })
    .unwrap();
    assert!(checked > 50, "only {checked} points checked");
}

#[test]
fn sink_can_stop_the_scan_early() {
    let g = open();
    let mut seen = 0;
    let visited = g
        .for_each_in_bbox("tlm_strassen_strasse", &fixture_bbox(), &[], |_| {
            seen += 1;
            seen < 10
        })
        .unwrap();
    assert_eq!(seen, 10);
    assert_eq!(visited, 10, "the scan must stop, not run to completion");
}

#[test]
fn unknown_attribute_names_are_ignored_rather_than_failing() {
    // Layers differ in which columns they carry; asking for a missing one must not
    // abort the extraction.
    let g = open();
    let n = g
        .for_each_in_bbox(
            "tlm_namen_flurname",
            &fixture_bbox(),
            &["name", "wanderwege", "definitely_not_a_column"],
            |f| {
                assert!(f.attributes.contains_key("name"));
                assert!(!f.attributes.contains_key("definitely_not_a_column"));
                true
            },
        )
        .unwrap();
    assert_eq!(n, 104);
}

#[test]
fn rejects_blobs_that_are_not_geopackage_geometry() {
    assert!(parse_gpkg_geometry(b"not a gpkg blob at all").is_err());
    assert!(parse_gpkg_geometry(&[]).is_err());
}

#[test]
fn reads_an_empty_geometry_as_none() {
    // "GP", version 0, flags with the empty bit (0x10) set, srs_id
    let blob = [b'G', b'P', 0, 0x10, 0, 0, 0, 0];
    assert!(parse_gpkg_geometry(&blob).unwrap().is_none());
}
