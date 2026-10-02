mod backend;
mod commands;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            backend::setup(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::app_status,
            commands::settings_get,
            commands::settings_put,
            commands::probe_video,
            commands::analyze_start,
            commands::analyze_status,
            commands::analyze_cancel,
            commands::analysis_result,
            commands::events_retreshold,
            commands::proxy_ensure,
            commands::markers_load,
            commands::markers_save,
            commands::export_markers,
            commands::reveal_path,
            commands::cache_stats,
            commands::clear_cache,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CyTracer");
}
