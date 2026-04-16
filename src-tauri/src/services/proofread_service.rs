//! AI 校对服务。
//!
//! 这是业务编排最集中的文件，负责：
//! - 创建 job
//! - 选取待处理 block
//! - 并发调度 worker
//! - 调模型
//! - 记录 calls / issues / 日志
//! - 更新 job 和项目状态

use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::repository::app_settings_repository::AppSettingsRepository;
use crate::repository::project_repository::ProjectRepository;
use crate::repository::proofreading_repository::{
    JobMetrics, NewCallRecord, ProofreadingRepository,
};
use crate::types::{
    AppSettings, DocumentBlock, IssueSeverity, IssueStatus, IssueType, ProjectStatus,
    ProofreadOptions, ProofreadingIssue, ProofreadingJob, ProofreadingMode,
    ProofreadingStatus,
};

pub struct ProofreadService {
    db: Database,
}

/// 下面几组结构体对应模型接口的最小请求/响应结构。
#[derive(Debug, Deserialize, Serialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize, Serialize)]
struct Message {
    content: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Usage {
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ModelPayload {
    issues: Vec<ModelIssue>,
}

#[derive(Debug, Deserialize)]
struct ModelIssue {
    #[serde(rename = "type")]
    issue_type: String,
    severity: String,
    start_offset: i64,
    end_offset: i64,
    quote: String,
    suggestion: String,
    explanation: String,
    normalized_replacement: Option<String>,
}

#[derive(Debug, Serialize)]
struct RequestPayload<'a> {
    block_id: &'a str,
    text: &'a str,
    rules: Vec<String>,
}

struct SanitizedText {
    text: String,
    char_map: Vec<usize>,
}

#[derive(Debug, Serialize)]
pub struct DebugModelCall {
    pub request_json: String,
    pub response_json: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

#[derive(Debug, Serialize)]
struct ModelCallResult {
    message: Message,
    usage: Usage,
}

/// 每个并发 worker 处理 block 时共享的上下文。
#[derive(Clone)]
struct WorkerContext {
    db: Database,
    settings: AppSettings,
    job_id: String,
    project_id: String,
    issue_types: Vec<IssueType>,
    queue: Arc<Mutex<VecDeque<DocumentBlock>>>,
    client: Option<Client>,
}

impl ProofreadService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 创建一个新 job，但不负责实际执行。
    /// 真正的执行由 `AppState::spawn_job` 异步触发。
    pub fn start_job(
        &self,
        project_id: &str,
        options: ProofreadOptions,
    ) -> AppResult<ProofreadingJob> {
        let repo = ProofreadingRepository::new(self.db.clone());
        let project_repo = ProjectRepository::new(self.db.clone());
        let settings = AppSettingsRepository::new(self.db.clone()).get()?;
        if let Some(job) = repo.get_running_job(project_id)? {
            return Ok(job);
        }
        let blocks = select_blocks(
            &repo.list_blocks(project_id)?,
            options.mode,
            settings.proofread_skip_pages,
        )?;
        if blocks.is_empty() {
            return Err(AppError::new("empty_document", "当前项目没有可校对的正文块"));
        }

        let now = crate::services::import_service::now_rfc3339()?;
        // 本次要跑的 block 先统一回到 pending，保证任务起点明确。
        repo.reset_selected_blocks(project_id, options.mode, &now)?;

        let job = ProofreadingJob {
            id: Uuid::new_v4().to_string(),
            project_id: project_id.to_string(),
            mode: options.mode,
            status: ProofreadingStatus::Running,
            options_json: Some(serde_json::to_string(&options)?),
            auto_resume: true,
            started_at: Some(now.clone()),
            finished_at: None,
            total_blocks: blocks.len() as i64,
            completed_blocks: 0,
            failed_blocks: 0,
            total_issues: 0,
            total_tokens_in: 0,
            total_tokens_out: 0,
            total_latency_ms: 0,
        };

        repo.create_job(&job)?;
        project_repo.update_progress(
            project_id,
            ProjectStatus::Processing,
            0,
            0,
            &now,
        )?;
        Ok(job)
    }

