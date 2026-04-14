//! Repository 层负责直接读写数据库。
//!
//! 原则上：
//! - SQL 放在这一层
//! - 业务编排放在 service 层

pub mod app_settings_repository;
pub mod project_repository;
pub mod proofreading_repository;
