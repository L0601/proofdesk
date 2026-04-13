use std::fs;
use std::path::Path;

use crate::db::Database;
use crate::error::AppResult;
use crate::types::ProjectSummary;

#[derive(Debug, Clone)]
pub struct ProjectRepository {
    db: Database,
}

impl ProjectRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn list(&self) -> AppResult<Vec<ProjectSummary>> {
        let conn = self.db.connect()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT
              id,
              name,
              source_type,
              source_file_name,
              status,
              total_blocks,
              completed_blocks,
              failed_blocks,
              created_at,
              updated_at
            FROM projects
            ORDER BY updated_at DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ProjectSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                source_type: serde_json::from_str(&format!("\"{}\"", row.get::<_, String>(2)?))
                    .unwrap_or(crate::types::SourceType::Docx),
                source_file_name: row.get(3)?,
                status: serde_json::from_str(&format!("\"{}\"", row.get::<_, String>(4)?))
                    .unwrap_or(crate::types::ProjectStatus::Draft),
                total_blocks: row.get(5)?,
                completed_blocks: row.get(6)?,
                failed_blocks: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get(&self, project_id: &str) -> AppResult<Option<crate::types::ProjectDetail>> {
        let conn = self.db.connect()?;
        let mut stmt = conn.prepare(
            r#"
            SELECT
              id,
              name,
              source_type,
              source_file_name,
              source_file_path,
              normalized_doc_path,
              status,
              total_blocks,
              completed_blocks,
              failed_blocks,
              created_at,
              updated_at
            FROM projects
            WHERE id = ?1
            "#,
        )?;

        let mut rows = stmt.query([project_id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(crate::types::ProjectDetail {
                summary: ProjectSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    source_type: parse_source_type(&row.get::<_, String>(2)?),
                    source_file_name: row.get(3)?,
                    status: parse_project_status(&row.get::<_, String>(6)?),
                    total_blocks: row.get(7)?,
                    completed_blocks: row.get(8)?,
                    failed_blocks: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                },
                source_file_path: row.get(4)?,
                normalized_doc_path: row.get(5)?,
            }));
        }

        Ok(None)
    }

    pub fn update_progress(
        &self,
        project_id: &str,
        status: crate::types::ProjectStatus,
        completed_blocks: i64,
        failed_blocks: i64,
        updated_at: &str,
    ) -> AppResult<()> {
        let conn = self.db.connect()?;
        conn.execute(
            r#"
            UPDATE projects
            SET status = ?2,
                completed_blocks = ?3,
                failed_blocks = ?4,
                updated_at = ?5
            WHERE id = ?1
            "#,
            rusqlite::params![
                project_id,
                project_status_name(status),
                completed_blocks,
                failed_blocks,
                updated_at
            ],
        )?;
        Ok(())
    }

    pub fn delete(&self, project_id: &str) -> AppResult<()> {
        let mut conn = self.db.connect()?;
        let tx = conn.transaction()?;

        tx.execute(
            "DELETE FROM proofreading_issues WHERE project_id = ?1",
            [project_id],
        )?;
        tx.execute(
            "DELETE FROM proofreading_calls WHERE project_id = ?1",
            [project_id],
        )?;
        tx.execute(
            "DELETE FROM proofreading_jobs WHERE project_id = ?1",
            [project_id],
        )?;
        tx.execute(
            "DELETE FROM document_blocks WHERE project_id = ?1",
            [project_id],
        )?;
        tx.execute("DELETE FROM projects WHERE id = ?1", [project_id])?;

        tx.commit()?;
        Ok(())
    }

    pub fn delete_project_dir(&self, project_root: &Path) -> AppResult<()> {
        if project_root.exists() {
            fs::remove_dir_all(project_root)?;
        }
        Ok(())
    }
}

fn parse_source_type(value: &str) -> crate::types::SourceType {
    match value {
        "pdf" => crate::types::SourceType::Pdf,
        _ => crate::types::SourceType::Docx,
    }
}

fn parse_project_status(value: &str) -> crate::types::ProjectStatus {
    match value {
        "ready" => crate::types::ProjectStatus::Ready,
        "processing" => crate::types::ProjectStatus::Processing,
        "completed" => crate::types::ProjectStatus::Completed,
        "failed" => crate::types::ProjectStatus::Failed,
        _ => crate::types::ProjectStatus::Draft,
    }
}

fn project_status_name(value: crate::types::ProjectStatus) -> &'static str {
    match value {
        crate::types::ProjectStatus::Draft => "draft",
        crate::types::ProjectStatus::Ready => "ready",
        crate::types::ProjectStatus::Processing => "processing",
        crate::types::ProjectStatus::Completed => "completed",
        crate::types::ProjectStatus::Failed => "failed",
    }
}
