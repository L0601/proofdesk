//! 统一错误模型。
//!
//! 后端内部会遇到 IO、数据库、JSON、网络等多类错误。
//! 这里把它们统一收敛成 `AppError`，便于前端稳定处理。

use std::fmt::{Display, Formatter};

use serde::Serialize;

/// 项目里的统一结果类型。
pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
}

#[derive(Debug)]
pub struct AppError {
    /// 机器可读的错误码，前端适合按它做分支。
    pub code: &'static str,
    /// 面向人的错误信息，可直接提示给用户。
    pub message: String,
}

impl AppError {
    /// 快速创建一条统一格式的业务错误。
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Tauri 在把错误返回给前端时会走序列化。
        // 这里显式转成固定结构，避免直接暴露复杂错误对象。
        ErrorPayload {
            code: self.code.to_string(),
            message: self.message.clone(),
        }
        .serialize(serializer)
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::new("io_error", value.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        Self::new("db_error", value.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(value: tauri::Error) -> Self {
        Self::new("tauri_error", value.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::new("serde_json_error", value.to_string())
    }
}

impl From<zip::result::ZipError> for AppError {
    fn from(value: zip::result::ZipError) -> Self {
        Self::new("zip_error", value.to_string())
    }
}
