# Proposal

## Why

DJs and booth sound engineers need a fast, non-intrusive way to verify that bit-perfect Pioneer audio is actively recording without pulling out USB drives or interfering with the live recording engine. In addition, when monitoring during a set, users need both a quick 15-second bit-perfect FLAC audio test and an optional continuous live stream that uses minimal Wi-Fi bandwidth (192kbps MP3) to prevent phone battery drain and audio stutter. In the Android companion app, DJs currently cannot easily distinguish between a lost Wi-Fi connection to the laptop and an unplugged mixer USB-B cable, and the app does not automatically detect the appliance on Wi-Fi without manual intervention.

## What Changes

- **Hybrid Audio Streaming Without Modifying `djm-daemon` (`djm-remote`)**:
  - Add authenticated HTTP endpoint `GET /api/audio/test-stream?token=<PIN>` to stream up to 15 seconds of the active 24-bit FLAC recording directly from disk via `ffmpeg` (`-sseof -15 ... -c:a flac -f flac -`) with 0% CPU transcoding overhead.
  - Add authenticated HTTP endpoint `GET /api/audio/live-stream?token=<PIN>` to stream continuous live audio transcoded to 192kbps MP3 via `ffmpeg` (`-follow 1 -re ... -c:a libmp3lame -b:a 192k -f mp3 -`), saving 97% Wi-Fi bandwidth for stable continuous phone monitoring.
  - Add interactive dual playback controls ("🎧 Test (15s FLAC)" with countdown bar and "🔴 Live Stream (MP3)" with elapsed timer and instant Stop buttons) to the Web Remote Dashboard.
- **Smart Wi-Fi Autodetection & Android Companion Enhancements (`flutter_app`)**:
  - Implement background auto-probing in `TransportManager` across the booth Wi-Fi Hotspot (`10.42.0.1:8080`) and LAN mDNS (`djm-box.local:8080`) on app launch, automatically establishing the WebSocket connection if a PIN is saved, or presenting a 1-tap PIN modal if unauthenticated.
  - Redesign the disconnected dashboard layout to clearly distinguish between **Appliance Offline** (phone disconnected from djm-box over Wi-Fi) and **Mixer Disconnected** (djm-box connected, but Pioneer mixer unplugged), with an unmistakable top status card and dimmed action controls when offline.
  - Add native dual player controls ("🎧 Test 15s" and "🔴 Live Stream") into the Android companion app using `audioplayers`, honoring recording state gates.
- **Release Version Bump**:
  - Bump appliance and companion app versions from `0.2.3` to `0.2.4` across workspace packages, UI badges, and Android bundle config, and create git release tag `v0.2.4`.

## Capabilities

### New Capabilities
- `flac-test-stream`: 15-second lossless 24-bit FLAC audio preview endpoint and continuous 192kbps MP3 live streaming in `djm-remote` and player controls in Web and Android companion monitors, served directly from disk without altering `djm-daemon`.
- `smart-wifi-discovery-and-connection`: Automatic background discovery of the recording appliance over Wi-Fi Hotspot (`10.42.0.1`) and mDNS (`djm-box.local`), distinct offline UI state separation, and pure Wi-Fi operation.

### Modified Capabilities

## Impact

- **`crates/djm-daemon`**: Untouched. Retains 100% existing stability, real-time ALSA capture, SPSC ring buffering, and FLAC writing behavior.
- **`crates/djm-remote`**: Adds `/api/audio/test-stream` and `/api/audio/live-stream` endpoints in `src/web_server.rs` using `ffmpeg`, and updates `DASHBOARD_HTML` with dual Test (15s) and Live Stream controls.
- **`flutter_app`**: Adds `audioplayers` dependency to `pubspec.yaml`, adds auto-probe lifecycle in `transport_manager.dart`, redesigns `dashboard_screen.dart` with top offline status card, dimmed controls, and dual audio test/live preview controls.
- **Project Versioning**: Updates workspace `Cargo.toml`, `pubspec.yaml`, `README.md`, and creates git tag `v0.2.4`.
