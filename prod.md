可以。下面我把**项目命名**和**之前的技术方案**整合成一份可直接交给 Codex 开工的技术文档草案。

---

# ProofDesk（砚准）技术方案 v1.0

## 1. 项目概述

### 1.1 项目名称

* **中文名**：砚准
* **英文名**：ProofDesk

### 1.2 产品定位

砚准是一款**面向长文稿件的本地智能校对桌面工具**，支持导入 **DOCX** 与**文本型 PDF**，将文稿解析为内部标准文档模型，按段调用 AI 进行错字、标点、语病、用词、重复、术语一致性等问题检测，并以**可追踪、可定位、可预览**的方式展示校对结果。

### 1.3 技术定位

* 运行形态：本地桌面客户端
* 架构模式：`Vue 3 + Tauri v2 + Rust + SQLite`
* 核心特点：

  * 原文件本地保存
  * 内部标准化文档模型
  * AI 校对任务调度
  * 实时进度与状态展示
  * 问题高亮与侧边栏详情
  * 原文对照预览

Tauri 官方支持任意前端框架，Vue/Vite 是常规接入方式；Tiptap 官方支持 Vue 3；PDF.js 适合 PDF 渲染与文本提取；`docx-preview` 适合 DOCX 原文对照渲染。

---

## 2. 产品目标与边界

### 2.1 MVP 目标

第一版实现以下能力：

1. 导入 `.docx`
2. 导入**文本型** `.pdf`
3. 扫描版 / 图片型 PDF 直接提示“不支持”
4. 解析文档并生成内部标准文档模型
5. 按段调用 AI 进行校对
6. 展示校对进度、状态、调用耗时、累计统计
7. 将问题锚定到正文对应位置
8. 提供问题侧边栏
9. 支持本地保存、恢复、失败重试
10. 提供原文对照视图

### 2.2 MVP 不做

第一版不做以下内容：

* OCR
* `.doc`
* Word Track Changes 回写
* 像素级还原 Word/PDF 原始版式
* 云同步
* 多人协作
* 复杂表格深度语义校对
* 图片内文字识别

---

## 3. 总体设计原则

### 3.1 核心原则

**原始 DOCX/PDF 不作为运行时唯一数据源。**

采用“双存储 + 标准化中间层”方案：

1. 保存原始文件
2. 导入后解析为内部标准文档 JSON
3. 校对任务、锚点、状态、结果都基于内部模型运行
4. 预览分为：

   * **校对视图**：基于内部模型
   * **原文对照视图**：基于原始文件

### 3.2 原因

直接在 Word/PDF 上做运行时校对会有几个问题：

* 不利于逐段任务调度
* 不利于偏移锚点稳定存储
* 不利于进度与失败重试
* 不利于结果高亮与跳转
* PDF 的文本提取结果天然碎片化，不是稳定的段落模型。PDF.js 的 API 重点是文档加载、页面获取、文本内容提取与渲染，不是现成的语义段落层。

---

## 4. 技术栈

## 4.1 客户端

* Vue 3
* Vite
* TypeScript
* Pinia
* Vue Router

## 4.2 编辑与展示

* Tiptap
* PDF.js
* docx-preview

Tiptap 是基于 ProseMirror 的 headless 编辑器框架，官方支持 Vue 3；PDF.js 适合 PDF 加载、渲染和文本提取；`docx-preview` 用于将 DOCX 渲染为 HTML 做原文对照。

## 4.3 桌面与后端

* Tauri v2
* Rust
* Tokio
* Reqwest
* Serde
* SQLite
* `sqlx` 或 `rusqlite`
* `zip`
* `quick-xml`
* `uuid`

## 4.4 数据存储

* SQLite：业务数据
* 本地目录：原始文件、标准化 JSON、日志、导出文件

---

## 5. 产品命名与标识

### 5.1 产品信息

* 产品名：**砚准**
* 英文名：**ProofDesk**
* slogan：**面向长文的本地智能校对台**

### 5.2 工程标识

* Git 仓库名：`proofdesk`
* Tauri `productName`：`砚准`
* Tauri `identifier`：`com.yanzhun.proofdesk`

### 5.3 README 开头

```markdown
# ProofDesk（砚准）

砚准是一款面向长文稿件的本地智能校对桌面工具，支持导入 DOCX 与文本型 PDF，按段调用 AI 完成错字、语病、标点、术语一致性等问题检查，并以可追踪、可定位、可预览的方式呈现校对结果。
```

