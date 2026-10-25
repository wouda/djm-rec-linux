# djm-rec-linux 🎛️

[![Release](https://img.shields.io/github/v/release/wouda/djm-rec-linux?color=brightgreen&label=Latest%20Release)](https://github.com/wouda/djm-rec-linux/releases/latest)
[![License](https://img.shields.io/badge/License-Proprietary-blue.svg)](LICENSE)

> [!NOTE]
> **Notice**: `djm-rec-linux` is developed specifically for recording an upcoming [craterdnb.com](https://craterdnb.com) event. All rights reserved. Totally vibe coded. ⚡🎧

**Autonomous, bulletproof DJ mixer recording appliance for Pioneer and AlphaTheta club mixers.**

Designed for Linux (CachyOS / Arch Linux), `djm-rec-linux` transforms a laptop, mini PC, or Raspberry Pi into a dedicated set-and-forget recording box. It automatically detects when a Pioneer DJ mixer is connected over USB-B, engages hardware attenuation pads, unmutes the microphone into the REC OUT stream, buffers pre-roll audio in RAM, and records bit-perfect, crash-resilient 24-bit FLAC audio.

---

## Key Features

- **Automated USB-B Mixer Detection**: Instantly identifies connected Pioneer & AlphaTheta mixers (`DJM-900NXS2`, `DJM-V10`, `DJM-A9`, `DJM-900NXS`, `DJM-850`, `DJM-750MK2`, `DJM-450`, `DJM-250MK2`).
- **Pristine Master Music by Default**: Automatically unlinks mixer microphone talk-over from REC OUT master stream so the DJ recording is 100% clean music.
- **Dedicated DJM Microphone Recording**: Optionally captures the mixer microphone into an isolated, parallel FLAC stream (`..._djm_mic.flac`).
- **Simultaneous Ambient Crowd Recording**: Records room energy and crowd chatter via the recording device's built-in or USB microphone into a synchronized FLAC file (`..._ambient.flac`), with sample-accurate DAW alignment metadata.
- **Post-Set Trailing Hangover (15m Default)**: Keeps ambient room recording active after the mixer stops playing to capture after-hours closing atmosphere and applause.
- **Hardware Headroom Protection (-19 dB)**: Configures ALSA hardware `Capture Level` to `-19 dB`, matching the physical mixer 0 dB reference line and leaving 19 dB of digital headroom for club DJs driving channels into the red.
- **Auto-Guard Dynamic Attenuation**: Continuously monitors signal peaks; if a DJ exceeds `-0.5 dBFS`, `Auto-Guard` instantly steps down gain toward `-19 dB` and logs the exact timestamp in the session audit log.
- **Silence Gate & 2.0s Circular RAM Pre-Roll Buffer**: Audio below `-65 dBFS` is buffered in RAM without creating empty files. When the first track drops, the pre-roll buffer is prepended to disk so the first beat is never clipped.
- **Laptop Closed-Lid Operation**: Automatically configures `/etc/systemd/logind.conf.d/djm-appliance.conf` to ignore lid-close events. DJs can close the laptop lid in the booth; the screen blanks to save power while audio recording and telemetry continue uninterrupted.
- **Dynamic Performance Power Profiles**: Switches the system to `performance` mode via `powerprofilesctl` when recording begins to prevent CPU frequency scaling latency/xruns, and reverts to `balanced` when idle.
- **Power-Loss & Crash Resilience**: Staged `.flac.part` files, 2-second `fdatasync()`, 30-second `STREAMINFO` header checkpoints, and automatic boot-time repair guarantee playable audio even if the power cable is pulled mid-set.
- **Fault-Isolated Multi-Process Architecture**: Audio capture (`djm-daemon`), terminal UI (`djm-tui`), and remote bridges (`djm-remote`) run in completely separate OS processes communicating over a non-blocking Unix Domain Socket. A crash or lag in the UI or network never drops a single audio frame.
- **Multi-Transport Remote Monitoring**:
  - **Local Web Dashboard (`http://<ip>:8080`)**: Clean dark-mode UI with dual stereo VU meters, file stats, and remote actions (Split, Bookmark, Lock -19dB).
  - **Web Bluetooth (BLE GATT)**: Offline monitoring using 20-byte packed binary telemetry at 15–20 Hz when Wi-Fi is unavailable in the booth.
  - **QR Code & 6-Digit PIN Pairing**: Authenticates remote clients securely.

---

## Architecture

```
                   +-----------------------------------------------+
                   |          PIONEER / ALPHATHETA MIXER           |
                   |    (DJM-900NXS2, DJM-V10, DJM-A9, etc.)       |
                   +-----------------------+-----------------------+
                                           | USB-B (Ch 5/6 REC OUT + Mic)
                                           v
+-----------------------------------------------------------------------------------------+
| djm-daemon (Systemd User Service: djm-box-daemon.service)                               |
|                                                                                         |
|  +--------------------+        +---------------------+        +-----------------------+ |
|  | Native ALSA Capture| -----> | Lock-Free SPSC Ring | -----> | FLAC Staged Writer    | |
|  | (96kHz/48kHz 24bit)|        | Buffer + 2s PreRoll |        | (.flac.part + fdatasync|
|  +--------------------+        +---------------------+        +-----------+-----------+ |
|                                                                           |             |
|  +--------------------+        +---------------------+                    v             |
|  | Hardware Controls  |        | Power Profiles      |         +----------------------+ |
|  | (-19dB Pad + Mic)  |        | (Performance Mode)  |         | Session Audit Log    | |
|  +--------------------+        +---------------------+         | (*.log with MB/min)  | |
+-------------------------------------------+---------------------------------------------+
                                            | Non-Blocking Unix Socket (/run/user/1000/djm-box.sock)
                   +------------------------+------------------------+
                   |                                                 |
                   v                                                 v
+---------------------------------------+         +---------------------------------------+
| djm-tui (Terminal Dashboard)          |         | djm-remote (djm-box-remote.service)   |
| - Header with Mixer & Record status   |         | - Web Remote HTTP & WebSocket (:8080) |
| - High-precision Stereo VU Meters     |         | - BLE GATT Peripheral (20-byte stream)|
| - Hotkeys: Bookmark, Split, Pad, QR   |         | - PIN Authentication Watchdog         |
+---------------------------------------+         +-------------------+-------------------+
                                                                      |
                                                                      v
                                                  +---------------------------------------+
                                                  | Phone Browser / Web Bluetooth         |
                                                  | http://djm-box.local:8080             |
                                                  +---------------------------------------+
```

---

## Installation & Setup

### 1. Download Pre-built Release (Recommended)

Download the latest pre-compiled bundle from the **[GitHub Releases](https://github.com/wouda/djm-rec-linux/releases/latest)** page:

- **Linux x86_64** (Laptops, Mini PCs, Desktops): [djm-rec-linux-v0.2.0-x86_64.tar.gz](https://github.com/wouda/djm-rec-linux/releases/latest/download/djm-rec-linux-v0.2.0-x86_64.tar.gz)
- **Linux aarch64** (Raspberry Pi 4 / 5): [djm-rec-linux-v0.2.0-aarch64.tar.gz](https://github.com/wouda/djm-rec-linux/releases/latest/download/djm-rec-linux-v0.2.0-aarch64.tar.gz)

```bash
# Extract and run setup
tar -xzvf djm-rec-linux-*.tar.gz
cd djm-rec-linux-*/bin
./djm-box setup
```

### 2. Build from Source (Alternative)
```bash
# Clone the repository
git clone https://github.com/wouda/djm-rec-linux.git
cd djm-rec-linux

# Build release binaries
cargo build --release
```

### 3. Run Appliance Setup
The `setup` command automatically installs system dependencies (`alsa-lib`, `flac`, `pipewire`, `bluez`, `avahi`), installs binaries to `~/.cargo/bin/`, sets up systemd user services, configures WirePlumber pro-audio rules, and ensures lid-close sleep is disabled:
```bash
./target/release/djm-box setup
```

### 4. Start Background Services
```bash
systemctl --user enable --now djm-box-daemon djm-box-remote
```

---

## CLI Usage

### Check Appliance Status
```bash
djm-box status
```
```text
=== djm-box v0.2.0 Appliance Status ===
Mixer Detection:  ✓ DETECTED (Pioneer DJM-900NXS2 @ 96000 Hz)
Recording Status: ● ACTIVELY RECORDING TO DISK (File: Pioneer_DJM-900NXS2_2026-10-25_17-30-00_pt01.flac)
Mic Routing:      REC OUT (Master + Mic captured via ALSA unmuted routing)
Lid-Close Action: Stay Awake (Screen blanks, recording continues)
Hardware Pad:     -19 dB (Club DJ headroom attenuation)
Auto-Guard:       Active (Auto-attenuate on redline)
Clips:            0 (Clean Signal)
Active File:      Pioneer_DJM-900NXS2_2026-10-25_17-30-00_pt01.flac
Duration & Size:  840s (00:14:00), 185 MB
Free Storage:     457.5 GB (~416 hours remaining)

--- Incoming Audio Levels & Gain Staging ---
L Peak: [█████████████████████│░░░░░░░░]   -18.4 dBFS  (RMS: -22.1 dBFS)
R Peak: [█████████████████████│░░░░░░░░]   -17.9 dBFS  (RMS: -21.8 dBFS)
         -60 dBFS             ▲ -19 dB (Mixer 0 dB)    0 dBFS (Clip)
Gain Headroom:    17.9 dB to digital 0 dBFS | +1.1 dB vs physical mixer 0 dB reference
```

### Live Terminal Audio Monitor
Streams live VU meters and headroom indicators directly in the terminal:
```bash
djm-box monitor
```

### Launch Interactive TUI
```bash
djm-box tui
```
* **[P]**: Display QR code for phone pairing
* **[S]**: Split current recording segment immediately
* **[M]**: Drop a timestamped bookmark in the session log
* **[ [ / ] ]**: Step hardware pad attenuation up/down
* **[A]**: Toggle Auto-Guard headroom protection
* **[R]**: Reset digital peak hold

### Remote Booth Web App
Open `http://<ip-of-appliance>:8080` in any phone browser. Enter your 6-digit PIN (shown via `djm-box pair` or in `~/.config/djm-box/config.toml`).

---

## Output Files & Audit Logs

Recordings are saved to `~/Music/DJ_Recordings/` (configurable in `config.toml`):
- **FLAC Files**: `Pioneer_DJM-900NXS2_YYYY-MM-DD_HH-MM-SS_pt01.flac`
- **Session Audit Logs**: `Pioneer_DJM-900NXS2_YYYY-MM-DD_HH-MM-SS.log` containing start timestamp, sample rate, hardware pad, periodic status updates (written MB and duration), bookmarks, digital clipping warnings, and clean termination summaries.

---

## License

All Rights Reserved. Developed specifically for recording an upcoming [craterdnb.com](https://craterdnb.com) event — totally vibe coded.
