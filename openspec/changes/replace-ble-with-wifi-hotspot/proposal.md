# Proposal

## Why

Bluetooth Low Energy (BLE GATT) pairing and Web Bluetooth across mobile browsers and OS-level BlueZ D-Bus permissions introduce unnecessary complexity, unreliable browser support (e.g. iOS Safari lacking Web Bluetooth), and high-interference booth failure modes. Replacing Bluetooth completely with an automatic fallback local Wi-Fi Hotspot (`nmcli`) lets any phone or the Android Companion App connect directly to the laptop over high-speed Wi-Fi (WebSocket at 60 FPS) when venue Wi-Fi is unavailable, while simplifying dependencies across the entire stack.

## What Changes

- **BREAKING**: Remove all Bluetooth (BLE GATT) functionality from `djm-remote` (`ble_gatt.rs`, `bluer` crate dependency), `djm-common` (`ble_enabled`, `ble_device_name`, `ble_service_uuid`), `djm-cli` (`bluez` and `bluez-utils` system dependencies, `bluetooth.service` unit ordering), the embedded Web Dashboard (`connectBLE()` and Web Bluetooth UI button), and the Android Flutter Companion App (`flutter_blue_plus` dependency, BLE GATT transport fallback, and Android Bluetooth manifest permissions).
- Add **Local Wi-Fi Hotspot Management** (`hotspot.rs` in `djm-remote` and `djm-common`):
  - **Auto-fallback mode (`hotspot_mode = "auto"` by default)**: When `djm-remote` detects no active Wi-Fi or LAN connection, it automatically brings up a local NetworkManager (`nmcli`) Wi-Fi Hotspot (default SSID `DJM-Box-Booth`, gateway `10.42.0.1`, auto-generated 8-char WPA2 password stored in `config.toml`).
  - **Manual CLI & TUI controls**: Add `djm-box hotspot [on|off|status|auto]` CLI subcommand, and add `[H]` hotkey in `djm-tui` to toggle the local Wi-Fi hotspot on/off on demand.
- Add **Two-Step Toggleable QR Code Pairing** in `djm-tui` (`[P]` modal) and `djm-box pair`:
  - Display both Wi-Fi Hotspot credentials (SSID, WPA2 Password, active status) and the Web Dashboard URL (`http://10.42.0.1:8080/?token=<PIN>` when hotspot is active, or LAN IP / mDNS when on venue Wi-Fi).
  - Allow `[Tab]` / `[W]` inside the `djm-tui` `[P]` modal to toggle the rendered QR code between **Step 1: Join Wi-Fi Hotspot QR** (`WIFI:T:WPA;S:<SSID>;P:<PASSWORD>;;`) and **Step 2: Open Dashboard / Companion App QR** (`http://<ip>:8080/?token=<PIN>`).
- Update the **Flutter Companion App** (`flutter_app`) to use a pure Wi-Fi / WebSocket transport chain (prioritizing mDNS / paired IP, automatic `10.42.0.1` hotspot gateway fallback, and optional Remote/Tailscale URL).
- Update `README.md` architecture diagrams, feature list, CLI usage, and setup documentation to reflect the Wi-Fi Hotspot workflow and Bluetooth removal.

## Capabilities

### New Capabilities
- `wifi-hotspot-and-web-remote`: Local Wi-Fi Hotspot lifecycle management via NetworkManager (`nmcli`) with automatic offline fallback, manual CLI/TUI controls, two-step Wi-Fi + Dashboard QR pairing, and BLE-free Web Remote server.
- `wifi-only-mobile-companion`: Pure WebSocket mobile companion app supporting local LAN, direct laptop Wi-Fi hotspot (`10.42.0.1`), and remote URLs without Bluetooth dependencies or permissions.

### Modified Capabilities

## Impact

- **`crates/djm-common`**: Replace BLE fields in `RemoteConfig` and `PairingInfo` with Wi-Fi hotspot configuration (`hotspot_mode`, `hotspot_ssid`, `hotspot_password`), Wi-Fi QR formatting (`WIFI:T:WPA;...`), and `ToggleHotspot` control/CLI support.
- **`crates/djm-remote`**: Delete `src/ble_gatt.rs` and `bluer` dependency; add `src/hotspot.rs` managing `nmcli device wifi hotspot` with auto-fallback monitoring; remove Web BLE button and JS from `src/web_server.rs`.
- **`crates/djm-tui`**: Update `[P]` pairing modal (`src/ui.rs`, `src/app.rs`) with `[Tab]`/`[W]` QR toggle between Wi-Fi Hotspot QR and Dashboard URL QR, and `[H]` hotkey to toggle the hotspot.
- **`crates/djm-cli`**: Remove `bluez` and `bluez-utils` from `ApplianceSetup::PACKAGES` (`src/setup.rs`), add `networkmanager` check, remove `bluetooth.service` from `djm-box-remote.service`, add `djm-box hotspot` subcommand, and update `djm-box pair`.
- **`flutter_app`**: Remove `flutter_blue_plus` from `pubspec.yaml`, remove BLE permissions from `inject_permissions.py`, and update `transport_manager.dart`, `pairing_info.dart`, and `dashboard_screen.dart` to connect over Wi-Fi / Hotspot (`10.42.0.1`) / Remote WebSocket only.
- **`.github/workflows/release.yml` & `README.md`**: Remove `libdbus-1-dev` if unused, and update all documentation and architecture diagrams.
