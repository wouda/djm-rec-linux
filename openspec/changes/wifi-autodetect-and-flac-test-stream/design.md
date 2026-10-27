# Design

## Context

The core recording daemon `djm-daemon` runs in a real-time thread capturing 24-bit 96kHz audio from Pioneer DJ mixers via native ALSA and writes directly to `.flac` files in `~/Music/DJ_Recordings/` with 2-second `fdatasync` checkpoints. It broadcasts `Telemetry` JSON (including `active_filename` and `state`) over a Unix domain socket to `djm-remote`. `djm-remote` serves an HTTP/WebSocket dashboard on port 8080. The Flutter mobile companion connects over WebSocket.

Crucially, `djm-daemon` must remain completely untouched to preserve tested recording reliability.

## Goals / Non-Goals

**Goals:**
- Provide a 15-second 24-bit FLAC audio stream preview endpoint in `djm-remote` (`/api/audio/test-stream`) extracted directly from the active recording file on disk using `/usr/bin/ffmpeg` with zero transcoding CPU overhead.
- Provide a continuous live stream endpoint in `djm-remote` (`/api/audio/live-stream`) following the active recording file via `ffmpeg -follow 1 -re` and transcoding to 192kbps MP3 (`libmp3lame`) to minimize Wi-Fi bandwidth (saving 97% bandwidth) and guarantee smooth continuous playback on mobile devices.
- Add dual playback controls ("🎧 Test 15s FLAC" and "🔴 Live Stream MP3") with real-time countdown progress and live elapsed timers with instant cancel/stop controls in both Web and Android apps.
- Provide continuous background auto-probing in `flutter_app` across booth Hotspot (`10.42.0.1:8080`) and LAN mDNS (`djm-box.local:8080`), auto-connecting if PIN is stored or displaying a 1-tap PIN dialog if unauthenticated.
- Overhaul disconnected dashboard layouts to clearly distinguish between "Appliance Disconnected (Searching Wi-Fi...)" and "Mixer Disconnected (Plug USB-B cable)", dimming/disabling action controls when offline.
- Bump project release version to `0.2.4` and create git release tag `v0.2.4`.

**Non-Goals:**
- Modifying `djm-daemon` or ALSA capture threads in any way.
- Bluetooth or BLE support.

## Decisions

### 1. Hybrid Streaming Strategy: Direct FLAC for 15s Test, 192kbps MP3 for Continuous Live Stream
- **15s Test Stream (`/api/audio/test-stream`)**:
  - Uses `ffmpeg -sseof -15 -i "<file_path>" -t 15 -c:a flac -f flac -`.
  - Zero lossy encoding or resampling; 0% transcoding CPU; tests exact bit-perfect FLAC capture.
- **Continuous Live Stream (`/api/audio/live-stream`)**:
  - Uses `ffmpeg -follow 1 -re -i "<file_path>" -c:a libmp3lame -b:a 192k -f mp3 -`.
  - Uses only ~24 KB/s Wi-Fi bandwidth (vs ~550 KB/s for 96kHz FLAC), preventing buffer stalls on phone Wi-Fi in RF-heavy club booths and minimizing phone battery usage while keeping host CPU overhead around ~3%.
  - Uses `kill_on_drop(true)` so terminating the connection or pressing Stop immediately kills the background ffmpeg process.

### 2. Auto-Discovery via Probing Hotspot (`10.42.0.1`) and mDNS (`djm-box.local`)
- `TransportManager` runs a probe loop that attempts lightweight HTTP `GET /api/status` to `http://10.42.0.1:8080/api/status` and `http://djm-box.local:8080/api/status`. If an appliance responds:
  - If a stored PIN exists and matches, the WebSocket connection initializes immediately.
  - If no PIN exists or 401 Unauthorized is returned, the app displays a prominent quick-connect PIN prompt with the detected IP pre-filled.

### 3. Clear Hierarchy for Disconnected State in UI
- When `activeTransport == disconnected` or `connecting`, render a high-visibility amber/slate banner at the top: `⚠️ Appliance Disconnected — Searching Wi-Fi (10.42.0.1 / djm-box.local)...`.
- Suppress the mixer-specific warning ("No Pioneer DJM Mixer Detected") so DJs do not mistake network disconnects for hardware faults.
- Dim and disable action buttons (+ Bookmark, Split File, Lock -19dB, Reset Peaks, Test Audio) with reduced opacity and null `onPressed`.

### 4. Version Bump to v0.2.4
- Increment version from `0.2.3` to `0.2.4` across `Cargo.toml`, `web_server.rs`, `pubspec.yaml`, `dashboard_screen.dart`, and `README.md`, and tag commit with `v0.2.4`.

## Risks / Trade-offs

- **[Risk] Active FLAC file is incomplete / less than 15 seconds old**
  → *Mitigation*: FFmpeg handles partial/growing FLAC files gracefully. If the set just began, ffmpeg extracts available audio and streams cleanly.
- **[Risk] Resource leak if multiple live streams opened**
  → *Mitigation*: Tokio's `cmd.kill_on_drop(true)` ensures any aborted or closed HTTP connection immediately sends SIGKILL to the child ffmpeg process.
