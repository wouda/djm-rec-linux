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
    #[serde(default)]
    pub audio: AudioConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            storage: StorageConfig::default(),
            splitting: SplittingConfig::default(),
            gain_and_hardware: GainAndHardwareConfig::default(),
            remote: RemoteConfig::default(),
            audio: AudioConfig::default(),
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
    #[serde(default = "default_hotspot_mode")]
    pub hotspot_mode: String,
    #[serde(default = "default_hotspot_ssid")]
    pub hotspot_ssid: String,
    #[serde(default = "default_hotspot_password")]
    pub hotspot_password: String,
    #[serde(default = "default_web_enabled")]
    pub web_enabled: bool,
    #[serde(default = "default_web_bind_addr")]
    pub web_bind_addr: String,
    #[serde(default = "default_auth_token")]
    pub auth_token: String,
    #[serde(default)]
    pub remote_public_url: String,
}

fn default_hotspot_mode() -> String {
    "auto".to_string()
}
fn default_hotspot_ssid() -> String {
    "DJM-Box-Booth".to_string()
}
fn default_hotspot_password() -> String {
    "djm-box-123".to_string()
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
pub fn generate_random_wpa2_password() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    const CHARSET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    (0..8)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}
fn default_auth_token() -> String {
    "123456".to_string()
}

impl Default for RemoteConfig {
    fn default() -> Self {
        Self {
            hotspot_mode: default_hotspot_mode(),
            hotspot_ssid: default_hotspot_ssid(),
            hotspot_password: default_hotspot_password(),
            web_enabled: default_web_enabled(),
            web_bind_addr: default_web_bind_addr(),
            auth_token: default_auth_token(),
            remote_public_url: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AudioConfig {
    #[serde(default)]
    pub djm_mic: DjmMicConfig,
    #[serde(default)]
    pub ambient_mic: AmbientMicConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DjmMicConfig {
    #[serde(default = "default_djm_mic_include_in_master")]
    pub include_in_master: bool,
    #[serde(default = "default_djm_mic_separate_recording")]
    pub separate_recording: bool,
}

fn default_djm_mic_include_in_master() -> bool {
    false
}
fn default_djm_mic_separate_recording() -> bool {
    true
}

impl Default for DjmMicConfig {
    fn default() -> Self {
        Self {
            include_in_master: default_djm_mic_include_in_master(),
            separate_recording: default_djm_mic_separate_recording(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AmbientMicConfig {
    #[serde(default = "default_ambient_enabled")]
    pub enabled: bool,
    #[serde(default = "default_ambient_device")]
    pub device: String,
    #[serde(default = "default_ambient_hangover")]
    pub post_set_hangover_mins: u32,
    #[serde(default = "default_ambient_sample_rate")]
    pub sample_rate: u32,
    #[serde(default = "default_ambient_channels")]
    pub channels: u32,
}

fn default_ambient_enabled() -> bool {
    true
}
fn default_ambient_device() -> String {
    "auto".to_string()
}
fn default_ambient_hangover() -> u32 {
    15
}
fn default_ambient_sample_rate() -> u32 {
    48000
}
fn default_ambient_channels() -> u32 {
    2
}

impl Default for AmbientMicConfig {
    fn default() -> Self {
        Self {
            enabled: default_ambient_enabled(),
            device: default_ambient_device(),
            post_set_hangover_mins: default_ambient_hangover(),
            sample_rate: default_ambient_sample_rate(),
            channels: default_ambient_channels(),
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
            config.remote.hotspot_password = generate_random_wpa2_password();
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
        assert_eq!(cfg.remote.hotspot_mode, "auto");
        assert_eq!(cfg.remote.hotspot_ssid, "DJM-Box-Booth");
        assert!(!cfg.remote.hotspot_password.is_empty());
    }

    #[test]
    fn test_wpa2_password_generation() {
        let p1 = generate_random_wpa2_password();
        let p2 = generate_random_wpa2_password();
        assert_eq!(p1.len(), 8);
        assert_eq!(p2.len(), 8);
        assert_ne!(p1, p2);
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
        assert!(!loaded.audio.djm_mic.include_in_master);
        assert!(loaded.audio.djm_mic.separate_recording);
        assert!(loaded.audio.ambient_mic.enabled);
        assert_eq!(loaded.audio.ambient_mic.device, "auto");
        assert_eq!(loaded.audio.ambient_mic.post_set_hangover_mins, 15);
    }

    #[test]
    fn test_audio_config_defaults_and_override() {
        let default_cfg = AppConfig::default();
        assert!(!default_cfg.audio.djm_mic.include_in_master);
        assert!(default_cfg.audio.djm_mic.separate_recording);
        assert!(default_cfg.audio.ambient_mic.enabled);
        assert_eq!(default_cfg.audio.ambient_mic.device, "auto");
        assert_eq!(default_cfg.audio.ambient_mic.post_set_hangover_mins, 15);
        assert_eq!(default_cfg.audio.ambient_mic.sample_rate, 48000);
        assert_eq!(default_cfg.audio.ambient_mic.channels, 2);

        let custom_raw = r#"
        [audio.djm_mic]
        include_in_master = true
        separate_recording = false

        [audio.ambient_mic]
        enabled = true
        device = "hw:0,0"
        post_set_hangover_mins = 30
        sample_rate = 96000
        channels = 1
        "#;
        let loaded = AppConfig::from_toml(custom_raw).expect("deserialization failed");
        assert!(loaded.audio.djm_mic.include_in_master);
        assert!(!loaded.audio.djm_mic.separate_recording);
        assert_eq!(loaded.audio.ambient_mic.device, "hw:0,0");
        assert_eq!(loaded.audio.ambient_mic.post_set_hangover_mins, 30);
        assert_eq!(loaded.audio.ambient_mic.sample_rate, 96000);
        assert_eq!(loaded.audio.ambient_mic.channels, 1);
    }
}
