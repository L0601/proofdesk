下面直接给你一套 **可贴给 Codex 的开发任务清单模板**。
目标是：**按阶段逐步生成 ProofDesk（砚准）MVP**，避免一次性把工程做乱。

---

# ProofDesk（砚准）Codex 开发任务清单

## 0. 全局约束

先把这段作为所有任务的统一前置说明：

```text
你正在开发一个桌面端中文文稿智能校对工具：ProofDesk（中文名：砚准）。

技术栈：
- 前端：Vue 3 + Vite + TypeScript + Pinia + Vue Router
- 桌面端：Tauri v2
- 后端：Rust
- 本地存储：SQLite
- 文档展示：Tiptap
- PDF 原文预览：PDF.js
- DOCX 原文预览：docx-preview

项目目标：
- 支持导入 .docx 和文本型 .pdf
- 导入后生成内部标准化文档模型
- 按段调用 OpenAI-compatible 接口进行校对
- 实时显示进度、状态、累计耗时、调用日志
- 将校对问题高亮到正文对应位置
- 右侧显示问题列表和详情
- 所有数据本地保存

重要约束：
1. 不要把 DOCX/PDF 作为运行时唯一数据源
2. 必须保留“原始文件 + 标准化文档 JSON”双存储结构
3. 校对粒度按 block/paragraph，而不是按页
4. issue 锚点必须保存 blockId + startOffset + endOffset + quote/prefix/suffix
5. 第一版不做 OCR，不支持扫描型 PDF
6. 第一版不做 Word Track Changes 回写
7. 代码必须模块清晰、类型完整、可编译、可运行
8. 优先写 MVP，不要过度设计

代码风格要求：
- TypeScript 和 Rust 都要有清晰类型
- 所有模块职责单一
- 所有接口返回结构化对象
- 对错误做显式处理
- 关键逻辑加必要注释
- 不要生成伪代码，尽量生成真实可运行代码

输出要求：
- 每个任务都直接修改对应文件
- 新增文件时给出完整内容
- 修改文件时给出完整 patch
- 说明如何运行和验证
```

---

# 第一阶段：工程初始化

## 任务 1：初始化项目骨架

```text
请初始化一个名为 proofdesk 的项目，使用以下技术栈：
- Vue 3
- Vite
- TypeScript
- Tauri v2
- Pinia
- Vue Router

要求：
1. 生成前端目录和 src-tauri 目录
2. 配置基础路由：
   - /
   - /project/:id
   - /settings
3. 创建基础页面：
   - ProjectListPage.vue
   - ProjectDetailPage.vue
   - SettingsPage.vue
4. 创建基础布局：
   - 顶部标题栏显示“砚准 ProofDesk”
5. Tauri productName 设置为“砚准”
6. Tauri identifier 设置为“com.yanzhun.proofdesk”

完成后请输出：
- 新增/修改的文件列表
- 关键配置说明
- 本地启动命令
```

---

## 任务 2：安装并接入核心依赖

```text
在现有 proofdesk 项目中安装并接入以下依赖：

前端：
- @tiptap/vue-3
- @tiptap/starter-kit
- @tiptap/extension-unique-id
- pdfjs-dist
- docx-preview
- pinia
- vue-router

Rust：
- tokio
- serde
- serde_json
- uuid
- rusqlite 或 sqlx（二选一，优先更简单稳定的方案）
- reqwest
- zip
- quick-xml

要求：
1. 更新 package.json / Cargo.toml
2. 保证项目可编译
3. 创建一个最小可运行的 Tiptap 示例组件
4. 在 ProjectDetailPage 中临时挂载该组件验证依赖正常

完成后请输出：
- 安装的依赖列表
- 验证方式
- 运行命令
```

---

# 第二阶段：数据库与基础模型

## 任务 3：建立 SQLite schema 和 migration

