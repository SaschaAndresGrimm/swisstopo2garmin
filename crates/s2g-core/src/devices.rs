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
    /// Confirmed on hardware for the Edge 840.
    #[serde(default)]
    pub supports_dem: bool,
    /// The device advertises a `Garmin/CustomMaps` directory. Its presence does not
    /// prove KMZ raster overlays work, but it contradicts the common claim that Edge
    /// devices have no raster support at all.
    #[serde(default)]
    pub supports_custom_maps_dir: bool,
    /// The device advertises a `Garmin/BirdsEye` directory.
    #[serde(default)]
    pub supports_birds_eye_dir: bool,
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
    /// Garmin part numbers seen in `GarminDevice.xml`, which identify a model more
    /// reliably than its description string.
    #[serde(default)]
    pub part_numbers: Vec<String>,
    pub storage: Storage,
    pub map_file: MapFile,
    pub rendering: Rendering,
    pub confidence: ConfidenceInfo,
}

impl DeviceProfile {
    /// Budget to build against, after the safety margin for unverified limits.
    /// Apply a user's measured limits to this profile (SPEC.md FR-DEV3).
    ///
    /// An override replaces the shipped number *and* its confidence: a user who has
    /// measured their own device knows it better than the community report we ship, so
    /// the safety factor for a guess should no longer apply.
    pub fn with_override(mut self, o: &crate::settings::DeviceOverride) -> Self {
        if o.is_empty() {
            return self;
        }
        if let Some(v) = o.map_budget_bytes {
            self.storage.recommended_map_budget_bytes = v;
        }
        if let Some(v) = o.max_img_bytes {
            self.map_file.max_img_bytes = v;
        }
        if let Some(v) = o.max_tiles_per_mapset {
            self.map_file.max_tiles_per_mapset = v;
        }
        self.confidence.level = Confidence::Measured;
        self.confidence.notes = match &o.note {
            Some(n) if !n.trim().is_empty() => format!("Measured by you: {n}"),
            _ => "Measured by you.".to_string(),
        };
        self
    }

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

/// Garmin's USB vendor id, `0x091E`.
const GARMIN_VENDOR_ID: u32 = 0x091E;

/// A Garmin device attached over USB but **not** mounted as a filesystem.
///
/// Recent Edge and fēnix models default to MTP rather than USB mass storage. macOS has
/// no MTP filesystem at all, so such a device never appears under `/Volumes` and a
/// filesystem scan cannot see it — the app looked like it was failing to detect a device
/// that was plainly plugged in.
///
/// Nothing can be written to one of these directly. Reporting it is still worth doing:
/// "your Edge 840 is connected in MTP mode, switch USB mode to mass storage" is a
/// fixable problem, and silence is not.
#[derive(Debug, Clone, PartialEq)]
pub struct UsbDevice {
    pub model: String,
    pub serial: Option<String>,
}

/// Garmin devices visible on the USB bus, mounted or not.
pub fn usb_devices() -> Vec<UsbDevice> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("ioreg")
            .args(["-r", "-c", "IOUSBHostDevice", "-l"])
            .output()
            .ok();
        let text = out
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        parse_ioreg(&text)
    }
    #[cfg(target_os = "linux")]
    {
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir("/sys/bus/usb/devices") else {
            return out;
        };
        for e in rd.flatten() {
            let dir = e.path();
            let vendor = std::fs::read_to_string(dir.join("idVendor")).unwrap_or_default();
            if u32::from_str_radix(vendor.trim(), 16) != Ok(GARMIN_VENDOR_ID) {
                continue;
            }
            let model = std::fs::read_to_string(dir.join("product"))
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if model.is_empty() {
                continue;
            }
            out.push(UsbDevice {
                serial: std::fs::read_to_string(dir.join("serial"))
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
                model,
            });
        }
        out.sort_by(|a, b| a.model.cmp(&b.model));
        out.dedup();
        out
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        // Windows browses MTP devices through Explorer and mounts mass storage as a
        // drive letter, which the volume scan already covers.
        Vec::new()
    }
}

