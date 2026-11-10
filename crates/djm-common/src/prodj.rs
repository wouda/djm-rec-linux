use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use chrono::DateTime;
use crate::studio::UsbHandover;

pub fn default_prodj_socket_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("djm-prodj.sock")
    } else if Path::new("/run/user/1000").exists() {
        PathBuf::from("/run/user/1000/djm-prodj.sock")
    } else {
        let uid = unsafe { libc::getuid() };
        PathBuf::from(format!("/tmp/djm-prodj-{}.sock", uid))
    }
}

/// Connected Pro DJ Link Device (CDJ / DJM / XDJ)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProDjDevice {
    pub player_number: u8,
    pub device_name: String,
    pub ip_address: Ipv4Addr,
    pub mac_address: [u8; 6],
    pub model: String,
    pub firmware: String,
    pub is_mixer: bool,
    pub last_seen_ms: u64,
}

/// Real-time live status for an active CDJ deck
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProDjDeckStatus {
    pub player_number: u8,
    pub is_on_air: bool,
    pub is_playing: bool,
    pub is_master: bool,
    pub bpm: Option<f32>,
    pub pitch_percent: Option<f32>,
    pub rekordbox_id: Option<u32>,
    pub track_title: Option<String>,
    pub track_artist: Option<String>,
    pub track_key: Option<String>,
    pub usb_media_id: Option<String>,
    pub usb_media_name: Option<String>,
    pub usb_slot: Option<String>,
    pub playhead_secs: Option<f32>,
    pub duration_secs: Option<f32>,
    pub cumulative_on_air_secs: f32,
    pub degraded: bool,
}

impl Default for ProDjDeckStatus {
    fn default() -> Self {
        Self {
            player_number: 1,
            is_on_air: false,
            is_playing: false,
            is_master: false,
            bpm: None,
            pitch_percent: None,
            rekordbox_id: None,
            track_title: None,
            track_artist: None,
            track_key: None,
            usb_media_id: None,
            usb_media_name: None,
            usb_slot: None,
            playhead_secs: None,
            duration_secs: None,
            cumulative_on_air_secs: 0.0,
            degraded: true,
        }
    }
}

/// A played track logged from Pro DJ Link On-Air detection
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProDjPlayedTrack {
    pub timestamp_rfc3339: String,
    pub player_number: u8,
    pub rekordbox_id: Option<u32>,
    pub title: String,
    pub artist: String,
    pub bpm: Option<f32>,
    pub key: Option<String>,
    pub duration_secs: Option<u32>,
    pub usb_media_id: Option<String>,
    pub usb_media_name: Option<String>,
    pub cumulative_on_air_secs: f32,
}

/// Discovered USB media on a player
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProDjUsbMedia {
    pub media_id: String,
    pub media_name: String,
    pub player_number: u8,
    pub slot: String,
    pub tracks_played: u32,
    pub first_seen_rfc3339: String,
    pub last_seen_rfc3339: String,
}

/// Grouped USB media cluster (for B2B set identification)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProDjCluster {
    pub cluster_id: usize,
    pub media_ids: Vec<String>,
    pub media_names: Vec<String>,
    pub proposed_artist: Option<String>,
    pub track_count: usize,
    pub first_on_air_time: String,
    pub last_on_air_time: String,
}

/// Pro DJ Link live daemon telemetry broadcast over UNIX domain socket
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProDjTelemetry {
    pub devices: Vec<ProDjDevice>,
    pub decks: Vec<ProDjDeckStatus>,
    pub active_clusters: Vec<ProDjCluster>,
    pub virtual_player_number: Option<u8>,
    pub is_settling: bool,
    pub is_degraded: bool,
    pub active_media: Vec<ProDjUsbMedia>,
    pub network_device_count: usize,
}

impl Default for ProDjTelemetry {
    fn default() -> Self {
        Self {
            devices: Vec::new(),
            decks: Vec::new(),
            active_clusters: Vec::new(),
            virtual_player_number: None,
            is_settling: true,
            is_degraded: true,
            active_media: Vec::new(),
            network_device_count: 0,
        }
    }
}

/// A line in `<session>_prodj.jsonl`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum ProDjLogEntry {
    #[serde(rename = "device_discovered")]
    DeviceDiscovered {
        timestamp_rfc3339: String,
        device: ProDjDevice,
    },
    #[serde(rename = "device_removed")]
    DeviceRemoved {
        timestamp_rfc3339: String,
        player_number: u8,
        model: String,
    },
    #[serde(rename = "track_played")]
    TrackPlayed {
        timestamp_rfc3339: String,
        track: ProDjPlayedTrack,
    },
    #[serde(rename = "media_discovered")]
    MediaDiscovered {
        timestamp_rfc3339: String,
        media: ProDjUsbMedia,
    },
    #[serde(rename = "cluster_updated")]
    ClusterUpdated {
        timestamp_rfc3339: String,
        cluster: ProDjCluster,
    },
    #[serde(rename = "state_change")]
    StateChange {
        timestamp_rfc3339: String,
        state_event: String,
        details: String,
    },
}

