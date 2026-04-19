//! 校对仓储。
//!
//! 这是数据库层里最核心的一块：
//! - job 状态
//! - block 状态
//! - 模型调用记录
//! - 问题列表
//! 都通过这里读写。

use rusqlite::{params, OptionalExtension};

use crate::db::Database;
use crate::error::AppResult;
use crate::types::{
    DocumentBlock, IssueSeverity, IssueStatus, IssueType, ProofreadingCall, ProofreadingIssue,
    ProofreadingJob, ProofreadingMode, ProofreadingStatus,
};

#[derive(Debug, Clone)]
pub struct ProofreadingRepository {
    db: Database,
}

/// 写入 `proofreading_calls` 表时使用的参数对象。
/// 这样可以避免 `insert_call` 带上一长串位置参数。
#[derive(Debug, Clone)]
pub struct NewCallRecord {
    pub id: String,
    pub job_id: String,
    pub project_id: String,
    pub block_id: String,
    pub model_name: String,
    pub base_url: String,
    pub request_json: String,
    pub response_json: Option<String>,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub latency_ms: Option<i64>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub error_message: Option<String>,
}

/// job 收尾时用到的聚合统计结果。
#[derive(Debug, Clone, Copy)]
pub struct JobMetrics {
    pub pending_blocks: i64,
    pub running_blocks: i64,
    pub completed_blocks: i64,
    pub failed_blocks: i64,
    pub total_issues: i64,
    pub total_tokens_in: i64,
    pub total_tokens_out: i64,
    pub total_latency_ms: i64,
}

