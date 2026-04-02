mod ai_review;
mod analysis;
mod asset_metrics;
mod commands;
mod device_profile;
mod device_transfer;
mod graph;
mod profiler_report;
mod profiler_session;
mod unity_connection;
mod workspace;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::select_project,
            commands::get_project_info,
            commands::run_analysis,
            commands::get_asset_graph,
            commands::get_code_graph,
            commands::get_stats,
            commands::get_suspected_refs,
            commands::promote_suspected_ref,
            commands::ignore_suspected_ref,
            commands::get_hardcode_findings,
            commands::save_settings,
            commands::load_settings,
            commands::detect_ai_clis,
            commands::open_file_location,
            commands::read_image_base64,
            commands::run_ai_analysis,
            commands::run_deep_ai_analysis,
            commands::run_ai_batch_analysis,
            commands::update_node_ai_summary,
            commands::export_analysis,
            commands::has_analysis_cache,
            commands::save_analysis_cache,
            commands::get_orphan_nodes,
            commands::get_duplicate_resources,
            commands::get_hotspots,
            commands::get_asset_metrics,
            commands::run_ai_code_review,
            commands::run_ai_project_code_review,
            commands::run_ai_asset_review,
            // Profiler commands
            commands::discover_unity,
            commands::connect_unity,
            commands::get_unity_status,
            commands::disconnect_unity,
            commands::start_profiling,
            commands::stop_profiling,
            commands::list_profiler_sessions,
            commands::get_profiler_session,
            commands::delete_profiler_session,
            commands::rename_profiler_session,
            commands::generate_profiler_report,
            commands::generate_deep_profiler_analysis,
            commands::compare_profiler_sessions,
            commands::export_profiler_report,
            commands::export_profiler_comparison,
            // Device profiler commands
            commands::discover_devices,
            commands::get_device_status,
            commands::list_device_sessions,
            commands::download_device_session,
            commands::remote_start_capture,
            commands::remote_stop_capture,
            commands::import_gaprof_file,
            commands::parse_gaprof_session,
            commands::generate_device_report,
            commands::get_device_screenshot,
            commands::export_device_report,
            commands::get_frame_functions,
            commands::get_session_logs,
            commands::run_ai_device_analysis,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
