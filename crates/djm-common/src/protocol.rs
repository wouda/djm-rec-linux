use byteorder::{ByteOrder, LittleEndian};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

pub fn default_socket_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("djm-box.sock")
    } else {
        PathBuf::from("/tmp/djm-box.sock")
    }
}

#[derive(Error, Debug, PartialEq)]
pub enum ProtocolError {
    #[error("Invalid payload length: expected {expected}, got {actual}")]
    InvalidLength { expected: usize, actual: usize },
    #[error("Invalid state byte: {0}")]
    InvalidState(u8),
    #[error("Unknown command opcode: {0:#04x}")]
    UnknownOpcode(u8),
    #[error("Missing or invalid token")]
    InvalidToken,
    #[error("Serialization error: {0}")]
    Serialization(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum RecordingState {
    Disconnected = 0,
    ArmedSilent = 1,
    Recording = 2,
    Paused = 3,
}

impl TryFrom<u8> for RecordingState {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(RecordingState::Disconnected),
            1 => Ok(RecordingState::ArmedSilent),
            2 => Ok(RecordingState::Recording),
            3 => Ok(RecordingState::Paused),
            other => Err(ProtocolError::InvalidState(other)),
        }
    }
}

/// 20-Byte Compact Telemetry Packet for BLE GATT and High-Rate Streaming
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Telemetry {
    pub state: RecordingState,
    pub hw_pad_db: i8,
    pub auto_guard_active: bool,
    pub silence_countdown_active: bool,
    pub peak_l_centi_db: i16,
    pub peak_r_centi_db: i16,
    pub rms_l_centi_db: i16,
    pub rms_r_centi_db: i16,
    pub clip_count: u16,
    pub active_file_duration_secs: u32,
    pub active_file_size_mb: u16,
    pub free_disk_gb_tenths: u16,

    // Extended fields (serialized in JSON, omitted from 20-byte BLE)
    #[serde(default)]
    pub mixer_model: String,
    #[serde(default)]
    pub sample_rate: u32,
    #[serde(default)]
    pub active_filename: Option<String>,
    #[serde(default)]
    pub pause_countdown_secs: Option<f32>,
    #[serde(default)]
    pub recent_logs: Vec<String>,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self {
            state: RecordingState::Disconnected,
            hw_pad_db: -19,
            auto_guard_active: true,
            silence_countdown_active: false,
            peak_l_centi_db: -9600,
            peak_r_centi_db: -9600,
            rms_l_centi_db: -9600,
            rms_r_centi_db: -9600,
            clip_count: 0,
            active_file_duration_secs: 0,
            active_file_size_mb: 0,
            free_disk_gb_tenths: 0,
            mixer_model: "No Mixer".to_string(),
            sample_rate: 0,
            active_filename: None,
            pause_countdown_secs: None,
            recent_logs: Vec::new(),
        }
    }
}

pub const TELEMETRY_PACKED_SIZE: usize = 20;

impl Telemetry {
    /// Returns true if a DJM mixer is currently detected and plugged in.
    pub fn is_mixer_connected(&self) -> bool {
        self.state != RecordingState::Disconnected && !self.mixer_model.is_empty() && self.mixer_model != "No Mixer"
    }

    /// Returns true if audio is actively streaming and being recorded to disk.
    pub fn is_recording_to_disk(&self) -> bool {
        self.state == RecordingState::Recording
    }

