use std::fs::{self, File};
use std::io::Write;
use std::process::Command;

#[test]
fn test_cli_process_end_to_end_mock_session() {
    let temp_dir = std::env::temp_dir().join(format!("djm_test_cli_process_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("create temp dir");

    let flac_path = temp_dir.join("mock_2026-10-30_230000_pt01.flac");
    let log_path = temp_dir.join("mock_2026-10-30_230000_pt01.log");
    let tt_path = temp_dir.join("timetable.txt");
    let out_dir = temp_dir.join("output");

    // 1. Generate 20s test audio FLAC: 8s sine tone + 2s silence + 10s sine tone at 96 kHz stereo
    let status = Command::new("ffmpeg")
        .args([
            "-y", "-nostats",
            "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=96000:duration=8",
            "-f", "lavfi", "-i", "anullsrc=channel_layout=stereo:sample_rate=96000",
            "-f", "lavfi", "-i", "sine=frequency=880:sample_rate=96000:duration=10",
            "-filter_complex", "[0:a]aformat=channel_layouts=stereo[a0]; [1:a]atrim=0:2[a1]; [2:a]aformat=channel_layouts=stereo[a2]; [a0][a1][a2]concat=n=3:v=0:a=1[out]",
            "-map", "[out]",
            "-c:a", "flac",
            flac_path.to_str().unwrap(),
        ])
        .status()
        .expect("spawn ffmpeg to generate test fixture");
    assert!(status.success(), "ffmpeg test fixture generation failed");

    // 2. Write matching session log with silence gap
    let mut log_file = File::create(&log_path).expect("create mock log");
    writeln!(log_file, "[2026-10-30T23:00:00+01:00] [SESSION_INIT] Session log opened for {}", flac_path.display()).unwrap();
    writeln!(log_file, "[2026-10-30T23:00:00+01:00] [SESSION_START] Opened session recording for: {}", flac_path.display()).unwrap();
    writeln!(log_file, "[2026-10-30T23:00:00+01:00] [GATE_WAKE] Silence gate woke! Audio detected. Target file: {} Pre-roll samples: 0.", flac_path.display()).unwrap();
    writeln!(log_file, "[2026-10-30T23:00:08+01:00] [GATE_SILENCE] Silence detected below gate threshold (-65.0 dBFS) for 2.0s.").unwrap();
    writeln!(log_file, "[2026-10-30T23:00:10+01:00] [GATE_WAKE] Audio detected above wake threshold (-65.0 dBFS).").unwrap();
    writeln!(log_file, "[2026-10-30T23:00:20+01:00] [STATUS] Progress update: 1 MB written | Elapsed: 00:00:20 | Target: {} | Clips: 0 | Peak: -6.0 dBFS", flac_path.display()).unwrap();
    writeln!(log_file, "[2026-10-30T23:00:20+01:00] [SESSION_STOP] Silence timeout reached. Stopping recording.").unwrap();

    // 3. Write timetable
    let mut tt_file = File::create(&tt_path).expect("create timetable");
    writeln!(tt_file, "23:00 Mock DJ Alpha").unwrap();
    writeln!(tt_file, "23:01 Mock DJ Beta").unwrap();

    // 4. Run cargo run --bin djm-box or target/debug/djm-box
    let bin_path = env!("CARGO_BIN_EXE_djm-box");
    let cli_output = Command::new(bin_path)
        .args([
            "process",
            "--recordings-dir", temp_dir.to_str().unwrap(),
            "--timetable", tt_path.to_str().unwrap(),
            "--output-dir", out_dir.to_str().unwrap(),
            "--preset", "streaming",
            "--mp3",
            "--full-event",
            "--force",
        ])
        .output()
        .expect("run djm-box process");

    let stdout = String::from_utf8_lossy(&cli_output.stdout);
    let stderr = String::from_utf8_lossy(&cli_output.stderr);
    println!("CLI STDOUT:\n{}", stdout);
    println!("CLI STDERR:\n{}", stderr);

    assert!(cli_output.status.success(), "djm-box process failed: {}", stderr);

    // 5. Verify all expected outputs exist
    assert!(out_dir.join("01_Mock DJ Alpha.flac").exists(), "01 FLAC missing");
    assert!(out_dir.join("01_Mock DJ Alpha.mp3").exists(), "01 MP3 missing");
    assert!(out_dir.join("02_Mock DJ Beta.flac").exists(), "02 FLAC missing");
    assert!(out_dir.join("02_Mock DJ Beta.mp3").exists(), "02 MP3 missing");
    assert!(out_dir.join("mock_2026-10-30_230000_Full_Mastered.flac").exists(), "Full event FLAC missing");
    assert!(out_dir.join("Event_Report.html").exists(), "HTML report missing");
    assert!(out_dir.join("Event_Report.md").exists(), "Markdown report missing");
    assert!(out_dir.join("Event_Report.json").exists(), "JSON report missing");
    assert!(out_dir.join("tracklist.txt").exists(), "tracklist.txt missing");

    // Clean up
    let _ = fs::remove_dir_all(&temp_dir);
}