```text
请在 proofdesk 项目中实现 SQLite 数据库初始化和 migration 机制。

需要创建以下表：
- projects
- document_blocks
- proofreading_jobs
- proofreading_calls
- proofreading_issues
- app_settings

字段要求如下：

projects:
- id TEXT PRIMARY KEY
- name TEXT NOT NULL
- source_type TEXT NOT NULL
- source_file_name TEXT NOT NULL
- source_file_path TEXT NOT NULL
- normalized_doc_path TEXT NOT NULL
- created_at TEXT NOT NULL
- updated_at TEXT NOT NULL
- status TEXT NOT NULL
- total_blocks INTEGER NOT NULL DEFAULT 0
- completed_blocks INTEGER NOT NULL DEFAULT 0
- failed_blocks INTEGER NOT NULL DEFAULT 0

document_blocks:
- id TEXT PRIMARY KEY
- project_id TEXT NOT NULL
- block_index INTEGER NOT NULL
- type TEXT NOT NULL
- text_content TEXT NOT NULL
- json_payload TEXT NOT NULL
- source_page INTEGER
- source_locator TEXT
- char_count INTEGER NOT NULL
- proofreading_status TEXT NOT NULL
- updated_at TEXT NOT NULL

proofreading_jobs:
- id TEXT PRIMARY KEY
- project_id TEXT NOT NULL
- mode TEXT NOT NULL
- status TEXT NOT NULL
- started_at TEXT
- finished_at TEXT
- total_blocks INTEGER NOT NULL
- completed_blocks INTEGER NOT NULL DEFAULT 0
- failed_blocks INTEGER NOT NULL DEFAULT 0
- total_issues INTEGER NOT NULL DEFAULT 0
- total_tokens_in INTEGER NOT NULL DEFAULT 0
- total_tokens_out INTEGER NOT NULL DEFAULT 0
- total_latency_ms INTEGER NOT NULL DEFAULT 0

proofreading_calls:
- id TEXT PRIMARY KEY
- job_id TEXT NOT NULL
- project_id TEXT NOT NULL
- block_id TEXT NOT NULL
- model_name TEXT NOT NULL
- base_url TEXT NOT NULL
- request_json TEXT NOT NULL
- response_json TEXT
- status TEXT NOT NULL
- started_at TEXT NOT NULL
- finished_at TEXT
- latency_ms INTEGER
- prompt_tokens INTEGER
- completion_tokens INTEGER
- error_message TEXT

proofreading_issues:
- id TEXT PRIMARY KEY
- project_id TEXT NOT NULL
- job_id TEXT NOT NULL
- block_id TEXT NOT NULL
- issue_type TEXT NOT NULL
- severity TEXT NOT NULL
- start_offset INTEGER NOT NULL
- end_offset INTEGER NOT NULL
- quote_text TEXT NOT NULL
- prefix_text TEXT
- suffix_text TEXT
- suggestion TEXT NOT NULL
- explanation TEXT NOT NULL
- normalized_replacement TEXT
- status TEXT NOT NULL
- created_at TEXT NOT NULL

app_settings:
- key TEXT PRIMARY KEY
- value TEXT NOT NULL

要求：
1. 写 migration 文件
2. Rust 启动时自动初始化数据库
3. 提供 db 模块
4. 提供基础 repository skeleton
5. 代码可编译

完成后请输出：
- migration 文件内容
- 数据库文件位置
- 初始化入口
```

---

## 任务 4：定义前后端共享类型

```text
请在 proofdesk 中建立一套清晰的共享数据类型定义。

前端 TypeScript 需要定义：
- ProjectSummary
- ProjectDetail
- DocumentBlock
- ProofreadingJob
- ProofreadingIssue
- AppSettings
- NormalizedDocument
- NormalizedBlock
- BlockRun
- BlockLayout
- SourceMap
- ProofreadOptions

Rust 侧需要定义对应的 serde struct。

要求：
1. 字段命名风格统一
2. Rust 和 TS 结构语义一致
3. 不强求自动代码生成，但结构要能明确对齐
4. 所有关键 enum 用字符串序列化

完成后请输出：
- TS 类型文件
- Rust struct 文件
- 命名约定说明
```

---

