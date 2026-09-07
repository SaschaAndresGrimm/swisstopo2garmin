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
    /// True when the STAC API could not be reached and this is the last answer it
    /// gave (SPEC.md §12). The Data screen must say so rather than presenting old
    /// release information as current.
    pub stale: bool,
    /// When the release information was actually fetched, so "stale" can be dated.
    pub catalog_fetched_at: Option<String>,
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

/// Wire size and inflated size of a release's primary asset.
///
/// Shared by the Data screen's release line and by the download's space precheck, so
/// the number the user is shown is the number the precheck uses.
async fn probe_sizes(
    http: &ReqwestHttp,
    asset: &s2g_core::stac::Asset,
    kind: s2g_core::stac::AssetKind,
) -> (Option<u64>, Option<u64>, Option<String>) {
    use s2g_core::http::Http;
    let total = http.head(&asset.href).await.ok().and_then(|h| h.len);
    match (total, kind.is_archive()) {
        // A bare GeoPackage is its own member: nothing to probe, and probing it as a
        // zip is what made the Data screen show an error for it.
        (Some(total), false) => (Some(total), Some(total), Some(asset.name.clone())),
        (Some(total), true) => match s2g_core::zip::first_member(http, &asset.href, total).await {
            Ok(m) => (Some(total), Some(m.uncompressed_size), Some(m.name)),
            // A multi-member archive still reports its download size honestly.
            Err(_) => (Some(total), None, None),
        },
        (None, _) => (None, None, None),
    }
}

#[tauri::command]
pub async fn latest_release(collection: String) -> IpcResult<ReleaseInfo> {
    let http = ReqwestHttp::new().map_err(|e| e.to_string())?;
    let stac = Stac::new(&http);
    // Falls back to the last answer the API gave when it cannot be reached, so the Data
    // screen stays usable offline instead of saying nothing about data already on disk.
    let cached_item = stac
        .latest_cached(
            &collection,
            &cache().catalog_dir(),
            &s2g_core::clock::now_rfc3339(),
        )
        .await
        .map_err(|e| e.to_string())?;
    let (stale, catalog_fetched_at) = (cached_item.stale, cached_item.fetched_at);
    let item = cached_item.item;
    // Clone so `item` is free to be consumed below.
    let (asset, kind) = item.primary_asset().map_err(|e| e.to_string())?;
    let (asset, kind) = (asset.clone(), kind);

    // Probe the archive so the UI can state both the download size and the far larger
    // on-disk size before the user commits (SPEC.md FR-C1).
    let (archive_bytes, member_bytes, member_name) = probe_sizes(&http, &asset, kind).await;

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
        stale,
        catalog_fetched_at: Some(catalog_fetched_at),
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

    // SPEC.md §12 asks for a precheck "before download and before build". The build had
    // one; this did not, and swissTLM3D is the case that matters: 4.5 GB down the wire
    // becomes 10.0 GB on disk, so the wire size is not the number to check. A download
    // that fills the disk takes the rest of the machine down with it.
    let (archive_bytes, member_bytes, _) = probe_sizes(&http, &asset, kind).await;
    let need = s2g_core::cache::download_need(archive_bytes, member_bytes, stream_inflate);
    if let Some(need) = need {
        s2g_core::cache::precheck_space(&dir, need).map_err(|e| e.to_string())?;
    }

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
                    fetched_at: s2g_core::clock::now_rfc3339(),
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

// ---------------------------------------------------------------------------
// Devices, presets, area and build (SPEC.md §6.3-6.7)
// ---------------------------------------------------------------------------

use s2g_core::boundaries::{self, AdminLevel};
use s2g_core::devices::{self, DeviceProfile};
use s2g_core::estimate::{self, calibration_log_path};
use s2g_core::extract::{LayerGroup, CYCLE_LAYERS, DEFAULT_LAYERS, WINTER_LAYERS};
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
pub fn set_device_override(device_id: String, value: Option<DeviceOverrideInfo>) -> IpcResult<()> {
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
    Ok(devices::load_profiles(&resource_root().join("devices"))
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|p| match overrides.get(&p.id) {
            Some(o) => p.with_override(o),
            None => p,
        })
        .collect())
}