    pub fn to_packed_bytes(&self) -> [u8; TELEMETRY_PACKED_SIZE] {
        let mut buf = [0u8; TELEMETRY_PACKED_SIZE];

        // Byte 0: State
        buf[0] = self.state as u8;

        // Byte 1: hw_pad_db (-128..127) packed with bit 6 (auto_guard) and bit 7 (silence_countdown)
        // hw_pad_db is -19, -15, -10, -5 (fits in lower 6 bits with signed 6-bit mapping: val & 0x3F)
        let pad_bits = (self.hw_pad_db as u8) & 0x3F;
        let guard_bit = if self.auto_guard_active { 0x40 } else { 0x00 };
        let silence_bit = if self.silence_countdown_active { 0x80 } else { 0x00 };
        buf[1] = pad_bits | guard_bit | silence_bit;

        // Bytes 2..9: Audio levels (centi-dBFS, e.g. -1425 = -14.25 dBFS)
        LittleEndian::write_i16(&mut buf[2..4], self.peak_l_centi_db);
        LittleEndian::write_i16(&mut buf[4..6], self.peak_r_centi_db);
        LittleEndian::write_i16(&mut buf[6..8], self.rms_l_centi_db);
        LittleEndian::write_i16(&mut buf[8..10], self.rms_r_centi_db);

        // Bytes 10..11: Clip count
        LittleEndian::write_u16(&mut buf[10..12], self.clip_count);

        // Bytes 12..15: Active file duration (seconds)
        LittleEndian::write_u32(&mut buf[12..16], self.active_file_duration_secs);

        // Bytes 16..17: File size (MB)
        LittleEndian::write_u16(&mut buf[16..18], self.active_file_size_mb);

        // Bytes 18..19: Free disk (tenths of GB, e.g. 1482 = 148.2 GB)
        LittleEndian::write_u16(&mut buf[18..20], self.free_disk_gb_tenths);

        buf
    }

    pub fn from_packed_bytes(buf: &[u8]) -> Result<Self, ProtocolError> {
        if buf.len() != TELEMETRY_PACKED_SIZE {
            return Err(ProtocolError::InvalidLength {
                expected: TELEMETRY_PACKED_SIZE,
                actual: buf.len(),
            });
        }

        let state = RecordingState::try_from(buf[0])?;

        let b1 = buf[1];
        let auto_guard_active = (b1 & 0x40) != 0;
        let silence_countdown_active = (b1 & 0x80) != 0;

        // Sign extend 6-bit pad value
        let raw_pad = b1 & 0x3F;
        let hw_pad_db = if (raw_pad & 0x20) != 0 {
            (raw_pad | 0xC0) as i8
        } else {
            raw_pad as i8
        };

        let peak_l_centi_db = LittleEndian::read_i16(&buf[2..4]);
        let peak_r_centi_db = LittleEndian::read_i16(&buf[4..6]);
        let rms_l_centi_db = LittleEndian::read_i16(&buf[6..8]);
        let rms_r_centi_db = LittleEndian::read_i16(&buf[8..10]);

        let clip_count = LittleEndian::read_u16(&buf[10..12]);
        let active_file_duration_secs = LittleEndian::read_u32(&buf[12..16]);
        let active_file_size_mb = LittleEndian::read_u16(&buf[16..18]);
        let free_disk_gb_tenths = LittleEndian::read_u16(&buf[18..20]);

        Ok(Self {
            state,
            hw_pad_db,
            auto_guard_active,
            silence_countdown_active,
            peak_l_centi_db,
            peak_r_centi_db,
            rms_l_centi_db,
            rms_r_centi_db,
            clip_count,
            active_file_duration_secs,
            active_file_size_mb,
            free_disk_gb_tenths,
            mixer_model: String::new(),
            sample_rate: 0,
            active_filename: None,
            pause_countdown_secs: None,
            recent_logs: Vec::new(),
        })
    }
}

/// Commands accepted by the recording daemon and remote bridge
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlCommand {
    /// 0x00: Authenticate session with secret token
    Auth(String),
    /// 0x01: Drop timestamped bookmark note in session log
    DropBookmark(Option<String>),
    /// 0x02: Force immediate gapless file split
    SplitFile,
    /// 0x03: Set hardware capture pad (-19, -15, -10, -5 dB)
    SetHardwarePad(i8),
    /// 0x04: Toggle auto headroom guard
    ToggleAutoGuard,
    /// 0x05: Reset peak levels and clip counters
    ResetPeaks,
    /// 0x06: Open phone pairing window for N seconds
    OpenPairingWindow,
    /// 0x07: Toggle local Wi-Fi Hotspot on/off
    ToggleHotspot,
}

