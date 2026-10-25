# Design

## Context

Currently, `djm-daemon` runs a single `AlsaCapture` thread targeting the detected Pioneer DJ mixer USB card, reading interleaved multi-channel PCM, extracting the `rec_out_channels` pair, and writing to a single `FlacWriter`. In `mixer_controls.rs`, the daemon forcefully enables `Mic to Rec Out` switches. There is currently no mechanism to isolate the mic, capture ambient crowd noise, or build release binaries via continuous integration.

## Goals / Non-Goals

**Goals:**
- Provide clean, mic-free master set recordings by default via hardware ALSA control toggling.
- Capture dedicated DJM mic stream when routed over USB.
- Run an independent ALSA capture pipeline for the host machine's built-in or USB microphone to capture room/crowd ambience.
- Synchronize all streams with embedded UTC timestamps and sample offsets for painless multi-track DAW mixing.
- Support a configurable 15-minute trailing hangover for ambient recording after mixer silence.
- Provide a robust GitHub Actions workflow building and publishing x86_64 and aarch64 (Raspberry Pi) release tarballs.

**Non-Goals:**
- Real-time software mixing of ambient mic back into the DJ's monitor or master output.
- Cloud uploading or live streaming of audio streams (local recording appliance focus).
- Windows or macOS support (Linux-specific pro-audio appliance).

## Decisions

### 1. Dual Independent ALSA Capture Pipelines
- **Rationale**: The Pioneer DJ mixer (running at 96kHz or 48kHz 24-bit) and the host machine's onboard microphone (typically 48kHz or 44.1kHz 16/24-bit on ALC236 or USB) run on independent hardware audio clocks. Combining them into one clock domain in software would introduce jitter or require sample-rate conversion resampling artifacts.
- **Approach**: Run two separate ALSA capture threads:
  1. `MixerCaptureLoop`: Captures mixer USB PCM, splits Master pair and DJM Mic pair (if routed).
  2. `AmbientCaptureLoop`: Captures host microphone PCM at native rate/channels.
- **Alternatives Considered**: Using PipeWire loopbacks. Rejected because `djm-rec-linux` operates as a low-level, headless, bulletproof ALSA appliance that runs reliably even without a graphical session or desktop audio server running.

### 2. Cross-Stream DAW Synchronization
- **Rationale**: Producers and archivists need to align the master DJ set with the crowd/ambient recording in software like Ableton, Logic, or Reaper.
- **Approach**: Embed Vorbis comment metadata fields in each FLAC file:
  - `STREAM_TYPE`: `MASTER`, `DJM_MIC`, or `AMBIENT`
  - `SESSION_ID`: Unique UUID matching all sibling files in the set
  - `START_TIME_UTC`: High-precision ISO-8601 timestamp
  - `SAMPLE_RATE`: Native hardware capture rate
  - `SAMPLE_OFFSET`: Sample count relative to session start
- Additionally, log these exact timestamps and sample counts in the session audit log (`.log`).

### 3. Ambient Hangover State Machine
- **Rationale**: When a DJ finishes a set, mixer audio drops below `-75 dBFS`, but audience applause, closing vibes, and room ambience continue.
- **Approach**:
  - `SilenceGate` monitors mixer audio. When mixer silence timeout fires, the Master FLAC writer closes and finalizes.
  - A tokio `HangoverTimer` starts (configurable, default 15 minutes).
  - The ambient recording continues writing to its FLAC file.
  - If new mixer audio arrives before the timer expires, the timer is aborted and the ambient recording seamlessly continues alongside the new master segment.
  - If the timer expires, the ambient FLAC writer gracefully flushes and finalizes.

### 4. Configuration Schema Expansion
Add `[audio.djm_mic]` and `[audio.ambient_mic]` sections to `AppConfig`:
```toml
[audio.djm_mic]
include_in_master = false
separate_recording = true

[audio.ambient_mic]
enabled = true
device = "auto"
post_set_hangover_mins = 15
sample_rate = 48000
channels = 2
```

### 5. GitHub Actions Release Workflow
- Use GitHub Actions with `cargo` and cross-compilation tooling (`cross-rs` / `cross`) to target:
  - `x86_64-unknown-linux-gnu`
  - `aarch64-unknown-linux-gnu`
- Trigger on tags: `v*`.
- Package artifacts with release binaries, systemd unit templates, setup scripts, and checksums into `.tar.gz` and publish to GitHub Releases.

## Risks / Trade-offs

- **[Risk: Host microphone hardware busy]** → If another application has locked the host microphone in exclusive ALSA mode, ambient capture could fail to open.
  - *Mitigation*: Fall back to ALSA `default` / `pulse` PCM plugin or log a descriptive warning in the session log without aborting the mission-critical DJ master recording.
- **[Risk: Multiple host microphones available]** → The machine might have both an internal laptop mic and a webcam mic.
  - *Mitigation*: Auto-detection chooses the primary internal analog card (`HDA Intel`, `ALC*`, etc.) by default, while allowing explicit specification via `audio.ambient_mic.device = "hw:CARD,DEV"`.
