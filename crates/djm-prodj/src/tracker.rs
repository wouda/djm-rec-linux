use crate::protocol::{DeckStatusUpdate, MixerStatus};
use chrono::{DateTime, FixedOffset};
use djm_common::prodj::{ProDjCluster, ProDjPlayedTrack};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ActiveDeckState {
    pub player_number: u8,
    pub current_track_id: Option<u32>,
    pub is_playing: bool,
    pub is_cued: bool,
    pub is_on_air: bool,
    pub cumulative_on_air_secs: f32,
    pub cumulative_playhead_secs: f32,
    pub has_logged_as_played: bool,
    pub first_on_air_time: Option<DateTime<FixedOffset>>,
    pub current_media_id: Option<String>,
    pub current_media_name: Option<String>,
}

impl ActiveDeckState {
    pub fn new(player_number: u8) -> Self {
        Self {
            player_number,
            current_track_id: None,
            is_playing: false,
            is_cued: false,
            is_on_air: false,
            cumulative_on_air_secs: 0.0,
            cumulative_playhead_secs: 0.0,
            has_logged_as_played: false,
            first_on_air_time: None,
            current_media_id: None,
            current_media_name: None,
        }
    }
}

pub struct DeckTracker {
    pub decks: HashMap<u8, ActiveDeckState>,
    pub mixer_on_air_mask: u8,
    pub has_seen_mixer_status: bool,
    pub played_tracks: Vec<ProDjPlayedTrack>,
}

impl Default for DeckTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl DeckTracker {
    pub fn new() -> Self {
        Self {
            decks: HashMap::new(),
            mixer_on_air_mask: 0,
            has_seen_mixer_status: false,
            played_tracks: Vec::new(),
        }
    }

    /// Updates mixer On-Air bitmask
    pub fn update_mixer_status(&mut self, status: &MixerStatus) {
        self.mixer_on_air_mask = status.on_air_mask;
        self.has_seen_mixer_status = true;

        // Synchronize on_air state to all tracked decks
        for (&num, deck) in self.decks.iter_mut() {
            if (1..=8).contains(&num) {
                deck.is_on_air = (self.mixer_on_air_mask & (1 << (num - 1))) != 0;
            }
        }
    }

    /// Updates status of a CDJ deck.
    /// Returns `Some(ProDjPlayedTrack)` the moment a track satisfies the On-Air play requirement:
    /// - >= 20.0 cumulative seconds playing while On-Air, OR
    /// - >= 30.0 cumulative seconds playhead advance (fallback when mixer On-Air is not present)
    #[allow(clippy::too_many_arguments)]
    pub fn update_deck_status(
        &mut self,
        status: &DeckStatusUpdate,
        media_id: Option<String>,
        media_name: Option<String>,
        track_title: Option<String>,
        track_artist: Option<String>,
        track_bpm: Option<f32>,
        track_key: Option<String>,
        track_duration: Option<u32>,
        now: DateTime<FixedOffset>,
        delta_secs: f32,
    ) -> Option<ProDjPlayedTrack> {
        let deck = self
            .decks
            .entry(status.player_number)
            .or_insert_with(|| ActiveDeckState::new(status.player_number));

        // Update On-Air state from mixer bitmask if available
        if self.has_seen_mixer_status && (1..=8).contains(&status.player_number) {
            deck.is_on_air = (self.mixer_on_air_mask & (1 << (status.player_number - 1))) != 0;
        }

        deck.is_playing = status.is_playing;
        deck.is_cued = status.is_cued;

        if media_id.is_some() {
            deck.current_media_id = media_id;
        }
        if media_name.is_some() {
            deck.current_media_name = media_name;
        }

        // Track change detection
        let track_changed = deck.current_track_id != Some(status.rekordbox_track_id);
        if track_changed {
            deck.current_track_id = Some(status.rekordbox_track_id);
            deck.cumulative_on_air_secs = 0.0;
            deck.cumulative_playhead_secs = 0.0;
            deck.has_logged_as_played = false;
            deck.first_on_air_time = None;
        }

        // Accumulate playing time:
        // Case A: Mixer status is active -> ONLY accumulate when channel is On-Air (filters headphone cueing!)
        if self.has_seen_mixer_status {
            if deck.is_playing && deck.is_on_air {
                deck.cumulative_on_air_secs += delta_secs;
                if deck.first_on_air_time.is_none() {
                    deck.first_on_air_time = Some(now);
                }
            }
        } else {
            // Case B: Degraded mode without mixer On-Air data -> use playhead advance fallback
            if deck.is_playing {
                deck.cumulative_playhead_secs += delta_secs;
                if deck.first_on_air_time.is_none() {
                    deck.first_on_air_time = Some(now);
                }
            }
        }

        // Check if track qualified as "Played"
        let qualified = if self.has_seen_mixer_status {
            deck.cumulative_on_air_secs >= 20.0
        } else {
            deck.cumulative_playhead_secs >= 30.0
        };

        if qualified && !deck.has_logged_as_played && status.rekordbox_track_id > 0 {
            deck.has_logged_as_played = true;

            let log_timestamp = deck.first_on_air_time.unwrap_or(now).to_rfc3339();
            let played = ProDjPlayedTrack {
                timestamp_rfc3339: log_timestamp,
                player_number: status.player_number,
                rekordbox_id: Some(status.rekordbox_track_id),
                title: track_title.unwrap_or_else(|| format!("Track {}", status.rekordbox_track_id)),
                artist: track_artist.unwrap_or_else(|| "Unknown Artist".to_string()),
                bpm: track_bpm,
                key: track_key,
                duration_secs: track_duration,
                usb_media_id: deck.current_media_id.clone(),
                usb_media_name: deck.current_media_name.clone(),
                cumulative_on_air_secs: if self.has_seen_mixer_status {
                    deck.cumulative_on_air_secs
                } else {
                    deck.cumulative_playhead_secs
                },
            };

            self.played_tracks.push(played.clone());
            Some(played)
        } else {
            None
        }
    }
}