impl ControlCommand {
    pub const OPCODE_AUTH: u8 = 0x00;
    pub const OPCODE_BOOKMARK: u8 = 0x01;
    pub const OPCODE_SPLIT: u8 = 0x02;
    pub const OPCODE_SET_PAD: u8 = 0x03;
    pub const OPCODE_TOGGLE_GUARD: u8 = 0x04;
    pub const OPCODE_RESET_PEAKS: u8 = 0x05;
    pub const OPCODE_PAIRING_WINDOW: u8 = 0x06;
    pub const OPCODE_TOGGLE_HOTSPOT: u8 = 0x07;

    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            ControlCommand::Auth(token) => {
                let mut v = vec![Self::OPCODE_AUTH];
                v.extend_from_slice(token.as_bytes());
                v
            }
            ControlCommand::DropBookmark(Some(note)) => {
                let mut v = vec![Self::OPCODE_BOOKMARK];
                v.extend_from_slice(note.as_bytes());
                v
            }
            ControlCommand::DropBookmark(None) => vec![Self::OPCODE_BOOKMARK],
            ControlCommand::SplitFile => vec![Self::OPCODE_SPLIT],
            ControlCommand::SetHardwarePad(pad) => vec![Self::OPCODE_SET_PAD, *pad as u8],
            ControlCommand::ToggleAutoGuard => vec![Self::OPCODE_TOGGLE_GUARD],
            ControlCommand::ResetPeaks => vec![Self::OPCODE_RESET_PEAKS],
            ControlCommand::OpenPairingWindow => vec![Self::OPCODE_PAIRING_WINDOW],
            ControlCommand::ToggleHotspot => vec![Self::OPCODE_TOGGLE_HOTSPOT],
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.is_empty() {
            return Err(ProtocolError::InvalidLength { expected: 1, actual: 0 });
        }
        match bytes[0] {
            Self::OPCODE_AUTH => {
                let token = String::from_utf8_lossy(&bytes[1..]).trim().to_string();
                if token.is_empty() {
                    Err(ProtocolError::InvalidToken)
                } else {
                    Ok(ControlCommand::Auth(token))
                }
            }
            Self::OPCODE_BOOKMARK => {
                if bytes.len() > 1 {
                    let note = String::from_utf8_lossy(&bytes[1..]).trim().to_string();
                    Ok(ControlCommand::DropBookmark(Some(note)))
                } else {
                    Ok(ControlCommand::DropBookmark(None))
                }
            }
            Self::OPCODE_SPLIT => Ok(ControlCommand::SplitFile),
            Self::OPCODE_SET_PAD => {
                if bytes.len() < 2 {
                    return Err(ProtocolError::InvalidLength { expected: 2, actual: bytes.len() });
                }
                Ok(ControlCommand::SetHardwarePad(bytes[1] as i8))
            }
            Self::OPCODE_TOGGLE_GUARD => Ok(ControlCommand::ToggleAutoGuard),
            Self::OPCODE_RESET_PEAKS => Ok(ControlCommand::ResetPeaks),
            Self::OPCODE_PAIRING_WINDOW => Ok(ControlCommand::OpenPairingWindow),
            Self::OPCODE_TOGGLE_HOTSPOT => Ok(ControlCommand::ToggleHotspot),
            other => Err(ProtocolError::UnknownOpcode(other)),
        }
    }
}

