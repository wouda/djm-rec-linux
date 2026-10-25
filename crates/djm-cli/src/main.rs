pub mod setup;

use anyhow::Result;
use clap::{Parser, Subcommand};
use djm_common::config::AppConfig;
use djm_common::protocol::{default_socket_path, ControlCommand, PairingInfo, RecordingState, Telemetry};
use setup::ApplianceSetup;
use std::io::Write;
use std::process::Command;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(name = "djm-box")]
#[command(version)]
#[command(about = "Fault-Isolated DJ Mixer Recording Appliance for CachyOS / Arch Linux")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run initial setup and installation for CachyOS (udev, systemd, WirePlumber, dependencies)
    Setup {
        /// Only print what would be done without writing files
        #[arg(long)]
        dry_run: bool,
    },
    /// Launch the core real-time audio capture engine
    Daemon {
        /// Run with simulated audio tone for testing without physical mixers
        #[arg(long)]
        mock: bool,
    },
    /// Launch the stateless Ratatui terminal dashboard
    Tui,
    /// Launch the isolated BLE GATT and Web Remote Bridge service
    Remote,
    /// Show phone pairing PIN and QR code in terminal
    Pair,
    /// Send an immediate file split command to the recording daemon
    Split,
    /// Drop a bookmark note into the active session log
    Bookmark {
        /// Optional note text
        note: Option<String>,
    },
    /// Query recording status, hardware pad, and incoming audio dBFS levels
    Status {
        /// Continuously stream live audio VU meters and gain in terminal
        #[arg(short, long)]
        watch: bool,
    },
    /// Continuously stream live audio VU meters and gain in terminal
    Monitor,
}

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    let cli = Cli::parse();

    match cli.command {
        Commands::Setup { dry_run } => {
            ApplianceSetup::run_setup(dry_run)?;
        }
        Commands::Daemon { mock } => {
            let bin = find_binary("djm-daemon");
            let mut cmd = Command::new(&bin);
            if mock {
                cmd.arg("--mock");
            }
            let status = cmd.status().map_err(|e| anyhow::anyhow!("Failed to start '{}': {}", bin.display(), e))?;
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Commands::Tui => {
            let bin = find_binary("djm-tui");
            let mut cmd = Command::new(&bin);
            let status = cmd.status().map_err(|e| anyhow::anyhow!("Failed to start '{}': {}.\nPlease build with 'cargo build' or 'cargo build --release'.", bin.display(), e))?;
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Commands::Remote => {
            let bin = find_binary("djm-remote");
            let mut cmd = Command::new(&bin);
            let status = cmd.status().map_err(|e| anyhow::anyhow!("Failed to start '{}': {}", bin.display(), e))?;
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Commands::Pair => {
            let config_path = AppConfig::default_config_path();
            let config = AppConfig::load_or_create(&config_path)?;
            let info = PairingInfo::auto_detect(
                config.remote.ble_device_name,
                config.remote.auth_token,
                if !config.remote.remote_public_url.is_empty() {
                    Some(config.remote.remote_public_url)
                } else {
                    None
                },
                8080,
            );

            let web_url = info.to_web_url();
            println!("\n=== djm-box Companion Phone Pairing ===");
            println!("Direct Wi-Fi URL: {}", web_url);
            if let Some(ref mdns) = info.mdns_hostname {
                println!("mDNS Hostname:    http://{}:{}/?token={}", mdns, info.port, info.token);
            }
            if let Some(ref ip) = info.local_ip {
                println!("Detected LAN IP:  {}", ip);
            }
            println!("Auth PIN:         {}", info.token);
            println!("BLE Device:       {}", info.name);
            println!("BLE UUID:         {}\n", info.ble_service_uuid);
            println!("Tip: Open 'djm-box tui' and press [P] to scan the pairing QR code with your phone camera.");
        }
        Commands::Split => {
            send_socket_command(ControlCommand::SplitFile).await?;
            println!("Split command sent to djm-daemon.");
        }
        Commands::Bookmark { note } => {
            send_socket_command(ControlCommand::DropBookmark(note)).await?;
            println!("Bookmark sent to djm-daemon.");
        }
        Commands::Status { watch } => {
            if watch {
                stream_live_monitor().await?;
            } else {
                let status = query_socket_status().await?;
                print_telemetry_snapshot(&status);
            }
        }
        Commands::Monitor => {
            stream_live_monitor().await?;
        }
    }

    Ok(())
}

fn find_binary(name: &str) -> std::path::PathBuf {
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            let candidate = dir.join(name);
            if candidate.exists() {
                return candidate;
            }
            if let Ok(canon) = dir.canonicalize() {
                let release_candidate = canon.join("../release").join(name);
                if release_candidate.exists() {
                    return release_candidate;
                }
                let debug_candidate = canon.join("../debug").join(name);
                if debug_candidate.exists() {
                    return debug_candidate;
                }
            }
        }
    }
    std::path::PathBuf::from(name)
}