fn resource_root() -> PathBuf {
    if let Ok(p) = std::env::var("S2G_ROOT") {
        return PathBuf::from(p);
    }
    // Walk up from the executable and from the working directory, looking for the marker
    // files a build needs, and at each level also look in the places a packaged app puts
    // its resources. A development checkout has them at the repo root; a macOS bundle has
    // them in `Contents/Resources` beside `Contents/MacOS/<exe>`; the Linux and Windows
    // bundles put them in a `resources` directory next to the executable.
    //
    // Without the bundle cases this returned "." in a packaged app and every build failed
    // for want of a style directory -- on the developer's machine it worked, because the
    // walk found the repo.
    // The inner collects are load-bearing: `ancestors()` borrows the path it walks, and
    // the path is owned by the closure.
    let bases = std::env::current_exe()
        .ok()
        .into_iter()
        .flat_map(|exe| exe.ancestors().map(Path::to_path_buf).collect::<Vec<_>>())
        .chain(
            std::env::current_dir()
                .ok()
                .into_iter()
                .flat_map(|d| d.ancestors().map(Path::to_path_buf).collect::<Vec<_>>()),
        );

    resolve_resource_root(bases).unwrap_or_else(|| PathBuf::from("."))
}

/// The search itself, separated from where the candidates come from.
///
/// `current_exe` cannot be faked in a test, and this is the logic that was wrong: it is
/// worth being able to hand it a macOS bundle's ancestor list and see what it picks.
fn resolve_resource_root(bases: impl Iterator<Item = PathBuf>) -> Option<PathBuf> {
    for base in bases {
        for candidate in [base.clone(), base.join("Resources"), base.join("resources")] {
            if has_resources(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

/// Whether a directory holds the files a build cannot run without.
///
/// `devices` and `style` together, because either alone occurs by accident: `style` is a
/// common directory name, and a stray `devices` could be anything.
fn has_resources(dir: &Path) -> bool {
    dir.join("devices").is_dir() && dir.join("style").is_dir()
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
    let profiles = profiles()?;
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
    let profiles = profiles()?;
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
    let profiles = profiles()?;
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
    // Through datasets:: like the pipeline and the estimator. This used to probe
    // `winter/` and `routes/` directly -- the flat directories the Milestone 5 Python
    // spikes wrote -- so once the spike copies were deleted, every preset reported its
    // data missing while the app's own downloads sat in the content-addressed layout,
    // unseen. That is finding 5.2 for the third time; there is now a test asserting no
    // caller probes those paths itself.
    let winter_ready = !s2g_core::datasets::winter_geopackages(&root).is_empty();
    let cycle_ready = !s2g_core::datasets::route_shapefiles(&root).is_empty();

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
    /// How much has to go, and which changes to this recipe would help (SPEC.md §12).
    /// Only remedies that would actually change *this* recipe are listed.
    #[ts(type = "number")]
    pub overshoot_bytes: u64,
    /// `smallerArea` | `coarserContours` | `fewerLayers` | `noRelief` |
    /// `noSlopeClasses` | `splitIntoMapSets`, most effective first.
    pub remedies: Vec<String>,
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
/// How an oversized area would be divided (SPEC.md FR-36).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct PartitionPlan {
    pub parts: usize,
    pub columns: usize,
    pub rows: usize,
    /// Why the split is needed, in words. Empty when none is.
    pub reason: String,
    /// False when even the largest allowed split leaves parts over budget.
    pub fits: bool,
    /// Name and estimated size of each part, in build order.
    pub part_names: Vec<String>,
    #[ts(type = "number[]")]
    pub part_bytes: Vec<u64>,
}

/// Plan the split for a recipe, without building anything.
#[tauri::command]
pub async fn partition_plan(recipe: Recipe) -> IpcResult<PartitionPlan> {
    let profiles = profiles()?;
    let profile = profiles
        .iter()
        .find(|p| p.id == recipe.device_id)
        .ok_or_else(|| format!("unknown device profile {:?}", recipe.device_id))?;

    let cache_root = Cache::default_root();
    let bbox = recipe.area.bbox();
    let group_counts =
        match pipeline::find_tlm3d(&cache_root).and_then(|p| s2g_core::gpkg::Gpkg::open(p).ok()) {
            Some(gpkg) => estimate::count_groups(&gpkg, &recipe, Some(&cache_root)),
            None => Default::default(),
        };
    let predictors = estimate::Predictors {
        group_counts,
        area_km2: bbox.area_km2(),
        contour_interval_m: recipe.contours.interval_m,
        relief: recipe.relief,
        slope_classes: recipe.slope_classes,
        wrist: profile.is_wrist(),
    };
    let model = estimate::current_model(
        &resource_root().join("estimator").join("size-model.json"),
        &calibration_log_path(),
    );

    let plan = s2g_core::partition::plan(
        &recipe,
        &model,
        &predictors,
        profile.effective_budget_bytes(),
        profile.map_file.max_tiles_per_mapset,
    );
    Ok(PartitionPlan {
        parts: plan.parts.len(),
        columns: plan.columns,
        rows: plan.rows,
        reason: plan.reason.clone(),
        fits: plan.fits,
        part_names: plan.parts.iter().map(|p| p.recipe.name.clone()).collect(),
        part_bytes: plan.parts.iter().map(|p| p.estimated_bytes).collect(),
    })
}

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

/// Export an area selection as GeoJSON (SPEC.md FR-42).
#[tauri::command]
pub fn area_to_geojson(area: s2g_core::recipe::AreaSelection) -> IpcResult<String> {
    serde_json::to_string_pretty(&s2g_core::geojson::to_geojson(&area)).map_err(|e| e.to_string())
}

/// Read an area selection from GeoJSON text the frontend has already loaded.
#[tauri::command]
pub fn area_from_geojson(text: String) -> IpcResult<s2g_core::recipe::AreaSelection> {
    s2g_core::geojson::from_geojson(&text).map_err(|e| e.to_string())
}

/// Save a selection as a GeoJSON file.
#[tauri::command]
pub fn export_area(area: s2g_core::recipe::AreaSelection, path: String) -> IpcResult<String> {
    let json = serde_json::to_string_pretty(&s2g_core::geojson::to_geojson(&area))
        .map_err(|e| e.to_string())?;
    let path = PathBuf::from(path);
    // A selection is a document the user names; the extension is added only if missing.
    let path = if path.extension().is_some() {
        path
    } else {
        path.with_extension("geojson")
    };
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
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
pub async fn admin_extent(level: String, numbers: Vec<i64>, buffer_km: f64) -> IpcResult<[f64; 4]> {
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

/// A build the app was running when it last stopped (SPEC.md §12).
///
/// Not a `ts-rs` type: it carries a whole `Recipe`, which ts-rs cannot export, so the
/// interface is hand-written in `api.ts` beside `Recipe` itself.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterruptedBuild {
    /// Identifies the build for `discard_interrupted`; also what the user sees.
    pub work_dir: String,
    pub recipe_name: String,
    /// The whole recipe, so "Resume" can start it without the UI remembering anything.
    pub recipe: Recipe,
    pub started_at: String,
    pub bytes: u64,
    /// True when the cached region survived, so resuming skips about 80 % of the work.
    pub resumable: bool,
    /// Java processes from that build still running now.
    pub stray_processes: usize,
}

/// Builds that never finished, found on startup (FR-73, SPEC.md §12).
///
/// Called once when the app starts. Anything reported here is both wasting disk and,
/// if `strayProcesses` is non-zero, burning CPU right now.
#[tauri::command]
pub async fn interrupted_builds() -> IpcResult<Vec<InterruptedBuild>> {
    let root = Cache::default_root();
    let found = tokio::task::spawn_blocking(move || {
        s2g_core::recovery::scan(&root, &s2g_core::recovery::SystemProbe)
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(found
        .into_iter()
        .map(|o| InterruptedBuild {
            work_dir: o.work_dir.to_string_lossy().to_string(),
            recipe_name: o.recipe.name.clone(),
            started_at: o.started_at,
            bytes: o.bytes,
            resumable: o.resumable,
            stray_processes: o.stray_pids.len(),
            recipe: o.recipe,
        })
        .collect())
}

/// Kill what is left of an interrupted build and delete its files.
///
/// Takes the work directory rather than an index, so a stale list from before another
/// window discarded the same build cannot delete the wrong one.
#[tauri::command]
pub async fn discard_interrupted(work_dir: String) -> IpcResult<DataLocation> {
    let root = Cache::default_root();
    let scan_root = root.clone();
    let target = PathBuf::from(work_dir);
    tokio::task::spawn_blocking(move || {
        let found = s2g_core::recovery::scan(&scan_root, &s2g_core::recovery::SystemProbe);
        match found.iter().find(|o| o.work_dir == target) {
            Some(o) => s2g_core::recovery::discard(o).map(|_| ()),
            // Already gone, or never interrupted: nothing to do and nothing to report.
            None => Ok(()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    Ok(describe_location(root).await)
}

/// One third-party component, its licence, and where its licence text can be read.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct Component {
    pub name: String,
    pub version: Option<String>,
    pub license: String,
    /// Path to the licence text shipped with the app, when one is present. `None` means
    /// the component's licence is named in NOTICE but its text is not bundled — which
    /// the About screen says, rather than implying a file that is not there.
    pub license_path: Option<String>,
    pub url: String,
}

/// Everything FR-L1…FR-L4 require to be visible in the app.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
#[serde(rename_all = "camelCase")]
pub struct AboutInfo {
    pub app_version: String,
    /// The exact string embedded in every generated map, so what the user reads here is
    /// what travels with the file (FR-L1).
    pub map_copyright: String,
    pub components: Vec<Component>,
    /// Path to this installation's NOTICE, for the full text (FR-L2).
    pub notice_path: Option<String>,
}

/// The About screen's content (FR-L1…FR-L4).
///
/// Assembled here rather than written into the frontend so that the attribution the user
/// reads is the same string the pipeline embeds in the map, and the tool versions are the
/// ones actually installed rather than the ones the documentation remembers.
#[tauri::command]
pub fn about() -> IpcResult<AboutInfo> {
    let root = resource_root();
    let toolchain = s2g_core::garmin::Toolchain::discover(&root).ok();
    let present = |p: PathBuf| p.is_file().then(|| p.display().to_string());

    let mut components = vec![
        Component {
            name: "mkgmap".into(),
            version: toolchain.as_ref().and_then(|t| t.mkgmap_version()),
            license: "GPL-2.0".into(),
            license_path: present(root.join("vendor/mkgmap-r4924/LICENCE")),
            url: "https://www.mkgmap.org.uk/".into(),
        },
        Component {
            name: "splitter".into(),
            version: toolchain.as_ref().and_then(|t| t.splitter_version()),
            // GPL-3.0-only, from splitter's own source headers ("version 3", with no
            // "or later"); see the note in NOTICE for how that was settled.
            license: "GPL-3.0-only".into(),
            license_path: present(root.join("vendor/splitter-r654/doc/LICENSE-gpl-3.0.txt")),
            url: "https://www.mkgmap.org.uk/".into(),
        },
        Component {
            name: "Eclipse Temurin".into(),
            version: toolchain.as_ref().and_then(|t| t.java_version()),
            license: "GPL-2.0 with Classpath Exception".into(),
            // Temurin ships its own licences under `legal/`, per module. `java.base`
            // holds the GPL v2 text, the OpenJDK assembly exception and the Classpath
            // Exception, so pointing there is pointing at the real thing. Both layouts
            // are probed because macOS bundles put the home inside `Contents/Home`.
            license_path: [
                "vendor/jre/legal/java.base",
                "vendor/jre/Contents/Home/legal/java.base",
                "vendor/jdk/legal/java.base",
                "vendor/jdk/Contents/Home/legal/java.base",
            ]
            .iter()
            .map(|p| root.join(p))
            .find(|p| p.join("LICENSE").is_file())
            .map(|p| p.display().to_string()),
            url: "https://adoptium.net/".into(),
        },
    ];
    components.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(AboutInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        // Taken from the same place the build takes it, so the two cannot drift.
        map_copyright: s2g_core::garmin::MapIdentity::for_recipe("about", "about").description,
        components,
        notice_path: present(root.join("NOTICE")),
    })
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
    let profiles = profiles()?;
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
    let (group_counts, counted) =
        match pipeline::find_tlm3d(&cache_root).and_then(|p| s2g_core::gpkg::Gpkg::open(p).ok()) {
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
        // The wrist cartography drops layers and labels, so the same area compiles
        // considerably smaller and the estimate has to know which one this device gets.
        // An unrecognised device id falls back to the full cartography, which
        // over-estimates rather than under-estimates — the safe direction for a budget.
        wrist: profile.map(|p| p.is_wrist()).unwrap_or(false),
    };
    let estimated = model.predict(&predictors);
    // What to do about it, if anything. The decision and the ordering live in
    // s2g_core::estimate so they are tested; the UI only translates the names.
    let verdict = estimate::budget_verdict(estimated, budget, &probe);

    Ok(AreaInfo {
        area_km2: bbox.area_km2(),
        wgs84: bbox.to_wgs84(),
        within_switzerland: bbox.within_switzerland(),
        estimated_bytes: estimated,
        budget_bytes: budget,
        hard_limit_bytes: hard_limit,
        over_budget: verdict.over_budget,
        overshoot_bytes: verdict.overshoot_bytes,
        remedies: verdict
            .remedies
            .iter()
            .map(|r| {
                serde_json::to_value(r)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default()
            })
            .collect(),
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
        let weights =
            estimate::stage_weights(&estimate::CalibrationLog::read(&calibration_log_path()));
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
    let profiles = profiles()?;
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
    let profiles = profiles()?;
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
    let profiles = profiles()?;
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
    // The copy, the verification and the rollback live in s2g_core::install, where the
    // device-goes-away cases have tests. A cable cannot be unplugged in CI.
    let done = tokio::task::spawn_blocking(move || s2g_core::install::install(&src, &dst, backup))
        .await
        .map_err(|e| e.to_string())?;
    match done {
        Ok(done) => Ok(done.target.display().to_string()),
        Err(s2g_core::Error::ChecksumMismatch {
            expected, actual, ..
        }) => Err(format!(
            "the copy on the device does not match what was built \
             (built {}, on the device {}); it has been removed",
            &expected[..16],
            &actual[..16]
        )),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod resource_root_tests {
    use super::*;

    /// A macOS bundle puts the executable in `Contents/MacOS` and its resources in
    /// `Contents/Resources`. Walking ancestors alone never reaches them, which is why a
    /// packaged app resolved its root to "." and could not build anything (NFR-7).
    #[test]
    fn a_macos_bundle_layout_resolves_to_contents_resources() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("swisstopo2garmin.app");
        let resources = app.join("Contents").join("Resources");
        std::fs::create_dir_all(resources.join("devices")).unwrap();
        std::fs::create_dir_all(resources.join("style")).unwrap();
        std::fs::create_dir_all(app.join("Contents").join("MacOS")).unwrap();

        let exe = app.join("Contents").join("MacOS").join("swisstopo2garmin");
        let bases = exe.ancestors().map(Path::to_path_buf);
        assert_eq!(resolve_resource_root(bases), Some(resources));
    }

    /// The Linux and Windows bundles put a lowercase `resources` beside the executable.
    ///
    /// Compared after canonicalising, because macOS is case-insensitive by default and
    /// the search tries `Resources` first: it finds the right directory under a name
    /// that differs from the one on disk, which is correct behaviour and a string
    /// comparison would call a failure.
    #[test]
    fn a_sibling_resources_directory_resolves() {
        let dir = tempfile::tempdir().unwrap();
        let resources = dir.path().join("resources");
        std::fs::create_dir_all(resources.join("devices")).unwrap();
        std::fs::create_dir_all(resources.join("style")).unwrap();

        let exe = dir.path().join("swisstopo2garmin");
        let bases = exe.ancestors().map(Path::to_path_buf);
        let found = resolve_resource_root(bases).expect("should resolve");
        assert_eq!(
            found.canonicalize().unwrap(),
            resources.canonicalize().unwrap()
        );
    }

    /// A development checkout must keep working: the root itself holds the files.
    #[test]
    fn a_development_checkout_resolves_to_the_repository_root() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("devices")).unwrap();
        std::fs::create_dir_all(dir.path().join("style")).unwrap();
        let exe = dir.path().join("target").join("debug").join("app");
        let bases = exe.ancestors().map(Path::to_path_buf);
        assert_eq!(resolve_resource_root(bases), Some(dir.path().to_path_buf()));
    }

    /// Half a layout must not be accepted: `style` alone is a common directory name.
    #[test]
    fn a_directory_with_only_one_marker_is_not_a_resource_root() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("style")).unwrap();
        assert_eq!(
            resolve_resource_root(std::iter::once(dir.path().to_path_buf())),
            None
        );
    }

    /// The real bundle, when one has been built. This is the only assertion here that
    /// can fail because of a packaging change rather than a logic change, so it skips
    /// rather than failing when there is no bundle to look at.
    #[test]
    fn the_real_bundle_is_self_sufficient() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let app = repo.join("target/release/bundle/macos/swisstopo2garmin.app");
        if !app.is_dir() {
            eprintln!("skipping: no macOS bundle built (cargo tauri build --bundles app)");
            return;
        }

        let exe = app.join("Contents").join("MacOS").join("swisstopo2garmin");
        let bases = exe.ancestors().map(Path::to_path_buf);
        let root = resolve_resource_root(bases).expect("the bundle should carry its resources");
        assert!(root.ends_with("Contents/Resources"), "{root:?}");

        // Everything a build reads, present inside the bundle.
        for needed in ["devices", "style", "typ", "estimator", "NOTICE", "LICENSE"] {
            assert!(root.join(needed).exists(), "the bundle has no {needed}");
        }

        // And the toolchain, found from the layout. The env file the bundle also carries
        // holds absolute paths into the machine that built it -- including a `vendor/jdk`
        // that is deliberately not shipped -- so this is the assertion that a packaged
        // app can compile a map at all.
        let tc = s2g_core::garmin::Toolchain::discover(&root)
            .expect("the bundled toolchain should be discoverable");
        assert!(
            tc.java.starts_with(&root),
            "java came from outside the bundle: {:?}",
            tc.java
        );
        assert!(tc.mkgmap_jar.starts_with(&root), "{:?}", tc.mkgmap_jar);
        assert!(tc.splitter_jar.starts_with(&root), "{:?}", tc.splitter_jar);
        assert!(
            tc.mkgmap_version().is_some(),
            "the bundled java could not run mkgmap"
        );
    }
}
