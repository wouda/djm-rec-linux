use serde::{Deserialize, Serialize};

pub const PEAK_THRESHOLD_24BIT: i32 = 7_919_340; // -0.5 dBFS of 8,388,607
pub const MAX_HEADROOM_PAD_DB: i8 = -19;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GainStepEvent {
    pub frame_offset: u64,
    pub old_pad_db: i8,
    pub new_pad_db: i8,
    pub peak_sample: i32,
    pub timestamp_epoch_secs: u64,
}

pub struct HeadroomGuard {
    enabled: bool,
    current_pad: i8,
    total_frames_processed: u64,
}

impl HeadroomGuard {
    pub fn new(enabled: bool, initial_pad: i8) -> Self {
        Self {
            enabled,
            current_pad: initial_pad,
            total_frames_processed: 0,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn current_pad(&self) -> i8 {
        self.current_pad
    }

    pub fn set_pad_manual(&mut self, pad: i8) {
        self.current_pad = pad;
    }

    /// Checks a block of stereo 24-bit samples. If peak exceeds -0.5 dBFS and current pad is > -19dB,
    /// steps down towards -19dB and returns a GainStepEvent.
    pub fn process_frames(&mut self, samples: &[(i32, i32)]) -> Option<GainStepEvent> {
        let mut max_abs_peak = 0i32;

        for &(l, r) in samples {
            self.total_frames_processed += 1;
            let abs_l = l.abs();
            let abs_r = r.abs();
            if abs_l > max_abs_peak {
                max_abs_peak = abs_l;
            }
            if abs_r > max_abs_peak {
                max_abs_peak = abs_r;
            }
        }

        if self.enabled && self.current_pad > MAX_HEADROOM_PAD_DB && max_abs_peak >= PEAK_THRESHOLD_24BIT {
            let old_pad = self.current_pad;
            // Step down sequence: -5dB -> -10dB -> -15dB -> -19dB
            let new_pad = match old_pad {
                -5 => -10,
                -10 => -15,
                _ => MAX_HEADROOM_PAD_DB,
            };
            self.current_pad = new_pad;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            Some(GainStepEvent {
                frame_offset: self.total_frames_processed,
                old_pad_db: old_pad,
                new_pad_db: new_pad,
                peak_sample: max_abs_peak,
                timestamp_epoch_secs: now,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headroom_guard_step_down() {
        let mut guard = HeadroomGuard::new(true, -10);

        // Safe frames: peak ~ -6 dBFS (4,000,000)
        let safe_samples = vec![(4_000_000, 3_800_000); 100];
        assert!(guard.process_frames(&safe_samples).is_none());
        assert_eq!(guard.current_pad(), -10);

        // Loud frame: peak >= -0.5 dBFS (7_950_000)
        let loud_samples = vec![(7_950_000, 5_000_000)];
        let event = guard.process_frames(&loud_samples).expect("guard should have stepped down");
        assert_eq!(event.old_pad_db, -10);
        assert_eq!(event.new_pad_db, -15);
        assert_eq!(guard.current_pad(), -15);

        // Next loud peak steps to -19 dB
        let event2 = guard.process_frames(&loud_samples).expect("guard should step to -19");
        assert_eq!(event2.old_pad_db, -15);
        assert_eq!(event2.new_pad_db, -19);
        assert_eq!(guard.current_pad(), -19);

        // Once at -19dB, no further step down even if loud
        assert!(guard.process_frames(&loud_samples).is_none());
        assert_eq!(guard.current_pad(), -19);
    }

    #[test]
    fn test_headroom_guard_disabled() {
        let mut guard = HeadroomGuard::new(false, -10);
        let loud_samples = vec![(8_300_000, 8_300_000)];
        assert!(guard.process_frames(&loud_samples).is_none());
        assert_eq!(guard.current_pad(), -10);
    }
}