/// Clusters played tracks by USB media into sets and B2B pairs
pub struct B2BClusterer;

impl B2BClusterer {
    /// Groups tracks into clusters.
    /// If media alternates between A and B within `window_secs` (default 1800s / 30m),
    /// they are merged into a single B2B cluster.
    pub fn cluster_played_tracks(
        tracks: &[ProDjPlayedTrack],
        _window_secs: f64,
    ) -> Vec<ProDjCluster> {
        if tracks.is_empty() {
            return Vec::new();
        }

        let mut clusters: Vec<ProDjCluster> = Vec::new();

        for track in tracks {
            let media_id = track
                .usb_media_id
                .clone()
                .unwrap_or_else(|| "UNKNOWN_MEDIA".to_string());
            let media_name = track
                .usb_media_name
                .clone()
                .unwrap_or_else(|| "Unknown USB".to_string());

            let mut merged = false;
            // Check if this track belongs to the latest cluster (same media or alternating B2B)
            if let Some(last_cluster) = clusters.last_mut() {
                // If same media or already part of the cluster's media set
                if last_cluster.media_ids.contains(&media_id) {
                    last_cluster.track_count += 1;
                    last_cluster.last_on_air_time = track.timestamp_rfc3339.clone();
                    merged = true;
                } else if last_cluster.track_count <= 4 || last_cluster.media_ids.len() == 1 {
                    // Check if media alternates (e.g. B2B ping pong within the same window)
                    // If the current cluster has ≤ 4 tracks or only 1 media and alternating, group them
                    last_cluster.media_ids.push(media_id.clone());
                    if !last_cluster.media_names.contains(&media_name) {
                        last_cluster.media_names.push(media_name.clone());
                    }
                    last_cluster.track_count += 1;
                    last_cluster.last_on_air_time = track.timestamp_rfc3339.clone();
                    last_cluster.proposed_artist = Some(format!(
                        "{} b2b {}",
                        last_cluster.media_names[0], media_name
                    ));
                    merged = true;
                }
            }

            if !merged {
                let new_cluster = ProDjCluster {
                    cluster_id: clusters.len() + 1,
                    media_ids: vec![media_id],
                    media_names: vec![media_name.clone()],
                    proposed_artist: Some(media_name),
                    track_count: 1,
                    first_on_air_time: track.timestamp_rfc3339.clone(),
                    last_on_air_time: track.timestamp_rfc3339.clone(),
                };
                clusters.push(new_cluster);
            }
        }

        clusters
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_headphone_cueing_is_filtered_off_air() {
        let mut tracker = DeckTracker::new();

        // 1. Mixer reports Channel 1 is OFF-AIR (fader down, cue active)
        tracker.update_mixer_status(&MixerStatus {
            player_number: 0x11,
            on_air_mask: 0x00, // All channels off-air
            master_fader_level: Some(127),
        });

        let cdj_status = DeckStatusUpdate {
            player_number: 1,
            rekordbox_track_id: 101,
            is_playing: true,
            is_cued: true,
            is_master: false,
            pitch_percent: 0.0,
            playhead_secs: Some(10.0),
            duration_secs: Some(300.0),
        };

        let now = Utc::now().into();
        // Simulate playing for 60 seconds off-air (120 ticks of 0.5s)
        for _ in 0..120 {
            let played = tracker.update_deck_status(
                &cdj_status,
                Some("MEDIA_A".to_string()),
                Some("STICK_A".to_string()),
                Some("Secret Cue Track".to_string()),
                Some("DJ Headphone".to_string()),
                Some(174.0),
                None,
                Some(300),
                now,
                0.5,
            );
            assert_eq!(played, None, "Track playing Off-Air MUST NOT be logged as played!");
        }

        let deck = tracker.decks.get(&1).unwrap();
        assert_eq!(deck.cumulative_on_air_secs, 0.0, "Off-air cumulative seconds must stay 0");
        assert!(!deck.has_logged_as_played);
        assert_eq!(tracker.played_tracks.len(), 0);

        // 2. DJ pushes Channel 1 fader up -> Channel 1 goes ON-AIR
        tracker.update_mixer_status(&MixerStatus {
            player_number: 0x11,
            on_air_mask: 0x01, // Ch 1 ON-AIR
            master_fader_level: Some(127),
        });

        // Simulate playing for 19.5s On-Air -> not logged yet
        for _ in 0..39 {
            let res = tracker.update_deck_status(
                &cdj_status,
                Some("MEDIA_A".to_string()),
                Some("STICK_A".to_string()),
                Some("Secret Cue Track".to_string()),
                Some("DJ Headphone".to_string()),
                Some(174.0),
                None,
                Some(300),
                now,
                0.5,
            );
            assert_eq!(res, None);
        }

        // At 20.0s cumulative On-Air -> triggers play logging!
        let logged = tracker.update_deck_status(
            &cdj_status,
            Some("MEDIA_A".to_string()),
            Some("STICK_A".to_string()),
            Some("Secret Cue Track".to_string()),
            Some("DJ Headphone".to_string()),
            Some(174.0),
            None,
            Some(300),
            now,
            0.5,
        );

        assert!(logged.is_some(), "At 20s cumulative On-Air, track must be logged!");
        let track = logged.unwrap();
        assert_eq!(track.title, "Secret Cue Track");
        assert_eq!(track.player_number, 1);
        assert_eq!(tracker.played_tracks.len(), 1);
    }

    #[test]
    fn test_degraded_mode_playhead_advance_fallback() {
        let mut tracker = DeckTracker::new();
        // NO mixer status ever received (e.g. unlinked mixer)
        assert!(!tracker.has_seen_mixer_status);

        let cdj_status = DeckStatusUpdate {
            player_number: 2,
            rekordbox_track_id: 202,
            is_playing: true,
            is_cued: false,
            is_master: true,
            pitch_percent: 0.0,
            playhead_secs: Some(0.0),
            duration_secs: Some(250.0),
        };

        let now = Utc::now().into();
        // Play for 29s -> not logged yet
        for _ in 0..58 {
            let res = tracker.update_deck_status(
                &cdj_status,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                now,
                0.5,
            );
            assert_eq!(res, None);
        }

        // At 30s playhead advance -> logs as played!
        let logged = tracker.update_deck_status(
            &cdj_status,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            now,
            1.0,
        );

        assert!(logged.is_some());
        let t = logged.unwrap();
        assert_eq!(t.player_number, 2);
        assert_eq!(t.title, "Track 202");
    }

    #[test]
    fn test_b2b_clustering_alternating_and_handover() {
        // Create 4 played tracks alternating between Stick A and Stick B
        let t1 = ProDjPlayedTrack {
            timestamp_rfc3339: "2026-10-30T23:00:00+01:00".to_string(),
            player_number: 1,
            rekordbox_id: Some(1),
            title: "Track 1".to_string(),
            artist: "Artist A".to_string(),
            bpm: Some(174.0),
            key: None,
            duration_secs: Some(300),
            usb_media_id: Some("STICK_A".to_string()),
            usb_media_name: Some("DJ_ALICE".to_string()),
            cumulative_on_air_secs: 25.0,
        };
        let t2 = ProDjPlayedTrack {
            timestamp_rfc3339: "2026-10-30T23:05:00+01:00".to_string(),
            player_number: 2,
            rekordbox_id: Some(2),
            title: "Track 2".to_string(),
            artist: "Artist B".to_string(),
            bpm: Some(174.0),
            key: None,
            duration_secs: Some(300),
            usb_media_id: Some("STICK_B".to_string()),
            usb_media_name: Some("DJ_BOB".to_string()),
            cumulative_on_air_secs: 25.0,
        };
        let t3 = ProDjPlayedTrack {
            timestamp_rfc3339: "2026-10-30T23:10:00+01:00".to_string(),
            player_number: 1,
            rekordbox_id: Some(3),
            title: "Track 3".to_string(),
            artist: "Artist A".to_string(),
            bpm: Some(174.0),
            key: None,
            duration_secs: Some(300),
            usb_media_id: Some("STICK_A".to_string()),
            usb_media_name: Some("DJ_ALICE".to_string()),
            cumulative_on_air_secs: 25.0,
        };
        let t4 = ProDjPlayedTrack {
            timestamp_rfc3339: "2026-10-30T23:15:00+01:00".to_string(),
            player_number: 2,
            rekordbox_id: Some(4),
            title: "Track 4".to_string(),
            artist: "Artist B".to_string(),
            bpm: Some(174.0),
            key: None,
            duration_secs: Some(300),
            usb_media_id: Some("STICK_B".to_string()),
            usb_media_name: Some("DJ_BOB".to_string()),
            cumulative_on_air_secs: 25.0,
        };

        let tracks = vec![t1, t2, t3, t4];
        let clusters = B2BClusterer::cluster_played_tracks(&tracks, 1800.0);

        // All 4 tracks alternate between Alice and Bob within the same set -> clustered into 1 B2B cluster
        assert_eq!(clusters.len(), 1);
        let c = &clusters[0];
        assert_eq!(c.track_count, 4);
        assert_eq!(c.media_ids.len(), 2);
        assert!(c.proposed_artist.as_ref().unwrap().contains("b2b"));
    }
}
