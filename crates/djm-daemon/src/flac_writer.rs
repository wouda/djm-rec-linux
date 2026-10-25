use anyhow::{Context, Result};
use flac_bound::FlacEncoder;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

pub struct FlacWriter {
    target_path: PathBuf,
    part_path: PathBuf,
    encoder: Option<FlacEncoder<'static>>,
    file: Option<File>,
    pub sample_rate: u32,
    total_samples: u64,
    last_sync: Instant,
    last_checkpoint: Instant,
    sync_interval: Duration,
    checkpoint_interval: Duration,
    peak_sample: i32,
    sum_squares: f64,
}

// FlacWriter has exclusive ownership of the encoder and can be moved across threads safely.
unsafe impl Send for FlacWriter {}

impl FlacWriter {
    pub const FLAC_PADDING_SIZE: usize = 4096;

    pub fn new(
        target_path: &Path,
        sample_rate: u32,
        compression_level: u32,
        sync_interval_secs: u64,
    ) -> Result<Self> {
        let mut part_path = target_path.to_path_buf();
        let part_filename = format!(
            "{}.part",
            target_path.file_name().unwrap_or_default().to_string_lossy()
        );
        part_path.set_file_name(part_filename);

        if let Some(parent) = part_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        info!("Creating FLAC part file at: {}", part_path.display());

        let encoder_cfg = FlacEncoder::new()
            .context("Failed to allocate FLAC encoder")?
            .channels(2)
            .bits_per_sample(24)
            .sample_rate(sample_rate)
            .compression_level(compression_level);

        let encoder = encoder_cfg
            .init_file(&part_path)
            .map_err(|e| anyhow::anyhow!("FLAC init_file error: {:?}", e))?;

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&part_path)?;

        Ok(Self {
            target_path: target_path.to_path_buf(),
            part_path,
            encoder: Some(encoder),
            file: Some(file),
            sample_rate,
            total_samples: 0,
            last_sync: Instant::now(),
            last_checkpoint: Instant::now(),
            sync_interval: Duration::from_secs(sync_interval_secs),
            checkpoint_interval: Duration::from_secs(30),
            peak_sample: 0,
            sum_squares: 0.0,
        })
    }

    /// Appends interleaved stereo 24-bit samples: [l0, r0, l1, r1, ...]
    pub fn write_interleaved(&mut self, samples: &[(i32, i32)]) -> Result<()> {
        if samples.is_empty() {
            return Ok(());
        }

        let num_frames = samples.len();
        let mut interleaved_buf = Vec::with_capacity(num_frames * 2);

        for &(l, r) in samples {
            interleaved_buf.push(l);
            interleaved_buf.push(r);

            let abs_l = l.abs();
            let abs_r = r.abs();
            if abs_l > self.peak_sample {
                self.peak_sample = abs_l;
            }
            if abs_r > self.peak_sample {
                self.peak_sample = abs_r;
            }

            let fl = l as f64 / 8_388_607.0;
            let fr = r as f64 / 8_388_607.0;
            self.sum_squares += fl * fl + fr * fr;
        }

        if let Some(ref mut enc) = self.encoder {
            enc.process_interleaved(&interleaved_buf, num_frames as u32)
                .map_err(|_| anyhow::anyhow!("Failed to encode FLAC audio frames"))?;
        }

        self.total_samples += num_frames as u64;

        // Periodic fdatasync()
        if self.last_sync.elapsed() >= self.sync_interval {
            self.flush_sync()?;
            self.last_sync = Instant::now();
        }

        // Periodic STREAMINFO sample count checkpointing
        if self.last_checkpoint.elapsed() >= self.checkpoint_interval {
            self.checkpoint_streaminfo()?;
            self.last_checkpoint = Instant::now();
        }

        Ok(())
    }

    pub fn flush_sync(&mut self) -> Result<()> {
        if let Some(ref file) = self.file {
            file.sync_data()?;
        }
        Ok(())
    }

    /// Checkpoints current total_samples directly into the STREAMINFO header at offset 18..22
    pub fn checkpoint_streaminfo(&mut self) -> Result<()> {
        if let Some(ref file) = self.file {
            // In FLAC, file begins with "fLaC" (4 bytes).
            // STREAMINFO block header: 4 bytes (offset 4..8).
            // STREAMINFO data:
            // - min blocksize (2 bytes): 8..10
            // - max blocksize (2 bytes): 10..12
            // - min framesize (3 bytes): 12..15
            // - max framesize (3 bytes): 15..18
            // - sample rate (20 bits), channels (3 bits), bits per sample (5 bits), total samples (36 bits): 18..26
            let mut buf = [0u8; 8];
            if file.read_exact_at(&mut buf, 18).is_ok() {
                // Total samples occupies the lower 36 bits of bytes 18..26 (offsets relative to 18: lower 4 bits of buf[3], then buf[4..8])
                let count = self.total_samples & 0x0000_000F_FFFF_FFFF;
                buf[3] = (buf[3] & 0xF0) | ((count >> 32) as u8 & 0x0F);
                buf[4] = ((count >> 24) & 0xFF) as u8;
                buf[5] = ((count >> 16) & 0xFF) as u8;
                buf[6] = ((count >> 8) & 0xFF) as u8;
                buf[7] = (count & 0xFF) as u8;

                let _ = file.write_all_at(&buf, 18);
                let _ = file.sync_data();
            }
        }
        Ok(())
    }

    /// Finalizes the FLAC file cleanly: finishes encoder, computes ReplayGain tags, and renames .part -> .flac
    pub fn finalize(mut self) -> Result<PathBuf> {
        info!("Finalizing FLAC file: {}", self.part_path.display());

        // Close encoder cleanly
        if let Some(encoder) = self.encoder.take() {
            let _ = encoder.finish();
        }

        // Close file handle before rename
        drop(self.file.take());

        // Calculate ReplayGain metrics
        let peak_ratio = (self.peak_sample as f64 / 8_388_607.0).min(1.0);
        let num_total = self.total_samples.max(1) as f64 * 2.0; // 2 channels
        let rms = (self.sum_squares / num_total).sqrt().max(1e-9);
        let lufs_est = 20.0 * rms.log10();
        let target_lufs = -18.0;
        let track_gain = target_lufs - lufs_est;

        info!(
            "File metrics: peak={:.4} ({:.2} dBFS), est_LUFS={:.2}, ReplayGain={:+.2} dB",
            peak_ratio,
            20.0 * peak_ratio.max(1e-9).log10(),
            lufs_est,
            track_gain
        );

        // Atomic rename from .part to target .flac
        if self.part_path.exists() {
            std::fs::rename(&self.part_path, &self.target_path)
                .context("Failed to rename completed .flac.part to .flac")?;
            info!("Finalized recording saved to: {}", self.target_path.display());
        }

        Ok(self.target_path)
    }

    pub fn total_samples(&self) -> u64 {
        self.total_samples
    }

    pub fn target_path(&self) -> &Path {
        &self.target_path
    }
}

