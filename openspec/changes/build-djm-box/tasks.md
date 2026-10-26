# Tasks: djm-box Implementation

## 1. Cargo Workspace Scaffolding & Common Data Types

- [x] 1.1 Initialize Rust Cargo workspace containing `crates/djm-common`, `crates/djm-daemon`, `crates/djm-tui`, and `crates/djm-remote`, and verify `cargo check --workspace` succeeds.
- [x] 1.2 Implement configuration parsing in `djm-common` according to `~/.config/djm-box/config.toml` schema (including club-tuned silence defaults: `120.0s` / `-75.0 dBFS`, `auth_token`, and `remote_public_url`) and verify unit tests validate default values and TOML deserialization.
- [x] 1.3 Define IPC telemetry, pairing info, and command protocol data structures in `djm-common` (including 20-byte packed binary format, `0x00` auth command, and JSON serialization) and verify round-trip binary packing tests pass.
- [x] 1.4 Implement mixer profile catalog in `djm-common` covering Pioneer (`08e4`) and AlphaTheta/Pioneer DJ (`2b73`) models (DJM-900NXS2, DJM-V10, DJM-A9, DJM-900NXS, DJM-850, DJM-750MK2, DJM-450, DJM-250MK2, generic fallback) and verify lookup unit tests.

## 2. djm-daemon: ALSA Capture, Instant Re-Plug & Headroom Guard Engine

- [x] 2.1 Implement native ALSA capture loop in `djm-daemon` supporting native 96kHz and 48kHz 24-bit stereo streams, feeding into a lock-free SPSC circular buffer, and verify audio buffer ingestion tests.
- [x] 2.2 Implement ALSA mixer hardware control routines in `djm-daemon` to set `Capture Level` (or `Capture Level Capture Switch`) to `-19dB` and `Ch5 Input = Rec Out` on connection without altering input channels 1–6, and verify mock ALSA mixer tests.
- [x] 2.3 Implement the Auto-Headroom Guard in `djm-daemon` that dynamically steps `Capture Level` down toward `-19dB` when peak levels exceed `-0.5 dBFS`, logging exact sample frame offsets and dB deltas, and verify threshold step-down unit tests.
- [x] 2.4 Implement graceful ALSA disconnect handling (`-ENODEV` / `-EPIPE`) for accidental USB unplugs during DJ handovers, finalizing the current file and automatically re-applying hardware pad and resuming capture upon USB re-plug, verifying recovery tests.

## 3. djm-daemon: Silence Gating, Pre-Roll Buffer & Crash-Proof FLAC Pipeline

- [x] 3.1 Implement the in-memory 2.0-second PCM circular pre-roll buffer in `djm-daemon` that records rolling audio in `ARMED_SILENT` without writing to disk, and verify buffer windowing tests.
- [x] 3.2 Implement club-tuned silence-gate pause-split state machine (`DISCONNECTED` -> `ARMED_SILENT` -> `RECORDING`) with configurable thresholds (`silence_threshold_dbfs = -75.0`, `wake_threshold_dbfs = -65.0`, `pause_split_timeout_secs = 120.0`), and verify state transition tests.
- [x] 3.3 Implement 24-bit bit-perfect FLAC encoder pipeline writing to `.flac.part` with a pre-allocated 4 KB `PADDING` header block, periodic 2-second `fdatasync()`, and 30-second positional `STREAMINFO` `total_samples` checkpointing, verifying header integrity tests.
- [x] 3.4 Implement sample-accurate (zero-gap) file splitting across `max_segment_duration_mins` and manual splits by immediately opening the next segment on sample $N+1$ while offloading ReplayGain calculation, in-place 4 KB header tag writing (`REPLAYGAIN_*`, `DJM_GAIN_STEPS`), and `.flac.part` -> `.flac` rename to a background finalizer worker, verifying zero-sample-loss continuity tests.
- [x] 3.5 Implement startup `.flac.part` auto-repair scanner in `djm-daemon` that detects interrupted recordings from power cuts, patches `STREAMINFO`, and finalizes them to `.flac`, plus sidecar `.log` session writer, verifying crash-recovery unit tests.

## 4. djm-daemon: Non-Blocking Lag-Drop Unix Socket IPC

- [x] 4.1 Implement Unix domain socket server (`/run/user/$UID/djm-box.sock`) in `djm-daemon` with Tokio broadcast channel emitting 30–60 Hz telemetry with lag-drop semantics, and verify slow client drop behavior tests.
- [x] 4.2 Implement socket command receiver in `djm-daemon` for `DropBookmark`, `SplitFile`, `SetHardwarePad`, `ToggleAutoGuard`, `ResetPeaks`, and `OpenPairingWindow`, verifying command dispatch tests.

## 5. djm-tui: Stateless Ratatui Terminal UI & QR Pairing Modal

- [x] 5.1 Implement socket client connection and frame receiver in `djm-tui` connecting to `/run/user/$UID/djm-box.sock` with graceful exit on `q` / `Ctrl+C` without affecting daemon execution, and verify client lifecycle.
- [x] 5.2 Build Ratatui UI layout in `djm-tui` with appliance header (mixer model, sample rate, pad status, auto-guard indicator, authenticated phone connection status, recording badge) and verify terminal layout tests.
- [x] 5.3 Render dual horizontal stereo L/R dBFS meters with physical 0dB reference marker (`-19 dBFS`), peak hold, RMS, clip counters, and live silence-split countdown bar, verifying visual meter scaling.
- [x] 5.4 Render active recording card (filename, timer, size, bitrate, remaining disk hours) and `[P]` Unicode QR Code + 6-digit PIN phone pairing modal (`qrcode` crate rendered with half-block characters), wiring interactive hotkeys (`[`/`]`, `A`, `S`, `T`, `M`, `R`, `P`) and verifying QR payload generation tests.

