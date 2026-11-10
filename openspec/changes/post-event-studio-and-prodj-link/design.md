# Design

## Context

`djm-rec` runs as separate processes: `djm-daemon` (ALSA USB-B capture), `djm-remote` (web/hotspot bridge on `:8080`, dashboard served from an inline HTML string in `web_server.rs`), `djm-tui`, and the Flutter companion app. `djm-daemon` writes 3-hour rollover segments (`*_pt01..ptN.flac`) and session logs with `[GATE_*]`, `[BOOKMARK]`, `[DIGITAL_CLIP]`, and `[SEGMENT_SPLIT]` events. See `proposal.md` for motivation.

Findings from the manual run on the 2026-10-30 event that shape this design:
- `clip_counter` / `Telemetry.clip_count` / session log clip fields are `u16` and saturated at 65,535 in pt07.
- Log timestamps are local time without offset.
- `pt04_ambient.flac` ran ~4 h (22:00–02:00), pt05/pt06 have no ambient file, and `pt08_ambient` is ~98 s longer than `pt08`.
- Parsing ffmpeg's human-readable stderr was brittle (per-frame `ebur128` logs, hung measurement runs).
- `alimiter` limits sample peaks only; attenuating a clipped set does not remove its flat-topped peaks.
- `djm-box setup` does not install `ffmpeg`.

## Goals / Non-Goals

**Goals:**
- Keep `djm-daemon` isolated from network protocol handling and from heavy audio processing; its only changes are counter width, log offsets, and ambient rotation.
- Never disrupt physical CDJs: passive start, high player number only, release on any new device, never contest a number.
- One engine shared by `djm-remote` and `djm-cli`.
- Report only measured values for rendered outputs.
- Work on both x86_64 and aarch64 (Raspberry Pi), with honest time estimates per platform.

**Non-Goals:**
- Real-time DSP in `djm-daemon`.
- Cloud processing or external databases.
- Multi-band compression or automatic EQ matching.
- Guaranteed removal of clipping distortion (de-clip is optional and best-effort).

## Decisions

### 1. Recorder accuracy fixes first
- Widen clip counters to `u64` end to end (`audio_engine.rs`, `session_log.rs`, `protocol.rs`, Flutter `telemetry.dart`, web JS). JSON numbers up to 2^53 are safe in Dart and JS.
- Log timestamps as RFC 3339 with offset (`2026-10-30T23:00:02.921+01:00`). The parser accepts both old (no offset) and new formats; old logs are interpreted in the system timezone with a warning in the report.
- Rotate the ambient writer whenever a new master segment opens (including after silence-timeout stops), writing the master segment name and sample offset into ambient FLAC tags. Older sessions are handled by the alignment audit.
- **Why first**: the reports depend on these values; fixing them before building reports avoids building workarounds.

