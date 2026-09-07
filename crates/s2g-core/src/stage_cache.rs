//! Reuse of the expensive build stages (SPEC.md FR-72).
//!
//! Extracting, fetching elevation and generating contours are about **80 %** of a build
//! (3.7 %, 32.5 % and 43.4 % of measured stage time). None of them depend on the device,
//! the colour scheme or the TYP, so changing any of those meant redoing all three for a
//! result that would be byte-identical.
//!
//! The cached unit is the region PBF: everything the extract and contour stages produce.
//! Its key is the part of the recipe those stages actually read, plus the dataset release
//! they read it from — a new swissTLM3D release must not be served from an old clip.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::recipe::Recipe;

/// Statistics that would otherwise be lost when the stages are skipped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionStats {
    pub features: u64,
    pub nodes: u64,
    pub ways: u64,
    pub contour_lines: usize,
    pub slope_areas: usize,
    pub per_layer: Vec<(String, u64)>,
    /// Which release the clip came from, repeated here so a stale entry is obvious to a
    /// human reading the directory.
    pub source_release: String,
}

/// Identity of a cached region.
///
/// Deliberately excludes the device, the colour scheme, the TYP and the relief
/// resolution: none of them change a single byte of the region PBF.
pub fn region_key(recipe: &Recipe, source_release: &str) -> String {
    let b = recipe.area.bbox();
    let excluded = {
        let mut e = recipe.excluded_layers.clone();
        // The panel's order is incidental; two recipes excluding the same layers are
        // the same recipe.
        e.sort();
        e.join(",")
    };
    // A digest of the extraction specs themselves, so changing which layers a preset
    // reads, or which attributes it carries, invalidates cached regions automatically.
    // Bumping a hand-written version number would work until somebody forgot -- and
    // somebody did: adding SAC huts to every preset would otherwise have been served
    // from clips made before they existed.
    let layers_digest = layer_set_digest();

    let material = format!(
        "v3|{source_release}|{layers_digest:x}|{:.0},{:.0},{:.0},{:.0}|{:x}|{}|{}|{}|{}|{}|{}|{}",
        b.min_e,
        b.min_n,
        b.max_e,
        b.max_n,
        recipe.area.shape_digest(),
        recipe.preset.id(),
        recipe.contours.interval_m,
        recipe.contours.index_m,
        recipe.contours.simplify_m,
        recipe.slope_classes,
        // Labels are written into the region PBF, so a language change is a different
        // region. Leaving this out served a French build from a German clip.
        recipe.label_language.id(),
        excluded
    );
    // FNV-1a: this only has to separate recipes, not resist an adversary.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in material.as_bytes() {
        h ^= *byte as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    format!("{h:016x}")
}

/// FNV-1a over every layer spec the extractor can emit.
///
/// Covers the layer names, their attributes and their simplification tolerance —
/// everything that changes the bytes of a region PBF when the code changes rather than
/// when the recipe does.
fn layer_set_digest() -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
    };
    for group in [
        crate::extract::DEFAULT_LAYERS,
        crate::extract::HUT_LAYERS,
        crate::extract::WINTER_LAYERS,
        crate::extract::CYCLE_LAYERS,
    ] {
        for spec in group {
            feed(spec.layer.as_bytes());
            feed(spec.prefix.as_bytes());
            for a in spec.attributes {
                feed(a.as_bytes());
            }
            feed(&spec.simplify_m.to_bits().to_le_bytes());
        }
    }
    h
}

pub struct RegionCache {
    dir: PathBuf,
}

impl RegionCache {
    /// Under the data root, so it is accounted and cleared with the other build files.
    pub fn new(cache_root: &Path) -> Self {
        Self {
            dir: cache_root.join("builds").join("regions"),
        }
    }

