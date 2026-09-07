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

#[test]
fn place_lookup_returns_every_match_most_significant_first() {
    // Names are not unique, and collapsing them silently is what made every early
    // build cover the wrong valley (docs/m0-findings.md §4.9). The fixture is a 2x2 km
    // extract so it has few settlements, but the ordering contract must hold.
    let g = open();
    let places = g.find_places("Grindelwald").unwrap();
    for w in places.windows(2) {
        assert!(
            w[0].population_rank() >= w[1].population_rank(),
            "results must be ordered by significance"
        );
    }
    // An unknown name is empty, not an error.
    assert!(g.find_places("Nowhere At All").unwrap().is_empty());
}

/// The search field was exact, case-sensitive and accent-sensitive, which meant it
/// required the user to already know the answer and spell it the way the dataset does.
#[test]
fn place_lookup_is_case_insensitive_and_matches_a_prefix() {
    let g = open();
    let exact = g.find_places("Grindelwald").unwrap();
    if exact.is_empty() {
        eprintln!("skipping: the fixture has no named settlement");
        return;
    }

    for query in [
        "grindelwald",
        "GRINDELWALD",
        "GrInDeLwAlD",
        "grindel",
        "Grind",
    ] {
        let hits = g.find_places(query).unwrap();
        assert!(
            hits.iter().any(|p| p.name == exact[0].name),
            "{query:?} did not find {:?}",
            exact[0].name
        );
    }

    // A prefix of nothing matches nothing, rather than the whole country.
    assert!(g.find_places("   ").unwrap().is_empty());
    // And a prefix that matches nothing still returns nothing rather than erroring.
    assert!(g.find_places("Zzzzz").unwrap().is_empty());
}

/// Prefix matching introduces a new failure mode: a longer name outranking the exact
/// one. `Bern` must not arrive behind `Bernau`.
#[test]
fn an_exact_match_outranks_a_longer_one_with_the_same_prefix() {
    use s2g_core::gpkg::fold_name;
    let g = open();
    let hits = g.find_places("Grindelwald").unwrap();
    if hits.len() < 2 {
        eprintln!("skipping: the fixture has fewer than two matches to order");
        return;
    }
    // Every exact match must precede every non-exact one.
    let exactness: Vec<bool> = hits
        .iter()
        .map(|p| fold_name(&p.name) == "grindelwald")
        .collect();
    let first_inexact = exactness.iter().position(|e| !e);
    if let Some(i) = first_inexact {
        assert!(
            !exactness[i..].iter().any(|e| *e),
            "an exact match came after an inexact one: {:?}",
            hits.iter().map(|p| &p.name).collect::<Vec<_>>()
        );
    }
}

#[test]
fn population_rank_orders_the_swisstopo_bands() {
    use s2g_core::gpkg::Place;
    let mk = |cat: Option<&str>| Place {
        name: "x".into(),
        alternatives: Vec::new(),
        population_category: cat.map(str::to_string),
        easting: 0.0,
        northing: 0.0,
    };
    assert!(mk(Some("2'000 bis 9'999")).population_rank() > mk(Some("< 20")).population_rank());
    assert!(
        mk(Some("> 100'000")).population_rank() > mk(Some("10'000 bis 49'999")).population_rank()
    );
    assert!(mk(None).population_rank() < mk(Some("< 20")).population_rank());
}

#[test]
fn splits_multilingual_names_and_picks_the_local_one() {
    use s2g_core::gpkg::{primary_name, split_names};

    // swissTLM3D packs every language variant into one field. Rendered verbatim the
    // device shows "Bern | Berna | Berna | Berne".
    assert_eq!(
        split_names("Bern | Berna | Berna | Berne"),
        vec!["Bern", "Berna", "Berna", "Berne"]
    );
    assert_eq!(primary_name("Bern | Berna | Berna | Berne"), "Bern");
    // The first variant is the local name: French-speaking Genève leads in French.
    assert_eq!(primary_name("Genève | Genevra | Genf | Ginevra"), "Genève");
    assert_eq!(primary_name("Zermatt | Praborgne"), "Zermatt");

    // Monolingual names pass through untouched, which is why this was easy to miss.
    assert_eq!(primary_name("Grindelwald"), "Grindelwald");
    assert_eq!(split_names("Grindelwald"), vec!["Grindelwald"]);

    // Degenerate input must not panic or produce empty labels.
    assert_eq!(primary_name(""), "");
    assert_eq!(split_names("  |  "), Vec::<&str>::new());
    assert_eq!(primary_name("| Solo |"), "Solo");
}

#[test]
fn numeric_attributes_are_emitted_as_tags() {
    use s2g_core::gpkg::Value;

    // as_meaningful_str yields text only, which silently dropped every numeric
    // attribute. ski_network.access (0/1/2) is INTEGER, so the skiable / carrying /
    // caution distinction vanished with no error at all.
    assert_eq!(Value::Int(2).as_meaningful_str(), None);
    assert_eq!(Value::Int(2).as_tag_value().as_deref(), Some("2"));
    assert_eq!(Value::Int(0).as_tag_value().as_deref(), Some("0"));

    // A whole number stored as REAL must not become "457.0", or a rule written
    // against the integer form fails.
    assert_eq!(Value::Real(457.0).as_tag_value().as_deref(), Some("457"));
    assert_eq!(Value::Real(1.5).as_tag_value().as_deref(), Some("1.5"));
    assert_eq!(Value::Real(f64::NAN).as_tag_value(), None);

    // Text still passes through the no-data filter.
    assert_eq!(Value::Text("k_W".into()).as_tag_value(), None);
    assert_eq!(
        Value::Text("Skilift".into()).as_tag_value().as_deref(),
        Some("Skilift")
    );
    assert_eq!(Value::Null.as_tag_value(), None);
}
