//! Projection accuracy, checked against PROJ (EPSG:2056 -> EPSG:4326).
//!
//! The reference values below were produced with `gdaltransform` and are the
//! authority here. Note that swisstopo's published coordinates for the projection
//! origin (46°57'08.66"N 7°26'22.50"E) are in the **Bessel/CH1903 datum**, not WGS84;
//! using them as WGS84 truth makes a correct implementation look 164 m wrong.

use s2g_core::proj::{lv95_to_wgs84, wgs84_to_lv95, BBox, GARMIN_DEGREES_PER_UNIT};

const M_PER_DEG_LAT: f64 = 111_320.0;
fn m_per_deg_lon(lat: f64) -> f64 {
    M_PER_DEG_LAT * lat.to_radians().cos()
}
fn error_m(lon_a: f64, lat_a: f64, lon_b: f64, lat_b: f64) -> f64 {
    let dx = (lon_a - lon_b) * m_per_deg_lon(lat_a);
    let dy = (lat_a - lat_b) * M_PER_DEG_LAT;
    dx.hypot(dy)
}

/// (easting, northing, lon, lat, name) — lon/lat from PROJ.
const PROJ_REFERENCE: &[(f64, f64, f64, f64, &str)] = &[
    (
        2_600_000.0,
        1_200_000.0,
        7.4386324,
        46.9510828,
        "projection origin (Bern)",
    ),
    (
        2_602_030.7,
        1_191_775.0,
        7.4652723,
        46.8770944,
        "Zimmerwald",
    ),
    (
        2_645_921.0,
        1_163_748.0,
        8.0382085,
        46.6234077,
        "Grindelwald village",
    ),
    (2_679_500.0, 1_212_200.0, 8.4850291, 47.0560594, "Luzern"),
    (2_500_300.0, 1_117_300.0, 6.1469901, 46.1997555, "Geneve"),
    (2_762_300.0, 1_180_500.0, 9.5631667, 46.7559112, "Chur"),
    (
        2_825_000.0,
        1_108_000.0,
        10.3480799,
        46.0859609,
        "Val Mustair (SE extreme)",
    ),
    (
        2_495_000.0,
        1_290_000.0,
        6.0383577,
        47.7521192,
        "Basel/Jura (NW extreme)",
    ),
    (2_611_000.0, 1_267_000.0, 7.5847592, 47.5536254, "Solothurn"),
    (
        2_837_000.0,
        1_160_000.0,
        10.5295784,
        46.5492660,
        "Muenstertal (E extreme)",
    ),
];

/// Measured worst deviation from PROJ across the swissTLM3D coverage area.
/// About 1 m in the interior, rising toward the corners of the coverage rectangle,
/// which lie outside Swiss territory. Documented in SPEC.md FR-P1.
const MAX_DEVIATION_M: f64 = 5.0;

#[test]
fn matches_proj_across_the_coverage_area() {
    let mut worst = 0.0f64;
    let mut worst_name = "";
    for &(e, n, lon, lat, name) in PROJ_REFERENCE {
        let (got_lon, got_lat) = lv95_to_wgs84(e, n);
        let err = error_m(got_lon, got_lat, lon, lat);
        assert!(
            err < MAX_DEVIATION_M,
            "{name}: {err:.2} m from PROJ (got {got_lon:.7},{got_lat:.7})"
        );
        if err > worst {
            worst = err;
            worst_name = name;
        }
    }
    println!("worst deviation {worst:.2} m at {worst_name}");
}

#[test]
fn interior_points_are_accurate_to_about_a_metre() {
    // Away from the corners of the coverage rectangle the approximation is much
    // better than the overall bound; regressions there should be caught.
    for &(e, n, lon, lat, name) in PROJ_REFERENCE {
        if !(2_550_000.0..=2_800_000.0).contains(&e) || !(1_100_000.0..=1_270_000.0).contains(&n) {
            continue;
        }
        let (got_lon, got_lat) = lv95_to_wgs84(e, n);
        let err = error_m(got_lon, got_lat, lon, lat);
        assert!(err < 1.5, "{name}: interior error {err:.2} m");
    }
}

