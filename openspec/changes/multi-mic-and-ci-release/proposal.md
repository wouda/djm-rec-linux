# Proposal

## Why

During live event recording (specifically for events such as [craterdnb.com](https://craterdnb.com)), sets demand clean, pristine master music recordings without DJ or host microphone talk-over, while simultaneously preserving host microphone speech on a separate track for archival or post-production remixing. Additionally, capturing crowd energy and room ambience using the recording device's built-in or connected microphone provides invaluable spatial atmosphere that should outlast the DJ set by a configurable trailing duration (hangover). Finally, automated cross-platform release builds are required to deliver self-contained installable binaries to users.

## What Changes

- **DJM Microphone Isolation**: By default, disable hardware mixing of the DJM microphone into the REC OUT master stream (`Mic to Rec Out` = 0) so the primary recording is pure music.
- **Dedicated DJM Mic Stream**: Capture the DJM microphone into a synchronized secondary FLAC recording (`..._djm_mic.flac`) when routed over USB.
- **Host Device Ambient Recording**: Concurrently record room ambience using the host device's built-in or USB microphone into a 3rd FLAC recording (`..._ambient.flac`).
- **Precision Alignment & Trailing Hangover**: Embed exact wall-clock start times and sample offsets in FLAC metadata and session logs for instant DAW timeline alignment, and keep ambient recording active for a configurable trailing period (default 15 minutes) after mixer silence.
- **Configurable Multi-Audio Settings**: Provide intuitive TOML settings under `[audio.djm_mic]` and `[audio.ambient_mic]`.
- **GitHub Actions Release CI**: Automate Linux `x86_64` and `aarch64` (Raspberry Pi) binary compilation, packaging, and GitHub Releases on `v*` tag push with direct download links in README.

## Capabilities

### New Capabilities
- `multi-stream-audio`: Manages concurrent, synchronized audio capture streams (Master, DJM Mic, and Ambient Mic) with per-stream FLAC writers and cross-stream alignment metadata.
- `ambient-microphone-capture`: Discovers, initializes, and records host device audio input with independent lifecycle control and trailing post-set hangover recording.
- `release-automation`: Builds, archives, and releases multi-architecture binaries via GitHub Actions with README download links.

### Modified Capabilities
<!-- None: No existing specs in openspec/specs yet. -->

## Impact

- **Crates Affected**:
  - `djm-common`: Config models updated with `[audio.djm_mic]` and `[audio.ambient_mic]`.
  - `djm-daemon`: Audio engine updated to spawn and manage ambient ALSA capture thread, handle multi-channel de-interleaving for DJM mic, manage trailing hangover timer, and write synchronized FLAC files.
  - `djm-cli`: Appliance status and monitor updated to display ambient mic input levels and active multi-stream recorders.
- **CI / Repo**:
  - New workflow `.github/workflows/release.yml`.
  - Updated `README.md` with release badge and download instructions.
