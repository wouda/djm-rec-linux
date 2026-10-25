# Technical Design: djm-box DJ Mixer Recording Appliance

## Context

High-energy DJ performances across an entire 8–12 hour club night routinely clip digital audio capture systems when mixers are driven hard. Standard recording setups fail in club environments due to process crashes (GUI/Bluetooth locks), premature silence splitting during track breakdowns, corrupted FLAC headers after power loss, accidental USB unplugs during DJ handovers, unauthorized crowd BLE connections, or limited Bluetooth range when leaving the booth. See `proposal.md` for background motivation.

## Goals / Non-Goals

**Goals:**
- Guarantee zero audio dropouts or recording terminations due to UI crashes, Bluetooth disconnects, D-Bus errors, or network latency.
- Achieve bit-perfect 24-bit FLAC capture locked to mixer native hardware rates (96 kHz / 48 kHz) with zero missing samples across segment rollovers or manual splits.
- Automatically provide +19 dB digital headroom via Pioneer/AlphaTheta ALSA `Capture Level` hardware controls, safeguarding against loud DJ red-lining.
- Deliver true power-cut resilience using `.flac.part` staging, 4 KB pre-allocated header padding, 2-second `fdatasync()`, 30-second `STREAMINFO` header checkpointing, and boot-time auto-repair.
- Provide authenticated, multi-transport phone monitoring that seamlessly switches between Local Wi-Fi (`ws://djm-box.local:8080/ws`), Internet/VPN (Tailscale), and offline 20-byte BLE GATT (15–20 Hz).
- Protect the appliance in crowded clubs with two-layer BLE security (BlueZ pairing control + 3-second GATT token watchdog that forcibly disconnects unauthorized scanners) and Bearer token WebSocket auth, paired via a terminal QR code in `djm-tui`.
- Automate complete appliance setup for CachyOS / Arch Linux.

**Non-Goals:**
- Software resampling, equalisation, or multi-track channel recording (only stereo REC OUT capture is targeted).
- Live audio streaming over BLE or WebSockets (telemetry and control only, keeping CPU and bandwidth overhead minimal).
- Modifying channel inputs 1–6 (ensuring zero interference with DVS/Rekordbox performance on USB-B).

## Multi-Process Architecture & Fault Isolation

```
+-----------------------------------------------------------------------------------+
|  djm-daemon (djm-box-daemon.service)                                              |
|  - Realtime FIFO priority (80) | Zero BLE / Zero GUI / Zero Network dependencies  |
|                                                                                   |
|  +--------------------+        +---------------------+        +-----------------+ |
|  | Native ALSA Capture| -----> | Lock-Free SPSC Ring | -----> | Disk Writer &   | |
|  | + Instant Re-Plug  |        | Buffer + 2s PreRoll |        | Zero-Gap Split  | |
|  +--------------------+        +---------------------+        +--------+--------+ |
|                                                                        |          |
|                                               +------------------------+--------+ |
|                                               v                                 v |
|                                  +-------------------------+   +----------------+ |
|                                  | 2s fdatasync() + 30s    |   | Async Finalizer| |
|                                  | STREAMINFO Checkpoint   |   | (4KB Pad Tags) | |
|                                  +-------------------------+   +----------------+ |
|                                                                                   |
|  +--------------------+        +---------------------+                            |
|  | DSP Metering Engine| -----> | Non-blocking IPC    |                            |
|  | (Peak, RMS, Clips) |        | Broadcast (Tokio)   |                            |
|  +--------------------+        +----------+----------+                            |
+-------------------------------------------|---------------------------------------+
                                            | Unix Domain Socket
                         /run/user/$UID/djm-box.sock (lag-drop)
                         +------------------+------------------+
                         |                                     |
                         v                                     v
+------------------------------------+ +--------------------------------------------+
| djm-tui (CLI Process)              | | djm-remote (djm-box-remote.service)        |
| - Stateless Ratatui Terminal TUI   | | - Isolated Systemd User Service            |
| - [P] Unicode QR Code & PIN Modal  | | - BlueZ BLE GATT + 3s Auth Token Watchdog  |
| - Closing does not stop recording  | | - Token-Auth Axum HTTP & WebSockets (8080) |
+------------------------------------+ +---------------------+----------------------+
                                                             |
                   +-----------------------------------------+--------------------+
                   | 1. Local Wi-Fi (mDNS)    | 2. Internet / Tailscale   | 3. BLE GATT (Offline)
                   v                          v                           v
            +---------------------------------------------------------------------+
            | Companion Mobile Clients (Authenticated via QR / 6-Digit PIN)       |
            | - flutter_app/ (Smart Multi-Transport Auto-Switch: Wi-Fi/WAN/BLE)   |
            | - Mobile Web Browser (http://djm-box.local:8080/?token=...)         |
            +---------------------------------------------------------------------+
```