/// Reads and parses all entries from a `<session>_prodj.jsonl` log file.
pub fn parse_prodj_log(path: &Path) -> std::io::Result<Vec<ProDjLogEntry>> {
    use std::io::BufRead;
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines() {
        let l = line?;
        let trimmed = l.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<ProDjLogEntry>(trimmed) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

/// Extracts USB handover transitions from ProDJ log entries.
/// A handover is detected when a different USB media identifier plays a track.
pub fn extract_usb_handovers(entries: &[ProDjLogEntry]) -> Vec<UsbHandover> {
    let mut handovers = Vec::new();
    let mut current_media_id: Option<String> = None;

    for entry in entries {
        if let ProDjLogEntry::TrackPlayed { timestamp_rfc3339, track } = entry {
            if let Some(ref m_id) = track.usb_media_id {
                let is_new = match &current_media_id {
                    Some(cur) => cur != m_id,
                    None => true,
                };
                if is_new {
                    if let Ok(ts) = DateTime::parse_from_rfc3339(timestamp_rfc3339) {
                        handovers.push(UsbHandover {
                            timestamp: ts,
                            media_id: m_id.clone(),
                            media_name: track.usb_media_name.clone().unwrap_or_else(|| m_id.clone()),
                            player_number: track.player_number,
                            track_title: Some(track.title.clone()),
                        });
                        current_media_id = Some(m_id.clone());
                    }
                }
            }
        }
    }
    handovers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prodj_log_serialization_and_handover_extraction() {
        let temp_dir = std::env::temp_dir().join(format!("test_prodj_log_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let log_file = temp_dir.join("test_session_prodj.jsonl");

        let entries = vec![
            ProDjLogEntry::DeviceDiscovered {
                timestamp_rfc3339: "2026-10-30T22:00:00+01:00".to_string(),
                device: ProDjDevice {
                    player_number: 1,
                    device_name: "CDJ-3000".to_string(),
                    ip_address: "192.168.1.10".parse().unwrap(),
                    mac_address: [0, 1, 2, 3, 4, 5],
                    model: "CDJ-3000".to_string(),
                    firmware: "3.10".to_string(),
                    is_mixer: false,
                    last_seen_ms: 100,
                },
            },
            ProDjLogEntry::TrackPlayed {
                timestamp_rfc3339: "2026-10-30T22:05:00+01:00".to_string(),
                track: ProDjPlayedTrack {
                    timestamp_rfc3339: "2026-10-30T22:05:00+01:00".to_string(),
                    player_number: 1,
                    rekordbox_id: Some(101),
                    title: "Opening Track".to_string(),
                    artist: "Artist 1".to_string(),
                    bpm: Some(124.0),
                    key: Some("8A".to_string()),
                    duration_secs: Some(360),
                    usb_media_id: Some("USB_ALICE".to_string()),
                    usb_media_name: Some("Alice Stick".to_string()),
                    cumulative_on_air_secs: 22.0,
                },
            },
            ProDjLogEntry::TrackPlayed {
                timestamp_rfc3339: "2026-10-30T22:10:00+01:00".to_string(),
                track: ProDjPlayedTrack {
                    timestamp_rfc3339: "2026-10-30T22:10:00+01:00".to_string(),
                    player_number: 1,
                    rekordbox_id: Some(102),
                    title: "Second Track".to_string(),
                    artist: "Artist 1".to_string(),
                    bpm: Some(124.0),
                    key: Some("8A".to_string()),
                    duration_secs: Some(300),
                    usb_media_id: Some("USB_ALICE".to_string()),
                    usb_media_name: Some("Alice Stick".to_string()),
                    cumulative_on_air_secs: 25.0,
                },
            },
            ProDjLogEntry::TrackPlayed {
                timestamp_rfc3339: "2026-10-30T23:00:00+01:00".to_string(),
                track: ProDjPlayedTrack {
                    timestamp_rfc3339: "2026-10-30T23:00:00+01:00".to_string(),
                    player_number: 2,
                    rekordbox_id: Some(201),
                    title: "Headliner Track".to_string(),
                    artist: "Artist 2".to_string(),
                    bpm: Some(128.0),
                    key: Some("9A".to_string()),
                    duration_secs: Some(400),
                    usb_media_id: Some("USB_BOB".to_string()),
                    usb_media_name: Some("Bob Stick".to_string()),
                    cumulative_on_air_secs: 21.0,
                },
            },
        ];

        let mut content = String::new();
        for entry in &entries {
            content.push_str(&serde_json::to_string(entry).unwrap());
            content.push('\n');
        }
        std::fs::write(&log_file, content).unwrap();

        let parsed = parse_prodj_log(&log_file).unwrap();
        assert_eq!(parsed.len(), 4);

        let handovers = extract_usb_handovers(&parsed);
        assert_eq!(handovers.len(), 2);
        assert_eq!(handovers[0].media_id, "USB_ALICE");
        assert_eq!(handovers[0].media_name, "Alice Stick");
        assert_eq!(handovers[1].media_id, "USB_BOB");
        assert_eq!(handovers[1].media_name, "Bob Stick");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

