mod commands;
mod db;
mod error;
mod repository;
mod services;
mod state;
mod types;

use commands::{
    get_app_settings, get_project_detail, import_document, import_normalized_document,
    get_latest_proofreading_job, list_proofreading_issues, list_projects, ping,
    save_app_settings, start_proofreading,
};
use db::Database;
use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let db = Database::init(app.handle())?;
            app.manage(AppState { db });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            list_projects,
            get_project_detail,
            get_app_settings,
            get_latest_proofreading_job,
            list_proofreading_issues,
            save_app_settings,
            start_proofreading,
            import_document,
            import_normalized_document
        ])
        .run(tauri::generate_context!())
        .expect("failed to run proofdesk application");
}