    /// 真正执行一个 job。
    ///
    /// 流程是：
    /// 1. 读设置和 job 参数快照
    /// 2. 取出待处理 block
    /// 3. 按并发数创建 worker
    /// 4. 等所有 worker 退出
    /// 5. 聚合统计并收尾
    pub async fn run_job(&self, mut job: ProofreadingJob) -> AppResult<()> {
        let settings = AppSettingsRepository::new(self.db.clone()).get()?;
        let repo = ProofreadingRepository::new(self.db.clone());
        let project_repo = ProjectRepository::new(self.db.clone());
        let options = parse_job_options(&job)?;
        let now = crate::services::import_service::now_rfc3339()?;
        // 恢复任务前，先把意外退出留下的 running block 回退到 pending。
        repo.reset_running_blocks(&job.project_id, &now)?;

        let blocks = select_pending_blocks(
            &repo.list_blocks(&job.project_id)?,
            options.mode,
            settings.proofread_skip_pages,
        )?;
        if blocks.is_empty() {
            let finished_at = crate::services::import_service::now_rfc3339()?;
            finalize_job(&repo, &project_repo, &mut job, &finished_at)?;
            return Ok(());
        }

        let client = build_client(&settings)?;
        let queue = Arc::new(Mutex::new(VecDeque::from(blocks)));
        // 并发数来自设置页，并在后端再做一次上限保护。
        let worker_count = settings.max_concurrency.clamp(1, 32) as usize;
        let mut handles = Vec::new();

        for _ in 0..worker_count {
            let worker = WorkerContext {
                db: self.db.clone(),
                settings: settings.clone(),
                job_id: job.id.clone(),
                project_id: job.project_id.clone(),
                issue_types: options.issue_types.clone(),
                queue: queue.clone(),
                client: client.clone(),
            };
            handles.push(tauri::async_runtime::spawn(async move {
                worker.run().await
            }));
        }

        for handle in handles {
            handle.await.map_err(|error| {
                AppError::new("proofread_worker_join_error", error.to_string())
            })?;
        }

        sync_job_progress(&self.db, &job.id, &job.project_id)?;
        if matches!(current_job_status(&repo, &job.id)?, ProofreadingStatus::Paused) {
            return Ok(());
        }

        let finished_at = crate::services::import_service::now_rfc3339()?;
        finalize_job(&repo, &project_repo, &mut job, &finished_at)?;
        Ok(())
    }

    /// 整个 job 主流程失败时的兜底收尾逻辑。
    pub fn fail_job(&self, mut job: ProofreadingJob, error_message: &str) -> AppResult<()> {
        let repo = ProofreadingRepository::new(self.db.clone());
        let project_repo = ProjectRepository::new(self.db.clone());
        let finished_at = crate::services::import_service::now_rfc3339()?;

        let metrics = repo.job_metrics(&job.id)?;
        apply_metrics(&mut job, metrics, &finished_at);
        job.status = ProofreadingStatus::Failed;
        repo.update_job(&job)?;

        let (completed, failed) = repo.count_block_statuses(&job.project_id)?;
        project_repo.update_progress(
            &job.project_id,
            ProjectStatus::Failed,
            completed,
            failed,
            &finished_at,
        )?;
        let _ = append_model_log(
            &job.project_id,
            &job.id,
            "system",
            "{}",
            None,
            Some(error_message),
        );
        Ok(())
    }
}

impl WorkerContext {
    /// 一个 worker 会不断从共享队列取 block，直到队列为空。
    async fn run(&self) {
        loop {
            if !self.should_continue().await {
                return;
            }
            let block = self.next_block().await;
            let Some(block) = block else {
                return;
            };
            if let Err(error) = self.process_block(block.clone()).await {
                let _ = self.mark_internal_failure(&block, &error.message).await;
            }
        }
    }

    async fn should_continue(&self) -> bool {
        let repo = ProofreadingRepository::new(self.db.clone());
        matches!(
            current_job_status(&repo, &self.job_id),
            Ok(ProofreadingStatus::Running)
        )
    }

    /// 从共享队列里取一个 block。
    async fn next_block(&self) -> Option<DocumentBlock> {
        let mut queue = self.queue.lock().await;
        queue.pop_front()
    }

    /// 单个 block 的完整处理流程。
    async fn process_block(&self, block: DocumentBlock) -> AppResult<()> {
        let repo = ProofreadingRepository::new(self.db.clone());
        let started_at = crate::services::import_service::now_rfc3339()?;
        repo.update_block_status(&block.id, ProofreadingStatus::Running, &started_at)?;

        let timer = Instant::now();
        let sanitized = sanitize_model_input(&block.text_content);
        let payload = RequestPayload {
            block_id: &block.id,
            text: &sanitized.text,
            rules: build_rules(&self.issue_types),
        };
        let request_body = build_model_request_body(&self.settings, &payload)?;
        let request_json = serde_json::to_string(&request_body)?;

        match &self.client {
            Some(client) => {
                self.handle_remote(
                    &repo,
                    &block,
                    &sanitized,
                    client,
                    timer,
                    started_at,
                    request_json,
                )
                .await
            }
            None => self.handle_skipped(&repo, &block, started_at, request_json),
        }
    }

