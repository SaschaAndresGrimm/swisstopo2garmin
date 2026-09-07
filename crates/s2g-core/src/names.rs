//! Label language, from swissNAMES3D (SPEC.md FR-53).
//!
//! swissTLM3D's own `name` carries several spellings separated by pipes, and **their
//! order is not by language**: `Sion | Sitten` is French then German, `Schwyz | Schwytz |
//! Svitto | Sviz` is German then French then Italian then Romansh, and
//! `Stadtkreis 3 | Wiedikon` is not languages at all. So a language cannot be chosen from
//! that field, and the first entry — the locally used name — is all it reliably gives.
//!
//! swissNAMES3D carries the same names with a `SPRACHCODE`, and joins to swissTLM3D by
//! `uuid`. Measured on the current releases (names 2026, TLM 2026): **60 of 60** sampled
//! rows matched. The 2021 release still joins at 59 of 60, so the key survives a release
//! gap. Where a feature has no name in the chosen language the local name stands, which
//! is the right answer anyway: a Valais hamlet has no German name, and inventing one
//! would be worse than leaving it alone.
//!
//! Verified end to end: a Sion 3 km build labels the town "Sion" locally and adds
//! "Sitten" when German is chosen.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Which language to label in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelLanguage {
    /// Whatever the feature carries locally. The right default for Switzerland, and the
    /// only option that needs no extra dataset.
    #[default]
    Local,
    German,
    French,
    Italian,
    Romansh,
}

impl LabelLanguage {
    pub fn all() -> &'static [LabelLanguage] {
        &[
            LabelLanguage::Local,
            LabelLanguage::German,
            LabelLanguage::French,
            LabelLanguage::Italian,
            LabelLanguage::Romansh,
        ]
    }

    pub fn id(&self) -> &'static str {
        match self {
            LabelLanguage::Local => "local",
            LabelLanguage::German => "german",
            LabelLanguage::French => "french",
            LabelLanguage::Italian => "italian",
            LabelLanguage::Romansh => "romansh",
        }
    }

    /// The `SPRACHCODE` prefix this language appears under.
    ///
    /// The values are long and carry a qualifier — "Hochdeutsch inkl. Lokalsprachen" —
    /// so matching is by prefix rather than equality.
    fn sprachcode_prefix(&self) -> Option<&'static str> {
        match self {
            LabelLanguage::Local => None,
            LabelLanguage::German => Some("Hochdeutsch"),
            LabelLanguage::French => Some("Franzoesisch"),
            LabelLanguage::Italian => Some("Italienisch"),
            LabelLanguage::Romansh => Some("Rumantsch"),
        }
    }
}

/// `uuid` to the name in the chosen language.
#[derive(Debug, Default)]
pub struct NameIndex {
    by_uuid: HashMap<String, String>,
}

impl NameIndex {
    pub fn len(&self) -> usize {
        self.by_uuid.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_uuid.is_empty()
    }

    pub fn get(&self, uuid: &str) -> Option<&str> {
        self.by_uuid.get(uuid).map(String::as_str)
    }

    /// Read the swissNAMES3D CSVs, keeping only the requested language.
    ///
    /// Streamed and filtered as it reads: the three files are 88 MB of text and only
    /// one language's names are wanted, so nothing else is ever held (NFR-2).
    pub fn load(dir: &Path, language: LabelLanguage) -> Result<NameIndex> {
        let Some(prefix) = language.sprachcode_prefix() else {
            return Ok(NameIndex::default());
        };

        let mut index = NameIndex::default();
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| Error::io(dir, e))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension().map(|x| x == "csv").unwrap_or(false)
                    && p.file_name()
                        .map(|n| n.to_string_lossy().contains("swissNAMES3D"))
                        .unwrap_or(false)
            })
            .collect();
        files.sort();

        for path in files {
            let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
            let mut lines = text.lines();
            let Some(header) = lines.next() else { continue };
            // The first column carries a UTF-8 BOM.
            let columns: Vec<&str> = header.trim_start_matches('\u{feff}').split(';').collect();
            let col = |name: &str| columns.iter().position(|c| *c == name);
            let (Some(i_uuid), Some(i_name), Some(i_lang)) =
                (col("UUID"), col("NAME"), col("SPRACHCODE"))
            else {
                return Err(Error::NotFound(format!(
                    "{}: expected UUID, NAME and SPRACHCODE columns, found {columns:?}",
                    path.display()
                )));
            };

            for line in lines {
                let fields: Vec<&str> = line.split(';').collect();
                let (Some(uuid), Some(name), Some(lang)) =
                    (fields.get(i_uuid), fields.get(i_name), fields.get(i_lang))
                else {
                    continue;
                };
                if !lang.starts_with(prefix) || name.trim().is_empty() {
                    continue;
                }
                // One name per feature; the first wins, which for a feature with two
                // names in one language is as good an answer as any.
                index
                    .by_uuid
                    .entry(uuid.trim().to_string())
                    .or_insert_with(|| name.trim().to_string());
            }
        }
        Ok(index)
    }
}

