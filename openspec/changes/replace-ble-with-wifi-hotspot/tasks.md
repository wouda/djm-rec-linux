# Tasks

## 1. Common Configuration, Pairing Protocol & Hotspot Helpers (`djm-common`)

- [x] 1.1 Update `RemoteConfig` in `crates/djm-common/src/config.rs` to replace `ble_enabled` and `ble_device_name` with `hotspot_mode` (`"auto"` default), `hotspot_ssid` (`"DJM-Box-Booth"` default), and `hotspot_password` (auto-generated 8-character WPA2 password), and verify `cargo test -p djm-common` passes
- [x] 1.2 Update `PairingInfo` and IP detection in `crates/djm-common/src/protocol.rs` to remove `ble_service_uuid`/BLE constants, add `hotspot_ssid`, `hotspot_password`, `hotspot_active`, and `to_wifi_qr_payload()` (`WIFI:T:WPA;S:<SSID>;P:<PASSWORD>;;`), and add `HotspotControl` helpers (`start_hotspot`, `stop_hotspot`, `is_hotspot_active`, `has_active_network`) in `djm-common`, verifying unit tests in `crates/djm-common` pass

## 2. Remote Service BLE Removal & Auto-Hotspot Manager (`djm-remote`)

- [x] 2.1 Remove `bluer` from `crates/djm-remote/Cargo.toml`, delete `crates/djm-remote/src/ble_gatt.rs`, and remove BLE module exports in `crates/djm-remote/src/lib.rs` and `main.rs`, verifying `djm-remote` compiles without `bluer`
- [x] 2.2 Implement `HotspotWatchdog` in `crates/djm-remote/src/hotspot.rs` that honors `hotspot_mode` (`"auto"`, `"always"`, `"off"`) and automatically brings up the NetworkManager Wi-Fi hotspot (`10.42.0.1`) when no active Wi-Fi/LAN connection is detected, verifying unit tests for hotspot state evaluation pass
- [x] 2.3 Remove the "Connect via Bluetooth (Web BLE)" button and `connectBLE()` JavaScript from `crates/djm-remote/src/web_server.rs` and verify `cargo test -p djm-remote` passes

## 3. TUI Two-Step QR Pairing & Hotspot Toggle (`djm-tui` & `djm-cli`)

- [x] 3.1 Update `crates/djm-tui/src/app.rs` and `crates/djm-tui/src/ui.rs` to add `[H]` hotkey for toggling the local Wi-Fi Hotspot on/off and `[Tab]`/`[W]` inside the `[P]` pairing modal to switch between Step 1 ("Join Wi-Fi Hotspot QR") and Step 2 ("Open Web Dashboard / App QR"), verifying `cargo test -p djm-tui` passes
- [x] 3.2 Update `crates/djm-cli/src/setup.rs` and `crates/djm-cli/src/main.rs` to replace `bluez`/`bluez-utils` with `networkmanager`, remove `bluetooth.service` from `djm-box-remote.service`, update `djm-box pair` output, and add `djm-box hotspot [on|off|status]`, verifying `cargo test -p djm-cli` passes

## 4. Bluetooth-Free Mobile Companion App (`flutter_app`)

- [x] 4.1 Remove `flutter_blue_plus` from `flutter_app/pubspec.yaml` and strip Bluetooth permissions from `flutter_app/inject_permissions.py`, verifying `python3 inject_permissions.py` only injects network, vibrate, and camera permissions
- [x] 4.2 Update `flutter_app/lib/models/pairing_info.dart`, `flutter_app/lib/services/transport_manager.dart`, and `flutter_app/lib/screens/dashboard_screen.dart` to remove BLE scanning/GATT code and add `10.42.0.1` (Hotspot gateway) to the WebSocket auto-connect priority chain, verifying `flutter test` passes

## 5. CI Workflow, Documentation & Workspace Verification

- [x] 5.1 Update `.github/workflows/release.yml` (remove `libdbus-1-dev` if no longer needed by Rust build) and update `README.md` (Key Features, ASCII Architecture diagram, Setup dependencies, CLI `djm-box hotspot` & TUI `[H]`/`[P]` docs) to replace Bluetooth references with the local Wi-Fi Hotspot workflow, then run `cargo test --workspace` and `openspec validate replace-ble-with-wifi-hotspot` to verify all checks pass
