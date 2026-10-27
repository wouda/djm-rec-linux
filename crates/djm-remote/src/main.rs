use anyhow::Result;
use djm_common::config::AppConfig;
use djm_common::protocol::default_socket_path;
use djm_remote::daemon_client::DaemonClient;
use djm_remote::hotspot::HotspotWatchdog;
use djm_remote::web_server::WebServer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("djm-remote bridge starting...");

    let config_path = AppConfig::default_config_path();
    let config = AppConfig::load_or_create(&config_path)?;
    info!("Loaded configuration with auth token: [hidden]");

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        info!("Received SIGINT. Shutting down djm-remote...");
        r.store(false, Ordering::Relaxed);
    });

    let socket_path = default_socket_path();
    let (client, telemetry_rx, command_tx) = DaemonClient::new(socket_path, running.clone());

    // Spawn IPC client connection to daemon
    tokio::spawn(async move {
        client.run().await;
    });

    // Start Hotspot Watchdog service (auto-fallback if offline)
    let hotspot = HotspotWatchdog::new(
        config.remote.hotspot_mode.clone(),
        config.remote.hotspot_ssid.clone(),
        config.remote.hotspot_password.clone(),
        running.clone(),
    );
    tokio::spawn(async move {
        hotspot.run().await;
    });

    // Start Web Server / WebSocket bridge if enabled
    if config.remote.web_enabled {
        let web = WebServer::new(
            config.remote.web_bind_addr.clone(),
            config.remote.auth_token.clone(),
            config.storage.resolved_output_dir(),
            telemetry_rx.clone(),
            command_tx.clone(),
            running.clone(),
        );
        web.run().await?;
    } else {
        while running.load(Ordering::Relaxed) {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }

    info!("djm-remote exited cleanly.");
    Ok(())
}
