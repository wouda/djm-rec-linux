# Spec Delta

## Purpose

Provides a cross-platform Flutter mobile companion application that connects to the recording appliance over local Wi-Fi LAN, direct laptop Wi-Fi Hotspot (`10.42.0.1`), or remote WebSocket endpoints without any Bluetooth dependencies or permissions.

## ADDED Requirements

### Requirement: Bluetooth-free mobile companion build and permissions
The mobile companion app SHALL NOT depend on `flutter_blue_plus` or request Android Bluetooth permissions (`BLUETOOTH`, `BLUETOOTH_ADMIN`, `BLUETOOTH_SCAN`, `BLUETOOTH_CONNECT`, `ACCESS_FINE_LOCATION`).

#### Scenario: Building the Android APK in CI
- **WHEN** `inject_permissions.py` and `flutter build apk --release` execute
- **THEN** `AndroidManifest.xml` contains only `INTERNET`, `ACCESS_NETWORK_STATE`, `VIBRATE`, and `CAMERA` permissions, and compiles without Bluetooth libraries

### Requirement: Multi-endpoint Wi-Fi and Hotspot WebSocket auto-connect chain
The mobile companion app SHALL automatically attempt WebSocket connections across the paired host/IP, the standard NetworkManager Wi-Fi Hotspot gateway (`10.42.0.1`), mDNS hostname, and optional remote URL.

#### Scenario: Connecting directly over the laptop's local Wi-Fi Hotspot
- **WHEN** the phone is connected to the laptop's `DJM-Box-Booth` Wi-Fi hotspot
- **THEN** `TransportManager` connects to `ws://10.42.0.1:8080/ws?token=<PIN>` (or the scanned QR host) and streams real-time telemetry at up to 60 FPS
