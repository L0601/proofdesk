CREATE TABLE IF NOT EXISTS projects (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  source_type TEXT NOT NULL,
  source_file_name TEXT NOT NULL,
  source_file_path TEXT NOT NULL,
  normalized_doc_path TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  status TEXT NOT NULL,
  total_blocks INTEGER NOT NULL DEFAULT 0,
  completed_blocks INTEGER NOT NULL DEFAULT 0,
  failed_blocks INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS document_blocks (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  block_index INTEGER NOT NULL,
  type TEXT NOT NULL,
  text_content TEXT NOT NULL,
  json_payload TEXT NOT NULL,
  source_page INTEGER,
  source_locator TEXT,
  char_count INTEGER NOT NULL,
  proofreading_status TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_blocks_project_index
ON document_blocks(project_id, block_index);

CREATE TABLE IF NOT EXISTS proofreading_jobs (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  mode TEXT NOT NULL,
  status TEXT NOT NULL,
  started_at TEXT,
  finished_at TEXT,
  total_blocks INTEGER NOT NULL,
  completed_blocks INTEGER NOT NULL DEFAULT 0,
  failed_blocks INTEGER NOT NULL DEFAULT 0,
  total_issues INTEGER NOT NULL DEFAULT 0,
  total_tokens_in INTEGER NOT NULL DEFAULT 0,
  total_tokens_out INTEGER NOT NULL DEFAULT 0,
  total_latency_ms INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS proofreading_calls (
  id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL,
  project_id TEXT NOT NULL,
  block_id TEXT NOT NULL,
  model_name TEXT NOT NULL,
  base_url TEXT NOT NULL,
  request_json TEXT NOT NULL,
  response_json TEXT,
  status TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  latency_ms INTEGER,
  prompt_tokens INTEGER,
  completion_tokens INTEGER,
  error_message TEXT
);

CREATE INDEX IF NOT EXISTS idx_calls_job_block
ON proofreading_calls(job_id, block_id);

CREATE TABLE IF NOT EXISTS proofreading_issues (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  job_id TEXT NOT NULL,
  block_id TEXT NOT NULL,
  issue_type TEXT NOT NULL,
  severity TEXT NOT NULL,
  start_offset INTEGER NOT NULL,
  end_offset INTEGER NOT NULL,
  quote_text TEXT NOT NULL,
  prefix_text TEXT,
  suffix_text TEXT,
  suggestion TEXT NOT NULL,
  explanation TEXT NOT NULL,
  normalized_replacement TEXT,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_issues_project_block
ON proofreading_issues(project_id, block_id);

CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
