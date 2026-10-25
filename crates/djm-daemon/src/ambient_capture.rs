use alsa::pcm::{Access, Format, HwParams, PCM};
use alsa::ValueOr;
use djm_common::profile::match_mixer_from_card_id;
use rtrb::Producer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{error, info, warn};

#[derive(Error, Debug)]
pub enum AmbientCaptureError {
    #[error("ALSA error: {0}")]
    Alsa(#[from] alsa::Error),
    #[error("No host microphone detected")]
    DeviceNotFound,
    #[error("Capture stream ended or disconnected")]
    Disconnected,
}

/// Discovers the host machine's primary audio capture card, skipping DJ mixer cards.
pub fn detect_host_capture_device() -> Option<String> {
    for card in alsa::card::Iter::new() {
        if let Ok(card) = card {
            let card_name = card.get_name().unwrap_or_default();
            let card_longname = card.get_longname().unwrap_or_default();

            // Filter out Pioneer / AlphaTheta DJ mixers
            if match_mixer_from_card_id(&card_name).is_some()
                || match_mixer_from_card_id(&card_longname).is_some()
                || card_name.to_lowercase().contains("pioneer")
                || card_name.to_lowercase().contains("alphatheta")
            {
                continue;
            }

            // Test if card has capture PCM subdevices
            let dev_name = format!("plughw:{},0", card.get_index());
            if let Ok(pcm) = PCM::new(&dev_name, alsa::Direction::Capture, false) {
                drop(pcm);
                info!(
                    "Auto-detected host ambient microphone on card {} ('{}')",
                    card.get_index(),
                    card_name
                );
                return Some(dev_name);
            }
        }
    }

    // Fallback to ALSA "default" if specific hardware probe did not open
    if let Ok(pcm) = PCM::new("default", alsa::Direction::Capture, false) {
        drop(pcm);
        info!("Auto-detected host ambient microphone via ALSA 'default'");
        return Some("default".to_string());
    }

    None
}

/// Dedicated real-time capture loop for ambient crowd/venue microphone
pub struct AmbientCaptureLoop {
    device_name: String,
    target_sample_rate: u32,
    running: Arc<AtomicBool>,
}

impl AmbientCaptureLoop {
    pub fn new(device_name: &str, target_sample_rate: u32, running: Arc<AtomicBool>) -> Self {
        Self {
            device_name: device_name.to_string(),
            target_sample_rate,
            running,
        }
    }

