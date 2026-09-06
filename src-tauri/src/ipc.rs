//! Typed IPC surface (SPEC.md §10.4).
//!
//! Every type here derives `TS`, and `cargo test -p swisstopo2garmin` writes the
//! TypeScript definitions to `frontend/src/state/bindings.ts`. CI fails if the checked-in
//! file differs, so the two sides cannot drift.

use std::collections::HashMap;
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
pub struct TaskDone {
    pub task_id: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../frontend/src/state/bindings.ts")]
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
