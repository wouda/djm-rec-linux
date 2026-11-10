use anyhow::Result;
use djm_common::prodj::ProDjLogEntry;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

pub struct SessionJsonlLogger {
    log_path: Option<PathBuf>,
    file: Option<File>,
}

impl Default for SessionJsonlLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionJsonlLogger {
    pub fn new() -> Self {
        Self {
            log_path: None,
            file: None,
        }
    }

    /// Opens or rotates to a new session log file `<session>_prodj.jsonl`
    pub fn open(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let p = path.as_ref().to_path_buf();
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&p)?;

        info!("Pro DJ Link session log opened at {}", p.display());
        self.log_path = Some(p);
        self.file = Some(f);
        Ok(())
    }

    /// Closes the current session log file
    pub fn close(&mut self) {
        if let Some(path) = &self.log_path {
            debug!("Closing Pro DJ Link session log at {}", path.display());
        }
        self.file = None;
        self.log_path = None;
    }

    /// Returns the active log file path, if any
    pub fn log_path(&self) -> Option<&Path> {
        self.log_path.as_deref()
    }

    /// Appends a `ProDjLogEntry` as a JSON line and flushes to disk
    pub fn log_entry(&mut self, entry: &ProDjLogEntry) -> Result<()> {
        if let Some(file) = &mut self.file {
            let json = serde_json::to_string(entry)?;
            writeln!(file, "{}", json)?;
            file.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use djm_common::prodj::{parse_prodj_log, ProDjDevice};

    #[test]
    fn test_session_jsonl_logger_open_write_rotate() {
        let temp_dir = std::env::temp_dir().join(format!("test_prodj_logger_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let log_file1 = temp_dir.join("set1_prodj.jsonl");
        let mut logger = SessionJsonlLogger::new();
        logger.open(&log_file1).unwrap();
        assert_eq!(logger.log_path(), Some(log_file1.as_path()));

        let entry1 = ProDjLogEntry::DeviceDiscovered {
            timestamp_rfc3339: "2026-10-30T22:00:00+01:00".to_string(),
            device: ProDjDevice {
                player_number: 1,
                device_name: "CDJ-3000".to_string(),
                ip_address: "192.168.1.10".parse().unwrap(),
                mac_address: [0, 1, 2, 3, 4, 5],
                model: "CDJ-3000".to_string(),
                firmware: "3.10".to_string(),
                is_mixer: false,
                last_seen_ms: 100,
            },
        };
        logger.log_entry(&entry1).unwrap();

        // Rotate to set2
        let log_file2 = temp_dir.join("set2_prodj.jsonl");
        logger.open(&log_file2).unwrap();
        assert_eq!(logger.log_path(), Some(log_file2.as_path()));

        let entry2 = ProDjLogEntry::StateChange {
            timestamp_rfc3339: "2026-10-30T23:00:00+01:00".to_string(),
            state_event: "settled".to_string(),
            details: "Virtual player claimed slot 6".to_string(),
        };
        logger.log_entry(&entry2).unwrap();
        logger.close();
        assert_eq!(logger.log_path(), None);

        // Verify set1 has entry1
        let parsed1 = parse_prodj_log(&log_file1).unwrap();
        assert_eq!(parsed1.len(), 1);
        assert_eq!(parsed1[0], entry1);

        // Verify set2 has entry2
        let parsed2 = parse_prodj_log(&log_file2).unwrap();
        assert_eq!(parsed2.len(), 1);
        assert_eq!(parsed2[0], entry2);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