## 6. djm-remote: Authenticated BLE GATT & Axum Web Bridge

- [x] 6.1 Implement `djm-remote` daemon connecting as an unprivileged client to `/run/user/$UID/djm-box.sock` with auto-reconnect logic, verifying socket consumption tests.
- [x] 6.2 Implement Two-Layer Bluetooth Security in `djm-remote` using `bluer`: keep BlueZ `Pairable = false` outside explicit 60-second `[P]` pairing windows, and enforce a 3.0-second GATT `0x00 <auth_token>` watchdog that calls BlueZ `device.disconnect()` on unauthenticated scanners and resumes advertising, verifying auth timeout eviction tests.
- [x] 6.3 Implement BLE GATT peripheral characteristics (`0000d900-0000-1000-8000-00805f9b34fb`) broadcasting 20-byte packed telemetry at 15–20 Hz, JSON metadata, and authenticated write command dispatch (`0x00` auth, `0x01` bookmark, `0x02` split, `0x03` pad, `0x04` guard, `0x05` reset) to authenticated clients, verifying GATT protocol tests.
- [x] 6.4 Implement token-authenticated (`Authorization: Bearer` / `?token=`) embedded Axum HTTP server and 60 Hz WebSocket bridge on `0.0.0.0:8080` with mDNS broadcast (`djm-box.local`) and Internet/Tailscale compatibility, serving a self-contained dark-mode HTML5/CSS/JS dashboard, verifying auth middleware and WebSocket streaming tests.

## 7. flutter_app: Multi-Transport Companion Mobile Client

- [x] 7.1 Scaffold Flutter companion mobile application under `flutter_app/` with dependencies (`flutter_blue_plus`, `web_socket_channel`, `shared_preferences`), and verify `flutter pub get` and project analysis.
- [x] 7.2 Implement QR Code / 6-digit PIN pairing flow and Smart Multi-Transport Connection Manager in `flutter_app/` that automatically prioritizes Local Wi-Fi WebSocket (`ws://djm-box.local:8080/ws`), Internet/Tailscale WebSocket (`ws(s)://<remote>/ws`), and Offline BLE GATT (`0000d900-...` with automatic `0x00 <auth_token>` handshake), verifying transport failover and telemetry unpacking unit tests.
- [x] 7.3 Build high-contrast OLED dark theme UI featuring active transport & connection pill (`Wi-Fi Connected`, `Remote Connected`, or `BLE Connected`), recording status badge (`● RECORDING` / `⏸ ARMED`), mixer info card, and hardware pad selector, verifying widget layout.
- [x] 7.4 Build dual smooth stereo VU meters with `-19 dBFS` physical reference line and prominent red clip indicator, verifying meter animations.
- [x] 7.5 Build active file info card (timer, size, remaining disk) and authenticated action buttons (`Bookmark Set`, `Split File Now`, `Reset Peaks`) with haptic feedback, verifying user action handling across both WebSocket and BLE transports.

## 8. CachyOS Systemd, Udev & Appliance Automation

- [x] 8.1 Implement `djm-box setup` CLI subcommand in Rust that checks/installs system packages (`alsa-lib`, `alsa-utils`, `flac`, `pipewire`, `bluez`, `bluez-utils`, `avahi`) via `yay` / `pacman` and generates `~/.config/djm-box/config.toml` with a random `auth_token` (`0600` permissions), verifying CLI setup tests.
- [x] 8.2 Generate and install `/etc/udev/rules.d/99-djm-box.rules` matching Pioneer and AlphaTheta USB vendor IDs (`08e4`, `2b73`), and verify udev rule syntax.
- [x] 8.3 Generate and install systemd user units: `~/.config/systemd/user/djm-box-daemon.service` (`CPUSchedulingPolicy=fifo`, `CPUSchedulingPriority=80`, `Restart=always`) and `~/.config/systemd/user/djm-box-remote.service` (`Restart=always`), verifying unit file correctness.
- [x] 8.4 Generate and install WirePlumber pro-audio rules (`51-djm-pro-audio.conf`) and systemd logind configuration (`/etc/systemd/logind.conf.d/djm-appliance.conf` with `HandleLidSwitch=ignore`), verifying configuration file syntax.

## 9. End-to-End Appliance Integration & Fault-Tolerance Verification

- [x] 9.1 Run simulated end-to-end recording pipeline tests (synthetic PCM audio feeding daemon, 2s pre-roll buffer flush, zero-gap background split, 4 KB padded FLAC generation, `STREAMINFO` checkpointing, `.flac.part` crash-recovery repair, ReplayGain tags, and `.log` generation).
- [x] 9.2 Execute stress tests simulating unauthorized BLE connection attempts, USB cable unplug/re-plug cycles, and sudden kills/restarts of `djm-tui` and `djm-remote` during active recording to verify that `djm-daemon` suffers zero audio frame drops or stalls.
