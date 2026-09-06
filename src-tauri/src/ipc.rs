//! Typed IPC surface (SPEC.md §10.4).
//!
//! Every type here derives `TS`, and `cargo test -p swisstopo2garmin` writes the
//! TypeScript definitions to `frontend/src/state/bindings.ts`. CI fails if the checked-in
//! file differs, so the two sides cannot drift.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use s2g_core::cache::{available_bytes, Cache, Provenance};
use s2g_core::download::{download_zip_member_inflated, Cancel, Progress};
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
    let asset = item
        .asset_ending(".gpkg.zip")
        .map_err(|e| e.to_string())?
        .clone();

    // Probe the archive so the UI can state both the download size and the far larger
    // on-disk size before the user commits (SPEC.md FR-C1).
    let (archive_bytes, member_bytes, member_name) = {
        use s2g_core::http::Http;
        match http.head(&asset.href).await {
            Ok(h) => match h.len {
                Some(total) => match s2g_core::zip::first_member(&http, &asset.href, total).await {
                    Ok(m) => (Some(total), Some(m.uncompressed_size), Some(m.name)),
                    Err(_) => (Some(total), None, None),
                },
                None => (None, None, None),
            },
            Err(_) => (None, None, None),
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
    let asset = item
        .asset_ending(".gpkg.zip")
        .map_err(|e| e.to_string())?
        .clone();

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

        let result = download_zip_member_inflated(
            &http,
            &asset.href,
            &dest,
            asset.checksum.as_ref(),
            &cancel,
            &mut on_progress,
        )
        .await;

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
use s2g_core::pipeline::{self, BuildContext, Stage};
use s2g_core::proj::{lv95_to_wgs84, BBox};
use s2g_core::recipe::{Preset, Recipe};

/// Directory holding the app's data files. In development this is the repo; in a
/// bundle it is the resource directory.
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
        devices::load_profiles(&resource_root().join("devices")).map_err(|e| e.to_string())?;
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

#[tauri::command]
pub fn detect_devices() -> IpcResult<Vec<ConnectedDevice>> {
    let profiles =
        devices::load_profiles(&resource_root().join("devices")).map_err(|e| e.to_string())?;
    Ok(devices::detect()
        .into_iter()
        .map(|d| {
            // A watch reports itself as fenix/epix/Forerunner; anything else falls back
            // to the generic Edge profile.
            let looks_wrist = d
                .model
                .as_deref()
                .map(|m| {
                    let m = m.to_lowercase();
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
                })
                .unwrap_or(false);
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
    /// Rough output size, from area alone until the estimator is calibrated (FR-60).
    #[ts(type = "number")]
    pub estimated_bytes: u64,
    pub over_budget: bool,
}

/// Measured on the Grindelwald builds: about 1.17 MB per 144 km² with 20 m contours,
/// i.e. ~8.1 KB/km². Crude but honest, and replaced by the calibrated estimator.
const BYTES_PER_KM2: f64 = 8_100.0;

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

#[tauri::command]
pub fn describe_area(
    min_e: f64,
    min_n: f64,
    max_e: f64,
    max_n: f64,
    device_id: String,
) -> IpcResult<AreaInfo> {
    let bbox = BBox::new(min_e, min_n, max_e, max_n);
    let profiles =
        devices::load_profiles(&resource_root().join("devices")).map_err(|e| e.to_string())?;
    let budget = profiles
        .iter()
        .find(|p| p.id == device_id)
        .map(|p| p.effective_budget_bytes())
        .unwrap_or(u64::MAX);
    let estimated = (bbox.area_km2() * BYTES_PER_KM2) as u64;
    Ok(AreaInfo {
        area_km2: bbox.area_km2(),
        wgs84: bbox.to_wgs84(),
        within_switzerland: bbox.within_switzerland(),
        estimated_bytes: estimated,
        over_budget: estimated > budget,
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
    pub fraction: Option<f64>,
    pub detail: String,
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
    let profiles = devices::load_profiles(&root.join("devices")).map_err(|e| e.to_string())?;
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
        };

        let emit = app.clone();
        let pid = id.clone();
        let mut last = std::time::Instant::now() - PROGRESS_INTERVAL;
        let result = pipeline::build(&ctx, &recipe, &profile, &cancel, move |u| {
            // Stage changes are always reported; progress within a stage is throttled.
            if last.elapsed() < PROGRESS_INTERVAL && u.fraction.is_some() {
                return;
            }
            last = std::time::Instant::now();
            let index = Stage::all().iter().position(|s| *s == u.stage).unwrap_or(0);
            let _ = emit.emit(
                "build:progress",
                BuildProgress {
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
                        warnings: report.warnings,
                    },
                );
            }
            Err(e) => {
                eprintln!("build failed: {e}");
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
        devices::load_profiles(&resource_root().join("devices")).map_err(|e| e.to_string())?;
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

    // Verify by size; hashing a multi-gigabyte file over USB is not worth the wait.
    let copied = std::fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
    if copied != plan.bytes {
        return Err(format!(
            "copy verification failed: {copied} bytes on the device, expected {}",
            plan.bytes
        ));
    }
    Ok(dst.display().to_string())
}
