# Spec Delta: flutter-mobile-companion-app

## Purpose

Defines the cross-platform Flutter companion mobile application with QR/PIN security pairing and Smart Multi-Transport auto-switching across Local Wi-Fi, Internet/VPN, and offline BLE GATT.

## ADDED Requirements

### Requirement: QR Code and PIN Security Pairing
The mobile app SHALL allow the user to pair with the appliance by scanning the `djm-tui` QR code or entering the 6-digit PIN, persisting the `auth_token` and connection endpoints securely on the device.

#### Scenario: User scans terminal QR code in booth
- **WHEN** the user scans the pairing QR code displayed in `djm-tui`
- **THEN** the app SHALL store the `auth_token`, local mDNS address, remote host URL, and BLE service UUID and immediately authenticate

### Requirement: Smart Multi-Transport Connection Manager
The mobile app SHALL automatically connect using the highest-bandwidth available transport in priority order: Local Wi-Fi WebSocket (`ws://djm-box.local:8080/ws`), Internet/Tailscale WebSocket, or Offline BLE GATT (`0000d900-...`).

#### Scenario: User walks from booth (BLE) to venue bar (Wi-Fi or 5G Tailscale)
- **WHEN** the phone leaves BLE range while connected to venue Wi-Fi or a Tailscale VPN
- **THEN** the app SHALL seamlessly maintain or switch telemetry streaming and control over the authenticated WebSocket connection

#### Scenario: Booth has no Wi-Fi or Internet connectivity
- **WHEN** WebSocket endpoints are unreachable in an offline basement booth
- **THEN** the app SHALL automatically connect to `DJM-Box-Recorder` via BLE GATT and transmit the `[0x00, ...auth_token]` handshake within 1 second

### Requirement: Binary GATT and JSON WebSocket Telemetry Unpacking
The mobile app SHALL unpack both 20-byte binary BLE notifications (15–20 Hz) and JSON WebSocket frames (60 Hz) into a unified reactive UI state model.

#### Scenario: Telemetry frame arrives from active transport
- **WHEN** a telemetry packet arrives over either BLE GATT or WebSocket
- **THEN** the app SHALL update recording state, pad dB, L/R peak and RMS levels, clip count, elapsed duration, file size, and free disk space

### Requirement: High-Contrast OLED Dark-Mode Booth UI
The mobile app UI SHALL feature high-contrast dark-mode graphics optimized for low-light DJ booths, displaying the active transport badge (`Wi-Fi`, `Remote`, or `BLE`), stereo VU meters with a `-19 dBFS` reference marker, and clip warnings.

#### Scenario: Audio clipping detected
- **WHEN** incoming telemetry indicates `clip_count > 0`
- **THEN** the mobile app SHALL illuminate a prominent red digital clip badge on the display

### Requirement: Authenticated Haptic-Feedback Control Actions
The mobile app SHALL provide interactive action buttons (`Bookmark Set`, `Split File Now`, `Reset Peaks`, `Hardware Pad`) that send authenticated commands over the active transport accompanied by tactile haptic feedback.

#### Scenario: User taps bookmark button
- **WHEN** the user taps the `[ + Bookmark Set ]` button on the mobile UI
- **THEN** the app SHALL trigger phone haptic feedback and send the authenticated bookmark command over the active WebSocket or BLE GATT transport
