use crate::db::Database;
use crate::error::AppResult;

#[derive(Debug, Clone)]
pub struct AppSettingsRepository {
    db: Database,
}

impl AppSettingsRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn upsert(&self, key: &str, value: &str) -> AppResult<()> {
        let conn = self.db.connect()?;
        conn.execute(
            r#"
            INSERT INTO app_settings(key, value)
            VALUES (?1, ?2)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
            (key, value),
        )?;
        Ok(())
    }
}