    /// 真实调用模型的分支。
    async fn handle_remote(
        &self,
        repo: &ProofreadingRepository,
        block: &DocumentBlock,
        sanitized: &SanitizedText,
        client: &Client,
        timer: Instant,
        started_at: String,
        request_json: String,
    ) -> AppResult<()> {
        let request_body = serde_json::from_str(&request_json)?;
        match call_model(client, &request_body, &self.settings).await {
            Ok(response) => {
                let finished_at = crate::services::import_service::now_rfc3339()?;
                let latency_ms = timer.elapsed().as_millis() as i64;
                let response_json = serde_json::to_string(&response)?;

                // 日志会同时写入文件和数据库。
                let _ = append_model_log(
                    &self.project_id,
                    &self.job_id,
                    &block.id,
                    &request_json,
                    Some(&response_json),
                    None,
                );
                match to_proofreading_issues(
                    &self.project_id,
                    &self.job_id,
                    &block.id,
                    &block.text_content,
                    sanitized,
                    &response.message.content,
                    &finished_at,
                ) {
                    Ok(issues) => {
                        repo.insert_call(&NewCallRecord {
                            id: Uuid::new_v4().to_string(),
                            job_id: self.job_id.clone(),
                            project_id: self.project_id.clone(),
                            block_id: block.id.clone(),
                            model_name: display_model_name(&self.settings),
                            base_url: self.settings.base_url.clone(),
                            request_json,
                            response_json: Some(response_json),
                            status: "completed".to_string(),
                            started_at,
                            finished_at: Some(finished_at.clone()),
                            latency_ms: Some(latency_ms),
                            prompt_tokens: response.usage.prompt_tokens,
                            completion_tokens: response.usage.completion_tokens,
                            error_message: None,
                        })?;
                        repo.replace_issues(&self.project_id, &self.job_id, &block.id, &issues)?;
                        repo.update_block_status(
                            &block.id,
                            ProofreadingStatus::Completed,
                            &finished_at,
                        )?;
                        sync_job_progress(&self.db, &self.job_id, &self.project_id)?;
                        Ok(())
                    }
                    Err(error) => {
                        let _ = append_model_log(
                            &self.project_id,
                            &self.job_id,
                            &block.id,
                            &request_json,
                            Some(&response_json),
                            Some(&error.message),
                        );
                        repo.insert_call(&NewCallRecord {
                            id: Uuid::new_v4().to_string(),
                            job_id: self.job_id.clone(),
                            project_id: self.project_id.clone(),
                            block_id: block.id.clone(),
                            model_name: display_model_name(&self.settings),
                            base_url: self.settings.base_url.clone(),
                            request_json,
                            response_json: Some(response_json),
                            status: "failed".to_string(),
                            started_at,
                            finished_at: Some(finished_at.clone()),
                            latency_ms: Some(latency_ms),
                            prompt_tokens: response.usage.prompt_tokens,
                            completion_tokens: response.usage.completion_tokens,
                            error_message: Some(error.message.clone()),
                        })?;
                        repo.update_block_status(
                            &block.id,
                            ProofreadingStatus::Failed,
                            &finished_at,
                        )?;
                        sync_job_progress(&self.db, &self.job_id, &self.project_id)?;
                        Ok(())
                    }
                }
            }
            Err(error) => {
                let finished_at = crate::services::import_service::now_rfc3339()?;
                let latency_ms = timer.elapsed().as_millis() as i64;
                let _ = append_model_log(
                    &self.project_id,
                    &self.job_id,
                    &block.id,
                    &request_json,
                    None,
                    Some(&error.message),
                );
                repo.insert_call(&NewCallRecord {
                    id: Uuid::new_v4().to_string(),
                    job_id: self.job_id.clone(),
                    project_id: self.project_id.clone(),
                    block_id: block.id.clone(),
                    model_name: display_model_name(&self.settings),
                    base_url: self.settings.base_url.clone(),
                    request_json,
                    response_json: None,
                    status: "failed".to_string(),
                    started_at,
                    finished_at: Some(finished_at.clone()),
                    latency_ms: Some(latency_ms),
                    prompt_tokens: None,
                    completion_tokens: None,
                    error_message: Some(error.message.clone()),
                })?;
                repo.update_block_status(&block.id, ProofreadingStatus::Failed, &finished_at)?;
                sync_job_progress(&self.db, &self.job_id, &self.project_id)?;
                Ok(())
            }
        }
    }

