# Tasks

## 1. Configuration & Hardware Controls

- [x] 1.1 Add `AudioConfig`, `DjmMicConfig`, and `AmbientMicConfig` to `crates/djm-common/src/config.rs` with unit tests covering default values
- [x] 1.2 Update `AlsaMixerController::configure_rec_out` in `crates/djm-daemon/src/mixer_controls.rs` to set `Mic to Rec Out` to 0 (unlinked) by default when `include_in_master` is false

## 2. Host Ambient Microphone Capture

- [x] 2.1 Implement host capture card auto-detection in `crates/djm-daemon/src/ambient_capture.rs` filtering out Pioneer mixer cards
- [x] 2.2 Implement real-time `AmbientCaptureLoop` with dedicated ring buffer and native sample rate capture
- [x] 2.3 Implement the post-set trailing hangover timer (default 15 minutes) with cancellation on mixer playback resumption

## 3. Multi-Stream FLAC Writing & DAW Alignment

- [x] 3.1 Update `FlacWriter` to write Vorbis comment tags (`STREAM_TYPE`, `SESSION_ID`, `START_TIME_UTC`, `SAMPLE_OFFSET`)
- [x] 3.2 Update `AudioEngine` in `crates/djm-daemon/src/audio_engine.rs` to coordinate Master, DJM Mic, and Ambient streams
- [x] 3.3 Update `SessionLog` in `crates/djm-daemon/src/session_log.rs` to log start, split, and completion of all active streams

## 4. CI/CD Release Automation & Documentation

- [x] 4.1 Create `.github/workflows/release.yml` building `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` release tarballs on tag push (`v*`)
- [x] 4.2 Update `README.md` with GitHub Releases download badge and architecture-specific installation links

## 5. Verification & Testing

- [x] 5.1 Run full unit and integration test suite (`cargo test --all`) verifying multi-stream synchronization and config parsing
