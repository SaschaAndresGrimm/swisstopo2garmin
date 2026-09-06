//! Persistent application settings (SPEC.md FR-C2).
//!
//! Only the data location so far. It is the one setting that has to be persistent and
//! movable: the national GeoPackage alone is 10.0 GB inflated, plus elevation tiles and
//! build working files, so a laptop with a small internal disk needs to point this at an
//! external volume before the first download rather than after it.
//!
//! Precedence, highest first: the `S2G_CACHE` environment variable, the saved setting,
//! then the platform default. The environment variable wins so a build script or a test
//! can redirect the cache without disturbing what the user chose.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Where datasets and build working files live. `None` means the default.
    #[serde(default)]
    pub data_root: Option<PathBuf>,
}

/// Where the settings file itself lives.
///
/// Deliberately *not* inside the data directory: it records where that directory is, so
/// storing it there would be circular.
pub fn settings_path() -> PathBuf {
    if let Ok(p) = std::env::var("S2G_SETTINGS") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join(".config")
        .join("swisstopo2garmin")
        .join("settings.json")
}

impl Settings {
    pub fn load_from(path: &Path) -> Settings {
        // A missing or unreadable settings file must never stop the app starting; the
        // defaults are always usable.
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn load() -> Settings {
        Settings::load_from(&settings_path())
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        // Through a temporary file: an interrupted save must not leave a settings file
        // that fails to parse and silently resets the data location.
        let tmp = path.with_extension("json.part");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?).map_err(|e| Error::io(&tmp, e))?;
        std::fs::rename(&tmp, path).map_err(|e| Error::io(path, e))
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&settings_path())
    }

    /// The platform default, used when nothing else is set.
    pub fn default_data_root() -> PathBuf {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".cache").join("swisstopo2garmin")
    }

    /// The data root this settings value implies, ignoring the environment.
    pub fn resolved_data_root(&self) -> PathBuf {
        self.data_root
            .clone()
            .unwrap_or_else(Settings::default_data_root)
    }
}

/// The data root in force right now.
pub fn data_root() -> PathBuf {
    if let Ok(p) = std::env::var("S2G_CACHE") {
        return PathBuf::from(p);
    }
    Settings::load().resolved_data_root()
}

/// Why a directory cannot be used for data, if it cannot.
///
/// Checked before saving rather than at the first download, because the failure would
/// otherwise land four gigabytes into a transfer.
pub fn check_writable(dir: &Path) -> std::result::Result<(), String> {
    if !dir.is_absolute() {
        return Err(format!("{} is not an absolute path", dir.display()));
    }
    if dir.exists() && !dir.is_dir() {
        return Err(format!("{} exists and is not a directory", dir.display()));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let probe = dir.join(".s2g-write-test");
    std::fs::write(&probe, b"ok").map_err(|e| format!("cannot write in {}: {e}", dir.display()))?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_settings_file_yields_defaults() {
        let s = Settings::load_from(Path::new("/nonexistent/s2g/settings.json"));
        assert_eq!(s, Settings::default());
        assert_eq!(s.resolved_data_root(), Settings::default_data_root());
    }

    #[test]
    fn a_corrupt_settings_file_yields_defaults_rather_than_failing_startup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, b"{ not json").unwrap();
        assert_eq!(Settings::load_from(&path), Settings::default());
    }

    #[test]
    fn a_saved_data_root_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        let s = Settings {
            data_root: Some(PathBuf::from("/Volumes/Maps/s2g")),
        };
        s.save_to(&path).unwrap();
        assert_eq!(Settings::load_from(&path), s);
        assert_eq!(
            Settings::load_from(&path).resolved_data_root(),
            PathBuf::from("/Volumes/Maps/s2g")
        );
        // The temporary file must not be left behind.
        assert!(!path.with_extension("json.part").exists());
    }

    #[test]
    fn saving_replaces_rather_than_appends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        Settings {
            data_root: Some(PathBuf::from("/first")),
        }
        .save_to(&path)
        .unwrap();
        Settings {
            data_root: Some(PathBuf::from("/second")),
        }
        .save_to(&path)
        .unwrap();
        assert_eq!(
            Settings::load_from(&path).data_root,
            Some(PathBuf::from("/second"))
        );
    }

    #[test]
    fn a_writable_directory_is_accepted_and_left_clean() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("data");
        assert_eq!(check_writable(&target), Ok(()));
        assert!(target.is_dir());
        // The probe file must not survive.
        assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
    }

    #[test]
    fn a_relative_path_is_rejected_with_the_reason() {
        let err = check_writable(Path::new("relative/path")).unwrap_err();
        assert!(err.contains("absolute"), "{err}");
    }

    #[test]
    fn a_path_that_is_a_file_is_rejected_with_the_reason() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a-file");
        std::fs::write(&file, b"x").unwrap();
        let err = check_writable(&file).unwrap_err();
        assert!(err.contains("not a directory"), "{err}");
    }
}
