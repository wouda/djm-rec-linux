# Spec Delta

## Purpose

Enables automated discovery, capture, and post-set trailing hangover recording of ambient crowd and venue atmosphere using the host machine's microphone.

## ADDED Requirements

### Requirement: Host Device Microphone Auto-Detection
The daemon SHALL automatically detect and select the host machine's primary non-DJM capture device (such as built-in laptop microphones or USB room mics) when `audio.ambient_mic.device` is configured as `"auto"`.

#### Scenario: Host microphone auto-detection
- **WHEN** the appliance daemon initializes and `audio.ambient_mic.enabled` is `true`
- **THEN** the daemon scans ALSA capture cards, filters out Pioneer DJ mixer devices, and selects the default host microphone

#### Scenario: Explicit hardware override
- **WHEN** `audio.ambient_mic.device` specifies an explicit ALSA card or device identifier (such as `"hw:0,0"`)
- **THEN** the daemon captures ambient audio directly from the specified hardware interface

### Requirement: Trailing Hangover Recording
The ambient microphone recording stream SHALL continue recording for a configurable trailing duration after mixer audio stops, capturing room ambience, crowd chatter, and event closing atmosphere.

#### Scenario: Post-set trailing hangover duration
- **WHEN** the Pioneer mixer ceases audio playback or enters prolonged silence
- **THEN** the master recording transitions to idle while the ambient recording continues running for `post_set_hangover_mins` (default 15 minutes) before cleanly closing its FLAC file

#### Scenario: Mixer resumes playback during hangover
- **WHEN** the mixer resumes audio playback while an ambient hangover timer is actively counting down
- **THEN** the ambient hangover timer cancels and the ambient stream seamlessly continues alongside the new master set
