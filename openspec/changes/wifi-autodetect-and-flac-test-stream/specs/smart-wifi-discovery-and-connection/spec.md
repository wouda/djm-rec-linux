# Spec Delta

## Purpose

Enables automatic background Wi-Fi and Hotspot discovery of the recording appliance, explicit visual separation between disconnected network state and unplugged mixer hardware, and pure Wi-Fi operation on Android.

## ADDED Requirements

### Requirement: Automatic background Wi-Fi and Hotspot probing
The mobile companion app SHALL probe standard appliance endpoints (`10.42.0.1:8080` for booth Hotspot and `djm-box.local:8080` for local LAN mDNS) automatically on launch without requiring manual host entry.

#### Scenario: Appliance detected with saved PIN
- **WHEN** the app starts or enters foreground while connected to the booth Hotspot or venue Wi-Fi and a valid auth PIN is in local storage
- **THEN** `TransportManager` automatically connects to the detected endpoint and begins streaming real-time telemetry

#### Scenario: Appliance detected without saved PIN
- **WHEN** the app detects an appliance responding on `10.42.0.1:8080` or `djm-box.local:8080` but has no valid stored PIN
- **THEN** the app presents a quick 1-tap dialog prompting for the 6-digit PIN while pre-filling the detected host

### Requirement: Clear visual distinction of appliance offline state
The companion dashboard SHALL prominently display when the mobile client is disconnected from the appliance, preventing confusion with mixer hardware disconnects.

#### Scenario: Client disconnected from recording appliance
- **WHEN** `activeTransport == disconnected` or `connecting`
- **THEN** the dashboard displays a prominent top alert card indicating appliance disconnection, suppresses the "No Pioneer DJM Mixer Detected" banner, and disables or dims action buttons

#### Scenario: Client connected but mixer unplugged
- **WHEN** `activeTransport == wifi` or `remote` and `telemetry.isMixerConnected == false`
- **THEN** the dashboard displays the USB-B mixer warning banner, enables pairing and system controls, and shows `○ IDLE`
