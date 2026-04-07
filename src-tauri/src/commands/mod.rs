use serde::Serialize;
use tauri::State;

use crate::error::AppResult;
use crate::services::import_service::ImportService;
use crate::state::AppState;
use crate::types::{ProjectDetail, ProjectSummary};

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
