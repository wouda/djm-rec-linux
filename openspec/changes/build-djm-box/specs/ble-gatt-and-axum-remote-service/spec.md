# Spec Delta: ble-gatt-and-axum-remote-service

## Purpose

Provides a standalone, crash-isolated systemd bridge service that exposes an authenticated 15–20 Hz BLE GATT peripheral server and an authenticated Axum WebSocket/HTTP server for Wi-Fi and Internet monitoring.

## ADDED Requirements

### Requirement: Independent Remote Bridge Service Lifecycle
The remote service (`djm-remote`) SHALL execute as an isolated user process managed by systemd, completely independent of the core audio recording daemon process.

#### Scenario: BlueZ D-Bus daemon failure
- **WHEN** BlueZ or the Bluetooth controller encounters a D-Bus error or crashes
- **THEN** `djm-remote` may restart independently via systemd while `djm-daemon` continues recording audio without disruption

### Requirement: Bluetooth Authentication and Unauthorized Scanner Eviction
The remote service SHALL keep BlueZ non-pairable outside explicit pairing windows and require connected BLE clients to write `0x00` followed by the valid `auth_token` within 3.0 seconds or be forcibly disconnected.

#### Scenario: Unauthorized BLE scanner connects in club
- **WHEN** a third-party phone or BLE scanner connects to `DJM-Box-Recorder` without sending the valid `auth_token` within 3.0 seconds
- **THEN** `djm-remote` SHALL reject all commands, invoke BlueZ `disconnect()` on that device immediately, and resume peripheral advertising for the owner's phone

### Requirement: Compact 20-Byte BLE GATT Telemetry Notification
The remote service SHALL host a BLE GATT service (`0000d900-0000-1000-8000-00805f9b34fb`) publishing compact 20-byte binary telemetry notifications at 15 to 20 Hz to authenticated client devices.

#### Scenario: Authenticated mobile phone subscribes to telemetry characteristic
- **WHEN** an authenticated BLE client subscribes to the fast telemetry characteristic
- **THEN** the remote bridge SHALL transmit packed 20-byte frames containing state, hardware pad, peak/RMS centi-dB levels, clip counter, duration, file size, and disk space

### Requirement: JSON Metadata Characteristic Over BLE
The remote service SHALL expose a UTF-8 JSON characteristic providing mixer model name, sample rate, active filename, pause countdown seconds, and recent log events to authenticated BLE clients.

#### Scenario: Authenticated BLE client reads metadata characteristic
- **WHEN** an authenticated BLE client reads or receives notifications from the metadata characteristic
- **THEN** the remote bridge SHALL return a structured JSON string containing current mixer parameters and recent session log entries

### Requirement: Authenticated BLE GATT Write Command Dispatch
The remote service SHALL expose a GATT control characteristic that verifies session authentication before decoding command bytes and proxying them to the core daemon's Unix domain socket.

#### Scenario: Authenticated mobile client writes bookmark command
- **WHEN** an authenticated BLE client writes byte `0x01` to the control characteristic
- **THEN** the remote bridge SHALL relay the bookmark command to `/run/user/$UID/djm-box.sock` and respond with write success

### Requirement: Authenticated Axum Web Dashboard and WebSocket Server
The remote service SHALL host a token-authenticated dark-mode HTTP web dashboard and 60 Hz WebSocket server on `0.0.0.0:8080` accessible via local Wi-Fi mDNS (`djm-box.local`) and Internet/VPN (Tailscale).

#### Scenario: Authenticated client connects via Wi-Fi or Internet
- **WHEN** the mobile app or browser connects to `/ws` with a valid `auth_token` over LAN or Tailscale/Internet
- **THEN** the Axum server SHALL upgrade the connection and stream 60 Hz telemetry and accept authenticated control commands
