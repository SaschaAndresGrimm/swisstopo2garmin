//! Recipe model and the build orchestrator.
//!
//! The full build is exercised by tests/build.rs against the fixture; here the focus
//! is the recipe contract the GUI depends on.

use s2g_core::recipe::{AreaSelection, Preset, Recipe, ReliefDetail};

fn area() -> AreaSelection {
    AreaSelection::Place {
        name: "Grindelwald".into(),
        radius_km: 8.0,
        easting: 2_645_921.0,
        northing: 1_163_748.0,
    }
}

#[test]
fn presets_declare_the_data_they_need() {
    // The UI must be able to say what a preset needs before the user commits to it.
    assert!(!Preset::Hiking.needs_winter() && !Preset::Hiking.needs_cycle());
    assert!(Preset::Skimo.needs_winter() && !Preset::Skimo.needs_cycle());
    assert!(Preset::Cycling.needs_cycle() && !Preset::Cycling.needs_winter());
    assert!(Preset::Full.needs_winter() && Preset::Full.needs_cycle());
    assert_eq!(Preset::all().len(), 4);
}

#[test]
fn full_topo_defaults_to_finer_contours() {
    assert_eq!(Preset::Hiking.default_contour_m(), 20);
    assert_eq!(Preset::Full.default_contour_m(), 10);
    assert_eq!(Preset::Full.default_index_contour_m(), 50);
}

#[test]
fn choosing_a_preset_updates_the_contour_defaults() {
    let r = Recipe::new("Test", "edge-840", area()).with_preset(Preset::Full);
    assert_eq!(r.preset, Preset::Full);
    assert_eq!(r.contours.interval_m, 10);
    assert_eq!(r.contours.index_m, 50);
}

#[test]
fn place_areas_store_the_resolved_coordinate() {
    // Place names are not unique. Re-resolving "Grindelwald" later could pick the
    // <20 inhabitant hamlet instead of the village, so the coordinate is part of the
    // recipe rather than something to look up again.
    let r = Recipe::new("Test", "edge-840", area());
    let b = r.area.bbox();
    assert_eq!(b.area_km2(), 256.0);
    assert!(b.contains(2_645_921.0, 1_163_748.0));
    assert!(b.within_switzerland());
}

#[test]
fn cache_key_ignores_the_name_but_not_the_content() {
    let base = Recipe::new("Valais", "edge-840", area());

    // Renaming must not change the device identity or invalidate cached stages.
    let mut renamed = base.clone();
    renamed.name = "Something else".into();
    assert_eq!(base.cache_key(), renamed.cache_key());

    // Anything that changes the output must change the key.
    let mut other_preset = base.clone();
    other_preset.preset = Preset::Skimo;
    assert_ne!(base.cache_key(), other_preset.cache_key());

    let mut other_device = base.clone();
    other_device.device_id = "fenix-5-plus".into();
    assert_ne!(base.cache_key(), other_device.cache_key());

    let mut other_contours = base.clone();
    other_contours.contours.interval_m = 10;
    assert_ne!(base.cache_key(), other_contours.cache_key());

    let mut other_relief = base.clone();
    other_relief.relief = ReliefDetail::Off;
    assert_ne!(base.cache_key(), other_relief.cache_key());

    let mut other_area = base.clone();
    other_area.area = AreaSelection::BBox {
        min_e: 2_600_000.0,
        min_n: 1_100_000.0,
        max_e: 2_610_000.0,
        max_n: 1_110_000.0,
    };
    assert_ne!(base.cache_key(), other_area.cache_key());

    let mut excluded = base.clone();
    excluded.excluded_layers = vec!["tlm_bauten_gebaeude_footprint".into()];
    assert_ne!(base.cache_key(), excluded.cache_key());
}

#[test]
fn relief_maps_to_a_dem_resolution() {
    use s2g_core::dem::Resolution;
    assert_eq!(ReliefDetail::Off.resolution(), None);
    assert_eq!(
        ReliefDetail::Gentle.resolution(),
        Some(Resolution::ArcSecond3)
    );
    assert_eq!(
        ReliefDetail::Detailed.resolution(),
        Some(Resolution::ArcSecond1)
    );
}

#[test]
fn a_recipe_round_trips_through_json() {
    // The GUI saves and reloads recipes (FR-55), so the schema must survive.
    let r = Recipe::new("Berner Oberland", "fenix-5-plus", area()).with_preset(Preset::Skimo);
    let json = serde_json::to_string_pretty(&r).unwrap();
    let back: Recipe = serde_json::from_str(&json).unwrap();
    assert_eq!(back.cache_key(), r.cache_key());
    assert_eq!(back.preset, Preset::Skimo);
    assert_eq!(back.name, "Berner Oberland");
    // The discriminated union must be readable from TypeScript too.
    assert!(json.contains("\"kind\": \"place\""), "{json}");
}

#[test]
fn stages_are_ordered_and_labelled() {
    use s2g_core::pipeline::Stage;
    let all = Stage::all();
    assert_eq!(all.len(), 7);
    assert_eq!(all[0], Stage::Extract);
    assert_eq!(*all.last().unwrap(), Stage::Verify);
    for s in all {
        assert!(!s.label().is_empty());
    }
}
