#[tokio::main]
async fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Err(error) = proofdesk_lib::local_debug::run_probe(&args).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
