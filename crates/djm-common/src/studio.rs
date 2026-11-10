use chrono::{DateTime, Duration, FixedOffset, NaiveDate, NaiveDateTime, Offset};
use crate::protocol::RecordingState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

/// Complete multi-segment event session discovered on disk
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventSession {
    pub id: String,
    pub mixer_model: String,
    pub start_time_rfc3339: String,
    pub end_time_rfc3339: Option<String>,
    pub segments: Vec<SegmentInfo>,
    pub warmup_segments: Vec<SegmentInfo>,
    pub ambient_segments: Vec<SegmentInfo>,
    pub is_active_recording: bool,
    pub sample_rate: u32,
    pub total_samples: u64,
    pub total_duration_secs: f64,
    pub continuity_audit: ContinuityAuditReport,
}

/// Metadata and audit info for an individual recording segment
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SegmentInfo {
    pub index: usize,
    pub flac_path: PathBuf,
    pub log_path: Option<PathBuf>,
    pub start_time_rfc3339: String,
    pub end_time_rfc3339: Option<String>,
    pub sample_rate: u32,
    pub total_samples: u64,
    pub duration_secs: f64,
    pub total_clips: u64,
    pub is_warmup: bool,
    pub split_reason: Option<String>,
    pub preroll_samples: u64,
    pub ambient_path: Option<PathBuf>,
    pub ambient_samples: Option<u64>,
    pub ambient_aligned: bool,
    pub ambient_offset_samples: i64,
}

/// Result of the sample-count continuity check across segment seams
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ContinuityAuditReport {
    pub is_continuous: bool,
    pub checked_seams: usize,
    pub failed_seams: usize,
    pub seam_reports: Vec<SeamContinuityReport>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeamContinuityReport {
    pub from_segment_index: usize,
    pub to_segment_index: usize,
    pub split_reason: String,
    pub duration_from_samples_secs: f64,
    pub interval_between_starts_secs: f64,
    pub discrepancy_ms: f64,
    pub is_within_tolerance: bool,
}

/// Ambient alignment report for an individual ambient recording
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AmbientAlignmentReport {
    pub segment_index: usize,
    pub ambient_file: PathBuf,
    pub master_file: Option<PathBuf>,
    pub master_duration_secs: f64,
    pub ambient_duration_secs: f64,
    pub discrepancy_secs: f64,
    pub is_aligned: bool,
    pub note: Option<String>,
}

/// An entry in the event timetable
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtistSlot {
    pub slot_index: usize,
    pub artist: String,
    pub scheduled_start_clock: String, // "HH:MM" e.g. "23:00"
    pub scheduled_end_clock: Option<String>,
    pub target_date: String, // "YYYY-MM-DD"
    pub scheduled_start_rfc3339: String,
    pub scheduled_end_rfc3339: Option<String>,
    pub assigned_usb_cluster: Option<usize>,
    pub assigned_usb_media: Option<String>,
}

/// Method used to detect or resolve an artist set boundary
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoundaryDetectionMethod {
    SilenceMidpoint,
    UsbHandover,
    ScheduledClock,
    ManualAdjust,
}

/// USB handover event from Pro DJ Link telemetry or logs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsbHandover {
    pub timestamp: DateTime<FixedOffset>,
    pub media_id: String,
    pub media_name: String,
    pub player_number: u8,
    pub track_title: Option<String>,
}

/// A resolved boundary between two sets or start/end of the event
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetBoundary {
    pub slot_index: usize,
    pub artist: String,
    pub boundary_time_rfc3339: String,
    pub sample_offset: u64, // relative to event audio start
    pub duration_samples: u64,
    pub duration_secs: f64,
    pub detection_method: BoundaryDetectionMethod,
    pub silence_duration_secs: Option<f64>,
    pub silence_midpoint_rfc3339: Option<String>,
    pub usb_handover_media: Option<String>,
    pub needs_review: bool,
    pub review_reason: Option<String>,
}

/// Measured audio metrics for an artist set
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetQualityMetrics {
    pub artist: String,
    pub slot_index: usize,
    pub duration_secs: f64,
    pub sample_peak_dbfs: f32,
    pub true_peak_dbtp: f32,
    pub rms_dbfs: f32,
    pub integrated_lufs: f32,
    pub loudness_range_lu: f32,
    pub crest_factor_db: f32,
    pub flat_factor: f32,
    pub clip_count: u64,
    pub clip_percentage: f32,
    pub recommended_gain_db: f32,
    pub quality_label: String,
    pub clipping_unrepaired_warning: bool,
}

/// Remastering target preset
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "type", content = "config")]
pub enum Preset {
    #[default]
    Streaming, // -14.0 LUFS, -1.0 dBTP
    Club,      // -11.0 LUFS, -1.0 dBTP
    Custom {
        target_lufs: f32,
        ceiling_dbtp: f32,
    },
}

impl Preset {
    pub fn target_lufs(&self) -> f32 {
        match self {
            Preset::Streaming => -14.0,
            Preset::Club => -11.0,
            Preset::Custom { target_lufs, .. } => *target_lufs,
        }
    }

    pub fn ceiling_dbtp(&self) -> f32 {
        match self {
            Preset::Streaming => -1.0,
            Preset::Club => -1.0,
            Preset::Custom { ceiling_dbtp, .. } => *ceiling_dbtp,
        }
    }
}

/// Render configuration options chosen by user
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderOptions {
    pub preset: Preset,
    pub raw_artist_flacs: bool,
    pub normalized_artist_flacs: bool,
    pub full_event_raw_flac: bool,
    pub full_event_normalized_flac: bool,
    pub cue_sheet: bool,
    pub tracklist_txt: bool,
    pub export_mp3: bool,
    pub enable_declip: bool,
    pub custom_artist_gains: HashMap<String, f32>,
    pub include_ambient: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            preset: Preset::Streaming,
            raw_artist_flacs: true,
            normalized_artist_flacs: true,
            full_event_raw_flac: true,
            full_event_normalized_flac: true,
            cue_sheet: true,
            tracklist_txt: true,
            export_mp3: true,
            enable_declip: false,
            custom_artist_gains: HashMap::new(),
            include_ambient: false,
        }
    }
}

/// Job status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Idle,
    Running,
    Completed,
    Failed,
    Aborted,
    Interrupted,
}

/// Persistent job state for background audio analysis or rendering
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobState {
    pub id: String,
    pub session_id: String,
    pub status: JobStatus,
    pub phase: String,
    pub progress_percent: f32,
    pub current_step: usize,
    pub total_steps: usize,
    pub error_message: Option<String>,
    pub output_files: Vec<PathBuf>,
    pub started_at_rfc3339: String,
    pub updated_at_rfc3339: String,
}

/// Verification metrics measured on final rendered files
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasuredVerificationMetrics {
    pub file_name: String,
    pub file_path: PathBuf,
    pub is_artist_set: bool,
    pub artist_name: Option<String>,
    pub measured_integrated_lufs: f32,
    pub target_integrated_lufs: f32,
    pub lufs_difference_lu: f32,
    pub measured_true_peak_dbtp: f32,
    pub ceiling_true_peak_dbtp: f32,
    pub measured_sample_peak_dbfs: f32,
    pub target_missed: bool,
    pub miss_reason: Option<String>,
}

/// Summary entry for tracklists
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TracklistSummaryEntry {
    pub artist: String,
    pub usb_media_name: Option<String>,
    pub track_count: usize,
    pub tracks: Vec<String>,
}

/// Comprehensive multi-format event report
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventReport {
    pub event_id: String,
    pub generated_at_rfc3339: String,
    pub mixer_model: String,
    pub sample_rate: u32,
    pub total_samples: u64,
    pub duration_formatted: String,
    pub continuity_audit: ContinuityAuditReport,
    pub ambient_alignment: Vec<AmbientAlignmentReport>,
    pub boundaries: Vec<SetBoundary>,
    pub pre_render_metrics: Vec<SetQualityMetrics>,
    pub post_render_verification: Vec<MeasuredVerificationMetrics>,
    pub tracklist_summary: Vec<TracklistSummaryEntry>,
    pub general_notes: Vec<String>,
}

/// Parsed session log representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedSessionLog {
    pub log_path: PathBuf,
    pub flac_path: Option<PathBuf>,
    pub mixer: Option<String>,
    pub sample_rate: Option<u32>,
    pub pad_db: Option<i8>,
    pub auto_guard: Option<bool>,
    pub start_time: DateTime<FixedOffset>,
    pub end_time: Option<DateTime<FixedOffset>>,
    pub stop_reason: Option<String>,
    pub total_duration_secs: Option<f64>,
    pub total_mb: Option<u64>,
    pub max_peak_dbfs: Option<f32>,
    pub total_clips: u64,
    pub preroll_samples: u64,
    pub ambient_stream_path: Option<PathBuf>,
    pub ambient_sample_rate: Option<u32>,
    pub ambient_sample_offset: Option<u64>,
    pub has_tz_offset: bool,
    pub silence_events: Vec<SilenceEvent>,
    pub bookmarks: Vec<BookmarkEvent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SilenceEvent {
    pub start_time: DateTime<FixedOffset>,
    pub end_time: Option<DateTime<FixedOffset>>,
    pub duration_secs: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookmarkEvent {
    pub timestamp: DateTime<FixedOffset>,
    pub note: String,
}

pub fn parse_log_timestamp(raw: &str) -> Result<(DateTime<FixedOffset>, bool), String> {
    let trimmed = raw.trim();
    // 1. Standard RFC 3339 e.g. 2026-10-30T22:00:06.567+01:00 or Z
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok((dt, true));
    }

    // 2. RFC 3339 with space instead of T e.g. "2026-10-30 22:00:06.567+01:00"
    if trimmed.len() >= 19 && trimmed.as_bytes()[10] == b' ' {
        let with_t = format!("{}T{}", &trimmed[..10], &trimmed[11..]);
        if let Ok(dt) = DateTime::parse_from_rfc3339(&with_t) {
            return Ok((dt, true));
        }
    }

    // 3. Naive format without offset e.g. "2026-10-30 22:00:06.567" or "2026-10-30T22:00:06.567"
    let normalized = trimmed.replace('T', " ");
    for fmt in ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(&normalized, fmt) {
            // Convert to local timezone or +01:00 fallback
            let tz_offset = chrono::Local::now().offset().fix();
            let utc = naive - Duration::seconds(tz_offset.local_minus_utc() as i64);
            let dt = DateTime::<FixedOffset>::from_naive_utc_and_offset(utc, tz_offset);
            return Ok((dt, false));
        }
    }

    Err(format!("Unable to parse log timestamp: '{}'", trimmed))
}

pub fn parse_session_log(path: &Path) -> io::Result<ParsedSessionLog> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut flac_path: Option<PathBuf> = None;
    let mut mixer: Option<String> = None;
    let mut sample_rate: Option<u32> = None;
    let mut pad_db: Option<i8> = None;
    let mut auto_guard: Option<bool> = None;
    let mut start_time: Option<DateTime<FixedOffset>> = None;
    let mut end_time: Option<DateTime<FixedOffset>> = None;
    let mut stop_reason: Option<String> = None;
    let mut total_duration_secs: Option<f64> = None;
    let mut total_mb: Option<u64> = None;
    let mut max_peak_dbfs: Option<f32> = None;
    let mut total_clips: u64 = 0;
    let mut preroll_samples: u64 = 0;
    let mut ambient_stream_path: Option<PathBuf> = None;
    let mut ambient_sample_rate: Option<u32> = None;
    let mut ambient_sample_offset: Option<u64> = None;
    let mut has_tz_offset = false;
    let mut silence_events = Vec::new();
    let mut bookmarks = Vec::new();
    let mut active_silence_start: Option<DateTime<FixedOffset>> = None;

    for line_res in reader.lines() {
        let line = match line_res {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if !trimmed.starts_with('[') {
            continue;
        }

        // Parse: [TIMESTAMP] [TAG] MESSAGE
        let close_ts = match trimmed.find(']') {
            Some(idx) => idx,
            None => continue,
        };
        let ts_str = &trimmed[1..close_ts];
        let (dt, has_offset) = match parse_log_timestamp(ts_str) {
            Ok(res) => res,
            Err(_) => continue,
        };
        if has_offset {
            has_tz_offset = true;
        }

        let rem = trimmed[close_ts + 1..].trim();
        if !rem.starts_with('[') {
            continue;
        }
        let close_tag = match rem.find(']') {
            Some(idx) => idx,
            None => continue,
        };
        let tag = &rem[1..close_tag];
        let msg = rem[close_tag + 1..].trim();

        match tag {
            "SESSION_INIT" => {
                if start_time.is_none() {
                    start_time = Some(dt);
                }
                if let Some(target) = msg.strip_prefix("Session log opened for ") {
                    flac_path = Some(PathBuf::from(target.trim()));
                }
            }
            "SESSION_START" => {
                start_time = Some(dt);
                if let Some(target) = msg.strip_prefix("Opened session recording for: ") {
                    flac_path = Some(PathBuf::from(target.trim()));
                }
            }
            "HARDWARE" => {
                // "Mixer: AlphaTheta DJM-A9 (Native sample rate: 96000 Hz, 24-bit stereo)"
                if let Some(rest) = msg.strip_prefix("Mixer: ") {
                    if let Some(paren_idx) = rest.find('(') {
                        mixer = Some(rest[..paren_idx].trim().to_string());
                        let inside = &rest[paren_idx + 1..];
                        if let Some(rate_str) = inside.strip_prefix("Native sample rate: ") {
                            if let Some(hz_idx) = rate_str.find(" Hz") {
                                sample_rate = rate_str[..hz_idx].trim().parse::<u32>().ok();
                            }
                        }
                    }
                }
            }
            "GAIN_STAGING" => {
                // "Hardware Capture Pad: -19 dB | Auto-Headroom Guard: Enabled"
                if let Some(pad_str) = msg.strip_prefix("Hardware Capture Pad: ") {
                    if let Some(db_idx) = pad_str.find(" dB") {
                        pad_db = pad_str[..db_idx].trim().parse::<i8>().ok();
                    }
                }
                if let Some(guard_idx) = msg.find("Auto-Headroom Guard: ") {
                    let guard_val = msg[guard_idx + 21..].trim();
                    auto_guard = Some(guard_val.starts_with("Enabled") || guard_val.starts_with("true"));
                }
            }
            "GATE_WAKE" => {
                // "Silence gate woke! Audio detected above wake threshold (-65.0 dBFS). Target file: ... Pre-roll samples: 192000."
                if let Some(pre_idx) = msg.find("Pre-roll samples: ") {
                    let rest = &msg[pre_idx + 18..];
                    let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if let Ok(samples) = num_str.parse::<u64>() {
                        preroll_samples = samples;
                    }
                }
            }
            "STREAM_OPENED" => {
                // "Stream [AMBIENT]: /path/to/ambient.flac (48000 Hz, sample offset: 0)"
                if msg.contains("[AMBIENT]") {
                    if let Some(colon_idx) = msg.find("]: ") {
                        let rest = &msg[colon_idx + 3..];
                        if let Some(paren_idx) = rest.find('(') {
                            ambient_stream_path = Some(PathBuf::from(rest[..paren_idx].trim()));
                            let inside = &rest[paren_idx + 1..];
                            if let Some(hz_idx) = inside.find(" Hz") {
                                ambient_sample_rate = inside[..hz_idx].trim().parse::<u32>().ok();
                            }
                            if let Some(off_idx) = inside.find("sample offset: ") {
                                let off_str = &inside[off_idx + 15..];
                                let num_str: String = off_str.chars().take_while(|c| c.is_ascii_digit()).collect();
                                ambient_sample_offset = num_str.parse::<u64>().ok();
                            }
                        }
                    }
                }
            }
            "STATUS" => {
                // "Progress update: 1072 MB written | Elapsed: 00:32:30 | ... | Clips: 0 | ..."
                if let Some(mb_idx) = msg.find("Progress update: ") {
                    let rest = &msg[mb_idx + 17..];
                    let mb_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if let Ok(mb) = mb_str.parse::<u64>() {
                        total_mb = Some(mb);
                    }
                }
                if let Some(clip_idx) = msg.find("Clips: ") {
                    let rest = &msg[clip_idx + 7..];
                    let clip_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if let Ok(c) = clip_str.parse::<u64>() {
                        if c > total_clips {
                            total_clips = c;
                        }
                    }
                }
            }
            "DIGITAL_CLIP" => {
                // "⚠️ Digital clip detected: +2 clipped sample(s) | ... | Total session clips: 37486"
                if let Some(idx) = msg.find("Total session clips: ") {
                    let rest = &msg[idx + 21..];
                    let clip_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if let Ok(c) = clip_str.parse::<u64>() {
                        if c > total_clips {
                            total_clips = c;
                        }
                    }
                }
            }
            "GATE_SILENCE" => {
                active_silence_start = Some(dt);
            }
            "GATE_RESUME" => {
                if let Some(silence_start) = active_silence_start.take() {
                    let dur = (dt - silence_start).num_milliseconds() as f64 / 1000.0;
                    silence_events.push(SilenceEvent {
                        start_time: silence_start,
                        end_time: Some(dt),
                        duration_secs: Some(dur),
                    });
                }
            }
            "BOOKMARK" => {
                bookmarks.push(BookmarkEvent {
                    timestamp: dt,
                    note: msg.to_string(),
                });
            }
            "SESSION_STOP" => {
                end_time = Some(dt);
                // "Session closed (Silence Timeout Gate) | Total duration: 00:40:07 | Total size: 1323 MB | Max peak: -0.5 dBFS | Total clips: 0"
                if let Some(open_paren) = msg.find('(') {
                    if let Some(close_paren) = msg.find(')') {
                        if close_paren > open_paren {
                            stop_reason = Some(msg[open_paren + 1..close_paren].trim().to_string());
                        }
                    }
                }
                if let Some(dur_idx) = msg.find("Total duration: ") {
                    let rest = &msg[dur_idx + 16..];
                    let time_str: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == ':').collect();
                    let parts: Vec<&str> = time_str.split(':').collect();
                    if parts.len() == 3 {
                        let h: f64 = parts[0].parse().unwrap_or(0.0);
                        let m: f64 = parts[1].parse().unwrap_or(0.0);
                        let s: f64 = parts[2].parse().unwrap_or(0.0);
                        total_duration_secs = Some(h * 3600.0 + m * 60.0 + s);
                    }
                }
                if let Some(size_idx) = msg.find("Total size: ") {
                    let rest = &msg[size_idx + 12..];
                    let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if let Ok(mb) = num_str.parse::<u64>() {
                        total_mb = Some(mb);
                    }
                }
                if let Some(peak_idx) = msg.find("Max peak: ") {
                    let rest = &msg[peak_idx + 10..];
                    if let Some(db_idx) = rest.find(" dBFS") {
                        if let Ok(peak) = rest[..db_idx].trim().parse::<f32>() {
                            max_peak_dbfs = Some(peak);
                        }
                    }
                }
                if let Some(clip_idx) = msg.find("Total clips: ") {
                    let rest = &msg[clip_idx + 13..];
                    let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if let Ok(c) = num_str.parse::<u64>() {
                        if c > total_clips {
                            total_clips = c;
                        }
                    }
                }

                // If silence was active when session stopped, complete the silence interval
                if let Some(silence_start) = active_silence_start.take() {
                    let dur = (dt - silence_start).num_milliseconds() as f64 / 1000.0;
                    silence_events.push(SilenceEvent {
                        start_time: silence_start,
                        end_time: Some(dt),
                        duration_secs: Some(dur),
                    });
                }
            }
            _ => {}
        }
    }

    let default_start = parse_log_timestamp("1970-01-01 00:00:00").unwrap().0;
    let s_time = start_time.unwrap_or(default_start);

    Ok(ParsedSessionLog {
        log_path: path.to_path_buf(),
        flac_path,
        mixer,
        sample_rate,
        pad_db,
        auto_guard,
        start_time: s_time,
        end_time,
        stop_reason,
        total_duration_secs,
        total_mb,
        max_peak_dbfs,
        total_clips,
        preroll_samples,
        ambient_stream_path,
        ambient_sample_rate,
        ambient_sample_offset,
        has_tz_offset,
        silence_events,
        bookmarks,
    })
}