## Decisions

### 1. Multi-Process Separation vs. Single Multi-Threaded Binary
- **Decision**: Separate the system into 3 independent binaries (`djm-daemon`, `djm-tui`, `djm-remote`).
- **Rationale**: The BlueZ D-Bus stack and network sockets are prone to latency spikes, service restarts, and OS-level stalls. With process isolation, `djm-remote` or `djm-tui` can crash and restart indefinitely while `djm-daemon` continues capturing audio and flushing to disk without missing a single frame.

### 2. Lock-Free SPSC Audio Ring Buffer & Zero-Gap Background File Splitting
- **Decision**: The ALSA capture callback pushes raw interleaved PCM chunks into a lock-free Single-Producer Single-Consumer (SPSC) ring buffer. When a file split occurs (either at `max_segment_duration_mins = 180` or manual `[Split File Now]`), the consumer thread closes the active write stream at sample $N$, immediately opens the next `.flac.part` file at sample $N+1$, and hands the completed file over an unbounded channel to a low-priority **Background Finalizer Worker** that writes ReplayGain tags into the 4 KB header padding and renames `.flac.part` -> `.flac`.
- **Rationale**: In an 8-hour club night with back-to-back DJs, music never stops. Offloading file finalization ensures 100% phase-continuous, sample-accurate transitions across split files with zero dropped frames.

### 3. Crash-Proof FLAC Checkpointing, 4 KB Header Padding & `.flac.part` Auto-Repair
- **Decision**:
  1. Write active recordings to `<name>.flac.part` and reserve a **4096-byte `PADDING` metadata block** immediately after `STREAMINFO`.
  2. Call `fdatasync()` every `fsync_interval_secs` (2.0s) to flush encoded FLAC frames to non-volatile storage.
  3. Every 30 seconds, use positional write (`pwrite()`) to update `total_samples` in the 34-byte `STREAMINFO` block at the beginning of the file without disrupting the append cursor.
  4. Upon file closure, write `REPLAYGAIN_TRACK_GAIN`, `REPLAYGAIN_TRACK_PEAK`, and `DJM_GAIN_STEPS` Vorbis comments directly into the 4 KB `PADDING` block in-place (<1 ms) and atomically rename `.flac.part` -> `.flac`.
  5. On `djm-daemon` startup, scan `output_dir` for any orphaned `.flac.part` files from a power loss, verify the last valid frame, patch `STREAMINFO`, and rename to `.flac`.
- **Rationale**: `fdatasync()` alone leaves `total_samples = 0` in the FLAC header on power loss, causing DAWs and CDJs to reject the file, and appending Vorbis tags without pre-allocated padding forces a multi-gigabyte file rewrite.

### 4. Club-Tuned Silence Gate, Pre-Roll Buffer & Instant USB Re-Plug Recovery
- **Decision**:
  - Set default `pause_split_timeout_secs = 120.0`, `silence_threshold_dbfs = -75.0`, and `wake_threshold_dbfs = -65.0`.
  - Maintain a 2.0-second circular PCM buffer in RAM during `ARMED_SILENT`; prepend it immediately when signal exceeds `-65.0 dBFS`.
  - If ALSA returns `-ENODEV` or `-EPIPE` (e.g., a DJ accidentally unplugs the USB cable during a laptop changeover), immediately finalize the current `.flac.part` file, enter `DISCONNECTED`, and poll/listen for USB re-enumeration to automatically re-apply `Ch5 Input = Rec Out` and `Capture Level = -19dB` and resume recording.
- **Rationale**: With a `-19dB` hardware pad on a 24-bit mixer, `-75 dBFS` is `-56 dB` on the mixer meters, and a 120-second timeout prevents ambient breakdowns or brief B2B pauses from splitting a DJ set prematurely.

### 5. Two-Layer Bluetooth & Web Security with Terminal QR Pairing
- **Decision**:
  - `~/.config/djm-box/config.toml` stores a 6-digit/8-char `auth_token` (auto-generated on `djm-box setup`).
  - Pressing `[P]` in `djm-tui` renders a high-contrast Unicode QR code (`▀`/`▄`) and displays the PIN, while instructing `djm-remote` to open a 60-second pairing window.
  - **Layer 1 (BlueZ Adapter Control)**: Outside the 60-second pairing window, `djm-remote` keeps the BlueZ adapter `Pairable = false` so random phones cannot initiate OS Bluetooth pairing.
  - **Layer 2 (GATT 3-Second Auth Watchdog)**: Any BLE central connecting to service `0000d900-0000-1000-8000-00805f9b34fb` MUST write `[0x00, ...auth_token_bytes]` to the Control Characteristic within **3.0 seconds**. Until authenticated, telemetry notifications and control writes are blocked. If the 3.0-second timer expires without valid auth, `djm-remote` calls `device.disconnect()` over BlueZ D-Bus immediately and restarts BLE advertising.
  - **Web / WebSocket Auth**: Axum requires `Authorization: Bearer <auth_token>` or `?token=<auth_token>` on `/ws` and API routes.
