# Tasks

## 1. Remote Audio Streaming Endpoints & Web Dashboard (`djm-remote`)

- [x] 1.1 Implement authenticated `GET /api/audio/test-stream?token=<PIN>` endpoint in `crates/djm-remote/src/web_server.rs` that verifies `state == Recording` and `active_filename` presence, launches `ffmpeg -sseof -15 -i <active_flac> -t 15 -c:a flac -f flac -`, and pipes the 15-second 24-bit FLAC stream as chunked HTTP response body, verifying endpoint unit tests pass
- [x] 1.2 Implement authenticated `GET /api/audio/live-stream?token=<PIN>` endpoint in `crates/djm-remote/src/web_server.rs` that verifies `state == Recording`, follows the active file with `ffmpeg -follow 1 -re -i <active_flac> -c:a libmp3lame -b:a 192k -f mp3 -`, and streams continuous 192kbps MP3 audio with `kill_on_drop(true)`, verifying endpoint unit tests pass
- [x] 1.3 Add dual audio monitor controls in `crates/djm-remote/src/web_server.rs` `DASHBOARD_HTML` featuring "🎧 Test (15s FLAC)" with countdown bar and "🔴 Live Stream (MP3)" with pulsing live indicator and instant Stop buttons, verifying `cargo test -p djm-remote` passes

## 2. Android Companion Wi-Fi Autodetection & Dual Player (`flutter_app`)

- [x] 2.1 Add `audioplayers` dependency to `flutter_app/pubspec.yaml` and verify `flutter pub get` and build configurations succeed without introducing Bluetooth dependencies or permissions
- [x] 2.2 Implement continuous smart auto-probing in `flutter_app/lib/services/transport_manager.dart` targeting booth Hotspot (`10.42.0.1:8080`) and LAN mDNS (`djm-box.local:8080`), automatically connecting if a saved PIN exists and surfacing detected appliances for 1-tap PIN entry if unauthenticated, verifying `flutter test` passes
- [x] 2.3 Overhaul `flutter_app/lib/screens/dashboard_screen.dart` to render a prominent high-contrast "Appliance Disconnected (Searching Wi-Fi...)" top card when offline, suppress mixer hardware disconnect warnings until connected, and dim/disable action controls when offline, verifying UI state rendering tests pass
- [x] 2.4 Integrate dual audio controls into `flutter_app/lib/screens/dashboard_screen.dart` supporting both 15-second FLAC test preview and continuous 192kbps MP3 live streaming via `audioplayers`, with elapsed timer and instant Stop buttons, verifying `flutter test` passes

## 3. Version Bump, Tagging & Documentation

- [x] 3.1 Bump version to `0.2.4` across `Cargo.toml`, `crates/djm-remote/src/web_server.rs`, `flutter_app/pubspec.yaml`, `flutter_app/lib/screens/dashboard_screen.dart`, and `README.md`
- [x] 3.2 Run `openspec validate wifi-autodetect-and-flac-test-stream` and `cargo test --workspace` to verify all OpenSpec artifacts and workspace tests pass cleanly
- [x] 3.3 Commit changes and create git release tag `v0.2.4`
