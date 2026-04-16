//! Tauri 后端入口。
//!
//! 这里主要负责三件事：
//! 1. 注册模块。
//! 2. 初始化数据库与全局状态。
//! 3. 把前端可调用的命令暴露给 Tauri。

mod commands;
mod db;
mod error;
pub mod local_debug;
mod repository;
mod services;
mod state;
mod types;

use commands::{
    delete_project, get_app_settings, get_latest_proofreading_job, get_project_detail,
    import_document, import_normalized_document, list_proofreading_calls,
    list_proofreading_issues, list_projects, pause_proofreading, ping, save_app_settings,
    start_proofreading,
};
use db::Database;
use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // `Builder` 可以理解成桌面应用启动时的装配器。
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // 先保证数据库和迁移已经就绪。
            let db = Database::init(app.handle())?;
            let state = AppState::new(db);

            // 启动时自动恢复上次未完成且允许恢复的任务。
            state.resume_pending_jobs();

            // 把状态挂进 Tauri，后续 command 可通过 `State<AppState>` 取到。
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            list_projects,
            get_project_detail,
            delete_project,
            get_app_settings,
            get_latest_proofreading_job,
            list_proofreading_calls,
            list_proofreading_issues,
            save_app_settings,
            start_proofreading,
            pause_proofreading,
            import_document,
            import_normalized_document
        ])
        .run(tauri::generate_context!())
        .expect("failed to run proofdesk application");
}
