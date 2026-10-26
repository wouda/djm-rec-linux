use crate::ui::draw_ui;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use djm_common::config::AppConfig;
use djm_common::protocol::{ControlCommand, HotspotControl, PairingInfo, Telemetry};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use tracing::info;

pub struct TuiApp {
    socket_path: PathBuf,
    config: AppConfig,
}

impl TuiApp {
    pub fn new(socket_path: PathBuf, config: AppConfig) -> Self {
        Self { socket_path, config }
    }

    fn make_pairing_info(&self) -> PairingInfo {
        let hotspot_active = HotspotControl::is_hotspot_active();
        PairingInfo::auto_detect(
            "DJM-Box".to_string(),
            self.config.remote.auth_token.clone(),
            if !self.config.remote.remote_public_url.is_empty() {
                Some(self.config.remote.remote_public_url.clone())
            } else {
                None
            },
            8080,
            self.config.remote.hotspot_ssid.clone(),
            self.config.remote.hotspot_password.clone(),
            hotspot_active,
        )
    }

    pub async fn run(&mut self) -> Result<()> {
        let stream = match UnixStream::connect(&self.socket_path).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "\n=== djm-tui Connection Error ===\nCould not connect to djm-daemon socket at {}: {}\n\nThe core audio engine is not currently running.\nPlease start it first:\n  djm-box daemon --mock    (test mode with continuous simulated audio)\n  djm-box daemon           (recording mode for Pioneer/AlphaTheta mixers)\n  systemctl --user start djm-box-daemon\n",
                    self.socket_path.display(),
                    e
                );
                return Err(e.into());
            }
        };

        let (reader, mut writer) = stream.into_split();
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<ControlCommand>(32);

        // Outbound command writer task
        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                if let Ok(json) = serde_json::to_string(&cmd) {
                    let line = format!("{}\n", json);
                    let _ = writer.write_all(line.as_bytes()).await;
                }
            }
        });

        // Inbound telemetry reader task
        let (telemetry_tx, telemetry_rx) = tokio::sync::watch::channel(Telemetry::default());
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim();
                if let Ok(t) = serde_json::from_str::<Telemetry>(trimmed) {
                    let _ = telemetry_tx.send(t);
                }
            }
        });

        // Setup terminal
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend)?;

        let mut pairing_info = self.make_pairing_info();
        let mut show_pairing = false;
        let mut show_wifi_qr = false;
        let mut status_msg: Option<String> = None;
        let mut status_expire = std::time::Instant::now();

        let tick_rate = Duration::from_millis(25); // 40 FPS

        loop {
            let current_telemetry = telemetry_rx.borrow().clone();

            if status_expire <= std::time::Instant::now() {
                status_msg = None;
            }

            terminal.draw(|f| {
                draw_ui(
                    f,
                    &current_telemetry,
                    &pairing_info,
                    show_pairing,
                    show_wifi_qr,
                    status_msg.as_deref(),
                );
            })?;

            if event::poll(tick_rate)? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                        KeyCode::Char('p') | KeyCode::Char('P') => {
                            show_pairing = !show_pairing;
                            if show_pairing {
                                pairing_info = self.make_pairing_info();
                                let _ = cmd_tx.send(ControlCommand::OpenPairingWindow).await;
                                status_msg = Some("Phone pairing modal opened (QR code refreshed)".to_string());
                                status_expire = std::time::Instant::now() + Duration::from_secs(5);
                            }
                        }
                        KeyCode::Tab | KeyCode::Char('w') | KeyCode::Char('W') if show_pairing => {
                            show_wifi_qr = !show_wifi_qr;
                            let mode_desc = if show_wifi_qr { "Wi-Fi Hotspot" } else { "Web Dashboard" };
                            status_msg = Some(format!("Switched to Step {} QR: {}", if show_wifi_qr { "1" } else { "2" }, mode_desc));
                            status_expire = std::time::Instant::now() + Duration::from_secs(3);
                        }
                        KeyCode::Char('h') | KeyCode::Char('H') => {
                            if HotspotControl::is_hotspot_active() {
                                match HotspotControl::stop_hotspot() {
                                    Ok(_) => {
                                        status_msg = Some("Local Wi-Fi Hotspot stopped".to_string());
                                    }
                                    Err(e) => {
                                        status_msg = Some(format!("Failed to stop hotspot: {}", e));
                                    }
                                }
                            } else {
                                match HotspotControl::start_hotspot(&self.config.remote.hotspot_ssid, &self.config.remote.hotspot_password) {
                                    Ok(true) => {
                                        status_msg = Some(format!("✓ Wi-Fi Hotspot started ('{}' on 10.42.0.1)", self.config.remote.hotspot_ssid));
                                    }
                                    Ok(false) => {
                                        status_msg = Some("Failed to start Wi-Fi Hotspot (check nmcli/hardware)".to_string());
                                    }
                                    Err(e) => {
                                        status_msg = Some(format!("Hotspot error: {}", e));
                                    }
                                }
                            }
                            pairing_info = self.make_pairing_info();
                            status_expire = std::time::Instant::now() + Duration::from_secs(4);
                        }
                        KeyCode::Esc => {
                            show_pairing = false;
                        }
                        KeyCode::Char('s') | KeyCode::Char('S') => {
                            let _ = cmd_tx.send(ControlCommand::SplitFile).await;
                            status_msg = Some("Requested gapless file split".to_string());
                            status_expire = std::time::Instant::now() + Duration::from_secs(3);
                        }
                        KeyCode::Char('m') | KeyCode::Char('M') => {
                            let _ = cmd_tx.send(ControlCommand::DropBookmark(None)).await;
                            status_msg = Some("Dropped bookmark in session log".to_string());
                            status_expire = std::time::Instant::now() + Duration::from_secs(3);
                        }
                        KeyCode::Char('[') => {
                            // Step pad down (-5 -> -10 -> -15 -> -19)
                            let current = current_telemetry.hw_pad_db;
                            let next = match current {
                                -5 => -10,
                                -10 => -15,
                                _ => -19,
                            };
                            let _ = cmd_tx.send(ControlCommand::SetHardwarePad(next)).await;
                            status_msg = Some(format!("Set pad to {} dB", next));
                            status_expire = std::time::Instant::now() + Duration::from_secs(3);
                        }
                        KeyCode::Char(']') => {
                            // Step pad up (-19 -> -15 -> -10 -> -5)
                            let current = current_telemetry.hw_pad_db;
                            let next = match current {
                                -19 => -15,
                                -15 => -10,
                                _ => -5,
                            };
                            let _ = cmd_tx.send(ControlCommand::SetHardwarePad(next)).await;
                            status_msg = Some(format!("Set pad to {} dB", next));
                            status_expire = std::time::Instant::now() + Duration::from_secs(3);
                        }
                        KeyCode::Char('a') | KeyCode::Char('A') => {
                            let _ = cmd_tx.send(ControlCommand::ToggleAutoGuard).await;
                            status_msg = Some("Toggled Auto-Headroom Guard".to_string());
                            status_expire = std::time::Instant::now() + Duration::from_secs(3);
                        }
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            let _ = cmd_tx.send(ControlCommand::ResetPeaks).await;
                            status_msg = Some("Reset peak meters and clip counter".to_string());
                            status_expire = std::time::Instant::now() + Duration::from_secs(3);
                        }
                        _ => {}
                    }
                }
            }
        }

        // Restore terminal
        disable_raw_mode()?;
        stdout().execute(LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        info!("TUI exited. Recording daemon continues uninterrupted.");
        Ok(())
    }
}
