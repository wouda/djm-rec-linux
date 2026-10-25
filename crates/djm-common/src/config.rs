use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("I/O error reading configuration: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse TOML configuration: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("Failed to serialize TOML configuration: {0}")]
    Serialize(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub splitting: SplittingConfig,
    #[serde(default)]
    pub gain_and_hardware: GainAndHardwareConfig,
    #[serde(default)]
    pub remote: RemoteConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            storage: StorageConfig::default(),
            splitting: SplittingConfig::default(),
            gain_and_hardware: GainAndHardwareConfig::default(),
            remote: RemoteConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
    #[serde(default = "default_filename_template")]
    pub filename_template: String,
    #[serde(default = "default_flac_compression")]
    pub flac_compression_level: u32,
    #[serde(default = "default_fsync_interval")]
    pub fsync_interval_secs: u64,
    #[serde(default = "default_min_free_disk")]
    pub min_free_disk_gb: f64,
}

fn default_output_dir() -> String {
    "~/Recordings".to_string()
}
fn default_filename_template() -> String {
    "{mixer}_{date}_{time}.flac".to_string()
}
fn default_flac_compression() -> u32 {
    5
}
fn default_fsync_interval() -> u64 {
    2
}
fn default_min_free_disk() -> f64 {
    2.0
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir(),
            filename_template: default_filename_template(),
            flac_compression_level: default_flac_compression(),
            fsync_interval_secs: default_fsync_interval(),
            min_free_disk_gb: default_min_free_disk(),
        }
    }
}

impl StorageConfig {
    pub fn resolved_output_dir(&self) -> PathBuf {
        let p = &self.output_dir;
        if p.starts_with("~/") || p == "~" {
            if let Some(home) = std::env::var_os("HOME") {
                let rest = if p == "~" { "" } else { &p[2..] };
                return PathBuf::from(home).join(rest);
            }
        }
        PathBuf::from(p)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SplittingConfig {
    #[serde(default = "default_pause_split_timeout")]
    pub pause_split_timeout_secs: f64,
    #[serde(default = "default_max_segment_duration")]
    pub max_segment_duration_mins: u32,
    #[serde(default = "default_preroll_buffer")]
    pub preroll_buffer_secs: f64,
    #[serde(default = "default_silence_threshold")]
    pub silence_threshold_dbfs: f32,
    #[serde(default = "default_wake_threshold")]
    pub wake_threshold_dbfs: f32,
}

fn default_pause_split_timeout() -> f64 {
    120.0
}
fn default_max_segment_duration() -> u32 {
    180
}
fn default_preroll_buffer() -> f64 {
    2.0
}
fn default_silence_threshold() -> f32 {
    -75.0
}
fn default_wake_threshold() -> f32 {
    -65.0
}

impl Default for SplittingConfig {
    fn default() -> Self {
        Self {
            pause_split_timeout_secs: default_pause_split_timeout(),
            max_segment_duration_mins: default_max_segment_duration(),
            preroll_buffer_secs: default_preroll_buffer(),
            silence_threshold_dbfs: default_silence_threshold(),
            wake_threshold_dbfs: default_wake_threshold(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GainAndHardwareConfig {
    #[serde(default = "default_capture_level")]
    pub default_capture_level: String,
    #[serde(default = "default_auto_headroom_guard")]
    pub auto_headroom_guard: bool,
    #[serde(default = "default_write_replaygain_tags")]
    pub write_replaygain_tags: bool,
}

fn default_capture_level() -> String {
    "-19dB".to_string()
}
fn default_auto_headroom_guard() -> bool {
    true
}
fn default_write_replaygain_tags() -> bool {
    true
}

impl Default for GainAndHardwareConfig {
    fn default() -> Self {
        Self {
            default_capture_level: default_capture_level(),
            auto_headroom_guard: default_auto_headroom_guard(),
            write_replaygain_tags: default_write_replaygain_tags(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteConfig {
    #[serde(default = "default_ble_enabled")]
    pub ble_enabled: bool,
    #[serde(default = "default_ble_device_name")]
    pub ble_device_name: String,
    #[serde(default = "default_web_enabled")]
    pub web_enabled: bool,
    #[serde(default = "default_web_bind_addr")]
    pub web_bind_addr: String,
    #[serde(default = "default_auth_token")]
    pub auth_token: String,
    #[serde(default)]
    pub remote_public_url: String,
}

fn default_ble_enabled() -> bool {
    true
}
fn default_ble_device_name() -> String {
    "DJM-Box-Recorder".to_string()
}
fn default_web_enabled() -> bool {
    true
}
fn default_web_bind_addr() -> String {
    "0.0.0.0:8080".to_string()
}
pub fn generate_random_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    // 6-digit numeric PIN for dark booth ease
    let num: u32 = rng.gen_range(100_000..=999_999);
    format!("{:06}", num)
}
fn default_auth_token() -> String {
    "123456".to_string()
}

impl Default for RemoteConfig {
    fn default() -> Self {
        Self {
            ble_enabled: default_ble_enabled(),
            ble_device_name: default_ble_device_name(),
            web_enabled: default_web_enabled(),
            web_bind_addr: default_web_bind_addr(),
            auth_token: default_auth_token(),
            remote_public_url: String::new(),
        }
    }
}

impl AppConfig {
    pub fn default_config_path() -> PathBuf {
        if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(".config/djm-box/config.toml")
        } else {
            PathBuf::from("/etc/djm-box/config.toml")
        }
    }

    pub fn load_or_create(path: &Path) -> Result<Self, ConfigError> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let config: AppConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            let mut config = AppConfig::default();
            config.remote.auth_token = generate_random_token();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let toml_str = toml::to_string_pretty(&config)?;
            std::fs::write(path, toml_str)?;
            Ok(config)
        }
    }

    pub fn from_toml(content: &str) -> Result<Self, ConfigError> {
        let config: AppConfig = toml::from_str(content)?;
        Ok(config)
    }

    pub fn to_toml(&self) -> Result<String, ConfigError> {
        Ok(toml::to_string_pretty(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.splitting.pause_split_timeout_secs, 120.0);
        assert_eq!(cfg.splitting.silence_threshold_dbfs, -75.0);
        assert_eq!(cfg.splitting.wake_threshold_dbfs, -65.0);
        assert_eq!(cfg.gain_and_hardware.default_capture_level, "-19dB");
        assert!(cfg.gain_and_hardware.auto_headroom_guard);
        assert_eq!(cfg.storage.fsync_interval_secs, 2);
    }

    #[test]
    fn test_toml_roundtrip() {
        let mut cfg = AppConfig::default();
        cfg.remote.auth_token = "849201".to_string();
        cfg.remote.remote_public_url = "djm-box.tailscale.net".to_string();

        let toml_str = cfg.to_toml().expect("failed to serialize");
        let loaded = AppConfig::from_toml(&toml_str).expect("failed to deserialize");
        assert_eq!(cfg, loaded);
    }

    #[test]
    fn test_partial_toml() {
        let raw = r#"
        [storage]
        output_dir = "/tmp/music"

        [splitting]
        pause_split_timeout_secs = 60.0
        "#;
        let loaded = AppConfig::from_toml(raw).expect("deserialization failed");
        assert_eq!(loaded.storage.output_dir, "/tmp/music");
        assert_eq!(loaded.splitting.pause_split_timeout_secs, 60.0);
        // Default fields preserved
        assert_eq!(loaded.splitting.silence_threshold_dbfs, -75.0);
        assert_eq!(loaded.gain_and_hardware.default_capture_level, "-19dB");
    }
}
