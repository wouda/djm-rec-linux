use alsa::pcm::{Access, Format, HwParams, PCM};
use alsa::ValueOr;
use djm_common::profile::MixerProfile;
use rtrb::Producer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tracing::{error, info, warn};

#[derive(Error, Debug)]
pub enum CaptureError {
    #[error("ALSA error: {0}")]
    Alsa(#[from] alsa::Error),
    #[error("Device disconnected (-ENODEV / -EPIPE)")]
    Disconnected,
    #[error("Buffer overrun or ring buffer full")]
    BufferOverrun,
}

pub struct AlsaCapture {
    profile: MixerProfile,
    device_name: String,
    running: Arc<AtomicBool>,
}

impl AlsaCapture {
    pub fn new(profile: MixerProfile, device_name: &str, running: Arc<AtomicBool>) -> Self {
        Self {
            profile,
            device_name: device_name.to_string(),
            running,
        }
    }

    /// Captures audio in a dedicated real-time thread, de-interleaving the REC OUT channel pair
    /// and writing into the lock-free SPSC ring buffer.
    pub fn run_capture_loop(&self, mut producer: Producer<(i32, i32)>) -> Result<(), CaptureError> {
        info!(
            "Initializing ALSA capture for {} on device {} ({} channels, {} Hz)",
            self.profile.name, self.device_name, self.profile.total_channels, self.profile.native_sample_rate
        );

        let pcm = match PCM::new(&self.device_name, alsa::Direction::Capture, false) {
            Ok(p) => p,
            Err(e) => {
                warn!("Failed to open PCM capture device {}: {}", self.device_name, e);
                return Err(CaptureError::Alsa(e));
            }
        };

        // Configure hardware parameters
        let hwp = HwParams::any(&pcm)?;
        hwp.set_access(Access::RWInterleaved)?;

        // Pioneer / AlphaTheta USB mixers natively use S24_3LE (3-byte 24-bit PCM) on hw:X,0,
        // while plughw or other interfaces may use S32_LE or S24_LE (4-byte container).
        let (sample_format, bytes_per_sample) = if hwp.set_format(Format::S243LE).is_ok() {
            (Format::S243LE, 3usize)
        } else if hwp.set_format(Format::s32()).is_ok() {
            (Format::s32(), 4usize)
        } else {
            hwp.set_format(Format::s24())?;
            (Format::s24(), 4usize)
        };

        hwp.set_channels(self.profile.total_channels)?;
        hwp.set_rate(self.profile.native_sample_rate, ValueOr::Nearest)?;

        let buffer_size = (self.profile.native_sample_rate / 20) as alsa::pcm::Frames; // ~50ms buffer
        let period_size = (self.profile.native_sample_rate / 100) as alsa::pcm::Frames; // ~10ms period
        let _ = hwp.set_buffer_size_near(buffer_size);
        let _ = hwp.set_period_size_near(period_size, ValueOr::Nearest);

        pcm.hw_params(&hwp)?;

        let channels = self.profile.total_channels as usize;
        let (rec_l_idx, rec_r_idx) = (
            self.profile.rec_out_channels.0 as usize,
            self.profile.rec_out_channels.1 as usize,
        );

        let frames_per_period = period_size as usize;
        let frame_bytes = channels * bytes_per_sample;
        let mut read_buffer = vec![0u8; frames_per_period * frame_bytes];

        let io = pcm.io_bytes();

        info!(
            "Starting ALSA capture loop on {} (format={:?}, {} bytes/sample, rec_out=({}, {}))...",
            self.device_name, sample_format, bytes_per_sample, rec_l_idx, rec_r_idx
        );

        while self.running.load(Ordering::Relaxed) {
            match io.readi(&mut read_buffer) {
                Ok(frames_read) => {
                    for f in 0..frames_read {
                        let frame_offset = f * frame_bytes;
                        let l_off = frame_offset + rec_l_idx * bytes_per_sample;
                        let r_off = frame_offset + rec_r_idx * bytes_per_sample;

                        let (s24_l, s24_r) = if sample_format == Format::S243LE || sample_format == Format::s24() {
                            let l = ((read_buffer[l_off + 2] as i8 as i32) << 16)
                                | ((read_buffer[l_off + 1] as i32) << 8)
                                | (read_buffer[l_off] as i32);
                            let r = ((read_buffer[r_off + 2] as i8 as i32) << 16)
                                | ((read_buffer[r_off + 1] as i32) << 8)
                                | (read_buffer[r_off] as i32);
                            (l, r)
                        } else {
                            let l32 = i32::from_le_bytes([
                                read_buffer[l_off],
                                read_buffer[l_off + 1],
                                read_buffer[l_off + 2],
                                read_buffer[l_off + 3],
                            ]);
                            let r32 = i32::from_le_bytes([
                                read_buffer[r_off],
                                read_buffer[r_off + 1],
                                read_buffer[r_off + 2],
                                read_buffer[r_off + 3],
                            ]);
                            (l32 >> 8, r32 >> 8)
                        };

                        if producer.push((s24_l, s24_r)).is_err() {
                            // Ring buffer overrun, drop frame to maintain realtime capture
                        }
                    }
                }
                Err(err) => {
                    let errno = err.errno();
                    if errno == nix::libc::ENODEV || errno == nix::libc::EBADFD {
                        error!("ALSA capture device disconnected: {}", err);
                        return Err(CaptureError::Disconnected);
                    } else if errno == nix::libc::EAGAIN || errno == nix::libc::EINTR {
                        continue;
                    } else {
                        // Attempt xrun / suspend recovery
                        if let Err(rec_err) = pcm.try_recover(err, false) {
                            error!("ALSA recover failed: {}", rec_err);
                            return Err(CaptureError::Disconnected);
                        }
                    }
                }
            }
        }

        info!("ALSA capture loop terminated cleanly.");
        Ok(())
    }
}

/// Simulated capture generator for synthetic testing and integration tests without physical mixers
pub struct SyntheticCapture {
    sample_rate: u32,
    running: Arc<AtomicBool>,
}

impl SyntheticCapture {
    pub fn new(sample_rate: u32, running: Arc<AtomicBool>) -> Self {
        Self {
            sample_rate,
            running,
        }
    }