async fn send_socket_command(cmd: ControlCommand) -> Result<()> {
    let socket_path = default_socket_path();
    let mut stream = UnixStream::connect(&socket_path).await?;
    let json = serde_json::to_string(&cmd)?;
    stream.write_all(format!("{}\n", json).as_bytes()).await?;
    Ok(())
}

async fn query_socket_status() -> Result<Telemetry> {
    let socket_path = default_socket_path();
    let stream = match UnixStream::connect(&socket_path).await {
        Ok(s) => s,
        Err(e) => {
            anyhow::bail!(
                "Could not connect to djm-daemon at {}: {}.\nIs djm-daemon running? Start it with 'djm-box daemon' or 'systemctl --user start djm-box-daemon'",
                socket_path.display(),
                e
            );
        }
    };
    let mut lines = BufReader::new(stream).lines();
    if let Some(line) = lines.next_line().await? {
        let t: Telemetry = serde_json::from_str(&line)?;
        return Ok(t);
    }
    anyhow::bail!("No response from djm-daemon socket")
}

fn format_vu_bar(cdb: i16, width: usize) -> String {
    let db = (cdb as f32 / 100.0).clamp(-60.0, 0.0);
    let ratio = ((db - (-60.0)) / 60.0).clamp(0.0, 1.0);
    let filled = (ratio * width as f32).round() as usize;

    // -19 dBFS corresponds to: (-19 - (-60)) / 60 = 41 / 60 ≈ 0.6833
    let pad_ref_idx = ((41.0 / 60.0) * width as f32).round() as usize;
    // -3 dBFS corresponds to: (-3 - (-60)) / 60 = 57 / 60 = 0.95
    let danger_idx = ((57.0 / 60.0) * width as f32).round() as usize;

    let mut out = String::with_capacity(width * 10);
    for i in 0..width {
        if i < filled {
            if i < pad_ref_idx {
                out.push_str("\x1b[32m█\x1b[0m"); // Green (safe / standard)
            } else if i < danger_idx {
                out.push_str("\x1b[33m█\x1b[0m"); // Yellow (mixer headroom above physical 0 dB)
            } else {
                out.push_str("\x1b[31m█\x1b[0m"); // Red (clipping danger!)
            }
        } else if i == pad_ref_idx {
            out.push_str("\x1b[33m│\x1b[0m"); // 0 dB reference tick mark
        } else {
            out.push_str("\x1b[90m░\x1b[0m"); // Empty tick
        }
    }
    out
}

