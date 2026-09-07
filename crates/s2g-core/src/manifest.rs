//! The build manifest (SPEC.md FR-71, §11.3).
//!
//! Written beside every output. Records the recipe, the dataset releases and checksums a
//! build actually read, the tool versions that produced it, stage timings, feature counts
//! per layer, the output hash, and the attribution string.
//!
//! Reproducibility was a claim before this: nothing recorded which release of swissTLM3D
//! a map came from, so a map on a device could not be traced back to its inputs, and two
//! maps built months apart could not be told apart except by looking at them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::recipe::Recipe;

/// A dataset as it was on disk when the build read it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRelease {
    pub collection: String,
    pub item: String,
    /// Publication datetime from STAC, when the provenance recorded one.
    pub datetime: Option<String>,
    /// The asset's published checksum, so the bytes can be traced to swisstopo's.
    pub checksum: Option<String>,
    /// Files the release put in the cache. A shapefile dataset is a dozen of them.
    pub files: usize,
    /// Total bytes on disk for this release.
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tools {
    pub app: String,
    pub mkgmap: String,
    pub splitter: String,
    pub java: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub file: String,
    pub bytes: u64,
    pub sha256: String,
    pub tile_count: usize,
    pub family_id: u16,
    pub has_dem: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub schema_version: u32,
    /// RFC 3339, so a manifest can be sorted without parsing the recipe.
    pub built_at: String,
    pub recipe: Recipe,
    /// The identity the recipe hashes to, which is also the map's identity on the device.
    pub recipe_key: String,
    pub sources: Vec<SourceRelease>,
    pub tools: Tools,
    pub output: Output,
    /// Seconds per stage, in the order they ran.
    pub stage_seconds: Vec<(String, f64)>,
    pub features_per_layer: BTreeMap<String, u64>,
    pub contour_lines: usize,
    pub slope_areas: usize,
    pub warnings: Vec<String>,
    /// Embedded in the map itself, and required by the licence (FR-L1).
    pub attribution: String,
}

impl Manifest {
    /// Beside the output, named after it: `gmapsupp.img` -> `gmapsupp.manifest.json`.
    pub fn path_for(output: &Path) -> PathBuf {
        output.with_extension("manifest.json")
    }

    pub fn write_beside(&self, output: &Path) -> Result<PathBuf> {
        let path = Manifest::path_for(output);
        let json = serde_json::to_vec_pretty(self)?;
        std::fs::write(&path, json).map_err(|e| Error::io(&path, e))?;
        Ok(path)
    }

    pub fn read(path: &Path) -> Result<Manifest> {
        let bytes = std::fs::read(path).map_err(|e| Error::io(path, e))?;
        Ok(serde_json::from_slice(&bytes)?)
    }
}

