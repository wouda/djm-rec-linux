# Proposal

## Why
When the djm-box recording appliance is connected to an existing venue or studio Wi-Fi network rather than operating as an isolated hotspot (10.42.0.1), Android devices cannot always resolve mDNS domains like `djm-box.local` due to Android DNS subsystem limitations. Adding automatic and manual /24 subnet probing allows the Android companion app to reliably discover and connect to djm-box on any local Wi-Fi network without requiring manual IP entry. Additionally, updating the authentication PIN to `125555` standardizes the appliance credentials for booth operation.

## What Changes
- **Subnet Auto-Discovery**: Companion app scans the local IPv4 `/24` subnet (e.g. `192.168.x.1` through `192.168.x.254`) on port 8080 targeting `/api/status` using pure Dart `dart:io` interfaces in parallel batches.
- **Dual Triggering**: Subnet scanning runs automatically as a fallback when direct candidates (`10.42.0.1`, `djm-box.local`, saved host) fail to connect, and can also be triggered manually via a "Scan Subnet" button in the Disconnected banner and Pairing screen.
- **Connection & Device Selection**: If 1 responsive appliance is discovered and a PIN is saved, the app auto-connects immediately; if multiple appliances or no saved PIN, it displays the discovered appliance(s) for selection and PIN entry.
- **Zero Native Plugins**: Uses pure Dart `NetworkInterface.list()` and `HttpClient`, avoiding third-party Android plugins and preserving CI build reliability.
- **Config PIN Update**: Sets `auth_token = "125555"` in `~/.config/djm-box/config.toml` and updates default fallbacks/hint texts in `crates/djm-common` and Flutter companion app.
- **Daemon Untouched**: Strictly preserves `crates/djm-daemon` without modifications.

## Capabilities

### New Capabilities
- `wifi-subnet-scanning`: Subnet IP range derivation and parallel HTTP probing for companion app Wi-Fi auto-discovery.
- `appliance-auth-pin`: Updated appliance authentication PIN configuration (`125555`) across daemon config and app defaults.

### Modified Capabilities
- None.

## Impact
- `flutter_app/lib/services/transport_manager.dart`: Local subnet extraction and batch probing logic.
- `flutter_app/lib/screens/dashboard_screen.dart` & `pairing_screen.dart`: UI integration for subnet scan action and multi-appliance discovery.
- `flutter_app/test/telemetry_test.dart`: Subnet probe and URL tests updated.
- `~/.config/djm-box/config.toml`: Updated auth token to `125555`.
- `crates/djm-common/src/config.rs`: Default auth token fallback updated to `125555`.
