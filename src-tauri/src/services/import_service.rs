//! 文档导入服务。
//!
//! 这里负责把源文件转成统一的标准化结构，
//! 再把项目信息和 block 明细写入数据库。

use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use rusqlite::params;
use tauri::{AppHandle, Manager};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;
use zip::ZipArchive;

use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::types::{
    BlockLayout, BlockRun, BlockType, NormalizedBlock, NormalizedDocument, ProjectStatus,
    ProjectSummary, SourceMap, SourceType, TextAlign, TextMark,
};

pub struct ImportService {
    db: Database,
}

impl ImportService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// 直接导入原始文件。
    ///
    /// 当前规则：
    /// - DOCX 由后端解析
    /// - PDF 必须先在前端提取文本，再调用 `import_normalized_document`
    pub fn import_document(&self, app: &AppHandle, file_path: &str) -> AppResult<ProjectSummary> {
        let source_path = PathBuf::from(file_path);
        validate_source_path(&source_path)?;
        let source_type = detect_source_type(&source_path)?;
        match source_type {
            SourceType::Docx => self.import_docx(app, &source_path),
            SourceType::Pdf => Err(AppError::new(
                "pdf_frontend_required",
                "PDF 需先由前端提取文本后再导入",
            )),
        }
    }

    /// 导入前端已经标准化好的文档。
    /// 这主要服务于 PDF 导入链路。
    pub fn import_normalized_document(
        &self,
        app: &AppHandle,
        file_path: &str,
        source_type: SourceType,
        normalized: NormalizedDocument,
    ) -> AppResult<ProjectSummary> {
        let source_path = PathBuf::from(file_path);
        validate_source_path(&source_path)?;

        let detected = detect_source_type(&source_path)?;
        if detected.as_str() != source_type.as_str() {
            return Err(AppError::new("source_type_mismatch", "文件类型与标准化文档类型不匹配"));
        }

        let project_id = Uuid::new_v4().to_string();
        let now = now_rfc3339()?;
        let project_name = file_stem(&source_path)?;
        let project_dir = project_root(app, &project_id)?;
        let original_path = copy_original_file(&source_path, &project_dir, source_type)?;
        let normalized = NormalizedDocument {
            doc_id: project_id.clone(),
            source_type,
            version: normalized.version,
            blocks: normalized.blocks,
        };
        let normalized_path = write_normalized_doc(&project_dir, &normalized)?;
        let summary = build_project_summary(
            &project_id,
            &project_name,
            &source_path,
            source_type,
            normalized.blocks.len() as i64,
            &now,
        );

        persist_project(&self.db, &summary, &original_path, &normalized_path, &normalized)?;
        Ok(summary)
    }

    /// DOCX 的完整导入路径：
    /// 复制原文件 -> 解析 XML -> 写标准化文档 -> 落库。
    fn import_docx(&self, app: &AppHandle, source_path: &Path) -> AppResult<ProjectSummary> {
        let project_id = Uuid::new_v4().to_string();
        let now = now_rfc3339()?;
        let project_name = file_stem(source_path)?;
        let project_dir = project_root(app, &project_id)?;
        let original_path = copy_original_file(source_path, &project_dir, SourceType::Docx)?;
        let normalized = parse_docx(&project_id, source_path)?;
        let normalized_path = write_normalized_doc(&project_dir, &normalized)?;
        let summary = build_project_summary(
            &project_id,
            &project_name,
            source_path,
            SourceType::Docx,
            normalized.blocks.len() as i64,
            &now,
        );

        persist_project(&self.db, &summary, &original_path, &normalized_path, &normalized)?;
        Ok(summary)
    }
}

/// 导入前的基础文件校验。
fn validate_source_path(path: &Path) -> AppResult<()> {
    if !path.exists() {
        return Err(AppError::new("file_not_found", "指定文件不存在"));
    }

    if !path.is_file() {
        return Err(AppError::new("invalid_file", "导入目标不是有效文件"));
    }

    Ok(())
}

/// 仅根据扩展名判断文件类型。
fn detect_source_type(path: &Path) -> AppResult<SourceType> {
    match extension(path)?.as_str() {
        "docx" => Ok(SourceType::Docx),
        "pdf" => Ok(SourceType::Pdf),
        _ => Err(AppError::new("unsupported_extension", "当前仅支持导入 DOCX 或 PDF")),
    }
}

/// 读取并规范化扩展名。
fn extension(path: &Path) -> AppResult<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| AppError::new("missing_extension", "文件缺少扩展名"))
}

