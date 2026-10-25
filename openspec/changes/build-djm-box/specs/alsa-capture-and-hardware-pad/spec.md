# Spec Delta: alsa-capture-and-hardware-pad

## Purpose

Enables automated discovery, profile selection, REC OUT channel routing, dynamic hardware digital pad attenuation, and instant USB re-plug recovery for Pioneer and AlphaTheta professional DJ mixers via native ALSA interfaces.

## ADDED Requirements

### Requirement: Mixer Hotplug Detection and Identification
The system SHALL detect USB connection of supported Pioneer (`08e4`) and AlphaTheta/Pioneer DJ (`2b73`) mixers and map each device to its hardware audio profile.

#### Scenario: DJM-900NXS2 mixer connected
- **WHEN** a USB audio device with vendor ID `2b73` and product ID `000a` is plugged in
- **THEN** the system SHALL recognize the device as a DJM-900NXS2 with 10 channels at 96 kHz / 24-bit and select REC OUT channels 9 and 10

#### Scenario: Unknown Pioneer USB mixer connected
- **WHEN** a USB audio device matching vendor ID `08e4` or `2b73` without a dedicated profile is plugged in
- **THEN** the system SHALL fall back to the generic USB Audio Class 2.0 mixer profile and attempt stereo capture on the last channel pair

### Requirement: Non-Disruptive REC OUT Routing
The system SHALL only configure routing controls for the dedicated REC OUT channel pair and leave DVS and performance input channels 1 through 6 untouched.

#### Scenario: ALSA control initialization
- **WHEN** the daemon initializes mixer controls for DJM-900NXS2
- **THEN** the daemon SHALL set the ALSA mixer element `Ch5 Input` to `Rec Out` without altering elements for channels 1 through 4

### Requirement: ALSA Hardware Digital Pad Configuration
The system SHALL configure the ALSA mixer element `Capture Level` (or `Capture Level Capture Switch` on V10) to the user-specified default pad value (default `-19dB`).

#### Scenario: Mixer initialization with default pad
- **WHEN** the audio engine acquires the mixer ALSA handle
- **THEN** the daemon SHALL set the hardware capture pad attenuation to `-19dB` to ensure +19 dB of headroom above mixer 0 dB meter level

### Requirement: Auto-Headroom Guard Dynamic Step-Down and Logging
The system SHALL monitor real-time peak audio samples and automatically step down the hardware pad attenuation if incoming peaks exceed `-0.5 dBFS` when set higher than `-19dB`, logging the exact sample frame offset.

#### Scenario: Loud DJ drives mixer beyond safe USB threshold
- **WHEN** incoming peak levels equal or exceed `-0.5 dBFS` while `Capture Level` is at `-15dB` and `auto_headroom_guard` is enabled
- **THEN** the daemon SHALL immediately command ALSA to attenuate `Capture Level` to `-19dB` and record the exact sample frame index and dB delta in both the session `.log` and the FLAC `DJM_GAIN_STEPS` metadata tag

### Requirement: Instant USB Unplug and Dual-USB Handover Recovery
The system SHALL gracefully handle ALSA device disconnects (`-ENODEV` or `-EPIPE`) during B2B DJ laptop handovers by finalizing the current recording and automatically re-initializing capture upon USB re-connection.

#### Scenario: USB cable accidentally unplugged and re-plugged during gig
- **WHEN** the USB connection to the mixer drops mid-recording and is re-connected 15 seconds later
- **THEN** the daemon SHALL finalize the interrupted `.flac.part` file to `.flac`, enter `DISCONNECTED`, and upon re-plug immediately re-apply `-19dB` pad and resume recording into a new segment
