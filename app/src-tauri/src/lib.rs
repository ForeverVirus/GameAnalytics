mod analysis;
mod commands;
mod graph;
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
            commands::open_file_location,
            commands::read_image_base64,
            commands::run_ai_analysis,
            commands::run_deep_ai_analysis,
            commands::run_ai_batch_analysis,
            commands::update_node_ai_summary,
            commands::export_analysis,
            commands::has_analysis_cache,
            commands::save_analysis_cache,
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