/// 每个项目都有一个独立工作目录。
fn project_root(app: &AppHandle, project_id: &str) -> AppResult<PathBuf> {
    let root = app.path().app_data_dir()?.join("projects").join(project_id);
    fs::create_dir_all(root.join("original"))?;
    fs::create_dir_all(root.join("normalized"))?;
    fs::create_dir_all(root.join("exports"))?;
    fs::create_dir_all(root.join("logs"))?;
    Ok(root)
}

/// 把原始文件复制进项目目录，避免依赖用户外部路径长期稳定存在。
fn copy_original_file(
    source_path: &Path,
    project_dir: &Path,
    source_type: SourceType,
) -> AppResult<PathBuf> {
    let target = project_dir.join("original").join(match source_type {
        SourceType::Docx => "source.docx",
        SourceType::Pdf => "source.pdf",
    });
    fs::copy(source_path, &target)?;
    Ok(target)
}

/// 把标准化文档写成 JSON 文件，方便前端和调试工具复用。
fn write_normalized_doc(project_dir: &Path, document: &NormalizedDocument) -> AppResult<PathBuf> {
    let path = project_dir.join("normalized").join("document.json");
    let content = serde_json::to_string_pretty(document)?;
    fs::write(&path, content)?;
    Ok(path)
}

/// 构造项目摘要对象。
fn build_project_summary(
    project_id: &str,
    project_name: &str,
    source_path: &Path,
    source_type: SourceType,
    total_blocks: i64,
    now: &str,
) -> ProjectSummary {
    ProjectSummary {
        id: project_id.to_string(),
        name: project_name.to_string(),
        source_type,
        source_file_name: source_path.file_name().unwrap().to_string_lossy().to_string(),
        status: ProjectStatus::Ready,
        total_blocks,
        completed_blocks: 0,
        failed_blocks: 0,
        created_at: now.to_string(),
        updated_at: now.to_string(),
    }
}

/// 用事务把项目主记录和全部 block 一次性写入数据库。
fn persist_project(
    db: &Database,
    summary: &ProjectSummary,
    original_path: &Path,
    normalized_path: &Path,
    document: &NormalizedDocument,
) -> AppResult<()> {
    let mut conn = db.connect()?;
    let tx = conn.transaction()?;

    tx.execute(
        r#"
        INSERT INTO projects (
          id, name, source_type, source_file_name, source_file_path, normalized_doc_path,
          created_at, updated_at, status, total_blocks, completed_blocks, failed_blocks
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
        params![
            summary.id,
            summary.name,
            source_type_name(summary.source_type),
            summary.source_file_name,
            original_path.to_string_lossy().to_string(),
            normalized_path.to_string_lossy().to_string(),
            summary.created_at,
            summary.updated_at,
            project_status_name(summary.status),
            summary.total_blocks,
            summary.completed_blocks,
            summary.failed_blocks,
        ],
    )?;

    for (index, block) in document.blocks.iter().enumerate() {
        tx.execute(
            r#"
            INSERT INTO document_blocks (
              id, project_id, block_index, type, text_content, json_payload, source_page,
              source_locator, char_count, proofreading_status, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                block.id,
                summary.id,
                index as i64,
                block_type_name(block.block_type),
                block.text,
                serde_json::to_string(block)?,
                block.page,
                block.source_map.locator.clone(),
                block.text.chars().count() as i64,
                "pending",
                summary.updated_at,
            ],
        )?;
    }

    tx.commit()?;
    Ok(())
}

/// 解析 DOCX 的核心逻辑：
/// 从压缩包里取出 `word/document.xml`，再按 XML 流构建 block。
fn parse_docx(project_id: &str, source_path: &Path) -> AppResult<NormalizedDocument> {
    let file = File::open(source_path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut doc_xml = String::new();
    archive
        .by_name("word/document.xml")?
        .read_to_string(&mut doc_xml)?;

    let mut reader = Reader::from_str(&doc_xml);
    reader.config_mut().trim_text(false);

    let mut blocks = Vec::new();
    let mut paragraph = ParagraphBuilder::new(0);
    let mut in_run_props = false;
    let mut current_marks = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                handle_start(&event, &mut paragraph, &mut current_marks, &mut in_run_props)?;
            }
            Ok(Event::Empty(event)) => {
                handle_empty(&event, &mut paragraph, &current_marks)?;
            }
            Ok(Event::Text(event)) => {
                if paragraph.in_text {
                    let text = String::from_utf8_lossy(event.as_ref()).into_owned();
                    paragraph.push_text(text, current_marks.clone());
                }
            }
            Ok(Event::End(event)) => {
                handle_end(event.name().as_ref(), &mut paragraph, &mut current_marks, &mut in_run_props, &mut blocks)?;
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(AppError::new("docx_parse_error", error.to_string())),
            _ => {}
        }
    }

    Ok(NormalizedDocument {
        doc_id: project_id.to_string(),
        source_type: SourceType::Docx,
        version: 1,
        blocks,
    })
}

