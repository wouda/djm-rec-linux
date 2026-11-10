use anyhow::Result;
use chrono::Utc;
use djm_common::prodj::{
    default_prodj_socket_path, ProDjDeckStatus, ProDjLogEntry, ProDjTelemetry,
};
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tracing::{error, info, warn};

use crate::broadcaster::ProDjBroadcaster;
use crate::metadata::MetadataResolver;
use crate::protocol::{
    has_valid_magic, parse_announce_packet, parse_cdj_status_packet, parse_mixer_status_packet,
    DEFAULT_ANNOUNCE_PORT, DEFAULT_STATUS_PORT,
};
use crate::session_logger::SessionJsonlLogger;
use crate::state_machine::{ParticipationConfig, ParticipationStateMachine, StateMachineAction};
use crate::tracker::{B2BClusterer, DeckTracker};

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub socket_path: PathBuf,
    pub session_log: Option<PathBuf>,
    pub mock: bool,
    pub bind_address: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            socket_path: default_prodj_socket_path(),
            session_log: None,
            mock: false,
            bind_address: "0.0.0.0".to_string(),
        }
    }
}

pub struct ProDjDaemon {
    config: DaemonConfig,
    running: Arc<AtomicBool>,
}

impl ProDjDaemon {
    pub fn new(config: DaemonConfig, running: Arc<AtomicBool>) -> Self {
        Self { config, running }
    }