# 第三阶段：DOCX 导入

## 任务 5：实现 DOCX 文件导入命令

```text
请实现 Tauri command：import_document(file_path: String)

要求：
1. 先只支持 .docx
2. 把原始文件复制到项目目录：
   {app_data}/projects/{project_id}/original/source.docx
3. 创建 projects 记录
4. 返回 ProjectSummary
5. 暂时可以不解析正文，只先把导入链路打通

注意：
- project_id 使用 uuid
- 项目名默认取文件名（不含扩展名）
- 需要处理文件不存在、扩展名不支持等错误
- 保证命令可从前端 invoke

完成后请输出：
- command 实现
- 前端 invoke 示例
- 如何验证
```

---

## 任务 6：实现 DOCX 标准化解析

```text
在已实现的 DOCX 导入链路上，增加 DOCX 标准化解析能力。

要求：
1. 解压 .docx（zip）
2. 读取 word/document.xml
3. 解析段落、run、文本、显式换行、tab
4. 抽取基础样式：
   - bold
   - italic
   - underline
   - strike
   - superscript
   - subscript
5. 生成内部标准化文档 JSON：
   - docId
   - sourceType=docx
   - version=1
   - blocks[]
6. 每个段落映射为一个 block
7. block 字段必须包括：
   - id
   - type
   - page（可先为 null 或 0）
   - runs
   - text
   - layout
   - sourceMap
8. 同时写入 SQLite 的 document_blocks
9. 将 normalized document.json 保存到：
   {project_dir}/normalized/document.json
10. 更新 projects.total_blocks

注意：
- 多空格不要 trim
- 空段要保留
- tab 要保留
- 不要引入过度复杂的 Word 样式系统，先做 MVP

完成后请输出：
- 核心解析模块
- 标准化 JSON 示例
- 如何测试
```

---

## 任务 7：实现 DOCX 原文对照视图

```text
请在前端实现 DOCX 原文对照视图组件 DocxSourcePreview.vue。

要求：
1. 使用 docx-preview 渲染项目原始 DOCX
2. 在 ProjectDetailPage 中支持切换：
   - 校对视图
   - 原文对照视图
3. 原文对照视图只负责展示，不参与校对
4. 若加载失败，显示友好错误提示

完成后请输出：
- 组件文件
- ProjectDetailPage 的改动
- 验证方式
```

---

# 第四阶段：PDF 导入

## 任务 8：实现 PDF 文件导入与文本提取

```text
请扩展 import_document(file_path) 支持 .pdf。

要求：
1. 使用前端 pdfjs-dist 读取 PDF
2. 逐页提取 text content
3. 将提取结果通过 invoke 传给 Rust 保存
4. Rust 仍负责：
   - 创建项目
   - 复制原始 PDF 到项目目录
   - 保存 normalized document.json
   - 写 document_blocks
5. PDF 暂时不要求完美段落重建，但至少要：
   - 按页处理
   - 行内排序
   - 形成基础段落 block

注意：
- 不要把 PDF 文本提取逻辑写死在 Rust 里，优先复用前端 PDF.js
- block.sourceMap 中要记录 page 和 item range 信息

完成后请输出：
- 前端提取逻辑
- Rust 保存逻辑
- 数据流说明
```

---

## 任务 9：实现扫描型 PDF 检测

```text
请在 PDF 导入逻辑中加入“疑似扫描型 PDF”检测。

规则：
1. 每页统计 text items 数量和总字符数
2. 如果某页文本极少，则标记为疑似扫描页
3. 如果整份 PDF 大部分页面都是疑似扫描页，则导入失败
4. 返回明确错误：
   “该 PDF 可能为扫描件或图片型文档，当前版本暂不支持”

要求：
- 阈值写成可调整常量
- 错误要能在前端提示
- 失败时不要生成残缺项目记录，或者要做清理

完成后请输出：
- 判定逻辑
- 错误处理逻辑
- 测试建议
```

---

## 任务 10：实现 PDF 原文预览组件