/// 处理 XML 起始标签。
fn handle_start(
    event: &BytesStart<'_>,
    paragraph: &mut ParagraphBuilder,
    current_marks: &mut Vec<TextMark>,
    in_run_props: &mut bool,
) -> AppResult<()> {
    match local_name(event.name().as_ref()) {
        b"p" => paragraph.start_next(),
        b"r" => current_marks.clear(),
        b"rPr" => *in_run_props = true,
        b"t" => paragraph.in_text = true,
        b"pStyle" => paragraph.capture_style(event)?,
        b"b" if *in_run_props => push_mark(current_marks, TextMark::Bold),
        b"i" if *in_run_props => push_mark(current_marks, TextMark::Italic),
        b"u" if *in_run_props => push_mark(current_marks, TextMark::Underline),
        b"strike" if *in_run_props => push_mark(current_marks, TextMark::Strike),
        b"vertAlign" if *in_run_props => paragraph.capture_vert_align(event, current_marks)?,
        _ => {}
    }

    Ok(())
}

/// 处理 XML 自闭合标签，比如 `<w:tab/>`。
fn handle_empty(
    event: &BytesStart<'_>,
    paragraph: &mut ParagraphBuilder,
    current_marks: &[TextMark],
) -> AppResult<()> {
    match local_name(event.name().as_ref()) {
        b"tab" => paragraph.push_text("\t".to_string(), current_marks.to_vec()),
        b"br" => paragraph.push_text("\n".to_string(), current_marks.to_vec()),
        b"pStyle" => paragraph.capture_style(event)?,
        _ => {}
    }

    Ok(())
}

/// 处理 XML 结束标签。
fn handle_end(
    name: &[u8],
    paragraph: &mut ParagraphBuilder,
    current_marks: &mut Vec<TextMark>,
    in_run_props: &mut bool,
    blocks: &mut Vec<NormalizedBlock>,
) -> AppResult<()> {
    match local_name(name) {
        b"t" => paragraph.in_text = false,
        b"rPr" => *in_run_props = false,
        b"p" => {
            blocks.push(paragraph.build()?);
            paragraph.advance();
        }
        b"r" => current_marks.clear(),
        _ => {}
    }

    Ok(())
}

/// 避免同一个样式标记被重复压入。
fn push_mark(marks: &mut Vec<TextMark>, mark: TextMark) {
    if !marks.contains(&mark) {
        marks.push(mark);
    }
}

/// 去掉 XML 命名空间前缀，只保留本地标签名。
fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

/// 从文件名推导项目名。
fn file_stem(path: &Path) -> AppResult<String> {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(str::to_string)
        .ok_or_else(|| AppError::new("invalid_file_name", "无法解析项目名称"))
}

/// 项目统一使用 RFC3339 时间字符串，便于前后端直接传输。
pub fn now_rfc3339() -> AppResult<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| AppError::new("time_format_error", error.to_string()))
}

fn source_type_name(source_type: SourceType) -> &'static str {
    match source_type {
        SourceType::Docx => "docx",
        SourceType::Pdf => "pdf",
    }
}

fn project_status_name(status: ProjectStatus) -> &'static str {
    match status {
        ProjectStatus::Draft => "draft",
        ProjectStatus::Ready => "ready",
        ProjectStatus::Processing => "processing",
        ProjectStatus::Completed => "completed",
        ProjectStatus::Failed => "failed",
    }
}

fn block_type_name(block_type: BlockType) -> &'static str {
    match block_type {
        BlockType::Paragraph => "paragraph",
        BlockType::Heading => "heading",
        BlockType::TableCell => "table_cell",
    }
}

/// 一个流式 DOCX 段落构建器。
/// 可以把它理解成“按 XML 事件逐步积累当前段状态”的小状态机。
struct ParagraphBuilder {
    index: i64,
    block_type: BlockType,
    in_text: bool,
    runs: Vec<BlockRun>,
}

