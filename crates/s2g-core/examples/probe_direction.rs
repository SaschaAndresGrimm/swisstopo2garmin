//! Does swissTLM3D digitise a road in the direction of travel?
//!
//!   cargo run --release -q -p s2g-core --example probe_direction
//!
//! The routing work needs to turn `richtungsgetrennt=Wahr` into `oneway=yes`, and that
//! is only correct if a directionally-separated carriageway is digitised the way traffic
//! moves. Getting it wrong routes a cyclist the wrong way down a dual carriageway, so it
//! is not a thing to assume.
//!
//! It is checkable from the data alone. Motorway ramps are one-way by construction: an
//! `Ausfahrt` (exit) leaves the motorway and an `Einfahrt` (entry) joins it. If
//! digitisation follows travel, an exit's **first** vertex sits on the motorway and its
//! last does not; an entry is the other way round.

use s2g_core::gpkg::Gpkg;
use s2g_core::proj::BBox;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache = s2g_core::cache::Cache::default_root();
    let path = s2g_core::pipeline::find_tlm3d(&cache).ok_or("swissTLM3D not cached")?;
    let g = Gpkg::open(&path)?;

    // A region with motorway junctions in it: the Mittelland between Bern and Zürich.
    let bbox = BBox::new(2_580_000.0, 1_180_000.0, 2_700_000.0, 1_260_000.0);

    // Motorway carriageways first, as the thing ramps attach to.
    let mut motorway: Vec<(f64, f64)> = Vec::new();
    let mut ramps: Vec<(String, Vec<(f64, f64)>)> = Vec::new();
    g.for_each_in_bbox("tlm_strassen_strasse", &bbox, &["objektart"], |f| {
        let kind = f.attr("objektart").unwrap_or_default().to_string();
        let pts: Vec<(f64, f64)> = f.geometry.coords().map(|c| (c.e, c.n)).collect();
        if pts.len() < 2 {
            return true;
        }
        match kind.as_str() {
            "Autobahn" | "Autostrasse" => motorway.extend(pts),
            "Ausfahrt" | "Einfahrt" => ramps.push((kind, pts)),
            _ => {}
        }
        true
    })?;

    println!(
        "{} motorway vertices, {} ramps in the probe area",
        motorway.len(),
        ramps.len()
    );
    if motorway.is_empty() || ramps.is_empty() {
        return Err("nothing to compare".into());
    }

    let nearest = |p: (f64, f64)| -> f64 {
        motorway
            .iter()
            .map(|m| ((m.0 - p.0).powi(2) + (m.1 - p.1).powi(2)).sqrt())
            .fold(f64::INFINITY, f64::min)
    };

    let (mut agree, mut disagree, mut ambiguous) = (0, 0, 0);
    for (kind, pts) in &ramps {
        let first = nearest(pts[0]);
        let last = nearest(*pts.last().unwrap());
        // Both ends near the motorway, or both far, says nothing about direction. The
        // threshold comes from the command line so the residual disagreement can be
        // told apart from a slack test.
        let slack: f64 = std::env::args()
            .nth(1)
            .and_then(|a| a.parse().ok())
            .unwrap_or(20.0);
        if (first - last).abs() < slack {
            ambiguous += 1;
            continue;
        }
        // Exit: starts on the motorway. Entry: ends on it.
        let travel_direction = match kind.as_str() {
            "Ausfahrt" => first < last,
            _ => last < first,
        };
        if travel_direction {
            agree += 1;
        } else {
            disagree += 1;
        }
    }

    let decided = agree + disagree;
    println!("\nramps whose ends differ enough to judge: {decided} ({ambiguous} ambiguous)");
    println!("  digitised in the direction of travel : {agree}");
    println!("  digitised against it                 : {disagree}");
    if decided > 0 {
        println!("  agreement: {:.1}%", agree as f64 / decided as f64 * 100.0);
    }
    Ok(())
}
