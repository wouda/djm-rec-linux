# Design

## Context
See `proposal.md` for background. The Android companion app previously relied solely on fixed candidate addresses (`10.42.0.1`, `djm-box.local`, and previously saved hostnames). In venue environments where djm-box is connected to local Wi-Fi rather than running a standalone hotspot, mDNS resolution is frequently unsupported by Android's DNS resolver.

## Goals / Non-Goals

**Goals:**
- Implement pure Dart `/24` subnet scanning using `dart:io` `NetworkInterface.list()`.
- Scan 254 candidate IP addresses concurrently in bounded batches of 25 with a short 500ms timeout against `http://<ip>:8080/api/status`.
- Provide both automatic fallback and explicit manual triggering ("Scan Subnet" button) in the UI.
- Handle single-appliance auto-connect (with saved PIN) vs. multi-appliance selection.
- Update active configuration PIN and default codebase fallbacks to `125555`.
- Maintain 100% build stability by avoiding third-party native Android plugins.

**Non-Goals:**
- Modifying `crates/djm-daemon` (strictly forbidden; daemon remains untouched).
- Adding complex network plugins (`network_info_plus`, etc.) that could introduce AGP/Kotlin build issues.
- Multi-subnet scanning beyond the local interface's `/24` prefix.

## Decisions

### 1. Pure Dart Network Discovery
- **Decision**: Use `dart:io` `NetworkInterface.list(type: InternetAddressType.IPv4)` to retrieve active non-loopback addresses and extract the `/24` subnet prefix (e.g. `192.168.1.`).
- **Rationale**: Introduces zero native Android code or Gradle plugins, preventing any CI or AGP compatibility regressions.
- **Alternatives Considered**: `network_info_plus` (requires extra Android native permissions and plugin configurations).

### 2. Bounded Batch Probing
- **Decision**: Probe candidate IPs in batches of 25 concurrent `HttpClient` requests with a 500ms timeout per request.
- **Rationale**: 254 IP addresses complete in ~5–6 seconds total without overwhelming the mobile device socket pool or local Wi-Fi router.
- **Validation**: Verifies responses match djm-box API criteria (HTTP 200 or 401 with JSON containing `state` or `is_mixer_connected`).

### 3. Connection Precedence & Selection
- **Decision**:
  1. Direct candidates: `10.42.0.1`, `djm-box.local`, saved host.
  2. Subnet scan fallback if direct candidates fail.
  3. Single device found + saved PIN → auto-connect.
  4. Multiple devices or no saved PIN → present selection card/dialog.

### 4. Config PIN Update
- **Decision**: Update `~/.config/djm-box/config.toml` (`auth_token = "125555"`), update `default_auth_token()` in `crates/djm-common/src/config.rs`, and update UI placeholder hints.

## Risks / Trade-offs

- **[Subnet isolation / Client isolation on public Wi-Fi]** → On guest Wi-Fi networks with client isolation enabled, subnet probes may not reach peer devices. Mitigation: The user can still connect directly to the djm-box booth hotspot (`10.42.0.1`) or use the QR code pairing / Tailscale remote URL.
- **[Battery impact of subnet scans]** → Continuous background scanning could drain battery. Mitigation: Scans only run once as a fallback when disconnected, or when triggered manually via the UI button.
