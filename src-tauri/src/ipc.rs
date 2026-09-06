//! Typed IPC surface (SPEC.md §10.4).
//!
//! Every type here derives `TS`, and `cargo test -p swisstopo2garmin` writes the
//! TypeScript definitions to `frontend/src/state/bindings.ts`. CI fails if the checked-in
//! file differs, so the two sides cannot drift.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use s2g_core::cache::{available_bytes, Cache, Provenance};
use s2g_core::download::{
    download, download_zip_all, download_zip_member_inflated, Cancel, Progress,
};
use s2g_core::http::ReqwestHttp;
use s2g_core::stac::Stac;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use ts_rs::TS;

#[derive(Default)]
pub struct AppState {
    tasks: Mutex<HashMap<String, Cancel>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct DatasetEntry {
    pub collection: String,
    pub item: String,
    pub file: String,
    #[ts(type = "number")]
    pub bytes: u64,
    pub inflated: bool,
    pub fetched_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct CacheStatus {
    pub root: String,
    #[ts(type = "number")]
    pub total_bytes: u64,
    #[ts(type = "number | null")]
    pub free_bytes: Option<u64>,
    pub entries: Vec<DatasetEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub collection: String,
    pub item: String,
    pub datetime: Option<String>,
    pub asset: String,
    pub href: String,
    /// Wire size of the archive, when the server reports it.
    #[ts(type = "number | null")]
    pub archive_bytes: Option<u64>,
    /// Inflated size of the member inside the archive.
    #[ts(type = "number | null")]
    pub member_bytes: Option<u64>,
    pub member_name: Option<String>,
    pub cached: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub task_id: String,
    #[ts(type = "number")]
    pub read: u64,
    #[ts(type = "number | null")]
    pub total: Option<u64>,
    #[ts(type = "number")]
    pub written: u64,
    pub retries: u32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct TaskDone {
    pub task_id: String,
    pub path: String,
}

/// A build failure, interpreted (SPEC.md FR-73).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct BuildFailure {
    pub task_id: String,
    /// One sentence naming what failed.
    pub summary: String,
    /// What to do about it, when there is something specific to say.
    pub suggestion: Option<String>,
    /// False when the cause was not recognised, so the UI can say so plainly.
    pub recognised: bool,
    /// The raw error, for the details pane and for copy-diagnostics.
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct TaskError {
    pub task_id: String,
    pub message: String,
}

// NOTE: Tauri sends u64 over IPC as a JSON number, so every u64 is exported to
// TypeScript as `number` rather than ts-rs's default `bigint`. Our largest value is
// the 10.78 GB GeoPackage (~1.1e10), comfortably inside Number.MAX_SAFE_INTEGER.

/// Minimum interval between `task:progress` events sent to the UI.
const PROGRESS_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

type IpcResult<T> = Result<T, String>;

fn cache() -> Cache {
    Cache::new(Cache::default_root())
}

#[tauri::command]
pub async fn cache_status() -> IpcResult<CacheStatus> {
    let c = cache();
    let entries = c.list().await.map_err(|e| e.to_string())?;
    let total_bytes = entries.iter().map(|e| e.bytes).sum();
    Ok(CacheStatus {
        root: c.root().display().to_string(),
        total_bytes,
        free_bytes: available_bytes(c.root()),
        entries: entries
            .into_iter()
            .map(|e| DatasetEntry {
                collection: e.collection,
                item: e.item,
                file: e
                    .path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default(),
                bytes: e.bytes,
                inflated: e.provenance.as_ref().map(|p| p.inflated).unwrap_or(false),
                fetched_at: e.provenance.map(|p| p.fetched_at),
            })
            .collect(),
    })
}

#[tauri::command]
pub async fn latest_release(collection: String) -> IpcResult<ReleaseInfo> {
    let http = ReqwestHttp::new().map_err(|e| e.to_string())?;
    let stac = Stac::new(&http);
    let item = stac.latest(&collection).await.map_err(|e| e.to_string())?;
    // Clone so `item` is free to be consumed below.
    let (asset, kind) = item.primary_asset().map_err(|e| e.to_string())?;
    let (asset, kind) = (asset.clone(), kind);

    // Probe the archive so the UI can state both the download size and the far larger
    // on-disk size before the user commits (SPEC.md FR-C1).
    let (archive_bytes, member_bytes, member_name) = {
        use s2g_core::http::Http;
        let total = http.head(&asset.href).await.ok().and_then(|h| h.len);
        match (total, kind.is_archive()) {
            // A bare GeoPackage is its own member: nothing to probe, and probing it as
            // a zip is what made the Data screen show an error for it.
            (Some(total), false) => (Some(total), Some(total), Some(asset.name.clone())),
            (Some(total), true) => {
                match s2g_core::zip::first_member(&http, &asset.href, total).await {
                    Ok(m) => (Some(total), Some(m.uncompressed_size), Some(m.name)),
                    // A multi-member archive still reports its download size honestly.
                    Err(_) => (Some(total), None, None),
                }
            }
            (None, _) => (None, None, None),
        }
    };

    let cached = cache()
        .read_provenance(&collection, &item.id)
        .await
        .is_some();

    Ok(ReleaseInfo {
        collection,
        item: item.id,
        datetime: item.datetime,
        asset: asset.name.clone(),
        href: asset.href.clone(),
        archive_bytes,
        member_bytes,
        member_name,
        cached,
    })
}

/// Start acquiring a dataset. Returns a task id immediately; progress arrives as
/// `task:progress`, then `task:done` or `task:error`.
#[tauri::command]
pub async fn acquire_dataset(
    app: AppHandle,
    state: State<'_, AppState>,
    collection: String,
) -> IpcResult<String> {
    let http = ReqwestHttp::new().map_err(|e| e.to_string())?;
    let stac = Stac::new(&http);
    let item = stac.latest(&collection).await.map_err(|e| e.to_string())?;
    // How the data is packaged decides how it is acquired. The collection id does not:
    // the six winter and route collections publish three different packagings between
    // them, and asking every one of them for a `.gpkg.zip` is what broke the Data
    // screen for half of them.
    let (asset, kind) = item.primary_asset().map_err(|e| e.to_string())?;
    let (asset, kind) = (asset.clone(), kind);

    // Only swissTLM3D is streamed and inflated as it downloads. It is the one archive
    // where that matters — 4.5 GB compressed to 10.0 GB inflated, so keeping both would
    // need 15 GB of disk — and the one archive that really holds a single member.
    //
    // Everything else is downloaded whole and extracted. The ASTRA route networks ship
    // a dozen shapefile components, and the SAC skitouren archive holds *two*
    // GeoPackages: taking only the first would silently drop the ski network, which
    // carries the skiable / carrying / caution classification.
    let stream_inflate = collection == s2g_core::stac::TLM3D;

    let task_id = format!("{}:{}", collection, item.id);
    let cancel = Cancel::new();
    state
        .tasks
        .lock()
        .expect("task registry poisoned")
        .insert(task_id.clone(), cancel.clone());

    let c = cache();
    let dir = c
        .ensure_dir(&collection, &item.id)
        .await
        .map_err(|e| e.to_string())?;

    let id = task_id.clone();
    let datetime = item.datetime.clone();
    let item_id = item.id.clone();
    tauri::async_runtime::spawn(async move {
        let http = match ReqwestHttp::new() {
            Ok(h) => h,
            Err(e) => {
                let _ = app.emit(
                    "task:error",
                    TaskError {
                        task_id: id,
                        message: e.to_string(),
                    },
                );
                return;
            }
        };

        // Provisional destination; the real member name is known after probing, so the
        // downloader renames into place once verified.
        let dest = dir.join(format!("{item_id}.gpkg"));
        let emit = app.clone();
        let progress_id = id.clone();
        // The downloader reports every chunk, which is thousands of events per second
        // over a 4.8 GB transfer. The UI cannot use that, and flooding the IPC channel
        // competes with the download itself, so emit at most every 100 ms.
        let mut last_emit = std::time::Instant::now() - PROGRESS_INTERVAL;
        let mut on_progress = move |p: Progress| {
            let now = std::time::Instant::now();
            let complete = p.total.is_some_and(|t| p.read >= t);
            if !complete && now.duration_since(last_emit) < PROGRESS_INTERVAL {
                return;
            }
            last_emit = now;
            let _ = emit.emit(
                "task:progress",
                TaskProgress {
                    task_id: progress_id.clone(),
                    read: p.read,
                    total: p.total,
                    written: p.written,
                    retries: p.retries,
                },
            );
        };

        let result = if !kind.is_archive() {
            // Published as a plain GeoPackage: download it as it is.
            let dest = dir.join(&asset.name);
            download(
                &http,
                &asset.href,
                &dest,
                asset.checksum.as_ref(),
                &cancel,
                &mut on_progress,
            )
            .await
            .map(|path| {
                let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                (
                    path,
                    s2g_core::zip::Member {
                        name: asset.name.clone(),
                        data_start: 0,
                        compressed_size: bytes,
                        uncompressed_size: bytes,
                        method: 0,
                    },
                )
            })
        } else if !stream_inflate {
            download_zip_all(
                &http,
                &asset.href,
                &dir,
                asset.checksum.as_ref(),
                &cancel,
                &mut on_progress,
            )
            .await
            // Report the first extracted file, with the archive's own size: a
            // multi-file dataset has no single member to name.
            .map(|files| {
                let bytes = files
                    .iter()
                    .filter_map(|f| std::fs::metadata(f).ok().map(|m| m.len()))
                    .sum();
                (
                    files.first().cloned().unwrap_or_else(|| dir.clone()),
                    s2g_core::zip::Member {
                        name: asset.name.clone(),
                        data_start: 0,
                        compressed_size: 0,
                        uncompressed_size: bytes,
                        method: 8,
                    },
                )
            })
        } else {
            download_zip_member_inflated(
                &http,
                &asset.href,
                &dest,
                asset.checksum.as_ref(),
                &cancel,
                &mut on_progress,
            )
            .await
        };

        match result {
            Ok((path, member)) => {
                // Record where the bytes came from, so a build is reproducible (FR-D4).
                let prov = Provenance {
                    collection: collection.clone(),
                    item: item_id.clone(),
                    datetime,
                    asset: asset.name.clone(),
                    href: asset.href.clone(),
                    checksum: asset.checksum.clone(),
                    file: path
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    bytes: member.uncompressed_size,
                    fetched_at: now_rfc3339(),
                    inflated: true,
                };
                let _ = Cache::new(Cache::default_root())
                    .write_provenance(&prov)
                    .await;
                let _ = app.emit(
                    "task:done",
                    TaskDone {
                        task_id: id,
                        path: path.display().to_string(),
                    },
                );
            }
            Err(e) => {
                eprintln!("acquisition failed for {id}: {e}");
                if let Err(emit_err) = app.emit(
                    "task:error",
                    TaskError {
                        task_id: id,
                        message: e.to_string(),
                    },
                ) {
                    eprintln!("failed to emit task:error: {emit_err}");
                }
            }
        }
    });

    Ok(task_id)
}

#[tauri::command]
pub fn cancel_task(state: State<'_, AppState>, task_id: String) -> IpcResult<()> {
    if let Some(c) = state
        .tasks
        .lock()
        .expect("task registry poisoned")
        .get(&task_id)
    {
        c.cancel();
    }
    Ok(())
}

#[tauri::command]
pub async fn remove_dataset(collection: String, item: String) -> IpcResult<()> {
    cache()
        .remove(&collection, &item)
        .await
        .map_err(|e| e.to_string())
}

/// UTC timestamp for provenance. Written by hand rather than pulling in a date-time
/// crate for a single string; the civil-from-days algorithm is Howard Hinnant's.
fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn timestamp_is_rfc3339() {
        let s = super::now_rfc3339();
        assert_eq!(s.len(), 20, "{s}");
        assert!(s.ends_with('Z'), "{s}");
        let (date, time) = s[..19].split_once('T').expect("T separator");
        let parts: Vec<&str> = date.split('-').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts[0].parse::<i32>().unwrap() >= 2024, "{s}");
        assert!((1..=12).contains(&parts[1].parse::<u32>().unwrap()), "{s}");
        assert!((1..=31).contains(&parts[2].parse::<u32>().unwrap()), "{s}");
        assert_eq!(time.split(':').count(), 3, "{s}");
    }
}

// ---------------------------------------------------------------------------
// Devices, presets, area and build (SPEC.md §6.3-6.7)
// ---------------------------------------------------------------------------

use s2g_core::devices::{self, DeviceProfile};
use s2g_core::extract::{LayerGroup, CYCLE_LAYERS, DEFAULT_LAYERS, WINTER_LAYERS};
use s2g_core::boundaries::{self, AdminLevel};
use s2g_core::estimate::{self, calibration_log_path};
use s2g_core::library;
use s2g_core::pipeline::{self, BuildContext, Stage};
use s2g_core::proj::{lv95_to_wgs84, BBox};
use s2g_core::recipe::{Preset, Recipe, ReliefDetail};

/// Directory holding the app's data files. In development this is the repo; in a
/// bundle it is the resource directory.
/// A user's override of one device's limits (SPEC.md FR-DEV3).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct DeviceOverrideInfo {
    #[ts(type = "number | null")]
    pub map_budget_bytes: Option<u64>,
    #[ts(type = "number | null")]
    pub max_img_bytes: Option<u64>,
    pub max_tiles_per_mapset: Option<usize>,
    pub note: Option<String>,
}

