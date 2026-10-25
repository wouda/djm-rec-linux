use crate::alsa_capture::SyntheticCapture;
use crate::flac_writer::{scan_and_repair_orphaned_parts, FlacWriter};
use crate::headroom_guard::HeadroomGuard;
use crate::ipc_server::IpcServer;
use crate::preroll_buffer::PrerollBuffer;
use crate::session_log::SessionLog;
use crate::silence_gate::{GateAction, SilenceGate};
use anyhow::Result;
use djm_common::config::AppConfig;
use djm_common::profile::{find_profile_by_usb_id, match_mixer_from_card_id, MixerProfile};
use djm_common::protocol::{ControlCommand, RecordingState, Telemetry};
use rtrb::RingBuffer;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

pub struct AudioEngine {
    config: AppConfig,
    running: Arc<AtomicBool>,
}

impl AudioEngine {
    pub fn new(config: AppConfig, running: Arc<AtomicBool>) -> Self {
        Self { config, running }
    }

    pub async fn run(&mut self, mock_mode: bool) -> Result<()> {
        let output_dir = self.config.storage.resolved_output_dir();
        std::fs::create_dir_all(&output_dir)?;

        // Auto-heal any orphaned .flac.part files on startup
        let _ = scan_and_repair_orphaned_parts(&output_dir);

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<ControlCommand>(32);
        let socket_path = IpcServer::default_socket_path();
        let (ipc, telemetry_tx) = IpcServer::new(socket_path, cmd_tx, self.running.clone());

        // Spawn IPC server
        tokio::spawn(async move {
            if let Err(e) = ipc.run().await {
                error!("IPC server error: {}", e);
            }
        });

        info!("Audio Engine started. Awaiting DJ mixer connection...");

        // Setup ring buffer for real-time audio (100ms capacity)
        let sample_rate = 96000;
        let ring_capacity = (sample_rate / 10) as usize;
        let (producer, mut consumer) = RingBuffer::<(i32, i32)>::new(ring_capacity);

        // Pre-roll buffer (default 2 seconds in RAM)
        let preroll_capacity = (sample_rate as f64 * self.config.splitting.preroll_buffer_secs) as usize;
        let mut preroll = PrerollBuffer::new(preroll_capacity);

        // Silence gate
        let mut silence_gate = SilenceGate::new(
            self.config.splitting.silence_threshold_dbfs,
            self.config.splitting.wake_threshold_dbfs,
            self.config.splitting.pause_split_timeout_secs,
        );

        // Headroom guard
        let mut headroom_guard = HeadroomGuard::new(
            self.config.gain_and_hardware.auto_headroom_guard,
            -19,
        );

        let mut active_writer: Option<FlacWriter> = None;
        let mut active_log: Option<SessionLog> = None;
        let mut file_start_time = Instant::now();
        let mut segment_index = 1;

        let mut clip_counter = 0u16;
        let mut peak_l_hold = -9600i16;
        let mut peak_r_hold = -9600i16;

        let active_mixer_profile: Option<MixerProfile> = if mock_mode {
            Some(find_profile_by_usb_id(0x2b73, 0x000a).unwrap())
        } else {
            Self::probe_connected_mixer()
        };

        if active_mixer_profile.is_some() {
            silence_gate.set_state(RecordingState::ArmedSilent);
        } else {
            silence_gate.set_state(RecordingState::Disconnected);
        }

        // If in mock mode, spawn a continuous simulated audio generator
        if mock_mode {
            let synth_running = self.running.clone();
            let mock_producer = producer;
            tokio::task::spawn_blocking(move || {
                let synth = SyntheticCapture::new(48000, synth_running.clone());
                synth.run_continuous(mock_producer, 440.0, 0.4);
            });
        }

        let mut last_telemetry_emit = Instant::now();
        let telemetry_period = Duration::from_millis(25); // 40 Hz telemetry emission
        let mut last_status_log_time = Instant::now();
        let mut last_status_log_mb = 0u64;

        while self.running.load(Ordering::Relaxed) {
            // Check incoming control commands
            while let Ok(cmd) = cmd_rx.try_recv() {
                match cmd {
                    ControlCommand::Auth(_) => {}
                    ControlCommand::DropBookmark(note) => {
                        let note_str = note.unwrap_or_else(|| "Manual Bookmark".to_string());
                        if let Some(ref mut logger) = active_log {
                            let _ = logger.log_event("BOOKMARK", &note_str);
                        }
                    }
                    ControlCommand::SplitFile => {
                        if active_writer.is_some() {
                            info!("Manual file split triggered");
                            Self::split_active_file(
                                &mut active_writer,
                                &mut active_log,
                                &output_dir,
                                &active_mixer_profile,
                                &mut segment_index,
                                sample_rate,
                                self.config.storage.flac_compression_level,
                                self.config.storage.fsync_interval_secs,
                                file_start_time,
                                headroom_guard.current_pad(),
                                headroom_guard.is_enabled(),
                                peak_l_hold.max(peak_r_hold),
                                clip_counter,
                                "Manual Split via Remote/CLI/TUI",
                            )?;
                            file_start_time = Instant::now();
                            last_status_log_time = Instant::now();
                            last_status_log_mb = 0;
                        }
                    }
                    ControlCommand::SetHardwarePad(pad) => {
                        info!("Setting hardware capture pad to {} dB", pad);
                        headroom_guard.set_pad_manual(pad);
                        if let Some(ref mut logger) = active_log {
                            let _ = logger.log_event("PAD_MANUAL", &format!("Pad set to {} dB", pad));
                        }
                    }
                    ControlCommand::ToggleAutoGuard => {
                        let new_val = !headroom_guard.is_enabled();
                        headroom_guard.set_enabled(new_val);
                        info!("Auto-headroom guard toggled to {}", new_val);
                    }
                    ControlCommand::ResetPeaks => {
                        clip_counter = 0;
                        peak_l_hold = -9600;
                        peak_r_hold = -9600;
                    }
                    ControlCommand::OpenPairingWindow => {}
                }
            }

            // Drain audio samples from the SPSC ring buffer
            let mut chunk = Vec::with_capacity(512);
            while let Ok(sample) = consumer.pop() {
                chunk.push(sample);
                if chunk.len() >= 1024 {
                    break;
                }
            }

            if !chunk.is_empty() {
                // Check Auto-Headroom Guard
                if let Some(step_event) = headroom_guard.process_frames(&chunk) {
                    warn!(
                        "Auto-Headroom Guard triggered! Stepped pad from {} dB to {} dB",
                        step_event.old_pad_db, step_event.new_pad_db
                    );
                    if let Some(ref mut logger) = active_log {
                        let _ = logger.log_event(
                            "AUTO_GUARD_STEP",
                            &format!(
                                "Peak sample {} exceeded threshold. Stepped pad from {} dB to {} dB at frame {}",
                                step_event.peak_sample, step_event.old_pad_db, step_event.new_pad_db, step_event.frame_offset
                            ),
                        );
                    }
                }

                // Process DSP Levels
                let (peak_l, peak_r, _rms_l, _rms_r, clips) = Self::calculate_dsp_metrics(&chunk);
                if peak_l > peak_l_hold { peak_l_hold = peak_l; }
                if peak_r > peak_r_hold { peak_r_hold = peak_r; }
                clip_counter = clip_counter.saturating_add(clips);

                // Process Silence Gate
                let block_duration = Duration::from_secs_f64(chunk.len() as f64 / sample_rate as f64);
                let gate_action = silence_gate.process_block(&chunk, block_duration);

                match gate_action {
                    GateAction::WakeToRecord => {
                        info!("Silence gate woke! Audio detected. Starting recording...");
                        let mut writer = Self::create_new_flac_segment(
                            &output_dir,
                            &active_mixer_profile,
                            segment_index,
                            sample_rate,
                            self.config.storage.flac_compression_level,
                            self.config.storage.fsync_interval_secs,
                        )?;

                        // Prepend pre-roll buffer
                        let preroll_audio = preroll.drain_chronological();
                        if !preroll_audio.is_empty() {
                            info!("Prepending {} pre-roll samples to recording", preroll_audio.len());
                            writer.write_interleaved(&preroll_audio)?;
                        }

                        // Write current chunk
                        writer.write_interleaved(&chunk)?;
                        file_start_time = Instant::now();
                        last_status_log_time = Instant::now();
                        last_status_log_mb = 0;

                        let mut log = SessionLog::new(writer.target_path())?;
                        let mixer_name = active_mixer_profile.as_ref().map(|p| p.name).unwrap_or("Pioneer DJ Mixer");
                        let free_gb = Self::get_free_disk_space_tenths(&output_dir) as f32 / 10.0;
                        let _ = log.log_start(
                            writer.target_path(),
                            mixer_name,
                            sample_rate,
                            headroom_guard.current_pad(),
                            headroom_guard.is_enabled(),
                            self.config.storage.flac_compression_level,
                            self.config.storage.fsync_interval_secs,
                            free_gb,
                        );
                        active_writer = Some(writer);
                        active_log = Some(log);
                        Self::set_system_power_profile_async("performance");
                    }
                    GateAction::SilenceTimeoutSplit => {
                        info!("Silence timeout reached. Closing current recording segment...");
                        Self::set_system_power_profile_async("balanced");
                        if let Some(mut log) = active_log.take() {
                            let dur = file_start_time.elapsed().as_secs();
                            let total_mb = if let Some(ref w) = active_writer { (w.total_samples() * 6 / (1024 * 1024)) as u64 } else { 0 };
                            let peak_db = (peak_l_hold.max(peak_r_hold) as f32) / 100.0;
                            let _ = log.log_stop("Silence Timeout Gate", dur, total_mb, peak_db, clip_counter);
                        }
                        if let Some(writer) = active_writer.take() {
                            let _ = writer.finalize();
                        }
                        segment_index += 1;
                    }
                    GateAction::None => {
                        if silence_gate.state() == RecordingState::ArmedSilent {
                            preroll.push_block(&chunk);
                        } else if silence_gate.state() == RecordingState::Recording {
                            if let Some(ref mut writer) = active_writer {
                                writer.write_interleaved(&chunk)?;

                                // Check max segment duration rollover
                                if self.config.splitting.max_segment_duration_mins > 0 {
                                    let max_dur = Duration::from_secs(
                                        self.config.splitting.max_segment_duration_mins as u64 * 60,
                                    );
                                    if file_start_time.elapsed() >= max_dur {
                                        info!("Max segment duration reached. Performing zero-gap rollover...");
                                        Self::split_active_file(
                                            &mut active_writer,
                                            &mut active_log,
                                            &output_dir,
                                            &active_mixer_profile,
                                            &mut segment_index,
                                            sample_rate,
                                            self.config.storage.flac_compression_level,
                                            self.config.storage.fsync_interval_secs,
                                            file_start_time,
                                            headroom_guard.current_pad(),
                                            headroom_guard.is_enabled(),
                                            peak_l_hold.max(peak_r_hold),
                                            clip_counter,
                                            "Max Segment Duration Rollover",
                                        )?;
                                        file_start_time = Instant::now();
                                        last_status_log_time = Instant::now();
                                        last_status_log_mb = 0;
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }

            // Emit Telemetry at periodic interval
            if last_telemetry_emit.elapsed() >= telemetry_period {
                let duration_secs = if active_writer.is_some() {
                    file_start_time.elapsed().as_secs() as u32
                } else {
                    0
                };

                let file_size_mb = if let Some(ref writer) = active_writer {
                    (writer.total_samples() * 6 / (1024 * 1024)) as u16
                } else {
                    0
                };

                // Periodic MB written progress updates to session log (every 60s or every 50 MB, without spamming)
                if let Some(ref mut logger) = active_log {
                    let now = Instant::now();
                    let current_mb = file_size_mb as u64;
                    if now.duration_since(last_status_log_time) >= Duration::from_secs(60)
                        || (current_mb >= last_status_log_mb + 50 && now.duration_since(last_status_log_time) >= Duration::from_secs(15))
                    {
                        last_status_log_time = now;
                        last_status_log_mb = current_mb;
                        let dur = file_start_time.elapsed().as_secs();
                        let peak_db = (peak_l_hold.max(peak_r_hold) as f32) / 100.0;
                        let _ = logger.log_progress(current_mb, dur, peak_db, clip_counter, headroom_guard.current_pad());
                    }
                }

                let free_disk_tenths = Self::get_free_disk_space_tenths(&output_dir);

                let telemetry = Telemetry {
                    state: silence_gate.state(),
                    hw_pad_db: headroom_guard.current_pad(),
                    auto_guard_active: headroom_guard.is_enabled(),
                    silence_countdown_active: silence_gate.is_silence_countdown_active(),
                    peak_l_centi_db: peak_l_hold,
                    peak_r_centi_db: peak_r_hold,
                    rms_l_centi_db: -2400,
                    rms_r_centi_db: -2400,
                    clip_count: clip_counter,
                    active_file_duration_secs: duration_secs,
                    active_file_size_mb: file_size_mb,
                    free_disk_gb_tenths: free_disk_tenths,
                    mixer_model: active_mixer_profile
                        .as_ref()
                        .map(|p| p.name.to_string())
                        .unwrap_or_else(|| "No Mixer".to_string()),
                    sample_rate,
                    active_filename: active_writer.as_ref().map(|w| {
                        w.target_path()
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    }),
                    pause_countdown_secs: silence_gate.silence_countdown_remaining(),
                    recent_logs: Vec::new(),
                };

                let _ = telemetry_tx.send(telemetry);
                last_telemetry_emit = Instant::now();
            }
        }

        // Clean shutdown: finalize open session log and FLAC file
        if let Some(mut log) = active_log.take() {
            let dur = file_start_time.elapsed().as_secs();
            let total_mb = if let Some(ref w) = active_writer {
                (w.total_samples() * 6 / (1024 * 1024)) as u64
            } else {
                0
            };
            let peak_db = (peak_l_hold.max(peak_r_hold) as f32) / 100.0;
            let _ = log.log_stop("Daemon Shutdown", dur, total_mb, peak_db, clip_counter);
        }
        if let Some(writer) = active_writer.take() {
            let _ = writer.finalize();
        }

        Self::set_system_power_profile_async("balanced");
        info!("Audio Engine shutdown complete.");
        Ok(())
    }

    fn set_system_power_profile_async(profile: &'static str) {
        std::thread::spawn(move || {
            let _ = std::process::Command::new("powerprofilesctl")
                .args(["set", profile])
                .output();
        });
    }

    fn calculate_dsp_metrics(samples: &[(i32, i32)]) -> (i16, i16, i16, i16, u16) {
        let mut max_l = 0i32;
        let mut max_r = 0i32;
        let mut sum_sq_l = 0.0f64;
        let mut sum_sq_r = 0.0f64;
        let mut clips = 0u16;

        const CLIP_THRESHOLD: i32 = 8_388_600; // >= 0 dBFS in 24-bit

        for &(l, r) in samples {
            let abs_l = l.abs();
            let abs_r = r.abs();
            if abs_l > max_l { max_l = abs_l; }
            if abs_r > max_r { max_r = abs_r; }
            if abs_l >= CLIP_THRESHOLD || abs_r >= CLIP_THRESHOLD {
                clips = clips.saturating_add(1);
            }

            let fl = l as f64 / 8_388_607.0;
            let fr = r as f64 / 8_388_607.0;
            sum_sq_l += fl * fl;
            sum_sq_r += fr * fr;
        }

        let peak_l_cdb = Self::amp_to_centi_db(max_l);
        let peak_r_cdb = Self::amp_to_centi_db(max_r);

        let n = samples.len().max(1) as f64;
        let rms_l = (sum_sq_l / n).sqrt().max(1e-9);
        let rms_r = (sum_sq_r / n).sqrt().max(1e-9);
        let rms_l_cdb = (2000.0 * rms_l.log10()) as i16;
        let rms_r_cdb = (2000.0 * rms_r.log10()) as i16;

        (peak_l_cdb, peak_r_cdb, rms_l_cdb, rms_r_cdb, clips)
    }

    fn amp_to_centi_db(amp: i32) -> i16 {
        if amp <= 0 {
            -14400
        } else {
            let ratio = amp as f64 / 8_388_607.0;
            (2000.0 * ratio.max(1e-9).log10()) as i16
        }
    }

    fn probe_connected_mixer() -> Option<MixerProfile> {
        // Scans ALSA cards for Pioneer/AlphaTheta DJ mixers
        if let Ok(cards) = alsa::card::Iter::new().collect::<Result<Vec<_>, _>>() {
            for card in cards {
                if let Ok(name) = card.get_name() {
                    if let Some(prof) = match_mixer_from_card_id(&name) {
                        info!("Detected connected DJ mixer: {} (card {})", prof.name, card.get_index());
                        return Some(prof);
                    }
                }
            }
        }
        None
    }

    fn create_new_flac_segment(
        output_dir: &Path,
        profile: &Option<MixerProfile>,
        segment_index: usize,
        sample_rate: u32,
        compression_level: u32,
        sync_interval_secs: u64,
    ) -> Result<FlacWriter> {
        let mixer_name = profile
            .as_ref()
            .map(|p| p.name.replace(' ', "_"))
            .unwrap_or_else(|| "DJM".to_string());

        let timestamp = chrono_date_time_string();
        let filename = format!("{}_{}_pt{:02}.flac", mixer_name, timestamp, segment_index);
        let target_path = output_dir.join(filename);

        FlacWriter::new(&target_path, sample_rate, compression_level, sync_interval_secs)
    }

    fn split_active_file(
        writer: &mut Option<FlacWriter>,
        log: &mut Option<SessionLog>,
        output_dir: &Path,
        profile: &Option<MixerProfile>,
        segment_index: &mut usize,
        sample_rate: u32,
        compression_level: u32,
        sync_interval_secs: u64,
        file_start_time: Instant,
        current_pad: i8,
        auto_guard: bool,
        peak_hold: i16,
        clips: u16,
        reason: &str,
    ) -> Result<()> {
        let dur = file_start_time.elapsed().as_secs();
        let total_mb = if let Some(ref w) = writer {
            (w.total_samples() * 6 / (1024 * 1024)) as u64
        } else {
            0
        };
        let peak_db = (peak_hold as f32) / 100.0;

        if let Some(mut old_log) = log.take() {
            let _ = old_log.log_stop(reason, dur, total_mb, peak_db, clips);
        }
        if let Some(old_writer) = writer.take() {
            // Offload old file finalization asynchronously so new file starts on next sample without dropping frames
            tokio::task::spawn_blocking(move || {
                let _ = old_writer.finalize();
            });
        }

        *segment_index += 1;
        let new_writer = Self::create_new_flac_segment(
            output_dir,
            profile,
            *segment_index,
            sample_rate,
            compression_level,
            sync_interval_secs,
        )?;

        let mut new_log = SessionLog::new(new_writer.target_path())?;
        let mixer_name = profile.as_ref().map(|p| p.name).unwrap_or("Pioneer DJ Mixer");
        let free_gb = Self::get_free_disk_space_tenths(output_dir) as f32 / 10.0;
        let _ = new_log.log_start(
            new_writer.target_path(),
            mixer_name,
            sample_rate,
            current_pad,
            auto_guard,
            compression_level,
            sync_interval_secs,
            free_gb,
        );

        *writer = Some(new_writer);
        *log = Some(new_log);

        Ok(())
    }

    fn get_free_disk_space_tenths(dir: &Path) -> u16 {
        // Simple statvfs wrapper for remaining disk space
        if let Ok(c_path) = std::ffi::CString::new(dir.to_string_lossy().as_bytes()) {
            unsafe {
                let mut stat: nix::libc::statvfs = std::mem::zeroed();
                if nix::libc::statvfs(c_path.as_ptr(), &mut stat) == 0 {
                    let free_bytes = stat.f_bavail as u64 * stat.f_frsize as u64;
                    let free_gb = free_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                    return (free_gb * 10.0).clamp(0.0, 65535.0) as u16;
                }
            }
        }
        1000 // default ~100.0 GB
    }
}

fn chrono_date_time_string() -> String {
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs();
    let rem_secs = secs % 86400;
    let h = (rem_secs / 3600) as u32;
    let m = ((rem_secs % 3600) / 60) as u32;
    let s = (rem_secs % 60) as u32;

    // Gregorian calendar calculation from unix epoch days
    let days = (secs / 86400) as i64;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1024 + doe / 1461 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mon = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if mon <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02}_{:02}-{:02}-{:02}", year, mon, d, h, m, s)
}