/// Detects the machine's primary local IP address (e.g. Wi-Fi / Ethernet LAN IP)
pub fn detect_local_ip() -> Option<String> {
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() || socket.connect("1.1.1.1:80").is_ok() {
            if let Ok(local_addr) = socket.local_addr() {
                let ip = local_addr.ip();
                if !ip.is_loopback() {
                    return Some(ip.to_string());
                }
            }
        }
    }

    // Fallback: parse global IP on Linux
    if let Ok(output) = std::process::Command::new("ip")
        .args(["-4", "-o", "addr", "show", "scope", "global"])
        .output()
    {
        if let Ok(text) = String::from_utf8(output.stdout) {
            for line in text.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(pos) = parts.iter().position(|&x| x == "inet") {
                    if let Some(cidr) = parts.get(pos + 1) {
                        let ip = cidr.split('/').next().unwrap_or("");
                        if !ip.is_empty() && !ip.starts_with("127.") {
                            return Some(ip.to_string());
                        }
                    }
                }
            }
        }
    }

    None
}

/// Detects the machine's actual system hostname (formatted for mDNS e.g. hostname.local)
pub fn detect_hostname() -> Option<String> {
    if let Ok(host) = std::fs::read_to_string("/etc/hostname") {
        let trimmed = host.trim();
        if !trimmed.is_empty() && trimmed != "localhost" {
            return Some(format!("{}.local", trimmed));
        }
    }
    if let Ok(host) = std::env::var("HOSTNAME") {
        let trimmed = host.trim();
        if !trimmed.is_empty() && trimmed != "localhost" {
            return Some(format!("{}.local", trimmed));
        }
    }
    None
}

/// Pairing payload serialized to QR code and displayed in `djm-tui`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingInfo {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub token: String,
    #[serde(default)]
    pub local_ip: Option<String>,
    #[serde(default)]
    pub mdns_hostname: Option<String>,
    #[serde(default)]
    pub hotspot_ssid: String,
    #[serde(default)]
    pub hotspot_password: String,
    #[serde(default)]
    pub hotspot_active: bool,
}

impl PairingInfo {
    pub const DEFAULT_HOTSPOT_GATEWAY: &'static str = "10.42.0.1";

    /// Automatically detects Wi-Fi/LAN IP and mDNS hostname, prioritizing direct reachable endpoints
    pub fn auto_detect(
        name: String,
        token: String,
        remote_public_url: Option<String>,
        port: u16,
        hotspot_ssid: String,
        hotspot_password: String,
        hotspot_active: bool,
    ) -> Self {
        let local_ip = detect_local_ip();
        let mdns_hostname = detect_hostname();

        // Host resolution preference:
        // 1. Explicitly configured remote_public_url (e.g. Tailscale or static domain)
        // 2. Detected local Wi-Fi / LAN IP (camera-clickable URL on same subnet)
        // 3. Detected mDNS hostname (e.g. robtop.local)
        // 4. Default fallback ("djm-box.local")
        let host = if let Some(url) = remote_public_url.filter(|u| !u.trim().is_empty()) {
            url
        } else if hotspot_active {
            Self::DEFAULT_HOTSPOT_GATEWAY.to_string()
        } else if let Some(ref ip) = local_ip {
            ip.clone()
        } else if let Some(ref mdns) = mdns_hostname {
            mdns.clone()
        } else {
            "djm-box.local".to_string()
        };

        Self {
            name,
            host,
            port,
            token,
            local_ip,
            mdns_hostname,
            hotspot_ssid,
            hotspot_password,
            hotspot_active,
        }
    }

    /// Complete direct-connect web URL with authentication token query parameter
    pub fn to_web_url(&self) -> String {
        format!("http://{}:{}/?token={}", self.host, self.port, self.token)
    }

    /// Dedicated hotspot gateway URL
    pub fn to_hotspot_web_url(&self) -> String {
        format!("http://{}:{}/?token={}", Self::DEFAULT_HOTSPOT_GATEWAY, self.port, self.token)
    }

    /// Direct Wi-Fi hotspot connection string for phone camera auto-join QR code
    pub fn to_wifi_qr_payload(&self) -> String {
        format!("WIFI:T:WPA;S:{};P:{};;", self.hotspot_ssid, self.hotspot_password)
    }

    pub fn to_qr_payload(&self) -> String {
        // Direct clickable URL for phone camera and companion app instant auth
        self.to_web_url()
    }
}

