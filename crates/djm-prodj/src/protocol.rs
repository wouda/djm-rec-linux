use std::net::Ipv4Addr;

pub const PRO_DJ_LINK_MAGIC: [u8; 10] = [
    0x51, 0x73, 0x70, 0x74, 0x31, 0x57, 0x6d, 0x6a, 0x51, 0x73,
]; // "Qspt1WmjQs"

pub const DEFAULT_ANNOUNCE_PORT: u16 = 50000;
pub const DEFAULT_CLAIM_PORT: u16 = 50001;
pub const DEFAULT_STATUS_PORT: u16 = 50002;

#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredDevice {
    pub device_name: String,
    pub player_number: u8,
    pub ip_address: Ipv4Addr,
    pub mac_address: [u8; 6],
    pub is_mixer: bool,
}

impl From<DiscoveredDevice> for djm_common::prodj::ProDjDevice {
    fn from(disc: DiscoveredDevice) -> Self {
        let model = disc.device_name.clone();
        Self {
            player_number: disc.player_number,
            device_name: disc.device_name,
            ip_address: disc.ip_address,
            mac_address: disc.mac_address,
            model,
            firmware: "1.00".to_string(),
            is_mixer: disc.is_mixer,
            last_seen_ms: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MixerStatus {
    pub player_number: u8,
    pub on_air_mask: u8, // bit 0 = ch1, bit 1 = ch2, ..., bit 5 = ch6
    pub master_fader_level: Option<u8>,
}

impl MixerStatus {
    pub fn is_channel_on_air(&self, channel_1_based: u8) -> bool {
        if (1..=8).contains(&channel_1_based) {
            (self.on_air_mask & (1 << (channel_1_based - 1))) != 0
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeckStatusUpdate {
    pub player_number: u8,
    pub rekordbox_track_id: u32,
    pub is_playing: bool,
    pub is_cued: bool,
    pub is_master: bool,
    pub pitch_percent: f32,
    pub playhead_secs: Option<f32>,
    pub duration_secs: Option<f32>,
}

/// Verifies that a buffer begins with the 10-byte Pro DJ Link magic header
pub fn has_valid_magic(buf: &[u8]) -> bool {
    buf.len() >= 10 && buf[0..10] == PRO_DJ_LINK_MAGIC
}

/// Parses an announce or keep-alive packet received on UDP port 50000
pub fn parse_announce_packet(buf: &[u8], src_ip: Ipv4Addr) -> Option<DiscoveredDevice> {
    if !has_valid_magic(buf) || buf.len() < 0x2C {
        return None;
    }

    // Device name: 20 bytes at offset 0x0A..0x1E, null-terminated ASCII
    let raw_name = &buf[0x0A..0x1E];
    let name_len = raw_name.iter().position(|&b| b == 0).unwrap_or(raw_name.len());
    let device_name = String::from_utf8_lossy(&raw_name[..name_len]).trim().to_string();

    let packet_type = buf[0x1E];
    // 0x01 = Initial discovery, 0x06 = Announce keep-alive, 0x02 = Stage-2 keep-alive
    if packet_type != 0x01 && packet_type != 0x06 && packet_type != 0x02 {
        return None;
    }

    let player_number = buf[0x24];
    let is_mixer = player_number >= 0x10 || device_name.to_uppercase().contains("DJM");

    // IP address at offset 0x28..0x2C if non-zero, otherwise fallback to src_ip
    let reported_ip = Ipv4Addr::new(buf[0x28], buf[0x29], buf[0x2A], buf[0x2B]);
    let final_ip = if reported_ip.is_unspecified() { src_ip } else { reported_ip };

    // MAC address at offset 0x2C..0x32 if available
    let mut mac = [0u8; 6];
    if buf.len() >= 0x32 {
        mac.copy_from_slice(&buf[0x2C..0x32]);
    }

    Some(DiscoveredDevice {
        device_name,
        player_number,
        ip_address: final_ip,
        mac_address: mac,
        is_mixer,
    })
}

/// Parses a mixer status packet received on UDP port 50002 (packet subtype 0x04)
pub fn parse_mixer_status_packet(buf: &[u8]) -> Option<MixerStatus> {
    if !has_valid_magic(buf) || buf.len() < 0x2D {
        return None;
    }

    let packet_type = buf[0x1E];
    if packet_type != 0x04 {
        return None;
    }

    let player_number = buf[0x24];
    let on_air_mask = buf[0x2C];
    let master_fader_level = if buf.len() > 0x30 { Some(buf[0x30]) } else { None };

    Some(MixerStatus {
        player_number,
        on_air_mask,
        master_fader_level,
    })
}

/// Parses a CDJ player status packet received on UDP port 50002 (packet subtype 0x0A)
pub fn parse_cdj_status_packet(buf: &[u8]) -> Option<DeckStatusUpdate> {
    if !has_valid_magic(buf) || buf.len() < 0x30 {
        return None;
    }

    let packet_type = buf[0x1E];
    if packet_type != 0x0A {
        return None;
    }

    let player_number = buf[0x24];

    // Rekordbox track ID: 32-bit big endian integer at 0x28..0x2C
    let track_id = u32::from_be_bytes([buf[0x28], buf[0x29], buf[0x2A], buf[0x2B]]);

    // Play state at byte 0x89 (if buffer is full length ~0xA0+), else default
    let (is_playing, is_cued) = if buf.len() > 0x89 {
        let state_byte = buf[0x89];
        (state_byte == 0x02, state_byte == 0x03)
    } else {
        (false, false)
    };

    let is_master = if buf.len() > 0x92 {
        buf[0x92] == 0x01
    } else {
        false
    };

    // Pitch ratio at 0x8C..0x90 if available
    let pitch_percent = if buf.len() >= 0x90 {
        let raw_pitch = u32::from_be_bytes([buf[0x8C], buf[0x8D], buf[0x8E], buf[0x8F]]);
        // 0x00100000 = 100% (0.00% pitch shift)
        if raw_pitch > 0 {
            ((raw_pitch as f32 / 1048576.0) - 1.0) * 100.0
        } else {
            0.0
        }
    } else {
        0.0
    };

    Some(DeckStatusUpdate {
        player_number,
        rekordbox_track_id: track_id,
        is_playing,
        is_cued,
        is_master,
        pitch_percent,
        playhead_secs: None,
        duration_secs: None,
    })
}

/// Builds a keep-alive announcement packet to broadcast on port 50000
pub fn build_keepalive_packet(
    device_name: &str,
    player_number: u8,
    ip: Ipv4Addr,
    mac: [u8; 6],
) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(0x36);
    // 1. Magic header (10 bytes)
    pkt.extend_from_slice(&PRO_DJ_LINK_MAGIC);

    // 2. Device Name (20 bytes, null padded)
    let mut name_bytes = [0u8; 20];
    let src_bytes = device_name.as_bytes();
    let copy_len = src_bytes.len().min(19);
    name_bytes[..copy_len].copy_from_slice(&src_bytes[..copy_len]);
    pkt.extend_from_slice(&name_bytes);

    // 3. Subtype: 0x06 (Announce Keep-Alive)
    pkt.push(0x06);
    // 4. Version: 0x02
    pkt.push(0x02);

    // 5. Sequence/index (4 bytes)
    pkt.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);

    // 6. Player number (1 byte at 0x24)
    pkt.push(player_number);

    // 7. Padding (3 bytes at 0x25..0x27)
    pkt.extend_from_slice(&[0x00, 0x00, 0x00]);

    // 8. IP Address (4 bytes at 0x28..0x2B)
    pkt.extend_from_slice(&ip.octets());

    // 9. MAC address (6 bytes at 0x2C..0x31)
    pkt.extend_from_slice(&mac);

    // 10. Trailing padding
    pkt.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);

    pkt
}

/// Builds a player number claim packet to broadcast on port 50001
pub fn build_claim_packet(player_number: u8, mac: [u8; 6]) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(0x30);
    pkt.extend_from_slice(&PRO_DJ_LINK_MAGIC); // 0x00..0x09

    let mut name_bytes = [0u8; 20];
    name_bytes[..7].copy_from_slice(b"DJM-BOX");
    pkt.extend_from_slice(&name_bytes); // 0x0A..0x1D

    pkt.push(0x00); // 0x1E Claim request
    pkt.push(0x00); // 0x1F

    pkt.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // 0x20..0x23
    pkt.push(player_number); // 0x24 Player Number
    pkt.push(0x00); // 0x25
    pkt.extend_from_slice(&mac); // 0x26..0x2B

    pkt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_real_hardware_djm_a9_announce_packet() {
        // Wire capture of DJM-A9 announce packet from PROTOCOL_NOTES.md
        let raw_capture: [u8; 48] = [
            0x51, 0x73, 0x70, 0x74, 0x31, 0x57, 0x6d, 0x6a, 0x51, 0x73, // Magic "Qspt1WmjQs"
            0x44, 0x4a, 0x4d, 0x2d, 0x41, 0x39, 0x00, 0x00, 0x00, 0x00, // "DJM-A9\0..."
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x06, 0x02, // Subtype 0x06
            0x00, 0x00, 0x00, 0x01, // MAC / header
            0x11, 0x00, 0x00, 0x00, // Player number 0x11 (Mixer Ch 1)
            0xc0, 0xa8, 0x55, 0x11, // IP: 192.168.85.17
            0x00, 0x00, 0x00, 0x00,
        ];

        let dev = parse_announce_packet(&raw_capture, Ipv4Addr::new(127, 0, 0, 1)).expect("parse announce");
        assert_eq!(dev.device_name, "DJM-A9");
        assert_eq!(dev.player_number, 0x11);
        assert_eq!(dev.ip_address, Ipv4Addr::new(192, 168, 85, 17));
        assert!(dev.is_mixer, "DJM-A9 should be identified as mixer");
    }

    #[test]
    fn test_parse_real_hardware_mixer_status_on_air() {
        // Wire capture of DJM-A9 Mixer Status packet from PROTOCOL_NOTES.md with channels 2 & 3 On-Air
        let mut raw_mixer: Vec<u8> = vec![
            0x51, 0x73, 0x70, 0x74, 0x31, 0x57, 0x6d, 0x6a, 0x51, 0x73,
            0x44, 0x4a, 0x4d, 0x2d, 0x41, 0x39, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x04, 0x00, // Subtype 0x04 (Mixer status)
            0x00, 0x00, 0x00, 0x01,
            0x11, 0x00, 0x00, 0x00, // Player number 0x11
            0x00, 0x00, 0x00, 0x00,
            0x06, 0x00, 0x00, 0x00, // On-Air mask: 0x06 (Ch 2 + Ch 3 on-air)
        ];
        raw_mixer.resize(0x40, 0);

        let status = parse_mixer_status_packet(&raw_mixer).expect("parse mixer status");
        assert_eq!(status.on_air_mask, 0x06);
        assert!(!status.is_channel_on_air(1), "Ch 1 should be Off-Air");
        assert!(status.is_channel_on_air(2), "Ch 2 should be On-Air");
        assert!(status.is_channel_on_air(3), "Ch 3 should be On-Air");
        assert!(!status.is_channel_on_air(4), "Ch 4 should be Off-Air");
    }

    #[test]
    fn test_parse_real_hardware_cdj_3000_status() {
        // Wire capture of CDJ-3000 deck status packet from PROTOCOL_NOTES.md
        let mut raw_cdj = vec![0u8; 0xA0];
        raw_cdj[0..10].copy_from_slice(&PRO_DJ_LINK_MAGIC);
        raw_cdj[0x0A..0x12].copy_from_slice(b"CDJ-3000");
        raw_cdj[0x1E] = 0x0A; // Subtype 0x0A (Player status)
        raw_cdj[0x24] = 0x02; // Player 2
        // Track ID 1001 = 0x000003e9
        raw_cdj[0x28..0x2C].copy_from_slice(&[0x00, 0x00, 0x03, 0xE9]);
        raw_cdj[0x89] = 0x02; // Playing
        raw_cdj[0x92] = 0x01; // Master

        let status = parse_cdj_status_packet(&raw_cdj).expect("parse CDJ status");
        assert_eq!(status.player_number, 2);
        assert_eq!(status.rekordbox_track_id, 1001);
        assert!(status.is_playing);
        assert!(status.is_master);
        assert!(!status.is_cued);
    }

    #[test]
    fn test_build_keepalive_packet_roundtrip() {
        let mac = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
        let ip = Ipv4Addr::new(192, 168, 85, 99);
        let pkt = build_keepalive_packet("DJM-BOX", 6, ip, mac);

        let parsed = parse_announce_packet(&pkt, ip).expect("parse self keepalive");
        assert_eq!(parsed.device_name, "DJM-BOX");
        assert_eq!(parsed.player_number, 6);
        assert_eq!(parsed.ip_address, ip);
    }
}
