# Spec Delta: cachyos-systemd-udev-setup

## Purpose

Automates appliance configuration for CachyOS and Arch Linux, setting up system packages, cryptographic auth tokens, udev mixer hotplug triggers, systemd services, PipeWire pro-audio routing, and laptop lid-close power policies.

## ADDED Requirements

### Requirement: Automated System Package Installation
The setup subcommand SHALL verify and install required Arch Linux packages using `yay` or `pacman` (`alsa-lib`, `alsa-utils`, `flac`, `pipewire`, `bluez`, `bluez-utils`, `avahi`).

#### Scenario: Running setup on fresh CachyOS install
- **WHEN** the user executes `djm-box setup`
- **THEN** the setup process SHALL detect missing system dependencies and invoke the package manager to install them

### Requirement: Initial Configuration and Auth Token Generation
The setup subcommand SHALL generate `~/.config/djm-box/config.toml` with club-safe silence defaults and a randomly generated `auth_token` if no configuration file exists.

#### Scenario: First-time setup execution
- **WHEN** `djm-box setup` runs on a host without an existing `config.toml`
- **THEN** it SHALL generate a cryptographically random `auth_token` and write the default configuration file with `0600` permissions

### Requirement: Udev Hotplug Trigger Rules
The setup subcommand SHALL install `/etc/udev/rules.d/99-djm-box.rules` matching USB vendor IDs `08e4` (Pioneer) and `2b73` (AlphaTheta/Pioneer DJ).

#### Scenario: Pioneer mixer USB insertion
- **WHEN** a supported DJ mixer is plugged into any USB port
- **THEN** udev SHALL trigger systemd to start or notify `djm-box-daemon.service`

### Requirement: Systemd User Services Configuration
The setup subcommand SHALL install and enable `djm-box-daemon.service` (with real-time FIFO priority) and `djm-box-remote.service` (BLE & Web bridge) in the user's systemd directory.

#### Scenario: User logs in or system boots
- **WHEN** user session starts
- **THEN** systemd SHALL supervise both user units with automatic restart policies (`Restart=always`)

### Requirement: WirePlumber Pro-Audio Mixer Isolation
The setup subcommand SHALL install `51-djm-pro-audio.conf` in WirePlumber configuration directories to prevent desktop audio services from grabbing or altering DJ mixer channel configurations.

#### Scenario: WirePlumber detects DJ mixer
- **WHEN** WirePlumber initializes audio nodes for a DJ mixer
- **THEN** it SHALL apply pro-audio routing profile rules leaving raw ALSA hardware streams dedicated to the recording daemon

### Requirement: Laptop Lid Sleep Prevention
The setup subcommand SHALL configure `/etc/systemd/logind.conf.d/djm-appliance.conf` to set `HandleLidSwitch=ignore`, `HandleLidSwitchExternalPower=ignore`, and `HandleLidSwitchDocked=ignore`.

#### Scenario: DJ closes laptop lid during live gig
- **WHEN** the physical laptop lid is closed while recording
- **THEN** the operating system SHALL remain fully awake, the display screen SHALL blank to save power, and audio recording threads SHALL continue uninterrupted

### Requirement: Automated Lid-Sleep & Power Profile Diagnostics
The setup subcommand and status CLI SHALL detect whether `/etc/systemd/logind.conf.d/djm-appliance.conf` is installed and warn the user if closing the lid will suspend the machine.

#### Scenario: Running setup or status check
- **WHEN** `djm-box setup` or `djm-box status` runs
- **THEN** it SHALL inspect the active logind drop-in and power profile and report whether lid-close sleep is inhibited

### Requirement: Dynamic Audio Performance Power Profile
The recording daemon SHALL switch the system power profile to `performance` via `powerprofilesctl` when audio recording starts, and revert to `balanced` when the session is stopped or idle.

#### Scenario: Audio starts streaming above silence threshold
- **WHEN** recording transitions from `ArmedSilent` to `Recording`
- **THEN** `djm-daemon` SHALL trigger `powerprofilesctl set performance` asynchronously without blocking audio capture threads