/// Where the swissNAMES3D CSVs live, if they have been acquired.
pub fn find_names3d(cache_root: &Path) -> Option<std::path::PathBuf> {
    let root = cache_root.join(crate::stac::NAMES3D);
    let mut items: Vec<_> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    items.sort();
    items.into_iter().rev().find(|item| {
        std::fs::read_dir(item)
            .map(|rd| {
                rd.flatten()
                    .any(|e| e.path().extension().map(|x| x == "csv").unwrap_or(false))
            })
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str =
        "\u{feff}UUID;OBJEKTART;OBJEKTKLASSE_TLM;NAME_UUID;NAME;STATUS;SPRACHCODE;NAMEN_TYP\n\
        {A};Ort;TLM_SIEDLUNG;{1};Sitten;offiziell;Hochdeutsch inkl. Lokalsprachen;Endonym\n\
        {A};Ort;TLM_SIEDLUNG;{2};Sion;offiziell;Franzoesisch inkl. Lokalsprachen;Endonym\n\
        {B};Ort;TLM_SIEDLUNG;{3};Coira;offiziell;Italienisch inkl. Lokalsprachen;Exonym\n\
        {B};Ort;TLM_SIEDLUNG;{4};Chur;offiziell;Hochdeutsch inkl. Lokalsprachen;Endonym\n\
        {C};Ort;TLM_SIEDLUNG;{5};Cuira;offiziell;Rumantsch Grischun inkl. Lokalsprachen;Endonym\n\
        {D};Ort;TLM_SIEDLUNG;{6};;offiziell;Franzoesisch inkl. Lokalsprachen;Endonym\n";

    fn dir_with_csv() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("swissNAMES3D_PKT.csv"), CSV).unwrap();
        dir
    }

    #[test]
    fn each_language_selects_its_own_names() {
        let dir = dir_with_csv();
        for (lang, uuid, expected) in [
            (LabelLanguage::German, "{A}", "Sitten"),
            (LabelLanguage::French, "{A}", "Sion"),
            (LabelLanguage::German, "{B}", "Chur"),
            (LabelLanguage::Italian, "{B}", "Coira"),
            (LabelLanguage::Romansh, "{C}", "Cuira"),
        ] {
            let index = NameIndex::load(dir.path(), lang).unwrap();
            assert_eq!(index.get(uuid), Some(expected), "{lang:?} {uuid}");
        }
    }

    #[test]
    fn local_needs_no_dataset_and_overrides_nothing() {
        // The default must work with no swissNAMES3D at all.
        let index = NameIndex::load(Path::new("/nonexistent"), LabelLanguage::Local).unwrap();
        assert!(index.is_empty());
        assert_eq!(index.get("{A}"), None);
    }

    #[test]
    fn a_feature_with_no_name_in_that_language_is_absent_rather_than_blank() {
        let dir = dir_with_csv();
        let index = NameIndex::load(dir.path(), LabelLanguage::Italian).unwrap();
        // {A} has German and French only.
        assert_eq!(index.get("{A}"), None);
        // An empty NAME must not become an empty label.
        assert_eq!(index.get("{D}"), None);
    }

    #[test]
    fn a_missing_dataset_is_an_error_that_names_the_directory() {
        let err =
            NameIndex::load(Path::new("/nonexistent/names"), LabelLanguage::French).unwrap_err();
        assert!(err.to_string().contains("/nonexistent/names"), "{err}");
    }

    #[test]
    fn a_csv_without_the_expected_columns_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("swissNAMES3D_PKT.csv"), "A;B;C\n1;2;3\n").unwrap();
        let err = NameIndex::load(dir.path(), LabelLanguage::German).unwrap_err();
        assert!(err.to_string().contains("SPRACHCODE"), "{err}");
    }

    #[test]
    fn the_language_ids_are_stable() {
        // They travel in saved recipes, so renaming one silently changes old recipes.
        let ids: Vec<&str> = LabelLanguage::all().iter().map(|l| l.id()).collect();
        assert_eq!(ids, ["local", "german", "french", "italian", "romansh"]);
    }
}