---

## 6. 架构设计

```text
Frontend (Vue 3)
├─ 项目列表
├─ 项目详情
├─ 设置页
├─ Tiptap 校对视图
├─ PDF 原文预览
├─ DOCX 原文预览
├─ 结果侧栏
└─ Tauri invoke / event

Rust (Tauri backend)
├─ 导入服务
│  ├─ DOCX 解析
│  └─ PDF 标准化接收/保存
├─ 标准化处理
├─ AI 校对调度器
├─ LLM Client
├─ 锚点解析
├─ 导出服务
├─ SQLite Repository
└─ File Store
```

Tauri 的前后端通信主要通过 command invoke 和 event 完成，这很适合当前这种“长任务 + 实时进度推送”的本地桌面场景。

---

## 7. 数据模型设计

## 7.1 项目目录结构

```text
{app_data}/projects/{project_id}/
├─ meta.json
├─ original/
│  └─ source.docx | source.pdf
├─ normalized/
│  ├─ document.json
│  ├─ source_map.json
│  └─ preview_cache/
├─ exports/
└─ logs/
```

---

## 7.2 SQLite 表结构

### projects

```sql
CREATE TABLE projects (
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
```

### document_blocks

```sql
CREATE TABLE document_blocks (
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
CREATE INDEX idx_blocks_project_index ON document_blocks(project_id, block_index);
```

### proofreading_jobs

```sql
CREATE TABLE proofreading_jobs (
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
```

### proofreading_calls

```sql
CREATE TABLE proofreading_calls (
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
CREATE INDEX idx_calls_job_block ON proofreading_calls(job_id, block_id);
```

### proofreading_issues

```sql
CREATE TABLE proofreading_issues (
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
CREATE INDEX idx_issues_project_block ON proofreading_issues(project_id, block_id);
```

### app_settings

```sql
CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
```

---

## 7.3 内部标准文档格式

```json
{
  "docId": "proj_xxx",
  "sourceType": "docx",
  "version": 1,
  "blocks": [
    {
      "id": "blk_000001",
      "type": "paragraph",
      "page": 1,
      "runs": [
        { "text": "第一段文本", "marks": ["bold"] },
        { "text": "继续内容", "marks": [] }
      ],
      "text": "第一段文本继续内容",
      "layout": {
        "align": "left",
        "indent": 0,
        "lineBreakBefore": 0,
        "lineBreakAfter": 1
      },
      "sourceMap": {
        "sourceType": "docx",
        "paragraphIndex": 12,
        "runRange": [0, 1]
      }
    }
  ]
}
```

### 约束

* `block.id` 全局唯一
* `text` 为 runs 拼接结果
* `runs` 保留基础格式
* `layout` 保留有限布局信息
* `sourceMap` 用于回溯原始来源
* 所有校对锚点以 `block.id + offset` 为主

---

## 8. 导入方案

## 8.1 DOCX 导入

### 导入策略

DOCX 使用**双通道**：

1. **标准化通道**

   * Rust 读取 `.docx`
   * 解压 zip
   * 解析 `word/document.xml`
   * 抽取段落、run、显式换行、tab、基础样式、列表、表格文本
   * 生成内部标准文档模型

2. **原文对照通道**

   * 前端通过 `docx-preview` 渲染 HTML
   * 仅用于原文参考，不作为校对真相源

`docx-preview` 的定位是将 DOCX 转为 HTML 展示；而 Mammoth 明确主打语义化 HTML，不追求复制字体、字号、颜色等原始样式，因此不适合做本项目的唯一解析源。

### 第一版支持的 DOCX 元素

* 段落
* 文本 run
* 显式换行
* tab
* bold / italic / underline / strike
* superscript / subscript
* 标题层级
* 列表
* 表格中的文本
* 空段
* 分页符（可记录，不做强展示）

### 规则

* 一个 Word 段落通常映射为一个 `block`
* 多空格保留
* 空段保留
* 文本不自动 trim
* 表格单元格文本可先简化为 block

---

## 8.2 PDF 导入

### 导入策略

PDF 采用**文本型 PDF 支持，扫描型 PDF 拒绝**的方案。

流程：