    /// 没配完整模型参数时，走演示模式，不做真实调用。
    fn handle_skipped(
        &self,
        repo: &ProofreadingRepository,
        block: &DocumentBlock,
        started_at: String,
        request_json: String,
    ) -> AppResult<()> {
        let finished_at = crate::services::import_service::now_rfc3339()?;
        let response_json = "{\"issues\":[]}".to_string();
        let message = "未配置完整模型参数，当前以演示模式跳过真实调用";

        let _ = append_model_log(
            &self.project_id,
            &self.job_id,
            &block.id,
            &request_json,
            Some(&response_json),
            Some(message),
        );
        repo.insert_call(&NewCallRecord {
            id: Uuid::new_v4().to_string(),
            job_id: self.job_id.clone(),
            project_id: self.project_id.clone(),
            block_id: block.id.clone(),
            model_name: display_model_name(&self.settings),
            base_url: self.settings.base_url.clone(),
            request_json,
            response_json: Some(response_json),
            status: "skipped".to_string(),
            started_at,
            finished_at: Some(finished_at.clone()),
            latency_ms: Some(0),
            prompt_tokens: Some(0),
            completion_tokens: Some(0),
            error_message: Some(message.to_string()),
        })?;
        repo.replace_issues(&self.project_id, &self.job_id, &block.id, &[])?;
        repo.update_block_status(&block.id, ProofreadingStatus::Completed, &finished_at)?;
        sync_job_progress(&self.db, &self.job_id, &self.project_id)?;
        Ok(())
    }

    /// 处理代码内部异常，确保 block 不会卡在 running。
    async fn mark_internal_failure(
        &self,
        block: &DocumentBlock,
        error_message: &str,
    ) -> AppResult<()> {
        let repo = ProofreadingRepository::new(self.db.clone());
        let started_at = crate::services::import_service::now_rfc3339()?;
        let finished_at = crate::services::import_service::now_rfc3339()?;
        let _ = append_model_log(
            &self.project_id,
            &self.job_id,
            &block.id,
            "{}",
            None,
            Some(error_message),
        );
        repo.insert_call(&NewCallRecord {
            id: Uuid::new_v4().to_string(),
            job_id: self.job_id.clone(),
            project_id: self.project_id.clone(),
            block_id: block.id.clone(),
            model_name: display_model_name(&self.settings),
            base_url: self.settings.base_url.clone(),
            request_json: "{}".to_string(),
            response_json: None,
            status: "failed".to_string(),
            started_at,
            finished_at: Some(finished_at.clone()),
            latency_ms: Some(0),
            prompt_tokens: None,
            completion_tokens: None,
            error_message: Some(error_message.to_string()),
        })?;
        repo.update_block_status(&block.id, ProofreadingStatus::Failed, &finished_at)?;
        sync_job_progress(&self.db, &self.job_id, &self.project_id)?;
        Ok(())
    }
}

/// 本地命令行调试用的直接调用入口，不经过 job/worker 调度。
pub async fn debug_call_text(
    settings: &AppSettings,
    block_id: &str,
    text: &str,
    issue_types: &[IssueType],
) -> AppResult<DebugModelCall> {
    if !can_call_model(settings) {
        return Err(AppError::new(
            "missing_model_config",
            "本地测试至少需要填写 Base URL 和 Model，API Key 可为空",
        ));
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_millis(settings.timeout_ms as u64))
        .build()
        .map_err(|error| AppError::new("http_client_error", error.to_string()))?;
    let payload = RequestPayload {
        block_id,
        text,
        rules: build_rules(issue_types),
    };
    let request_body = build_model_request_body(settings, &payload)?;
    let request_json = serde_json::to_string_pretty(&request_body)?;
    let response = call_model(&client, &request_body, settings).await?;

    Ok(DebugModelCall {
        request_json,
        response_json: serde_json::to_string_pretty(&response)?,
        prompt_tokens: response.usage.prompt_tokens.unwrap_or(0),
        completion_tokens: response.usage.completion_tokens.unwrap_or(0),
    })
}

/// 根据设置构造 HTTP 客户端。
fn build_client(settings: &AppSettings) -> AppResult<Option<Client>> {
    if !can_call_model(settings) {
        return Ok(None);
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_millis(settings.timeout_ms as u64))
        .build()
        .map_err(|error| AppError::new("http_client_error", error.to_string()))?;
    Ok(Some(client))
}

/// 调用 chat completions 接口，并解析最小结果结构。
async fn call_model(
    client: &Client,
    request_body: &serde_json::Value,
    settings: &AppSettings,
) -> AppResult<ModelCallResult> {
    let url = format!("{}/chat/completions", settings.base_url.trim_end_matches('/'));
    let request = client.post(url);
    let request = if settings.api_key.trim().is_empty() {
        request
    } else {
        request.bearer_auth(&settings.api_key)
    };
    let response = request
        .json(request_body)
        .send()
        .await
        .map_err(|error| AppError::new("llm_request_error", error.to_string()))?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::new(
            "llm_response_error",
            format!("模型请求失败: {}", body),
        ));
    }

    let parsed: ChatCompletionResponse = response
        .json()
        .await
        .map_err(|error| AppError::new("llm_response_parse_error", error.to_string()))?;
    let choice = parsed
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| AppError::new("empty_model_response", "模型未返回有效结果"))?;

    Ok(ModelCallResult {
        message: choice.message,
        usage: parsed.usage.unwrap_or(Usage {
            prompt_tokens: Some(0),
            completion_tokens: Some(0),
        }),
    })
}

