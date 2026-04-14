# ProofDesk 数据库技术文档

## 数据库位置

- 开发环境数据库文件位于：`~/Library/Application Support/com.yanzhun.proofdesk/proofdesk.sqlite3`
- 数据库初始化入口：`src-tauri/src/db/mod.rs`
- 当前迁移版本：
  - `001_init`
  - `002_proofreading_runtime`

## 表结构

### `projects`

用途：
保存项目主记录。

主要字段：
- `id`：项目 ID，UUID 字符串
- `name`：项目名，通常来自原文件名
- `source_type`：`docx` / `pdf`
- `source_file_name`：原始文件名
- `source_file_path`：复制后的原始文件路径
- `normalized_doc_path`：标准化文档 JSON 路径
- `status`：`draft` / `ready` / `processing` / `completed` / `failed`
- `total_blocks`
- `completed_blocks`
- `failed_blocks`
- `created_at`
- `updated_at`

### `document_blocks`

用途：
保存每个文档块，也就是后续 AI 校对的最小处理单元。

主要字段：
- `id`：块 ID，如 `blk_000001`
- `project_id`：所属项目
- `block_index`：块顺序，从 0 递增
- `type`：`paragraph` / `heading` / `table_cell`
- `text_content`：块纯文本
- `json_payload`：完整标准化 block JSON
- `source_page`：原文页码，PDF 时可能有值
- `source_locator`：原文定位信息
- `char_count`：字符数
- `proofreading_status`：`pending` / `running` / `completed` / `failed`
- `updated_at`

### `proofreading_jobs`

用途：
保存一次校对任务的主记录。

主要字段：
- `id`：任务 ID，UUID 字符串
- `project_id`
- `mode`：`full` / `retry_failed` / `selection`
- `status`：`pending` / `running` / `completed` / `failed`
- `options_json`：任务参数快照，用于恢复任务
- `auto_resume`：是否允许自动恢复
- `started_at`
- `finished_at`
- `total_blocks`
- `completed_blocks`
- `failed_blocks`
- `total_issues`
- `total_tokens_in`
- `total_tokens_out`
- `total_latency_ms`

### `proofreading_calls`

用途：
保存每个 block 的一次模型调用记录。

主要字段：
- `id`：调用 ID，UUID 字符串
- `job_id`
- `project_id`
- `block_id`
- `model_name`
- `base_url`
- `request_json`
- `response_json`
- `status`：`completed` / `failed` / `skipped`
- `started_at`
- `finished_at`
- `latency_ms`
- `prompt_tokens`
- `completion_tokens`
- `error_message`

### `proofreading_issues`

用途：
保存模型识别出的校对问题。

主要字段：
- `id`：问题 ID，UUID 字符串
- `project_id`
- `job_id`
- `block_id`
- `issue_type`
- `severity`
- `start_offset`
- `end_offset`
- `quote_text`
- `prefix_text`
- `suffix_text`
- `suggestion`
- `explanation`
- `normalized_replacement`
- `status`：`open` / `accepted` / `ignored` / `resolved`
- `created_at`

### `app_settings`

用途：
保存应用配置，目前主要是模型设置、并发配置和 PDF 导入阈值。

主要字段：
- `key`
- `value`

当前使用的 key：
- `proofread_settings`

## ID 生成规则

### 项目 ID

- 使用 `Uuid::new_v4().to_string()`
- 生成位置：导入文档时

### 任务 ID

- 使用 `Uuid::new_v4().to_string()`
- 生成位置：开始校对任务时

### 调用 ID

- 使用 `Uuid::new_v4().to_string()`
- 生成位置：每次写入 `proofreading_calls` 时

### 问题 ID

- 使用 `Uuid::new_v4().to_string()`
- 生成位置：模型返回 issue 并落库时

### 文档块 ID

- 不是 UUID
- 使用顺序号格式：`blk_000001`
- 生成规则：
  - DOCX：按段落顺序生成
  - PDF：按推断段落顺序生成

## 数据写入流程

### 1. 导入 DOCX

流程：
- 复制原文件到项目目录
- 解析 `word/document.xml`
- 每个 Word 段落生成一个标准化 block
- 写入 `projects`
- 写入 `document_blocks`
- 写入标准化文档 JSON

### 2. 导入 PDF

流程：
- 前端先用 `pdfjs-dist` 提取文本
- 按页面文本项合并成行
- 根据垂直间距推断段落
- 清洗段首段尾制表符、全角空格和普通空白
- 过滤掉长度小于 `pdfMinBlockChars` 的 block
- 生成标准化文档
- 后端写入 `projects`
- 后端写入 `document_blocks`

### 3. 启动校对任务

流程：
- 读取项目 block
- 根据模式选择 block
- 生成 `proofreading_jobs`
- 把项目状态改为 `processing`
- 把待处理 block 状态置为 `pending`
- 后台调度器按并发数开始处理

### 4. 执行单个 block 校对

流程：
- block 状态改为 `running`
- 组装模型请求
- 写日志文件
- 调用模型
- 写入 `proofreading_calls`
- 用新结果替换该 block 的 `proofreading_issues`
- 成功时 block 状态改为 `completed`
- 失败时 block 状态改为 `failed`

### 5. 任务结束

流程：
- 汇总 `proofreading_calls`
- 统计 `completed_blocks`
- 统计 `failed_blocks`
- 统计 token 和耗时
- 统计 `proofreading_issues`
- 更新 `proofreading_jobs`
- 更新 `projects`

## 状态变更

### 项目状态 `projects.status`

状态流转：
- `ready` -> `processing`
- `processing` -> `completed`
- `processing` -> `failed`

说明：
- 导入完成后通常是 `ready`
- 启动校对后进入 `processing`
- 如果任务至少有成功结果，最终通常会进入 `completed`
- 如果全部失败，任务和项目会进入 `failed`

### block 状态 `document_blocks.proofreading_status`

状态流转：
- `pending` -> `running`
- `running` -> `completed`
- `running` -> `failed`

恢复逻辑：
- 应用重启时，如果发现任务仍是 `running`
- 会把遗留的 `running` block 回退成 `pending`
- 然后继续后台调度

### job 状态 `proofreading_jobs.status`

状态流转：
- `running` -> `completed`
- `running` -> `failed`

恢复依赖：
- `options_json`
- `auto_resume`

如果旧任务没有 `options_json`，则无法恢复。

## 项目删除流程

删除项目时会清理：
- `projects`
- `document_blocks`
- `proofreading_jobs`
- `proofreading_calls`
- `proofreading_issues`
- 本地项目目录 `app_data_dir/projects/<project_id>`

不会清理：
- 按日期共享的 AI 日志文件

如果项目当前处于后台处理中，前后端都会拒绝删除。
