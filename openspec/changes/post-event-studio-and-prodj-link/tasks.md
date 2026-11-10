# Tasks

## 1. Recorder Accuracy Fixes (`djm-daemon`, `djm-common`)

- [x] 1.1 Widen clip counters from `u16` to `u64` in `audio_engine.rs`, `session_log.rs`, and `Telemetry.clip_count` in `protocol.rs`; verify a unit test with >65,535 simulated clips shows correct totals in telemetry and the session log
- [x] 1.2 Update Flutter `telemetry.dart` and the dashboard JS to accept `u64` clip counts; verify `flutter test` and a manual dashboard check with a value above 65,535
- [x] 1.3 Write session log timestamps as RFC 3339 with UTC offset; verify `session_log` unit tests check the format
- [x] 1.4 Rotate the ambient writer whenever a master segment opens (including after silence-timeout stops) and tag ambient files with master segment name and sample offset; verify with `djm-daemon --mock` that each master segment gets a matching ambient file

## 2. Studio Models, Session Grouping & Timetable (`djm-common`)

- [x] 2.1 Add studio data models (`EventSession`, `SegmentInfo`, `ArtistSlot`, `SetBoundary`, `SetQualityMetrics`, `Preset`, `RenderOptions`, `JobState`, `EventReport`) and ProDJ protocol structs; verify `cargo check -p djm-common`
- [x] 2.2 Implement the log parser accepting old (no offset) and new (offset) timestamps; verify unit tests using copies of the 2026-10-30 pt04–pt08 logs
- [x] 2.3 Implement session grouping, warmup detection, and the sample-count continuity check (50 ms tolerance); verify a unit test reproduces pt06→pt07→pt08 as continuous and pt04/pt05 as warmups
- [x] 2.4 Implement the ambient alignment audit; verify it reports the pt04 ambient mismatch from the 2026-10-30 logs
- [x] 2.5 Implement the timetable parser anchored to an event date (overnight wrap) and the boundary resolver (silence midpoint → USB handover → clock + review flag); verify unit tests reproduce the 7 boundaries found manually for 2026-10-30 within 1 s
- [x] 2.6 Implement the DST case in the resolver; verify a unit test with a synthetic log spanning a DST change

## 3. Analysis, Rendering, Verification & Reports (`djm-common`, `djm-cli`)

- [x] 3.1 Add `ffmpeg`/`ffprobe` to `djm-box setup` dependencies and a startup availability check; verify `djm-box setup --dry-run` lists ffmpeg
- [x] 3.2 Implement measurement via `loudnorm` JSON, `astats` metadata files, and `silencedetect`, with subprocess timeouts and `-nostats`; verify unit tests against recorded ffmpeg outputs and a 30 s fixture
- [x] 3.3 Implement the job runner: `nice`/`ionice`, quiet-period guardrail, abort on `Recording` (kill process group, delete partial outputs), disk-space estimate and margin check; verify tests with a mocked telemetry source
- [x] 3.4 Implement the normalization renderer (high-pass, gain, 4× oversampled limiting, optional de-clip, presets, tags, MP3) and full-event concatenation; verify a fixture render meets target within 1.0 LU and ceiling by measured true peak
- [x] 3.5 Implement post-render verification that re-measures every output and flags misses; verify a test where a forced miss is flagged
- [x] 3.6 Implement `.txt` tracklist, `.cue` sheet, and `Event_Report.{html,md,json}` writers containing measured values only and a clipping-is-not-repaired note for redlined sets; verify snapshot tests
- [x] 3.7 Add `djm-box process` (session, timetable file, preset, outputs) with terminal progress; verify an end-to-end run on a short mock session and document it in `README.md`
- [x] 3.8 Measure render time per hour of 96 kHz audio on x86_64 and aarch64 and record the figures in `README.md`

## 4. Pro DJ Link Hardware Spike (`crates/djm-prodj`)

- [x] 4.1 Capture Pro DJ Link traffic on real hardware (DJM-A9 or V10 with CDJ-3000; DJM-900NXS2 with NXS2 if available) and document announce/claim packet types against the dysentery docs in `crates/djm-prodj/PROTOCOL_NOTES.md`; verify the notes include annotated captures
- [x] 4.2 Test whether status packets reach a virtual player on numbers 5/6 and whether NXS2 players serve metadata to numbers above 4; record results in `PROTOCOL_NOTES.md`
- [x] 4.3 Test reading `export.pdb` and `DEVSETTING.DAT` without a player number (NFS vs. database server) and parse them with `rekordcrate`; record results and update `design.md` if a decision changes

## 5. Pro DJ Link Daemon (`crates/djm-prodj`)

- [x] 5.1 Create the crate with passive UDP listeners, device inventory, and mixer On-Air parsing; verify `cargo test -p djm-prodj` with captured packets from 4.1
- [x] 5.2 Implement the participation state machine (passive start, settling, highest free supported number, 3-minute live-audio gate on 4-player networks, release on any new device, no contesting packets); verify tests replaying a mid-event CDJ boot capture show no announcement after the first new-device packet
- [x] 5.3 Implement USB media identification and metadata resolution using the method chosen in 4.3, with degraded mode when unavailable; verify tests with fixture databases
- [x] 5.4 Implement On-Air play detection (20 s On-Air / 30 s playhead fallback) and B2B clustering; verify tests for headphone-cue filtering and alternating media
- [x] 5.5 Implement `<session>_prodj.jsonl` logging and the `/run/user/1000/djm-prodj.sock` broadcaster; verify an integration test reading the socket
- [x] 5.6 Add `djm-box-prodj.service` to setup and a `djm-box prodj` subcommand (status/devices); verify `djm-box prodj --help` and document the Ethernet requirement in `README.md`

## 6. `djm-remote` API, Jobs & Web Studio

- [x] 6.1 Subscribe `djm-remote` and `djm-tui` to the ProDJ socket and show deck cards with "unavailable" fields in degraded mode; verify `cargo test -p djm-remote -p djm-tui`
- [x] 6.2 Add `/api/studio/sessions` and `/api/studio/timetable` with session-ID validation against the discovered list; verify tests that traversal attempts are rejected before filesystem access
- [x] 6.3 Add `/api/studio/analyze`, `/api/studio/render`, `/api/studio/jobs/{id}`, and `/api/studio/report` using the job runner, persisting job state to `.studio_job.json` and marking running jobs `Interrupted` on startup; verify API tests including 409 during recording and restart recovery
- [x] 6.4 Add `/api/studio/preview-transition` with a concurrency limit of 2; verify a test that a third concurrent request gets a busy error
- [x] 6.5 Move Studio web UI into `crates/djm-remote/assets/` via `include_str!` and build the timetable editor and USB mapping view; verify in a browser against a mock session
- [x] 6.6 Build the web split review table with preview player, preset/format selection, disk estimate, progress, and report viewer; verify an end-to-end render from the browser on a mock session

## 7. Flutter Companion App

- [x] 7.1 Add models and `transport_manager.dart` calls for ProDJ telemetry and `/api/studio/*`; verify `flutter analyze` and model tests
- [x] 7.2 Add deck cards to `dashboard_screen.dart`; verify widget tests including degraded mode
- [x] 7.3 Build `PostEventStudioScreen` timetable editor and USB mapping; verify widget tests
- [x] 7.4 Build the split review table with transition preview, preset/format selection, progress, and report view; verify widget tests and a manual run against a mock session

## 8. Integration

- [x] 8.1 Run the full pipeline on the 2026-10-30 recordings and verify the 7 boundaries, measured metrics, and continuity audit match the manual analysis (allowing for the corrected clip totals)