- **Rationale**: Prevents anyone in the club crowd with a BLE scanner or Flipper Zero from triggering splits, altering hardware gain, or holding the single BLE peripheral connection slot hostage.

### 6. Smart Multi-Transport Architecture (Local Wi-Fi + Internet/Tailscale + BLE)
- **Decision**: `flutter_app/` implements a unified `TransportManager` that maintains connection state across three prioritized transports:
  1. **Local Wi-Fi WebSocket (`ws://djm-box.local:8080/ws`)**: 60 Hz JSON telemetry + full log history when on booth/venue Wi-Fi.
  2. **Remote / Internet / Tailscale WebSocket (`ws(s)://<remote_host>:8080/ws`)**: Works over 4G/5G anywhere in the club or off-site when laptop and phone are on Tailscale/VPN or port-forwarded.
  3. **Offline BLE GATT (`0000d900-...`)**: Compact 20-byte binary payload at 15–20 Hz when no Wi-Fi/Internet exists.
  - Control commands (`0x00` Auth, `0x01` Bookmark, `0x02` Split, `0x03` Pad, `0x04` Auto-Guard, `0x05` Reset Peaks) work identically over both WebSocket and BLE.

### 7. 20-Byte Compact Binary BLE GATT Profile
- **Decision**: Transmit fast telemetry over BLE as a compact 20-byte packed binary struct at 15–20 Hz over characteristic `0000d901-0000-1000-8000-00805f9b34fb`:
  - Byte 0: `state` (uint8: 0=Disconnected, 1=ArmedSilent, 2=Recording, 3=Paused)
  - Byte 1: `hw_pad_db` (int8: -19, -15, -10, -5) with bit 6 (`auto_guard`) and bit 7 (`silence_countdown_active`)
  - Bytes 2..3: `peak_l_centi_db` (int16_le: e.g. -1425 = -14.25 dBFS)
  - Bytes 4..5: `peak_r_centi_db` (int16_le)
  - Bytes 6..7: `rms_l_centi_db` (int16_le)
  - Bytes 8..9: `rms_r_centi_db` (int16_le)
  - Bytes 10..11: `clip_count` (uint16_le)
  - Bytes 12..15: `active_file_duration_secs` (uint32_le)
  - Bytes 16..17: `active_file_size_mb` (uint16_le)
  - Bytes 18..19: `free_disk_gb_tenths` (uint16_le: e.g. 1482 = 148.2 GB)

## Risks / Trade-offs

- **[Risk]** Unauthorized BLE scanner connects and blocks legitimate owner phone from seeing BLE advertisements.
  - **Mitigation**: 3.0-second authentication watchdog forcibly disconnects unauthenticated BLE centrals via BlueZ D-Bus and immediately resumes advertising.
- **[Risk]** BlueZ D-Bus service can be unstable or slow to register GATT characteristics.
  - **Mitigation**: `djm-remote` runs under systemd with `Restart=always` and `RestartSec=3`, completely isolated from `djm-daemon`.
- **[Risk]** Power cord yanked mid-recording after 2 hours of a DJ set.
  - **Mitigation**: 2-second `fdatasync()` + 30-second `STREAMINFO` `pwrite()` checkpoint + startup `.flac.part` auto-repair guarantees a playable FLAC file with at most 2 seconds of lost audio.
- **[Risk]** PipeWire / WirePlumber desktop audio daemon may attempt to acquire or re-route mixer audio.
  - **Mitigation**: Provide `/etc/udev/rules.d/99-djm-box.rules` and WirePlumber configuration (`51-djm-pro-audio.conf`) instructing WirePlumber to leave DJ mixer USB nodes in pro-audio raw mode.
- **[Risk]** DJ closes laptop lid in the booth or system powers down into sleep/suspend.
  - **Mitigation**: Install `/etc/systemd/logind.conf.d/djm-appliance.conf` with `HandleLidSwitch=ignore`, `HandleLidSwitchExternalPower=ignore`, and `HandleLidSwitchDocked=ignore`. In addition, `djm-daemon` automatically commands `powerprofilesctl set performance` asynchronously during recording to eliminate CPU throttle jitter and reverts to `balanced` when idle.
