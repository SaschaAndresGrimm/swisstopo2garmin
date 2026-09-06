//! Tauri shell. Deliberately thin: everything of substance lives in `s2g-core`,
//! which is testable without a GUI (PLAN.md working agreement, rule 5).

mod ipc;

pub fn run() {
    tauri::Builder::default()
        .setup(|_app| {
            // Reclaim partial downloads stranded by a previous kill or power loss.
            // An inflating download cannot be resumed, so a leftover `.part` is pure
            // waste -- a killed 4.8 GB acquisition strands gigabytes (SPEC.md §12).
            tauri::async_runtime::spawn(async {
                use s2g_core::cache::Cache;
                let cache = Cache::new(Cache::default_root());
                match cache.sweep_partials().await {
                    Ok((0, _)) => {}
                    Ok((freed, removed)) => {
                        eprintln!(
                            "reclaimed {:.2} GB from {} orphaned partial download(s)",
                            freed as f64 / 1e9,
                            removed.len()
                        );
                    }
                    Err(e) => eprintln!("could not sweep partial downloads: {e}"),
                }
            });
            Ok(())
        })
        .manage(ipc::AppState::default())
        .invoke_handler(tauri::generate_handler![
            ipc::cache_status,
            ipc::latest_release,
            ipc::acquire_dataset,
            ipc::cancel_task,
            ipc::remove_dataset,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start swisstopo2garmin");
}
