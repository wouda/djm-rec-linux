# Spec Delta

## Purpose

Exposes live Pro DJ Link deck monitoring and the interactive Post-Event Studio (timetable entry, split review, transition preview, render configuration, job progress, and report viewing) across the Web Dashboard, Flutter Companion App, TUI, and CLI.

## ADDED Requirements

### Requirement: Live Now-Playing Deck Display
The Web Dashboard, Flutter app, and TUI SHALL show live deck cards whenever `djm-prodj` provides telemetry.

#### Scenario: Active decks during an event
- **WHEN** `djm-prodj` streams deck telemetry
- **THEN** each card shows player number, On-Air state, title, artist, BPM, and USB media name, or "unavailable" for fields `djm-prodj` cannot obtain

### Requirement: Anytime Timetable Input and USB Mapping
The Web Dashboard and Flutter app SHALL let users enter or edit a plain-text timetable (`HH:MM Artist`) at any time and map detected USB media to artists.

#### Scenario: Timetable entered during recording
- **WHEN** a timetable is saved while `djm-daemon` is recording
- **THEN** it is stored for the session and analysis and render actions stay disabled

#### Scenario: USB media assignment
- **WHEN** USB clusters are detected for a session
- **THEN** the UI shows each cluster next to the timetable artists and lets the user confirm or reassign it

### Requirement: Split Review and Transition Preview
The Studio SHALL show a review table of boundaries with detection method, silence duration, and measured metrics, and SHALL offer a 15-second preview centered on each boundary.

#### Scenario: Previewing a boundary
- **WHEN** the user selects `Preview Transition` on a row
- **THEN** `djm-remote` streams a 15-second FLAC snippet centered on that boundary

#### Scenario: Adjusting a boundary
- **WHEN** the user edits a boundary timestamp
- **THEN** adjacent set durations update before rendering

### Requirement: Session Identifier Validation
`djm-remote` MUST accept only session identifiers present in its discovered session list and MUST reject any parameter that resolves outside the recordings directory.

#### Scenario: Path traversal attempt
- **WHEN** a studio request contains a session value not in the discovered list or containing path separators
- **THEN** `djm-remote` responds with an error and opens no file

### Requirement: Preview Concurrency Limit
`djm-remote` SHALL limit concurrent transition preview streams (default 2).

#### Scenario: Too many previews
- **WHEN** a preview is requested while the limit is reached
- **THEN** `djm-remote` responds with a busy error without starting a new subprocess

### Requirement: Persistent Render Jobs and Progress
The Web Dashboard, Flutter app, and `djm-box process` SHALL let users choose outputs and a preset, follow progress, and view the report; job state SHALL survive a `djm-remote` restart.

#### Scenario: Rendering from the app
- **WHEN** the user selects outputs and a preset and starts rendering
- **THEN** progress streams over WebSocket and the report opens on completion

#### Scenario: djm-remote restarts during a job
- **WHEN** `djm-remote` restarts while a job is running
- **THEN** on restart the job is shown as `Interrupted` with its last completed step and can be restarted

#### Scenario: Rendering from the CLI
- **WHEN** the user runs `djm-box process` with a session and optional timetable file
- **THEN** the CLI runs analysis, rendering, tracklist export, and report generation with terminal progress