impl ProofreadingRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 新建一条 job 记录。
    pub fn create_job(&self, job: &ProofreadingJob) -> AppResult<()> {
        let conn = self.db.connect()?;
        conn.execute(
            r#"
            INSERT INTO proofreading_jobs (
              id, project_id, mode, status, options_json, auto_resume, started_at, finished_at,
              total_blocks, completed_blocks, failed_blocks, total_issues, total_tokens_in,
              total_tokens_out, total_latency_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
            params![
                job.id,
                job.project_id,
                mode_name(job.mode),
                status_name(job.status),
                job.options_json,
                i64::from(job.auto_resume),
                job.started_at,
                job.finished_at,
                job.total_blocks,
                job.completed_blocks,
                job.failed_blocks,
                job.total_issues,
                job.total_tokens_in,
                job.total_tokens_out,
                job.total_latency_ms,
            ],
        )?;
        Ok(())
    }

    /// 列出所有允许自动恢复的 running job。
    pub fn list_resumable_jobs(&self) -> AppResult<Vec<ProofreadingJob>> {
        let conn = self.db.connect()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, project_id, mode, status, options_json, auto_resume, started_at, finished_at,
                   total_blocks, completed_blocks, failed_blocks, total_issues, total_tokens_in,
                   total_tokens_out, total_latency_ms
            FROM proofreading_jobs
            WHERE status = 'running' AND auto_resume = 1
            ORDER BY started_at DESC
            "#,
        )?;
        let rows = stmt.query_map([], map_job_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// 查询某个项目当前是否有运行中的 job。
    pub fn get_running_job(&self, project_id: &str) -> AppResult<Option<ProofreadingJob>> {
        let conn = self.db.connect()?;
        conn.query_row(
            r#"
            SELECT id, project_id, mode, status, options_json, auto_resume, started_at, finished_at,
                   total_blocks, completed_blocks, failed_blocks, total_issues, total_tokens_in,
                   total_tokens_out, total_latency_ms
            FROM proofreading_jobs
            WHERE project_id = ?1 AND status = 'running'
            ORDER BY started_at DESC
            LIMIT 1
            "#,
            [project_id],
            map_job_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_job(&self, job_id: &str) -> AppResult<Option<ProofreadingJob>> {
        let conn = self.db.connect()?;
        conn.query_row(
            r#"
            SELECT id, project_id, mode, status, options_json, auto_resume, started_at, finished_at,
                   total_blocks, completed_blocks, failed_blocks, total_issues, total_tokens_in,
                   total_tokens_out, total_latency_ms
            FROM proofreading_jobs
            WHERE id = ?1
            LIMIT 1
            "#,
            [job_id],
            map_job_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn project_exists(&self, project_id: &str) -> AppResult<bool> {
        let conn = self.db.connect()?;
        let exists = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
            [project_id],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(exists == 1)
    }

    /// 更新 job 的聚合状态和统计数字。
    pub fn update_job(&self, job: &ProofreadingJob) -> AppResult<()> {
        let conn = self.db.connect()?;
        conn.execute(
            r#"
            UPDATE proofreading_jobs
            SET status = ?2,
                finished_at = ?3,
                completed_blocks = ?4,
                failed_blocks = ?5,
                total_issues = ?6,
                total_tokens_in = ?7,
                total_tokens_out = ?8,
                total_latency_ms = ?9
            WHERE id = ?1
            "#,
            params![
                job.id,
                status_name(job.status),
                job.finished_at,
                job.completed_blocks,
                job.failed_blocks,
                job.total_issues,
                job.total_tokens_in,
                job.total_tokens_out,
                job.total_latency_ms,
            ],
        )?;
        Ok(())
    }

    /// 读取某个项目的全部 block。
    pub fn list_blocks(&self, project_id: &str) -> AppResult<Vec<DocumentBlock>> {
        let conn = self.db.connect()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, project_id, block_index, type, text_content, json_payload, source_page,
                   source_locator, char_count, proofreading_status, updated_at
            FROM document_blocks
            WHERE project_id = ?1
            ORDER BY block_index ASC
            "#,
        )?;

        let rows = stmt.query_map([project_id], |row| {
            Ok(DocumentBlock {
                id: row.get(0)?,
                project_id: row.get(1)?,
                block_index: row.get(2)?,
                block_type: parse_block_type(&row.get::<_, String>(3)?),
                text_content: row.get(4)?,
                json_payload: row.get(5)?,
                source_page: row.get(6)?,
                source_locator: row.get(7)?,
                char_count: row.get(8)?,
                proofreading_status: parse_status(&row.get::<_, String>(9)?),
                updated_at: row.get(10)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// 聚合项目维度的 completed/failed 数量。
    pub fn count_block_statuses(&self, project_id: &str) -> AppResult<(i64, i64)> {
        let conn = self.db.connect()?;
        let completed = conn.query_row(
            "SELECT COUNT(1) FROM document_blocks WHERE project_id = ?1 AND proofreading_status = 'completed'",
            [project_id],
            |row| row.get(0),
        )?;
        let failed = conn.query_row(
            "SELECT COUNT(1) FROM document_blocks WHERE project_id = ?1 AND proofreading_status = 'failed'",
            [project_id],
            |row| row.get(0),
        )?;
        Ok((completed, failed))
    }

    /// 恢复任务前，把遗留在 `running` 的 block 回退到 `pending`。
    pub fn reset_running_blocks(&self, project_id: &str, updated_at: &str) -> AppResult<()> {
        let conn = self.db.connect()?;
        conn.execute(
            r#"
            UPDATE document_blocks
            SET proofreading_status = 'pending', updated_at = ?2
            WHERE project_id = ?1 AND proofreading_status = 'running'
            "#,
            params![project_id, updated_at],
        )?;
        Ok(())
    }

    /// 启动新 job 时，把本次要处理的 block 重置成 `pending`。
    ///
    /// - `RetryFailed` 只处理失败块
    /// - 其他模式处理整个项目
    pub fn reset_selected_blocks(
        &self,
        project_id: &str,
        mode: ProofreadingMode,
        updated_at: &str,
    ) -> AppResult<i64> {
        let conn = self.db.connect()?;
        let changed = match mode {
            ProofreadingMode::RetryFailed => conn.execute(
                r#"
                UPDATE document_blocks
                SET proofreading_status = 'pending', updated_at = ?2
                WHERE project_id = ?1 AND proofreading_status = 'failed'
                "#,
                params![project_id, updated_at],
            )?,
            _ => conn.execute(
                r#"
                UPDATE document_blocks
                SET proofreading_status = 'pending', updated_at = ?2
                WHERE project_id = ?1
                "#,
                params![project_id, updated_at],
            )?,
        };
        Ok(changed as i64)
    }

    /// 更新单个 block 的执行状态。
    pub fn update_block_status(
        &self,
        block_id: &str,
        status: ProofreadingStatus,
        updated_at: &str,
    ) -> AppResult<()> {
        let conn = self.db.connect()?;
        conn.execute(
            "UPDATE document_blocks SET proofreading_status = ?2, updated_at = ?3 WHERE id = ?1",
            params![block_id, status_name(status), updated_at],
        )?;
        Ok(())
    }

    /// 写入一次模型调用记录。
    pub fn insert_call(&self, record: &NewCallRecord) -> AppResult<()> {
        let conn = self.db.connect()?;
        conn.execute(
            r#"
            INSERT INTO proofreading_calls (
              id, job_id, project_id, block_id, model_name, base_url, request_json, response_json,
              status, started_at, finished_at, latency_ms, prompt_tokens, completion_tokens,
              error_message
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
            params![
                record.id,
                record.job_id,
                record.project_id,
                record.block_id,
                record.model_name,
                record.base_url,
                record.request_json,
                record.response_json,
                record.status,
                record.started_at,
                record.finished_at,
                record.latency_ms,
                record.prompt_tokens,
                record.completion_tokens,
                record.error_message,
            ],
        )?;
        Ok(())
    }

    /// 聚合一个 job 的统计信息。
    ///
    /// 这里既统计 block 当前状态，也统计 calls/issues 的数量和 token。
    pub fn job_metrics(&self, job_id: &str) -> AppResult<JobMetrics> {
        let conn = self.db.connect()?;
        let base = conn.query_row(
            r#"
            SELECT
              COALESCE(SUM(CASE WHEN proofreading_status = 'pending' THEN 1 ELSE 0 END), 0),
              COALESCE(SUM(CASE WHEN proofreading_status = 'running' THEN 1 ELSE 0 END), 0),
              COALESCE(SUM(CASE WHEN status IN ('completed', 'skipped') THEN 1 ELSE 0 END), 0),
              COALESCE(SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END), 0),
              COALESCE(SUM(prompt_tokens), 0),
              COALESCE(SUM(completion_tokens), 0),
              COALESCE(SUM(latency_ms), 0)
            FROM document_blocks
            LEFT JOIN proofreading_calls
              ON proofreading_calls.block_id = document_blocks.id
             AND proofreading_calls.job_id = ?1
            WHERE document_blocks.project_id = (
              SELECT project_id FROM proofreading_jobs WHERE id = ?1
            )
            "#,
            [job_id],
            |row| {
                Ok(JobMetrics {
                    pending_blocks: row.get(0)?,
                    running_blocks: row.get(1)?,
                    completed_blocks: row.get(2)?,
                    failed_blocks: row.get(3)?,
                    total_issues: 0,
                    total_tokens_in: row.get(4)?,
                    total_tokens_out: row.get(5)?,
                    total_latency_ms: row.get(6)?,
                })
            },
        )?;
        let total_issues = conn.query_row(
            "SELECT COUNT(1) FROM proofreading_issues WHERE job_id = ?1",
            [job_id],
            |row| row.get(0),
        )?;

        Ok(JobMetrics {
            total_issues,
            ..base
        })
    }

    /// 用最新结果覆盖某个 block 的问题列表。
    ///
    /// 一次 block 校对结果天然就是全量快照，所以这里先删后插。
    pub fn replace_issues(
        &self,
        project_id: &str,
        job_id: &str,
        block_id: &str,
        issues: &[ProofreadingIssue],
    ) -> AppResult<()> {
        let mut conn = self.db.connect()?;
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM proofreading_issues WHERE project_id = ?1 AND block_id = ?2",
            params![project_id, block_id],
        )?;

        for issue in issues {
            tx.execute(
                r#"
                INSERT INTO proofreading_issues (
                  id, project_id, job_id, block_id, issue_type, severity, start_offset, end_offset,
                  quote_text, prefix_text, suffix_text, suggestion, explanation,
                  normalized_replacement, status, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                "#,
                params![
                    issue.id,
                    project_id,
                    job_id,
                    block_id,
                    issue_type_name(issue.issue_type),
                    severity_name(issue.severity),
                    issue.start_offset,
                    issue.end_offset,
                    issue.quote_text,
                    issue.prefix_text,
                    issue.suffix_text,
                    issue.suggestion,
                    issue.explanation,
                    issue.normalized_replacement,
                    issue_status_name(issue.status),
                    issue.created_at,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// 读取项目的问题列表，供前端问题面板展示。
    pub fn list_issues(&self, project_id: &str) -> AppResult<Vec<ProofreadingIssue>> {
        let conn = self.db.connect()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT proofreading_issues.id,
                   proofreading_issues.project_id,
                   proofreading_issues.job_id,
                   proofreading_issues.block_id,
                   proofreading_issues.issue_type,
                   proofreading_issues.severity,
                   proofreading_issues.start_offset,
                   proofreading_issues.end_offset,
                   proofreading_issues.quote_text,
                   proofreading_issues.prefix_text,
                   proofreading_issues.suffix_text,
                   proofreading_issues.suggestion,
                   proofreading_issues.explanation,
                   proofreading_issues.normalized_replacement,
                   proofreading_issues.status,
                   proofreading_issues.created_at
            FROM proofreading_issues
            INNER JOIN document_blocks
              ON document_blocks.id = proofreading_issues.block_id
            WHERE proofreading_issues.project_id = ?1
            ORDER BY document_blocks.block_index ASC,
                     proofreading_issues.start_offset ASC,
                     proofreading_issues.created_at ASC
            "#,
        )?;

        let rows = stmt.query_map([project_id], |row| {
            Ok(ProofreadingIssue {
                id: row.get(0)?,
                project_id: row.get(1)?,
                job_id: row.get(2)?,
                block_id: row.get(3)?,
                issue_type: parse_issue_type(&row.get::<_, String>(4)?),
                severity: parse_severity(&row.get::<_, String>(5)?),
                start_offset: row.get(6)?,
                end_offset: row.get(7)?,
                quote_text: row.get(8)?,
                prefix_text: row.get(9)?,
                suffix_text: row.get(10)?,
                suggestion: row.get(11)?,
                explanation: row.get(12)?,
                normalized_replacement: row.get(13)?,
                status: parse_issue_status(&row.get::<_, String>(14)?),
                created_at: row.get(15)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// 读取最近一次 job，供详情页和防重逻辑使用。
    pub fn get_latest_job(&self, project_id: &str) -> AppResult<Option<ProofreadingJob>> {
        let conn = self.db.connect()?;
        conn.query_row(
            r#"
            SELECT id, project_id, mode, status, options_json, auto_resume, started_at, finished_at,
                   total_blocks, completed_blocks, failed_blocks, total_issues, total_tokens_in,
                   total_tokens_out, total_latency_ms
            FROM proofreading_jobs
            WHERE project_id = ?1
            ORDER BY started_at DESC
            LIMIT 1
            "#,
            [project_id],
            map_job_row,
        )
        .optional()
        .map_err(Into::into)
    }

    /// 读取项目的调用日志。
    pub fn list_calls(&self, project_id: &str) -> AppResult<Vec<ProofreadingCall>> {
        let conn = self.db.connect()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT id, job_id, project_id, block_id, model_name, base_url, request_json,
                   response_json, status, started_at, finished_at, latency_ms, prompt_tokens,
                   completion_tokens, error_message
            FROM proofreading_calls
            WHERE project_id = ?1
            ORDER BY started_at DESC
            "#,
        )?;

        let rows = stmt.query_map([project_id], |row| {
            Ok(ProofreadingCall {
                id: row.get(0)?,
                job_id: row.get(1)?,
                project_id: row.get(2)?,
                block_id: row.get(3)?,
                model_name: row.get(4)?,
                base_url: row.get(5)?,
                request_json: row.get(6)?,
                response_json: row.get(7)?,
                status: row.get(8)?,
                started_at: row.get(9)?,
                finished_at: row.get(10)?,
                latency_ms: row.get(11)?,
                prompt_tokens: row.get(12)?,
                completion_tokens: row.get(13)?,
                error_message: row.get(14)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

fn map_job_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProofreadingJob> {
    Ok(ProofreadingJob {
        id: row.get(0)?,
        project_id: row.get(1)?,
        mode: parse_mode(&row.get::<_, String>(2)?),
        status: parse_status(&row.get::<_, String>(3)?),
        options_json: row.get(4)?,
        auto_resume: row.get::<_, i64>(5)? == 1,
        started_at: row.get(6)?,
        finished_at: row.get(7)?,
        total_blocks: row.get(8)?,
        completed_blocks: row.get(9)?,
        failed_blocks: row.get(10)?,
        total_issues: row.get(11)?,
        total_tokens_in: row.get(12)?,
        total_tokens_out: row.get(13)?,
        total_latency_ms: row.get(14)?,
    })
}

fn parse_block_type(value: &str) -> crate::types::BlockType {
    match value {
        "heading" => crate::types::BlockType::Heading,
        "table_cell" => crate::types::BlockType::TableCell,
        _ => crate::types::BlockType::Paragraph,
    }
}

fn parse_status(value: &str) -> ProofreadingStatus {
    match value {
        "running" => ProofreadingStatus::Running,
        "paused" => ProofreadingStatus::Paused,
        "completed" => ProofreadingStatus::Completed,
        "failed" => ProofreadingStatus::Failed,
        _ => ProofreadingStatus::Pending,
    }
}

fn parse_mode(value: &str) -> ProofreadingMode {
    match value {
        "retry_failed" => ProofreadingMode::RetryFailed,
        "selection" => ProofreadingMode::Selection,
        _ => ProofreadingMode::Full,
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

fn parse_issue_status(value: &str) -> IssueStatus {
    match value {
        "accepted" => IssueStatus::Accepted,
        "ignored" => IssueStatus::Ignored,
        "resolved" => IssueStatus::Resolved,
        _ => IssueStatus::Open,
    }
}

fn status_name(value: ProofreadingStatus) -> &'static str {
    match value {
        ProofreadingStatus::Pending => "pending",
        ProofreadingStatus::Running => "running",
        ProofreadingStatus::Paused => "paused",
        ProofreadingStatus::Completed => "completed",
        ProofreadingStatus::Failed => "failed",
    }
}

fn mode_name(value: ProofreadingMode) -> &'static str {
    match value {
        ProofreadingMode::Full => "full",
        ProofreadingMode::RetryFailed => "retry_failed",
        ProofreadingMode::Selection => "selection",
    }
}

fn issue_type_name(value: IssueType) -> &'static str {
    match value {
        IssueType::Typo => "typo",
        IssueType::Punctuation => "punctuation",
        IssueType::Grammar => "grammar",
        IssueType::Wording => "wording",
        IssueType::Redundancy => "redundancy",
        IssueType::Consistency => "consistency",
    }
}

fn severity_name(value: IssueSeverity) -> &'static str {
    match value {
        IssueSeverity::Low => "low",
        IssueSeverity::Medium => "medium",
        IssueSeverity::High => "high",
    }
}

fn issue_status_name(value: IssueStatus) -> &'static str {
    match value {
        IssueStatus::Open => "open",
        IssueStatus::Accepted => "accepted",
        IssueStatus::Ignored => "ignored",
        IssueStatus::Resolved => "resolved",
    }
}