1. 前端使用 PDF.js 加载 PDF
2. 逐页调用 `getTextContent()`
3. 提取 text items
4. 进行行聚合、段落重建
5. 生成内部标准文档模型
6. 将标准化结果交给 Rust 落库

PDF.js 官方 API 就提供页面获取、文本内容提取与渲染能力。

### 扫描型判断

如果一页文本项极少、总字符数远低于阈值，则标记为疑似图片页。
如果整份文档大部分页面都为疑似图片页，则导入失败并提示：

> 该 PDF 可能为扫描件或图片型文档，当前版本暂不支持。

### PDF 段落重建规则

* 按 `y` 坐标聚合成行
* 行内按 `x` 排序
* 根据空行、缩进、行距推断段落边界
* 页眉页脚做重复文本剔除
* 英文连字符跨行做温和修复
* 中文不做连字符修复

### 注意

PDF 的段落与格式还原能力一定弱于 DOCX。
产品定位上要明确：

* **DOCX 是主支持格式**
* **PDF 是只读型校对支持格式**

---

## 9. 校对执行方案

## 9.1 粒度

**按段（block）处理**，不是按页，也不是严格逐句。

原因：

* 页不是语义单位
* 逐句会导致请求数过多
* 段级上下文更充分
* 锚点更稳定

如果某个 block 太长，再进行句子级切片。

### 建议阈值

* 中文单次 600~1200 字
* 超过阈值就拆分 chunk
* chunk 之间保留少量上下文重叠

---

## 9.2 校对问题类型

第一版统一识别：

1. 错别字
2. 标点符号错误
3. 语法不通顺
4. 用词不当
5. 重复啰嗦
6. 术语前后不一致

---

## 9.3 LLM 配置项

设置页需要支持：

* `base_url`
* `api_key`
* `model`
* `timeout_ms`
* `max_concurrency`
* `temperature`
* `max_tokens`
* `system_prompt_template`

---

## 9.4 建议输出协议

强制模型返回结构化 JSON。

### 请求体示例

```json
{
  "block_id": "blk_000123",
  "text": "原始段落内容……",
  "rules": [
    "只找校对问题，不改写整体风格",
    "只输出明确存在的问题",
    "每项必须返回 start_offset/end_offset/quote/suggestion/explanation/type/severity",
    "若无问题，返回 issues: []"
  ]
}
```

### 返回体示例

```json
{
  "block_id": "blk_000123",
  "issues": [
    {
      "type": "typo",
      "severity": "high",
      "start_offset": 14,
      "end_offset": 16,
      "quote": "以经",
      "suggestion": "已经",
      "explanation": "常见错别字"
    }
  ]
}
```

---

## 10. 锚点方案

## 10.1 主锚点

每个 issue 必须至少保存：

* `block_id`
* `start_offset`
* `end_offset`

这是运行时的主定位方式。

## 10.2 备用锚点

同时保存：

* `quote_text`
* `prefix_text`
* `suffix_text`

原因：

* 文档可能被轻微编辑
* offset 可能失效
* 需要二次定位

### 锚点恢复优先级

1. `start_offset/end_offset`
2. `quote_text`
3. `quote + prefix/suffix`
4. 标记 unresolved anchor

---

## 11. Tiptap 方案

## 11.1 角色定位

Tiptap 负责：

* 展示标准化文档
* 高亮校对问题
* 正文与侧栏双向跳转
* 人工查看与轻量修改

不负责：

* 解析 DOCX/PDF
* 原始文档真相源
* PDF 页级展示

Tiptap 官方支持 Vue 3，且支持扩展系统、命令系统和唯一 ID 扩展，适合做这种交互式正文视图。

## 11.2 扩展

基础扩展：

* `StarterKit`
* `UniqueID`

自定义扩展：

* `IssueMark`
* `BlockMeta`

### IssueMark 属性

* `issueId`
* `issueType`
* `severity`
* `status`

### BlockMeta 属性

* `blockId`
* `sourcePage`
* `sourceLocator`

## 11.3 前端能力

需要实现：

* `scrollToBlock(blockId)`
* `focusIssue(issueId)`
* `highlightIssue(issueId)`
* `applySuggestion(issueId)`
* `ignoreIssue(issueId)`

---

## 12. 原文对照视图

## 12.1 PDF 对照视图

* 使用 PDF.js 原页展示
* 点击 issue 可跳到对应页
* 第一版不做页内精确字符高亮

