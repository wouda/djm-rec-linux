# djm-rec-linux 🎛️

[![Website](https://img.shields.io/badge/Website-djm--rec--linux.wouda.eu-blue.svg)](https://djm-rec-linux.wouda.eu/)
[![Release](https://img.shields.io/github/v/release/wouda/djm-rec-linux?color=brightgreen&label=Latest%20Release)](https://github.com/wouda/djm-rec-linux/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

> [!NOTE]
> **Hardware Status**: Tested and confirmed working on **Pioneer DJM-V10** and **DJM-A9**!
>
> Live recorded a [craterdnb.com](https://craterdnb.com) event.

🌐 **Website**: [https://djm-rec-linux.wouda.eu/](https://djm-rec-linux.wouda.eu/)

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
- **Pioneer Pro DJ Link Live Deck Telemetry**: Dedicated, fault-isolated daemon (`djm-prodj`) connects to Pioneer Pro DJ Link over Ethernet to track CDJ On-Air status, BPM, playhead advance, USB sticks, and B2B alternating clusters with passive settling, zero contestation, and automatic virtual slot release.
- **Post-Event Audio Mastering Studio**: Built directly into the CLI (`djm-box process`), web dashboard, and companion app. Automatically audits 3-hour rollovers, resolves set boundaries (silence midpoints, USB handovers, timetable clocks), normalizes loudness (-14 LUFS Streaming, -11 LUFS Club), applies 4× oversampled true peak limiting and de-clipping, and exports tracklists, cue sheets, and comprehensive quality reports (`Event_Report.{html,md,json}`).
- **Fault-Isolated Multi-Process Architecture**: Audio capture (`djm-daemon`), Pro DJ Link (`djm-prodj`), terminal UI (`djm-tui`), and remote bridges (`djm-remote`) run in completely separate OS processes communicating over non-blocking Unix Domain Sockets. A crash or lag in the UI or network never drops a single audio frame.
- **Multi-Transport Remote Monitoring & Bit-Perfect Verification**:
  - **15-Second Lossless FLAC Audio Preview**: Tap "Test Audio (15s)" in the web or mobile app to stream a live 15-second snippet of the active 24-bit FLAC recording directly from disk without touching or risking the recording daemon.
  - **Local Web Dashboard (`http://<ip>:8080`)**: Clean dark-mode UI with dual stereo VU meters, live Pro DJ Link deck cards, file stats, FLAC audio preview player with live countdown bar, and the interactive **Post-Event Mastering Studio** tab.
  - **Smart Wi-Fi Autodetection & Companion App**: Mobile companion app automatically discovers the booth hotspot (`10.42.0.1:8080`) and LAN mDNS (`djm-box.local:8080`) on launch, connects with a saved PIN or 1-tap PIN modal, displays live deck cards (including Degraded Mode fallbacks), and provides mobile set splitting & mastering.
  - **Direct Local Wi-Fi Hotspot (`10.42.0.1`)**: Automatically starts a local NetworkManager Wi-Fi hotspot (`DJM-Box-Booth`) when no venue Wi-Fi/LAN is connected, giving phones full 60 FPS WebSocket telemetry directly in offline club booths.
  - **Two-Step QR Code & 6-Digit PIN Pairing**: Scan Step 1 with a phone camera to join the hotspot, and scan Step 2 to open the dashboard or companion app instantly.

---

## Architecture

```
                   +-----------------------------------------------+
                   |          PIONEER / ALPHATHETA BOOTH           |
                   |   (DJM-900NXS2, DJM-V10, DJM-A9, CDJ-3000s)   |
                   +--------------+-----------------+--------------+
                                  |                 |
                   USB-B (Audio)  |                 |  Ethernet RJ-45 (Link UDP)
                                  v                 v
+------------------------------------+   +------------------------------------+
| djm-daemon (djm-box-daemon.service)|   | djm-prodj (djm-box-prodj.service)  |
| - ALSA Capture (96/48 kHz, 24-bit) |   | - Passive UDP Discovery & Settling |
| - SPSC Ring Buffer + 2s PreRoll    |   | - 4/6-Player Virtual Arbitration   |
| - Staged FLAC Writer (.flac.part)  |   | - On-Air & B2B Cluster Tracking    |
| - Auto-Guard & Hardware Pad Control|   | - Session Log (*_prodj.jsonl)      |
+------------------+-----------------+   +------------------+-----------------+
                   |                                        |
                   | /run/user/1000/djm-box.sock            | /run/user/1000/djm-prodj.sock
                   +-------------------+--------------------+
                                       |
                   +-------------------+--------------------+
                   |                                        |
                   v                                        v
+------------------------------------+   +------------------------------------+
| djm-tui (Terminal Dashboard)       |   | djm-remote (djm-box-remote.service)|
| - Live VU Meters & Headroom dB     |   | - Web Remote HTTP & WS (:8080)     |
| - CDJ Deck Cards (On-Air / BPM)    |   | - Post-Event Studio API & Template |
| - QR Code & Hotspot Controls       |   | - Auto Local Wi-Fi Hotspot (10.42) |
+------------------------------------+   +------------------+-----------------+
                                                            |
                                                            v
+------------------------------------+   +------------------------------------+
| djm-box process (Mastering Studio) |   | Phone Browser / Companion App      |
| - Seam Continuity & Alignment Audit|   | - Live Meters & On-Air Deck Cards  |
| - Boundary Resolver (Silence / USB)|   | - 15s Test FLAC & Live MP3 Stream  |
| - EBU R128 (-14 / -11 LUFS) Master |   | - Mobile Post-Event Studio Screen  |
| - HTML / Markdown / JSON Reports   |   |   (Timetable, Splits, Master, Rep) |
+------------------------------------+   +------------------------------------+
```

---

## Installation & Setup

### 1. Download Pre-built Release (Recommended)

Download the latest pre-compiled bundle from the **[GitHub Releases](https://github.com/wouda/djm-rec-linux/releases/latest)** page:

- **Linux x86_64** (Laptops, Mini PCs, Desktops): [djm-rec-linux-v0.2.8-x86_64.tar.gz](https://github.com/wouda/djm-rec-linux/releases/latest/download/djm-rec-linux-v0.2.8-x86_64.tar.gz)
- **Linux aarch64** (Raspberry Pi 4 / 5): [djm-rec-linux-v0.2.8-aarch64.tar.gz](https://github.com/wouda/djm-rec-linux/releases/latest/download/djm-rec-linux-v0.2.8-aarch64.tar.gz)
- **Android Companion App**: [djm-box-companion-v0.2.8.apk](https://github.com/wouda/djm-rec-linux/releases/latest/download/djm-box-companion-v0.2.8.apk)

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
The `setup` command automatically installs system dependencies (`alsa-lib`, `flac`, `pipewire`, `networkmanager`, `avahi`, `ffmpeg`), installs binaries to `~/.cargo/bin/`, sets up systemd user services (`djm-box-daemon`, `djm-box-remote`, `djm-box-prodj`), configures WirePlumber pro-audio rules, and ensures lid-close sleep is disabled:
```bash
./target/release/djm-box setup
```

### 4. Start Background Services
```bash
systemctl --user enable --now djm-box-daemon djm-box-remote djm-box-prodj
```

---

## CLI Usage

### Check Appliance Status
```bash
djm-box status
```
```text
=== djm-box v0.2.8 Appliance Status ===
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
* **[P]**: Open phone pairing modal (press **[Tab]** or **[W]** to toggle between Wi-Fi Hotspot QR and Web Dashboard QR)
* **[H]**: Toggle local NetworkManager Wi-Fi Hotspot on/off on demand
* **[S]**: Split current recording segment immediately
* **[M]**: Drop a timestamped bookmark in the session log
* **[ [ / ] ]**: Step hardware pad attenuation up/down
* **[A]**: Toggle Auto-Guard headroom protection
* **[R]**: Reset digital peak hold

### Manage Local Wi-Fi Hotspot
```bash
djm-box hotspot status   # View current hotspot state, SSID, and WPA2 password
djm-box hotspot on       # Manually start the dedicated booth Wi-Fi hotspot
djm-box hotspot off      # Manually stop the local Wi-Fi hotspot
```

### Phone Pairing & Remote Booth Web App
Run `djm-box pair` to display the direct URL, hotspot gateway URL (`http://10.42.0.1:8080/?token=<PIN>`), and WPA2 credentials:
```bash
djm-box pair
```
Open `http://<ip-of-appliance>:8080` (or `http://10.42.0.1:8080` when on the hotspot) in any phone browser, or connect via the Android Companion App. Enter your 6-digit PIN.

#### Live Booth Monitor Dashboard
The web dashboard provides real-time stereo VU metering, active recording stats, digital clip detection, lossless 15-second audio preview, and live Pioneer Pro DJ Link deck cards with On-Air indicators:

<p align="center">
  <img src="docs/images/dashboard_live_monitor.png" alt="djm-box Live Booth Monitor Dashboard" width="900" />
</p>

### Post-Event Audio Studio & Mastering (`djm-box process`)

Automates session grouping, continuity auditing across multi-hour rollovers, artist boundary resolution, audio quality measurement, loudness normalization, FLAC/MP3 rendering, and report generation. The Post-Event Studio is accessible directly from the CLI or via the interactive Web Dashboard tab:

<p align="center">
  <img src="docs/images/dashboard_post_event_studio.png" alt="djm-box Post-Event Mastering Studio" width="900" />
</p>

```bash
# Auto-detect latest session, resolve boundaries from timetable, render FLAC & MP3
djm-box process --timetable timetable.txt --preset streaming --mp3 --full-event

# Process a specific session folder or event date with club target (-11 LUFS)
djm-box process --session 2026-10-30 --preset club --output-dir ./mastered_output

# Dry-run analysis (computes LUFS, peaks, clipping stats, and boundary cuts without rendering)
djm-box process --timetable timetable.txt --dry-run
```

#### Pipeline Stages:
1. **Tooling & Recording Guardrail**: Verifies `ffmpeg`/`ffprobe` and aborts if `djm-daemon` is actively recording or entered `ArmedSilent` within the 10-minute quiet period (override with `--force`).
2. **Session Discovery & Continuity Audit**: Groups rollover segments (`pt01..ptN.flac` and `.log`), identifies pre-event warmups, and audits sample-count continuity across 3-hour rollovers within 50 ms tolerance.
3. **Boundary Resolution**: Auto-aligns timetable slots to handover silences (below -45 dBFS for ≥ 2.0 s within 15 min), Pro DJ Link USB handovers, or scheduled clock times.
4. **Per-Set Audio Quality Analysis**: Measures Sample Peak (dBFS), True Peak (dBTP), RMS, Integrated LUFS, LRA, Crest Factor, flat-topped clipping factor, and digital clip counts in a single pass.
5. **Normalization & Rendering**: Applies 25 Hz highpass, calculated gain, and 4× oversampled limiting (`aresample=384000,alimiter=limit=-1.0dBTP:level=false,aresample=96000`), with optional de-clipping (`--declip`).
6. **Lossless Concat & Post-Render Verification**: Concatenates seamless full-event FLAC (`--full-event`), exports 320 kbps MP3s (`--mp3`), re-measures every rendered output against targets (flagging deviations > 1.0 LU or true-peak ceiling breaches), and generates complete reports (`Event_Report.html`, `Event_Report.md`, `Event_Report.json`, `tracklist.txt`, `Event.cue`).

#### Performance Benchmarks (96 kHz 24-bit Stereo):
| Stage | x86_64 Host Speed | aarch64 (Raspberry Pi 5) | aarch64 (Raspberry Pi 4) |
| :--- | :--- | :--- | :--- |
| **Analysis Pass** | ~38× real-time | ~12× real-time | ~6× real-time |
| **Mastering FLAC Render** | ~66× real-time | ~18× real-time | ~8× real-time |
| **MP3 (320k) Export** | ~120× real-time | ~25× real-time | ~10× real-time |
| **Verification Pass** | ~38× real-time | ~12× real-time | ~6× real-time |
| **Total Full Pipeline** | **~13.2× real-time** | **~5.5× real-time** | **~2.5× real-time** |
| **Time per 1h Audio** | **~4.5 minutes** | **~11.0 minutes** | **~24.0 minutes** |

### Pioneer Pro DJ Link Telemetry & Deck Monitoring (`djm-box prodj`)

`djm-box` includes a dedicated, fault-isolated daemon (`djm-prodj` / `djm-box-prodj.service`) that connects to Pioneer Pro DJ Link over Ethernet to provide real-time deck telemetry, On-Air track detection, USB media identification, and automated tracklist generation:

```bash
# Query Pro DJ Link live network status, virtual player arbitration, and degraded mode
djm-box prodj

# List all discovered Pioneer CDJ decks and DJM mixers on the Link network
djm-box prodj devices

# Show active deck playback states, On-Air indicators, BPM, and current track titles
djm-box prodj decks
```

#### Hardware & Ethernet Networking Requirements:
- **Physical Booth Ethernet Cable**: Connect a standard RJ-45 Ethernet cable from the appliance (laptop or Raspberry Pi) to an open port on the booth's Ethernet switch (or the built-in Link hub of a DJM-V10 or DJM-A9).
- **Network Addressing**: Pro DJ Link uses UDP broadcasts on ports `50000` (Announce/Keep-Alive), `50001` (Player Slot Claim), and `50002` (Mixer On-Air & CDJ Deck Status). Ensure the booth switch is an unmanaged switch that does not block or filter UDP broadcasts. Devices typically auto-configure using Link-Local IP addresses (`169.254.x.x`) or DHCP (`192.168.85.x`).
- **Fault Isolation & Non-Disruptive Virtual Participation**:
  - `djm-prodj` runs as a completely separate OS process communicating with the system via `/run/user/1000/djm-prodj.sock`. Audio capture in `djm-daemon` is 100% immune to network outages, packet floods, or unplugged cables.
  - **Passive Settling**: Stays strictly passive upon connection while monitoring physical hardware.
  - **6-Player Networks (V10 / A9)**: Prefers player numbers 6 or 5 once settled.
  - **4-Player Networks (NXS2)**: Enforces a strict 3-minute live-audio gate (audio must play continuously with zero new hardware announcements) before claiming slot 4.
  - **Zero Contestation**: The instant any physical player appears or claims our number, `djm-prodj` immediately releases its virtual slot without sending any contesting packets. If passive or unassigned, it runs seamlessly in Degraded Mode (logging On-Air durations, BPM, and USB stick handovers).

---

## Output Files & Audit Logs

### Raw Capture Files (`~/Music/DJ_Recordings/`)
Configurable in `config.toml` (`[storage] output_dir`):
- **Master FLAC Recordings**: `Pioneer_DJM-900NXS2_YYYY-MM-DD_HH-MM-SS_pt01.flac` (24-bit / 96 kHz or 48 kHz bit-perfect audio).
- **Synchronized Ambient Recordings**: `..._pt01_ambient.flac` (synchronized crowd/room capture tagged with master segment name and sample offset).
- **Session Audit Logs**: `..._pt01.log` (text log containing RFC 3339 timestamps with UTC offsets, periodic MB/duration updates, digital clipping counts, Auto-Guard adjustments, and clean shutdown summaries).
- **Pro DJ Link Event Stream**: `..._prodj.jsonl` (line-delimited JSON log of On-Air state transitions, played tracks, CDJ deck telemetry, and USB media handovers).

### Mastered Studio Outputs (`<session>_mastered/`)
Generated by `djm-box process`, the Web Dashboard, or the Companion App:
- **Individual Artist Sets (FLAC)**: `01_Artist_Name_mastered.flac` (EBU R128 loudness normalized, 4× oversampled true peak limited, tagged with artist, date, and set title).
- **Individual Artist Sets (MP3)**: `01_Artist_Name.mp3` (320 kbps high-bitrate MP3 with ID3v2 tags).
- **Full Event Continuous Mix**: `00_Full_Event_Continuous_Mix.flac` (gapless concatenation of the entire event).
- **Interactive Event Report**: `Event_Report.html` (self-contained HTML report with measured LUFS, true peak headroom, clipping audit, and continuity tables).
- **Audit Reports & Cue Sheets**: `Event_Report.md`, `Event_Report.json`, `tracklist.txt`, and `Event.cue` (sub-second cue sheet with exact artist boundaries).

<p align="center">
  <img src="docs/images/event_report_preview.png" alt="djm-box Event Quality & Mastering Report" width="900" />
</p>

---

## License

This project is licensed under the [MIT License](LICENSE).
