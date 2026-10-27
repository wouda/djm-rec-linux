# Spec Delta

## Purpose

Enables the mobile companion application to automatically and manually discover reachable djm-box appliances across the local Wi-Fi subnet.

## ADDED Requirements

### Requirement: Local Wi-Fi subnet derivation
The companion application SHALL detect the device's active local non-loopback IPv4 address and compute the associated /24 subnet IP address range (1 to 254).

#### Scenario: Device connected to Wi-Fi network
- **WHEN** the application starts subnet discovery on a Wi-Fi connection with IP 192.168.1.45
- **THEN** it derives the target scan candidate range from 192.168.1.1 through 192.168.1.254

#### Scenario: No active Wi-Fi interface
- **WHEN** the device has no active non-loopback IPv4 network interface
- **THEN** subnet discovery terminates gracefully with an informative offline notification

### Requirement: Parallel subnet endpoint probing
The companion application SHALL probe candidate IP addresses on port 8080 targeting the `/api/status` endpoint in bounded parallel batches with short connection timeouts.

#### Scenario: Valid djm-box appliance responds on subnet
- **WHEN** an IP on the subnet responds with HTTP 200 or 401 containing valid djm-box status JSON
- **THEN** the application records the host as a verified reachable appliance

#### Scenario: Unresponsive or non-djm host
- **WHEN** an IP fails to connect within the 500ms timeout or returns non-djm responses
- **THEN** the application ignores the host and continues probing remaining candidates

### Requirement: Automatic and manual discovery triggers
The application SHALL perform subnet discovery automatically when candidate endpoints (10.42.0.1, djm-box.local, saved host) fail to connect, and SHALL provide a manual scan button in the user interface.

#### Scenario: Automatic fallback on failed direct connection
- **WHEN** the app fails to reach 10.42.0.1 and djm-box.local while disconnected
- **THEN** it automatically initiates the local subnet scan

#### Scenario: User triggers manual scan
- **WHEN** the user taps the "Scan Subnet" button in the Disconnected card or Pairing screen
- **THEN** the app immediately triggers a fresh subnet scan and displays scanning status

### Requirement: Appliance selection and auto-connection
The application SHALL automatically connect if a single appliance is discovered and a PIN is saved, or present discovered appliance options when multiple devices exist or credentials are required.

#### Scenario: Single appliance found with saved credentials
- **WHEN** exactly one djm-box host is discovered and a valid auth PIN is saved
- **THEN** the application automatically connects WebSocket telemetry to that host

#### Scenario: Multiple appliances found or no credentials
- **WHEN** multiple appliances are discovered or no PIN is saved
- **THEN** the application presents the discovered hosts for user selection and PIN entry
