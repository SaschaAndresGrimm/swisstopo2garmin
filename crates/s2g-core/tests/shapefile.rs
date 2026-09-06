//! Shapefile reader, against the real ASTRA cycle network.
//!
//! Skipped when the route data has not been downloaded, so a clean checkout passes.
//! `spikes/s0/fetch_routes.py` fetches it.

use s2g_core::proj::BBox;
use s2g_core::shapefile::{ShapeType, Shapefile};
use std::path::PathBuf;

fn routes_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".cache/swisstopo2garmin/routes")
}

fn find(stem: &str) -> Option<PathBuf> {
    let mut stack = vec![routes_root()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).ok()?.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_stem().map(|s| s == stem).unwrap_or(false)
                && p.extension().map(|x| x == "shp").unwrap_or(false)
            {
                return Some(p);
            }
        }
    }
    None
}

#[test]
fn reads_the_real_cycle_network() {
    let Some(path) = find("VeloWeg") else {
        eprintln!("skipping: route data not downloaded (spikes/s0/fetch_routes.py)");
        return;
    };
    let shp = Shapefile::open(&path).expect("open VeloWeg");
    assert_eq!(
        shp.shape_type,
        ShapeType::PolyLine,
        "PolyLineZ maps to PolyLine"
    );

    // The declared bbox must cover Switzerland.
    assert!(shp.bbox.within_switzerland());
    assert!(
        shp.bbox.area_km2() > 40_000.0,
        "{} km2",
        shp.bbox.area_km2()
    );
    assert!(shp.field_names().iter().any(|f| f == "ObjektArt"));

    // A small region should yield a handful of segments, all inside it.
    let bbox = BBox::new(2_645_000.0, 1_163_000.0, 2_655_000.0, 1_173_000.0);
    let mut coords = 0usize;
    let mut outside = 0usize;
    let n = shp
        .for_each_in_bbox(&bbox, &["ObjektArt", "BelagTLM"], |f| {
            let b = f.geometry.bbox().expect("geometry has extent");
            if !b.intersects(&bbox) {
                outside += 1;
            }
            coords += f.geometry.coords().count();
            // Coordinates must be plausible LV95, which catches byte-order and
            // offset mistakes that a record count would not.
            for c in f.geometry.coords() {
                assert!(
                    (2_400_000.0..2_900_000.0).contains(&c.e)
                        && (1_000_000.0..1_400_000.0).contains(&c.n),
                    "coordinate outside LV95: {c:?}"
                );
            }
            true
        })
        .expect("scan");

    assert!(n > 0, "expected cycle segments near Grindelwald");
    assert_eq!(
        outside, 0,
        "{outside} features did not intersect the query box"
    );
    assert!(coords > n, "each polyline should have several vertices");
}

#[test]
fn decodes_dbf_attributes_including_numbers() {
    let Some(path) = find("MTBWeg") else {
        eprintln!("skipping: route data not downloaded");
        return;
    };
    let shp = Shapefile::open(&path).unwrap();
    let bbox = shp.bbox;

    let mut singletrail = 0usize;
    let mut normal = 0usize;
    let mut seen_text = false;
    let mut stop = 0usize;
    shp.for_each_in_bbox(&bbox, &["IsSTrail", "BelagTLM"], |f| {
        // IsSTrail is a dBASE numeric field; if numeric decoding were broken this
        // would be None and the singletrail distinction would silently vanish.
        match f.tag("IsSTrail").as_deref() {
            Some("1") => singletrail += 1,
            Some("0") => normal += 1,
            _ => {}
        }
        if f.tag("BelagTLM").is_some() {
            seen_text = true;
        }
        stop += 1;
        stop < 20_000
    })
    .unwrap();

    assert!(normal > 0, "expected non-singletrail segments");
    assert!(
        singletrail > 0,
        "expected singletrail segments (IsSTrail=1)"
    );
    assert!(seen_text, "expected a text attribute to decode");
}

#[test]
fn a_bbox_outside_switzerland_yields_nothing() {
    let Some(path) = find("VeloWeg") else {
        return;
    };
    let shp = Shapefile::open(&path).unwrap();
    let n = shp
        .for_each_in_bbox(
            &BBox::new(1_000_000.0, 1_000_000.0, 1_001_000.0, 1_001_000.0),
            &[],
            |_| true,
        )
        .unwrap();
    assert_eq!(n, 0);
}

#[test]
fn rejects_a_file_that_is_not_a_shapefile() {
    let dir = tempfile::tempdir().unwrap();
    let bogus = dir.path().join("nope.shp");
    std::fs::write(&bogus, vec![0u8; 200]).unwrap();
    assert!(Shapefile::open(&bogus).is_err());

    let short = dir.path().join("short.shp");
    std::fs::write(&short, b"tiny").unwrap();
    assert!(Shapefile::open(&short).is_err());
}
