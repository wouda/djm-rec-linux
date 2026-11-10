# Proposal

## Why

After recording a multi-hour club event across 3-hour rollovers (`pt01..ptN.flac`), users currently have to manually verify segment continuity, locate DJ handover silences, inspect redlining/clipping damage, normalize large loudness discrepancies (often 8–9 dB between openers and headliners), split sets by artist, and assemble tracklists. Integrating an automated, fault-isolated **Post-Event Studio** and optional **Pioneer Pro DJ Link (`djm-prodj`)** daemon into `djm-rec` turns raw multi-segment recordings into level-matched, tagged artist sets, full-event files, tracklists, and audio health reports from the app.

A manual dry run on the 2026-10-30 DJM-A9 event also exposed recorder issues that make reports unreliable: the session clip counter saturates at 65,535, session logs carry local time without a UTC offset, and ambient files do not rotate in step with master segments.

## What Changes

- **Recorder accuracy fixes (`djm-daemon`, `djm-common`)**:
  - Widen clip counters from `u16` to `u64` in telemetry, session logs, and the audio engine so totals no longer saturate at 65,535.
  - Write session log timestamps with a UTC offset so timelines remain unambiguous across DST transitions.
  - Rotate ambient FLAC files together with master segments, or record their master-segment alignment explicitly.
- **New optional Pro DJ Link daemon (`djm-prodj`)**:
  - Separate, fault-isolated daemon (`djm-box-prodj.service`) that observes CDJ deck state, DJM channel On-Air flags, active USB media, and track metadata on the booth Ethernet network.
  - Starts passive; joins as a virtual player only on a high player number when it is safe to do so, and releases that number as soon as any new device starts announcing itself. It never contests a number.
  - Logs played tracks (On-Air for a minimum play time, with playhead fallback) and USB media to `*_prodj.jsonl`, broadcasts live deck state over `/run/user/1000/djm-prodj.sock`, and clusters alternating USB sticks during B2B sets.
  - Protocol details (packet types, which player numbers older CDJs serve metadata to, metadata access without a player number) are confirmed in a hardware spike before the full implementation.
- **Post-event analysis, splitting, and loudness normalization engine**:
  - Groups contiguous event segments (`pt01..ptN.flac` + `.log` + `_prodj.jsonl`), flags pre-event warmups, and verifies rollover continuity using a defined sample-count check.
  - Parses pasted text timetables (`HH:MM Artist`) anytime and snaps set boundaries using silence midpoints, bookmarks, and Pro DJ Link USB On-Air handovers, falling back to the clock for review.
  - Measures per-set Sample Peak, True Peak, RMS, Integrated LUFS, LRA, Crest Factor, flat factor, and clipped samples using machine-readable analyzer output.
  - Renders selected outputs: raw artist FLACs, loudness-normalized artist FLACs (25 Hz high-pass, gain to target LUFS, oversampled peak limiting to the preset ceiling, optional de-clip stage), full-event raw and normalized FLACs, `.cue` sheets, `.txt` tracklists, and optional 320 kbps MP3. Every rendered file is re-measured and the measured values (not intended targets) go into the report.
  - Writes `Event_Report.html`, `Event_Report.md`, and `Event_Report.json`.
  - Blocks heavy analysis and rendering while recording is active or the recorder has only recently gone silent, checks free disk space before rendering, and runs jobs at low CPU/IO priority with abort on recording resume.
- **Multi-surface UI and CLI (`djm-remote`, `flutter_app`, `djm-tui`, `djm-cli`)**:
  - Live "Now Playing" deck cards when `djm-prodj` is active.
  - A Post-Event Studio workflow in the Web Dashboard and Flutter app: timetable input, USB-to-artist mapping, split review with 15-second transition preview, preset and format selection, persistent job progress, and an embedded report viewer.
  - `djm-box process` and `djm-box prodj` subcommands; `djm-box setup` installs `ffmpeg`.

## Capabilities

### New Capabilities
- `post-event-processing`: Session grouping, continuity auditing, boundary detection, audio quality analysis, loudness normalization and splitting, tracklist/CUE export, report generation, live-recording guardrails, and the recorder accuracy fixes that reports depend on.
- `prodj-link-telemetry`: Fault-isolated Pro DJ Link daemon for passive discovery, safe virtual player participation, On-Air track logging, USB media identification, and B2B clustering.
- `studio-remote-ui`: Web Dashboard, Flutter app, TUI, and CLI surfaces for live deck monitoring and the Post-Event Studio.

### Modified Capabilities
<!-- No existing specs in openspec/specs/ -->

## Impact

- **Crates & binaries**: new `crates/djm-prodj` (+ `djm-box-prodj.service`); updates to `djm-common` (studio models, timetable parser, report types, ProDJ socket protocol, `u64` clip count), `djm-daemon` (clip counter width, log UTC offsets, ambient rotation), `djm-remote` (studio API, job runner, preview streaming, web assets split out of the inline dashboard string), `djm-cli` (`process` / `prodj` subcommands, ffmpeg dependency, service install), `djm-tui` (deck status), and `flutter_app` (Studio screen, deck cards).
- **Dependencies**: `ffmpeg` / `ffprobe` become a required system dependency. Candidate Rust crates: `rekordcrate` (rekordbox `export.pdb` parsing), optionally `ebur128` for in-process loudness measurement.
- **Operational**: Pro DJ Link requires an Ethernet connection to the booth switch, which some venues will not permit; everything else works without it. A full render of a 7-hour 96 kHz event needs roughly 36 GB if every output is selected.
- **Compatibility**: the `Telemetry.clip_count` wire type widens to `u64`; the Flutter and web clients must accept the larger value.
