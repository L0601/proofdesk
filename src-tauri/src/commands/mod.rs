use serde::Serialize;
use tauri::State;

use crate::error::AppResult;
use crate::services::import_service::ImportService;
use crate::state::AppState;
use crate::types::{
    AppSettings, NormalizedDocument, ProjectDetail, ProjectSummary, SourceType,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    pub name: String,
    pub version: String,
    pub status: String,
}

#[tauri::command]
pub fn ping() -> HealthCheck {
    HealthCheck {
        name: "ProofDesk".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        status: "ok".to_string(),
    }
}

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> AppResult<Vec<ProjectSummary>> {
    state.project_repository().list()
}

#[tauri::command]
pub fn get_project_detail(
    state: State<'_, AppState>,
    project_id: String,
) -> AppResult<Option<ProjectDetail>> {
    state.project_repository().get(&project_id)
}

#[tauri::command]
pub fn import_document(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    file_path: String,
) -> AppResult<ProjectSummary> {
    ImportService::new(state.db.clone()).import_document(&app, &file_path)
}

#[tauri::command]
pub fn import_normalized_document(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    file_path: String,
    source_type: SourceType,
    normalized_document: NormalizedDocument,
) -> AppResult<ProjectSummary> {
    ImportService::new(state.db.clone()).import_normalized_document(
        &app,
        &file_path,
        source_type,
        normalized_document,
    )
}

#[tauri::command]
pub fn get_app_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    state.app_settings_repository().get()
}

#[tauri::command]
pub fn save_app_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> AppResult<AppSettings> {
    state.app_settings_repository().save(&settings)?;
    Ok(settings)
}
