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

    pub fn start_job(
        &self,
        project_id: &str,
        options: ProofreadOptions,
    ) -> AppResult<ProofreadingJob> {
        let repo = ProofreadingRepository::new(self.db.clone());
        let project_repo = ProjectRepository::new(self.db.clone());
        if let Some(job) = repo.get_running_job(project_id)? {
            return Ok(job);
        }
        let blocks = select_blocks(&repo.list_blocks(project_id)?, options.mode)?;
        if blocks.is_empty() {
            return Err(AppError::new("empty_document", "当前项目没有可校对的正文块"));
        }

        let now = crate::services::import_service::now_rfc3339()?;
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

    pub async fn run_job(&self, mut job: ProofreadingJob) -> AppResult<()> {
        let settings = AppSettingsRepository::new(self.db.clone()).get()?;
        let repo = ProofreadingRepository::new(self.db.clone());
        let project_repo = ProjectRepository::new(self.db.clone());
        let options = parse_job_options(&job)?;
        let now = crate::services::import_service::now_rfc3339()?;
        repo.reset_running_blocks(&job.project_id, &now)?;

        let blocks = select_pending_blocks(&repo.list_blocks(&job.project_id)?, options.mode)?;
        if blocks.is_empty() {
            let finished_at = crate::services::import_service::now_rfc3339()?;
            finalize_job(&repo, &project_repo, &mut job, &finished_at)?;
            return Ok(());
        }

        let client = build_client(&settings)?;
        let queue = Arc::new(Mutex::new(VecDeque::from(blocks)));
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

        let finished_at = crate::services::import_service::now_rfc3339()?;
        finalize_job(&repo, &project_repo, &mut job, &finished_at)?;
        Ok(())
    }

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
    async fn run(&self) {
        loop {
            let block = self.next_block().await;
            let Some(block) = block else {
                return;
            };
            if let Err(error) = self.process_block(block.clone()).await {
                let _ = self.mark_internal_failure(&block, &error.message).await;
            }
        }
    }

    async fn next_block(&self) -> Option<DocumentBlock> {
        let mut queue = self.queue.lock().await;
        queue.pop_front()
    }

    async fn process_block(&self, block: DocumentBlock) -> AppResult<()> {
        let repo = ProofreadingRepository::new(self.db.clone());
        let started_at = crate::services::import_service::now_rfc3339()?;
        repo.update_block_status(&block.id, ProofreadingStatus::Running, &started_at)?;

        let timer = Instant::now();
        let payload = RequestPayload {
            block_id: &block.id,
            text: &block.text_content,
            rules: build_rules(&self.issue_types),
        };
        let request_body = build_model_request_body(&self.settings, &payload)?;
        let request_json = serde_json::to_string(&request_body)?;

        match &self.client {
            Some(client) => {
                self.handle_remote(&repo, &block, client, timer, started_at, request_json)
                    .await
            }
            None => self.handle_skipped(&repo, &block, started_at, request_json),
        }
    }

    async fn handle_remote(
        &self,
        repo: &ProofreadingRepository,
        block: &DocumentBlock,
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
                let issues = to_proofreading_issues(
                    &self.project_id,
                    &self.job_id,
                    &block.id,
                    &block.text_content,
                    &response.message.content,
                    &finished_at,
                )?;

                let _ = append_model_log(
                    &self.project_id,
                    &self.job_id,
                    &block.id,
                    &request_json,
                    Some(&response_json),
                    None,
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
                    status: "completed".to_string(),
                    started_at,
                    finished_at: Some(finished_at.clone()),
                    latency_ms: Some(latency_ms),
                    prompt_tokens: response.usage.prompt_tokens,
                    completion_tokens: response.usage.completion_tokens,
                    error_message: None,
                })?;
                repo.replace_issues(&self.project_id, &self.job_id, &block.id, &issues)?;
                repo.update_block_status(&block.id, ProofreadingStatus::Completed, &finished_at)?;
                Ok(())
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
                Ok(())
            }
        }
    }

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
        Ok(())
    }

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
        Ok(())
    }
}

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

fn parse_job_options(job: &ProofreadingJob) -> AppResult<ProofreadOptions> {
    let raw = job
        .options_json
        .as_deref()
        .ok_or_else(|| AppError::new("missing_job_options", "缺少任务参数快照，无法恢复任务"))?;
    serde_json::from_str(raw)
        .map_err(|error| AppError::new("invalid_job_options", error.to_string()))
}

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

