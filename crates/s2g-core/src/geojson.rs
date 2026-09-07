//! GeoJSON import and export of area selections (SPEC.md FR-42).
//!
//! GeoJSON is WGS84 by specification (RFC 7946 §4), while everything here works in LV95,
//! so both directions project. That is the point of the format: a selection exported
//! here opens in QGIS, geojson.io or map.geo.admin.ch without anyone thinking about
//! Swiss coordinates.
//!
//! Import is deliberately forgiving about what it accepts and strict about what it
//! produces: a `FeatureCollection`, a bare `Feature`, or a naked geometry all work, and
//! anything that is not an area — a point, a bare line — is rejected with a reason
//! rather than silently turned into an empty selection.

use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::proj::{lv95_to_wgs84, wgs84_to_lv95};
use crate::recipe::AreaSelection;

/// Write a selection as GeoJSON in WGS84.
pub fn to_geojson(area: &AreaSelection) -> Value {
    json!({
        "type": "FeatureCollection",
        "features": features_of(area),
        // Not part of the format, but the obvious place to say where this came from.
        "properties": {
            "generator": "swisstopo2garmin",
            "note": "Coordinates are WGS84, as RFC 7946 requires. The app works in LV95."
        }
    })
}

fn ring_to_wgs84(points: &[[f64; 2]]) -> Vec<Vec<f64>> {
    let mut ring: Vec<Vec<f64>> = points
        .iter()
        .map(|p| {
            let (lon, lat) = lv95_to_wgs84(p[0], p[1]);
            vec![lon, lat]
        })
        .collect();
    // RFC 7946 §3.1.6: a linear ring must close.
    if ring.first() != ring.last() {
        if let Some(first) = ring.first().cloned() {
            ring.push(first);
        }
    }
    ring
}

/// A circle as a polygon. GeoJSON has no circle, and every consumer draws polygons.
fn circle_ring(easting: f64, northing: f64, radius_m: f64) -> Vec<Vec<f64>> {
    const STEPS: usize = 64;
    let mut ring: Vec<Vec<f64>> = (0..STEPS)
        .map(|i| {
            let a = i as f64 / STEPS as f64 * std::f64::consts::TAU;
            let (lon, lat) =
                lv95_to_wgs84(easting + radius_m * a.cos(), northing + radius_m * a.sin());
            vec![lon, lat]
        })
        .collect();
    ring.push(ring[0].clone());
    ring
}

fn features_of(area: &AreaSelection) -> Vec<Value> {
    let polygon = |ring: Vec<Vec<f64>>, props: Value| {
        json!({
            "type": "Feature",
            "properties": props,
            "geometry": { "type": "Polygon", "coordinates": [ring] }
        })
    };
    match area {
        AreaSelection::BBox { .. }
        | AreaSelection::Place { .. }
        | AreaSelection::AdminUnits { .. }
        | AreaSelection::Corridor { .. } => {
            // These are exported as their extent: the corridor's centreline and a
            // canton's outline are not reconstructable from a rectangle, and claiming
            // otherwise on re-import would quietly change the area.
            let b = area.bbox();
            let ring = ring_to_wgs84(&[
                [b.min_e, b.min_n],
                [b.max_e, b.min_n],
                [b.max_e, b.max_n],
                [b.min_e, b.max_n],
            ]);
            vec![polygon(
                ring,
                json!({ "kind": kind_name(area), "extentOnly": true }),
            )]
        }
        AreaSelection::Polygon { points } => {
            vec![polygon(ring_to_wgs84(points), json!({ "kind": "polygon" }))]
        }
        AreaSelection::Circle {
            easting,
            northing,
            radius_km,
        } => vec![polygon(
            circle_ring(*easting, *northing, radius_km * 1000.0),
            json!({ "kind": "circle", "radiusKm": radius_km }),
        )],
        AreaSelection::Composite { parts } => parts.iter().flat_map(features_of).collect(),
    }
}

fn kind_name(area: &AreaSelection) -> &'static str {
    match area {
        AreaSelection::BBox { .. } => "bbox",
        AreaSelection::Place { .. } => "place",
        AreaSelection::Polygon { .. } => "polygon",
        AreaSelection::Circle { .. } => "circle",
        AreaSelection::Composite { .. } => "composite",
        AreaSelection::AdminUnits { .. } => "adminUnits",
        AreaSelection::Corridor { .. } => "corridor",
    }
}