/// 从 job 保存的 JSON 快照恢复任务选项。
/// 这是“关闭程序后还能继续”的关键之一。
fn parse_job_options(job: &ProofreadingJob) -> AppResult<ProofreadOptions> {
    let raw = job
        .options_json
        .as_deref()
        .ok_or_else(|| AppError::new("missing_job_options", "缺少任务参数快照，无法恢复任务"))?;
    serde_json::from_str(raw)
        .map_err(|error| AppError::new("invalid_job_options", error.to_string()))
}

/// job 收尾逻辑：同步 job 与项目两层状态。
fn finalize_job(
    repo: &ProofreadingRepository,
    project_repo: &ProjectRepository,
    job: &mut ProofreadingJob,
    finished_at: &str,
) -> AppResult<()> {
    let metrics = repo.job_metrics(&job.id)?;
    apply_metrics(job, metrics, finished_at);
    repo.update_job(job)?;

    let (completed, failed) = repo.count_block_statuses(&job.project_id)?;
    let project_status = if matches!(job.status, ProofreadingStatus::Failed) {
        ProjectStatus::Failed
    } else {
        ProjectStatus::Completed
    };
    project_repo.update_progress(
        &job.project_id,
        project_status,
        completed,
        failed,
        finished_at,
    )?;
    Ok(())
}

/// 把聚合统计结果回填到 job。
fn apply_metrics(job: &mut ProofreadingJob, metrics: JobMetrics, finished_at: &str) {
    job.completed_blocks = metrics.completed_blocks;
    job.failed_blocks = metrics.failed_blocks;
    job.total_issues = metrics.total_issues;
    job.total_tokens_in = metrics.total_tokens_in;
    job.total_tokens_out = metrics.total_tokens_out;
    job.total_latency_ms = metrics.total_latency_ms;
    let has_unfinished = metrics.pending_blocks > 0 || metrics.running_blocks > 0;
    job.status = if has_unfinished {
        ProofreadingStatus::Failed
    } else if metrics.failed_blocks > 0 && metrics.completed_blocks == 0 {
        ProofreadingStatus::Failed
    } else {
        ProofreadingStatus::Completed
    };
    job.finished_at = Some(finished_at.to_string());
}

fn sync_job_progress(db: &Database, job_id: &str, project_id: &str) -> AppResult<()> {
    let repo = ProofreadingRepository::new(db.clone());
    let project_repo = ProjectRepository::new(db.clone());
    let Some(mut job) = repo.get_job(job_id)? else {
        return Ok(());
    };
    let metrics = repo.job_metrics(job_id)?;
    job.completed_blocks = metrics.completed_blocks;
    job.failed_blocks = metrics.failed_blocks;
    job.total_issues = metrics.total_issues;
    job.total_tokens_in = metrics.total_tokens_in;
    job.total_tokens_out = metrics.total_tokens_out;
    job.total_latency_ms = metrics.total_latency_ms;
    repo.update_job(&job)?;

    let (completed, failed) = repo.count_block_statuses(project_id)?;
    let now = crate::services::import_service::now_rfc3339()?;
    project_repo.update_progress(project_id, ProjectStatus::Processing, completed, failed, &now)?;
    Ok(())
}

fn current_job_status(
    repo: &ProofreadingRepository,
    job_id: &str,
) -> AppResult<ProofreadingStatus> {
    repo.get_job(job_id)?
        .map(|job| job.status)
        .ok_or_else(|| AppError::new("job_not_found", "未找到对应校对任务"))
}