## 12.2 DOCX 对照视图

* 使用 `docx-preview`
* 点击 issue 跳到对应段落附近
* 第一版不追求和校对视图完全同步高亮

---

## 13. 调度与状态管理

## 13.1 状态机

### 项目状态

* `importing`
* `ready`
* `proofreading`
* `completed`
* `failed`

### block 状态

* `pending`
* `running`
* `done`
* `error`
* `skipped`

### job 状态

* `queued`
* `running`
* `paused`
* `completed`
* `failed`
* `cancelled`

---

## 13.2 调度器

Rust 侧使用 Tokio 实现有限并发调度。

建议：

* 默认并发 2
* FIFO 调度
* 支持暂停 / 恢复 / 取消
* 单 block 失败后最多重试 2 次
* 每完成一个 block 就落库并发事件

---

## 13.3 事件推送

Rust -> 前端事件：

* `project/import-progress`
* `proofread/job-started`
* `proofread/block-started`
* `proofread/block-finished`
* `proofread/block-failed`
* `proofread/job-progress`
* `proofread/job-finished`

这类事件推送符合 Tauri 的标准前后端通信模型。

---

## 14. 页面设计

## 14.1 页面列表

* `/` 项目列表页
* `/project/:id` 项目详情页
* `/settings` 设置页
* `/logs/:projectId` 调用日志页

## 14.2 项目详情布局

```text
┌─────────────────────────────────────────────────────┐
│ 顶栏：项目名 | 文件类型 | 状态 | 开始/暂停/重试      │
├──────────────┬───────────────────────┬──────────────┤
│ 左栏         │ 中间正文               │ 右栏         │
│ 目录/块导航   │ 校对视图（Tiptap）      │ 问题列表      │
│ 页导航        │ 原文预览切换            │ 统计信息      │
│ 过滤器        │                       │ 问题详情      │
└──────────────┴───────────────────────┴──────────────┘
```

## 14.3 右栏模块

* 问题筛选
* 严重级别筛选
* issue 详情
* 采纳 / 忽略
* 调用统计

  * 已完成段落
  * 总问题数
  * 平均耗时
  * 总 token
  * 失败次数

---

## 15. 模块目录规划

## 15.1 Rust 目录

```text
src-tauri/src/
├─ main.rs
├─ app/
│  ├─ commands.rs
│  ├─ errors.rs
│  ├─ state.rs
│  └─ events.rs
├─ import/
│  ├─ mod.rs
│  ├─ docx.rs
│  ├─ pdf.rs
│  ├─ normalize.rs
│  └─ source_map.rs
├─ proofread/
│  ├─ mod.rs
│  ├─ scheduler.rs
│  ├─ prompt.rs
│  ├─ llm_client.rs
│  ├─ parser.rs
│  └─ anchor.rs
├─ repo/
│  ├─ mod.rs
│  ├─ projects.rs
│  ├─ blocks.rs
│  ├─ jobs.rs
│  ├─ issues.rs
│  └─ calls.rs
├─ export/
│  ├─ mod.rs
│  └─ report.rs
└─ utils/
   ├─ time.rs
   ├─ text.rs
   └─ ids.rs
```

## 15.2 前端目录

```text
src/
├─ app/
│  ├─ router.ts
│  ├─ store/
│  └─ api/
├─ pages/
│  ├─ ProjectListPage.vue
│  ├─ ProjectDetailPage.vue
│  ├─ SettingsPage.vue
│  └─ LogsPage.vue
├─ components/
│  ├─ layout/
│  ├─ proofread/
│  │  ├─ TiptapProofreadView.vue
│  │  ├─ IssueSidebar.vue
│  │  ├─ ProgressHeader.vue
│  │  └─ MetricsPanel.vue
│  ├─ source/
│  │  ├─ PdfSourcePreview.vue
│  │  └─ DocxSourcePreview.vue
│  └─ common/
├─ editor/
│  ├─ extensions/
│  │  ├─ IssueMark.ts
│  │  └─ BlockMeta.ts
│  ├─ buildEditorDoc.ts
│  └─ commands.ts
├─ types/
└─ utils/
```

---

## 16. Tauri Command 设计