```text
请实现 PdfSourcePreview.vue。

要求：
1. 使用 pdfjs-dist 渲染 PDF 页面
2. 支持基础翻页
3. 支持显示当前页码 / 总页数
4. 在 ProjectDetailPage 中与校对视图切换
5. 若项目是 PDF，则显示该组件

注意：
- 第一版不需要复杂缩放和搜索
- 先保证稳定可用

完成后请输出：
- 组件代码
- 页面接入方式
- 验证方法
```

---

# 第五阶段：Tiptap 校对视图

## 任务 11：把标准化文档渲染为 Tiptap 内容

```text
请实现 buildEditorDoc.ts，用于把 NormalizedDocument 转成 Tiptap JSON 文档。

要求：
1. 支持 paragraph
2. 支持 heading（如果 block.type 是 heading）
3. text runs 要映射为不同 marks
4. 节点 attrs 中保留：
   - blockId
   - sourcePage
5. 生成可直接传给 Tiptap editor 的 JSON

注意：
- 第一版先不处理复杂表格
- 保证文档内容与 block 顺序一致

完成后请输出：
- buildEditorDoc.ts
- 输入输出示例
```

---

## 任务 12：实现 Tiptap 校对视图组件

```text
请实现 TiptapProofreadView.vue。

要求：
1. 使用 @tiptap/vue-3
2. 加载 buildEditorDoc 生成的 JSON
3. 默认只读
4. 暴露方法：
   - scrollToBlock(blockId)
   - setIssues(issues)
5. 先不做 issue mark，只把正文稳定渲染出来

完成后请输出：
- 组件代码
- 父组件接入方式
- 验证方式
```

---

## 任务 13：实现 IssueMark 扩展

```text
请为 Tiptap 实现一个自定义 mark：IssueMark。

attrs：
- issueId
- issueType
- severity
- status

要求：
1. 可以将一个文本区间标记为 issue
2. 使用不同 CSS class 区分 issueType / severity
3. 点击被标记文本时发出事件或回调，把 issueId 通知父组件
4. 保证和现有只读视图兼容

完成后请输出：
- 扩展代码
- 使用方式
- 样式建议
```

---

## 任务 14：把 proofreading_issues 映射到正文高亮

```text
请实现一套逻辑：根据 ProofreadingIssue[] 把 issues 映射到 Tiptap 文本高亮。

要求：
1. 以 blockId + startOffset + endOffset 为主锚点
2. 对应 block 文本切片并加上 IssueMark
3. 支持多个 issue 共存
4. 若 offset 越界或文本不匹配，先记录 warning，不要直接崩溃

注意：
- 第一版先不处理 mark 重叠冲突的复杂情况，至少要保证基础可用
- 需要有 issue -> 文本区间的应用函数

完成后请输出：
- 核心映射逻辑
- 调用方式
- 一个包含多个 issue 的例子
```

---

# 第六阶段：设置与模型接入

## 任务 15：实现设置页

```text
请实现 SettingsPage.vue 和对应的 Tauri settings 读写接口。

设置项包括：
- baseUrl
- apiKey
- model
- timeoutMs
- maxConcurrency
- temperature
- maxTokens
- systemPromptTemplate

要求：
1. 前端表单可编辑
2. 保存到本地 SQLite 的 app_settings 表，或使用统一的 settings service
3. apiKey 输入框默认掩码
4. 保存后可再次读取
5. 提供默认值初始化逻辑

完成后请输出：
- 前端页面
- 后端 command
- 数据落盘方式
```

---

## 任务 16：实现 OpenAI-compatible LLM Client

```text
请在 Rust 中实现一个 OpenAI-compatible LLM client。

要求：
1. 可配置：
   - base_url
   - api_key
   - model
   - timeout
2. 支持调用 chat completions 风格接口
3. 返回原始响应和解析后的文本
4. 错误分类：
   - network_error
   - timeout
   - invalid_response
   - unauthorized
5. 请求和响应结果可写入 proofreading_calls

注意：
- 先只实现最常见的 OpenAI-compatible JSON 结构
- 不要绑定死 OpenAI 官方域名

完成后请输出：
- client 模块
- 调用示例
- 错误处理说明
```