/// 组装发给模型的请求体。
/// 输出约束主要通过提示词完成，以兼容更多 OpenAI-compatible 接口。
fn build_model_request_body(
    settings: &AppSettings,
    request_payload: &RequestPayload<'_>,
) -> AppResult<serde_json::Value> {
    Ok(json!({
        "model": settings.model,
        "temperature": settings.temperature,
        "max_tokens": settings.max_tokens,
        "messages": [
            {
                "role": "system",
                "content": build_system_prompt(settings)
            },
            {
                "role": "user",
                "content": serde_json::to_string(request_payload)?
            }
        ]
    }))
}

fn build_system_prompt(settings: &AppSettings) -> String {
    format!(
        "{}\n\n返回要求：\n1. 只返回 JSON，不要输出解释、Markdown、代码块。\n2. JSON 顶层必须是对象，结构为 {{\"issues\": [...]}}。\n3. 如果没有问题，返回 {{\"issues\": []}}。\n4. 每个 issue 必须包含字段：type、severity、start_offset、end_offset、quote、suggestion、explanation、normalized_replacement。\n5. normalized_replacement 没有明确替换值时返回 null。\n\n返回示例：\n{{\"issues\":[{{\"type\":\"typo\",\"severity\":\"medium\",\"start_offset\":12,\"end_offset\":14,\"quote\":\"示列\",\"suggestion\":\"示例\",\"explanation\":\"存在错别字，应改为“示例”\",\"normalized_replacement\":\"示例\"}}]}}",
        settings.system_prompt_template.trim()
    )
}

/// 把 prompt / response / error 追加写入当天日志文件。
fn append_model_log(
    project_id: &str,
    job_id: &str,
    block_id: &str,
    request_json: &str,
    response_json: Option<&str>,
    error_message: Option<&str>,
) -> AppResult<()> {
    let path = log_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let timestamp = crate::services::import_service::now_rfc3339()?;
    let line = json!({
        "timestamp": timestamp,
        "projectId": project_id,
        "jobId": job_id,
        "blockId": block_id,
        "request": request_json,
        "response": response_json,
        "error": error_message,
    });
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

/// 生成当天日志文件路径。
fn log_file_path() -> AppResult<PathBuf> {
    let current_dir = std::env::current_dir()
        .map_err(|error| AppError::new("log_path_error", error.to_string()))?;
    let date = time::OffsetDateTime::now_utc().date();
    Ok(current_dir.join("logs").join(format!("{date}.log")))
}

/// 把模型返回的 JSON 结构转换成数据库里的问题对象。
fn to_proofreading_issues(
    project_id: &str,
    job_id: &str,
    block_id: &str,
    text: &str,
    sanitized: &SanitizedText,
    raw_content: &str,
    created_at: &str,
) -> AppResult<Vec<ProofreadingIssue>> {
    let payload = parse_model_payload(raw_content)?;
    let issues = payload
        .issues
        .into_iter()
        .filter_map(|issue| {
            let (start_offset, end_offset) = map_issue_offsets(sanitized, text, &issue)?;
            let prefix = text_prefix(text, start_offset);
            let suffix = text_suffix(text, end_offset);

            Some(ProofreadingIssue {
                id: Uuid::new_v4().to_string(),
                project_id: project_id.to_string(),
                job_id: job_id.to_string(),
                block_id: block_id.to_string(),
                issue_type: parse_issue_type(&issue.issue_type),
                severity: parse_severity(&issue.severity),
                start_offset,
                end_offset,
                quote_text: issue.quote,
                prefix_text: prefix,
                suffix_text: suffix,
                suggestion: issue.suggestion,
                explanation: issue.explanation,
                normalized_replacement: issue.normalized_replacement,
                status: IssueStatus::Open,
                created_at: created_at.to_string(),
            })
        })
        .collect();

    Ok(issues)
}

fn map_issue_offsets(
    sanitized: &SanitizedText,
    original_text: &str,
    issue: &ModelIssue,
) -> Option<(i64, i64)> {
    let original_len = original_text.chars().count();
    let start = map_offset_to_original(&sanitized.char_map, issue.start_offset, original_len)?;
    let end = map_offset_to_original(&sanitized.char_map, issue.end_offset, original_len)?;
    if start >= end {
        return None;
    }
    Some((start as i64, end as i64))
}

fn map_offset_to_original(char_map: &[usize], offset: i64, original_len: usize) -> Option<usize> {
    let offset = offset.max(0) as usize;
    if offset == 0 {
        return Some(0);
    }
    if char_map.is_empty() {
        return Some(original_len);
    }
    if offset >= char_map.len() {
        return char_map.last().map(|index| index + 1);
    }
    Some(char_map[offset - 1] + 1)
}

/// 模型有时会返回带 Markdown 包裹的 JSON，这里先做一层容错解析。
fn parse_model_payload(raw_content: &str) -> AppResult<ModelPayload> {
    if raw_content.trim().is_empty() {
        return Err(AppError::new(
            "empty_model_content",
            "模型返回空内容，无法解析校对结果",
        ));
    }
    let content = extract_json_object(raw_content);
    serde_json::from_str(&content)
        .map_err(|error| AppError::new("model_payload_error", error.to_string()))
}

/// 尽量从任意文本中截出一个 JSON object。
fn extract_json_object(raw: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
        return value.to_string();
    }

    let trimmed = raw.trim();
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            return trimmed[start..=end].to_string();
        }
    }

    raw.to_string()
}