#[tauri::command]
pub fn device_override(device_id: String) -> IpcResult<DeviceOverrideInfo> {
    let o = s2g_core::settings::Settings::load()
        .device_overrides
        .get(&device_id)
        .cloned()
        .unwrap_or_default();
    Ok(DeviceOverrideInfo {
        map_budget_bytes: o.map_budget_bytes,
        max_img_bytes: o.max_img_bytes,
        max_tiles_per_mapset: o.max_tiles_per_mapset,
        note: o.note,
    })
}

/// Record limits the user has measured, or clear them.
///
/// Stored in the settings file rather than in `devices/*.json`, so an app update that
/// ships new profiles cannot silently discard them (FR-DEV3).
#[tauri::command]
pub fn set_device_override(
    device_id: String,
    value: Option<DeviceOverrideInfo>,
) -> IpcResult<()> {
    let mut settings = s2g_core::settings::Settings::load();
    match value {
        None => {
            settings.device_overrides.remove(&device_id);
        }
        Some(v) => {
            let o = s2g_core::settings::DeviceOverride {
                map_budget_bytes: v.map_budget_bytes,
                max_img_bytes: v.max_img_bytes,
                max_tiles_per_mapset: v.max_tiles_per_mapset,
                note: v.note,
            };
            if o.is_empty() {
                settings.device_overrides.remove(&device_id);
            } else {
                settings.device_overrides.insert(device_id, o);
            }
        }
    }
    settings.save().map_err(|e| e.to_string())
}

