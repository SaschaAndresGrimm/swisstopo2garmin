//! Every label the *backend* can ask the UI to display must exist (SPEC.md FR-4).
//!
//! The frontend's own `check-i18n.mjs` verifies that literal `t("...")` keys exist and
//! that the four bundles agree. It cannot verify a key built from a value, and most of
//! the UI's labels are: `t(`layer.${id}`)`, `t(`preset.${id}`)`, and so on. Its check
//! for those is that *some* key shares the prefix, which passes for a whole family while
//! one member is missing.
//!
//! That is not hypothetical. `tlm_oev_haltestelle` and `accomodation_winter` were added
//! to the extractor in Milestones 5 and 7 and their labels were not, so the layer panel
//! displayed the literal text `layer.tlm_oev_haltestelle` to the user while every check
//! in the project passed.
//!
//! The list of possible values lives in Rust, so the check belongs here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

/// The string serde writes for a unit-variant enum, which is what crosses the IPC and
/// therefore what the frontend keys its translation off.
fn serde_tag<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .expect("a unit enum variant serialises to a string")
}

fn bundle(lang: &str) -> BTreeMap<String, String> {
    let path = repo_root().join(format!("frontend/src/i18n/{lang}.json"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Every key the backend's values can compose, with the prefix the frontend uses.
fn required_keys() -> Vec<String> {
    use s2g_core::extract::{LayerGroup, CYCLE_LAYERS, DEFAULT_LAYERS, HUT_LAYERS, WINTER_LAYERS};
    use s2g_core::recipe::{Palette, Preset, ReliefDetail};

    let mut keys = Vec::new();

    // t(`layer.${l.id}`) in LayerPanel.
    for group in [DEFAULT_LAYERS, HUT_LAYERS, WINTER_LAYERS, CYCLE_LAYERS] {
        for spec in group {
            keys.push(format!("layer.{}", spec.layer));
        }
    }
    // t(`layers.group.${g}`) in LayerPanel.
    for g in LayerGroup::all() {
        keys.push(format!("layers.group.{}", g.id()));
    }
    // t(`preset.${p}`) in ContentStep and BuildStep.
    for p in Preset::all() {
        keys.push(format!("preset.{}", p.id()));
    }
    // t(`content.relief.${r}`) and t(`content.palette.${p}`). Both reach the frontend
    // as their serde tag, so that is what is asked of serde rather than reproduced by
    // hand -- a hand-written list would be the same kind of drift this test exists for.
    for r in [
        ReliefDetail::Off,
        ReliefDetail::Gentle,
        ReliefDetail::Detailed,
    ] {
        keys.push(format!("content.relief.{}", serde_tag(&r)));
    }
    for p in [Palette::Summer, Palette::Winter] {
        keys.push(format!("content.palette.{}", serde_tag(&p)));
    }
    // t(`area.selected.${outline.kind}`) in AreaStep.
    for kind in [
        "bbox",
        "place",
        "polygon",
        "circle",
        "corridor",
        "adminUnits",
        "composite",
    ] {
        keys.push(format!("area.selected.{kind}"));
    }
    // t(`remedy.${r}`) in OverBudget. The serde names of estimate::Remedy.
    for r in [
        "smallerArea",
        "coarserContours",
        "fewerLayers",
        "noRelief",
        "noSlopeClasses",
        "splitIntoMapSets",
    ] {
        keys.push(format!("remedy.{r}"));
    }
    keys.sort();
    keys.dedup();
    keys
}

#[test]
fn every_backend_label_exists_in_english() {
    let en = bundle("en");
    let required = required_keys();
    let missing: Vec<&str> = required
        .iter()
        .filter(|k| !en.contains_key(*k))
        .map(String::as_str)
        .collect();
    assert!(
        missing.is_empty(),
        "{} label(s) the UI will display as their own key name:\n  {}",
        missing.len(),
        missing.join("\n  ")
    );
}

/// English is the fallback, so a key missing from another bundle renders in English and
/// is invisible to anybody testing in English.
#[test]
fn every_backend_label_is_translated_in_all_four_languages() {
    let required = required_keys();
    let mut problems = Vec::new();
    for lang in ["de", "fr", "it"] {
        let b = bundle(lang);
        for key in &required {
            if !b.contains_key(key) {
                problems.push(format!("{lang}: {key}"));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "{} label(s) will silently fall back to English:\n  {}",
        problems.len(),
        problems.join("\n  ")
    );
}

/// A label that is only the layer's own identifier is not a label. This catches the
/// lazy fix -- adding `"layer.tlm_oev_haltestelle": "tlm_oev_haltestelle"` -- which
/// would satisfy the two tests above and show the user the same thing.
#[test]
fn no_label_is_just_the_identifier_it_is_keyed_by() {
    let en = bundle("en");
    let lazy: Vec<String> = required_keys()
        .into_iter()
        .filter(|k| {
            let suffix = k.rsplit('.').next().unwrap_or_default().to_string();
            en.get(k).map(|v| *v == suffix).unwrap_or(false)
        })
        .collect();
    assert!(lazy.is_empty(), "these labels restate their key: {lazy:?}");
}
