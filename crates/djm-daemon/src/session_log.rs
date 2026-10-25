use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tracing::info;

pub struct SessionLog {
    log_path: PathBuf,
    file: Option<File>,
}

impl SessionLog {
    pub fn new(flac_path: &Path) -> io::Result<Self> {
        let mut log_path = flac_path.to_path_buf();
        log_path.set_extension("log");

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        let mut logger = Self {
            log_path,
            file: Some(file),
        };
        logger.log_event("SESSION_INIT", &format!("Session log opened for {}", flac_path.display()))?;
        Ok(logger)
    }

    pub fn log_start(
        &mut self,
        flac_path: &Path,
        mixer: &str,
        sample_rate: u32,
        pad_db: i8,
        auto_guard: bool,
        compression_level: u32,
        fsync_secs: u64,
        free_gb: f32,
    ) -> io::Result<()> {
        let guard_str = if auto_guard { "Enabled" } else { "Disabled" };
        self.log_event("SESSION_START", &format!("Opened session recording for: {}", flac_path.display()))?;
        self.log_event("HARDWARE", &format!("Mixer: {} (Native sample rate: {} Hz, 24-bit stereo)", mixer, sample_rate))?;
        self.log_event("GAIN_STAGING", &format!("Hardware Capture Pad: {} dB | Auto-Headroom Guard: {}", pad_db, guard_str))?;
        self.log_event("STORAGE", &format!("FLAC Compression: Level {} | fsync interval: {}s | Free disk: {:.1} GB", compression_level, fsync_secs, free_gb))?;
        Ok(())
    }

    pub fn log_progress(
        &mut self,
        mb: u64,
        elapsed_secs: u64,
        peak_db: f32,
        clips: u16,
        pad_db: i8,
    ) -> io::Result<()> {
        let hh = elapsed_secs / 3600;
        let mm = (elapsed_secs % 3600) / 60;
        let ss = elapsed_secs % 60;
        self.log_event(
            "STATUS",
            &format!(
                "Progress update: {} MB written | Elapsed: {:02}:{:02}:{:02} | Current Peak: {:.1} dBFS | Clips: {} | Pad: {} dB",
                mb, hh, mm, ss, peak_db, clips, pad_db
            ),
        )
    }

    pub fn log_stop(
        &mut self,
        reason: &str,
        elapsed_secs: u64,
        total_mb: u64,
        peak_db: f32,
        clips: u16,
    ) -> io::Result<()> {
        let hh = elapsed_secs / 3600;
        let mm = (elapsed_secs % 3600) / 60;
        let ss = elapsed_secs % 60;
        self.log_event(
            "SESSION_STOP",
            &format!(
                "Session closed ({}) | Total duration: {:02}:{:02}:{:02} | Total size: {} MB | Max peak: {:.1} dBFS | Total clips: {}",
                reason, hh, mm, ss, total_mb, peak_db, clips
            ),
        )
    }

    pub fn log_event(&mut self, event: &str, message: &str) -> io::Result<()> {
        let now = chrono_now_string();
        let line = format!("[{}] [{}] {}\n", now, event, message);
        if let Some(ref mut f) = self.file {
            f.write_all(line.as_bytes())?;
            f.flush()?;
        }
        info!("[{}] {}", event, message);
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.log_path
    }
}

fn chrono_now_string() -> String {
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs();
    let millis = duration.subsec_millis();

    // Standard ISO-like timestamp without external heavy crate
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_log_creation_and_write() {
        let tmp_dir = std::env::temp_dir();
        let flac_path = tmp_dir.join("test_session.flac");
        let mut logger = SessionLog::new(&flac_path).unwrap();
        logger.log_start(&flac_path, "DJM-V10", 96000, -19, true, 5, 2, 450.0).unwrap();
        logger.log_progress(50, 120, -7.9, 0, -19).unwrap();
        logger.log_event("BOOKMARK", "Track ID: Bicep - Glue").unwrap();
        logger.log_event("PAD_CHANGE", "Auto-stepped from -15dB to -19dB").unwrap();
        logger.log_stop("Manual split", 120, 50, -7.9, 0).unwrap();

        let log_content = std::fs::read_to_string(logger.path()).unwrap();
        assert!(log_content.contains("[SESSION_START]"));
        assert!(log_content.contains("[HARDWARE]"));
        assert!(log_content.contains("[STATUS]"));
        assert!(log_content.contains("[BOOKMARK] Track ID: Bicep - Glue"));
        assert!(log_content.contains("[PAD_CHANGE]"));
        assert!(log_content.contains("[SESSION_STOP]"));

        let _ = std::fs::remove_file(logger.path());
    }
}
