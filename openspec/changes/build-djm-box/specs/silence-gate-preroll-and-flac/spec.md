# Spec Delta: silence-gate-preroll-and-flac

## Purpose

Provides a crash-proof, lossless 24-bit FLAC audio encoding pipeline with pre-roll circular buffering, club-tuned silence gating, zero-gap background splitting, header checkpointing, and ReplayGain metadata calculation.

## ADDED Requirements

### Requirement: Bit-Perfect Lossless FLAC Encoding
The recording engine SHALL encode captured PCM audio directly to 24-bit lossless FLAC at the mixer's native sample rate (96000 Hz or 48000 Hz) without software sample rate conversion or gain multiplication.

#### Scenario: 96 kHz mixer capture stream
- **WHEN** audio samples arrive from a DJM-V10 mixer operating at 96 kHz
- **THEN** the FLAC stream encoder SHALL write 24-bit FLAC blocks configured for 96000 Hz native stereo samples

### Requirement: Continuous Pre-Roll Circular Buffer in Memory
The recording engine SHALL maintain a rolling circular buffer of raw PCM audio in RAM covering at least `preroll_buffer_secs` (default 2.0 seconds) while in the `ARMED_SILENT` state without writing to disk.

#### Scenario: Audio detection from silence
- **WHEN** audio levels rise above `wake_threshold_dbfs` (default `-65.0 dBFS`) while in `ARMED_SILENT`
- **THEN** the engine SHALL open a new `.flac.part` file, prepend the entire 2-second RAM pre-roll buffer, and transition to the `RECORDING` state

### Requirement: Club-Tuned Silence-Based Pause-Split Automation
The recording engine SHALL finalize the active recording segment and transition to `ARMED_SILENT` if both stereo channels remain below `silence_threshold_dbfs` (default `-75.0 dBFS`) for `pause_split_timeout_secs` (default `120.0` seconds).

#### Scenario: Long silence after final DJ finishes
- **WHEN** audio levels stay below `-75.0 dBFS` continuously for 120.0 seconds while recording
- **THEN** the engine SHALL finalize the active FLAC file, finalize the sidecar log, and transition to `ARMED_SILENT` without false splits during brief track breakdowns

### Requirement: Sample-Accurate Zero-Gap File Splitting
The recording engine SHALL roll over into a new timestamped FLAC file with zero missing PCM samples when reaching `max_segment_duration_mins` or receiving a manual split command, offloading previous file finalization to a background worker.

#### Scenario: Manual split or segment duration rollover during live music
- **WHEN** the active file reaches `max_segment_duration_mins` (180 minutes) or the user triggers a manual split during continuous music
- **THEN** the writer SHALL write sample $N$ to the ending file and sample $N+1$ to the new file immediately while finalizing tags and headers asynchronously

### Requirement: Crash-Proof Storage Sync and Header Checkpointing
The recording engine SHALL invoke `fdatasync()` every `fsync_interval_secs` (default 2.0 seconds), checkpoint `STREAMINFO` `total_samples` every 30 seconds, and auto-repair interrupted `.flac.part` files on startup.

#### Scenario: Host power failure during active recording
- **WHEN** a sudden system power cut occurs during recording and the system boots back up
- **THEN** the daemon SHALL locate the interrupted `.flac.part` file, verify its header checkpoint, and finalize it into a valid playable `.flac` file

### Requirement: Pre-Allocated Header Padding and ReplayGain Tagging
The recording engine SHALL reserve a 4 KB `PADDING` metadata block in every FLAC header at creation and write `REPLAYGAIN_TRACK_GAIN`, `REPLAYGAIN_TRACK_PEAK`, and `DJM_GAIN_STEPS` tags in-place upon closure.

#### Scenario: Active multi-gigabyte file finalized
- **WHEN** a 3-hour audio recording segment is closed
- **THEN** the background finalizer SHALL write ReplayGain Vorbis comment tags directly into the pre-allocated 4 KB header padding without rewriting the audio payload on disk
