use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Duration};
use djm_common::config::AppConfig;
use djm_common::protocol::default_socket_path;
use djm_common::studio::{
    audit_ambient_alignment, build_normalization_filter, check_disk_space_margin,
    check_ffmpeg_ffprobe_available, check_recording_guardrail, compute_quality_metrics,
    concatenate_audio_files, estimate_render_output_bytes, get_available_disk_space,
    group_event_sessions, measure_audio_file, parse_session_log, parse_timetable,
    prepare_session_audio_source, render_artist_set_flac, render_artist_set_mp3,
    resolve_set_boundaries, seconds_to_hms, verify_all_rendered_outputs, write_event_reports,
    ArtistSlot, BoundaryDetectionMethod, EventReport, GuardrailCheckResult, Preset, RenderOptions,
};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;

#[derive(Debug, Clone)]
pub struct ProcessArgs {
    pub session: Option<String>,
    pub timetable: Option<PathBuf>,
    pub preset: String,
    pub output_dir: Option<PathBuf>,
    pub recordings_dir: Option<PathBuf>,
    pub target_lufs: Option<f32>,
    pub ceiling_dbtp: Option<f32>,
    pub declip: bool,
    pub mp3: bool,
    pub full_event: bool,
    pub force: bool,
    pub dry_run: bool,
}

