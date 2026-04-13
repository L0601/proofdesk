use std::fs;
use std::path::PathBuf;

use rusqlite::Connection;
use tauri::{AppHandle, Manager};

use crate::error::AppResult;

const DB_FILE_NAME: &str = "proofdesk.sqlite3";
const MIGRATION_001: &str = include_str!("migrations/001_init.sql");
const MIGRATION_002: &str = include_str!("migrations/002_proofreading_runtime.sql");

#[derive(Debug, Clone)]
pub struct Database {
    db_path: PathBuf,
}

impl Database {
    pub fn init(app: &AppHandle) -> AppResult<Self> {
        let app_data_dir = app.path().app_data_dir()?;
        fs::create_dir_all(&app_data_dir)?;

        let db_path = app_data_dir.join(DB_FILE_NAME);
        let db = Self { db_path };
        db.run_migrations()?;
        Ok(db)
    }

    pub fn from_path(db_path: PathBuf) -> AppResult<Self> {
        let db = Self { db_path };
        db.run_migrations()?;
        Ok(db)
    }

    pub fn connect(&self) -> AppResult<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }

    fn run_migrations(&self) -> AppResult<()> {
        let conn = self.connect()?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
              version TEXT PRIMARY KEY,
              applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )?;

        apply_migration(&conn, "001_init", MIGRATION_001)?;
        apply_migration(&conn, "002_proofreading_runtime", MIGRATION_002)?;

        Ok(())
    }
}

fn apply_migration(conn: &Connection, version: &str, sql: &str) -> AppResult<()> {
    let applied = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
        [version],
        |row| row.get::<_, i64>(0),
    )?;

    if applied == 0 {
        conn.execute_batch(sql)?;
        conn.execute(
            "INSERT INTO schema_migrations(version) VALUES (?1)",
            [version],
        )?;
    }

    Ok(())
}
