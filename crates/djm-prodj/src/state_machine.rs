use crate::inventory::DeviceInventory;
use crate::protocol::{
    has_valid_magic, parse_announce_packet, parse_mixer_status_packet,
};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticipationState {
    Passive,
    Settling { started_at_ms: u64 },
    ActiveVirtualPlayer { player_number: u8, claimed_at_ms: u64 },
}

#[derive(Debug, Clone)]
pub struct ParticipationConfig {
    pub settling_period_6player_ms: u64, // default 10,000 ms (10s)
    pub settling_period_4player_ms: u64, // default 30,000 ms (30s)
    pub live_audio_gate_ms: u64,         // default 180,000 ms (3 min)
}

impl Default for ParticipationConfig {
    fn default() -> Self {
        Self {
            settling_period_6player_ms: 10_000,
            settling_period_4player_ms: 30_000,
            live_audio_gate_ms: 180_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StateMachineAction {
    None,
    ClaimSlot(u8),
    ReleasedVirtualSlot { player_number: u8, reason: String },
    BroadcastKeepalive { player_number: u8 },
}

pub struct ParticipationStateMachine {
    pub state: ParticipationState,
    pub config: ParticipationConfig,
    pub inventory: DeviceInventory,
    pub audio_playing_since_ms: Option<u64>,
    pub last_new_device_ms: u64,
    pub last_keepalive_sent_ms: u64,
    pub announcements_sent_after_new_device: usize,
}

impl ParticipationStateMachine {
    pub fn new(config: ParticipationConfig) -> Self {
        Self {
            state: ParticipationState::Passive,
            config,
            inventory: DeviceInventory::new(),
            audio_playing_since_ms: None,
            last_new_device_ms: 0,
            last_keepalive_sent_ms: 0,
            announcements_sent_after_new_device: 0,
        }
    }

    /// Primary packet ingress. Processes an incoming Pro DJ Link UDP packet.
    /// GUARANTEE: If a new device arrives or our number is claimed, virtual slot is
    /// released immediately and zero announcements are generated.
    pub fn on_packet_received(
        &mut self,
        buf: &[u8],
        src_ip: Ipv4Addr,
        now_ms: u64,
    ) -> StateMachineAction {
        if !has_valid_magic(buf) {
            return StateMachineAction::None;
        }

        // 1. Check claim packet on port 50001 (opcode 0x00)
        if buf.len() >= 0x25 && buf[0x1E] == 0x00 {
            let claimed_number = buf[0x24];
            if let ParticipationState::ActiveVirtualPlayer { player_number, .. } = self.state {
                if player_number == claimed_number {
                    // Physical device claims our slot: release immediately without contesting!
                    self.state = ParticipationState::Passive;
                    self.last_new_device_ms = now_ms;
                    return StateMachineAction::ReleasedVirtualSlot {
                        player_number,
                        reason: format!("Physical device requested player number {}", player_number),
                    };
                }
            }
        }

        // 2. Check announce packet (port 50000, opcode 0x01 or 0x06 or 0x02)
        if let Some(disc) = parse_announce_packet(buf, src_ip) {
            let is_new_device = !self.inventory.devices.contains_key(&disc.player_number)
                && !(disc.is_mixer && self.inventory.mixer.is_some());

            // If another device is already using our claimed number
            if let ParticipationState::ActiveVirtualPlayer { player_number, .. } = self.state {
                if player_number == disc.player_number {
                    self.state = ParticipationState::Passive;
                    self.last_new_device_ms = now_ms;
                    return StateMachineAction::ReleasedVirtualSlot {
                        player_number,
                        reason: format!("Incoming announcement matches our player number {}", player_number),
                    };
                }

                // If ANY new device is seen while we are active, release immediately!
                if is_new_device {
                    self.state = ParticipationState::Passive;
                    self.last_new_device_ms = now_ms;
                    self.inventory.update_device(disc, now_ms);
                    return StateMachineAction::ReleasedVirtualSlot {
                        player_number,
                        reason: "New device connected to Pro DJ Link network".to_string(),
                    };
                }
            }

            if is_new_device {
                self.last_new_device_ms = now_ms;
            }

            self.inventory.update_device(disc, now_ms);
            return StateMachineAction::None;
        }

        // 3. Check mixer status packet (port 50002, opcode 0x04)
        if let Some(mixer_stat) = parse_mixer_status_packet(buf) {
            if mixer_stat.on_air_mask > 0 {
                if self.audio_playing_since_ms.is_none() {
                    self.audio_playing_since_ms = Some(now_ms);
                }
            } else {
                // If mixer is completely off-air, reset audio playing timer
                self.audio_playing_since_ms = None;
            }
        }

        StateMachineAction::None
    }

    /// Periodic state machine tick (called e.g. every 50-100 ms)
    pub fn tick(&mut self, now_ms: u64) -> StateMachineAction {
        match self.state {
            ParticipationState::Passive => {
                // Check if we can begin settling: need an active mixer and at least one free slot
                if self.inventory.mixer.is_some() && self.inventory.highest_free_player_number().is_some() {
                    self.state = ParticipationState::Settling { started_at_ms: now_ms };
                }
                StateMachineAction::None
            }
            ParticipationState::Settling { started_at_ms } => {
                // If a new device appeared recently, restart settling timer
                if now_ms.saturating_sub(self.last_new_device_ms) < 5_000 {
                    return StateMachineAction::None;
                }

                if self.inventory.is_six_player_supported {
                    // 6-player network (DJM-V10 / DJM-A9)
                    let settled_duration = now_ms.saturating_sub(started_at_ms);
                    if settled_duration >= self.config.settling_period_6player_ms {
                        // Check slot 6, fallback to 5
                        if !self.inventory.devices.contains_key(&6) {
                            self.state = ParticipationState::ActiveVirtualPlayer {
                                player_number: 6,
                                claimed_at_ms: now_ms,
                            };
                            return StateMachineAction::ClaimSlot(6);
                        } else if !self.inventory.devices.contains_key(&5) {
                            self.state = ParticipationState::ActiveVirtualPlayer {
                                player_number: 5,
                                claimed_at_ms: now_ms,
                            };
                            return StateMachineAction::ClaimSlot(5);
                        }
                    }
                } else {
                    // 4-player network (DJM-900NXS2)
                    // Requirements:
                    // 1. ≤ 3 physical players active
                    // 2. Slot 4 is free
                    // 3. Audio playing continuously for >= live_audio_gate_ms (3 min)
                    // 4. No new devices for at least settling_period_4player_ms (30s)
                    let audio_playing_ok = match self.audio_playing_since_ms {
                        Some(t) => now_ms.saturating_sub(t) >= self.config.live_audio_gate_ms,
                        None => false,
                    };
                    let no_new_devices_ok = now_ms.saturating_sub(self.last_new_device_ms) >= self.config.settling_period_4player_ms;
                    let slot_4_free = !self.inventory.devices.contains_key(&4) && self.inventory.devices.len() <= 3;

                    if audio_playing_ok && no_new_devices_ok && slot_4_free {
                        self.state = ParticipationState::ActiveVirtualPlayer {
                            player_number: 4,
                            claimed_at_ms: now_ms,
                        };
                        return StateMachineAction::ClaimSlot(4);
                    }
                }

                StateMachineAction::None
            }
            ParticipationState::ActiveVirtualPlayer { player_number, .. } => {
                // Keepalive broadcast every 280 ms
                if now_ms.saturating_sub(self.last_keepalive_sent_ms) >= 280 {
                    self.last_keepalive_sent_ms = now_ms;
                    StateMachineAction::BroadcastKeepalive { player_number }
                } else {
                    StateMachineAction::None
                }
            }
        }
    }

    /// Whether the state machine permits sending keepalive announcements
    pub fn is_active_player(&self) -> bool {
        matches!(self.state, ParticipationState::ActiveVirtualPlayer { .. })
    }

    pub fn is_settling(&self) -> bool {
        matches!(self.state, ParticipationState::Settling { .. })
    }

    /// Returns the currently claimed virtual player number if active
    pub fn claimed_player_number(&self) -> Option<u8> {
        match self.state {
            ParticipationState::ActiveVirtualPlayer { player_number, .. } => Some(player_number),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{build_claim_packet, build_keepalive_packet};
    use std::net::Ipv4Addr;

    #[test]
    fn test_passive_start_and_settling_on_6_player_network() {
        let config = ParticipationConfig {
            settling_period_6player_ms: 10_000,
            settling_period_4player_ms: 30_000,
            live_audio_gate_ms: 180_000,
        };
        let mut sm = ParticipationStateMachine::new(config);

        // Starts in Passive state
        assert_eq!(sm.state, ParticipationState::Passive);
        assert!(!sm.is_active_player());

        // Receive DJM-A9 mixer announce at t = 0
        let a9_pkt = build_keepalive_packet("DJM-A9", 0x11, Ipv4Addr::new(192, 168, 85, 17), [0; 6]);
        sm.on_packet_received(&a9_pkt, Ipv4Addr::new(192, 168, 85, 17), 0);
        assert!(sm.inventory.is_six_player_supported);

        // Tick at t = 100ms -> transitions to Settling
        sm.tick(100);
        assert!(matches!(sm.state, ParticipationState::Settling { .. }));

        // Tick at t = 5000ms -> still settling
        let action = sm.tick(5_000);
        assert_eq!(action, StateMachineAction::None);
        assert!(!sm.is_active_player());

        // Tick at t = 10_200ms -> settling period elapsed, claims slot 6
        let action = sm.tick(10_200);
        assert_eq!(action, StateMachineAction::ClaimSlot(6));
        assert!(sm.is_active_player());
        assert_eq!(sm.claimed_player_number(), Some(6));
    }

    #[test]
    fn test_4_player_network_requires_3_minute_live_audio_gate() {
        let config = ParticipationConfig {
            settling_period_6player_ms: 10_000,
            settling_period_4player_ms: 30_000,
            live_audio_gate_ms: 180_000, // 3 minutes
        };
        let mut sm = ParticipationStateMachine::new(config);

        // Connect DJM-900NXS2 (4-player network)
        let nxs2_pkt = build_keepalive_packet("DJM-900NXS2", 0x11, Ipv4Addr::new(192, 168, 85, 17), [0; 6]);
        sm.on_packet_received(&nxs2_pkt, Ipv4Addr::new(192, 168, 85, 17), 0);
        assert!(!sm.inventory.is_six_player_supported);

        // Add 2 CDJ-2000NXS2 players (slot 1 and slot 2)
        sm.on_packet_received(&build_keepalive_packet("CDJ-2000NXS2", 1, Ipv4Addr::new(192, 168, 85, 1), [0; 6]), Ipv4Addr::new(192, 168, 85, 1), 100);
        sm.on_packet_received(&build_keepalive_packet("CDJ-2000NXS2", 2, Ipv4Addr::new(192, 168, 85, 2), [0; 6]), Ipv4Addr::new(192, 168, 85, 2), 200);

        sm.tick(300);
        assert!(matches!(sm.state, ParticipationState::Settling { .. }));

        // At t = 60s, settling time is ok but NO audio playing yet
        let action = sm.tick(60_000);
        assert_eq!(action, StateMachineAction::None);
        assert!(!sm.is_active_player());

        // Audio starts at t = 60s (Mixer status with On-Air mask 0x01)
        let mut mixer_stat = vec![0u8; 0x40];
        mixer_stat[0..10].copy_from_slice(&crate::protocol::PRO_DJ_LINK_MAGIC);
        mixer_stat[0x1E] = 0x04;
        mixer_stat[0x24] = 0x11;
        mixer_stat[0x2C] = 0x01; // Ch 1 On-Air
        sm.on_packet_received(&mixer_stat, Ipv4Addr::new(192, 168, 85, 17), 60_000);

        // At t = 180s (2 minutes of audio), gate NOT passed yet
        let action = sm.tick(180_000);
        assert_eq!(action, StateMachineAction::None);
        assert!(!sm.is_active_player());

        // At t = 245s (3 minutes and 5 seconds of continuous audio), gate passes!
        let action = sm.tick(245_000);
        assert_eq!(action, StateMachineAction::ClaimSlot(4));
        assert!(sm.is_active_player());
        assert_eq!(sm.claimed_player_number(), Some(4));
    }

    #[test]
    fn test_immediate_silent_release_on_new_device_packet() {
        // Verification test specified in Task 5.2:
        // "verify tests replaying a mid-event CDJ boot capture show no announcement after the first new-device packet"
        let config = ParticipationConfig::default();
        let mut sm = ParticipationStateMachine::new(config);

        // Bring SM into ActiveVirtualPlayer on slot 6 (6-player network)
        sm.inventory.is_six_player_supported = true;
        sm.state = ParticipationState::ActiveVirtualPlayer {
            player_number: 6,
            claimed_at_ms: 10_000,
        };
        assert!(sm.is_active_player());

        // Mid-event: A guest DJ powers on a CDJ-3000 (Player 3)
        let new_cdj_pkt = build_keepalive_packet("CDJ-3000", 3, Ipv4Addr::new(192, 168, 85, 3), [0xAA; 6]);

        // Process first packet of the new device
        let action = sm.on_packet_received(&new_cdj_pkt, Ipv4Addr::new(192, 168, 85, 3), 50_000);

        // 1. MUST immediately release the virtual slot!
        assert_eq!(
            action,
            StateMachineAction::ReleasedVirtualSlot {
                player_number: 6,
                reason: "New device connected to Pro DJ Link network".to_string(),
            }
        );
        assert_eq!(sm.state, ParticipationState::Passive);
        assert!(!sm.is_active_player());

        // 2. Subsequent ticks MUST NOT generate any keepalive announcements!
        for t in 0..10 {
            let tick_action = sm.tick(50_000 + t * 100);
            assert_ne!(
                tick_action,
                StateMachineAction::BroadcastKeepalive { player_number: 6 },
                "Must never broadcast announcements after new device packet!"
            );
        }
    }

    #[test]
    fn test_immediate_silent_release_when_physical_device_claims_our_number() {
        let config = ParticipationConfig::default();
        let mut sm = ParticipationStateMachine::new(config);

        sm.state = ParticipationState::ActiveVirtualPlayer {
            player_number: 4,
            claimed_at_ms: 10_000,
        };

        // Incoming claim packet for player 4 from another hardware device
        let claim_pkt = build_claim_packet(4, [0xBB; 6]);
        let action = sm.on_packet_received(&claim_pkt, Ipv4Addr::new(192, 168, 85, 4), 20_000);

        assert!(matches!(action, StateMachineAction::ReleasedVirtualSlot { player_number: 4, .. }));
        assert_eq!(sm.state, ParticipationState::Passive);
        assert!(!sm.is_active_player());
    }
}