/// Pull Garmin devices out of `ioreg -r -c IOUSBHostDevice -l` output.
///
/// Kept separate from the command so it can be tested against captured output rather
/// than against whatever happens to be plugged in.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_ioreg(text: &str) -> Vec<UsbDevice> {
    // ioreg prints one property per line and nests children, so a device's properties
    // are not delimited. Collect per indent-independent run: a new "idVendor" starts a
    // new device record.
    let mut out: Vec<UsbDevice> = Vec::new();
    let mut vendor: Option<u32> = None;
    let mut model: Option<String> = None;
    let mut serial: Option<String> = None;

    let value = |line: &str| -> Option<String> {
        let (_, rhs) = line.split_once('=')?;
        let rhs = rhs.trim();
        Some(rhs.trim_matches('"').to_string())
    };

    let mut flush = |vendor: &mut Option<u32>, model: &mut Option<String>, serial: &mut Option<String>| {
        if *vendor == Some(GARMIN_VENDOR_ID) {
            if let Some(m) = model.clone() {
                let d = UsbDevice {
                    model: m,
                    serial: serial.clone(),
                };
                // ioreg lists a device once per interface; one entry each is enough.
                if !out.contains(&d) {
                    out.push(d);
                }
            }
        }
        *vendor = None;
        *model = None;
        *serial = None;
    };

    for line in text.lines() {
        // ioreg draws a tree, so every property line carries "| " and "+-o" prefixes
        // that survive trimming: match the quoted key anywhere in the line.
        //
        // Record boundaries are "+-o" nodes. Properties within a node print in
        // dictionary order, which is not a guaranteed order, so a key arriving when it
        // already has a value also ends the record — otherwise a device whose product
        // name precedes its vendor id is attributed to the previous one.
        if line.contains("+-o") {
            flush(&mut vendor, &mut model, &mut serial);
            continue;
        }
        if line.contains("\"idVendor\"") {
            if vendor.is_some() {
                flush(&mut vendor, &mut model, &mut serial);
            }
            vendor = value(line).and_then(|v| v.trim().parse::<u32>().ok());
        } else if line.contains("\"USB Product Name\"") {
            if model.is_some() {
                flush(&mut vendor, &mut model, &mut serial);
            }
            model = value(line).filter(|s| !s.is_empty());
        } else if line.contains("\"USB Serial Number\"") {
            if serial.is_some() {
                flush(&mut vendor, &mut model, &mut serial);
            }
            serial = value(line).filter(|s| !s.is_empty());
        }
    }
    flush(&mut vendor, &mut model, &mut serial);
    out
}

#[cfg(test)]
mod usb_tests {
    use super::*;

    /// Captured from a real Edge 840 attached in MTP mode, which is the case that made
    /// the app look broken: plainly plugged in, and invisible to a volume scan.
    const IOREG_EDGE_840: &str = r#"
+-o AppleUSB20HubPort@01100000  <class AppleUSB20HubPort, id 0x100000abc>
  |   "sessionID" = 12345
  |   "USB Serial Number" = "0000d0b4a4f6"
  |   "USB Vendor Name" = "Garmin"
  |   "USB Product Name" = "Edge 840"
  |   "idVendor" = 2334
  |   "idProduct" = 20446
  |   "UsbExclusiveOwner" = "pid 32153, OpenMTP Helper ("
  | +-o IOUSBHostInterface@0  <class IOUSBHostInterface>
  | |   "idProduct" = 20446
  | |   "USB Product Name" = "Edge 840"
  | |   "USB Vendor Name" = "Garmin"
  | |   "idVendor" = 2334
  | |   "USB Serial Number" = "0000d0b4a4f6"
"#;

    #[test]
    fn a_garmin_device_in_mtp_mode_is_recognised() {
        let found = parse_ioreg(IOREG_EDGE_840);
        assert_eq!(found.len(), 1, "listed once per interface, reported once: {found:?}");
        assert_eq!(found[0].model, "Edge 840");
        assert_eq!(found[0].serial.as_deref(), Some("0000d0b4a4f6"));
    }

    #[test]
    fn devices_from_other_vendors_are_ignored() {
        let text = r#"
  |   "USB Vendor Name" = "Logitech"
  |   "USB Product Name" = "USB Receiver"
  |   "idVendor" = 1133
  |   "USB Vendor Name" = "Garmin"
  |   "USB Product Name" = "fenix 5 Plus"
  |   "idVendor" = 2334
"#;
        let found = parse_ioreg(text);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].model, "fenix 5 Plus");
        assert_eq!(found[0].serial, None);
    }

    #[test]
    fn two_garmin_devices_are_both_reported() {
        let text = r#"
  |   "USB Serial Number" = "AAA"
  |   "USB Product Name" = "Edge 840"
  |   "idVendor" = 2334
  |   "USB Serial Number" = "BBB"
  |   "USB Product Name" = "fenix 5 Plus"
  |   "idVendor" = 2334
"#;
        let found = parse_ioreg(text);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].model, "Edge 840");
        assert_eq!(found[1].model, "fenix 5 Plus");
    }

    #[test]
    fn nothing_plugged_in_is_an_empty_list_not_a_failure() {
        assert!(parse_ioreg("").is_empty());
        assert!(parse_ioreg("no properties here at all").is_empty());
        // A Garmin entry without a product name cannot be reported usefully.
        assert!(parse_ioreg("\"idVendor\" = 2334\n").is_empty());
    }

    #[test]
    fn the_vendor_id_is_garmins() {
        // 0x091E, so a decimal 2334 in ioreg output is Garmin.
        assert_eq!(GARMIN_VENDOR_ID, 2334);
    }
}