impl ParagraphBuilder {
    fn new(index: i64) -> Self {
        Self {
            index,
            block_type: BlockType::Paragraph,
            in_text: false,
            runs: Vec::new(),
        }
    }

    /// 遇到新段落时重置内部状态。
    fn start_next(&mut self) {
        self.runs.clear();
        self.block_type = BlockType::Paragraph;
        self.in_text = false;
    }

    /// 根据段落样式判断是否应视为标题。
    fn capture_style(&mut self, event: &BytesStart<'_>) -> AppResult<()> {
        let style = attr_value(event, b"val")?.unwrap_or_default().to_ascii_lowercase();
        if style.starts_with("heading") {
            self.block_type = BlockType::Heading;
        }
        Ok(())
    }

    /// 处理上下标标记。
    fn capture_vert_align(
        &mut self,
        event: &BytesStart<'_>,
        marks: &mut Vec<TextMark>,
    ) -> AppResult<()> {
        match attr_value(event, b"val")?.unwrap_or_default().as_str() {
            "superscript" => push_mark(marks, TextMark::Superscript),
            "subscript" => push_mark(marks, TextMark::Subscript),
            _ => {}
        }
        Ok(())
    }

    /// 收集一个 run 的文本与样式。
    fn push_text(&mut self, text: String, marks: Vec<TextMark>) {
        self.runs.push(BlockRun { text, marks });
    }

    /// 把当前积累的段落状态组装成一个标准化 block。
    fn build(&self) -> AppResult<NormalizedBlock> {
        let block_id = format!("blk_{:06}", self.index + 1);
        let text = self.runs.iter().map(|item| item.text.as_str()).collect();

        Ok(NormalizedBlock {
            id: block_id,
            block_type: self.block_type,
            page: None,
            runs: self.runs.clone(),
            text,
            layout: BlockLayout {
                align: TextAlign::Left,
                indent: 0,
                line_break_before: 0,
                line_break_after: 1,
            },
            source_map: SourceMap {
                source_type: SourceType::Docx,
                paragraph_index: Some(self.index),
                run_range: self.run_range(),
                page: None,
                item_range: None,
                locator: Some(format!("paragraph:{}", self.index)),
            },
        })
    }

    /// 计算当前段落的 run 索引范围。
    fn run_range(&self) -> Option<(i64, i64)> {
        if self.runs.is_empty() {
            None
        } else {
            Some((0, self.runs.len() as i64 - 1))
        }
    }

    /// 段落结束后推进索引。
    fn advance(&mut self) {
        self.index += 1;
        self.start_next();
    }
}

impl Default for ParagraphBuilder {
    fn default() -> Self {
        Self::new(0)
    }
}

/// 从 XML 标签属性中读取指定键值。
fn attr_value(event: &BytesStart<'_>, expected: &[u8]) -> AppResult<Option<String>> {
    for attr in event.attributes().flatten() {
        if local_name(attr.key.as_ref()) == expected {
            let value = String::from_utf8_lossy(attr.value.as_ref()).into_owned();
            return Ok(Some(value));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;

    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    use super::{detect_source_type, parse_docx};

    #[test]
    fn should_detect_docx_source_type() {
        let source_type = detect_source_type(Path::new("/tmp/sample.docx")).unwrap();
        assert!(matches!(source_type, super::SourceType::Docx));
    }

    #[test]
    fn should_parse_docx_into_blocks() {
        let path = std::env::temp_dir().join(format!("proofdesk-{}.docx", uuid::Uuid::new_v4()));
        write_test_docx(&path).unwrap();

        let document = parse_docx("proj_test", &path).unwrap();
        assert_eq!(document.blocks.len(), 2);
        assert_eq!(document.blocks[0].text, "第一段");
        assert_eq!(document.blocks[1].text, "第二段\t带 tab");

        std::fs::remove_file(path).unwrap();
    }

    fn write_test_docx(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(path)?;
        let mut zip = ZipWriter::new(file);
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
              <w:body>
                <w:p><w:r><w:t>第一段</w:t></w:r></w:p>
                <w:p>
                  <w:r><w:t>第二段</w:t></w:r>
                  <w:r><w:tab/></w:r>
                  <w:r><w:t>带 tab</w:t></w:r>
                </w:p>
              </w:body>
            </w:document>"#;

        zip.start_file("word/document.xml", SimpleFileOptions::default())?;
        zip.write_all(xml.as_bytes())?;

        zip.finish()?;
        Ok(())
    }
}
