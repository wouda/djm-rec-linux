# Spec Delta

## Purpose

Provides an optional, fault-isolated Pioneer Pro DJ Link daemon (`djm-prodj`) that observes CDJ decks and DJM mixers over Ethernet without disrupting physical players, logs played On-Air tracks and USB media, and supplies set-boundary and tracklist data.

## ADDED Requirements

### Requirement: Fault-Isolated Pro DJ Link Daemon
`djm-prodj` MUST run as a separate OS process from `djm-daemon` so that network failures or protocol errors never affect audio capture.

#### Scenario: No Pro DJ Link network
- **WHEN** no booth Ethernet network is connected
- **THEN** `djm-prodj` stays idle listening for announcements while `djm-daemon` records normally

#### Scenario: Pro DJ Link devices present
- **WHEN** CDJs and a DJM announce themselves on the network
- **THEN** `djm-prodj` lists the devices, streams live telemetry over `/run/user/1000/djm-prodj.sock`, and appends events to `<session>_prodj.jsonl`

### Requirement: Passive Start
`djm-prodj` MUST transmit nothing on the Pro DJ Link network until it has observed the network for at least the settling period.

#### Scenario: Booth setup
- **WHEN** CDJs are being powered on and assigned player numbers
- **THEN** `djm-prodj` only listens and records device announcements and mixer On-Air data

### Requirement: Safe Virtual Player Participation
`djm-prodj` SHALL announce itself only on the highest player number the observed hardware supports that no device uses, and only after the event is live with audio playing and no new device announced for 3 minutes.

#### Scenario: 6-player capable network
- **WHEN** the observed hardware supports 6 player numbers and number 6 (or 5) is free after the settling conditions are met
- **THEN** `djm-prodj` announces itself on that number

#### Scenario: 4-player network with a free number
- **WHEN** the network supports 4 player numbers, at most 3 are in use, and the settling conditions are met
- **THEN** `djm-prodj` announces itself on number 4

#### Scenario: No free player number
- **WHEN** every supported player number is in use
- **THEN** `djm-prodj` remains passive and continues logging everything available without a player number

### Requirement: Immediate Release on New Devices
`djm-prodj` MUST stop announcing its player number as soon as any device not previously seen begins announcing or claiming a number, and MUST NOT send any packet that contests a number.

#### Scenario: CDJ connected mid-event
- **WHEN** a new device's announcement or number-claim packet is received
- **THEN** `djm-prodj` stops its own announcements before processing further packets, stays passive until the settling conditions are met again, and logs the release

#### Scenario: Device claims djm-prodj's number
- **WHEN** another device claims the number `djm-prodj` is using
- **THEN** `djm-prodj` releases the number without sending any defending packet

### Requirement: Degraded Operation Without Player Number
When passive, `djm-prodj` SHALL still log device presence, mixer channel On-Air state, beat/tempo data, and USB media identity where obtainable, and SHALL mark track identity as unavailable.

#### Scenario: Passive for the whole event
- **WHEN** `djm-prodj` never obtains a player number
- **THEN** the session log contains On-Air timing per deck and any USB media identity obtained, and tracklists label track titles as unavailable

### Requirement: On-Air Track Play Detection
The system SHALL log a track as played only after its deck has been playing with its channel On-Air for at least 20 cumulative seconds, or after 30 seconds of playhead advance when On-Air data is absent.

#### Scenario: Track cued off-air
- **WHEN** a deck plays while its channel is Off-Air
- **THEN** live telemetry shows the deck as `Cued` and the track is not logged

#### Scenario: Track played On-Air
- **WHEN** a deck plays On-Air for 20 cumulative seconds
- **THEN** the system logs timestamp, title, artist, BPM, key, deck number, and source USB media identifier, resolving metadata from the media's rekordbox database when available

### Requirement: USB Media Identification and B2B Clustering
The system SHALL identify the source USB media of each played track and SHALL group media that alternate within one set window into a single B2B set.

#### Scenario: Handover to a new USB stick
- **WHEN** media A stops supplying On-Air tracks and media B supplies consecutive On-Air tracks
- **THEN** the system proposes a set boundary at media B's first On-Air timestamp

#### Scenario: Alternating USB sticks
- **WHEN** tracks alternate between media A and media B within one set window
- **THEN** A and B form one B2B cluster and each track keeps its own media attribution

### Requirement: Tracklist Text and CUE Sheet Export
The system SHALL generate `.txt` tracklists and `.cue` sheets for the full event and each artist set.

#### Scenario: B2B tracklist
- **WHEN** a set used multiple USB sticks
- **THEN** the `.txt` header lists each stick with its track count and each track line shows timestamp, source stick, artist, title, and BPM
