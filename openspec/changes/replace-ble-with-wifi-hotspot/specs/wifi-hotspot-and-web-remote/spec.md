# Spec Delta

## Purpose

Provides local Wi-Fi Hotspot lifecycle management via NetworkManager (`nmcli`) with automatic offline fallback, manual CLI/TUI controls, two-step Wi-Fi and Dashboard QR code pairing, and an authenticated Web Remote dashboard free of Bluetooth dependencies.

## ADDED Requirements

### Requirement: Complete removal of Bluetooth Low Energy (BLE) dependencies and services
The system SHALL NOT include or depend on Bluetooth Low Energy (BLE GATT) libraries, system packages (`bluez`, `bluez-utils`), systemd `bluetooth.service` ordering, or Web Bluetooth client code.

#### Scenario: Appliance setup package verification
- **WHEN** the user runs `djm-box setup`
- **THEN** the system checks for `alsa-lib`, `alsa-utils`, `flac`, `pipewire`, `networkmanager`, and `avahi`, and does NOT require `bluez` or `bluez-utils`

#### Scenario: Web dashboard renders without Bluetooth controls
- **WHEN** a user opens the Web Remote Dashboard in a browser
- **THEN** the UI connects strictly via WebSocket/HTTP and does NOT display a "Connect via Bluetooth (Web BLE)" button

### Requirement: Automatic fallback local Wi-Fi Hotspot when offline
When `hotspot_mode` is configured as `"auto"` (the default), the remote service SHALL monitor network connectivity and automatically start a local Wi-Fi Hotspot via NetworkManager (`nmcli`) if no active Wi-Fi or LAN connection is available, allowing direct phone-to-laptop Wi-Fi connections in offline club booths.

#### Scenario: Automatic hotspot activation when no Wi-Fi or LAN is connected
- **WHEN** `djm-remote` is running with `hotspot_mode = "auto"` and the machine has no active global IPv4 Wi-Fi/LAN connection
- **THEN** `djm-remote` activates a local NetworkManager Wi-Fi hotspot using the configured `hotspot_ssid` (default `"DJM-Box-Booth"`) and `hotspot_password` (8+ character WPA2 password) on gateway `10.42.0.1`

#### Scenario: Existing venue Wi-Fi connection is preserved
- **WHEN** `djm-remote` is running with `hotspot_mode = "auto"` and the laptop is already connected to a venue or home Wi-Fi network
- **THEN** `djm-remote` does NOT override the active Wi-Fi connection and serves the Web Remote Dashboard on the existing LAN IP

### Requirement: Manual Wi-Fi Hotspot CLI and TUI controls
The system SHALL allow operators to manually start, stop, or query the local Wi-Fi Hotspot via `djm-box hotspot` CLI subcommands (`on`, `off`, `status`) and via the `[H]` hotkey in `djm-tui`.

#### Scenario: Starting the hotspot manually from CLI
- **WHEN** the user runs `djm-box hotspot on`
- **THEN** the system starts the NetworkManager Wi-Fi hotspot with the configured SSID and WPA2 password and prints the hotspot credentials and gateway URL (`http://10.42.0.1:8080/?token=<PIN>`)

#### Scenario: Stopping the hotspot manually from CLI
- **WHEN** the user runs `djm-box hotspot off`
- **THEN** the system brings down the active `djm-box` Wi-Fi hotspot connection

#### Scenario: Toggling the hotspot from TUI
- **WHEN** the user presses `[H]` in `djm-tui`
- **THEN** `djm-tui` toggles the local Wi-Fi hotspot on or off and updates the pairing endpoint (`10.42.0.1` when active)

### Requirement: Two-step toggleable QR code pairing in TUI and CLI
The system SHALL provide both the Wi-Fi Hotspot connection QR code (`WIFI:T:WPA;S:<SSID>;P:<PASSWORD>;;`) and the Web Dashboard URL QR code (`http://<host>:8080/?token=<PIN>`) so a phone can join the laptop's Wi-Fi and open the authenticated dashboard without typing credentials in a dark booth.

#### Scenario: Toggling QR codes in the TUI pairing modal
- **WHEN** the user opens the `[P]` pairing modal in `djm-tui` and presses `[Tab]` or `[W]`
- **THEN** the modal switches the displayed QR code between "Step 1: Join Wi-Fi Hotspot QR" (`WIFI:T:WPA;...`) and "Step 2: Open Dashboard / App QR" (`http://<host>:8080/?token=<PIN>`), while displaying the Hotspot SSID, WPA2 Password, Dashboard URL, and 6-digit PIN in text

#### Scenario: Displaying pairing info in CLI
- **WHEN** the user runs `djm-box pair`
- **THEN** the CLI outputs the Direct Wi-Fi URL, Hotspot Gateway URL (`http://10.42.0.1:8080/?token=<PIN>`), Hotspot SSID and WPA2 Password, and Auth PIN without any BLE fields
