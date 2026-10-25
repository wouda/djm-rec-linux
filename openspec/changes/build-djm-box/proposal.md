# Proposal: djm-box Fault-Isolated DJ Mixer Recording Appliance

## Why

Live recording of full-night, high-energy DJ sets directly from professional mixers (e.g., Pioneer DJM-900NXS2, DJM-V10, DJM-A9) in nightclub booths is prone to critical failure modes:
1. High-volume DJ double-drops routinely push mixer masters into physical red-lining, causing digital clipping or harsh digital distortion if input gain is misconfigured.
2. Monolithic recording software often crashes or stutters audio buffers due to GUI lockups, Bluetooth/D-Bus lag, accidental USB unplugging during DJ laptop handovers, or OS sleep states when closing a laptop lid.
3. Unsecured Bluetooth or web interfaces in a crowded club allow random patrons or devices (e.g., BLE scanners/Flipper Zero) to hijack the connection or disrupt recordings, while relying solely on Bluetooth limits monitoring range to 10 meters around the booth.

`djm-box` solves these problems by providing an automated, fault-isolated recording appliance specifically engineered for CachyOS (Arch Linux). It decouples the core audio capture daemon from the user interface and remote bridges, automates hardware digital pad attenuation to safeguard headroom, ensures zero-gap continuous recording across full club nights with crash-proof FLAC checkpointing, and offers authenticated multi-transport phone monitoring over Local Wi-Fi, Internet/VPN (Tailscale), and offline Bluetooth Low Energy (BLE) paired via a terminal QR code.

## What Changes

- Introduce a high-performance Rust Cargo workspace containing three strictly fault-isolated binaries:
  - `djm-daemon`: Dedicated real-time audio capture engine running under FIFO real-time scheduling with zero network, UI, or Bluetooth dependencies. Publishes telemetry and receives control over non-blocking Unix Domain Socket IPC with lag-drop semantics.
  - `djm-tui`: Stateless, disposable terminal dashboard built with `ratatui` and `crossterm`, featuring an on-demand Unicode QR code & 6-digit PIN pairing modal.
  - `djm-remote`: Separate systemd user service exposing an authenticated 15–20 Hz BLE GATT peripheral server (via `bluer` / BlueZ D-Bus with bonded/pairing control and a 3-second token auth watchdog that immediately disconnects unauthorized BLE scanners) and an authenticated embedded Axum HTTP/WebSocket server.
- Introduce `flutter_app/`: Cross-platform Flutter companion mobile app with a Smart Multi-Transport Connection Manager that automatically switches between Local Wi-Fi (`ws://djm-box.local:8080/ws`), Internet/Tailscale (`wss://` / `ws://` remote host), and offline BLE GATT, with QR-code pairing for one-scan setup.
- Implement native ALSA capture and hardware pad control for Pioneer (`08e4`) and AlphaTheta/Pioneer DJ (`2b73`) USB mixers with auto-headroom guarding (`-19dB` default pad), sample-accurate gain-step logging, and instant recovery from accidental USB cable unplugs during B2B DJ handovers.
- Implement bit-perfect 24-bit FLAC encoding at native mixer sample rates (96 kHz / 48 kHz) with a 2-second RAM pre-roll circular buffer, club-tuned silence-gate defaults (`120s` at `-75 dBFS`), zero-sample-loss background file splitting, 4 KB pre-allocated FLAC header padding, periodic `fdatasync()` + `STREAMINFO` header checkpointing, `.flac.part` startup auto-repair, and parallel ReplayGain metadata tagging.
- Provide a TOML configuration engine (`~/.config/djm-box/config.toml`) supporting hot-reloading, authentication tokens, and CLI overrides.
- Provide an automated CachyOS/Arch Linux appliance setup command (`djm-box setup`) that installs system dependencies via `yay`, generates a unique appliance auth token, and configures udev hotplug rules, systemd user services, WirePlumber pro-audio rules, and lid-switch inhibitors.

## Capabilities

### New Capabilities
- `daemon-ipc-and-fault-isolation`: Multi-process fault isolation, rootless systemd service lifecycle, non-blocking lag-drop Unix domain socket IPC telemetry and commands.
- `alsa-capture-and-hardware-pad`: Native ALSA hotplug detection and instant USB re-plug recovery for Pioneer/AlphaTheta DJ mixers, mixer card channel routing, hardware `Capture Level` pad adjustment, and auto-headroom guard with sample-offset logging.
- `silence-gate-preroll-and-flac`: Crash-proof 24-bit bit-perfect FLAC recording engine, 2-second PCM RAM pre-roll buffer, club-tuned pause-split state machine, zero-gap background segment splitting, periodic `fdatasync` + `STREAMINFO` header checkpointing, 4 KB header padding, `.flac.part` auto-repair, and parallel ReplayGain metadata tagging.
- `dsp-metering-and-ratatui-tui`: Real-time DSP metering (Peak, RMS, true peak headroom, clip counter) and stateless Ratatui/crossterm terminal UI monitoring dashboard with interactive QR code / PIN phone pairing.
- `ble-gatt-and-axum-remote-service`: Independent remote bridge service providing authenticated BLE GATT peripheral telemetry/control (with BlueZ pairing control + 3-second token watchdog) and token-authenticated embedded Axum WebSocket/HTML mobile dashboard for LAN and Internet/VPN access.
- `flutter-mobile-companion-app`: Cross-platform Flutter mobile client with QR-code pairing and Smart Multi-Transport auto-switching (Local Wi-Fi WebSocket, Internet/VPN WebSocket, and offline BLE GATT), providing an OLED dark-mode booth monitor, stereo VU meters, file stats, and remote actions.
- `cachyos-systemd-udev-setup`: Appliance automated installer for CachyOS / Arch Linux configuring udev rules, PipeWire/WirePlumber pro-audio rules, systemd user services, initial auth credentials, and logind lid-switch inhibitors.

### Modified Capabilities
*(None - fresh project)*

## Impact

- **Architecture & System Safety**: Strict separation of concerns ensures that terminal crashes, Bluetooth stack hangs, unauthorized crowd BLE connections, or network failures never interrupt or stutter ongoing recordings.
- **Dependencies**:
  - Rust workspace: `alsa`, `flac-bound` / `libflac-sys` or safe Rust FLAC encoder, `tokio`, `bluer`, `axum`, `tower`, `ratatui`, `crossterm`, `qrcode`, `serde`, `toml`, `notify`, `tracing`, `rand`.
  - CachyOS system packages: `alsa-lib`, `alsa-utils`, `flac`, `pipewire`, `bluez`, `bluez-utils`, `avahi`.
  - Mobile client: Flutter SDK, `flutter_blue_plus`, `web_socket_channel`, `mobile_scanner` (or PIN/QR entry), `shared_preferences`, `provider`/`riverpod`.
- **System Environment**: Creates user systemd services (`djm-box-daemon.service`, `djm-box-remote.service`), `/etc/udev/rules.d/99-djm-box.rules`, `/etc/systemd/logind.conf.d/djm-appliance.conf`, and WirePlumber pro-audio routing overrides.
