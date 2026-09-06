//! Device profiles and connected-device detection (SPEC.md §5.3, FR-DEV1..FR-DEV5).
//!
//! Garmin publishes none of these limits, and the figures circulating in forums are
//! inconsistent and often stale — the widely repeated "fenix maps must be under 20 MB"
//! is fenix 3-era and does not apply to a watch that ships multi-gigabyte TopoActive
//! maps from the factory. So every numeric limit carries a [`Confidence`] level and a
//! source list, anything below `Measured` gets a safety margin, and the UI is expected
//! to show the difference.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// How much a profile's numbers can be trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// Guessed conservatively; no source.
    Assumed,
    /// Reported by users, not verified here.
    Community,
    /// Measured on hardware per docs/device-verification.md.
    Measured,
    /// Published by Garmin.
    Vendor,
}

impl Confidence {
    /// Fraction of a stated budget to actually use.
    ///
    /// An unverified limit gets a 20% margin, because being wrong means a map that
    /// silently fails on the device (FR-DEV2).
    pub fn safety_factor(&self) -> f64 {
        match self {
            Confidence::Vendor | Confidence::Measured => 1.0,
            Confidence::Community => 0.8,
            Confidence::Assumed => 0.7,
        }
    }

    pub fn is_verified(&self) -> bool {
        matches!(self, Confidence::Vendor | Confidence::Measured)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScreenClass {
    /// Edge and similar: a large screen with rendering headroom.
    Handlebar,
    /// A watch: ~1.3 inch, far less headroom, needs the reduced style (FR-CART6).
    Wrist,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Storage {
    pub internal_bytes: Option<u64>,
    pub recommended_map_budget_bytes: u64,
    #[serde(default)]
    pub has_removable_storage: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapFile {
    pub max_img_bytes: u64,
    pub max_tiles_per_mapset: usize,
    pub install_paths: Vec<String>,
    /// True when the file must be named literally `gmapsupp.img`.
    #[serde(default)]
    pub requires_exact_filename: bool,
    #[serde(default)]
    pub supports_multiple_mapsets: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rendering {
    #[serde(default)]
    pub supports_typ_file: bool,
    #[serde(default)]
    pub supports_routable_maps: bool,
    /// Device-side shaded relief from an embedded DEM (FR-CART8).
    #[serde(default)]
    pub supports_dem: bool,
    #[serde(default)]
    pub displays_street_names: bool,
    pub screen_class: ScreenClass,
    #[serde(default)]
    pub recommended_max_detail_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceInfo {
    pub level: Confidence,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub last_verified: Option<String>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceProfile {
    pub id: String,
    pub display_name: String,
    pub family: String,
    #[serde(default)]
    pub generation: Option<u32>,
    /// Model strings from `GarminDevice.xml` that select this profile.
    #[serde(default)]
    pub match_models: Vec<String>,
    pub storage: Storage,
    pub map_file: MapFile,
    pub rendering: Rendering,
    pub confidence: ConfidenceInfo,
}

impl DeviceProfile {
    /// Budget to build against, after the safety margin for unverified limits.
    pub fn effective_budget_bytes(&self) -> u64 {
        (self.storage.recommended_map_budget_bytes as f64 * self.confidence.level.safety_factor())
            as u64
    }

    /// Hard ceiling for a single file, after the safety margin.
    pub fn effective_max_img_bytes(&self) -> u64 {
        (self.map_file.max_img_bytes as f64 * self.confidence.level.safety_factor()) as u64
    }

    pub fn is_wrist(&self) -> bool {
        self.rendering.screen_class == ScreenClass::Wrist
    }

    /// Filename to write on the device.
    pub fn output_filename(&self, map_name: &str) -> String {
        if self.map_file.requires_exact_filename || !self.map_file.supports_multiple_mapsets {
            "gmapsupp.img".to_string()
        } else {
            let slug: String = map_name
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect();
            format!("gmapsupp-{}.img", slug.trim_matches('-'))
        }
    }
}

/// Load every profile in a directory.
pub fn load_profiles(dir: &Path) -> Result<Vec<DeviceProfile>> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| Error::io(dir, e))?;
    for entry in entries {
        let path = entry.map_err(|e| Error::io(dir, e))?.path();
        if path.extension().map(|x| x != "json").unwrap_or(true) {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        let profile: DeviceProfile = serde_json::from_str(&text)
            .map_err(|e| Error::Zip(format!("{}: {e}", path.display())))?;
        out.push(profile);
    }
    out.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(out)
}

/// Pick a profile for a model string reported by a device, falling back to the generic
/// profile for its family so an unlisted model is never a dead end (FR-21).
pub fn match_profile<'a>(
    profiles: &'a [DeviceProfile],
    model: &str,
    is_wrist: bool,
) -> Option<&'a DeviceProfile> {
    let want = model.trim().to_lowercase();
    profiles
        .iter()
        .find(|p| {
            p.match_models
                .iter()
                .any(|m| m.trim().to_lowercase() == want)
        })
        .or_else(|| {
            profiles.iter().find(|p| {
                p.id == if is_wrist {
                    "generic-fenix"
                } else {
                    "generic-edge"
                }
            })
        })
}

// ---------------------------------------------------------------------------
// Connected devices
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct DetectedDevice {
    /// Mount point of the mass-storage volume.
    pub mount: PathBuf,
    /// Model description from `GarminDevice.xml`, when readable.
    pub model: Option<String>,
    pub unit_id: Option<String>,
    /// Existing map files found on the device.
    pub existing_maps: Vec<PathBuf>,
    pub free_bytes: Option<u64>,
}

/// Directories to probe for mounted removable volumes.
fn volume_roots() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    return vec![PathBuf::from("/Volumes")];
    #[cfg(target_os = "linux")]
    return vec![
        PathBuf::from("/media"),
        PathBuf::from("/run/media"),
        PathBuf::from("/mnt"),
    ];
    #[cfg(target_os = "windows")]
    return ('D'..='Z')
        .map(|c| PathBuf::from(format!("{c}:\\")))
        .collect();
}

/// The `Garmin` directory on a volume, whichever case the device uses.
fn garmin_dir(mount: &Path) -> Option<PathBuf> {
    for name in ["Garmin", "GARMIN", "garmin"] {
        let p = mount.join(name);
        if p.is_dir() {
            return Some(p);
        }
    }
    None
}

/// Find connected Garmin mass-storage devices.
///
/// Detection is read-only. Nothing is ever written without explicit confirmation
/// (FR-DEV5).
pub fn detect() -> Vec<DetectedDevice> {
    let mut found = Vec::new();
    for root in volume_roots() {
        let candidates: Vec<PathBuf> = if root.components().count() == 1 {
            vec![root.clone()] // a Windows drive letter is itself the mount
        } else {
            std::fs::read_dir(&root)
                .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
                .unwrap_or_default()
        };

        for mount in candidates {
            let Some(gdir) = garmin_dir(&mount) else {
                continue;
            };
            let (model, unit_id) = read_device_xml(&gdir).unwrap_or((None, None));
            found.push(DetectedDevice {
                existing_maps: find_maps(&gdir),
                free_bytes: crate::cache::available_bytes(&mount),
                mount,
                model,
                unit_id,
            });
        }
    }
    found
}

/// Extract the model description and unit id from `GarminDevice.xml`.
///
/// Parsed with a targeted scan rather than a full XML parser: the file is small,
/// device-written, and only two fields are needed.
fn read_device_xml(garmin_dir: &Path) -> Option<(Option<String>, Option<String>)> {
    let mut path = garmin_dir.join("GarminDevice.xml");
    if !path.exists() {
        path = garmin_dir.join("garmindevice.xml");
    }
    let text = std::fs::read_to_string(path).ok()?;
    Some((
        tag_text(&text, "Description").or_else(|| tag_text(&text, "PartNumber")),
        tag_text(&text, "UnitId"),
    ))
}

fn tag_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    let v = xml[start..end].trim();
    (!v.is_empty()).then(|| v.to_string())
}

fn find_maps(garmin_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for sub in [garmin_dir.to_path_buf(), garmin_dir.join("Maps")] {
        if let Ok(rd) = std::fs::read_dir(&sub) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension()
                    .map(|x| x.eq_ignore_ascii_case("img"))
                    .unwrap_or(false)
                {
                    out.push(p);
                }
            }
        }
    }
    out.sort();
    out
}

/// Guard against the classic mistake on a system that hides file extensions.
pub fn is_double_extension(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".img.img") || lower.matches(".img").count() > 1
}
