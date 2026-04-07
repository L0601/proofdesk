use crate::db::Database;
use crate::error::AppResult;
use crate::types::AppSettings;

const SETTINGS_KEY: &str = "proofread_settings";

#[derive(Debug, Clone)]
pub struct AppSettingsRepository {
    db: Database,
}

impl AppSettingsRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn get(&self) -> AppResult<AppSettings> {
        let conn = self.db.connect()?;
        let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key = ?1 LIMIT 1")?;
        let mut rows = stmt.query([SETTINGS_KEY])?;

        if let Some(row) = rows.next()? {
            let value: String = row.get(0)?;
            return Ok(serde_json::from_str(&value)?);
        }

        Ok(default_settings())
    }

    pub fn save(&self, settings: &AppSettings) -> AppResult<()> {
        let value = serde_json::to_string(settings)?;
        self.upsert(SETTINGS_KEY, &value)
    }

    fn upsert(&self, key: &str, value: &str) -> AppResult<()> {
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

fn default_settings() -> AppSettings {
    AppSettings {
        base_url: "https://api.openai.com/v1".to_string(),
        api_key: "".to_string(),
        model: "gpt-4.1-mini".to_string(),
        timeout_ms: 60_000,
        max_concurrency: 4,
        temperature: 0.2,
        max_tokens: 1200,
        system_prompt_template: "你是一名中文文稿校对助手。请严格输出结构化 JSON，只返回明确存在的问题。".to_string(),
    }
}
