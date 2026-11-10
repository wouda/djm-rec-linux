use anyhow::{Context, Result};
use djm_common::prodj::ProDjTelemetry;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn};

pub struct ProDjBroadcaster {
    socket_path: PathBuf,
    telemetry_tx: broadcast::Sender<ProDjTelemetry>,
    latest_telemetry: Arc<RwLock<ProDjTelemetry>>,
    running: Arc<AtomicBool>,
}

impl ProDjBroadcaster {
    pub const DEFAULT_CAPACITY: usize = 128;

    pub fn new(
        socket_path: PathBuf,
        running: Arc<AtomicBool>,
    ) -> (
        Self,
        broadcast::Sender<ProDjTelemetry>,
        Arc<RwLock<ProDjTelemetry>>,
    ) {
        let (telemetry_tx, _) = broadcast::channel(Self::DEFAULT_CAPACITY);
        let latest_telemetry = Arc::new(RwLock::new(ProDjTelemetry::default()));
        let broadcaster = Self {
            socket_path,
            telemetry_tx: telemetry_tx.clone(),
            latest_telemetry: latest_telemetry.clone(),
            running,
        };
        (broadcaster, telemetry_tx, latest_telemetry)
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub async fn run(&self) -> Result<()> {
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }

        if let Some(parent) = self.socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let listener = UnixListener::bind(&self.socket_path).with_context(|| {
            format!(
                "Failed to bind Pro DJ Link socket at {}",
                self.socket_path.display()
            )
        })?;

        info!(
            "Pro DJ Link broadcaster listening at {}",
            self.socket_path.display()
        );

        while self.running.load(Ordering::Relaxed) {
            tokio::select! {
                accept_res = listener.accept() => {
                    match accept_res {
                        Ok((stream, _addr)) => {
                            debug!("New client connected to Pro DJ Link socket");
                            let rx = self.telemetry_tx.subscribe();
                            let latest = self.latest_telemetry.clone();
                            let running = self.running.clone();

                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_client(stream, rx, latest, running).await {
                                    debug!("Pro DJ Link client disconnected: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            warn!("Pro DJ Link accept error: {}", e);
                        }
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(200)) => {
                    if !self.running.load(Ordering::Relaxed) {
                        break;
                    }
                }
            }
        }

        let _ = std::fs::remove_file(&self.socket_path);
        info!("Pro DJ Link broadcaster socket removed.");
        Ok(())
    }

    async fn handle_client(
        stream: UnixStream,
        mut rx: broadcast::Receiver<ProDjTelemetry>,
        latest: Arc<RwLock<ProDjTelemetry>>,
        running: Arc<AtomicBool>,
    ) -> Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();

        // 1. Immediately send current telemetry snapshot
        {
            let snapshot = latest.read().await;
            if let Ok(json) = serde_json::to_string(&*snapshot) {
                let payload = format!("{}\n", json);
                writer.write_all(payload.as_bytes()).await?;
            }
        }

        let write_task = async move {
            while running.load(Ordering::Relaxed) {
                match rx.recv().await {
                    Ok(telemetry) => {
                        let json = match serde_json::to_string(&telemetry) {
                            Ok(j) => j,
                            Err(_) => continue,
                        };
                        let line = format!("{}\n", json);
                        if let Err(e) = writer.write_all(line.as_bytes()).await {
                            return Err(anyhow::anyhow!("Socket write error: {}", e));
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(dropped)) => {
                        debug!("Dropped {} lagged ProDJ telemetry frames for client", dropped);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
            Ok::<(), anyhow::Error>(())
        };

        let read_task = async move {
            while let Ok(Some(_line)) = lines.next_line().await {
                // Currently ProDJ socket is primarily a telemetry broadcaster.
                // Ignore any client ping/input without dropping the connection.
            }
            Ok::<(), anyhow::Error>(())
        };

        tokio::select! {
            r = write_task => r,
            r = read_task => r,
        }
    }
}