/// Read a selection from GeoJSON.
///
/// Every polygon becomes one part; several become a composite. Holes are dropped: a
/// selection is an area to build, and an area with a hole in it would build the hole
/// anyway once the bounding box is clipped.
pub fn from_geojson(text: &str) -> Result<AreaSelection> {
    let doc: Value = serde_json::from_str(text)?;
    let mut parts: Vec<AreaSelection> = Vec::new();
    collect(&doc, &mut parts);

    match parts.len() {
        0 => Err(Error::NotFound(
            "the file contains no polygon; a selection needs an area, not a point or a line".into(),
        )),
        1 => Ok(parts.remove(0)),
        _ => Ok(AreaSelection::Composite { parts }),
    }
}

fn collect(node: &Value, out: &mut Vec<AreaSelection>) {
    match node.get("type").and_then(Value::as_str) {
        Some("FeatureCollection") => {
            if let Some(features) = node.get("features").and_then(Value::as_array) {
                for f in features {
                    collect(f, out);
                }
            }
        }
        Some("Feature") => {
            if let Some(g) = node.get("geometry") {
                collect(g, out);
            }
        }
        Some("GeometryCollection") => {
            if let Some(gs) = node.get("geometries").and_then(Value::as_array) {
                for g in gs {
                    collect(g, out);
                }
            }
        }
        Some("Polygon") => {
            if let Some(ring) = node
                .get("coordinates")
                .and_then(Value::as_array)
                .and_then(|r| r.first())
            {
                if let Some(area) = ring_to_selection(ring) {
                    out.push(area);
                }
            }
        }
        Some("MultiPolygon") => {
            if let Some(polys) = node.get("coordinates").and_then(Value::as_array) {
                for poly in polys {
                    if let Some(ring) = poly.as_array().and_then(|r| r.first()) {
                        if let Some(area) = ring_to_selection(ring) {
                            out.push(area);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn ring_to_selection(ring: &Value) -> Option<AreaSelection> {
    let coords = ring.as_array()?;
    let mut points: Vec<[f64; 2]> = Vec::with_capacity(coords.len());
    for c in coords {
        let pair = c.as_array()?;
        let lon = pair.first()?.as_f64()?;
        let lat = pair.get(1)?.as_f64()?;
        let (e, n) = wgs84_to_lv95(lon, lat);
        points.push([e, n]);
    }
    // A closing point repeats the first; the selection closes implicitly.
    if points.len() > 1 && points.first() == points.last() {
        points.pop();
    }
    (points.len() >= 3).then_some(AreaSelection::Polygon { points })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> AreaSelection {
        AreaSelection::Polygon {
            points: vec![
                [2_600_000.0, 1_200_000.0],
                [2_610_000.0, 1_200_000.0],
                [2_610_000.0, 1_210_000.0],
                [2_600_000.0, 1_210_000.0],
            ],
        }
    }

    #[test]
    fn a_polygon_survives_a_round_trip_through_wgs84() {
        let json = to_geojson(&square()).to_string();
        let back = from_geojson(&json).unwrap();

        let (a, b) = (square().bbox(), back.bbox());
        // Projection is not exact, but a metre either way on a 10 km square is nothing.
        for (x, y) in [
            (a.min_e, b.min_e),
            (a.min_n, b.min_n),
            (a.max_e, b.max_e),
            (a.max_n, b.max_n),
        ] {
            assert!((x - y).abs() < 1.0, "{x} vs {y}");
        }
        assert!(matches!(back, AreaSelection::Polygon { .. }));
    }

    #[test]
    fn exported_geojson_is_wgs84_and_its_rings_close() {
        let doc = to_geojson(&square());
        let ring = doc["features"][0]["geometry"]["coordinates"][0]
            .as_array()
            .unwrap();
        assert_eq!(ring.first(), ring.last(), "a linear ring must close");
        let lon = ring[0][0].as_f64().unwrap();
        let lat = ring[0][1].as_f64().unwrap();
        assert!((5.0..11.0).contains(&lon), "longitude {lon} is not Swiss");
        assert!((45.0..48.0).contains(&lat), "latitude {lat} is not Swiss");
    }

    #[test]
    fn a_circle_exports_as_a_polygon_because_geojson_has_no_circle() {
        let doc = to_geojson(&AreaSelection::Circle {
            easting: 2_600_000.0,
            northing: 1_200_000.0,
            radius_km: 5.0,
        });
        assert_eq!(doc["features"][0]["geometry"]["type"], "Polygon");
        assert_eq!(doc["features"][0]["properties"]["radiusKm"], 5.0);
        let ring = doc["features"][0]["geometry"]["coordinates"][0]
            .as_array()
            .unwrap();
        assert!(
            ring.len() > 60,
            "a circle needs enough points to look round"
        );
    }

    #[test]
    fn several_polygons_import_as_a_composite() {
        let text = r#"{
          "type": "FeatureCollection",
          "features": [
            {"type":"Feature","properties":{},"geometry":{"type":"Polygon","coordinates":
              [[[8.0,46.6],[8.1,46.6],[8.1,46.7],[8.0,46.7],[8.0,46.6]]]}},
            {"type":"Feature","properties":{},"geometry":{"type":"Polygon","coordinates":
              [[[7.0,46.0],[7.1,46.0],[7.1,46.1],[7.0,46.1],[7.0,46.0]]]}}
          ]}"#;
        let area = from_geojson(text).unwrap();
        match area {
            AreaSelection::Composite { parts } => assert_eq!(parts.len(), 2),
            other => panic!("expected a composite, got {other:?}"),
        }
    }

    #[test]
    fn a_bare_geometry_and_a_bare_feature_both_import() {
        for text in [
            r#"{"type":"Polygon","coordinates":[[[8.0,46.6],[8.1,46.6],[8.1,46.7],[8.0,46.6]]]}"#,
            r#"{"type":"Feature","properties":{},"geometry":{"type":"Polygon","coordinates":
               [[[8.0,46.6],[8.1,46.6],[8.1,46.7],[8.0,46.6]]]}}"#,
        ] {
            assert!(matches!(
                from_geojson(text).unwrap(),
                AreaSelection::Polygon { .. }
            ));
        }
    }

    #[test]
    fn a_file_with_no_area_is_rejected_with_a_reason() {
        let err = from_geojson(r#"{"type":"Point","coordinates":[8.0,46.6]}"#).unwrap_err();
        assert!(err.to_string().contains("needs an area"), "{err}");

        let err = from_geojson(r#"{"type":"FeatureCollection","features":[]}"#).unwrap_err();
        assert!(err.to_string().contains("no polygon"), "{err}");
    }

    #[test]
    fn malformed_json_is_an_error_not_a_panic() {
        assert!(from_geojson("{ not json").is_err());
        // A polygon with too few points is not a polygon.
        assert!(
            from_geojson(r#"{"type":"Polygon","coordinates":[[[8.0,46.6],[8.1,46.6]]]}"#).is_err()
        );
    }

    /// A corridor exports as its extent, and says so, rather than pretending a
    /// rectangle is the route.
    #[test]
    fn shapes_that_cannot_round_trip_are_marked_as_extents() {
        let doc = to_geojson(&AreaSelection::Corridor {
            name: "route".into(),
            buffer_km: 2.0,
            points: vec![[2_600_000.0, 1_200_000.0], [2_610_000.0, 1_210_000.0]],
        });
        assert_eq!(doc["features"][0]["properties"]["extentOnly"], true);
        assert_eq!(doc["features"][0]["properties"]["kind"], "corridor");
    }

    #[test]
    fn a_multipolygon_becomes_one_part_per_polygon() {
        let text = r#"{"type":"MultiPolygon","coordinates":[
          [[[8.0,46.6],[8.1,46.6],[8.1,46.7],[8.0,46.6]]],
          [[[7.0,46.0],[7.1,46.0],[7.1,46.1],[7.0,46.0]]]]}"#;
        match from_geojson(text).unwrap() {
            AreaSelection::Composite { parts } => assert_eq!(parts.len(), 2),
            other => panic!("expected a composite, got {other:?}"),
        }
    }
}