pub async fn run_studio_process(args: ProcessArgs) -> Result<()> {
    println!("\n=== djm-box Post-Event Mastering Studio ===");

    // 1. Tooling check
    let (ffmpeg_ver, ffprobe_ver) = check_ffmpeg_ffprobe_available()
        .map_err(|e| anyhow!("Audio tooling dependency missing: {}\nPlease run 'djm-box setup' or install ffmpeg/ffprobe.", e))?;
    println!("Tooling: ffmpeg {}, ffprobe {}", ffmpeg_ver, ffprobe_ver);

    // 2. Guardrail check: ensure recorder is not running unless force
    if !args.force {
        let sock = default_socket_path();
        if let Ok(stream) = UnixStream::connect(&sock).await {
            let mut lines = BufReader::new(stream).lines();
            if let Ok(Some(line)) = lines.next_line().await {
                if let Ok(telemetry) = serde_json::from_str::<djm_common::protocol::Telemetry>(&line) {
                    match check_recording_guardrail(telemetry.state, None, 600) {
                        GuardrailCheckResult::ActiveRecording => {
                            bail!("Recording guardrail prevented processing: Recorder is currently actively recording audio. Use --force to override.");
                        }
                        GuardrailCheckResult::QuietPeriodActive { remaining_secs } => {
                            bail!("Recording guardrail prevented processing: Recorder entered ArmedSilent recently. Quiet period active ({}s remaining). Use --force to override.", remaining_secs);
                        }
                        GuardrailCheckResult::Allowed => {}
                    }
                }
            }
        }
    }

    // 3. Discover recordings directory
    let rec_dir = args.recordings_dir.clone().unwrap_or_else(|| {
        let cfg = AppConfig::load_or_create(&AppConfig::default_config_path()).unwrap_or_default();
        let dir_str = cfg.storage.output_dir;
        if let Some(stripped) = dir_str.strip_prefix("~/") {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home).join(stripped);
            }
        }
        PathBuf::from(dir_str)
    });

    let search_dir = if let Some(ref target) = args.session {
        let target_path = Path::new(target);
        if target_path.is_dir() {
            target_path.to_path_buf()
        } else if let Some(parent) = target_path.parent() {
            if parent.is_dir() && parent != Path::new("") {
                parent.to_path_buf()
            } else {
                rec_dir.clone()
            }
        } else {
            rec_dir.clone()
        }
    } else {
        rec_dir.clone()
    };

    println!("Scanning recordings directory: {}", search_dir.display());
    let mut sessions = group_event_sessions(&search_dir).unwrap_or_default();
    if sessions.is_empty() && search_dir != rec_dir {
        sessions = group_event_sessions(&rec_dir).unwrap_or_default();
    }

    if sessions.is_empty() {
        bail!("No valid audio recording sessions (.flac + .log) found in '{}'", search_dir.display());
    }

    // Match session
    let session = if let Some(ref target) = args.session {
        sessions
            .into_iter()
            .find(|s| {
                s.id == *target
                    || s.id.contains(target)
                    || target.contains(&s.id)
                    || s.segments.iter().any(|seg| seg.flac_path.to_string_lossy().contains(target))
            })
            .ok_or_else(|| anyhow!("Target session '{}' not found in discovered sessions", target))?
    } else {
        sessions.pop().unwrap()
    };

    println!("Selected session: {}", session.id);
    println!("  Start time:     {}", session.start_time_rfc3339);
    println!("  Total duration: {} ({:.1}s)", seconds_to_hms(session.total_duration_secs), session.total_duration_secs);
    println!("  Main segments:  {}", session.segments.len());
    println!("  Warmup segments:{}", session.warmup_segments.len());
    println!("  Continuity:     {}", if session.continuity_audit.is_continuous { "✓ Verified continuous" } else { "⚠ Discontinuity detected" });

    // 4. Timetable parsing
    let timetable_content = if let Some(ref tt_path) = args.timetable {
        std::fs::read_to_string(tt_path).with_context(|| format!("Failed to read timetable file '{}'", tt_path.display()))?
    } else {
        let candidate_in_search = search_dir.join("timetable.txt");
        let candidate_in_rec = rec_dir.join("timetable.txt");
        if candidate_in_search.is_file() {
            std::fs::read_to_string(&candidate_in_search).unwrap_or_default()
        } else if candidate_in_rec.is_file() {
            std::fs::read_to_string(&candidate_in_rec).unwrap_or_default()
        } else {
            String::new()
        }
    };

    let start_dt = DateTime::parse_from_rfc3339(&session.start_time_rfc3339)
        .unwrap_or_else(|_| DateTime::from_naive_utc_and_offset(
            chrono::NaiveDate::from_ymd_opt(2026, 10, 30).unwrap().and_hms_opt(0, 0, 0).unwrap(),
            chrono::FixedOffset::east_opt(0).unwrap(),
        ));
    let event_date = start_dt.date_naive();
    let tz_offset = *start_dt.offset();

    let timetable_slots = if !timetable_content.trim().is_empty() {
        println!("Loaded timetable ({} lines)", timetable_content.lines().count());
        parse_timetable(&timetable_content, event_date, tz_offset)
            .map_err(|e| anyhow!("Failed to parse timetable: {}", e))?
    } else {
        println!("No timetable provided; creating single slot for event '{}'", session.id);
        vec![ArtistSlot {
            slot_index: 0,
            artist: session.id.clone(),
            scheduled_start_clock: "00:00".to_string(),
            scheduled_end_clock: None,
            target_date: event_date.to_string(),
            scheduled_start_rfc3339: session.start_time_rfc3339.clone(),
            scheduled_end_rfc3339: session.end_time_rfc3339.clone(),
            assigned_usb_cluster: None,
            assigned_usb_media: None,
        }]
    };

    // 5. Boundary resolution
    let mut all_silences = Vec::new();
    let mut all_bookmarks = Vec::new();
    for seg in &session.segments {
        if let Some(ref lp) = seg.log_path {
            if let Ok(parsed) = parse_session_log(lp) {
                all_silences.extend(parsed.silence_events);
                all_bookmarks.extend(parsed.bookmarks);
            }
        }
    }
    let event_end_dt = session.end_time_rfc3339.as_deref().and_then(|s| DateTime::parse_from_rfc3339(s).ok());

    let boundaries = resolve_set_boundaries(
        &timetable_slots,
        &all_silences,
        &[],
        &all_bookmarks,
        start_dt,
        event_end_dt,
        session.sample_rate,
    );

    println!("\nResolved {} artist set boundaries:", boundaries.len());
    println!("{:<4} {:<24} {:<30} {:<10} {:<18} Status", "Slot", "Artist", "Start Time", "Duration", "Method");
    println!("{}", "-".repeat(100));
    for (i, b) in boundaries.iter().enumerate() {
        let method = match b.detection_method {
            BoundaryDetectionMethod::SilenceMidpoint => "Silence Midpoint",
            BoundaryDetectionMethod::UsbHandover => "USB Handover",
            BoundaryDetectionMethod::ScheduledClock => "Scheduled Clock",
            BoundaryDetectionMethod::ManualAdjust => "Manual Adjust",
        };
        let status = if b.needs_review { "⚠ Review" } else { "✓ Clean" };
        println!(
            "{:<4} {:<24} {:<30} {:<10} {:<18} {}",
            i + 1,
            if b.artist.len() > 23 { format!("{}...", &b.artist[..20]) } else { b.artist.clone() },
            b.boundary_time_rfc3339,
            seconds_to_hms(b.duration_secs),
            method,
            status
        );
    }

    // 6. Target preset & render options
    let base_preset = match args.preset.to_lowercase().as_str() {
        "club" => Preset::Club,
        "streaming" => Preset::Streaming,
        _ => Preset::Streaming,
    };

    let final_preset = if args.target_lufs.is_some() || args.ceiling_dbtp.is_some() {
        Preset::Custom {
            target_lufs: args.target_lufs.unwrap_or_else(|| base_preset.target_lufs()),
            ceiling_dbtp: args.ceiling_dbtp.unwrap_or_else(|| base_preset.ceiling_dbtp()),
        }
    } else {
        base_preset
    };

    println!("\nMastering Target: {:.1} LUFS | True Peak Ceiling: {:.1} dBTP | De-clip: {}",
        final_preset.target_lufs(),
        final_preset.ceiling_dbtp(),
        if args.declip { "Enabled" } else { "Off" }
    );

    let out_dir = args.output_dir.unwrap_or_else(|| {
        search_dir.join(format!("{}_processed", session.id))
    });
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("Failed to create output directory '{}'", out_dir.display()))?;

    let render_opts = RenderOptions {
        preset: final_preset.clone(),
        raw_artist_flacs: true,
        normalized_artist_flacs: true,
        full_event_raw_flac: args.full_event,
        full_event_normalized_flac: args.full_event,
        cue_sheet: true,
        tracklist_txt: true,
        export_mp3: args.mp3,
        enable_declip: args.declip,
        custom_artist_gains: HashMap::new(),
        include_ambient: false,
    };

    let source_bitrate = (session.sample_rate as u64) * 24 * 2;
    let est_bytes = estimate_render_output_bytes(session.total_duration_secs, source_bitrate, &render_opts, boundaries.len());
    let free_bytes = get_available_disk_space(&out_dir).unwrap_or(0);
    println!("Estimated output: {:.1} MB | Available disk space: {:.2} GB",
        est_bytes as f64 / 1_000_000.0,
        free_bytes as f64 / 1_000_000_000.0
    );

    if !args.force {
        if let Err(disk_err) = check_disk_space_margin(&out_dir, est_bytes, 20 * 1024 * 1024 * 1024) {
            bail!("Disk space check failed: {}\nUse --force to override.", disk_err);
        }
    }

    // 7. Prepare audio source reference
    let audio_source = prepare_session_audio_source(&session, &out_dir)
        .map_err(|e| anyhow!("Failed to prepare audio source for session: {}", e))?;

    // 8. Step [1/5]: Audio Quality Measurement Pass
    println!("\n[1/5] Analyzing audio quality across artist sets...");
    let mut pre_render_metrics = Vec::new();

    for (idx, b) in boundaries.iter().enumerate() {
        print!("  [{}/{}] Measuring '{}' ({:.1}s)... ", idx + 1, boundaries.len(), b.artist, b.duration_secs);
        std::io::stdout().flush().ok();

        let start_s = b.sample_offset as f64 / session.sample_rate as f64;
        let dur_s = b.duration_secs;
        let raw = measure_audio_file(&audio_source, Some(start_s), Some(dur_s), 300)
            .map_err(|e| anyhow!("Measurement failed for '{}': {}", b.artist, e))?;

        // Extract logged digital clips within this set's time interval
        let mut slot_clips = 0u64;
        if let Ok(b_start) = DateTime::parse_from_rfc3339(&b.boundary_time_rfc3339) {
            let b_end = b_start + Duration::milliseconds((b.duration_secs * 1000.0) as i64);
            for seg in &session.segments {
                if let Ok(s_start) = DateTime::parse_from_rfc3339(&seg.start_time_rfc3339) {
                    let s_end = s_start + Duration::milliseconds((seg.duration_secs * 1000.0) as i64);
                    if b_start < s_end && b_end > s_start {
                        slot_clips += seg.total_clips;
                    }
                }
            }
        }

        let m = compute_quality_metrics(
            &b.artist,
            b.slot_index,
            &raw,
            dur_s,
            slot_clips,
            final_preset.target_lufs(),
        );

        println!("Done.");
        println!("    LUFS: {:>6.2} | TP: {:>5.2} dBTP | Peak: {:>5.2} dBFS | Flat: {:.5} | Clips: {:>5} | Gain: {:>+5.2} dB",
            m.integrated_lufs, m.true_peak_dbtp, m.sample_peak_dbfs, m.flat_factor, m.clip_count, m.recommended_gain_db
        );

        pre_render_metrics.push(m);
    }

    if args.dry_run {
        println!("\n[DRY RUN] Analysis complete. No audio files written.");
        return Ok(());
    }

    // 9. Step [2/5]: Render Normalized Artist FLACs
    println!("\n[2/5] Rendering normalized 24-bit FLAC artist sets...");
    let mut rendered_flacs = Vec::new();
    let event_date_str = session.start_time_rfc3339.split('T').next().unwrap_or("2026-10-30");

    for (idx, b) in boundaries.iter().enumerate() {
        let m = &pre_render_metrics[idx];
        let filter = build_normalization_filter(
            m.recommended_gain_db,
            final_preset.ceiling_dbtp(),
            session.sample_rate,
            args.declip,
        );

        let sanitized_artist = b.artist.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        let flac_name = format!("{:02}_{}.flac", idx + 1, sanitized_artist);
        let out_flac = out_dir.join(&flac_name);

        print!("  [{}/{}] Rendering '{}' -> '{}'... ", idx + 1, boundaries.len(), b.artist, flac_name);
        std::io::stdout().flush().ok();

        let start_s = b.sample_offset as f64 / session.sample_rate as f64;
        render_artist_set_flac(
            &audio_source,
            &out_flac,
            start_s,
            b.duration_secs,
            &filter,
            &b.artist,
            &session.id,
            event_date_str,
            idx + 1,
        ).map_err(|e| anyhow!("Failed to render artist set '{}': {}", b.artist, e))?;

        println!("Done.");
        rendered_flacs.push(out_flac);
    }

    // 10. Step [3/5]: Export MP3s
    let mut rendered_mp3s = Vec::new();
    if args.mp3 {
        println!("\n[3/5] Rendering 320 kbps MP3 files...");
        for (idx, flac_path) in rendered_flacs.iter().enumerate() {
            let b = &boundaries[idx];
            let mp3_name = flac_path.file_stem().unwrap().to_string_lossy().to_string() + ".mp3";
            let out_mp3 = out_dir.join(&mp3_name);

            print!("  [{}/{}] Exporting MP3 '{}'... ", idx + 1, rendered_flacs.len(), mp3_name);
            std::io::stdout().flush().ok();

            render_artist_set_mp3(
                flac_path,
                &out_mp3,
                &b.artist,
                &session.id,
                event_date_str,
                idx + 1,
            ).map_err(|e| anyhow!("Failed to export MP3 for '{}': {}", b.artist, e))?;

            println!("Done.");
            rendered_mp3s.push(out_mp3);
        }
    } else {
        println!("\n[3/5] Skipping MP3 exports (--mp3 not set)");
    }

    // 11. Step [4/5]: Concatenate Full Event
    if args.full_event && rendered_flacs.len() > 1 {
        println!("\n[4/5] Concatenating seamless full-event master FLAC...");
        let full_flac = out_dir.join(format!("{}_Full_Mastered.flac", session.id));
        concatenate_audio_files(&rendered_flacs, &full_flac)
            .map_err(|e| anyhow!("Full event concatenation failed: {}", e))?;
        println!("Full-event master: {}", full_flac.display());
    } else {
        println!("\n[4/5] Skipping full-event concatenation");
    }

    // 12. Step [5/5]: Verify Rendered Outputs
    println!("\n[5/5] Verifying rendered outputs against targets...");
    let files_to_verify: Vec<(PathBuf, bool, Option<String>)> = rendered_flacs
        .iter()
        .enumerate()
        .map(|(idx, path)| (path.clone(), true, Some(boundaries[idx].artist.clone())))
        .collect();

    let verifications = verify_all_rendered_outputs(
        &files_to_verify,
        final_preset.target_lufs(),
        final_preset.ceiling_dbtp(),
        300,
    ).map_err(|e| anyhow!("Post-render verification failed: {}", e))?;

    for v in &verifications {
        let status_str = if v.target_missed {
            format!("\x1b[31;1mMISSED: {}\x1b[0m", v.miss_reason.as_deref().unwrap_or("out of spec"))
        } else {
            "\x1b[32;1mPASS\x1b[0m".to_string()
        };
        println!("  - {}: Measured {:.2} LUFS (diff {:+.2} LU), TP {:.2} dBTP [{}]",
            v.file_name, v.measured_integrated_lufs, v.lufs_difference_lu, v.measured_true_peak_dbtp, status_str
        );
    }

    // 13. Reports and Tracklists
    let ambient_alignment = audit_ambient_alignment(&session.segments);
    let report = EventReport {
        event_id: session.id.clone(),
        generated_at_rfc3339: chrono::Utc::now().to_rfc3339(),
        mixer_model: "AlphaTheta / Pioneer DJ".to_string(),
        sample_rate: session.sample_rate,
        total_samples: session.total_samples,
        duration_formatted: seconds_to_hms(session.total_duration_secs),
        continuity_audit: session.continuity_audit.clone(),
        ambient_alignment,
        boundaries: boundaries.clone(),
        pre_render_metrics: pre_render_metrics.clone(),
        post_render_verification: verifications,
        tracklist_summary: Vec::new(),
        general_notes: vec![
            format!("Mastered using preset: {:?}", final_preset),
            format!("Target: {:.1} LUFS, Ceiling: {:.1} dBTP", final_preset.target_lufs(), final_preset.ceiling_dbtp()),
        ],
    };

    let cue_audio = if args.full_event {
        Some(format!("{}_Full_Mastered.flac", session.id))
    } else {
        rendered_flacs.first().and_then(|p| p.file_name().and_then(|f| f.to_str())).map(|s| s.to_string())
    };

    write_event_reports(&report, &out_dir, cue_audio.as_deref())
        .map_err(|e| anyhow!("Failed to write reports: {}", e))?;

    println!("\n=== Studio Mastering Complete ===");
    println!("Output directory:   {}", out_dir.display());
    println!("Reports generated:");
    println!("  - HTML Report:    {}", out_dir.join("Event_Report.html").display());
    println!("  - Markdown:       {}", out_dir.join("Event_Report.md").display());
    println!("  - JSON Data:      {}", out_dir.join("Event_Report.json").display());
    println!("  - Tracklist TXT:  {}", out_dir.join("tracklist.txt").display());
    if render_opts.cue_sheet {
        println!("  - CUE Sheet:      {}", out_dir.join("Event.cue").display());
    }

    Ok(())
}
