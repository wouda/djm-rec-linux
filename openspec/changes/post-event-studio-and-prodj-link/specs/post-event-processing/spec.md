# Spec Delta

## Purpose

Provides automated post-event session grouping, continuity verification, timetable/silence/USB set boundary detection, measured audio quality analysis, loudness normalization and artist splitting, reports, and the recorder accuracy guarantees those reports depend on.

## ADDED Requirements

### Requirement: Live Recording Protection Guardrail
The engine MUST refuse to start analysis or rendering while the recorder is `Recording`, or while it has been `ArmedSilent` for less than a configurable quiet period (default 10 minutes).

#### Scenario: Job requested during active recording
- **WHEN** a user starts analysis or rendering while `djm-daemon` is `Recording`
- **THEN** the system rejects the job with a conflict error and only permits timetable entry

#### Scenario: Job requested shortly after music stops
- **WHEN** `djm-daemon` has been `ArmedSilent` for less than the quiet period
- **THEN** the system rejects the job and reports the remaining wait time

#### Scenario: Job requested when recorder is idle
- **WHEN** `djm-daemon` is `Disconnected`, `Paused`, or `ArmedSilent` beyond the quiet period
- **THEN** the system allows analysis, preview, and rendering

### Requirement: Running Jobs Yield to Recording
Analysis and render jobs MUST run at idle CPU and IO priority and MUST abort when the recorder leaves its idle state.

#### Scenario: Recording resumes mid-job
- **WHEN** `djm-daemon` transitions to `Recording` while a job is running
- **THEN** the system terminates all job subprocesses, deletes partial outputs, and marks the job `Aborted` with the reason

### Requirement: Disk Space Pre-Check
The system MUST estimate the output size of a render from the selected outputs and refuse to start when free space minus the estimate falls below a safety margin (default 20 GB).

#### Scenario: Insufficient disk space
- **WHEN** the selected outputs are estimated to leave less than the safety margin free
- **THEN** the system rejects the render and shows the estimated size and available space

### Requirement: Required Tooling Availability
The system MUST verify that `ffmpeg` and `ffprobe` are installed before offering analysis or rendering.

#### Scenario: ffmpeg missing
- **WHEN** `ffmpeg` or `ffprobe` cannot be executed
- **THEN** the studio reports the missing dependency and how to install it, and analysis and render actions are disabled

### Requirement: Session Grouping and Continuity Auditing
The system SHALL group contiguous segments (`pt01..ptN.flac` and `.log`) into events, flag pre-event warmup segments, and verify each rollover seam.

#### Scenario: Multi-segment event with 3-hour rollovers
- **WHEN** consecutive segments are split by `Max Segment Duration Rollover`
- **THEN** the system checks that each segment's sample count divided by sample rate matches the interval between consecutive segment start timestamps (allowing for the pre-roll on the first segment) within 50 ms, and reports any seam outside tolerance

#### Scenario: Pre-event warmup detection
- **WHEN** earlier segments are separated from the main event by a silence-timeout stop before the first scheduled slot
- **THEN** the system flags them as warmups and excludes them from event exports by default

### Requirement: Ambient Stream Handling
The continuity audit SHALL report each ambient file's alignment to the master timeline, and ambient files SHALL be excluded from exports unless the user selects them.

#### Scenario: Ambient file spans several master segments
- **WHEN** an ambient file's duration or start offset does not match its paired master segment
- **THEN** the audit lists the mismatch with the computed offset and excludes the file from automatic exports

### Requirement: Accurate Clip Accounting
The recorder SHALL count clipped samples cumulatively without saturation for the lifetime of a session in telemetry and session logs.

#### Scenario: Long redlined event
- **WHEN** more than 65,535 clipped samples occur in one session
- **THEN** telemetry and session log totals continue to increase and match the sum of logged clip increments

### Requirement: Unambiguous Log Timestamps
The recorder SHALL write session log timestamps with a UTC offset, and the engine SHALL use offsets to order events across DST changes.

#### Scenario: Event spans a DST change
- **WHEN** a session crosses a daylight saving transition
- **THEN** log timestamps carry distinct offsets and the engine places events in correct order with correct durations

### Requirement: Artist Set Boundary Detection
The system SHALL resolve set boundaries from the pasted timetable using, in priority order: silence midpoints, Pro DJ Link USB handovers, then scheduled clock time.

#### Scenario: Handover silence near scheduled time
- **WHEN** a silence gap (below -45 dBFS for at least 2.0 s) or a bookmark occurs within 15 minutes of a scheduled transition
- **THEN** the system places the boundary at the midpoint of that silence

#### Scenario: Seamless transition with Pro DJ Link data
- **WHEN** no silence exists near the transition but USB On-Air data shows the incoming artist's media going On-Air
- **THEN** the system places the boundary at that On-Air timestamp

#### Scenario: Seamless transition without supporting data
- **WHEN** neither silence nor USB handover data is found
- **THEN** the system uses the scheduled time and flags the boundary for review

#### Scenario: Overnight timetable
- **WHEN** timetable times wrap past midnight
- **THEN** each slot is anchored to the event start date and times after midnight resolve to the following day

### Requirement: Per-Artist Audio Quality Analysis
The system SHALL measure each set's Sample Peak, True Peak, RMS, Integrated LUFS, LRA, Crest Factor, flat factor, and clipped sample count and percentage.

#### Scenario: Clean set
- **WHEN** a set has no clipped samples
- **THEN** the report shows its measured metrics, a quality label, and the gain needed to reach the selected target

#### Scenario: Redlined set
- **WHEN** a set contains samples at full scale
- **THEN** the report shows the clipped sample count, percentage, and flat factor, and states that loudness normalization lowers level but does not remove clipping distortion

### Requirement: Loudness Normalization and Batch Export
The system SHALL export selected raw artist FLACs, normalized artist FLACs, full-event raw and normalized FLACs, `.cue` sheets, and optional 320 kbps MP3, using the selected preset.

#### Scenario: Streaming preset
- **WHEN** the user selects `Streaming` (-14.0 LUFS, -1.0 dBTP ceiling)
- **THEN** each set is high-pass filtered at 25 Hz, gain-adjusted toward -14.0 LUFS, and peak-limited with oversampled detection to -1.0 dBTP, then tagged and joined at the set boundaries into the full-event normalized FLAC

#### Scenario: Club preset or custom gain
- **WHEN** the user selects `Club` (-11.0 LUFS, -1.0 dBTP ceiling) or sets custom per-artist gain
- **THEN** outputs are rendered with those parameters

#### Scenario: Optional de-clip stage
- **WHEN** the user enables de-clipping for a set with clipped samples
- **THEN** a de-clip stage runs before normalization and the report shows flat factor before and after

### Requirement: Measured Output Verification
The system MUST re-measure every rendered audio file and report measured Integrated LUFS, True Peak, and Sample Peak, never intended target values.

#### Scenario: Output misses its target
- **WHEN** a rendered file's measured loudness differs from target by more than 1.0 LU or its true peak exceeds the ceiling
- **THEN** the report flags the file with its measured values

### Requirement: Multi-Format Event Report Generation
The system SHALL write `Event_Report.html`, `Event_Report.md`, and `Event_Report.json` in the event output directory.

#### Scenario: Report after analysis or render
- **WHEN** analysis or rendering completes
- **THEN** the reports contain the continuity audit, ambient alignment, boundary table with detection method per boundary, measured before/after metrics, and USB/tracklist summaries