/// Helper utilities for querying and controlling NetworkManager Wi-Fi hotspots
pub struct HotspotControl;

impl HotspotControl {
    pub const CONNECTION_NAME: &'static str = "djm-box-hotspot";

    /// Checks if NetworkManager currently has an active hotspot connection named djm-box-hotspot
    pub fn is_hotspot_active() -> bool {
        if let Ok(output) = std::process::Command::new("nmcli")
            .args(["-t", "-f", "NAME,STATE", "connection", "show", "--active"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                for line in text.lines() {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() >= 2 && parts[0] == Self::CONNECTION_NAME && parts[1] == "activated" {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Checks if the host has any active non-loopback network connection (Wi-Fi, Ethernet, etc.) other than the hotspot
    pub fn has_active_network() -> bool {
        if let Ok(output) = std::process::Command::new("nmcli")
            .args(["-t", "-f", "NAME,TYPE,STATE", "connection", "show", "--active"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                for line in text.lines() {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() >= 3 && parts[0] != Self::CONNECTION_NAME && parts[2] == "activated" {
                        return true;
                    }
                }
            }
        }
        // Fallback: check if detect_local_ip() finds an IP other than loopback or 10.42.0.1
        if let Some(ip) = detect_local_ip() {
            if !ip.starts_with("127.") && ip != "10.42.0.1" {
                return true;
            }
        }
        false
    }

    /// Finds the first available Wi-Fi device interface name (e.g. wlan0, wlp2s0)
    pub fn find_wifi_device() -> Option<String> {
        if let Ok(output) = std::process::Command::new("nmcli")
            .args(["-t", "-f", "DEVICE,TYPE", "device"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                for line in text.lines() {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() >= 2 && parts[1] == "wifi" {
                        return Some(parts[0].to_string());
                    }
                }
            }
        }
        None
    }

    /// Starts the NetworkManager Wi-Fi hotspot
    pub fn start_hotspot(ssid: &str, password: &str) -> std::io::Result<bool> {
        if Self::is_hotspot_active() {
            return Ok(true);
        }
        let ifname_arg = Self::find_wifi_device();
        let mut cmd = std::process::Command::new("nmcli");
        cmd.args(["device", "wifi", "hotspot"]);
        if let Some(ref ifname) = ifname_arg {
            cmd.args(["ifname", ifname]);
        }
        cmd.args([
            "con-name",
            Self::CONNECTION_NAME,
            "ssid",
            ssid,
            "password",
            password,
        ]);
        let output = cmd.output()?;
        Ok(output.status.success())
    }

    /// Stops the NetworkManager Wi-Fi hotspot
    pub fn stop_hotspot() -> std::io::Result<bool> {
        let output = std::process::Command::new("nmcli")
            .args(["connection", "down", Self::CONNECTION_NAME])
            .output()?;
        Ok(output.status.success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_roundtrip_packed_bytes() {
        let t1 = Telemetry {
            state: RecordingState::Recording,
            hw_pad_db: -19,
            auto_guard_active: true,
            silence_countdown_active: false,
            peak_l_centi_db: -1425,
            peak_r_centi_db: -1380,
            rms_l_centi_db: -1850,
            rms_r_centi_db: -1820,
            clip_count: 3,
            active_file_duration_secs: 3624,
            active_file_size_mb: 480,
            free_disk_gb_tenths: 1482,
            ..Default::default()
        };

        let packed = t1.to_packed_bytes();
        assert_eq!(packed.len(), 20);

        let t2 = Telemetry::from_packed_bytes(&packed).expect("failed to unpack");
        assert_eq!(t1.state, t2.state);
        assert_eq!(t1.hw_pad_db, t2.hw_pad_db);
        assert_eq!(t1.auto_guard_active, t2.auto_guard_active);
        assert_eq!(t1.silence_countdown_active, t2.silence_countdown_active);
        assert_eq!(t1.peak_l_centi_db, t2.peak_l_centi_db);
        assert_eq!(t1.peak_r_centi_db, t2.peak_r_centi_db);
        assert_eq!(t1.rms_l_centi_db, t2.rms_l_centi_db);
        assert_eq!(t1.rms_r_centi_db, t2.rms_r_centi_db);
        assert_eq!(t1.clip_count, t2.clip_count);
        assert_eq!(t1.active_file_duration_secs, t2.active_file_duration_secs);
        assert_eq!(t1.active_file_size_mb, t2.active_file_size_mb);
        assert_eq!(t1.free_disk_gb_tenths, t2.free_disk_gb_tenths);
    }

    #[test]
    fn test_telemetry_pad_values() {
        for pad in [-19i8, -15, -10, -5, 0] {
            let mut t = Telemetry::default();
            t.hw_pad_db = pad;
            let packed = t.to_packed_bytes();
            let unpacked = Telemetry::from_packed_bytes(&packed).unwrap();
            assert_eq!(unpacked.hw_pad_db, pad);
        }
    }

    #[test]
    fn test_control_commands_binary_and_json() {
        let auth_cmd = ControlCommand::Auth("948201".to_string());
        let b = auth_cmd.to_bytes();
        let parsed = ControlCommand::from_bytes(&b).expect("auth parse failed");
        assert_eq!(auth_cmd, parsed);

        let split_cmd = ControlCommand::SplitFile;
        let b = split_cmd.to_bytes();
        let parsed = ControlCommand::from_bytes(&b).expect("split parse failed");
        assert_eq!(split_cmd, parsed);

        let pad_cmd = ControlCommand::SetHardwarePad(-15);
        let b = pad_cmd.to_bytes();
        let parsed = ControlCommand::from_bytes(&b).expect("pad parse failed");
        assert_eq!(pad_cmd, parsed);

        let bookmark_cmd = ControlCommand::DropBookmark(Some("DJ Dropped unreleased ID".to_string()));
        let b = bookmark_cmd.to_bytes();
        let parsed = ControlCommand::from_bytes(&b).expect("bookmark parse failed");
        assert_eq!(bookmark_cmd, parsed);

        let toggle_hs = ControlCommand::ToggleHotspot;
        let b = toggle_hs.to_bytes();
        let parsed = ControlCommand::from_bytes(&b).expect("toggle hotspot parse failed");
        assert_eq!(toggle_hs, parsed);
    }

    #[test]
    fn test_pairing_info_json() {
        let pair = PairingInfo {
            name: "DJM-Box-Recorder".to_string(),
            host: "djm-box.local".to_string(),
            port: 8080,
            token: "123456".to_string(),
            local_ip: Some("192.168.1.50".to_string()),
            mdns_hostname: Some("djm-box.local".to_string()),
            hotspot_ssid: "DJM-Box-Booth".to_string(),
            hotspot_password: "supersecret".to_string(),
            hotspot_active: false,
        };
        let url = pair.to_qr_payload();
        assert_eq!(url, "http://djm-box.local:8080/?token=123456");
        let wifi_qr = pair.to_wifi_qr_payload();
        assert_eq!(wifi_qr, "WIFI:T:WPA;S:DJM-Box-Booth;P:supersecret;;");
        let json = serde_json::to_string(&pair).unwrap();
        let loaded: PairingInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(pair, loaded);
    }

    #[test]
    fn test_telemetry_detection_helpers() {
        let mut t = Telemetry::default();
        assert!(!t.is_mixer_connected());
        assert!(!t.is_recording_to_disk());

        t.state = RecordingState::ArmedSilent;
        t.mixer_model = "Pioneer DJM-900NXS2".to_string();
        assert!(t.is_mixer_connected());
        assert!(!t.is_recording_to_disk());

        t.state = RecordingState::Recording;
        assert!(t.is_mixer_connected());
        assert!(t.is_recording_to_disk());

        t.state = RecordingState::Disconnected;
        assert!(!t.is_mixer_connected());
        assert!(!t.is_recording_to_disk());
    }
}
