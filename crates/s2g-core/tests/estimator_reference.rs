//! FR-60 against the committed reference suite.
//!
//! FR-60 asks for size estimates within ±25 % on at least ten reference areas. Leave-one-
//! out error over the training set answers a weaker question — whether the model
//! generalises within the kind of build it was fitted on — and every one of those sixteen
//! training builds was a handlebar map between 144 and 576 km² with no slope classes.
//!
//! The suite in `estimator/reference/` is sixteen real build manifests, ten of them from
//! places and radii deliberately absent from the training plan, six of them wrist builds,
//! and five carrying slope classes. A manifest records the recipe, the per-layer feature
//! counts and the actual output size, which is everything the predictors need — so the
//! requirement can be checked in CI, offline, in milliseconds, without rebuilding
//! anything.
//!
//! What this does **not** do is prove the model generalises. Two constants
//! (`WRIST_FEATURE_FACTOR`, `WRIST_SLOPE_FACTOR`) were fitted on these same sixteen
//! builds, so the numbers below are in-sample, and the worst of them is 24.5 % against a
//! 25 % requirement. That is half a percentage point of margin. `docs/size-model.md`
//! says so, and `tools/reference_suite.sh` is how to add areas that are genuinely held
//! out.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use s2g_core::estimate::{Predictors, SizeModel};
use s2g_core::extract::LayerGroup;
use s2g_core::manifest::Manifest;

const TOLERANCE: f64 = 0.25;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

struct Case {
    name: String,
    actual: u64,
    predicted: u64,
    error: f64,
}

fn evaluate() -> Vec<Case> {
    let root = repo_root();
    let model = SizeModel::load(&root.join("estimator/size-model.json"))
        .expect("the shipped size model must load");
    let profiles = s2g_core::devices::load_profiles(&root.join("devices"))
        .expect("the shipped device profiles must load");

    let mut paths: Vec<PathBuf> = std::fs::read_dir(root.join("estimator/reference"))
        .expect("the reference suite must be present")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".manifest.json"))
        .collect();
    paths.sort();

    paths
        .iter()
        .map(|path| {
            let m: Manifest = serde_json::from_slice(&std::fs::read(path).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let wrist = profiles
                .iter()
                .find(|p| p.id == m.recipe.device_id)
                .map(|p| p.is_wrist())
                .unwrap_or_else(|| panic!("unknown device {:?}", m.recipe.device_id));

            let mut group_counts: BTreeMap<String, u64> = BTreeMap::new();
            for (layer, count) in &m.features_per_layer {
                if let Some(group) = group_of(layer) {
                    *group_counts.entry(group.id().to_string()).or_default() += count;
                }
            }
            let predicted = model.predict(&Predictors {
                group_counts,
                area_km2: m.recipe.area.bbox().area_km2(),
                contour_interval_m: m.recipe.contours.interval_m,
                relief: m.recipe.relief,
                slope_classes: m.recipe.slope_classes,
                routing: m.recipe.routing,
                addresses: m.recipe.addresses,
                wrist,
            });
            let actual = m.output.bytes;
            Case {
                name: path.file_stem().unwrap().to_string_lossy().to_string(),
                actual,
                predicted,
                error: (predicted as f64 - actual as f64) / actual as f64,
            }
        })
        .collect()
}

/// Which estimator group a layer counts toward, asked of the extractor rather than
/// hard-coded, so a layer moved between groups cannot make this disagree with a live
/// estimate. Contours and slope areas have their own terms and are not features.
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

#[test]
fn the_suite_has_at_least_the_ten_reference_areas_fr60_asks_for() {
    let cases = evaluate();
    assert!(
        cases.len() >= 10,
        "FR-60 asks for at least ten reference areas; the suite has {}",
        cases.len()
    );
}

/// The suite must keep exercising the cases the training set does not, or it drifts back
/// into confirming what the model already knows.
#[test]
fn the_suite_still_covers_wrist_builds_and_slope_classes() {
    let root = repo_root();
    let mut wrist = 0;
    let mut slope = 0;
    for entry in std::fs::read_dir(root.join("estimator/reference"))
        .unwrap()
        .flatten()
    {
        if !entry.path().to_string_lossy().ends_with(".manifest.json") {
            continue;
        }
        let m: Manifest = serde_json::from_slice(&std::fs::read(entry.path()).unwrap()).unwrap();
        if m.recipe.device_id == "fenix-5-plus" {
            wrist += 1;
        }
        if m.recipe.slope_classes {
            slope += 1;
        }
    }
    assert!(
        wrist >= 4,
        "only {wrist} wrist build(s): the training set has none"
    );
    assert!(slope >= 3, "only {slope} slope build(s)");
}

/// FR-60 itself.
#[test]
fn every_reference_area_is_estimated_within_25_percent() {
    let cases = evaluate();
    let mut report = String::new();
    for c in &cases {
        report.push_str(&format!(
            "\n  {:<34} actual {:>9}  predicted {:>9}  {:>6.1}%{}",
            c.name,
            c.actual,
            c.predicted,
            c.error * 100.0,
            if c.error.abs() > TOLERANCE {
                "  OVER"
            } else {
                ""
            }
        ));
    }
    let over: Vec<&str> = cases
        .iter()
        .filter(|c| c.error.abs() > TOLERANCE)
        .map(|c| c.name.as_str())
        .collect();
    assert!(over.is_empty(), "over ±25% on {over:?}:{report}");

    let worst = cases.iter().map(|c| c.error.abs()).fold(0.0, f64::max);
    let mean = cases.iter().map(|c| c.error.abs()).sum::<f64>() / cases.len() as f64;
    println!(
        "{} reference areas: mean {:.1}%, worst {:.1}%{report}",
        cases.len(),
        mean * 100.0,
        worst * 100.0
    );
}

/// The margin is thin enough to be worth stating as a fact rather than a footnote: this
/// fails if the worst case creeps toward the requirement, which is the warning somebody
/// needs *before* a real build lands outside it.
#[test]
fn the_margin_against_the_requirement_has_not_shrunk() {
    let worst = evaluate().iter().map(|c| c.error.abs()).fold(0.0, f64::max);
    assert!(
        worst < 0.25,
        "worst error {:.1}% is outside the requirement",
        worst * 100.0
    );
    assert!(
        worst > 0.20,
        "worst error is now {:.1}%. If the model genuinely improved, lower this floor \
         and record the new figure in docs/size-model.md — it exists so an improvement \
         is noticed and written down rather than silently absorbed.",
        worst * 100.0
    );
}
