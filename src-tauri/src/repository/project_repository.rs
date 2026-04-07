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
}
