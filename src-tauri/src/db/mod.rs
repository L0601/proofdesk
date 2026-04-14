//! 数据库初始化与迁移。
//!
//! 当前项目使用 SQLite，结构上没有引入连接池。
//! `Database` 本身只保存数据库文件路径，需要时再打开连接。

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
    /// SQLite 文件的物理路径。
    db_path: PathBuf,
}

impl Database {
    /// 桌面程序正常启动时使用的初始化入口。
    pub fn init(app: &AppHandle) -> AppResult<Self> {
        let app_data_dir = app.path().app_data_dir()?;
        fs::create_dir_all(&app_data_dir)?;

        let db_path = app_data_dir.join(DB_FILE_NAME);
        let db = Self { db_path };
        db.run_migrations()?;
        Ok(db)
    }

    /// 调试工具使用的入口，允许直接指定数据库路径。
    pub fn from_path(db_path: PathBuf) -> AppResult<Self> {
        let db = Self { db_path };
        db.run_migrations()?;
        Ok(db)
    }

    /// 打开一个新的 SQLite 连接。
    pub fn connect(&self) -> AppResult<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }

    /// 依次执行迁移脚本。
    ///
    /// 这里通过 `schema_migrations` 表保证每个迁移版本只执行一次。
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

/// 迁移的最小执行单元。
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
