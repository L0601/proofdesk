use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::db::Database;
use crate::repository::{
    app_settings_repository::AppSettingsRepository, project_repository::ProjectRepository,
    proofreading_repository::ProofreadingRepository,
};
use crate::services::proofread_service::ProofreadService;
use crate::types::ProofreadingJob;

#[derive(Debug, Clone)]
pub struct AppState {
    pub db: Database,
    active_projects: Arc<Mutex<HashSet<String>>>,
}

impl AppState {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            active_projects: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn project_repository(&self) -> ProjectRepository {
        ProjectRepository::new(self.db.clone())
    }

    pub fn app_settings_repository(&self) -> AppSettingsRepository {
        AppSettingsRepository::new(self.db.clone())
    }

    pub fn proofreading_repository(&self) -> ProofreadingRepository {
        ProofreadingRepository::new(self.db.clone())
    }

    pub fn resume_pending_jobs(&self) {
        let state = self.clone();
        tauri::async_runtime::spawn(async move {
            let jobs = match state.proofreading_repository().list_resumable_jobs() {
                Ok(jobs) => jobs,
                Err(_) => return,
            };

            for job in jobs {
                state.spawn_job(job).await;
            }
        });
    }

    pub async fn is_project_active(&self, project_id: &str) -> bool {
        let active = self.active_projects.lock().await;
        active.contains(project_id)
    }

    pub async fn spawn_job(&self, job: ProofreadingJob) {
        if !self.acquire_project(&job.project_id).await {
            return;
        }

        let db = self.db.clone();
        let active_projects = self.active_projects.clone();
        let job_for_failure = job.clone();
        let project_id = job.project_id.clone();
        tauri::async_runtime::spawn(async move {
            let service = ProofreadService::new(db);
            if let Err(error) = service.run_job(job).await {
                let _ = service.fail_job(job_for_failure, &error.message);
            }
            let mut active = active_projects.lock().await;
            active.remove(&project_id);
        });
    }

    async fn acquire_project(&self, project_id: &str) -> bool {
        let mut active = self.active_projects.lock().await;
        if active.contains(project_id) {
            return false;
        }
        active.insert(project_id.to_string());
        true
    }
}