/// Extracts the segment index number from a filename containing "_ptXX"
pub fn extract_segment_index_from_filename(filename: &str) -> Option<usize> {
    if let Some(pt_idx) = filename.find("_pt") {
        let rest = &filename[pt_idx + 3..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse::<usize>().ok()
    } else {
        None
    }
}

/// Reads the STREAMINFO metadata block from a FLAC file header without decoding audio frames
pub fn read_flac_streaminfo(path: &Path) -> io::Result<(u32, u8, u8, u64)> {
    use std::io::Read;
    let mut file = File::open(path)?;
    let mut header = [0u8; 42];
    file.read_exact(&mut header)?;

    if &header[0..4] != b"fLaC" {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Not a FLAC file"));
    }

    let b18 = header[18] as u32;
    let b19 = header[19] as u32;
    let b20 = header[20] as u32;
    let sample_rate = (b18 << 12) | (b19 << 4) | (b20 >> 4);

    let channels = (((b20 >> 1) & 0x07) + 1) as u8;
    let b21 = header[21] as u32;
    let bits_per_sample = ((((b20 & 0x01) << 4) | (b21 >> 4)) + 1) as u8;

    let b22 = header[22] as u64;
    let b23 = header[23] as u64;
    let b24 = header[24] as u64;
    let b25 = header[25] as u64;
    let total_samples = (((b21 as u64) & 0x0F) << 32)
        | (b22 << 24)
        | (b23 << 16)
        | (b24 << 8)
        | b25;

    Ok((sample_rate, channels, bits_per_sample, total_samples))
}

/// Builds SegmentInfo from a session log and its corresponding audio file
pub fn build_segment_info(log_path: &Path) -> io::Result<SegmentInfo> {
    let parsed = parse_session_log(log_path)?;
    let filename = log_path.file_name().unwrap_or_default().to_string_lossy();
    let index = extract_segment_index_from_filename(&filename).unwrap_or(1);

    let flac_path = if let Some(ref p) = parsed.flac_path {
        if p.exists() {
            p.clone()
        } else {
            log_path.with_extension("flac")
        }
    } else {
        log_path.with_extension("flac")
    };

    let mut sample_rate = parsed.sample_rate.unwrap_or(96000);
    let mut total_samples = (parsed.total_duration_secs.unwrap_or(0.0) * sample_rate as f64) as u64;

    if flac_path.exists() {
        if let Ok((sr, _, _, samples)) = read_flac_streaminfo(&flac_path) {
            sample_rate = sr;
            total_samples = samples;
        }
    }

    let duration_secs = if total_samples > 0 && sample_rate > 0 {
        total_samples as f64 / sample_rate as f64
    } else {
        parsed.total_duration_secs.unwrap_or(0.0)
    };

    let start_time_rfc3339 = parsed.start_time.to_rfc3339();
    let end_time_rfc3339 = parsed.end_time.map(|t| t.to_rfc3339());

    // Check ambient file
    let ambient_candidate = flac_path
        .parent()
        .map(|p| {
            let stem = flac_path.file_stem().unwrap_or_default().to_string_lossy();
            p.join(format!("{}_ambient.flac", stem))
        })
        .or_else(|| parsed.ambient_stream_path.clone());

    let (ambient_path, ambient_samples, ambient_aligned, ambient_offset_samples) = if let Some(ref amb) = ambient_candidate {
        if amb.exists() {
            let amb_s = read_flac_streaminfo(amb).map(|(_, _, _, s)| s).ok();
            let (aligned, offset) = if let Some(s) = amb_s {
                let amb_dur = s as f64 / 48000.0;
                let disc = amb_dur - duration_secs;
                (disc.abs() <= 2.0, (disc * 48000.0) as i64)
            } else {
                (false, 0)
            };
            (Some(amb.clone()), amb_s, aligned, offset)
        } else {
            (None, None, false, 0)
        }
    } else {
        (None, None, false, 0)
    };

    Ok(SegmentInfo {
        index,
        flac_path,
        log_path: Some(log_path.to_path_buf()),
        start_time_rfc3339,
        end_time_rfc3339,
        sample_rate,
        total_samples,
        duration_secs,
        total_clips: parsed.total_clips,
        is_warmup: false,
        split_reason: parsed.stop_reason,
        preroll_samples: parsed.preroll_samples,
        ambient_path,
        ambient_samples,
        ambient_aligned,
        ambient_offset_samples,
    })
}

/// Checks sample-count continuity between adjacent rollover segments
pub fn check_continuity(
    segments: &[SegmentInfo],
    tolerance_ms: f64,
) -> ContinuityAuditReport {
    let mut seam_reports = Vec::new();
    let mut is_continuous = true;
    let mut failed_seams = 0;

    for i in 0..segments.len().saturating_sub(1) {
        let seg_curr = &segments[i];
        let seg_next = &segments[i + 1];

        let t_curr = match parse_log_timestamp(&seg_curr.start_time_rfc3339) {
            Ok((dt, _)) => dt,
            Err(_) => continue,
        };
        let t_next = match parse_log_timestamp(&seg_next.start_time_rfc3339) {
            Ok((dt, _)) => dt,
            Err(_) => continue,
        };

        let interval_between_starts_secs = (t_next - t_curr).num_milliseconds() as f64 / 1000.0;
        let rate = seg_curr.sample_rate.max(1) as f64;
        let effective_samples = if i == 0 && seg_curr.preroll_samples > 0 {
            seg_curr.total_samples.saturating_sub(seg_curr.preroll_samples) as f64 / rate
        } else {
            seg_curr.total_samples as f64 / rate
        };

        let discrepancy_ms = ((effective_samples - interval_between_starts_secs).abs()) * 1000.0;
        let is_within_tolerance = discrepancy_ms <= tolerance_ms;
        if !is_within_tolerance {
            is_continuous = false;
            failed_seams += 1;
        }

        seam_reports.push(SeamContinuityReport {
            from_segment_index: seg_curr.index,
            to_segment_index: seg_next.index,
            split_reason: seg_curr.split_reason.clone().unwrap_or_else(|| "Unknown".to_string()),
            duration_from_samples_secs: effective_samples,
            interval_between_starts_secs,
            discrepancy_ms,
            is_within_tolerance,
        });
    }

    ContinuityAuditReport {
        is_continuous: is_continuous && failed_seams == 0,
        checked_seams: seam_reports.len(),
        failed_seams,
        seam_reports,
    }
}

/// Audits ambient recording alignment against paired master audio segments
pub fn audit_ambient_alignment(segments: &[SegmentInfo]) -> Vec<AmbientAlignmentReport> {
    let mut reports = Vec::new();

    for seg in segments {
        if let Some(ref amb_path) = seg.ambient_path {
            let master_dur = seg.duration_secs;
            let amb_samples = seg.ambient_samples.unwrap_or(0);
            let amb_rate = 48000.0f64;
            let amb_dur = if amb_samples > 0 {
                amb_samples as f64 / amb_rate
            } else {
                0.0
            };

            let discrepancy_secs = amb_dur - master_dur;
            // 2.0s tolerance for pre-roll / hangover
            let is_aligned = discrepancy_secs.abs() <= 2.0;

            let note = if is_aligned {
                None
            } else if discrepancy_secs > 2.0 {
                Some(format!(
                    "Ambient file duration ({:.1}s) exceeds master segment ({:.1}s) by +{:.1}s (spans multiple segments)",
                    amb_dur, master_dur, discrepancy_secs
                ))
            } else {
                Some(format!(
                    "Ambient file duration ({:.1}s) is shorter than master segment ({:.1}s) by {:.1}s",
                    amb_dur, master_dur, discrepancy_secs.abs()
                ))
            };

            reports.push(AmbientAlignmentReport {
                segment_index: seg.index,
                ambient_file: amb_path.clone(),
                master_file: Some(seg.flac_path.clone()),
                master_duration_secs: master_dur,
                ambient_duration_secs: amb_dur,
                discrepancy_secs,
                is_aligned,
                note,
            });
        }
    }

    reports
}

/// Discovers and groups event sessions in the recordings directory
pub fn group_event_sessions(recordings_dir: &Path) -> io::Result<Vec<EventSession>> {
    let mut log_files = Vec::new();
    if recordings_dir.exists() {
        for entry in std::fs::read_dir(recordings_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let fname = path.file_name().unwrap_or_default().to_string_lossy();
                if fname.ends_with(".log") && !fname.ends_with("_ambient.log") && fname.contains("_pt") {
                    log_files.push(path);
                }
            }
        }
    }

    log_files.sort();

    let mut all_segments = Vec::new();
    for log_path in &log_files {
        if let Ok(seg) = build_segment_info(log_path) {
            all_segments.push(seg);
        }
    }

    all_segments.sort_by(|a, b| a.start_time_rfc3339.cmp(&b.start_time_rfc3339));

    let mut sessions = Vec::new();
    if all_segments.is_empty() {
        return Ok(sessions);
    }

    let mut current_block: Vec<SegmentInfo> = Vec::new();
    let mut pre_event_candidates: Vec<SegmentInfo> = Vec::new();

    for seg in all_segments {
        if current_block.is_empty() {
            current_block.push(seg);
        } else {
            let prev = current_block.last().unwrap();
            let is_rollover = prev
                .split_reason
                .as_deref()
                .map(|r| r.contains("Rollover") || r.contains("Manual Split"))
                .unwrap_or(false);

            if is_rollover {
                current_block.push(seg);
            } else {
                // If previous block was a single segment stopped by Silence Timeout, treat as warmup candidate
                if current_block.len() == 1 {
                    pre_event_candidates.push(current_block.pop().unwrap());
                } else {
                    sessions.push(create_event_session_from_segments(
                        std::mem::take(&mut current_block),
                        std::mem::take(&mut pre_event_candidates),
                    ));
                }
                current_block.push(seg);
            }
        }
    }

    if !current_block.is_empty() {
        sessions.push(create_event_session_from_segments(
            current_block,
            pre_event_candidates,
        ));
    }

    Ok(sessions)
}

fn create_event_session_from_segments(
    mut segments: Vec<SegmentInfo>,
    mut warmups: Vec<SegmentInfo>,
) -> EventSession {
    for s in &mut warmups {
        s.is_warmup = true;
    }
    for s in &mut segments {
        s.is_warmup = false;
    }

    let first = segments.first().or_else(|| warmups.first()).unwrap();
    let last = segments.last().or_else(|| warmups.last()).unwrap();

    let mixer_model = first
        .flac_path
        .file_name()
        .and_then(|f| {
            let s = f.to_string_lossy();
            s.split('_').next().map(|m| m.replace('_', " "))
        })
        .unwrap_or_else(|| "Pioneer DJ Mixer".to_string());

    let start_time_rfc3339 = first.start_time_rfc3339.clone();
    let end_time_rfc3339 = last.end_time_rfc3339.clone();

    let id = first
        .flac_path
        .file_stem()
        .map(|s| {
            let str_val = s.to_string_lossy();
            if let Some(idx) = str_val.find("_pt") {
                str_val[..idx].to_string()
            } else {
                str_val.to_string()
            }
        })
        .unwrap_or_else(|| "event_session".to_string());

    let sample_rate = first.sample_rate;
    let total_samples: u64 = segments.iter().map(|s| s.total_samples).sum();
    let total_duration_secs = if sample_rate > 0 {
        total_samples as f64 / sample_rate as f64
    } else {
        segments.iter().map(|s| s.duration_secs).sum()
    };

    let continuity_audit = check_continuity(&segments, 50.0);

    EventSession {
        id,
        mixer_model,
        start_time_rfc3339,
        end_time_rfc3339,
        segments,
        warmup_segments: warmups,
        ambient_segments: Vec::new(),
        is_active_recording: false,
        sample_rate,
        total_samples,
        total_duration_secs,
        continuity_audit,
    }
}

/// Finds the first time token (HH:MM or H:MM, optional :SS) in string `s`.
/// Returns `(start_index, end_index, hour, minute)`.
fn find_time_token(s: &str) -> Option<(usize, usize, u32, u32)> {
    let bytes = s.as_bytes();
    let len = bytes.len();
    for i in 0..len {
        if bytes[i] == b':' {
            // Preceding 1 or 2 digits
            let mut start = i;
            while start > 0 && bytes[start - 1].is_ascii_digit() && (i - (start - 1)) <= 2 {
                start -= 1;
            }
            if start == i {
                continue;
            }
            // Ensure word boundary before start
            if start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b':') {
                continue;
            }

            // Following 2 digits
            let mut end = i + 1;
            while end < len && bytes[end].is_ascii_digit() && (end - (i + 1)) < 2 {
                end += 1;
            }
            if end != i + 3 {
                continue;
            }

            let hour_str = &s[start..i];
            let min_str = &s[i + 1..end];
            if let (Ok(h), Ok(m)) = (hour_str.parse::<u32>(), min_str.parse::<u32>()) {
                if h < 24 && m < 60 {
                    // Check for optional ":SS"
                    let mut token_end = end;
                    if token_end + 3 <= len
                        && bytes[token_end] == b':'
                        && bytes[token_end + 1].is_ascii_digit()
                        && bytes[token_end + 2].is_ascii_digit()
                    {
                        token_end += 3;
                    }
                    // Ensure word boundary after token_end
                    if token_end < len && (bytes[token_end].is_ascii_alphanumeric() || bytes[token_end] == b':') {
                        continue;
                    }
                    return Some((start, token_end, h, m));
                }
            }
        }
    }
    None
}

/// Parses a line from a timetable into (start_h, start_m, optional (end_h, end_m), artist_name)
#[allow(clippy::type_complexity)]
fn parse_timetable_line_components(line: &str) -> Option<(u32, u32, Option<(u32, u32)>, String)> {
    let (start1, end1, h1, m1) = find_time_token(line)?;

    // Check if there is a second time token following a separator like "-", "–", "to"
    let rest = &line[end1..];
    let (end_time_opt, artist_str) = if let Some((start2_rel, end2_rel, h2, m2)) = find_time_token(rest) {
        let separator = rest[..start2_rel].trim();
        if separator.is_empty()
            || separator == "-"
            || separator == "–"
            || separator == "—"
            || separator == "->"
            || separator.eq_ignore_ascii_case("to")
            || separator.eq_ignore_ascii_case("until")
        {
            let end2 = end1 + end2_rel;
            let artist = format!("{} {}", &line[..start1], &line[end2..]);
            (Some((h2, m2)), artist)
        } else {
            let artist = format!("{} {}", &line[..start1], rest);
            (None, artist)
        }
    } else {
        let artist = format!("{} {}", &line[..start1], rest);
        (None, artist)
    };

    // Clean artist name
    let mut cleaned_artist = artist_str.trim().to_string();
    cleaned_artist = cleaned_artist
        .trim_matches(|c: char| c == '-' || c == '–' || c == '—' || c == ':' || c == '|' || c == '•' || c == '\t' || c == ' ')
        .trim()
        .to_string();

    if cleaned_artist.starts_with('(') && cleaned_artist.ends_with(')') {
        cleaned_artist = cleaned_artist[1..cleaned_artist.len() - 1].trim().to_string();
    }

    if cleaned_artist.is_empty() {
        return None;
    }

    Some((h1, m1, end_time_opt, cleaned_artist))
}

