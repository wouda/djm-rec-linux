use djm_common::prodj::ProDjTelemetry;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::watch;
use tracing::{debug, info};

pub struct ProDjClient {
    socket_path: PathBuf,
    prodj_tx: watch::Sender<Option<ProDjTelemetry>>,
    running: Arc<AtomicBool>,
}

impl ProDjClient {
    pub fn new(
        socket_path: PathBuf,
        running: Arc<AtomicBool>,
    ) -> (Self, watch::Receiver<Option<ProDjTelemetry>>) {
        let (prodj_tx, prodj_rx) = watch::channel(None);
        (
            Self {
                socket_path,
                prodj_tx,
                running,
            },
            prodj_rx,
        )
    }

    pub async fn run(self) {
        let mut reconnect_delay = Duration::from_millis(500);
        let max_delay = Duration::from_secs(3);

        while self.running.load(Ordering::Relaxed) {
            match UnixStream::connect(&self.socket_path).await {
                Ok(stream) => {
                    info!("Connected to Pro DJ Link socket at {}", self.socket_path.display());
                    reconnect_delay = Duration::from_millis(500);

                    let (reader, _) = stream.into_split();
                    let mut lines = BufReader::new(reader).lines();

                    while self.running.load(Ordering::Relaxed) {
                        match lines.next_line().await {
                            Ok(Some(line)) => {
                                let trimmed = line.trim();
                                if let Ok(t) = serde_json::from_str::<ProDjTelemetry>(trimmed) {
                                    let _ = self.prodj_tx.send(Some(t));
                                }
                            }
                            Ok(None) => {
                                debug!("Pro DJ Link socket EOF");
                                break;
                            }
                            Err(e) => {
                                debug!("Pro DJ Link socket read error: {}", e);
                                break;
                            }
                        }
                    }
                    let _ = self.prodj_tx.send(None);
                }
                Err(e) => {
                    debug!("Waiting for Pro DJ Link socket ({}): {}", self.socket_path.display(), e);
                }
            }

            tokio::time::sleep(reconnect_delay).await;
            reconnect_delay = (reconnect_delay * 2).min(max_delay);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_prodj_client_creation() {
        let running = Arc::new(AtomicBool::new(false));
        let path = PathBuf::from("/tmp/nonexistent_prodj.sock");
        let (_client, rx) = ProDjClient::new(path, running);
        assert_eq!(*rx.borrow(), None);
    }
}
