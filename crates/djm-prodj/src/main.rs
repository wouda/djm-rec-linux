use anyhow::Result;
use djm_prodj::daemon::{DaemonConfig, ProDjDaemon};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    info!("djm-prodj: Pro DJ Link Daemon initializing...");

    let mut config = DaemonConfig::default();

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--mock" => {
                config.mock = true;
            }
            "--socket" if i + 1 < args.len() => {
                config.socket_path = PathBuf::from(&args[i + 1]);
                i += 1;
            }
            "--session-log" if i + 1 < args.len() => {
                config.session_log = Some(PathBuf::from(&args[i + 1]));
                i += 1;
            }
            "--bind" if i + 1 < args.len() => {
                config.bind_address = args[i + 1].clone();
                i += 1;
            }
            "--help" | "-h" => {
                println!("djm-prodj: Pioneer Pro DJ Link Telemetry Daemon");
                println!();
                println!("USAGE:");
                println!("    djm-prodj [OPTIONS]");
                println!();
                println!("OPTIONS:");
                println!("    --socket <PATH>         Unix domain socket path (default: /run/user/1000/djm-prodj.sock)");
                println!("    --session-log <PATH>    Active session log file (<session>_prodj.jsonl)");
                println!("    --bind <IP>             UDP bind address (default: 0.0.0.0)");
                println!("    --mock                  Run in mock mode without binding hardware UDP ports");
                println!("    -h, --help              Print help information");
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        info!("Received SIGINT / Ctrl+C. Initiating graceful shutdown of djm-prodj...");
        r.store(false, Ordering::Relaxed);
    });

    let daemon = ProDjDaemon::new(config, running);
    daemon.run().await?;

    info!("djm-prodj exited cleanly.");
    Ok(())
}