fn build_model_request_body(
    settings: &AppSettings,
    request_payload: &RequestPayload<'_>,
) -> AppResult<serde_json::Value> {
    Ok(json!({
        "model": settings.model,
        "temperature": settings.temperature,
        "max_tokens": settings.max_tokens,
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "proofreading_issues",
                "schema": {
                    "type": "object",
                    "properties": {
                        "issues": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "type": { "type": "string" },
                                    "severity": { "type": "string" },
                                    "start_offset": { "type": "integer" },
                                    "end_offset": { "type": "integer" },
                                    "quote": { "type": "string" },
                                    "suggestion": { "type": "string" },
                                    "explanation": { "type": "string" },
                                    "normalized_replacement": {
                                        "type": ["string", "null"]
                                    }
                                },
                                "required": [
                                    "type",
                                    "severity",
                                    "start_offset",
                                    "end_offset",
                                    "quote",
                                    "suggestion",
                                    "explanation",
                                    "normalized_replacement"
                                ],
                                "additionalProperties": false
                            }
                        }
                    },
                    "required": ["issues"],
                    "additionalProperties": false
                }
            }
        },
        "messages": [
            {
                "role": "system",
                "content": settings.system_prompt_template
            },
            {
                "role": "user",
                "content": serde_json::to_string(request_payload)?
            }
        ]
    }))
}

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

fn log_file_path() -> AppResult<PathBuf> {
    let current_dir = std::env::current_dir()
        .map_err(|error| AppError::new("log_path_error", error.to_string()))?;
    let date = time::OffsetDateTime::now_utc().date();
    Ok(current_dir.join("logs").join(format!("{date}.log")))
}

fn to_proofreading_issues(
    project_id: &str,
    job_id: &str,
    block_id: &str,
    text: &str,
    raw_content: &str,
    created_at: &str,
) -> AppResult<Vec<ProofreadingIssue>> {
    let payload = parse_model_payload(raw_content)?;
    let issues = payload
        .issues
        .into_iter()
        .map(|issue| {
            let prefix = text_prefix(text, issue.start_offset);
            let suffix = text_suffix(text, issue.end_offset);

            ProofreadingIssue {
                id: Uuid::new_v4().to_string(),
                project_id: project_id.to_string(),
                job_id: job_id.to_string(),
                block_id: block_id.to_string(),
                issue_type: parse_issue_type(&issue.issue_type),
                severity: parse_severity(&issue.severity),
                start_offset: issue.start_offset,
                end_offset: issue.end_offset,
                quote_text: issue.quote,
                prefix_text: prefix,
                suffix_text: suffix,
                suggestion: issue.suggestion,
                explanation: issue.explanation,
                normalized_replacement: issue.normalized_replacement,
                status: IssueStatus::Open,
                created_at: created_at.to_string(),
            }
        })
        .collect();

    Ok(issues)
}

fn parse_model_payload(raw_content: &str) -> AppResult<ModelPayload> {
    let content = extract_json_object(raw_content);
    serde_json::from_str(&content)
        .map_err(|error| AppError::new("model_payload_error", error.to_string()))
}

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
        "每项必须返回 type、severity、start_offset、end_offset、quote、suggestion、explanation".to_string(),
    ]
}

fn select_blocks(
    blocks: &[crate::types::DocumentBlock],
    mode: ProofreadingMode,
) -> AppResult<Vec<crate::types::DocumentBlock>> {
    let selected = match mode {
        ProofreadingMode::RetryFailed => blocks
            .iter()
            .filter(|block| matches!(block.proofreading_status, ProofreadingStatus::Failed))
            .cloned()
            .collect::<Vec<_>>(),
        _ => blocks.to_vec(),
    };

    if matches!(mode, ProofreadingMode::RetryFailed) && selected.is_empty() {
        return Err(AppError::new("no_failed_blocks", "当前没有可重试的失败块"));
    }

    Ok(selected)
}

fn select_pending_blocks(
    blocks: &[crate::types::DocumentBlock],
    mode: ProofreadingMode,
) -> AppResult<Vec<crate::types::DocumentBlock>> {
    let selected = select_blocks(blocks, mode)?;
    Ok(selected
        .into_iter()
        .filter(|block| matches!(block.proofreading_status, ProofreadingStatus::Pending))
        .collect())
}

fn can_call_model(settings: &AppSettings) -> bool {
    !settings.base_url.trim().is_empty() && !settings.model.trim().is_empty()
}

fn display_model_name(settings: &AppSettings) -> String {
    if settings.model.trim().is_empty() {
        "demo-skip".to_string()
    } else {
        settings.model.clone()
    }
}

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

fn text_prefix(text: &str, start_offset: i64) -> Option<String> {
    let start = start_offset.max(0) as usize;
    let chars = text.chars().collect::<Vec<_>>();
    let end = start.min(chars.len());
    let from = end.saturating_sub(8);
    let prefix = chars[from..end].iter().collect::<String>();
    if prefix.is_empty() { None } else { Some(prefix) }
}

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
