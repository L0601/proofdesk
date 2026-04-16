//! 共享数据模型。
//!
//! 这一层的结构体同时承担两种职责：
//! 1. 后端内部的业务对象。
//! 2. 前后端通信时的 JSON DTO。

use serde::{Deserialize, Serialize};

/// 项目列表页需要的摘要信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub source_type: SourceType,
    pub source_file_name: String,
    pub status: ProjectStatus,
    pub total_blocks: i64,
    pub completed_blocks: i64,
    pub failed_blocks: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// 项目详情页对象。
/// 通过 `flatten` 复用 `ProjectSummary`，减少重复字段定义。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    #[serde(flatten)]
    pub summary: ProjectSummary,
    pub source_file_path: String,
    pub normalized_doc_path: String,
}

/// 一个文本 run 表示同一段中格式连续不变的一小段文字。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockRun {
    pub text: String,
    pub marks: Vec<TextMark>,
}

/// 标准化 block 的版式信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockLayout {
    pub align: TextAlign,
    pub indent: i64,
    pub line_break_before: i64,
    pub line_break_after: i64,
}

/// 从标准化 block 回溯到源文档位置的信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMap {
    pub source_type: SourceType,
    pub paragraph_index: Option<i64>,
    pub run_range: Option<(i64, i64)>,
    pub page: Option<i64>,
    pub item_range: Option<(i64, i64)>,
    pub locator: Option<String>,
}

/// 导入阶段生成的标准化 block。
/// 它是“源文件格式”和“数据库落库结构”之间的中间层。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedBlock {
    pub id: String,
    #[serde(rename = "type")]
    pub block_type: BlockType,
    pub page: Option<i64>,
    pub runs: Vec<BlockRun>,
    pub text: String,
    pub layout: BlockLayout,
    pub source_map: SourceMap,
}

/// 标准化文档。
/// PDF 前端解析和 DOCX 后端解析，最终都会统一落到这个结构上。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedDocument {
    pub doc_id: String,
    pub source_type: SourceType,
    pub version: i64,
    pub blocks: Vec<NormalizedBlock>,
}

/// 数据库 `document_blocks` 表的一行。
/// 这是 AI 校对真正消费的最小工作单元。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentBlock {
    pub id: String,
    pub project_id: String,
    pub block_index: i64,
    #[serde(rename = "type")]
    pub block_type: BlockType,
    pub text_content: String,
    pub json_payload: String,
    pub source_page: Option<i64>,
    pub source_locator: Option<String>,
    pub char_count: i64,
    pub proofreading_status: ProofreadingStatus,
    pub updated_at: String,
}

/// 一次校对任务。
/// 注意它是“整批任务”级别，不是单个 block 的状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofreadingJob {
    pub id: String,
    pub project_id: String,
    pub mode: ProofreadingMode,
    pub status: ProofreadingStatus,
    pub options_json: Option<String>,
    pub auto_resume: bool,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub total_blocks: i64,
    pub completed_blocks: i64,
    pub failed_blocks: i64,
    pub total_issues: i64,
    pub total_tokens_in: i64,
    pub total_tokens_out: i64,
    pub total_latency_ms: i64,
}

/// 一次具体的模型调用记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofreadingCall {
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

/// 模型返回的一条问题记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofreadingIssue {
    pub id: String,
    pub project_id: String,
    pub job_id: String,
    pub block_id: String,
    pub issue_type: IssueType,
    pub severity: IssueSeverity,
    pub start_offset: i64,
    pub end_offset: i64,
    pub quote_text: String,
    pub prefix_text: Option<String>,
    pub suffix_text: Option<String>,
    pub suggestion: String,
    pub explanation: String,
    pub normalized_replacement: Option<String>,
    pub status: IssueStatus,
    pub created_at: String,
}

/// 设置页里的模型参数配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_ms: i64,
    pub max_concurrency: i64,
    #[serde(default = "default_pdf_min_block_chars")]
    pub pdf_min_block_chars: i64,
    #[serde(default)]
    pub proofread_skip_pages: i64,
    pub temperature: f64,
    pub max_tokens: i64,
    pub system_prompt_template: String,
}

/// 兼容旧配置时的默认值。
fn default_pdf_min_block_chars() -> i64 {
    16
}

/// “开始校对”时传下来的选项快照。
/// 会被写入 job，供任务恢复时重新读取。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofreadOptions {
    pub mode: ProofreadingMode,
    pub max_chunk_chars: i64,
    pub overlap_chars: i64,
    pub issue_types: Vec<IssueType>,
}

/// 源文档类型。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceType {
    Docx,
    Pdf,
}

impl SourceType {
    /// 数据库存储用的小写字符串表示。
    pub fn as_str(self) -> &'static str {
        match self {
            SourceType::Docx => "docx",
            SourceType::Pdf => "pdf",
        }
    }
}

/// 项目整体状态。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Draft,
    Ready,
    Processing,
    Completed,
    Failed,
}

/// block 类型。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockType {
    Paragraph,
    Heading,
    TableCell,
}

/// 任务或 block 的执行状态。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofreadingStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
}

/// 校对模式。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofreadingMode {
    Full,
    RetryFailed,
    Selection,
}

/// 问题类别。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueType {
    Typo,
    Punctuation,
    Grammar,
    Wording,
    Redundancy,
    Consistency,
}

/// 问题严重程度。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Low,
    Medium,
    High,
}

/// 问题的人为处理状态。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueStatus {
    Open,
    Accepted,
    Ignored,
    Resolved,
}

/// 文本格式标记。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TextMark {
    Bold,
    Italic,
    Underline,
    Strike,
    Superscript,
    Subscript,
}

/// 段落对齐方式。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    Left,
    Center,
    Right,
    Justify,
}