/// 把前端勾选的问题类型转成提示词规则。
fn build_rules(issue_types: &[IssueType]) -> Vec<String> {
    let labels = issue_types
        .iter()
        .map(|item| match item {
            IssueType::Typo => "错别字",
            IssueType::Punctuation => "标点符号错误",
            IssueType::Grammar => "语法不通顺",
            IssueType::Wording => "用词不当",
            IssueType::Redundancy => "重复啰嗦",
            IssueType::Consistency => "术语前后不一致",
        })
        .collect::<Vec<_>>()
        .join("、");

    vec![
        "只找校对问题，不改写整体风格".to_string(),
        "若无问题，返回 {\"issues\":[]}".to_string(),
        format!("重点检查：{}", labels),
        "忽略标点问题、PDF 换行、分页断行、分隔线、空白字符导致的版式噪音".to_string(),
        "不要把“于\\n是”“一\\n种”“句号后换行”这类提取断行当作问题".to_string(),
        "每项必须返回 type、severity、start_offset、end_offset、quote、suggestion、explanation".to_string(),
    ]
}

/// 按模式筛选要处理的 block 集合。
fn select_blocks(
    blocks: &[crate::types::DocumentBlock],
    mode: ProofreadingMode,
    skip_pages: i64,
) -> AppResult<Vec<crate::types::DocumentBlock>> {
    let skip_until_page = skip_pages.max(0) + 1;
    let selected = match mode {
        ProofreadingMode::RetryFailed => blocks
            .iter()
            .filter(|block| should_proofread_block(block, skip_until_page))
            .filter(|block| matches!(block.proofreading_status, ProofreadingStatus::Failed))
            .cloned()
            .collect::<Vec<_>>(),
        _ => blocks
            .iter()
            .filter(|block| should_proofread_block(block, skip_until_page))
            .cloned()
            .collect::<Vec<_>>(),
    };

    if matches!(mode, ProofreadingMode::RetryFailed) && selected.is_empty() {
        return Err(AppError::new("no_failed_blocks", "当前没有可重试的失败块"));
    }

    Ok(selected)
}

/// 在模式筛选后，再只保留 `pending` block。
fn select_pending_blocks(
    blocks: &[crate::types::DocumentBlock],
    mode: ProofreadingMode,
    skip_pages: i64,
) -> AppResult<Vec<crate::types::DocumentBlock>> {
    let selected = select_blocks(blocks, mode, skip_pages)?;
    Ok(selected
        .into_iter()
        .filter(|block| matches!(block.proofreading_status, ProofreadingStatus::Pending))
        .collect())
}

/// 判断一个 block 是否应该进入本次校对范围。
///
/// `skip_until_page = 1` 表示跳过第 1 页，从第 2 页开始校对。
/// 没有页码的 block 默认保留，避免影响 DOCX 或缺页码数据。
fn should_proofread_block(block: &crate::types::DocumentBlock, skip_until_page: i64) -> bool {
    match block.source_page {
        Some(page) => page >= skip_until_page,
        None => true,
    }
}

/// 对送给模型的文本做轻量清洗，并记录清洗后字符到原文字符的映射。
fn sanitize_model_input(text: &str) -> SanitizedText {
    let chars = text.chars().collect::<Vec<_>>();
    let mut output = String::with_capacity(text.len());
    let mut char_map = Vec::with_capacity(chars.len());

    for (index, current) in chars.iter().enumerate() {
        let previous = if index > 0 { Some(chars[index - 1]) } else { None };
        let next = chars.get(index + 1).copied();
        if should_drop_inline_gap(*current, previous, next) || is_separator_char(*current) {
            continue;
        }
        if *current == '\n' && is_inline_line_break(previous, next) {
            output.push(' ');
            char_map.push(index);
            continue;
        }
        output.push(*current);
        char_map.push(index);
    }

    SanitizedText { text: output, char_map }
}