#[test]
fn forward_projection_matches_proj() {
    for &(e, n, lon, lat, name) in PROJ_REFERENCE {
        let (got_e, got_n) = wgs84_to_lv95(lon, lat);
        let err = (got_e - e).hypot(got_n - n);
        assert!(err < MAX_DEVIATION_M, "{name}: {err:.2} m from PROJ");
    }
}

#[test]
fn round_trip_is_consistent() {
    // Forward and inverse must be mutually consistent. Inside the interior — where
    // Swiss data actually is — this is well under Garmin's ~2.4 m grid. Toward the
    // corners of the coverage rectangle (which lie tens of km outside the country)
    // the approximation degrades to about 4 m. Both bounds are asserted so a
    // regression in either region is caught.
    let garmin_m = GARMIN_DEGREES_PER_UNIT * M_PER_DEG_LAT;
    let mut worst_interior = 0.0f64;
    let mut worst_overall = 0.0f64;
    let mut at = (0.0, 0.0);

    let mut e = 2_490_000.0;
    while e <= 2_830_000.0 {
        let mut n = 1_080_000.0;
        while n <= 1_290_000.0 {
            let (lon, lat) = lv95_to_wgs84(e, n);
            let (e2, n2) = wgs84_to_lv95(lon, lat);
            let err = (e2 - e).hypot(n2 - n);
            if err > worst_overall {
                worst_overall = err;
                at = (e, n);
            }
            let interior = (2_550_000.0..=2_800_000.0).contains(&e)
                && (1_100_000.0..=1_270_000.0).contains(&n);
            if interior {
                worst_interior = worst_interior.max(err);
            }
            n += 10_000.0;
        }
        e += 10_000.0;
    }

    assert!(
        worst_interior < garmin_m,
        "interior round-trip error {worst_interior:.2} m exceeds Garmin's {garmin_m:.2} m grid"
    );
    assert!(
        worst_overall < MAX_DEVIATION_M,
        "round-trip error {worst_overall:.2} m at {at:?} exceeds the documented \
         {MAX_DEVIATION_M} m bound"
    );
}

#[test]
fn bbox_to_wgs84_projects_all_four_corners() {
    // The LV95 grid is rotated relative to the graticule, so converting only the
    // south-west and north-east corners produces a box that clips real content.
    let b = BBox::new(2_600_000.0, 1_100_000.0, 2_700_000.0, 1_200_000.0);
    let [w, s, e, n] = b.to_wgs84();
    for (ce, cn) in [
        (b.min_e, b.min_n),
        (b.min_e, b.max_n),
        (b.max_e, b.min_n),
        (b.max_e, b.max_n),
    ] {
        let (lon, lat) = lv95_to_wgs84(ce, cn);
        assert!(lon >= w && lon <= e, "corner lon {lon} outside [{w},{e}]");
        assert!(lat >= s && lat <= n, "corner lat {lat} outside [{s},{n}]");
    }
}

#[test]
fn bbox_geometry_helpers() {
    let a = BBox::from_center(2_600_000.0, 1_200_000.0, 5_000.0);
    assert_eq!(a.width(), 10_000.0);
    assert_eq!(a.area_km2(), 100.0);
    assert!(a.contains(2_600_000.0, 1_200_000.0));
    assert!(!a.contains(2_610_000.0, 1_200_000.0));
    assert!(a.within_switzerland());

    // b starts 10 km east of a's eastern edge
    let b = BBox::from_center(2_620_000.0, 1_200_000.0, 5_000.0);
    assert!(!a.intersects(&b));
    assert!(
        !a.expand(6_000.0).intersects(&b),
        "6 km is not enough to reach b"
    );
    assert!(a.expand(10_001.0).intersects(&b));
    assert_eq!(a.union(&b).width(), 30_000.0);

    assert!(!BBox::from_center(1_000_000.0, 1_000_000.0, 1_000.0).within_switzerland());
}

#[test]
fn bbox_normalises_inverted_input() {
    let b = BBox::new(2_700_000.0, 1_200_000.0, 2_600_000.0, 1_100_000.0);
    assert_eq!(b.min_e, 2_600_000.0);
    assert_eq!(b.max_n, 1_200_000.0);
}