/// Device profiles with the user's measured overrides applied (SPEC.md FR-DEV3).
///
/// Every caller goes through here rather than `devices::load_profiles` directly: an
/// override that applied on one screen and not another would look like a bug in the
/// override, and be very hard to see.
fn profiles() -> Result<Vec<DeviceProfile>, String> {
    let overrides = s2g_core::settings::Settings::load().device_overrides;
    Ok(
        devices::load_profiles(&resource_root().join("devices"))
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|p| match overrides.get(&p.id) {
                Some(o) => p.with_override(o),
                None => p,
            })
            .collect(),
    )
}

fn resource_root() -> PathBuf {
    if let Ok(p) = std::env::var("S2G_ROOT") {
        return PathBuf::from(p);
    }
    // Walk up from the executable, then from the working directory, looking for the
    // marker files a build needs. Keeps `cargo run` and a bundle both working.
    let candidates = std::env::current_exe()
        .ok()
        .into_iter()
        .flat_map(|exe| exe.ancestors().map(Path::to_path_buf).collect::<Vec<_>>())
        .chain(
            std::env::current_dir()
                .ok()
                .into_iter()
                .flat_map(|d| d.ancestors().map(Path::to_path_buf).collect::<Vec<_>>()),
        );
    for c in candidates {
        if c.join("devices").is_dir() && c.join("style").is_dir() {
            return c;
        }
    }
    PathBuf::from(".")
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct DeviceSummary {
    pub id: String,
    pub display_name: String,
    pub family: String,
    pub screen_class: String,
    /// vendor | measured | community | assumed
    pub confidence: String,
    pub confidence_verified: bool,
    pub confidence_notes: String,
    #[ts(type = "number")]
    pub budget_bytes: u64,
    #[ts(type = "number")]
    pub max_img_bytes: u64,
    pub max_tiles: usize,
    pub supports_dem: bool,
    /// True when this profile was matched to a connected device.
    pub connected: bool,
}

fn summarise(p: &DeviceProfile, connected: bool) -> DeviceSummary {
    DeviceSummary {
        id: p.id.clone(),
        display_name: p.display_name.clone(),
        family: p.family.clone(),
        screen_class: if p.is_wrist() { "wrist" } else { "handlebar" }.into(),
        confidence: format!("{:?}", p.confidence.level).to_lowercase(),
        confidence_verified: p.confidence.level.is_verified(),
        confidence_notes: p.confidence.notes.clone(),
        budget_bytes: p.effective_budget_bytes(),
        max_img_bytes: p.effective_max_img_bytes(),
        max_tiles: p.map_file.max_tiles_per_mapset,
        supports_dem: p.rendering.supports_dem,
        connected,
    }
}

#[tauri::command]
pub fn list_devices() -> IpcResult<Vec<DeviceSummary>> {
    let profiles =
        profiles()?;
    let detected = devices::detect();
    Ok(profiles
        .iter()
        .map(|p| {
            let connected = detected.iter().any(|d| {
                d.model
                    .as_deref()
                    .and_then(|m| {
                        devices::match_profile(&profiles, m, p.is_wrist()).map(|x| x.id == p.id)
                    })
                    .unwrap_or(false)
            });
            summarise(p, connected)
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct ConnectedDevice {
    pub mount: String,
    pub model: Option<String>,
    /// Profile chosen for this device, if one matched.
    pub profile_id: Option<String>,
    #[ts(type = "number | null")]
    pub free_bytes: Option<u64>,
    pub existing_maps: Vec<String>,
}

/// A Garmin device on the USB bus that is not mounted as a filesystem.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct UsbDeviceInfo {
    pub model: String,
    pub serial: Option<String>,
    /// Profile matched by model, so the right size budget can still be offered.
    pub profile_id: Option<String>,
    /// True when the same model is also mounted, in which case there is nothing to fix.
    pub mounted: bool,
}

/// Garmin devices attached over USB, mounted or not (FR-DEV5).
///
/// Recent Edge and fēnix models default to MTP, and macOS has no MTP filesystem at all,
/// so such a device never appears under `/Volumes`. Without this the app simply reported
/// no device while one was plainly plugged in, and there was nothing to act on.
#[tauri::command]
pub fn usb_devices() -> IpcResult<Vec<UsbDeviceInfo>> {
    let profiles =
        profiles()?;
    let mounted = devices::detect();
    Ok(devices::usb_devices()
        .into_iter()
        .map(|d| {
            let is_mounted = mounted.iter().any(|m| {
                m.model
                    .as_deref()
                    .map(|x| x.eq_ignore_ascii_case(&d.model))
                    .unwrap_or(false)
            });
            UsbDeviceInfo {
                profile_id: devices::match_profile(&profiles, &d.model, looks_wrist(&d.model))
                    .map(|p| p.id.clone()),
                mounted: is_mounted,
                model: d.model,
                serial: d.serial,
            }
        })
        .collect())
}

/// A watch reports itself as fenix/epix/Forerunner and friends; anything else is
/// handlebar-shaped as far as the cartography is concerned.
fn looks_wrist(model: &str) -> bool {
    let m = model.to_lowercase();
    [
        "fenix",
        "epix",
        "forerunner",
        "marq",
        "tactix",
        "instinct",
        "enduro",
    ]
    .iter()
    .any(|k| m.contains(k))
}

#[tauri::command]
pub fn detect_devices() -> IpcResult<Vec<ConnectedDevice>> {
    let profiles =
        profiles()?;
    Ok(devices::detect()
        .into_iter()
        .map(|d| {
            let looks_wrist = d.model.as_deref().map(looks_wrist).unwrap_or(false);
            ConnectedDevice {
                profile_id: d
                    .model
                    .as_deref()
                    .and_then(|m| devices::match_profile(&profiles, m, looks_wrist))
                    .map(|p| p.id.clone()),
                mount: d.mount.display().to_string(),
                model: d.model,
                free_bytes: d.free_bytes,
                existing_maps: d
                    .existing_maps
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect(),
            }
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct PresetInfo {
    pub id: String,
    pub needs_winter: bool,
    pub needs_cycle: bool,
    pub contour_m: i32,
    pub index_contour_m: i32,
    /// True when every dataset this preset needs is already downloaded.
    pub data_ready: bool,
    /// What is missing, for the disabled-with-a-reason UI (FR-51).
    pub missing: Vec<String>,
}

#[tauri::command]
pub fn list_presets() -> IpcResult<Vec<PresetInfo>> {
    let root = Cache::default_root();
    let winter_ready = std::fs::read_dir(root.join("winter"))
        .map(|rd| {
            rd.flatten()
                .any(|e| e.path().extension().map(|x| x == "gpkg").unwrap_or(false))
        })
        .unwrap_or(false);
    let cycle_ready =
        root.join("routes").is_dir() && walk_has_extension(&root.join("routes"), "shp");

    Ok(Preset::all()
        .iter()
        .map(|p| {
            let mut missing = Vec::new();
            if p.needs_winter() && !winter_ready {
                missing.push("winter route data".to_string());
            }
            if p.needs_cycle() && !cycle_ready {
                missing.push("cycle route data".to_string());
            }
            PresetInfo {
                id: p.id().to_string(),
                needs_winter: p.needs_winter(),
                needs_cycle: p.needs_cycle(),
                contour_m: p.default_contour_m(),
                index_contour_m: p.default_index_contour_m(),
                data_ready: missing.is_empty(),
                missing,
            }
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct LayerInfo {
    /// The source layer name, which is also what `Recipe.excludedLayers` holds.
    pub id: String,
    pub group: String,
    /// How many attributes travel into the map with this layer, as a rough weight.
    pub attribute_count: usize,
    /// Which presets extract this layer at all.
    pub presets: Vec<String>,
}

/// The toggleable layers, in panel order (SPEC.md FR-51).
///
/// Hiking trails are absent by design: they are an attribute of the road layer, so a
/// separate toggle would be a lie. The panel says so rather than offering one.
#[tauri::command]
pub fn list_layers() -> IpcResult<Vec<LayerInfo>> {
    let winter: Vec<String> = Preset::all()
        .iter()
        .filter(|p| p.needs_winter())
        .map(|p| p.id().to_string())
        .collect();
    let cycle: Vec<String> = Preset::all()
        .iter()
        .filter(|p| p.needs_cycle())
        .map(|p| p.id().to_string())
        .collect();
    let all: Vec<String> = Preset::all().iter().map(|p| p.id().to_string()).collect();

    let mut out: Vec<LayerInfo> = DEFAULT_LAYERS
        .iter()
        .map(|l| (l, all.clone()))
        .chain(WINTER_LAYERS.iter().map(|l| (l, winter.clone())))
        .chain(CYCLE_LAYERS.iter().map(|l| (l, cycle.clone())))
        .map(|(l, presets)| LayerInfo {
            id: l.layer.to_string(),
            group: l.group.id().to_string(),
            attribute_count: l.attributes.len(),
            presets,
        })
        .collect();

    // Panel order follows the group order, not the extraction order.
    let order = |g: &str| {
        LayerGroup::all()
            .iter()
            .position(|x| x.id() == g)
            .unwrap_or(usize::MAX)
    };
    out.sort_by_key(|l| order(&l.group));
    Ok(out)
}

/// One entry in the saved-recipe library (SPEC.md FR-55).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct SavedRecipeInfo {
    pub id: String,
    pub name: String,
    pub device_id: String,
    pub preset: String,
    pub area_label: String,
    pub area_km2: f64,
    pub saved_at: u64,
}

fn recipes_dir() -> PathBuf {
    Cache::default_root().join("recipes")
}

#[tauri::command]
pub fn list_recipes() -> IpcResult<Vec<SavedRecipeInfo>> {
    Ok(library::list(&recipes_dir())
        .into_iter()
        .map(|r| SavedRecipeInfo {
            id: r.id,
            name: r.name,
            device_id: r.device_id,
            preset: r.preset,
            area_label: r.area_label,
            area_km2: r.area_km2,
            saved_at: r.saved_at,
        })
        .collect())
}

/// Save under the recipe's own name, replacing an earlier recipe of that name.
#[tauri::command]
pub fn save_recipe(recipe: Recipe) -> IpcResult<String> {
    library::save(&recipes_dir(), &recipe).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_recipe(id: String) -> IpcResult<Recipe> {
    library::load(&recipes_dir(), &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_recipe(id: String) -> IpcResult<()> {
    library::delete(&recipes_dir(), &id).map_err(|e| e.to_string())
}

fn walk_has_extension(root: &Path, ext: &str) -> bool {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().map(|x| x == ext).unwrap_or(false) {
                return true;
            }
        }
    }
    false
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct PlaceMatch {
    pub name: String,
    pub alternatives: Vec<String>,
    pub population_category: Option<String>,
    pub easting: f64,
    pub northing: f64,
    pub lat: f64,
    pub lon: f64,
}

/// Resolve a place name to **every** match, most significant first.
///
/// Never collapsed to one answer: two settlements are named Grindelwald, and picking
/// the wrong one silently builds a map of the wrong valley (FR-33).
#[tauri::command]
pub async fn find_places(name: String) -> IpcResult<Vec<PlaceMatch>> {
    let cache = Cache::default_root();
    let path = pipeline::find_tlm3d(&cache)
        .ok_or_else(|| "swissTLM3D is not downloaded yet".to_string())?;
    let gpkg = s2g_core::gpkg::Gpkg::open(&path).map_err(|e| e.to_string())?;
    Ok(gpkg
        .find_places(&name)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|p| {
            let (lon, lat) = lv95_to_wgs84(p.easting, p.northing);
            PlaceMatch {
                name: p.name,
                alternatives: p.alternatives,
                population_category: p.population_category,
                easting: p.easting,
                northing: p.northing,
                lat,
                lon,
            }
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct AreaInfo {
    pub area_km2: f64,
    /// [west, south, east, north] in WGS84, for the map view.
    pub wgs84: [f64; 4],
    pub within_switzerland: bool,
    /// Predicted output size from the calibrated model (FR-60).
    #[ts(type = "number")]
    pub estimated_bytes: u64,
    /// The device budget the estimate is measured against, and the hard ceiling
    /// above it. Shown together with the estimate (FR-63).
    #[ts(type = "number")]
    pub budget_bytes: u64,
    #[ts(type = "number")]
    pub hard_limit_bytes: u64,
    pub over_budget: bool,
    /// True once the model has been refit from the user's own builds.
    pub calibrated: bool,
    /// How many real builds the model has seen.
    pub model_samples: usize,
    /// False when swissTLM3D is absent, so feature counts could not be read and the
    /// estimate is area-only. The UI says so rather than implying precision.
    pub counted_features: bool,
}

/// LV95 bounding box for a WGS84 rectangle drawn on the map.
///
/// The projection lives only in Rust: duplicating it in TypeScript would mean two
/// implementations to keep in agreement, and the measured accuracy envelope is
/// documented against this one (FR-P1).
#[tauri::command]
pub fn wgs84_bbox_to_lv95(west: f64, south: f64, east: f64, north: f64) -> IpcResult<[f64; 4]> {
    use s2g_core::proj::wgs84_to_lv95;
    // All four corners: the LV95 grid is rotated relative to the graticule, so a
    // two-corner conversion would clip content along the edges.
    let corners = [
        wgs84_to_lv95(west, south),
        wgs84_to_lv95(west, north),
        wgs84_to_lv95(east, south),
        wgs84_to_lv95(east, north),
    ];
    let es: Vec<f64> = corners.iter().map(|c| c.0).collect();
    let ns: Vec<f64> = corners.iter().map(|c| c.1).collect();
    Ok([
        es.iter().cloned().fold(f64::INFINITY, f64::min),
        ns.iter().cloned().fold(f64::INFINITY, f64::min),
        es.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        ns.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    ])
}

/// Geometry facts plus a calibrated size estimate for a candidate area.
///
/// The content parameters are optional so the area step can ask before content has
/// been chosen; they default to the hiking preset's settings, which is what the
/// wizard starts with.
/// The area and content parameters an estimate depends on, as one value: a command
/// with eight positional arguments is easy to call wrongly from the frontend.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct AreaQuery {
    pub min_e: f64,
    pub min_n: f64,
    pub max_e: f64,
    pub max_n: f64,
    pub device_id: String,
    /// Absent until the content step has been visited.
    pub preset: Option<String>,
    pub contour_m: Option<i32>,
    pub relief: Option<String>,
}

/// Project a polyline to WGS84 for display.
///
/// The frontend needs the track in map coordinates, and the projection lives only in
/// Rust so there is one implementation whose accuracy envelope is the documented one
/// (FR-P1).
#[tauri::command]
pub fn lv95_line_to_wgs84(points: Vec<[f64; 2]>) -> IpcResult<Vec<[f64; 2]>> {
    Ok(points
        .iter()
        .map(|p| {
            let (lon, lat) = lv95_to_wgs84(p[0], p[1]);
            [lon, lat]
        })
        .collect())
}

/// One selectable administrative unit (SPEC.md FR-33).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct AdminUnitInfo {
    /// `kantonsnummer`, `bezirksnummer` or `bfs_nummer` — stable, unlike the name.
    ///
    /// Typed as `number`, not `bigint`: ts-rs maps i64 to bigint, but these arrive as
    /// ordinary JSON numbers and no unit number comes close to 2^53.
    #[ts(type = "number")]
    pub number: i64,
    pub name: String,
    /// Canton, shown because commune names are not unique.
    pub canton: String,
    #[ts(type = "number")]
    pub population: i64,
    pub area_km2: f64,
}

/// Every unit at one level, sorted by name.
///
/// The whole level at once — 26 cantons, 135 districts, 2,123 communes — because the
/// picker filters as the user types and a round trip per keystroke would be worse.
#[tauri::command]
pub async fn list_admin_units(level: String) -> IpcResult<Vec<AdminUnitInfo>> {
    let level = match level.as_str() {
        "commune" => AdminLevel::Commune,
        "district" => AdminLevel::District,
        "canton" => AdminLevel::Canton,
        other => return Err(format!("unknown administrative level {other:?}")),
    };
    let root = Cache::default_root();
    let units = tokio::task::spawn_blocking(move || boundaries::list_units(&root, level))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    Ok(units
        .into_iter()
        .map(|u| AdminUnitInfo {
            number: u.number,
            name: u.name,
            canton: u.canton,
            population: u.population,
            area_km2: u.area_km2,
        })
        .collect())
}

/// Outlines of the chosen units in WGS84, for the map (FR-33).
///
/// Simplified before projecting: a canton is 14,000 points, which is far more than a
/// screen can show and enough to make the map stutter while dragging. 100 m is invisible
/// at any zoom the picker is used at.
#[tauri::command]
pub async fn admin_outline(level: String, numbers: Vec<i64>) -> IpcResult<Vec<Vec<[f64; 2]>>> {
    let level = match level.as_str() {
        "commune" => AdminLevel::Commune,
        "district" => AdminLevel::District,
        "canton" => AdminLevel::Canton,
        other => return Err(format!("unknown administrative level {other:?}")),
    };
    if numbers.is_empty() {
        return Ok(Vec::new());
    }
    let root = Cache::default_root();
    let polys =
        tokio::task::spawn_blocking(move || boundaries::load_geometry(&root, level, &numbers))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

    Ok(polys
        .iter()
        .flatten()
        .map(|ring| {
            s2g_core::geom::simplify(ring, 100.0)
                .iter()
                .map(|c| {
                    let (lon, lat) = lv95_to_wgs84(c.e, c.n);
                    [lon, lat]
                })
                .collect()
        })
        .collect())
}

/// The LV95 extent of a set of units, grown by the buffer (FR-34).
///
/// Resolved once when the units are chosen and stored in the recipe, so the area
/// readout and size estimate need no file access afterwards.
#[tauri::command]
pub async fn admin_extent(
    level: String,
    numbers: Vec<i64>,
    buffer_km: f64,
) -> IpcResult<[f64; 4]> {
    let level = match level.as_str() {
        "commune" => AdminLevel::Commune,
        "district" => AdminLevel::District,
        "canton" => AdminLevel::Canton,
        other => return Err(format!("unknown administrative level {other:?}")),
    };
    if numbers.is_empty() {
        return Err("no administrative units selected".into());
    }
    let root = Cache::default_root();
    let b = tokio::task::spawn_blocking(move || boundaries::extent(&root, level, &numbers))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let m = buffer_km * 1000.0;
    Ok([b.min_e - m, b.min_n - m, b.max_e + m, b.max_n + m])
}

/// An imported GPX or FIT track, ready to become a corridor (SPEC.md FR-38..FR-40).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct TrackImport {
    pub name: String,
    /// LV95 `[easting, northing]` pairs, simplified.
    pub points: Vec<[f64; 2]>,
    pub length_km: f64,
    pub ascent_m: f64,
    /// Points before simplification, so the UI can say what was dropped.
    pub original_points: usize,
    /// False when part of the track falls outside the swisstopo coverage area.
    pub fully_within_switzerland: bool,
}

/// Import a track from file bytes rather than a path.
///
/// The frontend reads the file itself and sends the bytes, so no native file dialog
/// and no filesystem permission are needed for what is a read-only import.
#[tauri::command]
pub fn import_track(name: String, bytes: Vec<u8>) -> IpcResult<TrackImport> {
    use s2g_core::geom::Coord;
    use s2g_core::proj::wgs84_to_lv95;

    /// One imported position: longitude, latitude, and elevation where the file has one.
    type Wgs84Point = (f64, f64, Option<f64>);

    let lower = name.to_lowercase();
    let (title, wgs84): (Option<String>, Vec<Wgs84Point>) = if lower.ends_with(".fit") {
        let course = s2g_core::fit::parse(&bytes).map_err(|e| e.to_string())?;
        (
            course.name,
            course
                .points
                .iter()
                .map(|p| (p.lon, p.lat, p.ele_m))
                .collect(),
        )
    } else {
        let text = String::from_utf8_lossy(&bytes);
        let gpx = s2g_core::gpx::parse(&text);
        // Segments stay separate in the parse, but a corridor is one buffered path;
        // joining them here only affects the buffer, never rendered geometry.
        let title = gpx.tracks.iter().find_map(|t| t.name.clone());
        let points: Vec<(f64, f64, Option<f64>)> = gpx
            .tracks
            .iter()
            .flat_map(|t| t.points.iter())
            .map(|p| (p.lon, p.lat, p.ele_m))
            .collect();
        (title, points)
    };

    if wgs84.is_empty() {
        return Err(format!("{name} contains no track positions"));
    }

    // Ascent from the source elevations, before any simplification drops points.
    let mut ascent = 0.0;
    let mut last: Option<f64> = None;
    for (_, _, e) in &wgs84 {
        if let Some(e) = e {
            if let Some(prev) = last {
                if *e > prev {
                    ascent += e - prev;
                }
            }
            last = Some(*e);
        }
    }

    let projected: Vec<Coord> = wgs84
        .iter()
        .map(|(lon, lat, _)| {
            let (e, n) = wgs84_to_lv95(*lon, *lat);
            Coord::new(e, n)
        })
        .collect();
    let length_m: f64 = projected.windows(2).map(|w| w[0].distance(&w[1])).sum();

    // 50 m tolerance: a corridor is kilometres wide, so finer detail in its centreline
    // changes nothing and would bloat every saved recipe.
    let simplified = s2g_core::geom::simplify(&projected, 50.0);
    let ch = BBox::new(
        s2g_core::proj::LV95_BOUNDS.0,
        s2g_core::proj::LV95_BOUNDS.1,
        s2g_core::proj::LV95_BOUNDS.2,
        s2g_core::proj::LV95_BOUNDS.3,
    );
    let fully_within = projected
        .iter()
        .all(|c| c.e >= ch.min_e && c.e <= ch.max_e && c.n >= ch.min_n && c.n <= ch.max_n);

    Ok(TrackImport {
        name: title.unwrap_or_else(|| {
            // Fall back to the file name without its extension.
            name.rsplit('/')
                .next()
                .unwrap_or(&name)
                .rsplit_once('.')
                .map(|(stem, _)| stem.to_string())
                .unwrap_or_else(|| name.clone())
        }),
        points: simplified.iter().map(|c| [c.e, c.n]).collect(),
        length_km: length_m / 1000.0,
        ascent_m: ascent,
        original_points: projected.len(),
        fully_within_switzerland: fully_within,
    })
}

/// Where data lives, how much room is there, and how much is already used.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct DataLocation {
    pub path: String,
    /// True when `S2G_CACHE` is set, in which case the setting is overridden and the
    /// UI must say so rather than appearing not to work.
    pub from_environment: bool,
    /// True when nothing is configured and the platform default is in use.
    pub is_default: bool,
    #[ts(type = "number | null")]
    pub free_bytes: Option<u64>,
    #[ts(type = "number")]
    pub used_bytes: u64,
    /// Where that space went. The elevation tile cache is the part that grows without
    /// bound, and it used to be invisible here.
    #[ts(type = "number")]
    pub dataset_bytes: u64,
    #[ts(type = "number")]
    pub elevation_bytes: u64,
    #[ts(type = "number")]
    pub build_bytes: u64,
    #[ts(type = "number")]
    pub other_bytes: u64,
    pub exists: bool,
}

async fn describe_location(path: PathBuf) -> DataLocation {
    // usage() walks the tree rather than listing datasets, because the largest
    // directory — cached elevation tiles — is loose files that a dataset listing
    // cannot see.
    let dir = path.clone();
    let usage = tokio::task::spawn_blocking(move || Cache::new(dir).usage())
        .await
        .unwrap_or_default();
    DataLocation {
        from_environment: std::env::var("S2G_CACHE").is_ok(),
        is_default: path == s2g_core::settings::Settings::default_data_root(),
        free_bytes: available_bytes(&path),
        used_bytes: usage.total(),
        dataset_bytes: usage.datasets,
        elevation_bytes: usage.elevation,
        build_bytes: usage.builds,
        other_bytes: usage.recipes + usage.quarantine + usage.other,
        exists: path.is_dir(),
        path: path.display().to_string(),
    }
}

/// Delete the cached elevation tiles, which are re-downloadable (FR-C4).
#[tauri::command]
pub async fn clear_elevation_cache() -> IpcResult<DataLocation> {
    let root = Cache::default_root();
    let c = Cache::new(root.clone());
    tokio::task::spawn_blocking(move || c.clear_elevation())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    Ok(describe_location(root).await)
}

/// Delete build intermediates, which are re-derivable (FR-C4).
#[tauri::command]
pub async fn clear_build_files() -> IpcResult<DataLocation> {
    let root = Cache::default_root();
    let c = Cache::new(root.clone());
    tokio::task::spawn_blocking(move || c.clear_builds())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    Ok(describe_location(root).await)
}

#[tauri::command]
pub async fn data_location() -> IpcResult<DataLocation> {
    Ok(describe_location(Cache::default_root()).await)
}

/// Report on a candidate directory without committing to it (FR-C2).
#[tauri::command]
pub async fn inspect_data_location(path: String) -> IpcResult<DataLocation> {
    Ok(describe_location(PathBuf::from(path)).await)
}

/// Point the app at a different data directory.
///
/// Nothing is moved: the datasets already downloaded stay where they are, and the UI
/// says so. Copying gigabytes between volumes is the file manager's job, and doing it
/// silently behind a settings change would be worse than saying nothing happened.
#[tauri::command]
pub async fn set_data_location(path: Option<String>) -> IpcResult<DataLocation> {
    let mut settings = s2g_core::settings::Settings::load();
    match path {
        Some(p) => {
            let dir = PathBuf::from(p);
            s2g_core::settings::check_writable(&dir)?;
            settings.data_root = Some(dir);
        }
        None => settings.data_root = None,
    }
    settings.save().map_err(|e| e.to_string())?;
    Ok(describe_location(Cache::default_root()).await)
}

/// The full swissTLM3D coverage extent in LV95 (SPEC.md FR-35).
///
/// Read from the one place that defines it, so the "whole Switzerland" action cannot
/// drift away from the coverage check that validates it.
#[tauri::command]
pub fn coverage_bbox() -> IpcResult<[f64; 4]> {
    let b = s2g_core::proj::LV95_BOUNDS;
    Ok([b.0, b.1, b.2, b.3])
}

#[tauri::command]
pub fn describe_area(query: AreaQuery) -> IpcResult<AreaInfo> {
    let AreaQuery {
        min_e,
        min_n,
        max_e,
        max_n,
        device_id,
        preset,
        contour_m,
        relief,
    } = query;
    let bbox = BBox::new(min_e, min_n, max_e, max_n);
    let profiles =
        profiles()?;
    let profile = profiles.iter().find(|p| p.id == device_id);
    let budget = profile
        .map(|p| p.effective_budget_bytes())
        .unwrap_or(u64::MAX);
    let hard_limit = profile
        .map(|p| p.effective_max_img_bytes())
        .unwrap_or(u64::MAX);

    let preset = preset
        .and_then(|p| Preset::all().iter().find(|x| x.id() == p).copied())
        .unwrap_or(Preset::Hiking);
    let relief = match relief.as_deref() {
        Some("detailed") => ReliefDetail::Detailed,
        Some("off") => ReliefDetail::Off,
        _ => ReliefDetail::Gentle,
    };

    // A throwaway recipe, so the estimator counts exactly the layers a build would.
    let mut probe = Recipe::new(
        "estimate",
        &device_id,
        s2g_core::recipe::AreaSelection::BBox {
            min_e,
            min_n,
            max_e,
            max_n,
        },
    )
    .with_preset(preset);
    probe.relief = relief;
    if let Some(m) = contour_m {
        probe.contours.interval_m = m;
    }

    let cache_root = Cache::default_root();
    let (group_counts, counted) = match pipeline::find_tlm3d(&cache_root)
        .and_then(|p| s2g_core::gpkg::Gpkg::open(p).ok())
    {
        Some(gpkg) => (
            estimate::count_groups(&gpkg, &probe, Some(&cache_root)),
            true,
        ),
        None => (Default::default(), false),
    };

    let model = estimate::current_model(
        &resource_root().join("estimator").join("size-model.json"),
        &calibration_log_path(),
    );
    let predictors = estimate::Predictors {
        group_counts,
        area_km2: bbox.area_km2(),
        contour_interval_m: probe.contours.interval_m,
        relief,
        slope_classes: probe.slope_classes,
    };
    let estimated = model.predict(&predictors);

    Ok(AreaInfo {
        area_km2: bbox.area_km2(),
        wgs84: bbox.to_wgs84(),
        within_switzerland: bbox.within_switzerland(),
        estimated_bytes: estimated,
        budget_bytes: budget,
        hard_limit_bytes: hard_limit,
        over_budget: estimated > budget,
        calibrated: model.samples > 0,
        model_samples: model.samples,
        counted_features: counted,
    })
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct BuildProgress {
    pub task_id: String,
    pub stage: String,
    pub stage_label: String,
    pub stage_index: usize,
    pub stage_count: usize,
    /// Progress within the current stage, when the stage can report it.
    pub fraction: Option<f64>,
    pub detail: String,
    /// Progress through the whole build, weighted by how long each stage usually takes.
    pub overall: f64,
    pub elapsed_seconds: f64,
    /// Seconds remaining, absent until the extrapolation would mean something (FR-71).
    pub eta_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct BuildFinished {
    pub task_id: String,
    pub gmapsupp: String,
    #[ts(type = "number")]
    pub bytes: u64,
    pub tile_count: usize,
    #[ts(type = "number")]
    pub features: u64,
    pub contour_lines: usize,
    pub has_dem: bool,
    pub warnings: Vec<String>,
    /// The manifest written beside the map (FR-71), when it could be written.
    pub manifest: Option<String>,
    /// How long the build actually took. Shown on completion, and what makes the next
    /// build's estimate credible.
    pub seconds: f64,
}

/// Start a build. Progress arrives as `build:progress`, then `build:done` or
/// `build:error`.
#[tauri::command]
pub async fn start_build(
    app: AppHandle,
    state: State<'_, AppState>,
    recipe: Recipe,
) -> IpcResult<String> {
    let root = resource_root();
    let profiles = profiles()?;
    let profile = profiles
        .iter()
        .find(|p| p.id == recipe.device_id)
        .cloned()
        .ok_or_else(|| format!("unknown device profile {:?}", recipe.device_id))?;
    let toolchain = s2g_core::garmin::Toolchain::discover(&root).map_err(|e| e.to_string())?;

    let task_id = format!("build:{}", recipe.cache_key());
    let cancel = Cancel::new();
    state
        .tasks
        .lock()
        .expect("task registry poisoned")
        .insert(task_id.clone(), cancel.clone());

    let id = task_id.clone();
    tauri::async_runtime::spawn(async move {
        let http = match ReqwestHttp::new() {
            Ok(h) => h,
            Err(e) => {
                let _ = app.emit(
                    "build:error",
                    TaskError {
                        task_id: id,
                        message: e.to_string(),
                    },
                );
                return;
            }
        };
        let ctx = BuildContext {
            toolchain,
            style_root: root.join("style"),
            typ_root: root.join("typ"),
            cache_root: Cache::default_root(),
            work_dir: Cache::default_root()
                .join("builds")
                .join(safe_name(&recipe.name)),
            http: &http,
            calibration_log: Some(calibration_log_path()),
        };

        let emit = app.clone();
        let pid = id.clone();
        let mut last = std::time::Instant::now() - PROGRESS_INTERVAL;
        // Stage weights come from this user's own builds once there are any, because
        // the split between stages moves enormously with a warm or cold elevation
        // cache. Until then the shipped weights apply.
        let weights = estimate::stage_weights(&estimate::CalibrationLog::read(
            &calibration_log_path(),
        ));
        let build_started = std::time::Instant::now();
        let result = pipeline::build(&ctx, &recipe, &profile, &cancel, move |u| {
            // Stage changes are always reported; progress within a stage is throttled.
            if last.elapsed() < PROGRESS_INTERVAL && u.fraction.is_some() {
                return;
            }
            last = std::time::Instant::now();
            let index = Stage::all().iter().position(|s| *s == u.stage).unwrap_or(0);
            let elapsed = build_started.elapsed().as_secs_f64();
            let overall = estimate::build_fraction(&weights, index, u.fraction.unwrap_or(0.0));
            let _ = emit.emit(
                "build:progress",
                BuildProgress {
                    overall,
                    elapsed_seconds: elapsed,
                    eta_seconds: estimate::eta_seconds(elapsed, overall),
                    task_id: pid.clone(),
                    stage: format!("{:?}", u.stage).to_lowercase(),
                    stage_label: u.stage.label().to_string(),
                    stage_index: index,
                    stage_count: Stage::all().len(),
                    fraction: u.fraction,
                    detail: u.detail,
                },
            );
        })
        .await;

        match result {
            Ok(report) => {
                let _ = app.emit(
                    "build:done",
                    BuildFinished {
                        task_id: id,
                        gmapsupp: report.gmapsupp.display().to_string(),
                        bytes: report.bytes,
                        tile_count: report.tile_count,
                        features: report.features,
                        contour_lines: report.contour_lines,
                        has_dem: report.has_dem,
                        manifest: report.manifest.map(|p| p.display().to_string()),
                        seconds: report.stage_seconds.iter().map(|(_, v)| *v).sum(),
                        warnings: report.warnings,
                    },
                );
            }
            Err(e) => {
                eprintln!("build failed: {e}");
                let d = s2g_core::diagnose::diagnose(&e);
                let _ = app.emit(
                    "build:failed",
                    BuildFailure {
                        task_id: id.clone(),
                        summary: d.summary,
                        suggestion: d.suggestion,
                        recognised: d.recognised,
                        detail: e.to_string(),
                    },
                );
                let _ = app.emit(
                    "build:error",
                    TaskError {
                        task_id: id,
                        message: e.to_string(),
                    },
                );
            }
        }
    });

    Ok(task_id)
}

fn safe_name(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = cleaned.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "build".into()
    } else {
        trimmed
    }
}

// Deserialize as well as Serialize: the UI receives a plan, shows it, and passes the
// same value back to install_map, so the two sides cannot disagree about the target.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct InstallPlan {
    pub source: String,
    pub target: String,
    /// True when a file would be replaced.
    pub overwrites: bool,
    #[ts(type = "number")]
    pub bytes: u64,
    #[ts(type = "number | null")]
    pub free_bytes: Option<u64>,
    pub fits: bool,
}

/// Describe what installing would do, without doing it (FR-80, FR-81).
#[tauri::command]
pub fn plan_install(
    gmapsupp: String,
    mount: String,
    device_id: String,
    map_name: String,
) -> IpcResult<InstallPlan> {
    let profiles =
        profiles()?;
    let profile = profiles
        .iter()
        .find(|p| p.id == device_id)
        .ok_or_else(|| format!("unknown device profile {device_id:?}"))?;

    let src = PathBuf::from(&gmapsupp);
    let bytes = std::fs::metadata(&src)
        .map(|m| m.len())
        .map_err(|e| e.to_string())?;
    let filename = profile.output_filename(&map_name);
    if devices::is_double_extension(&filename) {
        return Err(format!("refusing to write {filename}: double extension"));
    }
    let dir = PathBuf::from(&mount).join("Garmin");
    let target = dir.join(&filename);
    let free = s2g_core::cache::available_bytes(&dir);
    Ok(InstallPlan {
        source: gmapsupp,
        target: target.display().to_string(),
        overwrites: target.exists(),
        bytes,
        free_bytes: free,
        fits: free.map(|f| f > bytes).unwrap_or(true),
    })
}

/// Where a map goes on a given device, in words (SPEC.md FR-83).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct InstallInstructions {
    /// Folders this profile records as valid, in preference order.
    pub folders: Vec<String>,
    pub filename: String,
    /// True when the device can hold several map sets, so the file name matters.
    pub multiple_maps: bool,
}

/// Textual install instructions for a device profile.
///
/// Needed whenever the app cannot write the file itself: a device in MTP mode, an SD
/// card in a reader, or a user who would simply rather copy it by hand.
#[tauri::command]
pub fn install_instructions(device_id: String, map_name: String) -> IpcResult<InstallInstructions> {
    let profiles =
        profiles()?;
    let profile = profiles
        .iter()
        .find(|p| p.id == device_id)
        .ok_or_else(|| format!("unknown device profile {device_id:?}"))?;
    // Whether an SD card is an option is a device fact no profile records, so it is
    // not claimed here (working agreement rule 3).
    Ok(InstallInstructions {
        folders: profile.map_file.install_paths.clone(),
        filename: profile.output_filename(&map_name),
        multiple_maps: profile.map_file.supports_multiple_mapsets,
    })
}

/// Copy the built map to a folder the user chose (SPEC.md FR-83).
///
/// The escape hatch for every case the direct install cannot serve, MTP devices above
/// all. Returns the path written.
#[tauri::command]
pub async fn export_map(
    gmapsupp: String,
    dir: String,
    device_id: String,
    map_name: String,
) -> IpcResult<String> {
    let profiles =
        profiles()?;
    let profile = profiles
        .iter()
        .find(|p| p.id == device_id)
        .ok_or_else(|| format!("unknown device profile {device_id:?}"))?;

    let filename = profile.output_filename(&map_name);
    if devices::is_double_extension(&filename) {
        return Err(format!("refusing to write {filename}: double extension"));
    }
    let dst = PathBuf::from(&dir).join(&filename);
    let src = PathBuf::from(&gmapsupp);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // Same write-then-rename as the device install: an interrupted copy must not leave
    // a truncated file that looks finished.
    let tmp = dst.with_extension("img.part");
    tokio::task::spawn_blocking({
        let (src, tmp, dst) = (src.clone(), tmp.clone(), dst.clone());
        move || -> std::io::Result<()> {
            std::fs::copy(&src, &tmp)?;
            std::fs::rename(&tmp, &dst)
        }
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    Ok(dst.display().to_string())
}

/// Copy the map to the device, optionally backing up what is replaced.
#[tauri::command]
pub async fn install_map(plan: InstallPlan, backup: bool) -> IpcResult<String> {
    let src = PathBuf::from(&plan.source);
    let dst = PathBuf::from(&plan.target);
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if dst.exists() && backup {
        let bak = dst.with_extension("img.bak");
        std::fs::rename(&dst, &bak).map_err(|e| e.to_string())?;
    }
    // Write beside the target then rename, so an interrupted copy cannot leave a
    // truncated map the device would try to load.
    let tmp = dst.with_extension("img.part");
    std::fs::copy(&src, &tmp).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &dst).map_err(|e| e.to_string())?;

    // Re-hash from the device, not just compare sizes (FR-82). A truncated copy has the
    // wrong size, but a copy corrupted mid-transfer -- which is what a flaky USB link
    // actually produces -- has the right one. These maps are a few megabytes, so the
    // read costs about a second.
    let (src_hash, dst_hash) = tokio::task::spawn_blocking({
        let (src, dst) = (src.clone(), dst.clone());
        move || -> std::io::Result<(String, String)> {
            Ok((
                s2g_core::cache::sha256_of(&src)?,
                s2g_core::cache::sha256_of(&dst)?,
            ))
        }
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    if src_hash != dst_hash {
        // Leave nothing half-installed: the device would try to load it.
        let _ = std::fs::remove_file(&dst);
        return Err(format!(
            "the copy on the device does not match what was built \
             (built {}, on the device {}); it has been removed",
            &src_hash[..16],
            &dst_hash[..16]
        ));
    }
    Ok(dst.display().to_string())
}