/// Every dataset in the cache that carries provenance, as sources for a manifest.
///
/// Recorded for the whole cache rather than only the layers a preset reads: which
/// datasets were *available* is part of what made the output what it is, and a preset
/// that silently found no winter data is exactly the case a manifest should explain.
pub fn sources_from_cache(entries: &[crate::cache::Entry]) -> Vec<SourceRelease> {
    // One entry per *release*, not per file: the cache lists every file, and a
    // shapefile dataset is fifteen of them, which would fill the manifest with fifteen
    // identical lines.
    let mut by_release: BTreeMap<(String, String), SourceRelease> = BTreeMap::new();
    for e in entries {
        let Some(p) = e.provenance.as_ref() else {
            continue;
        };
        let key = (p.collection.clone(), p.item.clone());
        let slot = by_release.entry(key).or_insert_with(|| SourceRelease {
            collection: p.collection.clone(),
            item: p.item.clone(),
            datetime: p.datetime.clone(),
            checksum: p.checksum.as_ref().map(|d| format!("{}:{}", d.algo, d.hex)),
            files: 0,
            bytes: 0,
        });
        slot.files += 1;
        slot.bytes += e.bytes;
    }
    by_release.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::{AreaSelection, Recipe};

    fn manifest() -> Manifest {
        Manifest {
            schema_version: 1,
            built_at: "2026-09-06T20:00:00Z".into(),
            recipe: Recipe::new(
                "Grindelwald 8 km",
                "edge-840",
                AreaSelection::Place {
                    name: "Grindelwald".into(),
                    radius_km: 8.0,
                    easting: 2_645_921.0,
                    northing: 1_163_748.0,
                },
            ),
            recipe_key: "edge-840|...".into(),
            sources: vec![SourceRelease {
                collection: "ch.swisstopo.swisstlm3d".into(),
                item: "swisstlm3d_2026-02".into(),
                datetime: Some("2026-02-24T00:00:00Z".into()),
                checksum: Some("1220abc".into()),
                files: 1,
                bytes: 10_777_276_416,
            }],
            tools: Tools {
                app: "0.1.0".into(),
                mkgmap: "mkgmap-r4924".into(),
                splitter: "splitter-r654".into(),
                java: "vendor/jre".into(),
            },
            output: Output {
                file: "gmapsupp.img".into(),
                bytes: 1_095_680,
                sha256: "deadbeef".into(),
                tile_count: 1,
                family_id: 39274,
                has_dem: true,
            },
            stage_seconds: vec![("extract".into(), 0.6), ("contours".into(), 24.7)],
            features_per_layer: [("tlm_strassen_strasse".to_string(), 1234u64)]
                .into_iter()
                .collect(),
            contour_lines: 8822,
            slope_areas: 0,
            warnings: vec![],
            attribution: "© swisstopo".into(),
        }
    }

    #[test]
    fn a_manifest_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("gmapsupp.img");
        std::fs::write(&output, b"not a real map").unwrap();

        let m = manifest();
        let path = m.write_beside(&output).unwrap();
        assert_eq!(path.file_name().unwrap(), "gmapsupp.manifest.json");
        assert_eq!(Manifest::read(&path).unwrap(), m);
    }

    /// The manifest must name the release a map came from, or it explains nothing.
    #[test]
    fn the_manifest_records_what_a_map_can_be_traced_by() {
        let m = manifest();
        let json = serde_json::to_string(&m).unwrap();
        for needed in [
            "swisstlm3d_2026-02", // which release
            "1220abc",            // its published checksum
            "mkgmap-r4924",       // which tools
            "deadbeef",           // the output's own hash
            "© swisstopo",        // the attribution the licence requires
            "recipeKey",          // the identity it will carry on the device
        ] {
            assert!(json.contains(needed), "manifest omits {needed}");
        }
    }

    #[test]
    fn the_manifest_sits_beside_the_output_it_describes() {
        assert_eq!(
            Manifest::path_for(Path::new("/out/build/img/gmapsupp.img")),
            PathBuf::from("/out/build/img/gmapsupp.manifest.json")
        );
        // A device file name with a map name in it keeps its stem.
        assert_eq!(
            Manifest::path_for(Path::new("/x/gmapsupp-valais.img")),
            PathBuf::from("/x/gmapsupp-valais.manifest.json")
        );
    }

    /// A multi-file dataset is one release, not fifteen sources.
    #[test]
    fn files_of_one_release_collapse_into_a_single_source() {
        use crate::cache::{Entry, Provenance};

        let provenance = |collection: &str, item: &str, file: &str| Provenance {
            collection: collection.into(),
            item: item.into(),
            datetime: Some("2026-02-25T00:00:00Z".into()),
            asset: "veloland_2056.shp.zip".into(),
            href: "https://example.test/x".into(),
            checksum: None,
            file: file.into(),
            bytes: 100,
            fetched_at: "@0".into(),
            inflated: false,
        };
        let entry = |file: &str, bytes: u64, collection: &str, item: &str| Entry {
            collection: collection.into(),
            item: item.into(),
            path: PathBuf::from(file),
            bytes,
            provenance: Some(provenance(collection, item, file)),
        };

        let sources = sources_from_cache(&[
            entry("VeloWeg.shp", 1_000, "ch.astra.veloland", "veloland"),
            entry("VeloWeg.dbf", 2_000, "ch.astra.veloland", "veloland"),
            entry("Route.shp", 3_000, "ch.astra.veloland", "veloland"),
            entry(
                "x.gpkg",
                500,
                "ch.swisstopo.swisstlm3d",
                "swisstlm3d_2026-02",
            ),
        ]);

        assert_eq!(sources.len(), 2, "{sources:#?}");
        let velo = sources
            .iter()
            .find(|s| s.collection.contains("veloland"))
            .unwrap();
        assert_eq!(velo.files, 3);
        assert_eq!(velo.bytes, 6_000, "bytes must be the release's total");
        // Sorted, so two manifests of the same cache compare cleanly.
        assert!(sources[0].collection < sources[1].collection);
    }

    #[test]
    fn entries_without_provenance_are_skipped_rather_than_invented() {
        // An entry the app did not download has nothing to record about its origin.
        let sources = sources_from_cache(&[]);
        assert!(sources.is_empty());
    }
}
