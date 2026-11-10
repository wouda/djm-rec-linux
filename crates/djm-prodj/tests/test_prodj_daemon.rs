use anyhow::Result;
use djm_common::prodj::{parse_prodj_log, ProDjDeckStatus, ProDjDevice, ProDjLogEntry, ProDjPlayedTrack, ProDjTelemetry};
use djm_prodj::broadcaster::ProDjBroadcaster;
use djm_prodj::session_logger::SessionJsonlLogger;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;

#[tokio::test]
async fn test_prodj_broadcaster_and_jsonl_logging() -> Result<()> {
    let temp_dir = std::env::temp_dir().join(format!("test_daemon_prodj_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    let sock_path = temp_dir.join("test_prodj.sock");
    let log_path = temp_dir.join("event_session_prodj.jsonl");

    let running = Arc::new(AtomicBool::new(true));
    let (broadcaster, tx, latest) = ProDjBroadcaster::new(sock_path.clone(), running.clone());

    let b_handle = tokio::spawn(async move {
        let _ = broadcaster.run().await;
    });

    // Wait for socket to bind
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Connect client to socket
    let stream = UnixStream::connect(&sock_path).await?;
    let mut lines = BufReader::new(stream).lines();

    // 1. Verify initial snapshot received immediately
    let initial_line = lines.next_line().await?.expect("Initial snapshot line");
    let initial_telem: ProDjTelemetry = serde_json::from_str(&initial_line)?;
    assert!(initial_telem.devices.is_empty());
    assert!(initial_telem.is_settling);

    // 2. Broadcast updated telemetry
    let updated_device = ProDjDevice {
        player_number: 1,
        device_name: "CDJ-3000".to_string(),
        ip_address: "192.168.1.10".parse()?,
        mac_address: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        model: "CDJ-3000".to_string(),
        firmware: "3.10".to_string(),
        is_mixer: false,
        last_seen_ms: 1000,
    };
    let updated_deck = ProDjDeckStatus {
        player_number: 1,
        is_on_air: true,
        is_playing: true,
        is_master: true,
        bpm: Some(126.0),
        pitch_percent: Some(0.0),
        rekordbox_id: Some(42),
        track_title: Some("Banger".to_string()),
        track_artist: Some("Headliner".to_string()),
        track_key: Some("4A".to_string()),
        usb_media_id: Some("USB_KEY_1".to_string()),
        usb_media_name: Some("Main Set".to_string()),
        usb_slot: Some("USB".to_string()),
        playhead_secs: Some(45.0),
        duration_secs: Some(360.0),
        cumulative_on_air_secs: 25.0,
        degraded: false,
    };

    let mut telem = ProDjTelemetry::default();
    telem.devices.push(updated_device.clone());
    telem.decks.push(updated_deck);
    telem.is_settling = false;
    telem.is_degraded = false;
    telem.virtual_player_number = Some(6);
    telem.network_device_count = 1;

    // Update latest and broadcast
    {
        let mut w = latest.write().await;
        *w = telem.clone();
    }
    tx.send(telem.clone())?;

    // Read broadcast update
    let next_line = lines.next_line().await?.expect("Broadcast update line");
    let received_telem: ProDjTelemetry = serde_json::from_str(&next_line)?;
    assert_eq!(received_telem.devices.len(), 1);
    assert_eq!(received_telem.devices[0].model, "CDJ-3000");
    assert_eq!(received_telem.virtual_player_number, Some(6));
    assert_eq!(received_telem.decks[0].track_title.as_deref(), Some("Banger"));

    // 3. Test JSONL session logging
    let mut logger = SessionJsonlLogger::new();
    logger.open(&log_path)?;

    let played_entry = ProDjLogEntry::TrackPlayed {
        timestamp_rfc3339: "2026-10-30T23:45:00+01:00".to_string(),
        track: ProDjPlayedTrack {
            timestamp_rfc3339: "2026-10-30T23:45:00+01:00".to_string(),
            player_number: 1,
            rekordbox_id: Some(42),
            title: "Banger".to_string(),
            artist: "Headliner".to_string(),
            bpm: Some(126.0),
            key: Some("4A".to_string()),
            duration_secs: Some(360),
            usb_media_id: Some("USB_KEY_1".to_string()),
            usb_media_name: Some("Main Set".to_string()),
            cumulative_on_air_secs: 25.0,
        },
    };
    logger.log_entry(&played_entry)?;
    logger.close();

    // Verify written log
    let parsed_entries = parse_prodj_log(&log_path)?;
    assert_eq!(parsed_entries.len(), 1);
    assert_eq!(parsed_entries[0], played_entry);

    // 4. Shutdown broadcaster
    running.store(false, Ordering::Relaxed);
    let _ = b_handle.await;

    // Verify socket cleaned up
    assert!(!sock_path.exists());

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}
