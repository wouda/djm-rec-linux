use anyhow::{Context, Result};
use djm_common::protocol::{ControlCommand, Telemetry};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info, warn};

pub struct IpcServer {
    socket_path: PathBuf,
    telemetry_tx: broadcast::Sender<Telemetry>,
    command_tx: mpsc::Sender<ControlCommand>,
    running: Arc<AtomicBool>,
}

impl IpcServer {
    pub const DEFAULT_BROADCAST_CAPACITY: usize = 128;

    pub fn default_socket_path() -> PathBuf {
        if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
            PathBuf::from(runtime_dir).join("djm-box.sock")
        } else {
            let uid = unsafe { nix::libc::getuid() };
            PathBuf::from(format!("/tmp/djm-box-{}.sock", uid))
        }
    }

    pub fn new(
        socket_path: PathBuf,
        command_tx: mpsc::Sender<ControlCommand>,
        running: Arc<AtomicBool>,
    ) -> (Self, broadcast::Sender<Telemetry>) {
        let (telemetry_tx, _) = broadcast::channel(Self::DEFAULT_BROADCAST_CAPACITY);
        (
            Self {
                socket_path,
                telemetry_tx: telemetry_tx.clone(),
                command_tx,
                running,
            },
            telemetry_tx,
        )
    }

    pub async fn run(&self) -> Result<()> {
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }

        if let Some(parent) = self.socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let listener = UnixListener::bind(&self.socket_path)
            .with_context(|| format!("Failed to bind Unix socket at {}", self.socket_path.display()))?;

        info!("IPC Unix domain socket listening at {}", self.socket_path.display());

        while self.running.load(Ordering::Relaxed) {
            tokio::select! {
                accept_res = listener.accept() => {
                    match accept_res {
                        Ok((stream, _addr)) => {
                            debug!("New client connected to IPC socket");
                            let telemetry_rx = self.telemetry_tx.subscribe();
                            let cmd_tx = self.command_tx.clone();
                            let running = self.running.clone();

                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_client(stream, telemetry_rx, cmd_tx, running).await {
                                    debug!("IPC client disconnected: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            warn!("IPC accept error: {}", e);
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
        info!("IPC Unix domain socket shut down cleanly.");
        Ok(())
    }

    async fn handle_client(
        stream: UnixStream,
        mut telemetry_rx: broadcast::Receiver<Telemetry>,
        cmd_tx: mpsc::Sender<ControlCommand>,
        running: Arc<AtomicBool>,
    ) -> Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();

        let write_task = async move {
            while running.load(Ordering::Relaxed) {
                match telemetry_rx.recv().await {
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
                        // Lag-drop semantics: client fell behind, drop frames and continue seamlessly
                        debug!("Dropped {} lagged telemetry frames for client", dropped);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
            Ok::<(), anyhow::Error>(())
        };

        let read_task = async move {
            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // Try JSON deserialization first
                if let Ok(cmd) = serde_json::from_str::<ControlCommand>(trimmed) {
                    let _ = cmd_tx.send(cmd).await;
                }
            }
            Ok::<(), anyhow::Error>(())
        };

        tokio::select! {
            r = write_task => r,
            r = read_task => r,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn test_ipc_command_and_telemetry() {
        let tmp_sock = std::env::temp_dir().join(format!("test_ipc_{}.sock", std::process::id()));
        let (cmd_tx, mut cmd_rx) = mpsc::channel(16);
        let running = Arc::new(AtomicBool::new(true));

        let (server, telemetry_tx) = IpcServer::new(tmp_sock.clone(), cmd_tx, running.clone());

        tokio::spawn(async move {
            let _ = server.run().await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Connect client
        let mut client = UnixStream::connect(&tmp_sock).await.expect("failed to connect");

        // Send a control command from client
        let cmd = ControlCommand::DropBookmark(Some("Test Drop".to_string()));
        let cmd_json = serde_json::to_string(&cmd).unwrap();
        client.write_all(format!("{}\n", cmd_json).as_bytes()).await.unwrap();

        // Verify daemon receives it
        let received = tokio::time::timeout(Duration::from_millis(500), cmd_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(received, cmd);

        // Send telemetry from daemon
        let t = Telemetry {
            clip_count: 5,
            ..Default::default()
        };
        telemetry_tx.send(t).unwrap();

        let mut reader = BufReader::new(client).lines();
        let frame_line = tokio::time::timeout(Duration::from_millis(500), reader.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let received_t: Telemetry = serde_json::from_str(&frame_line).unwrap();
        assert_eq!(received_t.clip_count, 5);

        running.store(false, Ordering::Relaxed);
        let _ = std::fs::remove_file(&tmp_sock);
    }
}
