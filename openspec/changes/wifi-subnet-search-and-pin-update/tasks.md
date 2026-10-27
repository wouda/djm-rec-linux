# Tasks

## 1. Config PIN Update (125555)

- [x] 1.1 Update `auth_token = "125555"` in `~/.config/djm-box/config.toml` and verify file contents
- [x] 1.2 Update `default_auth_token()` in `crates/djm-common/src/config.rs` and verify `cargo test -p djm-common` passes

## 2. Companion Subnet Discovery Service

- [x] 2.1 Implement local subnet prefix resolution using pure Dart `dart:io` `NetworkInterface.list()` in `TransportManager`
- [x] 2.2 Implement batch subnet probe in `TransportManager` scanning `/24` candidates in parallel batches against `/api/status` with 500ms timeout
- [x] 2.3 Hook subnet discovery as automatic fallback in `_connectPriorityChain` and handle single device auto-connect vs. multiple devices list

## 3. UI Controls and Device Selection

- [x] 3.1 Add manual "Scan Subnet" action button and scanning status feedback to `DashboardScreen` disconnected hero card
- [x] 3.2 Add manual subnet scan and discovered appliance selection in `PairingScreen`
- [x] 3.3 Update PIN input placeholders and hints to `125555` across `DashboardScreen` and `PairingScreen`

## 4. Verification and Validation

- [x] 4.1 Add unit tests for subnet derivation and URL formation in `flutter_app/test/telemetry_test.dart` and verify passing
- [x] 4.2 Run cargo test across workspace to verify all crates pass with updated PIN defaults
