//! `proofread_probe` 是一个独立的调试二进制。
//!
//! 它复用库里的 `local_debug` 模块，
//! 方便在不打开桌面程序的情况下排查数据库和模型调用问题。

#[tokio::main]
async fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Err(error) = proofdesk_lib::local_debug::run_probe(&args).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
