use djm_common::protocol::RecordingState;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GateAction {
    None,
    /// Level exceeded wake_threshold_dbfs -> Transition to Recording
    WakeToRecord,
    /// Silence persisted for pause_split_timeout_secs -> Transition to ArmedSilent and split
    SilenceTimeoutSplit,
}

pub struct SilenceGate {
    state: RecordingState,
    silence_threshold_dbfs: f32,
    wake_threshold_dbfs: f32,
    pause_split_timeout: Duration,
    current_silence_duration: Duration,
}

impl SilenceGate {
    pub const MAX_AMP_24BIT: f32 = 8_388_607.0;

    pub fn new(silence_threshold_dbfs: f32, wake_threshold_dbfs: f32, pause_split_timeout_secs: f64) -> Self {
        Self {
            state: RecordingState::Disconnected,
            silence_threshold_dbfs,
            wake_threshold_dbfs,
            pause_split_timeout: Duration::from_secs_f64(pause_split_timeout_secs),
            current_silence_duration: Duration::ZERO,
        }
    }

    pub fn state(&self) -> RecordingState {
        self.state
    }

    pub fn set_state(&mut self, state: RecordingState) {
        self.state = state;
        if state != RecordingState::Recording {
            self.current_silence_duration = Duration::ZERO;
        }
    }

    pub fn silence_countdown_remaining(&self) -> Option<f32> {
        if self.state == RecordingState::Recording && self.pause_split_timeout.as_secs_f64() > 0.0 {
            let elapsed = self.current_silence_duration.as_secs_f32();
            let total = self.pause_split_timeout.as_secs_f32();
            if elapsed > 0.0 {
                return Some((total - elapsed).max(0.0));
            }
        }
        None
    }

    pub fn is_silence_countdown_active(&self) -> bool {
        self.state == RecordingState::Recording && self.current_silence_duration > Duration::ZERO
    }

    /// Evaluates a block of audio samples and advances silence/wake timers.
    pub fn process_block(&mut self, samples: &[(i32, i32)], block_duration: Duration) -> GateAction {
        if self.state == RecordingState::Disconnected || self.state == RecordingState::Paused {
            return GateAction::None;
        }

        let mut max_abs = 0.0f32;
        for &(l, r) in samples {
            let abs_l = l.abs() as f32;
            let abs_r = r.abs() as f32;
            if abs_l > max_abs {
                max_abs = abs_l;
            }
            if abs_r > max_abs {
                max_abs = abs_r;
            }
        }

        let current_dbfs = if max_abs > 0.0 {
            20.0 * (max_abs / Self::MAX_AMP_24BIT).log10()
        } else {
            -144.0
        };

        match self.state {
            RecordingState::ArmedSilent => {
                if current_dbfs >= self.wake_threshold_dbfs {
                    self.state = RecordingState::Recording;
                    self.current_silence_duration = Duration::ZERO;
                    GateAction::WakeToRecord
                } else {
                    GateAction::None
                }
            }
            RecordingState::Recording => {
                if current_dbfs < self.silence_threshold_dbfs {
                    self.current_silence_duration += block_duration;
                    if self.pause_split_timeout.as_secs_f64() > 0.0
                        && self.current_silence_duration >= self.pause_split_timeout
                    {
                        self.state = RecordingState::ArmedSilent;
                        self.current_silence_duration = Duration::ZERO;
                        GateAction::SilenceTimeoutSplit
                    } else {
                        GateAction::None
                    }
                } else {
                    // Audio is above silence threshold, reset countdown
                    self.current_silence_duration = Duration::ZERO;
                    GateAction::None
                }
            }
            _ => GateAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wake_from_armed_silent() {
        let mut gate = SilenceGate::new(-75.0, -65.0, 10.0);
        gate.set_state(RecordingState::ArmedSilent);

        // Quiet audio (-80 dBFS): 8_388_607 * 10^(-80/20) ~ 838
        let quiet = vec![(800, 800); 100];
        assert_eq!(gate.process_block(&quiet, Duration::from_millis(50)), GateAction::None);
        assert_eq!(gate.state(), RecordingState::ArmedSilent);

        // Loud audio above wake (-50 dBFS): 8_388_607 * 10^(-50/20) ~ 26527
        let loud = vec![(30_000, 30_000); 100];
        assert_eq!(gate.process_block(&loud, Duration::from_millis(50)), GateAction::WakeToRecord);
        assert_eq!(gate.state(), RecordingState::Recording);
    }

    #[test]
    fn test_silence_timeout_split() {
        let mut gate = SilenceGate::new(-75.0, -65.0, 2.0); // 2 second timeout for test
        gate.set_state(RecordingState::Recording);

        let quiet = vec![(100, 100); 100]; // well below -75 dBFS
        let step = Duration::from_millis(500);

        // 0.5s silence
        assert_eq!(gate.process_block(&quiet, step), GateAction::None);
        assert!(gate.is_silence_countdown_active());

        // 1.0s silence
        assert_eq!(gate.process_block(&quiet, step), GateAction::None);

        // 1.5s silence
        assert_eq!(gate.process_block(&quiet, step), GateAction::None);

        // 2.0s silence -> split triggers!
        assert_eq!(gate.process_block(&quiet, step), GateAction::SilenceTimeoutSplit);
        assert_eq!(gate.state(), RecordingState::ArmedSilent);
    }

    #[test]
    fn test_silence_countdown_reset_by_sound() {
        let mut gate = SilenceGate::new(-75.0, -65.0, 5.0);
        gate.set_state(RecordingState::Recording);

        let quiet = vec![(100, 100); 100];
        gate.process_block(&quiet, Duration::from_secs(2));
        assert!(gate.is_silence_countdown_active());

        // Music resumes
        let loud = vec![(50_000, 50_000); 100];
        gate.process_block(&loud, Duration::from_millis(100));
        assert!(!gate.is_silence_countdown_active());
    }
}
