# Spec Delta

## Purpose

Standardizes the appliance authentication PIN to 125555 across active system configuration, default code fallbacks, and mobile companion hints.

## ADDED Requirements

### Requirement: Appliance active configuration PIN
The recording appliance SHALL authenticate remote web and WebSocket clients using the configured PIN value `125555` in its active configuration.

#### Scenario: Client connects with 125555
- **WHEN** a client provides auth token `125555` via query parameter or WebSocket auth frame
- **THEN** the appliance authorizes the connection and grants telemetry and control access

#### Scenario: Client connects with incorrect PIN
- **WHEN** a client connects with an incorrect auth PIN
- **THEN** the appliance rejects the request with HTTP 401 Unauthorized or closes the WebSocket

### Requirement: Default fallback and UI hint consistency
The system defaults and mobile companion placeholder hints SHALL reflect `125555` as the primary example PIN.

#### Scenario: New installation or unconfigured default
- **WHEN** the appliance initializes with default configuration values
- **THEN** `default_auth_token()` returns `125555`

#### Scenario: Mobile companion PIN input hints
- **WHEN** the user views the manual PIN entry fields in the companion application
- **THEN** the input hints and placeholders display `125555`
