//! 应用级共享状态。
//!
//! 在 Tauri 里，`State<T>` 可以理解成后端的全局依赖容器。
//! 这里保存数据库入口，以及“哪些项目正在后台运行”的内存状态。

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
    /// 数据库入口。它本身很轻，只保存数据库路径。
    pub db: Database,
    /// 当前正在后台处理的项目集合。
    /// 作用是阻止同一项目被同时启动多个 job。
    active_projects: Arc<Mutex<HashSet<String>>>,
}

impl AppState {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            active_projects: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// 这些辅助方法相当于轻量工厂，
    /// 方便 command/service 临时拿到对应 repository。
    pub fn project_repository(&self) -> ProjectRepository {
        ProjectRepository::new(self.db.clone())
    }

    pub fn app_settings_repository(&self) -> AppSettingsRepository {
        AppSettingsRepository::new(self.db.clone())
    }

    pub fn proofreading_repository(&self) -> ProofreadingRepository {
        ProofreadingRepository::new(self.db.clone())
    }

    /// 启动时恢复可续跑的 job。
    ///
    /// 恢复动作本身异步执行，不阻塞桌面程序启动。
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

    /// 查询某个项目是否已经有后台任务在跑。
    pub async fn is_project_active(&self, project_id: &str) -> bool {
        let active = self.active_projects.lock().await;
        active.contains(project_id)
    }

    /// 删除项目时主动释放内存里的运行标记。
    pub async fn release_project(&self, project_id: &str) {
        let mut active = self.active_projects.lock().await;
        active.remove(project_id);
    }

    /// 派发一个后台 job。
    ///
    /// 流程是：
    /// 1. 先尝试占用项目锁。
    /// 2. 再异步调用 `ProofreadService::run_job`。
    /// 3. 结束后把项目从运行中集合移除。
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
                // job 主流程异常时，再补一次失败收尾，防止状态留在中间态。
                let _ = service.fail_job(job_for_failure, &error.message);
            }
            let mut active = active_projects.lock().await;
            active.remove(&project_id);
        });
    }

    /// 抢占项目级运行锁。
    ///
    /// 返回 `false` 说明该项目已经在执行中。
    async fn acquire_project(&self, project_id: &str) -> bool {
        let mut active = self.active_projects.lock().await;
        if active.contains(project_id) {
            return false;
        }
        active.insert(project_id.to_string());
        true
    }
}
