use djm_common::config::AppConfig;
use djm_daemon::audio_engine::AudioEngine;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("djm-daemon initializing...");

    let config_path = AppConfig::default_config_path();
    let config = AppConfig::load_or_create(&config_path)?;
    info!("Loaded configuration from {}", config_path.display());

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        info!("Received SIGINT / Ctrl+C. Initiating graceful shutdown...");
        r.store(false, Ordering::Relaxed);
    });

    let mock_mode = std::env::args().any(|arg| arg == "--mock");
    if mock_mode {
        info!("Running in --mock mode with simulated audio generator");
    }

    let mut engine = AudioEngine::new(config, running);
    engine.run(mock_mode).await?;

    info!("djm-daemon exited cleanly.");
    Ok(())
}