---

# 第七阶段：校对调度器

## 任务 17：实现校对任务创建与调度器

```text
请实现 proofreading scheduler。

要求：
1. 新增 Tauri command：
   start_proofreading(project_id, options)
2. 读取该项目所有 pending blocks
3. 创建 proofreading_job
4. 按 maxConcurrency 并发处理
5. 每个 block 状态流转：
   pending -> running -> done/error
6. 每完成一个 block 就写库
7. 每完成一个 block 就通过 Tauri event 推送进度

事件包括：
- proofread/job-started
- proofread/block-started
- proofread/block-finished
- proofread/block-failed
- proofread/job-progress
- proofread/job-finished

完成后请输出：
- scheduler 核心代码
- 状态流说明
- 前端监听示例
```

---

## 任务 18：实现单个 block 的 prompt 组装和响应解析

```text
请实现：
- prompt.rs
- parser.rs

要求：
1. prompt 输入：
   - block_id
   - block text
   - 可选上下文
2. prompt 要求模型只返回 JSON
3. 识别的问题类型：
   - typo
   - grammar
   - punctuation
   - wording
   - redundancy
   - term_consistency
4. parser 负责校验：
   - block_id 匹配
   - issues 是数组
   - start/end offset 合法
   - suggestion/explanation/type/severity 存在
5. 若 offset 不可信，允许 fallback 到 quote 匹配
6. 最终返回结构化 issue 列表

完成后请输出：
- prompt 模板
- parser 实现
- 非法响应处理示例
```

---

## 任务 19：实现 proofreading_calls 和 proofreading_issues 入库

```text
请实现：
1. 每次模型调用都写 proofreading_calls
2. 成功解析后写 proofreading_issues
3. 更新 proofreading_jobs 汇总数据：
   - completed_blocks
   - failed_blocks
   - total_issues
   - total_tokens_in
   - total_tokens_out
   - total_latency_ms
4. 更新 projects.completed_blocks / failed_blocks

要求：
- 数据写入尽量在事务中进行
- 单个 block 的失败不能影响整个 job 已完成结果
- 支持重试时重复写入新的 call 记录，但 issue 要避免无脑重复

完成后请输出：
- repository 改动
- 数据一致性策略
```

---

# 第八阶段：进度、侧栏和交互

## 任务 20：实现项目详情页进度头部

```text
请实现 ProgressHeader.vue，并接入 ProjectDetailPage。

显示内容：
- 当前项目状态
- 已完成 blocks / 总 blocks
- 当前 job 状态
- 总问题数
- 平均耗时
- 失败 blocks 数

要求：
1. 能监听 Tauri event 实时更新
2. 页面刷新后也能从数据库恢复最新状态
3. 提供按钮：
   - 开始校对
   - 暂停
   - 重试失败项（可以先占位，后续再补）

完成后请输出：
- 组件代码
- 状态来源说明
```

---

## 任务 21：实现问题侧边栏

```text
请实现 IssueSidebar.vue。

要求：
1. 展示当前项目所有 issues
2. 支持按 issueType 筛选
3. 支持按 severity 筛选
4. 每条 issue 显示：
   - 类型
   - 原文片段
   - 建议
   - 说明
   - 状态
5. 点击 issue 时：
   - 通知正文滚动到对应 block
   - 高亮该 issue
6. 提供按钮：
   - 采纳
   - 忽略

完成后请输出：
- 组件代码
- 与正文联动方式
```

---

## 任务 22：实现 issue 采纳 / 忽略

```text
请实现：
- apply_issue(issue_id)
- ignore_issue(issue_id)

要求：
1. apply_issue：
   - 先只更新 issue.status = accepted
   - 暂时不直接修改正文内容（MVP 先不做自动回写）
2. ignore_issue：
   - 更新 issue.status = ignored
3. 前端侧边栏和正文高亮状态同步变化
4. 已 ignored 的 issue 样式弱化

完成后请输出：
- command 实现
- 前端接入方式
```

