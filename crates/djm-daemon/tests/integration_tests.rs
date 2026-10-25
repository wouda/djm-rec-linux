use djm_common::protocol::{RecordingState, Telemetry};
use djm_daemon::alsa_capture::SyntheticCapture;
use djm_daemon::flac_writer::FlacWriter;
use djm_daemon::headroom_guard::HeadroomGuard;
use djm_daemon::ipc_server::IpcServer;
use djm_daemon::preroll_buffer::PrerollBuffer;
use djm_daemon::session_log::SessionLog;
use djm_daemon::silence_gate::{GateAction, SilenceGate};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_end_to_end_recording_and_split_pipeline() {
    let tmp_dir = std::env::temp_dir().join(format!("djm_test_e2e_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp_dir);

    let sample_rate = 48000;
    let mut preroll = PrerollBuffer::new(sample_rate as usize * 2);
    let mut gate = SilenceGate::new(-75.0, -65.0, 1.0); // 1.0s timeout for test
    let mut guard = HeadroomGuard::new(true, -10);

    gate.set_state(RecordingState::ArmedSilent);

    // 1. In ArmedSilent, feed quiet audio (-85 dBFS)
    let quiet_chunk = vec![(200, 200); 480]; // 10ms
    assert_eq!(gate.process_block(&quiet_chunk, Duration::from_millis(10)), GateAction::None);
    preroll.push_block(&quiet_chunk);

    // 2. Feed loud audio (-20 dBFS) to trigger wake
    let loud_chunk = vec![(800_000, 800_000); 480];
    let action = gate.process_block(&loud_chunk, Duration::from_millis(10));
    assert_eq!(action, GateAction::WakeToRecord);
    assert_eq!(gate.state(), RecordingState::Recording);

    // 3. Open FLAC file part 1 and prepend pre-roll
    let flac_path1 = tmp_dir.join("DJM_test_pt01.flac");
    let mut writer1 = FlacWriter::new(&flac_path1, sample_rate, 5, 1).unwrap();
    let mut session_log1 = SessionLog::new(&flac_path1).unwrap();

    let drained_preroll = preroll.drain_chronological();
    assert!(!drained_preroll.is_empty());
    writer1.write_interleaved(&drained_preroll).unwrap();
    writer1.write_interleaved(&loud_chunk).unwrap();

    // Check Auto-Headroom Guard on very loud frame
    let clipping_chunk = vec![(8_000_000, 8_000_000); 100];
    if let Some(event) = guard.process_frames(&clipping_chunk) {
        session_log1.log_event("AUTO_GUARD", &format!("Pad stepped from {} to {}", event.old_pad_db, event.new_pad_db)).unwrap();
    }
    writer1.write_interleaved(&clipping_chunk).unwrap();

    // 4. Perform zero-gap split into Part 2
    let flac_path2 = tmp_dir.join("DJM_test_pt02.flac");
    let mut writer2 = FlacWriter::new(&flac_path2, sample_rate, 5, 1).unwrap();
    let _session_log2 = SessionLog::new(&flac_path2).unwrap();

    // Finalize part 1
    let finalized_path1 = writer1.finalize().unwrap();
    assert!(finalized_path1.exists());
    assert_eq!(finalized_path1, flac_path1);

    // Continue writing into part 2 immediately without gaps
    writer2.write_interleaved(&loud_chunk).unwrap();
    let finalized_path2 = writer2.finalize().unwrap();
    assert!(finalized_path2.exists());

    // 5. Check session logs created
    let log_content1 = std::fs::read_to_string(session_log1.path()).unwrap();
    assert!(log_content1.contains("[AUTO_GUARD]"));

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[tokio::test]
async fn test_fault_isolation_under_abrupt_client_crash() {
    let tmp_sock = std::env::temp_dir().join(format!("test_fault_iso_{}.sock", std::process::id()));
    let (cmd_tx, _cmd_rx) = mpsc::channel(32);
    let running = Arc::new(AtomicBool::new(true));

    let (server, telemetry_tx) = IpcServer::new(tmp_sock.clone(), cmd_tx, running.clone());

    tokio::spawn(async move {
        let _ = server.run().await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect client 1 (simulating TUI or remote)
    let client1 = UnixStream::connect(&tmp_sock).await.expect("client 1 connect");
    let mut reader1 = BufReader::new(client1).lines();

    // Wait until server has accepted and subscribed client 1
    for _ in 0..50 {
        if telemetry_tx.receiver_count() > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Emit telemetry
    let t = Telemetry {
        mixer_model: "DJM-V10".to_string(),
        clip_count: 1,
        ..Default::default()
    };
    telemetry_tx.send(t).unwrap();

    let frame = reader1.next_line().await.unwrap().unwrap();
    assert!(frame.contains("DJM-V10"));

    // Abruptly drop client 1 (simulating SIGKILL, crash, or freeze)
    drop(reader1);

    // Daemon emits 50 more frames with no client connected: MUST not panic or block!
    for i in 0..50 {
        let t_next = Telemetry {
            clip_count: i,
            ..Default::default()
        };
        let _ = telemetry_tx.send(t_next);
    }

    // Connect client 2: should seamlessly connect and receive latest frames
    let client2 = UnixStream::connect(&tmp_sock).await.expect("client 2 connect");
    let mut reader2 = BufReader::new(client2).lines();

    for _ in 0..50 {
        if telemetry_tx.receiver_count() > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let t_latest = Telemetry {
        mixer_model: "DJM-A9".to_string(),
        clip_count: 99,
        ..Default::default()
    };
    telemetry_tx.send(t_latest).unwrap();

    let frame2 = reader2.next_line().await.unwrap().unwrap();
    assert!(frame2.contains("DJM-A9"));

    running.store(false, Ordering::Relaxed);
    let _ = std::fs::remove_file(&tmp_sock);
}

#[test]
fn test_simulated_usb_disconnect_and_reconnection() {
    let tmp_dir = std::env::temp_dir().join(format!("test_djm_usb_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp_dir);

    let (ring_prod, mut ring_cons) = rtrb::RingBuffer::<(i32, i32)>::new(8192);
    let running = Arc::new(AtomicBool::new(true));

    // Phase 1: USB mixer connected, streaming audio
    let synthetic = SyntheticCapture::new(48000, running.clone());
    let h1 = std::thread::spawn(move || {
        synthetic.run_test_tone(ring_prod, 440.0, 0.5, Duration::from_millis(500));
    });

    // Write to FLAC
    let flac_path = tmp_dir.join("set_part1.flac");
    let mut writer = FlacWriter::new(&flac_path, 48000, 5, 2).unwrap();

    let mut buf = Vec::new();
    while buf.len() < 2400 {
        if let Ok(sample) = ring_cons.pop() {
            buf.push(sample);
        } else {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    writer.write_interleaved(&buf).unwrap();

    // Phase 2: Simulate DJ unplugging USB cable during set
    running.store(false, Ordering::Relaxed);
    let _ = h1.join();

    // Finalize first part safely without corruption
    let finalized1 = writer.finalize().unwrap();
    assert!(finalized1.exists());
    assert_eq!(finalized1.extension().unwrap(), "flac");

    // Phase 3: DJ plugs USB cable back in (or plugs into new mixer)
    let running2 = Arc::new(AtomicBool::new(true));
    let (ring_prod2, mut ring_cons2) = rtrb::RingBuffer::<(i32, i32)>::new(8192);
    let synthetic2 = SyntheticCapture::new(96000, running2.clone());
    let h2 = std::thread::spawn(move || {
        synthetic2.run_test_tone(ring_prod2, 880.0, 0.5, Duration::from_millis(500));
    });

    let flac_path2 = tmp_dir.join("set_part2.flac");
    let mut writer2 = FlacWriter::new(&flac_path2, 96000, 5, 2).unwrap();

    let mut buf2 = Vec::new();
    while buf2.len() < 4800 {
        if let Ok(sample) = ring_cons2.pop() {
            buf2.push(sample);
        } else {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    writer2.write_interleaved(&buf2).unwrap();

    running2.store(false, Ordering::Relaxed);
    let _ = h2.join();

    let finalized2 = writer2.finalize().unwrap();
    assert!(finalized2.exists());
    assert_eq!(finalized2.extension().unwrap(), "flac");

    let _ = std::fs::remove_dir_all(&tmp_dir);
}