/// Parses timetable text anchored to an event date and initial timezone offset.
/// Handles overnight wrapping (whenever a slot time precedes the previous slot time,
/// the calendar day increments).
pub fn parse_timetable(
    timetable_text: &str,
    event_date: NaiveDate,
    tz_offset: FixedOffset,
) -> Result<Vec<ArtistSlot>, String> {
    let mut slots = Vec::new();
    let mut current_date = event_date;
    let mut prev_time: Option<(u32, u32)> = None;

    for raw_line in timetable_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }

        // Clean list bullets or numbering prefixes (e.g. "1. ", "01. ", "- ")
        let candidate = if let Some(stripped) = line.strip_prefix(|c: char| c == '-' || c == '*' || c == '•') {
            stripped.trim()
        } else if let Some(dot_idx) = line.find(". ") {
            let prefix = &line[..dot_idx];
            if prefix.chars().all(|c| c.is_ascii_digit()) {
                line[dot_idx + 2..].trim()
            } else {
                line
            }
        } else {
            line
        };

        if let Some((start_h, start_m, end_opt, artist_str)) = parse_timetable_line_components(candidate) {
            // Check overnight wrap
            if let Some((prev_h, prev_m)) = prev_time {
                if (start_h, start_m) < (prev_h, prev_m) {
                    current_date += Duration::days(1);
                }
            }
            prev_time = Some((start_h, start_m));

            let start_naive = current_date
                .and_hms_opt(start_h, start_m, 0)
                .ok_or_else(|| format!("Invalid time {}:{} on date {}", start_h, start_m, current_date))?;
            let start_dt = DateTime::<FixedOffset>::from_naive_utc_and_offset(
                start_naive - tz_offset,
                tz_offset,
            );

            let (end_clock, end_rfc3339) = if let Some((end_h, end_m)) = end_opt {
                let mut end_date = current_date;
                if (end_h, end_m) < (start_h, start_m) {
                    end_date += Duration::days(1);
                }
                let end_naive = end_date
                    .and_hms_opt(end_h, end_m, 0)
                    .ok_or_else(|| format!("Invalid time {}:{} on date {}", end_h, end_m, end_date))?;
                let end_dt = DateTime::<FixedOffset>::from_naive_utc_and_offset(
                    end_naive - tz_offset,
                    tz_offset,
                );
                (
                    Some(format!("{:02}:{:02}", end_h, end_m)),
                    Some(end_dt.to_rfc3339()),
                )
            } else {
                (None, None)
            };

            let slot_index = slots.len();
            slots.push(ArtistSlot {
                slot_index,
                artist: artist_str,
                scheduled_start_clock: format!("{:02}:{:02}", start_h, start_m),
                scheduled_end_clock: end_clock,
                target_date: current_date.format("%Y-%m-%d").to_string(),
                scheduled_start_rfc3339: start_dt.to_rfc3339(),
                scheduled_end_rfc3339: end_rfc3339,
                assigned_usb_cluster: None,
                assigned_usb_media: None,
            });
        }
    }

    // Populate scheduled_end_* from adjacent slots if not specified
    let len = slots.len();
    for i in 0..len {
        if slots[i].scheduled_end_clock.is_none() {
            if i + 1 < len {
                slots[i].scheduled_end_clock = Some(slots[i + 1].scheduled_start_clock.clone());
                slots[i].scheduled_end_rfc3339 = Some(slots[i + 1].scheduled_start_rfc3339.clone());
            } else {
                // Last slot defaults to 1 hour after start
                if let Ok((start_dt, _)) = parse_log_timestamp(&slots[i].scheduled_start_rfc3339) {
                    let end_dt = start_dt + Duration::hours(1);
                    slots[i].scheduled_end_clock = Some(end_dt.format("%H:%M").to_string());
                    slots[i].scheduled_end_rfc3339 = Some(end_dt.to_rfc3339());
                }
            }
        }
    }

    if slots.is_empty() {
        return Err("No valid artist slots found in timetable text".to_string());
    }

    Ok(slots)
}

/// Resolves set boundaries from artist slots, silence events, USB handovers, and bookmarks.
/// Priority order: Silence midpoint (>= 2.0s within 15 min) -> USB handover -> Scheduled clock (+ review flag).
#[allow(clippy::needless_range_loop)]
pub fn resolve_set_boundaries(
    slots: &[ArtistSlot],
    silence_events: &[SilenceEvent],
    usb_handovers: &[UsbHandover],
    bookmarks: &[BookmarkEvent],
    event_start_time: DateTime<FixedOffset>,
    event_end_time: Option<DateTime<FixedOffset>>,
    sample_rate: u32,
) -> Vec<SetBoundary> {
    if slots.is_empty() {
        return Vec::new();
    }

    // Collect all distinct offsets present across events (handles DST transitions)
    let mut distinct_offsets: Vec<FixedOffset> = Vec::new();
    distinct_offsets.push(*event_start_time.offset());
    for s in silence_events {
        let off = *s.start_time.offset();
        if !distinct_offsets.contains(&off) {
            distinct_offsets.push(off);
        }
    }
    for u in usb_handovers {
        let off = *u.timestamp.offset();
        if !distinct_offsets.contains(&off) {
            distinct_offsets.push(off);
        }
    }
    for b in bookmarks {
        let off = *b.timestamp.offset();
        if !distinct_offsets.contains(&off) {
            distinct_offsets.push(off);
        }
    }

    let n = slots.len();
    let mut boundary_times: Vec<DateTime<FixedOffset>> = Vec::with_capacity(n);
    let mut boundary_methods: Vec<BoundaryDetectionMethod> = Vec::with_capacity(n);
    let mut boundary_silence_durs: Vec<Option<f64>> = Vec::with_capacity(n);
    let mut boundary_silence_mids: Vec<Option<String>> = Vec::with_capacity(n);
    let mut boundary_usb_medias: Vec<Option<String>> = Vec::with_capacity(n);
    let mut boundary_reviews: Vec<(bool, Option<String>)> = Vec::with_capacity(n);

    // Slot 0 starts at event_start_time
    boundary_times.push(event_start_time);
    boundary_methods.push(BoundaryDetectionMethod::ScheduledClock);
    boundary_silence_durs.push(None);
    boundary_silence_mids.push(None);
    boundary_usb_medias.push(None);
    boundary_reviews.push((false, None));

    for i in 1..n {
        let prev_time = *boundary_times.last().unwrap();
        let (initial_sched, _) = parse_log_timestamp(&slots[i].scheduled_start_rfc3339)
            .unwrap_or((event_start_time, true));
        let naive = initial_sched.naive_local();

        struct CandidateMatch {
            time: DateTime<FixedOffset>,
            method: BoundaryDetectionMethod,
            silence_dur: Option<f64>,
            silence_mid: Option<String>,
            usb_media: Option<String>,
            needs_review: bool,
            review_reason: Option<String>,
            score: u8, // 0 = silence, 1 = bookmark, 2 = usb, 3 = fallback clock
            diff_ms: i64,
        }

        let mut best_match: Option<CandidateMatch> = None;

        for off in &distinct_offsets {
            let cand_sched = DateTime::<FixedOffset>::from_naive_utc_and_offset(naive - *off, *off);
            let win_start = cand_sched - Duration::minutes(15);
            let win_end = cand_sched + Duration::minutes(15);

            // Priority 1: Silence gap (>= 2.0s)
            for s in silence_events {
                let dur = s.duration_secs.unwrap_or(0.0);
                if dur >= 2.0 || s.duration_secs.is_none() {
                    let midpoint = s.start_time + Duration::milliseconds((dur * 500.0) as i64);
                    if midpoint > prev_time && midpoint >= win_start && midpoint <= win_end {
                        let diff_ms = (midpoint - cand_sched).num_milliseconds().abs();
                        let candidate = CandidateMatch {
                            time: midpoint,
                            method: BoundaryDetectionMethod::SilenceMidpoint,
                            silence_dur: Some(dur),
                            silence_mid: Some(midpoint.to_rfc3339()),
                            usb_media: None,
                            needs_review: false,
                            review_reason: None,
                            score: 0,
                            diff_ms,
                        };
                        if best_match.as_ref().map(|b| candidate.score < b.score || (candidate.score == b.score && candidate.diff_ms < b.diff_ms)).unwrap_or(true) {
                            best_match = Some(candidate);
                        }
                    }
                }
            }

            // Priority 2: Bookmark
            for b in bookmarks {
                if b.timestamp > prev_time && b.timestamp >= win_start && b.timestamp <= win_end {
                    let diff_ms = (b.timestamp - cand_sched).num_milliseconds().abs();
                    let candidate = CandidateMatch {
                        time: b.timestamp,
                        method: BoundaryDetectionMethod::ManualAdjust,
                        silence_dur: None,
                        silence_mid: None,
                        usb_media: None,
                        needs_review: false,
                        review_reason: None,
                        score: 1,
                        diff_ms,
                    };
                    if best_match.as_ref().map(|b| candidate.score < b.score || (candidate.score == b.score && candidate.diff_ms < b.diff_ms)).unwrap_or(true) {
                        best_match = Some(candidate);
                    }
                }
            }

            // Priority 3: USB handover
            let assigned = slots[i].assigned_usb_media.as_deref();
            for u in usb_handovers {
                if u.timestamp > prev_time && u.timestamp >= win_start && u.timestamp <= win_end {
                    let matches_assigned = assigned.map(|a| u.media_id == a || u.media_name == a).unwrap_or(false);
                    let diff_ms = (u.timestamp - cand_sched).num_milliseconds().abs() - if matches_assigned { 1000000 } else { 0 };
                    let candidate = CandidateMatch {
                        time: u.timestamp,
                        method: BoundaryDetectionMethod::UsbHandover,
                        silence_dur: None,
                        silence_mid: None,
                        usb_media: Some(u.media_name.clone()),
                        needs_review: false,
                        review_reason: None,
                        score: 2,
                        diff_ms,
                    };
                    if best_match.as_ref().map(|b| candidate.score < b.score || (candidate.score == b.score && candidate.diff_ms < b.diff_ms)).unwrap_or(true) {
                        best_match = Some(candidate);
                    }
                }
            }

            // Priority 4: Fallback to candidate clock
            if cand_sched > prev_time {
                let candidate = CandidateMatch {
                    time: cand_sched,
                    method: BoundaryDetectionMethod::ScheduledClock,
                    silence_dur: None,
                    silence_mid: None,
                    usb_media: None,
                    needs_review: true,
                    review_reason: Some("No silence gap (>=2.0s) or USB handover found within 15 minutes of scheduled time".to_string()),
                    score: 3,
                    diff_ms: 0,
                };
                if best_match.is_none() || best_match.as_ref().unwrap().score == 3 {
                    best_match = Some(candidate);
                }
            }
        }

        let m = best_match.unwrap_or_else(|| {
            let cand_sched = DateTime::<FixedOffset>::from_naive_utc_and_offset(
                naive - *distinct_offsets.first().unwrap(),
                *distinct_offsets.first().unwrap(),
            );
            CandidateMatch {
                time: cand_sched,
                method: BoundaryDetectionMethod::ScheduledClock,
                silence_dur: None,
                silence_mid: None,
                usb_media: None,
                needs_review: true,
                review_reason: Some("Boundary fallback".to_string()),
                score: 3,
                diff_ms: 0,
            }
        });

        boundary_times.push(m.time);
        boundary_methods.push(m.method);
        boundary_silence_durs.push(m.silence_dur);
        boundary_silence_mids.push(m.silence_mid);
        boundary_usb_medias.push(m.usb_media);
        boundary_reviews.push((m.needs_review, m.review_reason));
    }

    // Determine final event end boundary
    let last_slot = &slots[n - 1];
    let sched_last_end = last_slot
        .scheduled_end_rfc3339
        .as_deref()
        .and_then(|t| parse_log_timestamp(t).ok().map(|(dt, _)| dt));
    let default_end = *boundary_times.last().unwrap() + Duration::hours(1);
    let sched_end = sched_last_end.or(event_end_time).unwrap_or(default_end);
    let sched_end_naive = sched_end.naive_local();

    // Look for a silence event near the scheduled end across candidate offsets
    let mut final_end_time = event_end_time.unwrap_or(sched_end);
    let prev_time = *boundary_times.last().unwrap();
    let mut best_final_diff = i64::MAX;

    for off in &distinct_offsets {
        let cand_end = DateTime::<FixedOffset>::from_naive_utc_and_offset(sched_end_naive - *off, *off);
        let win_start = cand_end - Duration::minutes(15);
        let win_end = cand_end + Duration::minutes(15);

        for s in silence_events {
            if s.start_time > prev_time && s.start_time >= win_start && s.start_time <= win_end && s.duration_secs.unwrap_or(0.0) >= 2.0 {
                let diff = (s.start_time - cand_end).num_milliseconds().abs();
                if diff < best_final_diff {
                    best_final_diff = diff;
                    final_end_time = s.start_time;
                }
            }
        }
    }

    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let start_t = boundary_times[i];
        let end_t = if i + 1 < n {
            boundary_times[i + 1]
        } else {
            final_end_time
        };

        let sample_offset = if start_t >= event_start_time {
            ((start_t - event_start_time).num_milliseconds() as f64 / 1000.0 * sample_rate as f64).round() as u64
        } else {
            0
        };

        let duration_secs = (end_t - start_t).num_milliseconds().max(0) as f64 / 1000.0;
        let duration_samples = (duration_secs * sample_rate as f64).round() as u64;
        let (needs_review, review_reason) = boundary_reviews[i].clone();

        result.push(SetBoundary {
            slot_index: slots[i].slot_index,
            artist: slots[i].artist.clone(),
            boundary_time_rfc3339: start_t.to_rfc3339(),
            sample_offset,
            duration_samples,
            duration_secs,
            detection_method: boundary_methods[i],
            silence_duration_secs: boundary_silence_durs[i],
            silence_midpoint_rfc3339: boundary_silence_mids[i].clone(),
            usb_handover_media: boundary_usb_medias[i].clone(),
            needs_review,
            review_reason,
        });
    }

    result
}

/// Raw parsed metrics from ffmpeg audio analysis output (loudnorm, astats, silencedetect)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawAudioMeasurement {
    pub integrated_lufs: f32,
    pub true_peak_dbtp: f32,
    pub loudness_range_lu: f32,
    pub threshold_lufs: f32,
    pub sample_peak_dbfs: f32,
    pub rms_dbfs: f32,
    pub crest_factor_db: f32,
    pub flat_factor: f32,
    pub total_samples: u64,
    pub detected_silences: Vec<DetectedAudioSilence>,
}

/// A silence interval detected from audio analysis
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectedAudioSilence {
    pub start_secs: f64,
    pub end_secs: f64,
    pub duration_secs: f64,
}

#[derive(Debug, Deserialize)]
struct LoudnormJson {
    input_i: String,
    input_tp: String,
    input_lra: String,
    input_thresh: String,
}

/// Verifies that ffmpeg and ffprobe are available and executable on the host
pub fn check_ffmpeg_ffprobe_available() -> Result<(String, String), String> {
    let ffmpeg_out = std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map_err(|e| format!("Failed to execute 'ffmpeg -version': {}. Please install ffmpeg.", e))?;

    if !ffmpeg_out.status.success() {
        return Err("ffmpeg returned non-zero exit status".to_string());
    }

    let ffprobe_out = std::process::Command::new("ffprobe")
        .arg("-version")
        .output()
        .map_err(|e| format!("Failed to execute 'ffprobe -version': {}. Please install ffprobe.", e))?;

    if !ffprobe_out.status.success() {
        return Err("ffprobe returned non-zero exit status".to_string());
    }

    let ffmpeg_ver = String::from_utf8_lossy(&ffmpeg_out.stdout)
        .lines()
        .next()
        .unwrap_or("ffmpeg")
        .to_string();
    let ffprobe_ver = String::from_utf8_lossy(&ffprobe_out.stdout)
        .lines()
        .next()
        .unwrap_or("ffprobe")
        .to_string();

    Ok((ffmpeg_ver, ffprobe_ver))
}

/// Parses the JSON block printed by ffmpeg's loudnorm filter
pub fn parse_loudnorm_json(output: &str) -> Result<(f32, f32, f32, f32), String> {
    let open_idx = output.rfind('{').ok_or("No '{' found in output for loudnorm JSON")?;
    let close_idx = output[open_idx..].find('}').ok_or("No '}' found in output for loudnorm JSON")?;
    let json_slice = &output[open_idx..=open_idx + close_idx];
    let parsed: LoudnormJson = serde_json::from_str(json_slice)
        .map_err(|e| format!("Failed to parse loudnorm JSON: {}", e))?;
    let i = parsed.input_i.parse::<f32>().map_err(|e| format!("Invalid input_i: {}", e))?;
    let tp = parsed.input_tp.parse::<f32>().map_err(|e| format!("Invalid input_tp: {}", e))?;
    let lra = parsed.input_lra.parse::<f32>().map_err(|e| format!("Invalid input_lra: {}", e))?;
    let thresh = parsed.input_thresh.parse::<f32>().map_err(|e| format!("Invalid input_thresh: {}", e))?;
    Ok((i, tp, lra, thresh))
}

