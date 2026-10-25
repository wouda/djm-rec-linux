# Spec Delta: daemon-ipc-and-fault-isolation

## Purpose

Provides a crash-resilient, dedicated core audio recording daemon with strictly isolated Unix Domain Socket IPC for telemetry broadcasting and control command execution without audio dropouts.

## ADDED Requirements

### Requirement: Fault Isolation and Dependency Hygiene
The recording daemon SHALL NOT link against, import, or invoke any Bluetooth, D-Bus, or GUI networking stacks.

#### Scenario: Bluetooth or UI service crash
- **WHEN** the BLE remote service or TUI monitor crashes, restarts, or hangs
- **THEN** the recording daemon SHALL continue audio capture and disk writes uninterrupted

### Requirement: Real-Time Process Scheduling
The recording daemon SHALL execute with elevated real-time FIFO scheduling when launched under systemd.

#### Scenario: Systemd service startup
- **WHEN** the daemon starts via `djm-box-daemon.service`
- **THEN** the operating system SHALL assign `CPUSchedulingPolicy=fifo` and `CPUSchedulingPriority=80`

### Requirement: Non-Blocking Telemetry Broadcast IPC
The daemon SHALL publish real-time telemetry frames over a Unix domain socket (`/run/user/$UID/djm-box.sock`) at a configurable rate between 30 Hz and 60 Hz using non-blocking broadcast channel semantics.

#### Scenario: Connected socket client slow or frozen
- **WHEN** a connected client fails to drain socket buffers in a timely manner
- **THEN** the daemon SHALL drop telemetry frames for that client or disconnect the client without blocking the audio capture thread or disk writer

### Requirement: Bidirectional Socket Command Handling
The daemon SHALL accept structured control commands (bookmark insertion, manual split trigger, hardware pad adjustment, auto-guard toggle, peak reset) over the Unix domain socket.

#### Scenario: Client sends bookmark command
- **WHEN** a client sends a valid `drop_bookmark` command over the socket
- **THEN** the daemon SHALL write the timestamped bookmark entry into the active session log and acknowledge the command over the socket

### Requirement: Configuration Loading and Hot Reloading
The daemon SHALL load configuration from `~/.config/djm-box/config.toml` upon launch and reload runtime parameters on `SIGHUP` or filesystem change notifications.

#### Scenario: User updates silence threshold in configuration file
- **WHEN** the user modifies `silence_threshold_dbfs` in `config.toml` while recording
- **THEN** the daemon SHALL reload the setting dynamically without interrupting the active audio recording stream