fn is_inline_line_break(previous: Option<char>, next: Option<char>) -> bool {
    match (previous, next) {
        (Some(left), Some(right)) => is_cjk_or_word(left) && is_cjk_or_word(right),
        _ => false,
    }
}

fn should_drop_inline_gap(
    current: char,
    previous: Option<char>,
    next: Option<char>,
) -> bool {
    current.is_whitespace() && matches!((previous, next), (Some(left), Some(right)) if is_cjk_or_word(left) && is_cjk_or_word(right))
}

fn is_cjk_or_word(value: char) -> bool {
    value.is_ascii_alphanumeric() || is_cjk(value)
}

fn is_cjk(value: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&value)
        || ('\u{3400}'..='\u{4DBF}').contains(&value)
        || ('\u{F900}'..='\u{FAFF}').contains(&value)
}

fn is_separator_char(value: char) -> bool {
    matches!(value, '-' | '—' | '－' | '_' | '─' | '━')
}

/// 判断是否具备真实调模型的最低条件。
fn can_call_model(settings: &AppSettings) -> bool {
    !settings.base_url.trim().is_empty() && !settings.model.trim().is_empty()
}

/// 展示给 UI 或日志时使用的模型名。
fn display_model_name(settings: &AppSettings) -> String {
    if settings.model.trim().is_empty() {
        "demo-skip".to_string()
    } else {
        settings.model.clone()
    }
}

/// 下面这些函数负责把模型的字符串值映射回内部枚举。
fn parse_issue_type(value: &str) -> IssueType {
    match value {
        "punctuation" => IssueType::Punctuation,
        "grammar" => IssueType::Grammar,
        "wording" => IssueType::Wording,
        "redundancy" => IssueType::Redundancy,
        "consistency" => IssueType::Consistency,
        _ => IssueType::Typo,
    }
}

fn parse_severity(value: &str) -> IssueSeverity {
    match value {
        "high" => IssueSeverity::High,
        "medium" => IssueSeverity::Medium,
        _ => IssueSeverity::Low,
    }
}

/// 截取问题前面的少量上下文，供前端展示。
fn text_prefix(text: &str, start_offset: i64) -> Option<String> {
    let start = start_offset.max(0) as usize;
    let chars = text.chars().collect::<Vec<_>>();
    let end = start.min(chars.len());
    let from = end.saturating_sub(8);
    let prefix = chars[from..end].iter().collect::<String>();
    if prefix.is_empty() { None } else { Some(prefix) }
}

/// 截取问题后面的少量上下文。
fn text_suffix(text: &str, end_offset: i64) -> Option<String> {
    let end = end_offset.max(0) as usize;
    let chars = text.chars().collect::<Vec<_>>();
    let suffix = chars[end.min(chars.len())..(end + 8).min(chars.len())]
        .iter()
        .collect::<String>();
    if suffix.is_empty() { None } else { Some(suffix) }
}

#[cfg(test)]
mod tests {
    use super::{can_call_model, extract_json_object, parse_model_payload};
    use crate::types::AppSettings;

    #[test]
    fn should_extract_json_from_fenced_block() {
        let raw = "```json\n{\"issues\":[]}\n```";
        assert_eq!(extract_json_object(raw), "{\"issues\":[]}");
    }

    #[test]
    fn should_parse_payload() {
        let parsed = parse_model_payload("{\"issues\":[]}").unwrap();
        assert_eq!(parsed.issues.len(), 0);
    }

    #[test]
    fn should_allow_model_call_when_settings_complete() {
        let settings = AppSettings {
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "sk-test".to_string(),
            model: "gpt-4.1-mini".to_string(),
            timeout_ms: 60_000,
            max_concurrency: 4,
            pdf_min_block_chars: 16,
            temperature: 0.2,
            max_tokens: 1200,
            system_prompt_template: "test".to_string(),
        };

        assert!(can_call_model(&settings));
    }

    #[test]
    fn should_allow_model_call_when_api_key_missing() {
        let settings = AppSettings {
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "".to_string(),
            model: "gpt-4.1-mini".to_string(),
            timeout_ms: 60_000,
            max_concurrency: 4,
            pdf_min_block_chars: 16,
            temperature: 0.2,
            max_tokens: 1200,
            system_prompt_template: "test".to_string(),
        };

        assert!(can_call_model(&settings));
    }
}
