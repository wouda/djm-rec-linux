use djm_common::protocol::{ControlCommand, Telemetry};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, watch};
use tracing::{debug, info, warn};

pub struct DaemonClient {
    socket_path: PathBuf,
    telemetry_tx: watch::Sender<Telemetry>,
    command_rx: mpsc::Receiver<ControlCommand>,
    running: Arc<AtomicBool>,
}

impl DaemonClient {
    pub fn new(
        socket_path: PathBuf,
        running: Arc<AtomicBool>,
    ) -> (Self, watch::Receiver<Telemetry>, mpsc::Sender<ControlCommand>) {
        let (telemetry_tx, telemetry_rx) = watch::channel(Telemetry::default());
        let (command_tx, command_rx) = mpsc::channel(32);

        (
            Self {
                socket_path,
                telemetry_tx,
                command_rx,
                running,
            },
            telemetry_rx,
            command_tx,
        )
    }

    pub async fn run(mut self) {
        let mut reconnect_delay = Duration::from_millis(500);
        let max_delay = Duration::from_secs(3);

        while self.running.load(Ordering::Relaxed) {
            match UnixStream::connect(&self.socket_path).await {
                Ok(stream) => {
                    info!("Connected to djm-daemon IPC socket at {}", self.socket_path.display());
                    reconnect_delay = Duration::from_millis(500);

                    let (reader, mut writer) = stream.into_split();
                    let mut lines = BufReader::new(reader).lines();
                    let telemetry_tx = self.telemetry_tx.clone();

                    let mut read_task = tokio::spawn(async move {
                        while let Ok(Some(line)) = lines.next_line().await {
                            let trimmed = line.trim();
                            if let Ok(t) = serde_json::from_str::<Telemetry>(trimmed) {
                                let _ = telemetry_tx.send(t);
                            }
                        }
                    });

                    loop {
                        tokio::select! {
                            _ = &mut read_task => {
                                warn!("IPC socket read disconnected");
                                break;
                            }
                            cmd_opt = self.command_rx.recv() => {
                                match cmd_opt {
                                    Some(cmd) => {
                                        if let Ok(json) = serde_json::to_string(&cmd) {
                                            let payload = format!("{}\n", json);
                                            if let Err(e) = writer.write_all(payload.as_bytes()).await {
                                                warn!("Failed to write command to IPC socket: {}", e);
                                                break;
                                            }
                                        }
                                    }
                                    None => return,
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    debug!("Waiting for djm-daemon socket ({}): {}", self.socket_path.display(), e);
                    tokio::time::sleep(reconnect_delay).await;
                    reconnect_delay = (reconnect_delay * 2).min(max_delay);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_daemon_client_creation() {
        let running = Arc::new(AtomicBool::new(true));
        let path = PathBuf::from("/tmp/nonexistent_test.sock");
        let (_client, tel_rx, _cmd_tx) = DaemonClient::new(path, running.clone());

        assert_eq!(tel_rx.borrow().clip_count, 0);
        running.store(false, Ordering::Relaxed);
    }
}
