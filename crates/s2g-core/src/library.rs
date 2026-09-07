//! The saved-recipe library (SPEC.md FR-55).
//!
//! Recipes are plain JSON files in one directory. A directory rather than a database so
//! a user can copy, diff, mail or version-control a recipe, and so a corrupt file loses
//! one recipe instead of all of them.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{Error, Result};
use crate::recipe::Recipe;

/// Turn a display name into a file stem.
///
/// Recipe names come from user input and from place names, so they can contain slashes,
/// dots and non-ASCII. Anything outside `[A-Za-z0-9]` becomes a hyphen, which is dull
/// but cannot escape the library directory.
pub fn safe_stem(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "recipe".into()
    } else {
        trimmed
    }
}

/// One entry as the library list shows it.
#[derive(Debug, Clone, Serialize)]
pub struct SavedRecipe {
    /// File stem, which is the id used to load and delete.
    pub id: String,
    pub name: String,
    pub device_id: String,
    pub preset: String,
    /// Human-readable area, for the list.
    pub area_label: String,
    pub area_km2: f64,
    /// Unix seconds, or 0 when the filesystem does not report it.
    pub saved_at: u64,
}

fn path_for(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{}.json", safe_stem(id)))
}

/// Write a recipe, overwriting an earlier one with the same name.
///
/// Returns the id it was stored under, which is not always derivable from the name
/// (two names can collapse to the same stem).
pub fn save(dir: &Path, recipe: &Recipe) -> Result<String> {
    std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    let id = safe_stem(&recipe.name);
    let path = path_for(dir, &id);
    // Write through a temporary file: an interrupted save must not leave a truncated
    // recipe where a valid one was.
    let tmp = path.with_extension("json.part");
    let json = serde_json::to_vec_pretty(recipe)?;
    std::fs::write(&tmp, &json).map_err(|e| Error::io(&tmp, e))?;
    std::fs::rename(&tmp, &path).map_err(|e| Error::io(&path, e))?;
    Ok(id)
}

pub fn load(dir: &Path, id: &str) -> Result<Recipe> {
    let path = path_for(dir, id);
    let bytes = std::fs::read(&path).map_err(|e| Error::io(&path, e))?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn delete(dir: &Path, id: &str) -> Result<()> {
    let path = path_for(dir, id);
    std::fs::remove_file(&path).map_err(|e| Error::io(&path, e))
}

/// List the library, newest first. Unreadable files are skipped, not fatal.
pub fn list(dir: &Path) -> Vec<SavedRecipe> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<SavedRecipe> = rd
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension().map(|x| x != "json").unwrap_or(true) {
                return None;
            }
            let recipe: Recipe = serde_json::from_slice(&std::fs::read(&path).ok()?).ok()?;
            let bbox = recipe.area.bbox();
            Some(SavedRecipe {
                id: path.file_stem()?.to_string_lossy().to_string(),
                // Composed from the two accessors on AreaSelection, which is also
                // what the area step's "currently selected" chip reads -- this match
                // used to live here inline and be the only place that could describe a
                // selection.
                area_label: match &recipe.area {
                    crate::recipe::AreaSelection::Circle { .. } => {
                        format!("circle · {}", recipe.area.detail())
                    }
                    crate::recipe::AreaSelection::Polygon { .. } => {
                        format!("{} points", recipe.area.detail())
                    }
                    crate::recipe::AreaSelection::Composite { .. } => {
                        format!("{} areas", recipe.area.detail())
                    }
                    other => other.detail(),
                },
                area_km2: bbox.area_km2(),
                name: recipe.name,
                device_id: recipe.device_id,
                preset: recipe.preset.id().to_string(),
                saved_at: e
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            })
        })
        .collect();
    out.sort_by(|a, b| b.saved_at.cmp(&a.saved_at).then(a.name.cmp(&b.name)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::{AreaSelection, Preset};

    fn recipe(name: &str) -> Recipe {
        Recipe::new(
            name,
            "edge-840",
            AreaSelection::Place {
                name: "Grindelwald".into(),
                radius_km: 8.0,
                easting: 2_645_921.0,
                northing: 1_163_748.0,
            },
        )
        .with_preset(Preset::Skimo)
    }

    #[test]
    fn safe_stem_collapses_runs_and_keeps_something() {
        assert_eq!(safe_stem("Valais hiking"), "valais-hiking");
        assert_eq!(safe_stem("../../etc/passwd"), "etc-passwd");
        assert_eq!(safe_stem("Zermatt // 10 km"), "zermatt-10-km");
        assert_eq!(safe_stem("Grächen"), "gr-chen");
        assert_eq!(safe_stem("///"), "recipe");
    }

    #[test]
    fn round_trip_preserves_every_field() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = recipe("Valais skimo");
        r.excluded_layers = vec!["tlm_bauten_gebaeude_footprint".into()];
        let id = save(dir.path(), &r).unwrap();
        assert_eq!(id, "valais-skimo");

        let back = load(dir.path(), &id).unwrap();
        assert_eq!(back.name, r.name);
        assert_eq!(back.device_id, r.device_id);
        assert_eq!(back.preset, r.preset);
        assert_eq!(back.excluded_layers, r.excluded_layers);
        assert_eq!(back.cache_key(), r.cache_key());
    }

    #[test]
    fn saving_the_same_name_replaces_rather_than_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = recipe("Valais skimo");
        save(dir.path(), &r).unwrap();
        r.contours.interval_m = 50;
        save(dir.path(), &r).unwrap();

        let listed = list(dir.path());
        assert_eq!(listed.len(), 1);
        assert_eq!(
            load(dir.path(), &listed[0].id).unwrap().contours.interval_m,
            50
        );
    }

    #[test]
    fn list_skips_unreadable_files_instead_of_failing() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), &recipe("Good one")).unwrap();
        std::fs::write(dir.path().join("broken.json"), b"{not json").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"ignored").unwrap();

        let listed = list(dir.path());
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Good one");
        assert_eq!(listed[0].preset, "skimo");
        assert_eq!(listed[0].area_label, "Grindelwald · 8 km");
    }

    #[test]
    fn delete_removes_only_the_named_recipe() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), &recipe("First")).unwrap();
        save(dir.path(), &recipe("Second")).unwrap();
        delete(dir.path(), "first").unwrap();

        let names: Vec<_> = list(dir.path()).into_iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["Second"]);
    }

    #[test]
    fn listing_an_absent_directory_is_empty_not_an_error() {
        assert!(list(Path::new("/nonexistent/s2g/recipes")).is_empty());
    }

    #[test]
    fn a_bbox_recipe_gets_a_dimension_label() {
        let dir = tempfile::tempdir().unwrap();
        let r = Recipe::new(
            "Custom",
            "fenix-5-plus",
            AreaSelection::BBox {
                min_e: 2_600_000.0,
                min_n: 1_100_000.0,
                max_e: 2_630_000.0,
                max_n: 1_120_000.0,
            },
        );
        save(dir.path(), &r).unwrap();
        let listed = list(dir.path());
        assert_eq!(listed[0].area_label, "30 × 20 km");
        assert_eq!(listed[0].area_km2, 600.0);
    }
}
