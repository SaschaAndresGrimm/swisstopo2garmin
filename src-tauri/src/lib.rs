//! Tauri shell. Deliberately thin: everything of substance lives in `s2g-core`,
//! which is testable without a GUI (PLAN.md working agreement, rule 5).

mod ipc;

pub fn run() {
    tauri::Builder::default()
        // Only for the data-directory picker; a webview cannot browse the filesystem.
        .plugin(tauri_plugin_dialog::init())
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
            ipc::list_devices,
            ipc::device_override,
            ipc::set_device_override,
            ipc::detect_devices,
            ipc::usb_devices,
            ipc::list_presets,
            ipc::list_layers,
            ipc::list_recipes,
            ipc::save_recipe,
            ipc::load_recipe,
            ipc::delete_recipe,
            ipc::find_places,
            ipc::describe_area,
            ipc::partition_plan,
            ipc::wgs84_bbox_to_lv95,
            ipc::coverage_bbox,
            ipc::data_location,
            ipc::inspect_data_location,
            ipc::set_data_location,
            ipc::clear_elevation_cache,
            ipc::clear_build_files,
            ipc::import_track,
            ipc::list_admin_units,
            ipc::admin_extent,
            ipc::admin_outline,
            ipc::lv95_line_to_wgs84,
            ipc::area_to_geojson,
            ipc::area_from_geojson,
            ipc::export_area,
            ipc::start_build,
            ipc::plan_install,
            ipc::install_map,
            ipc::export_map,
            ipc::install_instructions,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start swisstopo2garmin");
}
