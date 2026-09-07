//! Device profiles: schema, safety margins, and filename rules.

use s2g_core::devices::{
    is_double_extension, load_profiles, match_profile, Confidence, ScreenClass,
};
use std::path::{Path, PathBuf};

fn devices_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("devices")
}

#[test]
fn every_shipped_profile_parses() {
    let profiles = load_profiles(&devices_dir()).expect("profiles should load");
    assert!(profiles.len() >= 3, "got {}", profiles.len());
    for p in &profiles {
        assert!(!p.id.is_empty());
        assert!(!p.display_name.is_empty());
        assert!(p.map_file.max_img_bytes > 0);
        assert!(p.map_file.max_tiles_per_mapset > 0);
        assert!(!p.map_file.install_paths.is_empty());
    }
}

#[test]
fn no_profile_claims_verification_it_does_not_have() {
    // SPEC FR-DEV1: a `measured` or `vendor` level must be backed by evidence. Nothing
    // may quietly claim verification before the hardware procedure has been run.
    for p in load_profiles(&devices_dir()).unwrap() {
        if p.confidence.level.is_verified() {
            assert!(
                p.confidence.last_verified.is_some(),
                "{} claims {:?} without a verification date",
                p.id,
                p.confidence.level
            );
            assert!(
                !p.confidence.sources.is_empty(),
                "{} claims {:?} without sources",
                p.id,
                p.confidence.level
            );
        }
    }
}

#[test]
fn unverified_limits_get_a_safety_margin() {
    // Being wrong about a limit means a map that silently fails on the device, so
    // anything below `measured` is deliberately under-used (FR-DEV2).
    assert_eq!(Confidence::Measured.safety_factor(), 1.0);
    assert_eq!(Confidence::Vendor.safety_factor(), 1.0);
    assert!(Confidence::Community.safety_factor() < 1.0);
    assert!(Confidence::Assumed.safety_factor() < Confidence::Community.safety_factor());

    let profiles = load_profiles(&devices_dir()).unwrap();
    let edge = profiles.iter().find(|p| p.id == "edge-840").unwrap();
    assert!(
        edge.effective_budget_bytes() < edge.storage.recommended_map_budget_bytes,
        "a community-confidence profile must not use its full stated budget"
    );
}

#[test]
fn confidence_ordering_reflects_trust() {
    assert!(Confidence::Vendor > Confidence::Measured);
    assert!(Confidence::Measured > Confidence::Community);
    assert!(Confidence::Community > Confidence::Assumed);
}

#[test]
fn matches_a_known_model_and_falls_back_for_an_unknown_one() {
    let profiles = load_profiles(&devices_dir()).unwrap();

    let m = match_profile(&profiles, "Edge 840", false).unwrap();
    assert_eq!(m.id, "edge-840");
    // Model strings vary in case and spacing between firmware versions.
    assert_eq!(
        match_profile(&profiles, "  edge 840 ", false).unwrap().id,
        "edge-840"
    );

    // An unlisted model must never be a dead end (FR-21).
    let unknown = match_profile(&profiles, "Edge 9999 Ultra", false).unwrap();
    assert_eq!(unknown.id, "generic-edge");
    let watch = match_profile(&profiles, "fenix 99", true).unwrap();
    assert_eq!(watch.id, "generic-fenix");
    assert_eq!(watch.rendering.screen_class, ScreenClass::Wrist);
    assert!(
        watch.is_wrist(),
        "wrist devices select the reduced cartography"
    );
}

#[test]
fn filename_follows_the_device_rule() {
    let profiles = load_profiles(&devices_dir()).unwrap();

    // A device that supports several map sets can take a descriptive name.
    let edge = profiles.iter().find(|p| p.id == "edge-840").unwrap();
    assert_eq!(
        edge.output_filename("Valais hiking"),
        "gmapsupp-Valais-hiking.img"
    );

    // A device that requires the exact name must get exactly that.
    let generic = profiles.iter().find(|p| p.id == "generic-fenix").unwrap();
    assert_eq!(generic.output_filename("Valais hiking"), "gmapsupp.img");
}