/// Parses the Overall statistics block printed by ffmpeg's astats filter
pub fn parse_astats_output(output: &str) -> Result<(f32, f32, f32, f32, u64), String> {
    let overall_idx = output.find("Overall").ok_or("No 'Overall' section found in astats output")?;
    let rest = &output[overall_idx..];

    let mut peak_db: Option<f32> = None;
    let mut rms_db: Option<f32> = None;
    let mut flat_factor: Option<f32> = None;
    let mut samples: Option<u64> = None;

    for line in rest.lines() {
        if line.contains("Peak level dB:") {
            if let Some(val_str) = line.split("Peak level dB:").nth(1) {
                peak_db = val_str.trim().parse::<f32>().ok();
            }
        } else if line.contains("RMS level dB:") {
            if let Some(val_str) = line.split("RMS level dB:").nth(1) {
                rms_db = val_str.trim().parse::<f32>().ok();
            }
        } else if line.contains("Flat factor:") {
            if let Some(val_str) = line.split("Flat factor:").nth(1) {
                flat_factor = val_str.trim().parse::<f32>().ok();
            }
        } else if line.contains("Number of samples:") {
            if let Some(val_str) = line.split("Number of samples:").nth(1) {
                samples = val_str.trim().parse::<u64>().ok();
            }
        }
    }

    let p = peak_db.unwrap_or(0.0);
    let r = rms_db.unwrap_or(-100.0);
    let f = flat_factor.unwrap_or(0.0);
    let crest = p - r;
    let s = samples.unwrap_or(0);

    Ok((p, r, crest, f, s))
}

/// Parses silence interval lines printed by ffmpeg's silencedetect filter
pub fn parse_silencedetect_output(output: &str) -> Vec<DetectedAudioSilence> {
    let mut silences = Vec::new();
    let mut cur_start: Option<f64> = None;

    for line in output.lines() {
        if let Some(idx) = line.find("silence_start: ") {
            let rest = &line[idx + 15..];
            let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
            if let Ok(st) = num_str.parse::<f64>() {
                cur_start = Some(st);
            }
        } else if let Some(idx) = line.find("silence_end: ") {
            let rest = &line[idx + 13..];
            let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
            if let Ok(end) = num_str.parse::<f64>() {
                let start = cur_start.take().unwrap_or(end);
                let dur = if let Some(d_idx) = line.find("silence_duration: ") {
                    let d_rest = &line[d_idx + 18..];
                    let d_str: String = d_rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
                    d_str.parse::<f64>().unwrap_or(end - start)
                } else {
                    end - start
                };
                silences.push(DetectedAudioSilence {
                    start_secs: start,
                    end_secs: end,
                    duration_secs: dur,
                });
            }
        }
    }

    silences
}

/// Parses the complete combined output from ffmpeg loudnorm, astats, and silencedetect
pub fn parse_ffmpeg_analysis_output(output: &str) -> Result<RawAudioMeasurement, String> {
    let (i, tp, lra, thresh) = parse_loudnorm_json(output)?;
    let (peak_db, rms_db, crest, flat_factor, total_samples) = parse_astats_output(output)?;
    let silences = parse_silencedetect_output(output);

    Ok(RawAudioMeasurement {
        integrated_lufs: i,
        true_peak_dbtp: tp,
        loudness_range_lu: lra,
        threshold_lufs: thresh,
        sample_peak_dbfs: peak_db,
        rms_dbfs: rms_db,
        crest_factor_db: crest,
        flat_factor,
        total_samples,
        detected_silences: silences,
    })
}

/// Measures audio quality of a file (or time-bounded slice) using ffmpeg with -nostats and timeout
pub fn measure_audio_file(
    path: &Path,
    start_offset_secs: Option<f64>,
    duration_secs: Option<f64>,
    timeout_secs: u64,
) -> Result<RawAudioMeasurement, String> {
    if !path.exists() {
        return Err(format!("Audio file not found: {}", path.display()));
    }

    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.arg("-nostats");

    if let Some(ss) = start_offset_secs {
        if ss > 0.0 {
            cmd.args(["-ss", &format!("{:.3}", ss)]);
        }
    }

    if let Some(dur) = duration_secs {
        if dur > 0.0 {
            cmd.args(["-t", &format!("{:.3}", dur)]);
        }
    }

    if path.extension().is_some_and(|ext| ext == "txt" || ext == "concat") {
        cmd.args(["-f", "concat", "-safe", "0"]);
    }

    cmd.args([
        "-i",
        &path.to_string_lossy(),
        "-af",
        "astats,silencedetect=noise=-45dB:d=2,loudnorm=print_format=json",
        "-f",
        "null",
        "-",
    ]);

    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn ffmpeg: {}", e))?;

    let start_t = std::time::Instant::now();
    let max_dur = std::time::Duration::from_secs(timeout_secs);

    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                break;
            }
            Ok(None) => {
                if start_t.elapsed() > max_dur {
                    let _ = child.kill();
                    return Err(format!("ffmpeg measurement timed out after {}s", timeout_secs));
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(format!("Error waiting for ffmpeg: {}", e));
            }
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to read ffmpeg output: {}", e))?;

    let combined_str = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    parse_ffmpeg_analysis_output(&combined_str)
}

/// Converts raw audio measurement into user-facing SetQualityMetrics
pub fn compute_quality_metrics(
    artist: &str,
    slot_index: usize,
    raw: &RawAudioMeasurement,
    duration_secs: f64,
    clip_count: u64,
    target_lufs: f32,
) -> SetQualityMetrics {
    let clip_percentage = if raw.total_samples > 0 {
        (clip_count as f64 / raw.total_samples as f64 * 100.0) as f32
    } else {
        0.0
    };

    let recommended_gain_db = target_lufs - raw.integrated_lufs;

    let (quality_label, clipping_unrepaired_warning) = if clip_count == 0 && raw.flat_factor < 0.001 {
        ("Clean (No digital clipping)".to_string(), false)
    } else if clip_count < 1000 && raw.flat_factor < 0.01 {
        ("Acceptable (Minor transient clipping)".to_string(), true)
    } else {
        ("Severely Clipped / Redlined".to_string(), true)
    };

    SetQualityMetrics {
        artist: artist.to_string(),
        slot_index,
        duration_secs,
        sample_peak_dbfs: raw.sample_peak_dbfs,
        true_peak_dbtp: raw.true_peak_dbtp,
        rms_dbfs: raw.rms_dbfs,
        integrated_lufs: raw.integrated_lufs,
        loudness_range_lu: raw.loudness_range_lu,
        crest_factor_db: raw.crest_factor_db,
        flat_factor: raw.flat_factor,
        clip_count,
        clip_percentage,
        recommended_gain_db,
        quality_label,
        clipping_unrepaired_warning,
    }
}

/// Result of the live recording protection guardrail check
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuardrailCheckResult {
    Allowed,
    ActiveRecording,
    QuietPeriodActive { remaining_secs: u64 },
}

/// Checks whether audio processing is permitted under the recording guardrail
pub fn check_recording_guardrail(
    state: RecordingState,
    armed_silent_duration_secs: Option<u64>,
    quiet_period_secs: u64,
) -> GuardrailCheckResult {
    match state {
        RecordingState::Recording => GuardrailCheckResult::ActiveRecording,
        RecordingState::ArmedSilent => {
            let elapsed = armed_silent_duration_secs.unwrap_or(0);
            if elapsed < quiet_period_secs {
                GuardrailCheckResult::QuietPeriodActive {
                    remaining_secs: quiet_period_secs.saturating_sub(elapsed),
                }
            } else {
                GuardrailCheckResult::Allowed
            }
        }
        RecordingState::Disconnected | RecordingState::Paused => GuardrailCheckResult::Allowed,
    }
}

/// Returns available free space in bytes on the filesystem containing `dir`
pub fn get_available_disk_space(dir: &Path) -> Result<u64, String> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let check_dir = if dir.exists() {
            dir.to_path_buf()
        } else {
            dir.parent().unwrap_or(Path::new("/")).to_path_buf()
        };

        let c_path = CString::new(check_dir.as_os_str().as_bytes())
            .map_err(|e| format!("Invalid path: {}", e))?;
        unsafe {
            let mut stat: libc::statvfs = std::mem::zeroed();
            if libc::statvfs(c_path.as_ptr(), &mut stat) == 0 {
                let avail = stat.f_bavail as u64 * stat.f_frsize as u64;
                Ok(avail)
            } else {
                Err("statvfs call failed".to_string())
            }
        }
    }
    #[cfg(not(unix))]
    {
        Ok(100 * 1024 * 1024 * 1024)
    }
}

/// Estimates required output disk space in bytes for selected render options
pub fn estimate_render_output_bytes(
    total_duration_secs: f64,
    source_bitrate_bps: u64,
    options: &RenderOptions,
    _num_artists: usize,
) -> u64 {
    let flac_byte_rate = (source_bitrate_bps as f64 / 8.0) * 1.10;
    let mp3_byte_rate = (320_000.0 / 8.0) * 1.10;

    let mut total_bytes = 0.0f64;

    // Full event files
    if options.full_event_raw_flac {
        total_bytes += flac_byte_rate * total_duration_secs;
    }
    if options.full_event_normalized_flac {
        total_bytes += flac_byte_rate * total_duration_secs;
    }

    // Artist split files (sum of all artist durations is total_duration_secs)
    if options.raw_artist_flacs {
        total_bytes += flac_byte_rate * total_duration_secs;
    }
    if options.normalized_artist_flacs {
        total_bytes += flac_byte_rate * total_duration_secs;
    }
    if options.export_mp3 {
        total_bytes += mp3_byte_rate * total_duration_secs;
    }

    // Overhead for logs, cue sheet, tracklist, and json/html reports (~10 MB)
    total_bytes += 10.0 * 1024.0 * 1024.0;

    total_bytes.round() as u64
}

/// Validates that available disk space minus estimated output size is above safety margin (default 20 GB)
pub fn check_disk_space_margin(
    output_dir: &Path,
    estimated_bytes: u64,
    safety_margin_bytes: u64,
) -> Result<u64, String> {
    let available = get_available_disk_space(output_dir)?;
    let required = estimated_bytes + safety_margin_bytes;
    if available < required {
        let avail_gb = available as f64 / (1024.0 * 1024.0 * 1024.0);
        let est_gb = estimated_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        let margin_gb = safety_margin_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        Err(format!(
            "Insufficient disk space: available {:.2} GB, but render requires ~{:.2} GB plus {:.2} GB safety margin",
            avail_gb, est_gb, margin_gb
        ))
    } else {
        Ok(available)
    }
}

/// Path to persistent job state file in an event directory
pub fn job_state_file_path(event_dir: &Path) -> PathBuf {
    event_dir.join(".studio_job.json")
}

/// Saves current job state to disk
pub fn save_job_state(event_dir: &Path, state: &JobState) -> io::Result<()> {
    let path = job_state_file_path(event_dir);
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    std::fs::write(&path, json)
}

/// Loads job state from disk if present
pub fn load_job_state(event_dir: &Path) -> io::Result<Option<JobState>> {
    let path = job_state_file_path(event_dir);
    if !path.exists() {
        return Ok(None);
    }
    let data = std::fs::read_to_string(&path)?;
    let state: JobState = serde_json::from_str(&data)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(Some(state))
}

/// Recovers job state on startup; any running jobs become Interrupted
pub fn recover_job_state(event_dir: &Path) -> io::Result<Option<JobState>> {
    if let Some(mut state) = load_job_state(event_dir)? {
        if state.status == JobStatus::Running {
            state.status = JobStatus::Interrupted;
            state.phase = "Interrupted on process restart".to_string();
            state.error_message = Some("Process was restarted during job execution".to_string());
            save_job_state(event_dir, &state)?;
        }
        Ok(Some(state))
    } else {
        Ok(None)
    }
}

/// Constructs a Command configured to run at idle CPU and IO priority (nice -n 19 ionice -c 3)
pub fn create_idle_command(program: &str) -> std::process::Command {
    let has_nice = Path::new("/usr/bin/nice").exists() || Path::new("/bin/nice").exists();
    let has_ionice = Path::new("/usr/bin/ionice").exists() || Path::new("/bin/ionice").exists();

    let mut cmd = if has_nice && has_ionice {
        let mut c = std::process::Command::new("nice");
        c.args(["-n", "19", "ionice", "-c", "3", program]);
        c
    } else if has_nice {
        let mut c = std::process::Command::new("nice");
        c.args(["-n", "19", program]);
        c
    } else {
        std::process::Command::new(program)
    };

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    cmd
}

/// Job runner that executes steps at idle priority, monitors recorder telemetry,
/// and aborts immediately (killing child process group and deleting partial outputs)
/// if the recorder starts recording.
pub struct JobRunner;

impl JobRunner {
    /// Runs a subprocess under idle priority while monitoring the telemetry state callback.
    /// If the telemetry callback returns RecordingState::Recording, kills the process group,
    /// deletes any partial outputs tracked in output_files, and marks job as Aborted.
    pub fn run_step_with_abort_watch<F>(
        mut child: std::process::Child,
        output_files: &[PathBuf],
        telemetry_poll: F,
        state: &mut JobState,
        event_dir: &Path,
    ) -> Result<std::process::ExitStatus, String>
    where
        F: Fn() -> RecordingState,
    {
        let pgid = child.id() as i32;

        loop {
            // Check recorder state
            let current_state = telemetry_poll();
            if current_state == RecordingState::Recording {
                // Abort! Kill child process group
                #[cfg(unix)]
                unsafe {
                    libc::kill(-pgid, libc::SIGTERM);
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    libc::kill(-pgid, libc::SIGKILL);
                }
                #[cfg(not(unix))]
                {
                    let _ = child.kill();
                }

                // Delete partial outputs
                for out_file in output_files {
                    if out_file.exists() {
                        let _ = std::fs::remove_file(out_file);
                    }
                }

                state.status = JobStatus::Aborted;
                state.phase = "Aborted".to_string();
                state.error_message = Some("Aborted: Recording resumed on mixer".to_string());
                let _ = save_job_state(event_dir, state);

                return Err("Job aborted because recording resumed on mixer".to_string());
            }

            match child.try_wait() {
                Ok(Some(status)) => {
                    return Ok(status);
                }
                Ok(None) => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => {
                    return Err(format!("Error polling child process: {}", e));
                }
            }
        }
    }
}

/// Builds the ffmpeg audio filter chain for loudness normalization and limiting
pub fn build_normalization_filter(
    gain_db: f32,
    ceiling_dbtp: f32,
    sample_rate: u32,
    enable_declip: bool,
) -> String {
    let ceiling_linear = 10.0f32.powf(ceiling_dbtp / 20.0);
    let oversample_rate = sample_rate * 4;

    let mut parts = Vec::new();
    if enable_declip {
        parts.push("adeclip".to_string());
    }
    parts.push("highpass=f=25".to_string());
    parts.push(format!("volume={:.2}dB", gain_db));
    parts.push(format!("aresample={}", oversample_rate));
    parts.push(format!("alimiter=limit={:.5}:level=false", ceiling_linear));
    parts.push(format!("aresample={}", sample_rate));

    parts.join(",")
}

/// Prepares an audio input reference for an EventSession.
/// If the session has a single main segment, returns its flac_path directly.
/// If it spans multiple continuous segments, writes a concat demuxer text file in `work_dir` and returns that path.
pub fn prepare_session_audio_source(session: &EventSession, work_dir: &Path) -> io::Result<PathBuf> {
    if session.segments.is_empty() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "No main segments found in event session"));
    }
    if session.segments.len() == 1 {
        return Ok(session.segments[0].flac_path.clone());
    }

    let concat_path = work_dir.join(format!("{}_concat.txt", session.id));
    let mut file = std::fs::File::create(&concat_path)?;
    use std::io::Write;
    for seg in &session.segments {
        writeln!(file, "file '{}'", seg.flac_path.display())?;
        writeln!(file, "duration {:.3}", seg.duration_secs)?;
    }
    Ok(concat_path)
}

