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

    pub fn import_document(&self, app: &AppHandle, file_path: &str) -> AppResult<ProjectSummary> {
        let source_path = PathBuf::from(file_path);
        validate_source_path(&source_path)?;
        let source_type = detect_source_type(&source_path)?;
        match source_type {
            SourceType::Docx => self.import_docx(app, &source_path),
            SourceType::Pdf => Err(AppError::new("unsupported_source", "当前仅完成 DOCX 导入链路")),
        }
    }

    fn import_docx(&self, app: &AppHandle, source_path: &Path) -> AppResult<ProjectSummary> {
        let project_id = Uuid::new_v4().to_string();
        let now = now_rfc3339()?;
        let project_name = file_stem(source_path)?;
        let project_dir = project_root(app, &project_id)?;
        let original_path = copy_original_file(source_path, &project_dir)?;
        let normalized = parse_docx(&project_id, source_path)?;
        let normalized_path = write_normalized_doc(&project_dir, &normalized)?;
        let summary = build_project_summary(
            &project_id,
            &project_name,
            source_path,
            normalized.blocks.len() as i64,
            &now,
        );

        persist_project(&self.db, &summary, &original_path, &normalized_path, &normalized)?;
        Ok(summary)
    }
}

fn validate_source_path(path: &Path) -> AppResult<()> {
    if !path.exists() {
        return Err(AppError::new("file_not_found", "指定文件不存在"));
    }

    if !path.is_file() {
        return Err(AppError::new("invalid_file", "导入目标不是有效文件"));
    }

    Ok(())
}

fn detect_source_type(path: &Path) -> AppResult<SourceType> {
    match extension(path)?.as_str() {
        "docx" => Ok(SourceType::Docx),
        "pdf" => Ok(SourceType::Pdf),
        _ => Err(AppError::new("unsupported_extension", "当前仅支持导入 DOCX 或 PDF")),
    }
}

fn extension(path: &Path) -> AppResult<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| AppError::new("missing_extension", "文件缺少扩展名"))
}

fn project_root(app: &AppHandle, project_id: &str) -> AppResult<PathBuf> {
    let root = app.path().app_data_dir()?.join("projects").join(project_id);
    fs::create_dir_all(root.join("original"))?;
    fs::create_dir_all(root.join("normalized"))?;
    fs::create_dir_all(root.join("exports"))?;
    fs::create_dir_all(root.join("logs"))?;
    Ok(root)
}

fn copy_original_file(source_path: &Path, project_dir: &Path) -> AppResult<PathBuf> {
    let target = project_dir.join("original").join("source.docx");
    fs::copy(source_path, &target)?;
    Ok(target)
}

fn write_normalized_doc(project_dir: &Path, document: &NormalizedDocument) -> AppResult<PathBuf> {
    let path = project_dir.join("normalized").join("document.json");
    let content = serde_json::to_string_pretty(document)?;
    fs::write(&path, content)?;
    Ok(path)
}

fn build_project_summary(
    project_id: &str,
    project_name: &str,
    source_path: &Path,
    total_blocks: i64,
    now: &str,
) -> ProjectSummary {
    ProjectSummary {
        id: project_id.to_string(),
        name: project_name.to_string(),
        source_type: SourceType::Docx,
        source_file_name: source_path.file_name().unwrap().to_string_lossy().to_string(),
        status: ProjectStatus::Ready,
        total_blocks,
        completed_blocks: 0,
        failed_blocks: 0,
        created_at: now.to_string(),
        updated_at: now.to_string(),
    }
}

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

fn push_mark(marks: &mut Vec<TextMark>, mark: TextMark) {
    if !marks.contains(&mark) {
        marks.push(mark);
    }
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn file_stem(path: &Path) -> AppResult<String> {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(str::to_string)
        .ok_or_else(|| AppError::new("invalid_file_name", "无法解析项目名称"))
}

fn now_rfc3339() -> AppResult<String> {
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

    fn start_next(&mut self) {
        self.runs.clear();
        self.block_type = BlockType::Paragraph;
        self.in_text = false;
    }

    fn capture_style(&mut self, event: &BytesStart<'_>) -> AppResult<()> {
        let style = attr_value(event, b"val")?.unwrap_or_default().to_ascii_lowercase();
        if style.starts_with("heading") {
            self.block_type = BlockType::Heading;
        }
        Ok(())
    }

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

    fn push_text(&mut self, text: String, marks: Vec<TextMark>) {
        self.runs.push(BlockRun { text, marks });
    }
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

    fn run_range(&self) -> Option<(i64, i64)> {
        if self.runs.is_empty() {
            None
        } else {
            Some((0, self.runs.len() as i64 - 1))
        }
    }

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
