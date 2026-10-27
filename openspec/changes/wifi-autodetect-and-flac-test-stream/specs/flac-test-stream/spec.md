# Spec Delta

## Purpose

Provides secure 15-second lossless 24-bit FLAC audio stream preview and continuous 192kbps MP3 live streaming endpoints in the remote server, with dual player controls in Web and Android monitors to verify recording quality and monitor sets without touching the recording daemon.

## ADDED Requirements

### Requirement: Zero-daemon 15-second FLAC audio preview stream endpoint
The remote service SHALL expose an authenticated HTTP endpoint `GET /api/audio/test-stream` that streams up to 15 seconds of the active recording directly from disk in 24-bit FLAC format without transcoding.

#### Scenario: Requesting preview while actively recording to disk
- **WHEN** an authenticated client sends `GET /api/audio/test-stream?token=<PIN>` while `state == Recording` and `active_filename` exists
- **THEN** the server returns HTTP 200 with `Content-Type: audio/flac` and streams the most recent 15 seconds of the active FLAC file

#### Scenario: Requesting preview while idle or armed in RAM
- **WHEN** an authenticated client sends `GET /api/audio/test-stream?token=<PIN>` while `state != Recording` or `active_filename` is null
- **THEN** the server returns HTTP 409 Conflict with a JSON error payload indicating that no active recording file exists yet

#### Scenario: Unauthorized preview request
- **WHEN** an unauthenticated client requests `/api/audio/test-stream` with an invalid or missing token
- **THEN** the server returns HTTP 401 Unauthorized

### Requirement: Continuous lightweight MP3 live stream endpoint
The remote service SHALL expose an authenticated HTTP endpoint `GET /api/audio/live-stream` that continuously follows and streams the active recording transcoded into 192kbps MP3 format.

#### Scenario: Requesting continuous live stream during active recording
- **WHEN** an authenticated client sends `GET /api/audio/live-stream?token=<PIN>` while `state == Recording` and `active_filename` exists
- **THEN** the server returns HTTP 200 with `Content-Type: audio/mpeg` and continuously streams live audio until client disconnection

#### Scenario: Requesting continuous live stream while idle or armed
- **WHEN** an authenticated client sends `GET /api/audio/live-stream?token=<PIN>` while `state != Recording` or `active_filename` is null
- **THEN** the server returns HTTP 409 Conflict

#### Scenario: Unauthorized live stream request
- **WHEN** an unauthenticated client requests `/api/audio/live-stream` with an invalid or missing token
- **THEN** the server returns HTTP 401 Unauthorized

### Requirement: Interactive dual audio monitor controls
The Web and Android interfaces SHALL provide separate controls for the 15-second FLAC test preview and the continuous MP3 live stream, displaying playback status and providing immediate stop controls.

#### Scenario: Initiating 15-second test audio playback
- **WHEN** the user triggers the "Test (15s FLAC)" action during active recording
- **THEN** the client requests the FLAC stream, begins audio playback, displays a 15-second countdown timer, and exposes a "Stop" button

#### Scenario: Initiating continuous live stream
- **WHEN** the user triggers the "Live Stream (MP3)" action during active recording
- **THEN** the client requests the MP3 live stream, begins continuous audio playback, displays a pulsing "LIVE" elapsed indicator, and transforms the button into a "Stop Live Stream" button

#### Scenario: Stopping audio playback
- **WHEN** the user clicks "Stop" while either test preview or continuous live streaming is active
- **THEN** the client immediately halts audio output, aborts the HTTP request, and returns controls to their ready state