    pub async fn run(self) -> Result<()> {
        info!("Starting Pro DJ Link Daemon (djm-prodj)...");

        // 1. Broadcaster setup
        let (broadcaster, tx, latest_telem) =
            ProDjBroadcaster::new(self.config.socket_path.clone(), self.running.clone());

        let b_handle = tokio::spawn(async move {
            if let Err(e) = broadcaster.run().await {
                error!("Broadcaster error: {}", e);
            }
        });

        // 2. Session Logger setup
        let mut logger = SessionJsonlLogger::new();
        if let Some(ref log_path) = self.config.session_log {
            if let Err(e) = logger.open(log_path) {
                warn!("Failed to open session log at {}: {}", log_path.display(), e);
            }
        }

        // 3. Components
        let mut sm = ParticipationStateMachine::new(ParticipationConfig::default());
        let mut tracker = DeckTracker::new();
        let resolver = MetadataResolver::new();

        // 4. UDP sockets (ports 50000 for announce, 50002 for status)
        let announce_sock = if !self.config.mock {
            match UdpSocket::bind(format!("{}:{}", self.config.bind_address, DEFAULT_ANNOUNCE_PORT)).await {
                Ok(s) => {
                    info!("Listening for Pro DJ Link announcements on UDP {}", DEFAULT_ANNOUNCE_PORT);
                    Some(s)
                }
                Err(e) => {
                    warn!("Could not bind UDP announce port {}: {} (running passive)", DEFAULT_ANNOUNCE_PORT, e);
                    None
                }
            }
        } else {
            None
        };

        let status_sock = if !self.config.mock {
            match UdpSocket::bind(format!("{}:{}", self.config.bind_address, DEFAULT_STATUS_PORT)).await {
                Ok(s) => {
                    info!("Listening for Pro DJ Link status on UDP {}", DEFAULT_STATUS_PORT);
                    Some(s)
                }
                Err(e) => {
                    warn!("Could not bind UDP status port {}: {} (running passive)", DEFAULT_STATUS_PORT, e);
                    None
                }
            }
        } else {
            None
        };

        let mut announce_buf = vec![0u8; 2048];
        let mut status_buf = vec![0u8; 2048];
        let mut tick_interval = tokio::time::interval(Duration::from_millis(250));

        let start_time = std::time::Instant::now();

        info!("Pro DJ Link Daemon initialized and active.");

        while self.running.load(Ordering::Relaxed) {
            tokio::select! {
                _ = tick_interval.tick() => {
                    let now_ms = start_time.elapsed().as_millis() as u64;
                    let now_dt: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();

                    // Run state machine tick
                    let sm_action = sm.tick(now_ms);
                    match sm_action {
                        StateMachineAction::ClaimSlot(slot) => {
                            info!("Pro DJ Link: Claimed virtual player slot {}", slot);
                            let entry = ProDjLogEntry::StateChange {
                                timestamp_rfc3339: now_dt.to_rfc3339(),
                                state_event: "slot_claimed".to_string(),
                                details: format!("Claimed virtual player slot {}", slot),
                            };
                            let _ = logger.log_entry(&entry);
                        }
                        StateMachineAction::ReleasedVirtualSlot { player_number, reason } => {
                            info!("Pro DJ Link: Released virtual player slot {}: {}", player_number, reason);
                            let entry = ProDjLogEntry::StateChange {
                                timestamp_rfc3339: now_dt.to_rfc3339(),
                                state_event: "slot_released".to_string(),
                                details: format!("Released slot {}: {}", player_number, reason),
                            };
                            let _ = logger.log_entry(&entry);
                        }
                        StateMachineAction::BroadcastKeepalive { player_number: _ } => {
                            // Virtual player keepalive (if sending enabled)
                        }
                        StateMachineAction::None => {}
                    }

                    // Build telemetry snapshot
                    let devices = sm.inventory.get_devices();
                    let mut decks = Vec::new();
                    for (&p_num, deck) in &tracker.decks {
                        decks.push(ProDjDeckStatus {
                            player_number: p_num,
                            is_on_air: deck.is_on_air,
                            is_playing: deck.is_playing,
                            is_master: false,
                            bpm: None,
                            pitch_percent: None,
                            rekordbox_id: deck.current_track_id,
                            track_title: None,
                            track_artist: None,
                            track_key: None,
                            usb_media_id: deck.current_media_id.clone(),
                            usb_media_name: deck.current_media_name.clone(),
                            usb_slot: None,
                            playhead_secs: Some(deck.cumulative_playhead_secs),
                            duration_secs: None,
                            cumulative_on_air_secs: deck.cumulative_on_air_secs,
                            degraded: resolver.is_degraded(p_num),
                        });
                    }

                    let clusters = B2BClusterer::cluster_played_tracks(&tracker.played_tracks, 1800.0);

                    let is_overall_degraded = resolver.databases.is_empty()
                        || tracker.decks.keys().any(|&p| resolver.is_degraded(p));

                    let telem = ProDjTelemetry {
                        devices: devices.clone(),
                        decks,
                        active_clusters: clusters,
                        virtual_player_number: sm.claimed_player_number(),
                        is_settling: sm.is_settling(),
                        is_degraded: is_overall_degraded,
                        active_media: Vec::new(),
                        network_device_count: devices.len(),
                    };

                    {
                        let mut w = latest_telem.write().await;
                        *w = telem.clone();
                    }
                    let _ = tx.send(telem);
                }

                res = async {
                    match &announce_sock {
                        Some(sock) => sock.recv_from(&mut announce_buf).await,
                        None => std::future::pending().await,
                    }
                } => {
                    if let Ok((len, src)) = res {
                        let packet = &announce_buf[..len];
                        let now_ms = start_time.elapsed().as_millis() as u64;
                        let now_dt: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();
                        let ipv4 = match src.ip() {
                            IpAddr::V4(v4) => v4,
                            IpAddr::V6(_) => Ipv4Addr::UNSPECIFIED,
                        };

                        if has_valid_magic(packet) {
                            if let Some(dev) = parse_announce_packet(packet, ipv4) {
                                let newly_discovered = sm.inventory.update_device(dev.clone(), now_ms);
                                if newly_discovered {
                                    info!("Pro DJ Link: Discovered device {} (Player {}) at {}", dev.device_name, dev.player_number, dev.ip_address);
                                    let entry = ProDjLogEntry::DeviceDiscovered {
                                        timestamp_rfc3339: now_dt.to_rfc3339(),
                                        device: dev.into(),
                                    };
                                    let _ = logger.log_entry(&entry);
                                }
                                sm.on_packet_received(packet, ipv4, now_ms);
                            }
                        }
                    }
                }

                res = async {
                    match &status_sock {
                        Some(sock) => sock.recv_from(&mut status_buf).await,
                        None => std::future::pending().await,
                    }
                } => {
                    if let Ok((len, src)) = res {
                        let packet = &status_buf[..len];
                        let now_ms = start_time.elapsed().as_millis() as u64;
                        let now_dt: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();
                        let ipv4 = match src.ip() {
                            IpAddr::V4(v4) => v4,
                            IpAddr::V6(_) => Ipv4Addr::UNSPECIFIED,
                        };

                        if has_valid_magic(packet) {
                            sm.on_packet_received(packet, ipv4, now_ms);

                            // Check for mixer status
                            if let Some(mixer_stat) = parse_mixer_status_packet(packet) {
                                tracker.update_mixer_status(&mixer_stat);
                            }

                            // Check for CDJ status
                            if let Some(cdj_stat) = parse_cdj_status_packet(packet) {
                                let metadata = if cdj_stat.rekordbox_track_id > 0 {
                                    resolver.resolve_track(cdj_stat.player_number, cdj_stat.rekordbox_track_id)
                                } else {
                                    None
                                };
                                let (title, artist, bpm, key, duration) = match metadata {
                                    Some(meta) => (
                                        Some(meta.title),
                                        Some(meta.artist),
                                        meta.bpm,
                                        meta.key,
                                        meta.duration_secs,
                                    ),
                                    None => (None, None, None, None, None),
                                };

                                let played = tracker.update_deck_status(
                                    &cdj_stat,
                                    None,
                                    None,
                                    title,
                                    artist,
                                    bpm,
                                    key,
                                    duration,
                                    now_dt,
                                    0.25, // estimated delta secs between status packets
                                );

                                if let Some(track) = played {
                                    info!("Pro DJ Link: Track Played: {} - {} on Player {}", track.artist, track.title, track.player_number);
                                    let entry = ProDjLogEntry::TrackPlayed {
                                        timestamp_rfc3339: now_dt.to_rfc3339(),
                                        track,
                                    };
                                    let _ = logger.log_entry(&entry);
                                }
                            }
                        }
                    }
                }
            }
        }

        info!("Shutting down Pro DJ Link Daemon...");
        logger.close();
        let _ = b_handle.await;
        Ok(())
    }
}