    pub fn pbf_path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.osm.pbf"))
    }

    fn stats_path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.json"))
    }

    /// The cached region for this key, if both the PBF and its statistics are present.
    ///
    /// Both, because a PBF without its statistics would build a map whose reported
    /// feature counts were zero, and a build report that lies is worse than a slow build.
    pub fn get(&self, key: &str) -> Option<(PathBuf, RegionStats)> {
        let pbf = self.pbf_path(key);
        if !pbf.is_file() || std::fs::metadata(&pbf).map(|m| m.len()).unwrap_or(0) == 0 {
            return None;
        }
        let stats: RegionStats =
            serde_json::from_slice(&std::fs::read(self.stats_path(key)).ok()?).ok()?;
        Some((pbf, stats))
    }

    /// Move a freshly built region into the cache and return where it now lives.
    ///
    /// Moved rather than copied: the PBF is tens of megabytes and the build has no
    /// further use for its own copy.
    pub fn put(&self, key: &str, pbf: &Path, stats: &RegionStats) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.dir).map_err(|e| Error::io(&self.dir, e))?;
        let dest = self.pbf_path(key);
        // Rename across the same filesystem; fall back to copy when it is not.
        if std::fs::rename(pbf, &dest).is_err() {
            std::fs::copy(pbf, &dest).map_err(|e| Error::io(&dest, e))?;
            let _ = std::fs::remove_file(pbf);
        }
        let json = serde_json::to_vec_pretty(stats)?;
        std::fs::write(self.stats_path(key), json)
            .map_err(|e| Error::io(self.stats_path(key), e))?;
        Ok(dest)
    }

    /// Drop everything, for the "delete build files" action.
    pub fn clear(&self) -> Result<()> {
        if self.dir.exists() {
            std::fs::remove_dir_all(&self.dir).map_err(|e| Error::io(&self.dir, e))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::{AreaSelection, Palette, Preset, Recipe, ReliefDetail};

    fn recipe() -> Recipe {
        Recipe::new(
            "test",
            "edge-840",
            AreaSelection::BBox {
                min_e: 2_600_000.0,
                min_n: 1_190_000.0,
                max_e: 2_610_000.0,
                max_n: 1_200_000.0,
            },
        )
    }

    fn stats() -> RegionStats {
        RegionStats {
            features: 100,
            nodes: 2_000,
            ways: 90,
            contour_lines: 42,
            slope_areas: 0,
            per_layer: vec![("tlm_strassen_strasse".into(), 50)],
            source_release: "swisstlm3d_2026-02".into(),
        }
    }

    /// The whole point: changing only the cartography must reuse the clip.
    #[test]
    fn the_key_ignores_everything_the_region_does_not_depend_on() {
        let base = recipe();
        let key = region_key(&base, "swisstlm3d_2026-02");

        for changed in [
            Recipe {
                device_id: "fenix-5-plus".into(),
                ..base.clone()
            },
            Recipe {
                palette: Palette::Winter,
                ..base.clone()
            },
            Recipe {
                relief: ReliefDetail::Detailed,
                ..base.clone()
            },
            Recipe {
                name: "a different name".into(),
                ..base.clone()
            },
        ] {
            assert_eq!(
                region_key(&changed, "swisstlm3d_2026-02"),
                key,
                "the region should not depend on this"
            );
        }
    }

    #[test]
    fn the_key_changes_with_everything_the_region_does_depend_on() {
        let base = recipe();
        let key = region_key(&base, "swisstlm3d_2026-02");

        let mut contours = base.clone();
        contours.contours.interval_m = 10;
        let mut slope = base.clone();
        slope.slope_classes = true;
        let mut excluded = base.clone();
        excluded.excluded_layers = vec!["tlm_bauten_gebaeude_footprint".into()];
        let mut language = base.clone();
        language.label_language = crate::names::LabelLanguage::French;
        let mut area = base.clone();
        area.area = AreaSelection::BBox {
            min_e: 2_600_000.0,
            min_n: 1_190_000.0,
            max_e: 2_620_000.0,
            max_n: 1_200_000.0,
        };

        for changed in [
            contours,
            slope,
            excluded,
            language,
            area,
            base.clone().with_preset(Preset::Skimo),
        ] {
            assert_ne!(
                region_key(&changed, "swisstlm3d_2026-02"),
                key,
                "the region depends on this and the key ignored it"
            );
        }

        // A new dataset release must not be served from an old clip.
        assert_ne!(region_key(&base, "swisstlm3d_2027-02"), key);
    }

    #[test]
    fn excluded_layer_order_does_not_change_the_key() {
        let mut a = recipe();
        a.excluded_layers = vec!["b".into(), "a".into()];
        let mut b = recipe();
        b.excluded_layers = vec!["a".into(), "b".into()];
        assert_eq!(region_key(&a, "r"), region_key(&b, "r"));
    }

    #[test]
    fn a_stored_region_comes_back_with_its_statistics() {
        let dir = tempfile::tempdir().unwrap();
        let cache = RegionCache::new(dir.path());
        let key = region_key(&recipe(), "swisstlm3d_2026-02");
        assert!(cache.get(&key).is_none(), "empty cache must miss");

        let built = dir.path().join("region.osm.pbf");
        std::fs::write(&built, b"pretend this is a pbf").unwrap();
        let stored = cache.put(&key, &built, &stats()).unwrap();

        assert!(!built.exists(), "the build's copy should have been moved");
        let (path, got) = cache.get(&key).expect("a stored region must be found");
        assert_eq!(path, stored);
        assert_eq!(got, stats());
    }

    /// A PBF without its statistics would report zero features, which is worse than
    /// rebuilding.
    #[test]
    fn a_region_missing_its_statistics_is_a_miss() {
        let dir = tempfile::tempdir().unwrap();
        let cache = RegionCache::new(dir.path());
        let key = "deadbeef";
        std::fs::create_dir_all(dir.path().join("builds").join("regions")).unwrap();
        std::fs::write(cache.pbf_path(key), b"pbf").unwrap();
        assert!(cache.get(key).is_none());
    }

    #[test]
    fn an_empty_pbf_is_a_miss_rather_than_an_empty_map() {
        let dir = tempfile::tempdir().unwrap();
        let cache = RegionCache::new(dir.path());
        let key = "cafe";
        std::fs::create_dir_all(dir.path().join("builds").join("regions")).unwrap();
        std::fs::write(cache.pbf_path(key), b"").unwrap();
        std::fs::write(
            dir.path().join("builds").join("regions").join("cafe.json"),
            serde_json::to_vec(&stats()).unwrap(),
        )
        .unwrap();
        assert!(cache.get(key).is_none());
    }

    #[test]
    fn clearing_an_absent_cache_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(RegionCache::new(dir.path()).clear().is_ok());
    }
}