#[test]
fn detects_the_double_extension_mistake() {
    // Easy to create on a system that hides file extensions (FR-84).
    assert!(is_double_extension("gmapsupp.img.img"));
    assert!(is_double_extension("GMAPSUPP.IMG.IMG"));
    assert!(!is_double_extension("gmapsupp.img"));
    assert!(!is_double_extension("gmapsupp-Valais.img"));
}

#[test]
fn detection_is_safe_when_nothing_is_connected() {
    // Must not panic or block on a machine with no Garmin volume mounted.
    let found = s2g_core::devices::detect();
    for d in &found {
        assert!(d.mount.exists());
    }
}

/// A measured override must replace the shipped guess, and its confidence with it.
#[test]
fn a_user_override_replaces_the_limit_and_its_confidence() {
    use s2g_core::settings::DeviceOverride;

    let profiles = load_profiles(&devices_dir()).unwrap();
    let base = profiles
        .iter()
        .find(|p| p.id == "edge-840")
        .expect("the edge-840 profile")
        .clone();
    let shipped_budget = base.effective_budget_bytes();

    let overridden = base.clone().with_override(&DeviceOverride {
        map_budget_bytes: Some(3_000_000_000),
        max_img_bytes: None,
        max_tiles_per_mapset: Some(2_000),
        note: Some("filled it until the device refused".into()),
    });

    // A measured number carries no safety factor, so the effective budget is the number
    // the user gave, not a discounted version of it.
    assert_eq!(overridden.effective_budget_bytes(), 3_000_000_000);
    assert_ne!(overridden.effective_budget_bytes(), shipped_budget);
    assert_eq!(overridden.map_file.max_tiles_per_mapset, 2_000);
    assert_eq!(overridden.confidence.level, Confidence::Measured);
    assert!(
        overridden.confidence.notes.contains("filled it until"),
        "the note must survive: {}",
        overridden.confidence.notes
    );

    // Limits not overridden keep the shipped value.
    assert_eq!(
        overridden.map_file.max_img_bytes,
        base.map_file.max_img_bytes
    );
}

#[test]
fn an_empty_override_changes_nothing_including_the_confidence() {
    use s2g_core::settings::DeviceOverride;

    let profiles = load_profiles(&devices_dir()).unwrap();
    let base = profiles
        .iter()
        .find(|p| p.id == "edge-840")
        .unwrap()
        .clone();
    let same = base.clone().with_override(&DeviceOverride::default());

    assert_eq!(same.effective_budget_bytes(), base.effective_budget_bytes());
    assert_eq!(same.confidence.level, base.confidence.level);
    assert_eq!(same.confidence.notes, base.confidence.notes);
}

/// Overrides live in the settings file so an app update cannot discard them.
#[test]
fn overrides_survive_a_settings_round_trip() {
    use s2g_core::settings::{DeviceOverride, Settings};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let mut settings = Settings::default();
    settings.device_overrides.insert(
        "edge-840".into(),
        DeviceOverride {
            map_budget_bytes: Some(3_000_000_000),
            max_img_bytes: None,
            max_tiles_per_mapset: None,
            note: Some("measured".into()),
        },
    );
    settings.save_to(&path).unwrap();

    let back = Settings::load_from(&path);
    assert_eq!(back, settings);
    assert_eq!(
        back.device_overrides["edge-840"].map_budget_bytes,
        Some(3_000_000_000)
    );
}

/// A settings file written before overrides existed must still load.
#[test]
fn settings_without_overrides_still_load() {
    use s2g_core::settings::Settings;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::write(&path, br#"{"dataRoot": "/Volumes/Maps"}"#).unwrap();
    let s = Settings::load_from(&path);
    assert!(s.device_overrides.is_empty());
    assert_eq!(s.data_root.unwrap().to_string_lossy(), "/Volumes/Maps");
}
