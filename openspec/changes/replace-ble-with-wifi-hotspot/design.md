# Design

## Context

See `proposal.md` for motivation. Currently, `djm-remote` links against `bluer` (BlueZ D-Bus GATT peripheral) and advertises a 20-byte packed BLE telemetry characteristic alongside an Axum HTTP/WebSocket server (`:8080`). Similarly, `flutter_app` includes `flutter_blue_plus` and Android BLE scan/connect permissions. Removing Bluetooth and replacing offline booth connectivity with a local Wi-Fi Hotspot via NetworkManager (`nmcli`) unifies all remote monitoring onto a single high-frequency WebSocket/HTTP protocol (`0.0.0.0:8080`) that works identically across iOS Safari, Android Chrome, desktop browsers, and the Flutter Companion App.

## Goals / Non-Goals

**Goals:**
- Completely remove `bluer`, `ble_gatt.rs`, `flutter_blue_plus`, Web Bluetooth JS, and `bluez` package dependencies.
- Provide a clean `HotspotManager` built on NetworkManager (`nmcli`) that supports automatic offline fallback (`hotspot_mode = "auto"`) as well as manual CLI (`djm-box hotspot on|off|status`) and TUI (`[H]`) control.
- Provide two-step QR pairing in `djm-tui` (`[P]` modal with `[Tab]`/`[W]` toggle) so a user in a dark booth can scan QR #1 with their phone camera to join the laptop's Wi-Fi hotspot (`WIFI:T:WPA;S:<SSID>;P:<PASSWORD>;;`) and scan QR #2 to open the authenticated Web Dashboard or pair the Companion App (`http://10.42.0.1:8080/?token=<PIN>`).
- Automatically include `10.42.0.1` (NetworkManager's default shared hotspot IPv4 gateway) in IP detection and the Flutter Companion App's connection priority chain.

**Non-Goals:**
- Custom DHCP/hostapd daemon management outside NetworkManager (`nmcli` handles `wpa_supplicant` AP mode + `dnsmasq` subnet `10.42.0.1/24` natively on CachyOS/Arch Linux).
- Modifying the core `djm-daemon` real-time ALSA audio capture engine or Unix Domain Socket IPC protocol.

## Decisions

1. **Use NetworkManager (`nmcli`) for Local Wi-Fi Hotspot Management**
   - *Rationale*: CachyOS and Arch Linux desktop/laptop installs use NetworkManager (`nmcli`) by default, and `nmcli device wifi hotspot ifname <wlan> con-name djm-box-hotspot ssid <SSID> password <PASS>` sets up AP mode, WPA2 encryption, and a local `10.42.0.1/24` DHCP server in a single unprivileged Polkit-authorized D-Bus call without requiring custom `hostapd.conf` or `dnsmasq.conf` files.
   - *Alternatives considered*: Raw `hostapd` + `dnsmasq` (requires root privileges, conflicts with NetworkManager, and breaks normal desktop Wi-Fi usage when stopped).

2. **Auto-Fallback (`hotspot_mode = "auto"`) + Manual Override (`djm-box hotspot` & TUI `[H]`)**
   - *Rationale*: In `auto` mode, `djm-remote` checks every 10 seconds if the system has an active non-loopback IPv4 address (or active Wi-Fi client connection). If offline for 15 seconds, it automatically starts `djm-box-hotspot` so the laptop is immediately reachable at `10.42.0.1:8080`. Users who want the hotspot on even when Ethernet is plugged in—or who want to toggle it manually—can press `[H]` in `djm-tui` or run `djm-box hotspot on|off`.

3. **Two-Step Toggleable QR Code in `djm-tui` (`[P]` Modal)**
   - *Rationale*: Standard phone cameras natively recognize `WIFI:T:WPA;S:<SSID>;P:<PASSWORD>;;` QR codes to join a Wi-Fi network with one tap, and `http://10.42.0.1:8080/?token=<PIN>` QR codes to open the browser/app. Toggling between Step 1 (Wi-Fi QR) and Step 2 (Dashboard QR) with `[Tab]` or `[W]` keeps each QR code compact and easy to scan in a dark booth.

4. **Config Schema Evolution (`RemoteConfig` in `djm-common`)**
   - Replace `ble_enabled` and `ble_device_name` with:
     - `hotspot_mode`: `String` (`"auto"` default; `"auto" | "always" | "off"`)
     - `hotspot_ssid`: `String` (`"DJM-Box-Booth"` default)
     - `hotspot_password`: `String` (auto-generated 8-digit numeric/alphanumeric WPA2 password on initial config creation, since WPA2 requires 8–63 characters)
   - Keep `#[serde(default)]` on all fields so existing `config.toml` files deserialize cleanly without breaking.

## Risks / Trade-offs

- **[Risk] Laptop Wi-Fi card does not support AP mode or `nmcli` is absent** → *Mitigation*: `HotspotManager` logs a clear warning and continues running the Web Server on `0.0.0.0:8080` without crashing `djm-remote`.
- **[Risk] Starting Hotspot disconnects an in-progress Wi-Fi association attempt** → *Mitigation*: In `auto` mode, wait for an initial 15-second startup grace period and verify via `nmcli` that no Wi-Fi connection is in `connecting` or `connected` state before activating the hotspot.
