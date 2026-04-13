use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    #[serde(flatten)]
    pub summary: ProjectSummary,
    pub source_file_path: String,
    pub normalized_doc_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockRun {
    pub text: String,
    pub marks: Vec<TextMark>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockLayout {
    pub align: TextAlign,
    pub indent: i64,
    pub line_break_before: i64,
    pub line_break_after: i64,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedDocument {
    pub doc_id: String,
    pub source_type: SourceType,
    pub version: i64,
    pub blocks: Vec<NormalizedBlock>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_ms: i64,
    pub max_concurrency: i64,
    pub temperature: f64,
    pub max_tokens: i64,
    pub system_prompt_template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofreadOptions {
    pub mode: ProofreadingMode,
    pub max_chunk_chars: i64,
    pub overlap_chars: i64,
    pub issue_types: Vec<IssueType>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceType {
    Docx,
    Pdf,
}

impl SourceType {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceType::Docx => "docx",
            SourceType::Pdf => "pdf",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Draft,
    Ready,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockType {
    Paragraph,
    Heading,
    TableCell,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofreadingStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofreadingMode {
    Full,
    RetryFailed,
    Selection,
}

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueStatus {
    Open,
    Accepted,
    Ignored,
    Resolved,
}

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    Left,
    Center,
    Right,
    Justify,
}