fn print_telemetry_snapshot(t: &Telemetry) {
    let peak_l = t.peak_l_centi_db as f32 / 100.0;
    let peak_r = t.peak_r_centi_db as f32 / 100.0;
    let rms_l = t.rms_l_centi_db as f32 / 100.0;
    let rms_r = t.rms_r_centi_db as f32 / 100.0;

    let bar_width = 30;
    let bar_l = format_vu_bar(t.peak_l_centi_db, bar_width);
    let bar_r = format_vu_bar(t.peak_r_centi_db, bar_width);

    let max_peak = peak_l.max(peak_r);
    let headroom_to_clip = (-max_peak).max(0.0);
    let over_mixer_zero = max_peak - (-19.0);

    let mixer_detected = t.is_mixer_connected();
    let mixer_detection_str = if mixer_detected {
        format!("\x1b[32;1m✓ DETECTED\x1b[0m ({} @ {} Hz)", t.mixer_model, t.sample_rate)
    } else {
        "\x1b[31;1m✗ NOT DETECTED\x1b[0m (Connect DJM mixer via USB-B cable)".to_string()
    };

    let rec_status_str = match t.state {
        RecordingState::Recording => {
            format!("\x1b[32;1m● ACTIVELY RECORDING TO DISK\x1b[0m (File: {})", t.active_filename.as_deref().unwrap_or("Active segment"))
        }
        RecordingState::ArmedSilent => {
            "\x1b[33;1m⏸ ARMED IN RAM\x1b[0m (Pre-roll active; waiting for music > -65 dBFS to write disk)".to_string()
        }
        RecordingState::Paused => "\x1b[36;1m⏸ PAUSED\x1b[0m".to_string(),
        RecordingState::Disconnected => "\x1b[90m○ IDLE (No mixer connected)\x1b[0m".to_string(),
    };

    let free_gb = t.free_disk_gb_tenths as f32 / 10.0;
    let est_hours_left = if free_gb > 0.0 { free_gb / 1.1 } else { 0.0 };

    let lid_protected = std::path::Path::new("/etc/systemd/logind.conf.d/djm-appliance.conf").exists();
    let lid_str = if lid_protected {
        "\x1b[32mStay Awake (Screen blanks, recording continues)\x1b[0m"
    } else {
        "\x1b[31;1mSuspend (Warning: Closing lid will stop recording! Run 'djm-box setup')\x1b[0m"
    };

    println!("\n=== djm-box v{} Appliance Status ===", djm_common::VERSION);
    println!("Mixer Detection:  {}", mixer_detection_str);
    println!("Recording Status: {}", rec_status_str);
    println!("Mic Routing:      \x1b[35;1mREC OUT\x1b[0m (Master + Mic captured via ALSA unmuted routing)");
    println!("Lid-Close Action: {}", lid_str);
    println!("Hardware Pad:     {} dB (Club DJ headroom attenuation)", t.hw_pad_db);
    println!("Auto-Guard:       {}", if t.auto_guard_active { "\x1b[32mActive (Auto-attenuate on redline)\x1b[0m" } else { "\x1b[90mDisabled\x1b[0m" });
    if t.clip_count > 0 {
        println!("Clips:            \x1b[31;1m{} CLIPPED SAMPLES\x1b[0m", t.clip_count);
    } else {
        println!("Clips:            \x1b[32m0 (Clean Signal)\x1b[0m");
    }
    println!("Active File:      {}", t.active_filename.as_deref().unwrap_or("None"));
    println!(
        "Duration & Size:  {}s ({:02}:{:02}:{:02}), {} MB",
        t.active_file_duration_secs,
        t.active_file_duration_secs / 3600,
        (t.active_file_duration_secs / 60) % 60,
        t.active_file_duration_secs % 60,
        t.active_file_size_mb
    );
    println!("Free Storage:     {:.1} GB (~{:.0} hours remaining)", free_gb, est_hours_left);

    println!("\n--- Incoming Audio Levels & Gain Staging ---");
    println!("L Peak: [{}] {:>6.1} dBFS  (RMS: {:>5.1} dBFS)", bar_l, peak_l, rms_l);
    println!("R Peak: [{}] {:>6.1} dBFS  (RMS: {:>5.1} dBFS)", bar_r, peak_r, rms_r);
    println!("         -60 dBFS             ▲ -19 dB (Mixer 0 dB)    0 dBFS (Clip)");

    if max_peak > -60.0 {
        let over_zero_sign = if over_mixer_zero >= 0.0 { "+" } else { "" };
        println!(
            "Gain Headroom:    {:.1} dB to digital 0 dBFS | {}{:.1} dB vs physical mixer 0 dB reference",
            headroom_to_clip, over_zero_sign, over_mixer_zero
        );
    } else {
        println!("Gain Headroom:    > 60 dB (Silence / Pre-Roll gate active)");
    }
    println!();
}

struct CursorGuard;
impl Drop for CursorGuard {
    fn drop(&mut self) {
        print!("\x1b[?25h\n");
        let _ = std::io::stdout().flush();
    }
}

