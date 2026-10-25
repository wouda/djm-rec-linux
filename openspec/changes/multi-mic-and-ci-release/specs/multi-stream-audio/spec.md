# Spec Delta

## Purpose

Enables independent, concurrent multi-stream audio capture for DJM Master (pure music) and DJM Microphone with metadata alignment across separate FLAC files.

## ADDED Requirements

### Requirement: DJM Microphone Exclusion from Master Stream
The audio engine SHALL ensure the Pioneer mixer microphone is unlinked from the REC OUT master stream by default, capturing pristine music without vocal bleed unless explicitly re-enabled in configuration.

#### Scenario: Default unlinked microphone capture
- **WHEN** a Pioneer mixer is connected and audio recording begins
- **THEN** ALSA microphone routing control `Mic to Rec Out` is set to 0/Disabled, and master FLAC recording contains only music channels

#### Scenario: Microphone explicitly linked in configuration
- **WHEN** `audio.djm_mic.include_in_master` is set to `true` in `config.toml`
- **THEN** ALSA microphone routing control is set to 1/Enabled, and master FLAC recording contains both music and microphone vocals

### Requirement: Dedicated DJM Microphone FLAC Stream
The recording daemon SHALL capture the DJM microphone stream into an independent secondary FLAC recording when a dedicated microphone channel is routed over USB.

#### Scenario: Dedicated mic channel detected
- **WHEN** `audio.djm_mic.separate_recording` is `true` and the mixer provides an isolated microphone USB channel
- **THEN** the daemon writes a parallel FLAC file with the `_djm_mic.flac` suffix alongside the master recording

### Requirement: Cross-Stream Timeline Alignment
All simultaneous recording streams SHALL embed ISO-8601 wall-clock start timestamps and relative sample offsets into FLAC metadata tags and session logs to enable sample-accurate timeline alignment in digital audio workstations.

#### Scenario: Session split alignment
- **WHEN** a manual or automatic split is triggered on the recording session
- **THEN** all active concurrent streams split simultaneously with synchronized frame counts and matching timestamp prefixes
