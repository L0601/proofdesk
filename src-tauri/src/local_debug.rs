//! 本地调试工具。
//!
//! 这个模块不经过桌面 UI，而是给命令行排查问题用：
//! - 查看数据库路径
//! - 列出项目
//! - 查看 block
//! - 直接测试某个 block 的模型调用

use std::env;
use std::path::PathBuf;
use std::time::Instant;

use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::repository::{
    app_settings_repository::AppSettingsRepository, project_repository::ProjectRepository,
    proofreading_repository::ProofreadingRepository,
};
use crate::services::proofread_service::debug_call_text;
use crate::types::IssueType;

/// 解析 probe 子命令并执行。
pub async fn run_probe(args: &[String]) -> AppResult<()> {
    match parse_command(args)? {
        ProbeCommand::PrintDbPath => {
            println!("{}", default_db_path().display());
            Ok(())
        }
        ProbeCommand::ListProjects { db_path } => {
            let db = Database::from_path(db_path)?;
            let repo = ProjectRepository::new(db);
            for project in repo.list()? {
                println!(
                    "{}\t{}\t{}\t{}",
                    project.id, project.name, project.source_type.as_str(), project.updated_at
                );
            }
            Ok(())
        }
        ProbeCommand::ListBlocks { db_path, project_id } => {
            let db = Database::from_path(db_path)?;
            let repo = ProofreadingRepository::new(db);
            for block in repo.list_blocks(&project_id)? {
                let preview = truncate_line(&block.text_content, 80);
                println!(
                    "{}\t#{}\tpage={}\tstatus={:?}\t{}",
                    block.id,
                    block.block_index,
                    block.source_page.unwrap_or(0),
                    block.proofreading_status,
                    preview
                );
            }
            Ok(())
        }
        ProbeCommand::CallBlock {
            db_path,
            project_id,
            block_id,
            block_index,
        } => call_block(db_path, &project_id, block_id.as_deref(), block_index).await,
    }
}

/// 不经过 job 调度器，直接挑一个 block 调模型。
async fn call_block(
    db_path: PathBuf,
    project_id: &str,
    block_id: Option<&str>,
    block_index: Option<i64>,
) -> AppResult<()> {
    let db = Database::from_path(db_path)?;
    let settings = AppSettingsRepository::new(db.clone()).get()?;
    let repo = ProofreadingRepository::new(db);
    let blocks = repo.list_blocks(project_id)?;
    let block = select_block(&blocks, block_id, block_index)?;

    println!("project_id: {}", project_id);
    println!("block_id: {}", block.id);
    println!("block_index: {}", block.block_index);
    println!("source_page: {}", block.source_page.unwrap_or(0));
    println!("model: {}", settings.model);
    println!("base_url: {}", settings.base_url);
    println!(
        "auth_header: {}",
        if settings.api_key.trim().is_empty() {
            "disabled"
        } else {
            "bearer"
        }
    );
    println!("text:\n{}\n", block.text_content);

    let timer = Instant::now();
    let result = debug_call_text(
        &settings,
        &block.id,
        &block.text_content,
        &[
            IssueType::Typo,
            IssueType::Punctuation,
            IssueType::Grammar,
            IssueType::Wording,
            IssueType::Redundancy,
            IssueType::Consistency,
        ],
    )
    .await?;

    println!("elapsed_ms: {}", timer.elapsed().as_millis());
    println!("prompt_tokens: {}", result.prompt_tokens);
    println!("completion_tokens: {}", result.completion_tokens);
    println!("\n=== request ===\n{}\n", result.request_json);
    println!("=== response ===\n{}\n", result.response_json);
    Ok(())
}

/// 允许通过 block_id 或 block_index 选中一个段落。
fn select_block(
    blocks: &[crate::types::DocumentBlock],
    block_id: Option<&str>,
    block_index: Option<i64>,
) -> AppResult<crate::types::DocumentBlock> {
    if let Some(block_id) = block_id {
        return blocks
            .iter()
            .find(|block| block.id == block_id)
            .cloned()
            .ok_or_else(|| AppError::new("block_not_found", format!("未找到 block_id={block_id}")));
    }

    if let Some(block_index) = block_index {
        return blocks
            .iter()
            .find(|block| block.block_index == block_index)
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    "block_not_found",
                    format!("未找到 block_index={block_index}"),
                )
            });
    }

    Err(AppError::new(
        "missing_block_selector",
        "请通过 --block 或 --index 指定要测试的段落",
    ))
}

/// 默认数据库路径，与主程序保持一致。
fn default_db_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("com.yanzhun.proofdesk")
        .join("proofdesk.sqlite3")
}

/// 打印列表时把长文本压成单行预览。
fn truncate_line(text: &str, max_chars: usize) -> String {
    let single_line = text.replace('\n', " ");
    let truncated = single_line.chars().take(max_chars).collect::<String>();
    if single_line.chars().count() > max_chars {
        format!("{truncated}...")
    } else {
        truncated
    }
}

/// probe 支持的命令集合。
enum ProbeCommand {
    PrintDbPath,
    ListProjects { db_path: PathBuf },
    ListBlocks { db_path: PathBuf, project_id: String },
    CallBlock {
        db_path: PathBuf,
        project_id: String,
        block_id: Option<String>,
        block_index: Option<i64>,
    },
}

/// 解析命令行参数。
fn parse_command(args: &[String]) -> AppResult<ProbeCommand> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage_error());
    };

    let db_path = read_option(args, "--db")
        .map(PathBuf::from)
        .unwrap_or_else(default_db_path);

    match command {
        "print-db-path" => Ok(ProbeCommand::PrintDbPath),
        "list-projects" => Ok(ProbeCommand::ListProjects { db_path }),
        "list-blocks" => {
            let project_id = read_option(args, "--project")
                .ok_or_else(|| AppError::new("missing_project_id", "请提供 --project"))?;
            Ok(ProbeCommand::ListBlocks {
                db_path,
                project_id,
            })
        }
        "call-block" => {
            let project_id = read_option(args, "--project")
                .ok_or_else(|| AppError::new("missing_project_id", "请提供 --project"))?;
            let block_id = read_option(args, "--block");
            let block_index = read_option(args, "--index")
                .map(|value| {
                    value.parse::<i64>().map_err(|_| {
                        AppError::new("invalid_block_index", "--index 必须是整数")
                    })
                })
                .transpose()?;

            Ok(ProbeCommand::CallBlock {
                db_path,
                project_id,
                block_id,
                block_index,
            })
        }
        _ => Err(usage_error()),
    }
}

/// 读取 `--key value` 形式的简单选项。
fn read_option(args: &[String], key: &str) -> Option<String> {
    args.windows(2)
        .find(|window| window[0] == key)
        .map(|window| window[1].clone())
}

/// 输出命令行用法说明。
fn usage_error() -> AppError {
    AppError::new(
        "invalid_args",
        "用法:
  cargo run --bin proofread_probe -- print-db-path
  cargo run --bin proofread_probe -- list-projects [--db /path/to/proofdesk.sqlite3]
  cargo run --bin proofread_probe -- list-blocks --project <project_id> [--db /path/to/proofdesk.sqlite3]
  cargo run --bin proofread_probe -- call-block --project <project_id> (--block <block_id> | --index <block_index>) [--db /path/to/proofdesk.sqlite3]",
    )
}