    pub fn run_test_tone(
        &self,
        mut producer: Producer<(i32, i32)>,
        freq_hz: f32,
        amplitude_ratio: f32,
        duration: Duration,
    ) {
        let total_samples = (self.sample_rate as f64 * duration.as_secs_f64()) as usize;
        let mut phase = 0.0f32;
        let phase_inc = (2.0 * std::f32::consts::PI * freq_hz) / self.sample_rate as f32;
        let max_amp = 8_388_607.0 * amplitude_ratio;

        for _ in 0..total_samples {
            if !self.running.load(Ordering::Relaxed) {
                break;
            }
            let sample_val = (phase.sin() * max_amp) as i32;
            let _ = producer.push((sample_val, sample_val));
            phase += phase_inc;
            if phase >= 2.0 * std::f32::consts::PI {
                phase -= 2.0 * std::f32::consts::PI;
            }
        }
    }

    /// Continuously generates simulated audio at real-time rate for mock mode testing
    pub fn run_continuous(
        &self,
        mut producer: Producer<(i32, i32)>,
        freq_hz: f32,
        amplitude_ratio: f32,
    ) {
        let chunk_samples = (self.sample_rate / 100) as usize; // 10ms chunk
        let mut phase = 0.0f32;
        let phase_inc = (2.0 * std::f32::consts::PI * freq_hz) / self.sample_rate as f32;
        let max_amp = 8_388_607.0 * amplitude_ratio;
        let chunk_interval = Duration::from_millis(10);

        while self.running.load(Ordering::Relaxed) {
            let start = std::time::Instant::now();
            let mut pushed = 0;
            while pushed < chunk_samples && self.running.load(Ordering::Relaxed) {
                let sample_val = (phase.sin() * max_amp) as i32;
                if producer.push((sample_val, sample_val)).is_ok() {
                    phase += phase_inc;
                    if phase >= 2.0 * std::f32::consts::PI {
                        phase -= 2.0 * std::f32::consts::PI;
                    }
                    pushed += 1;
                } else {
                    std::thread::sleep(Duration::from_millis(2));
                }
            }

            let elapsed = start.elapsed();
            if elapsed < chunk_interval {
                std::thread::sleep(chunk_interval - elapsed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtrb::RingBuffer;

    #[test]
    fn test_synthetic_capture_ingestion() {
        let running = Arc::new(AtomicBool::new(true));
        let synth = SyntheticCapture::new(48000, running.clone());
        let (producer, mut consumer) = RingBuffer::<(i32, i32)>::new(4800);

        synth.run_test_tone(producer, 440.0, 0.5, Duration::from_millis(50));

        let mut read_count = 0;
        while let Ok(sample) = consumer.pop() {
            read_count += 1;
            assert!(sample.0.abs() <= 8_388_607);
        }
        assert!(read_count > 0);
    }
}