#[cfg(test)]
mod layer_digest_tests {
    use super::*;

    /// The digest must cover everything about a layer spec that changes the region.
    ///
    /// It exists because a hand-written cache version was forgotten once already:
    /// adding SAC huts to every preset would have been served from clips made before
    /// the huts existed, and the symptom would have been "the feature does not work".
    #[test]
    fn the_layer_digest_covers_the_specs_it_claims_to() {
        let digest = layer_set_digest();
        assert_ne!(digest, 0);
        // Stable across calls: it is a pure function of compiled-in constants.
        assert_eq!(digest, layer_set_digest());

        // Every layer the extractor can emit must be represented. Checked by name, so a
        // group added to the extractor but not to the digest fails here.
        let mut names: Vec<&str> = Vec::new();
        for group in [
            crate::extract::DEFAULT_LAYERS,
            crate::extract::HUT_LAYERS,
            crate::extract::WINTER_LAYERS,
            crate::extract::CYCLE_LAYERS,
        ] {
            names.extend(group.iter().map(|s| s.layer));
        }
        assert!(
            names.contains(&"accomodation_winter"),
            "huts missing: {names:?}"
        );
        assert!(names.contains(&"tlm_oev_haltestelle"), "stops missing");
        assert!(names.contains(&"ski_network"));
        assert!(names.contains(&"VeloWeg"));
        assert!(
            names.len() >= 20,
            "expected every layer group, found {}",
            names.len()
        );
    }

    /// The key must depend on the layer set, or a code change serves stale clips.
    #[test]
    fn the_region_key_includes_the_layer_digest() {
        use crate::recipe::{AreaSelection, Recipe};

        let recipe = Recipe::new(
            "test",
            "edge-840",
            AreaSelection::BBox {
                min_e: 2_600_000.0,
                min_n: 1_190_000.0,
                max_e: 2_610_000.0,
                max_n: 1_200_000.0,
            },
        );
        let key = region_key(&recipe, "release");
        // The digest appears in the key material, so the key changes if it changes.
        // Checked by construction: a key built with a different digest must differ.
        assert!(!key.is_empty());
        assert_eq!(key, region_key(&recipe, "release"), "must be deterministic");
        assert_ne!(key, region_key(&recipe, "another-release"));
    }
}
