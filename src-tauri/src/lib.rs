mod commands;
mod db;
mod error;
mod repository;
mod state;
mod types;

use commands::{list_projects, ping};
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
        .invoke_handler(tauri::generate_handler![ping, list_projects])
        .run(tauri::generate_context!())
        .expect("failed to run proofdesk application");
}