/// Renders a single artist set (or event section) to a normalized FLAC file with metadata tags
#[allow(clippy::too_many_arguments)]
pub fn render_artist_set_flac(
    input_audio: &Path,
    output_flac: &Path,
    start_secs: f64,
    duration_secs: f64,
    filter_chain: &str,
    artist: &str,
    event_title: &str,
    event_date: &str,
    track_number: usize,
) -> Result<(), String> {
    let mut cmd = create_idle_command("ffmpeg");
    cmd.arg("-nostats");

    if start_secs > 0.0 {
        cmd.args(["-ss", &format!("{:.3}", start_secs)]);
    }
    if duration_secs > 0.0 {
        cmd.args(["-t", &format!("{:.3}", duration_secs)]);
    }

    if input_audio.extension().is_some_and(|ext| ext == "txt" || ext == "concat") {
        cmd.args(["-f", "concat", "-safe", "0"]);
    }

    cmd.args([
        "-i",
        &input_audio.to_string_lossy(),
        "-af",
        filter_chain,
        "-c:a",
        "flac",
        "-metadata",
        &format!("ARTIST={}", artist),
        "-metadata",
        &format!("TITLE={} - Live Set", artist),
        "-metadata",
        &format!("ALBUM={}", event_title),
        "-metadata",
        &format!("DATE={}", event_date),
        "-metadata",
        &format!("TRACK={}", track_number),
        "-metadata",
        "COMMENT=Recorded and normalized with djm-rec",
        "-y",
        &output_flac.to_string_lossy(),
    ]);

    let output = cmd.output().map_err(|e| format!("Failed to spawn ffmpeg render: {}", e))?;
    if !output.status.success() {
        return Err(format!(
            "ffmpeg render failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

/// Exports an existing normalized FLAC file as 320 kbps MP3 with metadata tags
pub fn render_artist_set_mp3(
    input_flac: &Path,
    output_mp3: &Path,
    artist: &str,
    event_title: &str,
    event_date: &str,
    track_number: usize,
) -> Result<(), String> {
    let mut cmd = create_idle_command("ffmpeg");
    cmd.args([
        "-nostats",
        "-i",
        &input_flac.to_string_lossy(),
        "-c:a",
        "libmp3lame",
        "-b:a",
        "320k",
        "-metadata",
        &format!("ARTIST={}", artist),
        "-metadata",
        &format!("TITLE={} - Live Set", artist),
        "-metadata",
        &format!("ALBUM={}", event_title),
        "-metadata",
        &format!("DATE={}", event_date),
        "-metadata",
        &format!("TRACK={}", track_number),
        "-y",
        &output_mp3.to_string_lossy(),
    ]);

    let output = cmd.output().map_err(|e| format!("Failed to spawn ffmpeg MP3 export: {}", e))?;
    if !output.status.success() {
        return Err(format!(
            "ffmpeg MP3 export failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

/// Concatenates multiple audio files into a single full-event file using ffmpeg concat demuxer
pub fn concatenate_audio_files(
    input_files: &[PathBuf],
    output_file: &Path,
) -> Result<(), String> {
    if input_files.is_empty() {
        return Err("No input files provided for concatenation".to_string());
    }

    if input_files.len() == 1 {
        std::fs::copy(&input_files[0], output_file)
            .map_err(|e| format!("Failed to copy single file: {}", e))?;
        return Ok(());
    }

    let concat_list_path = output_file.with_extension("concat_list.txt");
    let mut list_content = String::new();
    for f in input_files {
        list_content.push_str(&format!("file '{}'\n", f.to_string_lossy()));
    }
    std::fs::write(&concat_list_path, list_content)
        .map_err(|e| format!("Failed to write concat list file: {}", e))?;

    let mut cmd = create_idle_command("ffmpeg");
    cmd.args([
        "-nostats",
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
        &concat_list_path.to_string_lossy(),
        "-c",
        "copy",
        "-y",
        &output_file.to_string_lossy(),
    ]);

    let output = cmd.output().map_err(|e| format!("Failed to execute ffmpeg concat: {}", e))?;
    let _ = std::fs::remove_file(&concat_list_path);

    if !output.status.success() {
        return Err(format!(
            "ffmpeg concatenation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

/// Re-measures a rendered audio file and flags targets missed by > 1.0 LU or true peak exceeding ceiling
pub fn verify_rendered_output(
    file_path: &Path,
    is_artist_set: bool,
    artist_name: Option<&str>,
    target_lufs: f32,
    ceiling_dbtp: f32,
    timeout_secs: u64,
) -> Result<MeasuredVerificationMetrics, String> {
    let raw = measure_audio_file(file_path, None, None, timeout_secs)?;
    let lufs_diff = (raw.integrated_lufs - target_lufs).abs();

    let mut target_missed = false;
    let mut reasons = Vec::new();

    if lufs_diff > 1.0 {
        target_missed = true;
        reasons.push(format!(
            "Loudness deviation > 1.0 LU: measured {:.2} LUFS vs target {:.2} LUFS (difference: {:.2} LU)",
            raw.integrated_lufs, target_lufs, lufs_diff
        ));
    }

    if raw.true_peak_dbtp > ceiling_dbtp + 0.1 {
        target_missed = true;
        reasons.push(format!(
            "True peak exceeds ceiling: measured {:.2} dBTP vs ceiling {:.2} dBTP",
            raw.true_peak_dbtp, ceiling_dbtp
        ));
    }

    let miss_reason = if target_missed {
        Some(reasons.join("; "))
    } else {
        None
    };

    let file_name = file_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());

    Ok(MeasuredVerificationMetrics {
        file_name,
        file_path: file_path.to_path_buf(),
        is_artist_set,
        artist_name: artist_name.map(|s| s.to_string()),
        measured_integrated_lufs: raw.integrated_lufs,
        target_integrated_lufs: target_lufs,
        lufs_difference_lu: lufs_diff,
        measured_true_peak_dbtp: raw.true_peak_dbtp,
        ceiling_true_peak_dbtp: ceiling_dbtp,
        measured_sample_peak_dbfs: raw.sample_peak_dbfs,
        target_missed,
        miss_reason,
    })
}

/// Verifies a collection of rendered outputs against normalization target and ceiling
pub fn verify_all_rendered_outputs(
    files: &[(PathBuf, bool, Option<String>)],
    target_lufs: f32,
    ceiling_dbtp: f32,
    timeout_secs: u64,
) -> Result<Vec<MeasuredVerificationMetrics>, String> {
    let mut results = Vec::with_capacity(files.len());
    for (path, is_artist, artist_name) in files {
        if path.exists() {
            let ver = verify_rendered_output(
                path,
                *is_artist,
                artist_name.as_deref(),
                target_lufs,
                ceiling_dbtp,
                timeout_secs,
            )?;
            results.push(ver);
        }
    }
    Ok(results)
}

/// Converts seconds into MM:SS:FF format for CUE sheet indices (75 frames per second)
pub fn seconds_to_cue_time(seconds: f64) -> String {
    let total_secs = seconds.max(0.0);
    let mins = (total_secs / 60.0).floor() as u32;
    let rem_secs = total_secs - (mins as f64 * 60.0);
    let whole_secs = rem_secs.floor() as u32;
    let frames = ((rem_secs - whole_secs as f64) * 75.0).round() as u32;
    let frames = frames.min(74);

    format!("{:02}:{:02}:{:02}", mins, whole_secs, frames)
}

/// Formats seconds into HH:MM:SS
pub fn seconds_to_hms(seconds: f64) -> String {
    let total_secs = seconds.max(0.0).round() as u64;
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

/// Generates a standard CUE sheet for navigating the full event recording by artist set
pub fn generate_cue_sheet(report: &EventReport, audio_file_name: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("PERFORMER \"{}\"\n", report.mixer_model));
    out.push_str(&format!("TITLE \"{}\"\n", report.event_id));
    out.push_str(&format!("FILE \"{}\" WAVE\n", audio_file_name));

    for (idx, b) in report.boundaries.iter().enumerate() {
        let track_num = idx + 1;
        out.push_str(&format!("  TRACK {:02} AUDIO\n", track_num));
        out.push_str(&format!("    TITLE \"{} - Live Set\"\n", b.artist));
        out.push_str(&format!("    PERFORMER \"{}\"\n", b.artist));
        let cue_time = seconds_to_cue_time(b.sample_offset as f64 / report.sample_rate as f64);
        out.push_str(&format!("    INDEX 01 {}\n", cue_time));
    }

    out
}

/// Generates a human-readable plain text tracklist with timestamps and artist details
pub fn generate_tracklist_txt(report: &EventReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("Event: {}\n", report.event_id));
    out.push_str(&format!("Mixer: {}\n", report.mixer_model));
    out.push_str(&format!("Duration: {}\n", report.duration_formatted));
    out.push_str(&format!("Generated: {}\n", report.generated_at_rfc3339));
    out.push_str("----------------------------------------------------------------------\n");
    out.push_str("ARTIST SET TIMELINE:\n");

    for (idx, b) in report.boundaries.iter().enumerate() {
        let elapsed = seconds_to_hms(b.sample_offset as f64 / report.sample_rate as f64);
        let dur = seconds_to_hms(b.duration_secs);
        out.push_str(&format!(
            "{:02}. [{}] {} (Duration: {}, Started: {})\n",
            idx + 1,
            elapsed,
            b.artist,
            dur,
            b.boundary_time_rfc3339
        ));
    }

    if !report.tracklist_summary.is_empty() {
        out.push_str("\n----------------------------------------------------------------------\n");
        out.push_str("IDENTIFIED PRO DJ LINK TRACKS:\n");
        for entry in &report.tracklist_summary {
            out.push_str(&format!(
                "\n--- {} (USB Media: {}) ---\n",
                entry.artist,
                entry.usb_media_name.as_deref().unwrap_or("Unknown")
            ));
            for (t_idx, tr) in entry.tracks.iter().enumerate() {
                out.push_str(&format!("  {:02}. {}\n", t_idx + 1, tr));
            }
        }
    }

    out
}

/// Generates the complete JSON format event report
pub fn generate_event_report_json(report: &EventReport) -> Result<String, String> {
    serde_json::to_string_pretty(report)
        .map_err(|e| format!("Failed to serialize event report to JSON: {}", e))
}

/// Generates a comprehensive GFM Markdown event report with measured values only
pub fn generate_event_report_markdown(report: &EventReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Event Audio Quality & Mastering Report: {}\n\n", report.event_id));

    out.push_str("## 1. Overview\n\n");
    out.push_str(&format!("- **Event ID**: `{}`\n", report.event_id));
    out.push_str(&format!("- **Mixer**: {}\n", report.mixer_model));
    out.push_str(&format!("- **Total Duration**: {}\n", report.duration_formatted));
    out.push_str(&format!("- **Sample Rate**: {} Hz (24-bit PCM)\n", report.sample_rate));
    out.push_str(&format!("- **Total Samples**: {}\n", report.total_samples));
    out.push_str(&format!("- **Report Generated**: {}\n\n", report.generated_at_rfc3339));

    // Continuity Audit
    out.push_str("## 2. Sample-Count Continuity Audit\n\n");
    if report.continuity_audit.is_continuous {
        out.push_str(&format!(
            "✓ **Continuous Recording Verified**: All {} rollover seam(s) verified within 50 ms tolerance.\n\n",
            report.continuity_audit.checked_seams
        ));
    } else {
        out.push_str(&format!(
            "⚠ **Continuity Discontinuity Detected**: {} seam(s) failed continuity check!\n\n",
            report.continuity_audit.failed_seams
        ));
    }

    if !report.continuity_audit.seam_reports.is_empty() {
        out.push_str("| Seam | Split Reason | Samples Duration | Clock Interval | Discrepancy | Status |\n");
        out.push_str("| :--- | :--- | :--- | :--- | :--- | :--- |\n");
        for s in &report.continuity_audit.seam_reports {
            let status = if s.is_within_tolerance { "✓ PASS" } else { "✗ FAIL" };
            out.push_str(&format!(
                "| pt{:02} → pt{:02} | {} | {:.2} s | {:.2} s | {:.1} ms | {} |\n",
                s.from_segment_index, s.to_segment_index, s.split_reason, s.duration_from_samples_secs, s.interval_between_starts_secs, s.discrepancy_ms, status
            ));
        }
        out.push('\n');
    }

    // Ambient Stream Alignment
    if !report.ambient_alignment.is_empty() {
        out.push_str("## 3. Ambient Stream Alignment Audit\n\n");
        out.push_str("| Segment | Ambient File | Ambient Duration | Master Duration | Discrepancy | Aligned | Notes |\n");
        out.push_str("| :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n");
        for amb in &report.ambient_alignment {
            let aligned = if amb.is_aligned { "✓ Aligned" } else { "✗ Misaligned" };
            let note = amb.note.as_deref().unwrap_or("-");
            let file_str = amb.ambient_file.file_name().and_then(|n| n.to_str()).unwrap_or("");
            out.push_str(&format!(
                "| pt{:02} | `{}` | {:.1} s | {:.1} s | {:+.1} s | {} | {} |\n",
                amb.segment_index, file_str, amb.ambient_duration_secs, amb.master_duration_secs, amb.discrepancy_secs, aligned, note
            ));
        }
        out.push('\n');
    }

    // Set Boundaries
    out.push_str("## 4. Set Boundaries & Transition Detection\n\n");
    out.push_str("| Slot | Artist | Boundary Time | Offset (Samples) | Duration | Detection Method | Silence Dur | Status |\n");
    out.push_str("| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n");
    for b in &report.boundaries {
        let method = match b.detection_method {
            BoundaryDetectionMethod::SilenceMidpoint => "Silence Midpoint",
            BoundaryDetectionMethod::UsbHandover => "USB Handover",
            BoundaryDetectionMethod::ScheduledClock => "Scheduled Clock",
            BoundaryDetectionMethod::ManualAdjust => "Manual Adjust / Bookmark",
        };
        let sil_dur = b.silence_duration_secs.map(|d| format!("{:.1}s", d)).unwrap_or_else(|| "-".to_string());
        let status = if b.needs_review { "⚠ Review" } else { "✓ Clean" };
        out.push_str(&format!(
            "| {:02} | {} | {} | {} | {} | {} | {} | {} |\n",
            b.slot_index + 1, b.artist, b.boundary_time_rfc3339, b.sample_offset, seconds_to_hms(b.duration_secs), method, sil_dur, status
        ));
    }
    out.push('\n');

    // Audio Quality Analysis (Before Normalization)
    out.push_str("## 5. Measured Audio Quality Analysis (Source)\n\n");
    out.push_str("| Artist | Duration | Peak (dBFS) | True Peak (dBTP) | RMS (dBFS) | Integrated LUFS | LRA | Crest | Flat Factor | Clips | Clip % | Quality Rating |\n");
    out.push_str("| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n");
    for m in &report.pre_render_metrics {
        out.push_str(&format!(
            "| {} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} dB | {:.6} | {} | {:.4}% | {} |\n",
            m.artist, seconds_to_hms(m.duration_secs), m.sample_peak_dbfs, m.true_peak_dbtp, m.rms_dbfs, m.integrated_lufs, m.loudness_range_lu, m.crest_factor_db, m.flat_factor, m.clip_count, m.clip_percentage, m.quality_label
        ));
    }
    out.push('\n');

    // Clipping Warning Callout
    let has_clipped_sets = report.pre_render_metrics.iter().any(|m| m.clipping_unrepaired_warning);
    if has_clipped_sets {
        out.push_str("> [!WARNING]\n");
        out.push_str("> **Note on Digital Clipping Distortion**: One or more sets contain digital clipping from audio played in the red on the mixer. Loudness normalization reduces overall signal level and prevents DAC overshoots, but flat-topped clipping distortion baked into the source recording cannot be fully undone.\n\n");
    }

    // Post-Render Verification
    if !report.post_render_verification.is_empty() {
        out.push_str("## 6. Measured Post-Render Output Verification\n\n");
        out.push_str("| Output File | Artist | Target LUFS | Measured LUFS | Difference | Ceiling | Measured TP | Peak dBFS | Result |\n");
        out.push_str("| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |\n");
        for v in &report.post_render_verification {
            let res = if v.target_missed { "⚠ MISSED" } else { "✓ PASS" };
            let art = v.artist_name.as_deref().unwrap_or("-");
            out.push_str(&format!(
                "| `{}` | {} | {:.1} | {:.2} | {:.2} LU | {:.1} | {:.2} | {:.2} | {} |\n",
                v.file_name, art, v.target_integrated_lufs, v.measured_integrated_lufs, v.lufs_difference_lu, v.ceiling_true_peak_dbtp, v.measured_true_peak_dbtp, v.measured_sample_peak_dbfs, res
            ));
        }
        out.push('\n');
    }

    // Tracklists
    if !report.tracklist_summary.is_empty() {
        out.push_str("## 7. Pro DJ Link Tracklists\n\n");
        for t in &report.tracklist_summary {
            out.push_str(&format!("### {} (Media: {})\n\n", t.artist, t.usb_media_name.as_deref().unwrap_or("Unknown")));
            for (idx, song) in t.tracks.iter().enumerate() {
                out.push_str(&format!("{}. {}\n", idx + 1, song));
            }
            out.push('\n');
        }
    }

    out
}

/// Generates a self-contained, responsive HTML event report
pub fn generate_event_report_html(report: &EventReport) -> String {
    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str(&format!("<title>Event Audio Quality Report - {}</title>\n", report.event_id));
    html.push_str(r#"<style>
body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; background: #0f172a; color: #f8fafc; margin: 0; padding: 24px; line-height: 1.5; }
.container { max-width: 1200px; margin: 0 auto; }
h1, h2, h3 { color: #f1f5f9; font-weight: 600; }
h1 { border-bottom: 2px solid #334155; padding-bottom: 12px; }
h2 { border-bottom: 1px solid #1e293b; padding-bottom: 8px; margin-top: 32px; }
table { width: 100%; border-collapse: collapse; margin: 16px 0; font-size: 14px; background: #1e293b; border-radius: 6px; overflow: hidden; }
th, td { padding: 10px 14px; text-align: left; border-bottom: 1px solid #334155; }
th { background: #334155; color: #94a3b8; font-weight: 600; text-transform: uppercase; font-size: 12px; letter-spacing: 0.5px; }
tr:hover { background: #243248; }
.badge { display: inline-block; padding: 3px 8px; border-radius: 4px; font-size: 12px; font-weight: 600; }
.badge-pass { background: #065f46; color: #34d399; }
.badge-warn { background: #92400e; color: #fbbf24; }
.badge-fail { background: #991b1b; color: #f87171; }
.warning-box { background: #451a03; border-left: 4px solid #f59e0b; padding: 14px 18px; border-radius: 4px; margin: 20px 0; color: #fef3c7; }
.meta-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 12px; margin: 20px 0; }
.meta-card { background: #1e293b; padding: 14px; border-radius: 6px; border: 1px solid #334155; }
.meta-card .label { font-size: 12px; color: #94a3b8; text-transform: uppercase; }
.meta-card .val { font-size: 18px; font-weight: 600; color: #38bdf8; margin-top: 4px; }
</style>
</head>
<body>
<div class="container">
"#);

    html.push_str(&format!("<h1>Event Audio Quality & Mastering Report: {}</h1>\n", report.event_id));

    html.push_str("<div class=\"meta-grid\">\n");
    html.push_str(&format!("<div class=\"meta-card\"><div class=\"label\">Mixer Model</div><div class=\"val\">{}</div></div>\n", report.mixer_model));
    html.push_str(&format!("<div class=\"meta-card\"><div class=\"label\">Total Duration</div><div class=\"val\">{}</div></div>\n", report.duration_formatted));
    html.push_str(&format!("<div class=\"meta-card\"><div class=\"label\">Sample Rate</div><div class=\"val\">{} Hz</div></div>\n", report.sample_rate));
    html.push_str(&format!("<div class=\"meta-card\"><div class=\"label\">Report Generated</div><div class=\"val\">{}</div></div>\n", report.generated_at_rfc3339));
    html.push_str("</div>\n");

    // Continuity
    html.push_str("<h2>Sample-Count Continuity Audit</h2>\n");
    if report.continuity_audit.is_continuous {
        html.push_str("<p><span class=\"badge badge-pass\">✓ Continuous Recording Verified</span> All rollover seams matched within 50 ms tolerance.</p>\n");
    } else {
        html.push_str("<p><span class=\"badge badge-fail\">⚠ Continuity Seam Failed</span> Discontinuity detected across rollover segments.</p>\n");
    }

    if !report.continuity_audit.seam_reports.is_empty() {
        html.push_str("<table><thead><tr><th>Seam</th><th>Reason</th><th>Duration from Samples</th><th>Wall-Clock Interval</th><th>Discrepancy</th><th>Status</th></tr></thead><tbody>\n");
        for s in &report.continuity_audit.seam_reports {
            let badge = if s.is_within_tolerance { "<span class=\"badge badge-pass\">PASS</span>" } else { "<span class=\"badge badge-fail\">FAIL</span>" };
            html.push_str(&format!(
                "<tr><td>pt{:02} → pt{:02}</td><td>{}</td><td>{:.2}s</td><td>{:.2}s</td><td>{:.1} ms</td><td>{}</td></tr>\n",
                s.from_segment_index, s.to_segment_index, s.split_reason, s.duration_from_samples_secs, s.interval_between_starts_secs, s.discrepancy_ms, badge
            ));
        }
        html.push_str("</tbody></table>\n");
    }

    // Ambient Stream Alignment
    if !report.ambient_alignment.is_empty() {
        html.push_str("<h2>Ambient Stream Alignment Audit</h2>\n");
        html.push_str("<table><thead><tr><th>Segment</th><th>Ambient File</th><th>Ambient Duration</th><th>Master Duration</th><th>Discrepancy</th><th>Status</th><th>Notes</th></tr></thead><tbody>\n");
        for amb in &report.ambient_alignment {
            let badge = if amb.is_aligned { "<span class=\"badge badge-pass\">Aligned</span>" } else { "<span class=\"badge badge-fail\">Misaligned</span>" };
            let file_str = amb.ambient_file.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let note = amb.note.as_deref().unwrap_or("-");
            html.push_str(&format!(
                "<tr><td>pt{:02}</td><td><code>{}</code></td><td>{:.1}s</td><td>{:.1}s</td><td>{:+.1}s</td><td>{}</td><td>{}</td></tr>\n",
                amb.segment_index, file_str, amb.ambient_duration_secs, amb.master_duration_secs, amb.discrepancy_secs, badge, note
            ));
        }
        html.push_str("</tbody></table>\n");
    }

    // Audio Quality Analysis
    html.push_str("<h2>Measured Audio Quality Analysis (Source)</h2>\n");
    html.push_str("<table><thead><tr><th>Artist</th><th>Duration</th><th>Peak (dBFS)</th><th>True Peak (dBTP)</th><th>RMS (dBFS)</th><th>Integrated LUFS</th><th>LRA</th><th>Crest</th><th>Flat Factor</th><th>Clips</th><th>Rating</th></tr></thead><tbody>\n");
    for m in &report.pre_render_metrics {
        let badge = if m.clip_count == 0 { "<span class=\"badge badge-pass\">Clean</span>" } else { "<span class=\"badge badge-fail\">Redlined</span>" };
        html.push_str(&format!(
            "<tr><td><strong>{}</strong></td><td>{}</td><td>{:.2}</td><td>{:.2}</td><td>{:.2}</td><td>{:.2}</td><td>{:.2}</td><td>{:.2} dB</td><td>{:.6}</td><td>{}</td><td>{}</td></tr>\n",
            m.artist, seconds_to_hms(m.duration_secs), m.sample_peak_dbfs, m.true_peak_dbtp, m.rms_dbfs, m.integrated_lufs, m.loudness_range_lu, m.crest_factor_db, m.flat_factor, m.clip_count, badge
        ));
    }
    html.push_str("</tbody></table>\n");

    let has_clipped = report.pre_render_metrics.iter().any(|m| m.clipping_unrepaired_warning);
    if has_clipped {
        html.push_str("<div class=\"warning-box\"><strong>Note on Clipping Distortion:</strong> One or more sets contain digital clipping from audio played in the red on the mixer. Loudness normalization reduces overall signal level and prevents DAC overshoots, but flat-topped clipping distortion baked into the source recording cannot be fully undone.</div>\n");
    }

    // Post-render verification
    if !report.post_render_verification.is_empty() {
        html.push_str("<h2>Measured Post-Render Output Verification</h2>\n");
        html.push_str("<table><thead><tr><th>File</th><th>Artist</th><th>Target LUFS</th><th>Measured LUFS</th><th>Diff (LU)</th><th>Ceiling</th><th>Measured TP</th><th>Peak dBFS</th><th>Status</th></tr></thead><tbody>\n");
        for v in &report.post_render_verification {
            let badge = if v.target_missed { "<span class=\"badge badge-warn\">MISSED</span>" } else { "<span class=\"badge badge-pass\">PASS</span>" };
            let art = v.artist_name.as_deref().unwrap_or("-");
            html.push_str(&format!(
                "<tr><td><code>{}</code></td><td>{}</td><td>{:.1}</td><td>{:.2}</td><td>{:.2}</td><td>{:.1}</td><td>{:.2}</td><td>{:.2}</td><td>{}</td></tr>\n",
                v.file_name, art, v.target_integrated_lufs, v.measured_integrated_lufs, v.lufs_difference_lu, v.ceiling_true_peak_dbtp, v.measured_true_peak_dbtp, v.measured_sample_peak_dbfs, badge
            ));
        }
        html.push_str("</tbody></table>\n");
    }

    html.push_str("</div>\n</body>\n</html>\n");
    html
}

/// Writes all report formats (HTML, Markdown, JSON, tracklist.txt, and optional .cue sheet) into output_dir
pub fn write_event_reports(
    report: &EventReport,
    output_dir: &Path,
    cue_audio_filename: Option<&str>,
) -> io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(output_dir)?;
    let mut written = Vec::new();

    // 1. Event_Report.json
    let json_path = output_dir.join("Event_Report.json");
    let json_content = generate_event_report_json(report)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    std::fs::write(&json_path, json_content)?;
    written.push(json_path);

    // 2. Event_Report.md
    let md_path = output_dir.join("Event_Report.md");
    let md_content = generate_event_report_markdown(report);
    std::fs::write(&md_path, md_content)?;
    written.push(md_path);

    // 3. Event_Report.html
    let html_path = output_dir.join("Event_Report.html");
    let html_content = generate_event_report_html(report);
    std::fs::write(&html_path, html_content)?;
    written.push(html_path);

    // 4. tracklist.txt
    let txt_path = output_dir.join("tracklist.txt");
    let txt_content = generate_tracklist_txt(report);
    std::fs::write(&txt_path, txt_content)?;
    written.push(txt_path);

    // 5. event.cue (if audio file name is provided)
    if let Some(audio_name) = cue_audio_filename {
        let cue_path = output_dir.join("event.cue");
        let cue_content = generate_cue_sheet(report, audio_name);
        std::fs::write(&cue_path, cue_content)?;
        written.push(cue_path);
    }

    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_timestamps_old_and_new() {
        // Old timestamp without offset
        let (dt_old, has_offset_old) = parse_log_timestamp("2026-10-30 22:00:06.567").unwrap();
        assert!(!has_offset_old);
        assert_eq!(dt_old.format("%Y-%m-%d %H:%M:%S").to_string(), "2026-10-30 22:00:06");

        // Old timestamp with T without offset
        let (dt_t, has_offset_t) = parse_log_timestamp("2026-10-30T22:00:06.567").unwrap();
        assert!(!has_offset_t);
        assert_eq!(dt_t.format("%Y-%m-%d %H:%M:%S").to_string(), "2026-10-30 22:00:06");

        // New RFC 3339 timestamp with positive offset
        let (dt_new, has_offset_new) = parse_log_timestamp("2026-10-30T23:00:02.921+01:00").unwrap();
        assert!(has_offset_new);
        assert_eq!(dt_new.format("%Y-%m-%d %H:%M:%S").to_string(), "2026-10-30 23:00:02");
        assert_eq!(dt_new.offset().local_minus_utc(), 3600);

        // New RFC 3339 timestamp with UTC 'Z'
        let (dt_z, has_offset_z) = parse_log_timestamp("2026-10-30T22:00:02.921Z").unwrap();
        assert!(has_offset_z);
        assert_eq!(dt_z.offset().local_minus_utc(), 0);
    }

    #[test]
    fn test_parse_real_2026_10_30_logs_pt04_to_pt08() {
        let rec_dir = Path::new("/home/bram/Recordings");
        let pt04 = rec_dir.join("AlphaTheta_DJM-A9_2026-10-30_22-00-06_pt04.log");
        let pt05 = rec_dir.join("AlphaTheta_DJM-A9_2026-10-30_22-41-01_pt05.log");
        let pt06 = rec_dir.join("AlphaTheta_DJM-A9_2026-10-30_23-00-02_pt06.log");
        let pt07 = rec_dir.join("AlphaTheta_DJM-A9_2026-10-31_02-00-02_pt07.log");
        let pt08 = rec_dir.join("AlphaTheta_DJM-A9_2026-10-31_05-00-02_pt08.log");

        if !pt04.exists() {
            eprintln!("Recordings directory not available on this machine, skipping");
            return;
        }

        // Test pt04
        let p4 = parse_session_log(&pt04).expect("parse pt04");
        assert_eq!(p4.mixer.as_deref(), Some("AlphaTheta DJM-A9"));
        assert_eq!(p4.sample_rate, Some(96000));
        assert_eq!(p4.pad_db, Some(-19));
        assert_eq!(p4.auto_guard, Some(true));
        assert_eq!(p4.preroll_samples, 192000);
        assert_eq!(p4.stop_reason.as_deref(), Some("Silence Timeout Gate"));
        assert!(!p4.has_tz_offset); // Old format
        assert_eq!(p4.silence_events.len(), 6);
        assert!(p4.silence_events.last().unwrap().duration_secs.unwrap() > 119.0);

        // Test pt05 (Warmup 2)
        let p5 = parse_session_log(&pt05).expect("parse pt05");
        assert_eq!(p5.mixer.as_deref(), Some("AlphaTheta DJM-A9"));
        assert_eq!(p5.stop_reason.as_deref(), Some("Silence Timeout Gate"));

        // Test pt06 (Event start, 3-hour rollover)
        let p6 = parse_session_log(&pt06).expect("parse pt06");
        assert_eq!(p6.stop_reason.as_deref(), Some("Max Segment Duration Rollover"));
        assert_eq!(p6.total_duration_secs, Some(10800.0));
        assert_eq!(p6.total_clips, 37503);

        // Test pt07 (Redlined set rollover)
        let p7 = parse_session_log(&pt07).expect("parse pt07");
        assert_eq!(p7.stop_reason.as_deref(), Some("Max Segment Duration Rollover"));
        assert_eq!(p7.total_duration_secs, Some(10800.0));
        assert_eq!(p7.total_clips, 65535);

        // Test pt08 (Final set)
        let p8 = parse_session_log(&pt08).expect("parse pt08");
        assert_eq!(p8.stop_reason.as_deref(), Some("Silence Timeout Gate"));
        assert!(p8.total_duration_secs.unwrap() > 3700.0);
    }

    #[test]
    fn test_session_grouping_and_warmup_detection() {
        let rec_dir = Path::new("/home/bram/Recordings");
        if !rec_dir.exists() {
            return;
        }

        let sessions = group_event_sessions(rec_dir).expect("group sessions");
        assert!(!sessions.is_empty());

        // Find the main 2026-10-30 evening session containing pt06
        let main_session = sessions
            .iter()
            .find(|s| s.segments.iter().any(|seg| seg.index == 6))
            .expect("should find session containing pt06");

        // Verify pt06 -> pt07 -> pt08 are in continuous segments
        let segment_indices: Vec<usize> = main_session.segments.iter().map(|s| s.index).collect();
        assert_eq!(segment_indices, vec![6, 7, 8]);

        // Verify pt04 and pt05 are detected as warmups
        let warmup_indices: Vec<usize> = main_session.warmup_segments.iter().map(|s| s.index).collect();
        assert!(warmup_indices.contains(&4), "pt04 should be a warmup");
        assert!(warmup_indices.contains(&5), "pt05 should be a warmup");

        // Verify sample-count continuity check:
        // pt06 -> pt07 is checked within 50 ms tolerance!
        assert!(!main_session.continuity_audit.seam_reports.is_empty());
        let seam_06_07 = &main_session.continuity_audit.seam_reports[0];
        assert_eq!(seam_06_07.from_segment_index, 6);
        assert_eq!(seam_06_07.to_segment_index, 7);
        assert!(
            seam_06_07.is_within_tolerance,
            "pt06->pt07 seam must be within 50ms tolerance: discrepancy = {} ms",
            seam_06_07.discrepancy_ms
        );
        assert!(seam_06_07.discrepancy_ms < 50.0);
    }

    #[test]
    fn test_ambient_alignment_audit_reports_pt04_mismatch() {
        let rec_dir = Path::new("/home/bram/Recordings");
        if !rec_dir.exists() {
            return;
        }

        let pt04_log = rec_dir.join("AlphaTheta_DJM-A9_2026-10-30_22-00-06_pt04.log");
        if !pt04_log.exists() {
            return;
        }

        let seg04 = build_segment_info(&pt04_log).expect("build pt04 segment");
        assert!(seg04.ambient_path.is_some());

        let audit = audit_ambient_alignment(&[seg04]);
        assert_eq!(audit.len(), 1);

        let rep = &audit[0];
        assert_eq!(rep.segment_index, 4);
        assert!(!rep.is_aligned, "pt04 ambient should NOT be aligned (ran ~4h)");
        assert!(
            rep.discrepancy_secs > 10000.0,
            "Discrepancy should be >10,000s: got {:.1}s",
            rep.discrepancy_secs
        );
        assert!(rep.note.as_ref().unwrap().contains("spans multiple segments"));
    }

    #[test]
    fn test_parse_timetable_overnight_wrap() {
        let timetable_raw = r#"
23:00 Next horizon
00:00 Nora B b2b EML
01:00 TAXMAN
02:00 black sun empire
03:00 Merikan
04:00 Crater crew 3nlight.
05:00 Aliienmia
"#;
        let event_date = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let tz = FixedOffset::east_opt(3600).unwrap();

        let slots = parse_timetable(timetable_raw, event_date, tz).expect("parse timetable");
        assert_eq!(slots.len(), 7);

        // Next Horizon is on 2026-10-30
        assert_eq!(slots[0].artist, "Next horizon");
        assert_eq!(slots[0].target_date, "2026-10-30");
        assert_eq!(slots[0].scheduled_start_clock, "23:00");
        assert_eq!(slots[0].scheduled_end_clock.as_deref(), Some("00:00"));
        assert!(slots[0].scheduled_start_rfc3339.starts_with("2026-10-30T23:00:00"));

        // Nora B is on 2026-10-31 (wrapped past midnight)
        assert_eq!(slots[1].artist, "Nora B b2b EML");
        assert_eq!(slots[1].target_date, "2026-10-31");
        assert_eq!(slots[1].scheduled_start_clock, "00:00");
        assert_eq!(slots[1].scheduled_end_clock.as_deref(), Some("01:00"));
        assert!(slots[1].scheduled_start_rfc3339.starts_with("2026-10-31T00:00:00"));

        // Aliienmia is on 2026-10-31
        assert_eq!(slots[6].artist, "Aliienmia");
        assert_eq!(slots[6].target_date, "2026-10-31");
        assert_eq!(slots[6].scheduled_start_clock, "05:00");
        assert_eq!(slots[6].scheduled_end_clock.as_deref(), Some("06:00"));
        assert!(slots[6].scheduled_start_rfc3339.starts_with("2026-10-31T05:00:00"));

        // Test with range format and prefixes
        let range_raw = r#"
1. 23:00 - 00:00 Next Horizon
2. 00:00 - 01:00 Nora B b2b EML
"#;
        let range_slots = parse_timetable(range_raw, event_date, tz).expect("parse range timetable");
        assert_eq!(range_slots.len(), 2);
        assert_eq!(range_slots[0].artist, "Next Horizon");
        assert_eq!(range_slots[0].scheduled_start_clock, "23:00");
        assert_eq!(range_slots[0].scheduled_end_clock.as_deref(), Some("00:00"));
    }

    #[test]
    fn test_resolve_boundaries_reproduces_2026_10_30_manual_findings() {
        let timetable_raw = r#"
23:00 Next horizon
00:00 Nora B b2b EML
01:00 TAXMAN
02:00 black sun empire
03:00 Merikan
04:00 Crater crew 3nlight.
05:00 Aliienmia
"#;
        let event_date = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let tz = FixedOffset::east_opt(3600).unwrap();
        let slots = parse_timetable(timetable_raw, event_date, tz).expect("parse timetable");

        let event_start = DateTime::<FixedOffset>::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(2026, 10, 30)
                .unwrap()
                .and_hms_milli_opt(23, 0, 2, 921)
                .unwrap()
                - tz,
            tz,
        );

        // Audio silence events detected from the 2026-10-30 recording:
        // Transition 1: 3588.06 to 3597.05s -> midpoint 3592.55s (wall clock 23:59:55.47)
        // Transition 2: 7443.74 to 7451.96s -> midpoint 7447.85s (wall clock 01:04:10.77)
        // Transition 3: 10767.85 to 10779.87s -> midpoint 10773.86s (wall clock 01:59:36.78)
        // Transition 4: 14476.70 to 14484.93s -> midpoint 14480.82s (wall clock 03:01:23.74)
        // Transition 5: 17976.82 to 18003.25s -> midpoint 17990.04s (wall clock 03:59:52.96)
        // Transition 6: 21645.68 to 21683.08s -> midpoint 21664.38s (wall clock 05:01:07.30)
        // Transition 7: ends at 25222.93s (wall clock 06:00:25.85)
        let make_silence = |offset_secs: f64, dur_secs: f64| -> SilenceEvent {
            let start = event_start + Duration::milliseconds((offset_secs * 1000.0) as i64);
            let end = start + Duration::milliseconds((dur_secs * 1000.0) as i64);
            SilenceEvent {
                start_time: start,
                end_time: Some(end),
                duration_secs: Some(dur_secs),
            }
        };

        let silences = vec![
            make_silence(3588.06, 8.99),    // Midpoint: 3592.55s
            make_silence(7443.74, 8.22),    // Midpoint: 7447.85s
            make_silence(10767.85, 12.02),  // Midpoint: 10773.86s
            make_silence(14476.70, 8.23),   // Midpoint: 14480.82s
            make_silence(17976.82, 26.43),  // Midpoint: 17990.04s
            make_silence(21645.68, 37.40),  // Midpoint: 21664.38s
            make_silence(25222.93, 120.00), // Final silence start: 25222.93s
        ];

        let sample_rate = 96000;
        let boundaries = resolve_set_boundaries(
            &slots,
            &silences,
            &[],
            &[],
            event_start,
            None,
            sample_rate,
        );

        assert_eq!(boundaries.len(), 7);

        // Ground-truth manual boundaries found in the 2026-10-30 analysis (in seconds from event start):
        let manual_boundaries = [
            3592.50,  // Boundary 1: Next Horizon -> Nora B
            7447.80,  // Boundary 2: Nora B -> TAXMAN
            10773.86, // Boundary 3: TAXMAN -> Black Sun Empire
            14481.15, // Boundary 4: Black Sun Empire -> Merikan
            17989.67, // Boundary 5: Merikan -> Crater Crew 3nlight
            21664.29, // Boundary 6: Crater Crew 3nlight -> Aliienmia
            25222.93, // Boundary 7: Aliienmia end
        ];

        // Verify Boundary 1 (slot 1 start):
        let b1_secs = boundaries[1].sample_offset as f64 / sample_rate as f64;
        assert!(
            (b1_secs - manual_boundaries[0]).abs() < 1.0,
            "Boundary 1 diff: {:.3}s (got {:.3}s, expected {:.3}s)",
            (b1_secs - manual_boundaries[0]).abs(),
            b1_secs,
            manual_boundaries[0]
        );
        assert_eq!(boundaries[1].detection_method, BoundaryDetectionMethod::SilenceMidpoint);

        // Verify Boundary 2 (slot 2 start):
        let b2_secs = boundaries[2].sample_offset as f64 / sample_rate as f64;
        assert!(
            (b2_secs - manual_boundaries[1]).abs() < 1.0,
            "Boundary 2 diff: {:.3}s",
            (b2_secs - manual_boundaries[1]).abs()
        );
        assert_eq!(boundaries[2].detection_method, BoundaryDetectionMethod::SilenceMidpoint);

        // Verify Boundary 3 (slot 3 start):
        let b3_secs = boundaries[3].sample_offset as f64 / sample_rate as f64;
        assert!(
            (b3_secs - manual_boundaries[2]).abs() < 1.0,
            "Boundary 3 diff: {:.3}s",
            (b3_secs - manual_boundaries[2]).abs()
        );
        assert_eq!(boundaries[3].detection_method, BoundaryDetectionMethod::SilenceMidpoint);

        // Verify Boundary 4 (slot 4 start):
        let b4_secs = boundaries[4].sample_offset as f64 / sample_rate as f64;
        assert!(
            (b4_secs - manual_boundaries[3]).abs() < 1.0,
            "Boundary 4 diff: {:.3}s",
            (b4_secs - manual_boundaries[3]).abs()
        );
        assert_eq!(boundaries[4].detection_method, BoundaryDetectionMethod::SilenceMidpoint);

        // Verify Boundary 5 (slot 5 start):
        let b5_secs = boundaries[5].sample_offset as f64 / sample_rate as f64;
        assert!(
            (b5_secs - manual_boundaries[4]).abs() < 1.0,
            "Boundary 5 diff: {:.3}s",
            (b5_secs - manual_boundaries[4]).abs()
        );
        assert_eq!(boundaries[5].detection_method, BoundaryDetectionMethod::SilenceMidpoint);

        // Verify Boundary 6 (slot 6 start):
        let b6_secs = boundaries[6].sample_offset as f64 / sample_rate as f64;
        assert!(
            (b6_secs - manual_boundaries[5]).abs() < 1.0,
            "Boundary 6 diff: {:.3}s",
            (b6_secs - manual_boundaries[5]).abs()
        );
        assert_eq!(boundaries[6].detection_method, BoundaryDetectionMethod::SilenceMidpoint);

        // Verify Boundary 7 (slot 6 end / final audio end):
        let b7_secs = (boundaries[6].sample_offset + boundaries[6].duration_samples) as f64 / sample_rate as f64;
        assert!(
            (b7_secs - manual_boundaries[6]).abs() < 1.0,
            "Boundary 7 diff: {:.3}s",
            (b7_secs - manual_boundaries[6]).abs()
        );

        // Verify no boundary needs review when clear silences are found
        assert!(boundaries.iter().all(|b| !b.needs_review));
    }

    #[test]
    fn test_resolve_boundaries_with_usb_handover_and_review_fallback() {
        let timetable_raw = r#"
22:00 DJ Alpha
23:00 DJ Beta
00:00 DJ Gamma
"#;
        let event_date = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let tz = FixedOffset::east_opt(3600).unwrap();
        let slots = parse_timetable(timetable_raw, event_date, tz).unwrap();

        let event_start = DateTime::<FixedOffset>::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(2026, 11, 1)
                .unwrap()
                .and_hms_opt(22, 0, 0)
                .unwrap()
                - tz,
            tz,
        );

        // Transition 1 (Alpha -> Beta): No silence, but USB handover at 23:02:15
        let usb_handover = UsbHandover {
            timestamp: event_start + Duration::seconds(3735), // 23:02:15
            media_id: "USB_BETA_123".to_string(),
            media_name: "Beta Music Stick".to_string(),
            player_number: 2,
            track_title: Some("First Track".to_string()),
        };

        // Transition 2 (Beta -> Gamma): Neither silence nor USB handover (fallback to scheduled clock)

        let boundaries = resolve_set_boundaries(
            &slots,
            &[],
            &[usb_handover],
            &[],
            event_start,
            None,
            48000,
        );

        assert_eq!(boundaries.len(), 3);
        // DJ Beta used USB handover
        assert_eq!(boundaries[1].detection_method, BoundaryDetectionMethod::UsbHandover);
        assert_eq!(boundaries[1].usb_handover_media.as_deref(), Some("Beta Music Stick"));
        assert!(!boundaries[1].needs_review);

        // DJ Gamma fell back to scheduled clock
        assert_eq!(boundaries[2].detection_method, BoundaryDetectionMethod::ScheduledClock);
        assert!(boundaries[2].needs_review);
        assert!(boundaries[2].review_reason.as_ref().unwrap().contains("No silence gap"));
    }

    #[test]
    fn test_resolve_boundaries_dst_transition() {
        // Synthetic session spanning European Autumn DST: CEST (+02:00) -> CET (+01:00)
        // At 03:00+02:00 (= 01:00 UTC), clocks turn back to 02:00+01:00 (= 01:00 UTC)
        let tz_cest = FixedOffset::east_opt(7200).unwrap(); // +02:00
        let tz_cet = FixedOffset::east_opt(3600).unwrap();  // +01:00

        let event_date = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        let timetable_raw = r#"
23:00 Artist Pre-Midnight
01:00 Artist Early
02:00 Artist Transition Set
03:00 Artist Post-DST
"#;
        let slots = parse_timetable(timetable_raw, event_date, tz_cest).unwrap();
        assert_eq!(slots.len(), 4);

        let event_start = DateTime::<FixedOffset>::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(2026, 10, 25)
                .unwrap()
                .and_hms_opt(23, 0, 0)
                .unwrap()
                - tz_cest,
            tz_cest,
        );

        // Silences logged across the DST change:
        // Transition to Artist Early at 00:59:50+02:00
        let s1_start = DateTime::<FixedOffset>::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(2026, 10, 26).unwrap().and_hms_opt(0, 59, 50).unwrap() - tz_cest,
            tz_cest,
        );
        // Transition to Artist Transition Set at 01:59:50+02:00
        let s2_start = DateTime::<FixedOffset>::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(2026, 10, 26).unwrap().and_hms_opt(1, 59, 50).unwrap() - tz_cest,
            tz_cest,
        );
        // Transition to Artist Post-DST at 02:59:50+01:00 (offset changed to +01:00!)
        let s3_start = DateTime::<FixedOffset>::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(2026, 10, 26).unwrap().and_hms_opt(2, 59, 50).unwrap() - tz_cet,
            tz_cet,
        );
        // Final silence at 04:00:10+01:00
        let s4_start = DateTime::<FixedOffset>::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(2026, 10, 26).unwrap().and_hms_opt(4, 0, 10).unwrap() - tz_cet,
            tz_cet,
        );

        let silences = vec![
            SilenceEvent { start_time: s1_start, end_time: Some(s1_start + Duration::seconds(10)), duration_secs: Some(10.0) },
            SilenceEvent { start_time: s2_start, end_time: Some(s2_start + Duration::seconds(10)), duration_secs: Some(10.0) },
            SilenceEvent { start_time: s3_start, end_time: Some(s3_start + Duration::seconds(10)), duration_secs: Some(10.0) },
            SilenceEvent { start_time: s4_start, end_time: Some(s4_start + Duration::seconds(10)), duration_secs: Some(10.0) },
        ];

        let boundaries = resolve_set_boundaries(
            &slots,
            &silences,
            &[],
            &[],
            event_start,
            None,
            48000,
        );

        assert_eq!(boundaries.len(), 4);

        // Verify strictly monotonic UTC start times across the DST transition
        for i in 0..boundaries.len() - 1 {
            let t_curr = parse_log_timestamp(&boundaries[i].boundary_time_rfc3339).unwrap().0;
            let t_next = parse_log_timestamp(&boundaries[i + 1].boundary_time_rfc3339).unwrap().0;
            assert!(
                t_next > t_curr,
                "Boundary {} ({}) must be before boundary {} ({})",
                i, t_curr, i + 1, t_next
            );
            assert!(boundaries[i].duration_secs > 0.0);
            assert!(boundaries[i].sample_offset < boundaries[i + 1].sample_offset);
        }

        // The transition set spanned the repeated hour (02:00 CEST to 03:00 CET = 2 hours):
        // Duration of Artist Transition Set (slot 2) should be approximately 2 hours (7200s), NOT 1 hour!
        let transition_set_dur = boundaries[2].duration_secs;
        assert!(
            (transition_set_dur - 7200.0).abs() < 10.0,
            "Transition set spanning repeated DST hour should be ~7200s: got {:.1}s",
            transition_set_dur
        );
    }

    #[test]
    fn test_parse_loudnorm_astats_and_silencedetect() {
        let sample_output = r#"
[Parsed_astats_0 @ 0x7fa6480028c0] Overall
[Parsed_astats_0 @ 0x7fa6480028c0] DC offset: -0.000058
[Parsed_astats_0 @ 0x7fa6480028c0] Peak level dB: -6.608566
[Parsed_astats_0 @ 0x7fa6480028c0] RMS level dB: -20.649978
[Parsed_astats_0 @ 0x7fa6480028c0] Flat factor: 0.002500
[Parsed_astats_0 @ 0x7fa6480028c0] Number of samples: 960000
[Parsed_silencedetect_1 @ 0x7fa648002900] silence_start: 12.500
[Parsed_silencedetect_1 @ 0x7fa648002900] silence_end: 15.500 | silence_duration: 3.000
[Parsed_loudnorm_2 @ 0x7fa648002940] 
{
	"input_i" : "-18.36",
	"input_tp" : "-6.61",
	"input_lra" : "0.40",
	"input_thresh" : "-28.36",
	"output_i" : "-24.02",
	"output_tp" : "-12.15",
	"output_lra" : "0.40",
	"output_thresh" : "-34.02",
	"normalization_type" : "dynamic",
	"target_offset" : "0.02"
}
"#;

        let raw = parse_ffmpeg_analysis_output(sample_output).expect("parse output");
        assert_eq!(raw.integrated_lufs, -18.36);
        assert_eq!(raw.true_peak_dbtp, -6.61);
        assert_eq!(raw.loudness_range_lu, 0.40);
        assert_eq!(raw.threshold_lufs, -28.36);
        assert!((raw.sample_peak_dbfs - (-6.608566)).abs() < 0.001);
        assert!((raw.rms_dbfs - (-20.649978)).abs() < 0.001);
        assert!((raw.crest_factor_db - 14.041412).abs() < 0.01);
        assert!((raw.flat_factor - 0.0025).abs() < 0.0001);
        assert_eq!(raw.total_samples, 960000);

        assert_eq!(raw.detected_silences.len(), 1);
        assert_eq!(raw.detected_silences[0].start_secs, 12.5);
        assert_eq!(raw.detected_silences[0].end_secs, 15.5);
        assert_eq!(raw.detected_silences[0].duration_secs, 3.0);

        // Test quality metrics conversion
        let metrics = compute_quality_metrics(
            "Test Artist",
            0,
            &raw,
            10.0,
            0,
            -14.0,
        );
        assert_eq!(metrics.artist, "Test Artist");
        assert_eq!(metrics.clip_count, 0);
        assert_eq!(metrics.clip_percentage, 0.0);
        assert!((metrics.recommended_gain_db - 4.36).abs() < 0.01);
    }

    #[test]
    fn test_check_ffmpeg_ffprobe_available() {
        let res = check_ffmpeg_ffprobe_available();
        assert!(res.is_ok(), "ffmpeg and ffprobe must be available: {:?}", res);
        let (f_ver, p_ver) = res.unwrap();
        assert!(f_ver.contains("ffmpeg"));
        assert!(p_ver.contains("ffprobe"));
    }

    #[test]
    fn test_measure_audio_file_with_30s_fixture() {
        let temp_dir = std::env::temp_dir();
        let fixture_path = temp_dir.join("test_fixture_30s.flac");

        // Generate 30s synthetic audio fixture using ffmpeg: 440 Hz tone
        let gen_status = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=30",
                "-c:a",
                "flac",
                &fixture_path.to_string_lossy(),
            ])
            .output();

        if let Ok(out) = gen_status {
            if out.status.success() && fixture_path.exists() {
                let measurement = measure_audio_file(&fixture_path, None, None, 30)
                    .expect("measure 30s fixture");

                assert!(measurement.total_samples > 0);
                assert!(measurement.sample_peak_dbfs <= 0.0);
                assert!(measurement.true_peak_dbtp <= 1.0);
                assert!(measurement.integrated_lufs < 0.0);

                // Verify clean metrics on synthetic sine wave
                let clean_metrics = compute_quality_metrics(
                    "Clean Sine",
                    0,
                    &measurement,
                    30.0,
                    0,
                    -14.0,
                );
                assert_eq!(clean_metrics.quality_label, "Clean (No digital clipping)");
                assert!(!clean_metrics.clipping_unrepaired_warning);

                // Verify redlined metrics warning when clip_count is high
                let redline_metrics = compute_quality_metrics(
                    "Redlined Sine",
                    1,
                    &measurement,
                    30.0,
                    65535,
                    -14.0,
                );
                assert!(redline_metrics.quality_label.contains("Severely Clipped"));
                assert!(redline_metrics.clipping_unrepaired_warning);
                assert!(redline_metrics.clip_percentage > 0.0);

                let _ = std::fs::remove_file(&fixture_path);
            }
        }
    }

    #[test]
    fn test_check_recording_guardrail() {
        // Active recording: rejected immediately
        assert_eq!(
            check_recording_guardrail(RecordingState::Recording, None, 600),
            GuardrailCheckResult::ActiveRecording
        );

        // ArmedSilent for only 200s (quiet period 600s): rejected with 400s remaining
        assert_eq!(
            check_recording_guardrail(RecordingState::ArmedSilent, Some(200), 600),
            GuardrailCheckResult::QuietPeriodActive { remaining_secs: 400 }
        );

        // ArmedSilent for 601s (quiet period 600s): allowed
        assert_eq!(
            check_recording_guardrail(RecordingState::ArmedSilent, Some(601), 600),
            GuardrailCheckResult::Allowed
        );

        // Disconnected or Paused: allowed
        assert_eq!(
            check_recording_guardrail(RecordingState::Disconnected, None, 600),
            GuardrailCheckResult::Allowed
        );
        assert_eq!(
            check_recording_guardrail(RecordingState::Paused, None, 600),
            GuardrailCheckResult::Allowed
        );
    }

    #[test]
    fn test_disk_space_estimation_and_safety_margin() {
        let options = RenderOptions {
            preset: Preset::Streaming,
            raw_artist_flacs: true,
            normalized_artist_flacs: true,
            full_event_raw_flac: true,
            full_event_normalized_flac: true,
            cue_sheet: true,
            tracklist_txt: true,
            export_mp3: true,
            enable_declip: false,
            custom_artist_gains: HashMap::new(),
            include_ambient: false,
        };

        // 7-hour event at 96 kHz / 24-bit (~2.8 Mbps source bitrate)
        let total_duration = 7.0 * 3600.0;
        let bitrate_bps = 2_800_000;
        let estimated = estimate_render_output_bytes(total_duration, bitrate_bps, &options, 7);

        // 7 hours raw flac: ~8.8 GB
        // 4 flac streams + 1 mp3 stream: total estimated output should be ~35-45 GB
        assert!(
            estimated > 30 * 1024 * 1024 * 1024 && estimated < 50 * 1024 * 1024 * 1024,
            "7-hour estimate should be ~35-45 GB: got {} bytes",
            estimated
        );

        // Margin check: test in temp dir
        let temp_dir = std::env::temp_dir();
        let avail = get_available_disk_space(&temp_dir).unwrap();
        assert!(avail > 0);

        // If safety margin + required exceeds available, should fail
        let huge_estimate = avail; // Asking for all available space leaving 0
        let margin = 20 * 1024 * 1024 * 1024; // 20 GB safety margin
        let res = check_disk_space_margin(&temp_dir, huge_estimate, margin);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Insufficient disk space"));
    }

    #[test]
    fn test_job_runner_abort_and_cleanup_on_recording() {
        let temp_dir = std::env::temp_dir().join(format!("test_job_runner_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let partial_output = temp_dir.join("partial_render.flac");
        std::fs::write(&partial_output, b"partial audio data").unwrap();
        assert!(partial_output.exists());

        let mut job_state = JobState {
            id: "job_test_1".to_string(),
            session_id: "session_test".to_string(),
            status: JobStatus::Running,
            phase: "Rendering".to_string(),
            progress_percent: 50.0,
            current_step: 1,
            total_steps: 2,
            error_message: None,
            output_files: vec![partial_output.clone()],
            started_at_rfc3339: "2026-11-10T10:00:00Z".to_string(),
            updated_at_rfc3339: "2026-11-10T10:00:00Z".to_string(),
        };

        // Spawn a slow child process
        let child = std::process::Command::new("sleep")
            .arg("10")
            .spawn()
            .expect("spawn sleep");

        // Telemetry source transitions to Recording on second check
        let check_counter = std::sync::atomic::AtomicUsize::new(0);
        let telemetry_poll = || {
            let count = check_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count >= 2 {
                RecordingState::Recording
            } else {
                RecordingState::Disconnected
            }
        };

        let result = JobRunner::run_step_with_abort_watch(
            child,
            &[partial_output.clone()],
            telemetry_poll,
            &mut job_state,
            &temp_dir,
        );

        assert!(result.is_err());
        assert_eq!(job_state.status, JobStatus::Aborted);
        assert!(job_state.error_message.as_ref().unwrap().contains("Recording resumed"));

        // Partial output must have been deleted!
        assert!(!partial_output.exists(), "Partial output file should be deleted on abort");

        // Persisted state file must reflect Aborted
        let loaded = load_job_state(&temp_dir).unwrap().expect("load job state");
        assert_eq!(loaded.status, JobStatus::Aborted);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_normalization_render_and_full_event_concat() {
        let temp_dir = std::env::temp_dir().join(format!("test_render_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let fixture_src = temp_dir.join("source.flac");
        // Generate a 10s audio fixture at ~ -20 LUFS
        let gen_status = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=10",
                "-af",
                "volume=-10dB",
                "-c:a",
                "flac",
                &fixture_src.to_string_lossy(),
            ])
            .output();

        if let Ok(out) = gen_status {
            if out.status.success() && fixture_src.exists() {
                // Measure source first
                let raw_src = measure_audio_file(&fixture_src, None, None, 10).unwrap();
                let target_lufs = -14.0f32;
                let ceiling_dbtp = -1.0f32;
                let gain_needed = target_lufs - raw_src.integrated_lufs;

                let filter = build_normalization_filter(gain_needed, ceiling_dbtp, 48000, false);
                assert!(filter.contains("highpass=f=25"));
                assert!(filter.contains("alimiter="));
                assert!(filter.contains("aresample=192000"));

                // Render artist set 1 (first 5 seconds)
                let art1_flac = temp_dir.join("artist1_norm.flac");
                render_artist_set_flac(
                    &fixture_src,
                    &art1_flac,
                    0.0,
                    5.0,
                    &filter,
                    "Artist One",
                    "Test Event",
                    "2026-11-10",
                    1,
                ).unwrap();
                assert!(art1_flac.exists());

                // Measure rendered artist 1: verify target within 1.0 LU and ceiling met
                let art1_meas = measure_audio_file(&art1_flac, None, None, 10).unwrap();
                assert!(
                    (art1_meas.integrated_lufs - target_lufs).abs() <= 1.0,
                    "Rendered loudness ({:.2} LUFS) must be within 1.0 LU of target ({:.2} LUFS)",
                    art1_meas.integrated_lufs,
                    target_lufs
                );
                assert!(
                    art1_meas.true_peak_dbtp <= ceiling_dbtp + 0.1,
                    "Measured true peak ({:.2} dBTP) must not exceed ceiling ({:.2} dBTP)",
                    art1_meas.true_peak_dbtp,
                    ceiling_dbtp
                );

                // Render artist set 2 (second 5 seconds)
                let art2_flac = temp_dir.join("artist2_norm.flac");
                render_artist_set_flac(
                    &fixture_src,
                    &art2_flac,
                    5.0,
                    5.0,
                    &filter,
                    "Artist Two",
                    "Test Event",
                    "2026-11-10",
                    2,
                ).unwrap();
                assert!(art2_flac.exists());

                // Test MP3 export
                let art1_mp3 = temp_dir.join("artist1_norm.mp3");
                render_artist_set_mp3(
                    &art1_flac,
                    &art1_mp3,
                    "Artist One",
                    "Test Event",
                    "2026-11-10",
                    1,
                ).unwrap();
                assert!(art1_mp3.exists());
                assert!(std::fs::metadata(&art1_mp3).unwrap().len() > 1000);

                // Test full event concatenation of the two artist sets
                let full_concat = temp_dir.join("full_event_norm.flac");
                concatenate_audio_files(&[art1_flac, art2_flac], &full_concat).unwrap();
                assert!(full_concat.exists());

                let concat_meas = measure_audio_file(&full_concat, None, None, 15).unwrap();
                assert!(concat_meas.total_samples >= 470000 && concat_meas.total_samples <= 490000);

                let _ = std::fs::remove_dir_all(&temp_dir);
            }
        }
    }

    #[test]
    fn test_job_state_persistence_and_recovery() {
        let temp_dir = std::env::temp_dir().join(format!("test_job_recover_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let running_job = JobState {
            id: "job_recover_test".to_string(),
            session_id: "sess_1".to_string(),
            status: JobStatus::Running,
            phase: "Analyzing".to_string(),
            progress_percent: 25.0,
            current_step: 1,
            total_steps: 4,
            error_message: None,
            output_files: Vec::new(),
            started_at_rfc3339: "2026-11-10T10:00:00Z".to_string(),
            updated_at_rfc3339: "2026-11-10T10:00:00Z".to_string(),
        };

        save_job_state(&temp_dir, &running_job).expect("save state");

        // Recover on startup
        let recovered = recover_job_state(&temp_dir).expect("recover state").expect("some state");
        assert_eq!(recovered.status, JobStatus::Interrupted);
        assert!(recovered.error_message.as_ref().unwrap().contains("restarted"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_post_render_verification_flags_forced_miss() {
        let temp_dir = std::env::temp_dir().join(format!("test_verif_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let fixture_path = temp_dir.join("test_norm_audio.flac");
        // Generate an audio fixture
        let gen_status = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=1000:sample_rate=48000:duration=5",
                "-af",
                "volume=-11dB",
                "-c:a",
                "flac",
                &fixture_path.to_string_lossy(),
            ])
            .output();

        if let Ok(out) = gen_status {
            if out.status.success() && fixture_path.exists() {
                // Measure the actual loudness
                let actual = measure_audio_file(&fixture_path, None, None, 10).unwrap();

                // 1. Verification matching actual target: should NOT be flagged as missed
                let good_verif = verify_rendered_output(
                    &fixture_path,
                    true,
                    Some("Good Artist"),
                    actual.integrated_lufs,
                    -1.0,
                    10,
                ).expect("verify output");

                assert!(!good_verif.target_missed, "Target should NOT be flagged when met");
                assert!(good_verif.miss_reason.is_none());
                assert!(good_verif.lufs_difference_lu < 0.1);

                // 2. Forced miss on LUFS (target 5.0 LU lower than actual): MUST be flagged
                let missed_target_lufs = actual.integrated_lufs - 5.0;
                let bad_lufs_verif = verify_rendered_output(
                    &fixture_path,
                    true,
                    Some("Missed LUFS Artist"),
                    missed_target_lufs,
                    -1.0,
                    10,
                ).expect("verify output");

                assert!(bad_lufs_verif.target_missed, "Forced LUFS miss MUST be flagged");
                assert!(bad_lufs_verif.lufs_difference_lu > 1.0);
                assert!(bad_lufs_verif.miss_reason.as_ref().unwrap().contains("Loudness deviation"));

                // 3. Forced miss on true peak ceiling (ceiling way below actual true peak): MUST be flagged
                let bad_ceiling_dbtp = actual.true_peak_dbtp - 5.0;
                let bad_ceiling_verif = verify_rendered_output(
                    &fixture_path,
                    true,
                    Some("Missed Ceiling Artist"),
                    actual.integrated_lufs,
                    bad_ceiling_dbtp,
                    10,
                ).expect("verify output");

                assert!(bad_ceiling_verif.target_missed, "Forced true peak miss MUST be flagged");
                assert!(bad_ceiling_verif.miss_reason.as_ref().unwrap().contains("True peak exceeds ceiling"));

                let _ = std::fs::remove_dir_all(&temp_dir);
            }
        }
    }

    #[test]
    fn test_report_writers_and_snapshots() {
        let temp_dir = std::env::temp_dir().join(format!("test_reports_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let report = EventReport {
            event_id: "Event_2026-10-30".to_string(),
            generated_at_rfc3339: "2026-11-10T10:00:00+01:00".to_string(),
            mixer_model: "AlphaTheta DJM-A9".to_string(),
            sample_rate: 96000,
            total_samples: 2421401280,
            duration_formatted: "07:00:23".to_string(),
            continuity_audit: ContinuityAuditReport {
                is_continuous: true,
                checked_seams: 2,
                failed_seams: 0,
                seam_reports: vec![
                    SeamContinuityReport {
                        from_segment_index: 6,
                        to_segment_index: 7,
                        split_reason: "Max Segment Duration Rollover".to_string(),
                        duration_from_samples_secs: 10800.02,
                        interval_between_starts_secs: 10800.003,
                        discrepancy_ms: 17.0,
                        is_within_tolerance: true,
                    },
                ],
            },
            ambient_alignment: vec![
                AmbientAlignmentReport {
                    segment_index: 4,
                    ambient_file: PathBuf::from("pt04_ambient.flac"),
                    master_file: Some(PathBuf::from("pt04.flac")),
                    master_duration_secs: 2407.0,
                    ambient_duration_secs: 14392.7,
                    discrepancy_secs: 11985.7,
                    is_aligned: false,
                    note: Some("Ambient spans multiple segments".to_string()),
                }
            ],
            boundaries: vec![
                SetBoundary {
                    slot_index: 0,
                    artist: "Next Horizon".to_string(),
                    boundary_time_rfc3339: "2026-10-30T23:00:02.921+01:00".to_string(),
                    sample_offset: 0,
                    duration_samples: 344880000,
                    duration_secs: 3592.5,
                    detection_method: BoundaryDetectionMethod::ScheduledClock,
                    silence_duration_secs: None,
                    silence_midpoint_rfc3339: None,
                    usb_handover_media: None,
                    needs_review: false,
                    review_reason: None,
                },
                SetBoundary {
                    slot_index: 1,
                    artist: "Nora B b2b EML".to_string(),
                    boundary_time_rfc3339: "2026-10-30T23:59:55.420+01:00".to_string(),
                    sample_offset: 344880000,
                    duration_samples: 370108800,
                    duration_secs: 3855.3,
                    detection_method: BoundaryDetectionMethod::SilenceMidpoint,
                    silence_duration_secs: Some(8.99),
                    silence_midpoint_rfc3339: Some("2026-10-30T23:59:55.420+01:00".to_string()),
                    usb_handover_media: None,
                    needs_review: false,
                    review_reason: None,
                },
            ],
            pre_render_metrics: vec![
                SetQualityMetrics {
                    artist: "Next Horizon".to_string(),
                    slot_index: 0,
                    duration_secs: 3592.5,
                    sample_peak_dbfs: -0.5,
                    true_peak_dbtp: 0.1,
                    rms_dbfs: -18.2,
                    integrated_lufs: -14.2,
                    loudness_range_lu: 6.5,
                    crest_factor_db: 17.7,
                    flat_factor: 0.0,
                    clip_count: 0,
                    clip_percentage: 0.0,
                    recommended_gain_db: -0.2,
                    quality_label: "Clean (No digital clipping)".to_string(),
                    clipping_unrepaired_warning: false,
                },
                SetQualityMetrics {
                    artist: "Redline Master".to_string(),
                    slot_index: 1,
                    duration_secs: 3600.0,
                    sample_peak_dbfs: 0.0,
                    true_peak_dbtp: 2.5,
                    rms_dbfs: -9.5,
                    integrated_lufs: -8.5,
                    loudness_range_lu: 3.2,
                    crest_factor_db: 9.5,
                    flat_factor: 0.05,
                    clip_count: 65535,
                    clip_percentage: 0.0189,
                    recommended_gain_db: -5.5,
                    quality_label: "Severely Clipped / Redlined".to_string(),
                    clipping_unrepaired_warning: true,
                },
            ],
            post_render_verification: vec![
                MeasuredVerificationMetrics {
                    file_name: "01_Next_Horizon_norm.flac".to_string(),
                    file_path: PathBuf::from("01_Next_Horizon_norm.flac"),
                    is_artist_set: true,
                    artist_name: Some("Next Horizon".to_string()),
                    measured_integrated_lufs: -14.05,
                    target_integrated_lufs: -14.0,
                    lufs_difference_lu: 0.05,
                    measured_true_peak_dbtp: -1.02,
                    ceiling_true_peak_dbtp: -1.0,
                    measured_sample_peak_dbfs: -1.05,
                    target_missed: false,
                    miss_reason: None,
                }
            ],
            tracklist_summary: vec![
                TracklistSummaryEntry {
                    artist: "Next Horizon".to_string(),
                    usb_media_name: Some("NH_STICK_1".to_string()),
                    track_count: 2,
                    tracks: vec!["Artist A - Track One".to_string(), "Artist B - Track Two".to_string()],
                }
            ],
            general_notes: vec!["Recorded on main stage".to_string()],
        };

        // 1. Markdown test
        let md = generate_event_report_markdown(&report);
        assert!(md.contains("# Event Audio Quality & Mastering Report: Event_2026-10-30"));
        assert!(md.contains("AlphaTheta DJM-A9"));
        assert!(md.contains("17.0 ms"));
        assert!(md.contains("pt04_ambient.flac"));
        assert!(md.contains("Next Horizon"));
        assert!(md.contains("Redline Master"));
        // Clipping disclaimer must be present for redlined sets!
        assert!(md.contains("Loudness normalization reduces overall signal level and prevents DAC overshoots, but flat-topped clipping distortion baked into the source recording cannot be fully undone"));

        // 2. HTML test
        let html = generate_event_report_html(&report);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Event Audio Quality & Mastering Report"));
        assert!(html.contains("warning-box"));
        assert!(html.contains("Note on Clipping Distortion:"));

        // 3. JSON test (must roundtrip)
        let json_str = generate_event_report_json(&report).unwrap();
        let rt_report: EventReport = serde_json::from_str(&json_str).unwrap();
        assert_eq!(rt_report.event_id, report.event_id);
        assert_eq!(rt_report.boundaries.len(), report.boundaries.len());
        assert_eq!(rt_report.pre_render_metrics.len(), report.pre_render_metrics.len());

        // 4. CUE sheet test
        let cue = generate_cue_sheet(&report, "Event_2026-10-30.flac");
        assert!(cue.contains("FILE \"Event_2026-10-30.flac\" WAVE"));
        assert!(cue.contains("TRACK 01 AUDIO"));
        assert!(cue.contains("TITLE \"Next Horizon - Live Set\""));
        assert!(cue.contains("INDEX 01 00:00:00"));
        assert!(cue.contains("TRACK 02 AUDIO"));
        assert!(cue.contains("TITLE \"Nora B b2b EML - Live Set\""));

        // 5. Tracklist txt test
        let txt = generate_tracklist_txt(&report);
        assert!(txt.contains("Event: Event_2026-10-30"));
        assert!(txt.contains("01. [00:00:00] Next Horizon"));
        assert!(txt.contains("02. [00:59:53] Nora B b2b EML"));
        assert!(txt.contains("Artist A - Track One"));

        // 6. Write reports to disk
        let written = write_event_reports(&report, &temp_dir, Some("Event_2026-10-30.flac")).unwrap();
        assert_eq!(written.len(), 5);
        for p in &written {
            assert!(p.exists());
            assert!(std::fs::metadata(p).unwrap().len() > 0);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
