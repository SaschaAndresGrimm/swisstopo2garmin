//! Check the shipped size model against a suite of reference builds (SPEC.md FR-60).
//!
//!   cargo run --release -p s2g-core --example check_estimator -- out/device-test
//!
//! FR-60 asks for ±25 % on at least ten reference areas. Leave-one-out error over the
//! training set answers a related but weaker question: it says the model generalises
//! *within* the kind of build it was fitted on. Every one of those sixteen builds was a
//! handlebar map between 144 and 576 km², with no slope classes.
//!
//! This reads real build manifests instead. A manifest records the recipe, the per-layer
//! feature counts and the actual output size, which is everything the predictors need —
//! so any build ever made is a reference area, without rebuilding anything, and the
//! interesting ones are the builds the training set does not resemble.

use std::collections::BTreeMap;
use std::path::PathBuf;

use s2g_core::estimate::{Predictors, SizeModel};
use s2g_core::extract::LayerGroup;
use s2g_core::manifest::Manifest;

/// FR-60's tolerance.
const TOLERANCE: f64 = 0.25;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("out/device-test"));
    let root = repo_root();

    let model = SizeModel::load(&root.join("estimator/size-model.json"))?;
    let mut rows = Vec::new();

    let mut manifests: Vec<PathBuf> = std::fs::read_dir(&dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".manifest.json"))
        .collect();
    manifests.sort();

    if manifests.is_empty() {
        return Err(format!("no manifests in {}", dir.display()).into());
    }

    for path in &manifests {
        let m: Manifest = serde_json::from_slice(&std::fs::read(path)?)?;
        let predictors = predictors_from(&m);
        // `predict` applies the slope term itself, from the recipe. Adding it here as
        // well is a mistake this script made once: it reported every slope build as 33 %
        // over budget and pointed the blame at the model.
        let predicted = model.predict(&predictors);
        let actual = m.output.bytes;
        let error = (predicted as f64 - actual as f64) / actual as f64;
        rows.push((
            path.file_name().unwrap().to_string_lossy().to_string(),
            m.recipe.device_id.clone(),
            predictors.area_km2,
            actual,
            predicted,
            error,
        ));
    }

    println!(
        "{:<34} {:<14} {:>8} {:>10} {:>10} {:>8}",
        "reference area", "device", "km2", "actual", "predicted", "error"
    );
    for (name, device, km2, actual, predicted, error) in &rows {
        println!(
            "{:<34} {:<14} {:>8.0} {:>10} {:>10} {:>7.1}% {}",
            name.trim_end_matches(".manifest.json"),
            device,
            km2,
            actual,
            predicted,
            error * 100.0,
            if error.abs() <= TOLERANCE { "" } else { "OVER" }
        );
    }

    let n = rows.len();
    let mape = rows.iter().map(|r| r.5.abs()).sum::<f64>() / n as f64;
    let worst = rows.iter().map(|r| r.5.abs()).fold(0.0, f64::max);
    let over: Vec<&str> = rows
        .iter()
        .filter(|r| r.5.abs() > TOLERANCE)
        .map(|r| r.0.as_str())
        .collect();

    println!("\nreference areas : {n}");
    println!("mean error      : {:.1}%", mape * 100.0);
    println!("worst error     : {:.1}%", worst * 100.0);
    if n < 10 {
        println!(
            "\nFR-60 asks for at least ten reference areas; this suite has {n}. \
             Every build writes a manifest, so pointing this at a directory of them \
             extends the suite without rebuilding."
        );
    }
    if over.is_empty() {
        println!("\nwithin ±25% on every area.");
        Ok(())
    } else {
        Err(format!("over ±25% on: {}", over.join(", ")).into())
    }
}

/// Rebuild the estimator's inputs from what a manifest recorded.
///
/// The per-layer counts are mapped back to the groups the model has coefficients for,
/// by asking the extractor which group each layer belongs to — so a layer moved between
/// groups cannot make this disagree with a live estimate.
fn predictors_from(m: &Manifest) -> Predictors {
    let mut group_counts: BTreeMap<String, u64> = BTreeMap::new();
    for (layer, count) in &m.features_per_layer {
        if let Some(group) = group_of(layer) {
            *group_counts.entry(group.id().to_string()).or_default() += count;
        }
    }
    Predictors {
        group_counts,
        area_km2: m.recipe.area.bbox().area_km2(),
        contour_interval_m: m.recipe.contours.interval_m,
        relief: m.recipe.relief,
        slope_classes: m.recipe.slope_classes,
        wrist: wrist_device(&m.recipe.device_id),
    }
}

/// Whether a recorded device id is a wrist profile.
///
/// Read from the shipped profiles rather than pattern-matched on the name: the profiles
/// are where `isWrist` is declared, and a new wrist model must not need an edit here.
fn wrist_device(device_id: &str) -> bool {
    let root = repo_root();
    s2g_core::devices::load_profiles(&root.join("devices"))
        .ok()
        .and_then(|ps| ps.iter().find(|p| p.id == device_id).map(|p| p.is_wrist()))
        .unwrap_or(false)
}

/// Which estimator group a layer's features count toward.
///
/// Contours and slope areas are written by the pipeline rather than extracted, and the
/// model has separate terms for both, so they are excluded here rather than counted as
/// features.
fn group_of(layer: &str) -> Option<LayerGroup> {
    for group in [
        s2g_core::extract::DEFAULT_LAYERS,
        s2g_core::extract::HUT_LAYERS,
        s2g_core::extract::WINTER_LAYERS,
        s2g_core::extract::CYCLE_LAYERS,
    ] {
        if let Some(spec) = group
            .iter()
            .find(|s| s.layer == layer || layer.starts_with(s.layer) || s.layer.starts_with(layer))
        {
            return Some(spec.group);
        }
    }
    None
}

fn repo_root() -> PathBuf {
    std::env::current_dir()
        .expect("cwd")
        .ancestors()
        .find(|c| c.join("devices").is_dir() && c.join("style").is_dir())
        .expect("run from inside the repository")
        .to_path_buf()
}