async fn stream_live_monitor() -> Result<()> {
    let socket_path = default_socket_path();
    let stream = match UnixStream::connect(&socket_path).await {
        Ok(s) => s,
        Err(e) => {
            anyhow::bail!(
                "Could not connect to djm-daemon at {}: {}.\nIs djm-daemon running? Start it with 'djm-box daemon' or 'systemctl --user start djm-box-daemon'",
                socket_path.display(),
                e
            );
        }
    };

    println!("=== djm-box Live Audio Monitor [Press Ctrl+C to exit] ===");
    println!("Connecting to live audio telemetry stream...\n");

    let _guard = CursorGuard;
    print!("\x1b[?25l"); // Hide cursor
    let _ = std::io::stdout().flush();

    let mut lines = BufReader::new(stream).lines();
    let bar_width = 30;
    let mut first_frame = true;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                break;
            }
            res = lines.next_line() => {
                let line = match res? {
                    Some(l) => l,
                    None => break,
                };

                let t: Telemetry = match serde_json::from_str(&line) {
                    Ok(val) => val,
                    Err(_) => continue,
                };

                let peak_l = t.peak_l_centi_db as f32 / 100.0;
                let peak_r = t.peak_r_centi_db as f32 / 100.0;
                let rms_l = t.rms_l_centi_db as f32 / 100.0;
                let rms_r = t.rms_r_centi_db as f32 / 100.0;

                let bar_l = format_vu_bar(t.peak_l_centi_db, bar_width);
                let bar_r = format_vu_bar(t.peak_r_centi_db, bar_width);

                let max_peak = peak_l.max(peak_r);
                let headroom_to_clip = (-max_peak).max(0.0);
                let over_mixer_zero = max_peak - (-19.0);
                let over_zero_sign = if over_mixer_zero >= 0.0 { "+" } else { "" };

                let mixer_str = if t.is_mixer_connected() {
                    format!("\x1b[32;1m✓ {}\x1b[0m", t.mixer_model)
                } else {
                    "\x1b[31;1m✗ NO MIXER\x1b[0m".to_string()
                };

                let state_str = match t.state {
                    RecordingState::Recording => "\x1b[32;1m● RECORDING DISK\x1b[0m",
                    RecordingState::ArmedSilent => "\x1b[33;1m⏸ ARMED RAM (> -65dB)\x1b[0m",
                    RecordingState::Paused => "\x1b[36;1m⏸ PAUSED\x1b[0m",
                    RecordingState::Disconnected => "\x1b[90m○ IDLE\x1b[0m",
                };

                let clip_str = if t.clip_count > 0 {
                    format!("\x1b[31;1mCLIPS: {}\x1b[0m", t.clip_count)
                } else {
                    "\x1b[32mCLIPS: 0\x1b[0m".to_string()
                };

                let free_gb = t.free_disk_gb_tenths as f32 / 10.0;
                let dur_h = t.active_file_duration_secs / 3600;
                let dur_m = (t.active_file_duration_secs / 60) % 60;
                let dur_s = t.active_file_duration_secs % 60;

                if !first_frame {
                    // Move cursor up 6 lines to overwrite in-place
                    print!("\x1b[6A");
                } else {
                    first_frame = false;
                }

                // Line 1: Header / Mixer Detection / Recording State / Pad / Mic / Clips
                println!(
                    "\x1b[2K[djm-box v{}] MIXER: [{}] | REC: [{}] | Pad: {} dB | Mic: REC OUT | {}",
                    djm_common::VERSION,
                    mixer_str,
                    state_str,
                    t.hw_pad_db,
                    clip_str
                );
                // Line 2: File target / Duration / Size / Disk
                println!(
                    "\x1b[2KFile: {} ({:02}:{:02}:{:02}, {} MB) | Free Disk: {:.1} GB",
                    t.active_filename.as_deref().unwrap_or("None"),
                    dur_h,
                    dur_m,
                    dur_s,
                    t.active_file_size_mb,
                    free_gb
                );
                // Line 3: L Channel
                println!("\x1b[2KL Peak: [{}] {:>6.1} dBFS  (RMS: {:>5.1} dBFS)", bar_l, peak_l, rms_l);
                // Line 4: R Channel
                println!("\x1b[2KR Peak: [{}] {:>6.1} dBFS  (RMS: {:>5.1} dBFS)", bar_r, peak_r, rms_r);
                // Line 5: Meter scale
                println!("\x1b[2K         -60 dBFS             ▲ -19 dB (Mixer 0)     0 dBFS");
                // Line 6: Headroom / Gain
                println!(
                    "\x1b[2KHeadroom to 0 dBFS: {:>4.1} dB | Mixer 0 dB Ref: {}{:>4.1} dB | AutoGuard: {}",
                    headroom_to_clip,
                    over_zero_sign,
                    over_mixer_zero,
                    if t.auto_guard_active { "\x1b[32mON\x1b[0m" } else { "\x1b[90mOFF\x1b[0m" }
                );
                let _ = std::io::stdout().flush();
            }
        }
    }

    Ok(())
}