```rust
#[tauri::command]
async fn import_document(file_path: String) -> Result<ProjectSummary, AppError>;

#[tauri::command]
async fn get_project_detail(project_id: String) -> Result<ProjectDetail, AppError>;

#[tauri::command]
async fn start_proofreading(project_id: String, options: ProofreadOptions) -> Result<JobSummary, AppError>;

#[tauri::command]
async fn pause_proofreading(job_id: String) -> Result<(), AppError>;

#[tauri::command]
async fn resume_proofreading(job_id: String) -> Result<(), AppError>;

#[tauri::command]
async fn cancel_proofreading(job_id: String) -> Result<(), AppError>;

#[tauri::command]
async fn retry_failed_blocks(project_id: String, job_id: String) -> Result<JobSummary, AppError>;

#[tauri::command]
async fn apply_issue(issue_id: String) -> Result<(), AppError>;

#[tauri::command]
async fn ignore_issue(issue_id: String) -> Result<(), AppError>;

#[tauri::command]
async fn update_settings(input: SettingsInput) -> Result<(), AppError>;

#[tauri::command]
async fn get_settings() -> Result<AppSettings, AppError>;

#[tauri::command]
async fn export_report(project_id: String) -> Result<ExportResult, AppError>;
```

---

## 17. 安全与权限

### 原则

* 项目文件统一存 App Data 目录
* 前端不直接访问任意系统路径
* API Key 本地存储时做掩码显示
* 默认不完整记录文档正文到日志
* 调试模式下可选保存完整请求/响应

Tauri v2 的文件系统和插件权限需要显式配置，访问范围可以限制到指定目录。

---

## 18. 开发阶段划分

## Phase 1：项目骨架

1. 初始化 `Vue3 + Vite + Tauri v2`
2. 接通 invoke / event
3. 建立 SQLite migration
4. 完成项目列表页 / 设置页

## Phase 2：DOCX 导入

1. Rust 解压 DOCX
2. 解析 XML
3. 生成内部模型
4. 落库
5. Tiptap 基础正文展示
6. docx-preview 原文对照

## Phase 3：PDF 导入

1. PDF.js 文本提取
2. 行聚合与段落重建
3. 扫描型判断
4. 落库
5. PDF.js 原文预览

## Phase 4：AI 校对

1. 设置页配置 OpenAI-compatible 参数
2. Rust LLM Client
3. 调度器
4. 调用记录入库
5. 实时进度推送

## Phase 5：结果交互

1. IssueMark
2. 侧边栏
3. 正文/侧栏双向跳转
4. 采纳 / 忽略

## Phase 6：导出与日志

1. 导出 HTML / Markdown 报告
2. 调用日志页
3. 失败重试

---

## 19. 给 Codex 的首批任务

### 任务 A：初始化工程

* 建立 `proofdesk`
* 配置 Vue 3 + Vite + Tauri v2 + TS
* 安装 Tiptap、PDF.js、docx-preview、Pinia
* 配置基础路由和布局

### 任务 B：建立数据库与 Repository

* migrations
* `projects`
* `document_blocks`
* `proofreading_jobs`
* `proofreading_calls`
* `proofreading_issues`
* `app_settings`

### 任务 C：DOCX 导入

* ZIP 解压
* XML 解析
* paragraph/run 提取
* 内部模型生成
* 落库

### 任务 D：PDF 导入

* PDF.js 文本提取
* 扫描件判断
* 段落重建
* 落库

### 任务 E：Tiptap 校对视图

* `buildEditorDoc`
* `IssueMark`
* `BlockMeta`
* `scrollToBlock`
* `focusIssue`

### 任务 F：调度器

* Job 创建
* 并发执行
* 事件推送
* 日志与统计

### 任务 G：设置与日志

* base_url / model / key / timeout 配置
* 调用日志展示
* 统计面板

---

## 20. 最终结论

**砚准（ProofDesk）第一版应采用：**

* **双存储**

  * 原始 DOCX/PDF 文件
  * 内部标准化文档 JSON

* **双视图**

  * Tiptap 校对视图
  * PDF.js / docx-preview 原文对照视图

* **按段校对**

  * 基于 block 调度
  * AI 返回结构化 JSON
  * 结果保存为 `blockId + offset + quote`

* **本地架构**

  * Vue 3 + Tauri v2 + Rust + SQLite

这条路线是当前需求下**最稳、最像正式产品、也最适合直接进入编码阶段**的方案。Tauri/Vue、Tiptap、PDF.js 和 `docx-preview` 的能力边界与这个分层方案是匹配的。
