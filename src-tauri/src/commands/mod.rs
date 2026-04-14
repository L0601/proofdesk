//! Tauri command 层。
//!
//! 可以把它理解成前端调用后端的入口层：
//! 前端通过 `invoke` 调到这里，再由这里转发到 repository/service。

use serde::Serialize;
use tauri::{Manager, State};

use crate::error::AppResult;
use crate::services::import_service::ImportService;
use crate::services::proofread_service::ProofreadService;
use crate::state::AppState;
use crate::types::{
    AppSettings, NormalizedDocument, ProjectDetail, ProjectSummary, ProofreadOptions,
    ProofreadingCall, ProofreadingIssue, ProofreadingJob, SourceType,
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
    // 最基础的健康检查，用来确认前后端桥接是否可用。
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
pub async fn delete_project(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    project_id: String,
) -> AppResult<()> {
    // 删除前先确认后台没有任务在跑，避免数据库和任务互相打架。
    if state.is_project_active(&project_id).await {
        return Err(crate::error::AppError::new(
            "project_processing",
            "项目正在后台处理中，暂不允许删除",
        ));
    }

    let project_root = app.path().app_data_dir()?.join("projects").join(&project_id);
    state.project_repository().delete(&project_id)?;
    state.project_repository().delete_project_dir(&project_root)?;
    Ok(())
}

#[tauri::command]
pub fn import_document(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    file_path: String,
) -> AppResult<ProjectSummary> {
    ImportService::new(state.db.clone()).import_document(&app, &file_path)
}

/// PDF 链路会先由前端解析，再把标准化 JSON 发给后端落库。
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

#[tauri::command]
pub async fn start_proofreading(
    state: State<'_, AppState>,
    project_id: String,
    options: ProofreadOptions,
) -> AppResult<ProofreadingJob> {
    // 第一层保护：数据库里如果已经存在 running job，直接复用。
    if let Some(job) = state.proofreading_repository().get_running_job(&project_id)? {
        return Ok(job);
    }

    // 第二层保护：如果内存状态显示该项目正在执行，也直接返回最近的 job。
    if state.is_project_active(&project_id).await {
        if let Some(job) = state.proofreading_repository().get_latest_job(&project_id)? {
            return Ok(job);
        }
    }

    let job = ProofreadService::new(state.db.clone()).start_job(&project_id, options)?;
    state.spawn_job(job.clone()).await;
    Ok(job)
}

/// 下面这些命令都是纯读取，不会触发新的模型调用。
#[tauri::command]
pub fn get_latest_proofreading_job(
    state: State<'_, AppState>,
    project_id: String,
) -> AppResult<Option<ProofreadingJob>> {
    state.proofreading_repository().get_latest_job(&project_id)
}

#[tauri::command]
pub fn list_proofreading_issues(
    state: State<'_, AppState>,
    project_id: String,
) -> AppResult<Vec<ProofreadingIssue>> {
    state.proofreading_repository().list_issues(&project_id)
}

#[tauri::command]
pub fn list_proofreading_calls(
    state: State<'_, AppState>,
    project_id: String,
) -> AppResult<Vec<ProofreadingCall>> {
    state.proofreading_repository().list_calls(&project_id)
}