### 2. Pro DJ Link daemon with a hardware spike before full build
- `djm-prodj` is a standalone binary (`djm-box-prodj.service`), socket `/run/user/1000/djm-prodj.sock`, log `<session>_prodj.jsonl`.
- **Spike (task group 4) must confirm on real hardware before the daemon is built:**
  1. Exact packet types for device announce and number-claim stages, using Deep Symmetry's *dysentery* protocol documentation and packet captures. The spec states behavior ("release on any new device announcement or claim"), not packet IDs, so the IDs live in code only.
  2. Whether CDJ status packets (carrying the loaded track ID) reach a virtual player on number 5/6 from CDJ-3000 and from NXS2-generation players, and whether NXS2 players serve metadata to numbers above 4.
  3. Whether USB media databases (`export.pdb`, `DEVSETTING.DAT`) can be read from players without holding a player number, and which method works (NFS vs. the players' database server).
  4. Release latency measured as packets processed between first new-device packet and our last announcement (target: zero further announcements after the first new-device packet is processed).
- **Participation rules** (from the spec): passive until settled; announce only on the highest free supported number (6/5 on 6-player capable networks, 4 on 4-player networks with ≤3 players); on 4-player networks only after the event is live with 3 minutes of continuous audio and no new devices; release immediately and silently on any new device; never send a contesting packet.
- **Libraries**: use `rekordcrate` for `export.pdb` parsing rather than writing a parser; write the UDP layer in-house (small, well documented).
- **Degraded mode** is a first-class state: when passive, logs still carry On-Air timing per deck and any media identity obtained, so boundary detection still benefits.

### 3. On-Air play detection and B2B clustering
- Played = playing and On-Air for ≥20 cumulative seconds; fallback ≥30 s playhead advance if no mixer On-Air data.
- Sliding-window clustering over played tracks' media IDs merges interleaving media into one B2B cluster.

### 4. Shared engine in `djm-common::studio`
- Session grouper, log parser (old and new timestamp formats), timetable parser anchored to an event date, boundary resolver, analyzer, renderer, verifier, and report/tracklist/CUE writers.
- **Continuity check**: for consecutive rollover segments, `samples / sample_rate` of segment N must equal `start(N+1) - start(N)` (plus pre-roll on the first segment) within 50 ms. Verified against pt06/pt07: 10802.02 s vs 10800.003 s + 2.0 s pre-roll.

### 5. Analysis and rendering pipeline
- **Measurement**: use machine-readable output only: `ffmpeg -af loudnorm=print_format=json` for integrated loudness, LRA, and true peak; `astats=metadata=1` with `ametadata=print:file=` for sample peak, RMS, crest and flat factor; `silencedetect` lines parsed per event. Every subprocess gets a timeout and `-nostats`. In-process measurement with the `ebur128` crate is an allowed alternative if decoding cost is acceptable.
- **Normalization chain**: `highpass=f=25` → `volume={gain}dB` → upsample 4× → `alimiter=limit={ceiling}:level=false` → downsample to source rate → optional `adeclip` before the chain. Gain is computed from the measured loudness of the high-passed signal. Limiting at 4× rate approximates true-peak control; final true peak is still measured, not assumed.
- **Presets**: `Streaming` −14.0 LUFS / −1.0 dBTP; `Club` −11.0 LUFS / −1.0 dBTP (the ceiling stays at −1.0 because louder masters overshoot more on lossy encoding); `Custom` target and per-artist gain.
- **Verification**: after rendering, every file is measured with the same measurement path; the report shows measured values and flags misses (>1.0 LU from target, or true peak above ceiling).
- **Full event**: concatenate rendered artist files at their silence-midpoint boundaries.
- **Priority and abort**: subprocesses run under `nice -n 19` and `ionice -c 3`; the job runner watches daemon telemetry and kills the process group on `Recording`.
- **Disk estimate**: per output, estimated from source bitrate × duration (FLAC) or 320 kbps × duration (MP3), plus 10%.

### 6. `djm-remote` API, jobs, and web assets
- Endpoints: `GET /api/studio/sessions`, `POST /api/studio/timetable`, `POST /api/studio/analyze`, `GET /api/studio/preview-transition`, `POST /api/studio/render`, `GET /api/studio/jobs/{id}`, `GET /api/studio/report`. All require the existing PIN token.
- Session parameters are looked up in the discovered session list; anything else is rejected before any filesystem access.
- Preview uses the existing `ProcessStream` pattern with a semaphore (default 2).
- Job state is written to `<event_dir>/.studio_job.json` after each step; on startup, running jobs become `Interrupted`.
- The Studio UI lives in separate files under `crates/djm-remote/assets/` included with `include_str!`, instead of growing the inline `DASHBOARD_HTML` string.

## Risks / Trade-offs

- **[Risk] CPU/IO load during jobs affects a resumed recording** → quiet-period guardrail, idle priority, abort on `Recording`.
- **[Risk] Protocol assumptions wrong on real hardware** → spike gates the daemon build; spec avoids packet-level guarantees; degraded passive mode remains useful.
- **[Risk] Venue does not allow connecting to the booth switch** → `djm-prodj` is optional; all studio features work from audio and logs alone.
- **[Risk] Clipping cannot be undone** → report states this explicitly; de-clip is optional with before/after flat factor.
- **[Risk] Slow renders on Raspberry Pi** → measure on aarch64 during implementation and show per-platform estimates in the UI.
- **[Risk] Disk exhaustion (~36 GB per 7-hour event with all outputs)** → pre-render estimate and safety margin; raw artist splits optional since `.cue` on the full raw file covers navigation.
- **[Trade-off] Widening `clip_count` changes the telemetry wire type** → clients updated in the same change; JSON keeps it backward compatible for values under 65,536.

## Open Questions

- Default quiet period (10 minutes) and disk safety margin (20 GB) may be tuned after field use; they are configuration values and do not change the design.
