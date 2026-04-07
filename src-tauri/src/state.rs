use crate::db::Database;
use crate::repository::{
    app_settings_repository::AppSettingsRepository, project_repository::ProjectRepository,
    proofreading_repository::ProofreadingRepository,
};

#[derive(Debug, Clone)]
pub struct AppState {
    pub db: Database,
}

impl AppState {
    pub fn project_repository(&self) -> ProjectRepository {
        ProjectRepository::new(self.db.clone())
    }

    pub fn app_settings_repository(&self) -> AppSettingsRepository {
        AppSettingsRepository::new(self.db.clone())
    }

    pub fn proofreading_repository(&self) -> ProofreadingRepository {
        ProofreadingRepository::new(self.db.clone())
    }
}
