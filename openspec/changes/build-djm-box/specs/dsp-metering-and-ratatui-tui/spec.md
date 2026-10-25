# Spec Delta: dsp-metering-and-ratatui-tui

## Purpose

Calculates low-overhead real-time audio telemetry metrics in the audio engine and provides an interactive terminal user interface monitor using Ratatui and crossterm with QR-code phone pairing.

## ADDED Requirements

### Requirement: Low-Overhead DSP Metering Calculation
The audio engine SHALL compute stereo Peak dBFS, RMS dBFS, and saturating digital clip counts over sliding analysis windows without interrupting the real-time audio thread.

#### Scenario: Audio frames processed by DSP stage
- **WHEN** PCM audio blocks pass through the capture pipeline
- **THEN** the metering block SHALL calculate peak and RMS levels in centi-dBFS and detect consecutive 0.0 dBFS samples as digital clips

### Requirement: Stateless Terminal Dashboard Connection
The `djm-tui` monitor SHALL connect to `/run/user/$UID/djm-box.sock` as an ephemeral, read-only consumer of telemetry frames.

#### Scenario: TUI launch and exit
- **WHEN** a user opens or closes the terminal monitor via `q` or `Ctrl+C`
- **THEN** the core recording daemon SHALL continue recording uninterrupted and without memory leakage

### Requirement: Dual Stereo Horizontal dBFS VU Meter Display
The TUI SHALL render horizontal stereo dBFS meters displaying Peak hold, RMS levels, and a visual marker for the `-19 dBFS` physical mixer 0 dB reference level.

#### Scenario: Live audio rendering in terminal
- **WHEN** telemetry frames arrive from the socket
- **THEN** the TUI SHALL refresh meter bars at up to 60 FPS displaying peak, RMS, clip status, and pause-split countdown timers

### Requirement: Keyboard-Driven Remote Appliance Controls
The TUI SHALL send corresponding command payloads over the Unix domain socket when hotkeys are pressed (`[`/`]` pad adjust, `A` auto-guard, `S` split, `T` pause timeout, `M` bookmark, `R` peak reset, `P` phone pairing).

#### Scenario: User presses manual split hotkey
- **WHEN** the user presses the `S` key in `djm-tui`
- **THEN** the TUI SHALL transmit a manual file split command to the daemon socket and display confirmation

### Requirement: Terminal QR Code and PIN Pairing Overlay
The TUI SHALL render a scannable Unicode QR code and 6-digit auth PIN modal when the user presses `P`, enabling one-scan phone pairing for both BLE and Wi-Fi/Internet transports.

#### Scenario: User opens phone pairing modal in booth
- **WHEN** the user presses `P` in `djm-tui`
- **THEN** the TUI SHALL open a 60-second BLE pairing window and render a high-contrast Unicode QR code containing the appliance host URL, BLE service UUID, and `auth_token`