    /// Captures audio from the host device mic, normalizes to 24-bit stereo PCM,
    /// and pushes into the ring buffer.
    pub fn run(&self, mut producer: Producer<(i32, i32)>) -> Result<(), AmbientCaptureError> {
        info!(
            "Starting Ambient ALSA capture on '{}' at target {} Hz",
            self.device_name, self.target_sample_rate
        );

        let pcm = match PCM::new(&self.device_name, alsa::Direction::Capture, false) {
            Ok(p) => p,
            Err(e) => {
                warn!("Could not open ambient PCM device '{}': {}", self.device_name, e);
                return Err(AmbientCaptureError::Alsa(e));
            }
        };

        let hwp = HwParams::any(&pcm)?;
        hwp.set_access(Access::RWInterleaved)?;
        // Allow S16, S24, or S32 formats via ALSA plug layer
        if hwp.set_format(Format::s24()).is_err() && hwp.set_format(Format::s16()).is_err() {
            hwp.set_format(Format::s32())?;
        }

        let channels = match hwp.set_channels(2) {
            Ok(_) => 2,
            Err(_) => {
                hwp.set_channels(1)?;
                1
            }
        };

        let _ = hwp.set_rate(self.target_sample_rate, ValueOr::Nearest);
        let actual_rate = hwp.get_rate()?;

        let period_size = (actual_rate / 50).max(128) as alsa::pcm::Frames; // ~20ms
        let buffer_size = period_size * 4;
        let _ = hwp.set_buffer_size_near(buffer_size);
        let _ = hwp.set_period_size_near(period_size, ValueOr::Nearest);

        pcm.hw_params(&hwp)?;

        info!(
            "Ambient capture configured: {} channels, {} Hz on '{}'",
            channels, actual_rate, self.device_name
        );

        let io = pcm.io_i32()?;
        let frames_per_period = period_size as usize;
        let mut read_buf = vec![0i32; frames_per_period * channels as usize];

        while self.running.load(Ordering::Relaxed) {
            match io.readi(&mut read_buf) {
                Ok(frames_read) => {
                    for f in 0..frames_read {
                        let (s_l, s_r) = if channels == 2 {
                            let idx = f * 2;
                            (read_buf[idx] >> 8, read_buf[idx + 1] >> 8)
                        } else {
                            let mono = read_buf[f] >> 8;
                            (mono, mono) // duplicate mono to stereo
                        };

                        let _ = producer.push((s_l, s_r));
                    }
                }
                Err(err) => {
                    let errno = err.errno();
                    if errno == nix::libc::ENODEV || errno == nix::libc::EBADFD {
                        error!("Ambient capture device disconnected: {}", err);
                        return Err(AmbientCaptureError::Disconnected);
                    } else if errno == nix::libc::EAGAIN || errno == nix::libc::EINTR {
                        continue;
                    } else {
                        // Attempt xrun recovery
                        if let Err(rec_err) = pcm.try_recover(err, false) {
                            error!("Ambient ALSA recover failed: {}", rec_err);
                            return Err(AmbientCaptureError::Alsa(rec_err));
                        }
                    }
                }
            }
        }

        info!("Ambient capture loop terminated cleanly.");
        Ok(())
    }
}

/// State machine managing the post-set trailing hangover recording duration.
#[derive(Debug, Clone)]
pub struct HangoverTimer {
    hangover_duration: Duration,
    started_at: Option<Instant>,
}

impl HangoverTimer {
    pub fn new(hangover_mins: u32) -> Self {
        Self {
            hangover_duration: Duration::from_secs((hangover_mins as u64) * 60),
            started_at: None,
        }
    }

    pub fn start(&mut self) {
        if self.started_at.is_none() {
            info!(
                "Mixer audio silent: starting ambient hangover timer for {:?}",
                self.hangover_duration
            );
            self.started_at = Some(Instant::now());
        }
    }

    pub fn cancel(&mut self) {
        if self.started_at.is_some() {
            info!("Mixer audio resumed: cancelled ambient hangover timer.");
            self.started_at = None;
        }
    }

    pub fn is_active(&self) -> bool {
        self.started_at.is_some()
    }

    pub fn is_expired(&self) -> bool {
        if let Some(start) = self.started_at {
            start.elapsed() >= self.hangover_duration
        } else {
            false
        }
    }

    pub fn remaining_time(&self) -> Option<Duration> {
        self.started_at.map(|start| {
            let elapsed = start.elapsed();
            if elapsed < self.hangover_duration {
                self.hangover_duration - elapsed
            } else {
                Duration::from_secs(0)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hangover_timer_lifecycle() {
        let mut timer = HangoverTimer::new(15);
        assert!(!timer.is_active());
        assert!(!timer.is_expired());

        timer.start();
        assert!(timer.is_active());
        assert!(!timer.is_expired());
        assert!(timer.remaining_time().unwrap() <= Duration::from_secs(15 * 60));

        // Test cancel on audio resume
        timer.cancel();
        assert!(!timer.is_active());
        assert_eq!(timer.remaining_time(), None);

        // Test timer with zero duration (immediate expiration)
        let zero_timer = HangoverTimer {
            hangover_duration: Duration::from_millis(1),
            started_at: Some(Instant::now() - Duration::from_millis(5)),
        };
        assert!(zero_timer.is_expired());
    }

    #[test]
    fn test_host_capture_detection_smoke() {
        // Smoke test detection function (will detect host card or default on Linux test runner)
        let dev = detect_host_capture_device();
        // Should not panic, and returns device string if soundcards exist
        if let Some(ref d) = dev {
            assert!(!d.is_empty());
        }
    }
}