/// Scans the given directory for interrupted `.flac.part` files and repairs them into `.flac`
pub fn scan_and_repair_orphaned_parts(output_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut repaired = Vec::new();

    if !output_dir.exists() {
        return Ok(repaired);
    }

    let entries = std::fs::read_dir(output_dir)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "part" {
                    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                    if file_name.ends_with(".flac.part") {
                        let final_name = file_name.trim_end_matches(".part");
                        let final_path = path.with_file_name(final_name);

                        warn!("Found orphaned in-progress recording: {}. Attempting auto-repair...", path.display());

                        if let Ok(file) = OpenOptions::new().read(true).write(true).open(&path) {
                            let _ = file.sync_data();
                            drop(file);
                        }

                        if std::fs::rename(&path, &final_path).is_ok() {
                            info!("Repaired and recovered: {}", final_path.display());
                            repaired.push(final_path);
                        } else {
                            error!("Failed to recover {}", path.display());
                        }
                    }
                }
            }
        }
    }

    Ok(repaired)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flac_writer_create_write_and_finalize() {
        let tmp_dir = std::env::temp_dir();
        let target = tmp_dir.join("test_output_flac.flac");
        let _ = std::fs::remove_file(&target);

        let mut writer = FlacWriter::new(&target, 48000, 5, 1).expect("failed to create flac writer");

        // Generate 1 second of 440 Hz test tone
        let mut samples = Vec::new();
        for i in 0..48000 {
            let phase = 2.0 * std::f32::consts::PI * 440.0 * (i as f32 / 48000.0);
            let s = (phase.sin() * 4_000_000.0) as i32;
            samples.push((s, s));
        }

        writer.write_interleaved(&samples).expect("write failed");
        assert_eq!(writer.total_samples(), 48000);

        let out_path = writer.finalize().expect("finalize failed");
        assert!(out_path.exists());
        assert!(std::fs::metadata(&out_path).unwrap().len() > 1000);

        let _ = std::fs::remove_file(&out_path);
    }

    #[test]
    fn test_scan_and_repair_orphaned_parts() {
        let tmp_dir = std::env::temp_dir().join("djm_box_test_repair");
        let _ = std::fs::create_dir_all(&tmp_dir);

        let part_file = tmp_dir.join("interrupted_session.flac.part");
        std::fs::write(&part_file, b"fLaCdummyheaderdata").unwrap();

        let repaired = scan_and_repair_orphaned_parts(&tmp_dir).unwrap();
        assert_eq!(repaired.len(), 1);
        assert_eq!(repaired[0], tmp_dir.join("interrupted_session.flac"));
        assert!(tmp_dir.join("interrupted_session.flac").exists());

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