---

# 第九阶段：日志与导出

## 任务 23：实现调用日志页

```text
请实现 LogsPage.vue 或项目详情中的日志面板。

展示：
- 每次 LLM 调用时间
- blockId
- model
- latencyMs
- status
- token 使用
- errorMessage（如有）

要求：
1. 支持按 job 查看
2. 支持查看单次调用详情
3. 默认不要完整展示大段原文，可折叠显示 request/response

完成后请输出：
- 页面或面板代码
- 数据查询方式
```

---

## 任务 24：实现导出校对报告

```text
请实现 export_report(project_id)。

导出格式先支持：
- Markdown
- HTML

导出内容包括：
1. 项目名称
2. 原始文件名
3. 校对时间
4. 总 block 数
5. 已完成数
6. 总问题数
7. 各类型问题统计
8. 问题清单：
   - block 序号
   - issueType
   - severity
   - quote
   - suggestion
   - explanation
   - status

要求：
- 导出到项目目录 exports/
- 返回导出文件路径
- 前端可点击“导出报告”

完成后请输出：
- 导出模块
- 导出文件示例
```

---

# 第十阶段：补强与收尾

## 任务 25：实现失败重试

```text
请实现 retry_failed_blocks(project_id, job_id)。

要求：
1. 找出该 job 中状态为 error 的 block
2. 新建一个 retry job 或在原 job 下追加 retry 记录（二选一，优先更简单可维护）
3. 重新调度执行
4. 保留历史 call 记录
5. 成功后 block 状态改为 done

完成后请输出：
- 设计选择说明
- command 实现
```

---

## 任务 26：做一轮 MVP 清理和重构

```text
请对当前 proofdesk 项目做一轮 MVP 级清理和重构。

目标：
1. 清理无用代码
2. 合并重复类型
3. 修复明显命名不一致
4. 补足关键错误提示
5. 保证以下流程可用：
   - 导入 docx
   - 导入文本型 pdf
   - 查看正文
   - 查看原文对照
   - 开始校对
   - 实时看进度
   - 查看 issues
   - 导出报告

输出要求：
- 列出你做了哪些重构
- 列出仍未覆盖的风险点
- 给出下一步建议
```

---

# 你给 Codex 的执行方式建议

不要一次把全部任务丢给它。建议这样分批：

## 第一批

* 任务 1 ~ 4
  先把工程和数据库打起来。

## 第二批

* 任务 5 ~ 7
  先完成 DOCX 导入链路。

## 第三批

* 任务 8 ~ 10
  接 PDF。

## 第四批

* 任务 11 ~ 14
  把 Tiptap 正文和高亮做起来。

## 第五批

* 任务 15 ~ 19
  接模型与调度器。

## 第六批

* 任务 20 ~ 26
  补 UI、日志、导出、重试、清理。

---

# 最后给你一个更短的“总任务入口提示词”

如果你想先让 Codex 做第一大步，可以直接发这个：

```text
请基于以下要求，为 ProofDesk（砚准）实现第一阶段工程骨架和数据库基础设施：

技术栈：
- Vue 3 + Vite + TypeScript
- Tauri v2
- Rust
- SQLite
- Pinia
- Vue Router

项目目标：
这是一个本地智能校对桌面工具，支持 DOCX / 文本型 PDF 导入、标准化文档模型、AI 按段校对、进度展示、问题高亮和原文对照。

本次只做第一阶段：
1. 初始化项目骨架
2. 配置路由和基础页面
3. 建立 SQLite migration 和数据库初始化
4. 定义核心数据类型
5. 保证项目可启动、可编译

项目命名：
- 中文名：砚准
- 英文名：ProofDesk
- Tauri identifier：com.yanzhun.proofdesk

输出要求：
- 直接给出完整代码改动
- 不要伪代码
- 说明如何运行和验证
```

如果你愿意，我下一条可以继续帮你整理成 **README 初稿** 或者 **PRD + 技术设计文档合并版**。
