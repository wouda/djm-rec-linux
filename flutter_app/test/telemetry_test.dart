import 'dart:typed_data';
import 'package:flutter_test/flutter_test.dart';

import 'package:djm_companion/models/command.dart';
import 'package:djm_companion/models/pairing_info.dart';
import 'package:djm_companion/models/telemetry.dart';

void main() {
  group('Telemetry Binary Packing and Unpacking', () {
    test('Unpack 20-byte binary frame matching djm_common', () {
      final bytes = Uint8List(20);
      final bd = ByteData.sublistView(bytes);

      // State: Recording (2)
      bd.setUint8(0, 2);

      // Byte 1: hw_pad_db = -19 dB, auto_guard = true, silence_countdown = true
      // -19 + 64 = 45 (0x2D). 0x2D | 0x40 | 0x80 = 0xED
      bd.setUint8(1, 0xED);

      // Peak L: -18.4 dBFS (-1840 centi-dB)
      bd.setInt16(2, -1840, Endian.little);
      // Peak R: -17.9 dBFS (-1790 centi-dB)
      bd.setInt16(4, -1790, Endian.little);

      // RMS L: -22.1 dBFS (-2210)
      bd.setInt16(6, -2210, Endian.little);
      // RMS R: -21.8 dBFS (-2180)
      bd.setInt16(8, -2180, Endian.little);

      // Clip count: 3
      bd.setUint16(10, 3, Endian.little);

      // Active duration: 840 seconds
      bd.setUint32(12, 840, Endian.little);

      // Active size: 185 MB
      bd.setUint16(16, 185, Endian.little);

      // Free disk: 457.5 GB (4575 tenths)
      bd.setUint16(18, 4575, Endian.little);

      final t = Telemetry.fromPackedBytes(bytes);

      expect(t.state, RecordingState.recording);
      expect(t.isRecording, true);
      expect(t.hwPadDb, -19);
      expect(t.autoGuardActive, true);
      expect(t.silenceCountdownActive, true);
      expect(t.peakLDb, closeTo(-18.4, 0.01));
      expect(t.peakRDb, closeTo(-17.9, 0.01));
      expect(t.rmsLDb, closeTo(-22.1, 0.01));
      expect(t.rmsRDb, closeTo(-21.8, 0.01));
      expect(t.clipCount, 3);
      expect(t.activeFileDurationSecs, 840);
      expect(t.formattedDuration, "00:14:00");
      expect(t.activeFileSizeMb, 185);
      expect(t.freeDiskGb, closeTo(457.5, 0.01));
    });

    test('Telemetry pack and unpack round trip', () {
      const original = Telemetry(
        state: RecordingState.armedSilent,
        hwPadDb: -15,
        autoGuardActive: true,
        silenceCountdownActive: false,
        peakLCentiDb: -2500,
        peakRCentiDb: -2450,
        rmsLCentiDb: -3000,
        rmsRCentiDb: -2980,
        clipCount: 0,
        activeFileDurationSecs: 3661,
        activeFileSizeMb: 512,
        freeDiskGbTenths: 2000,
      );

      final packed = original.toPackedBytes();
      expect(packed.length, 20);

      final recovered = Telemetry.fromPackedBytes(packed);
      expect(recovered.state, original.state);
      expect(recovered.hwPadDb, original.hwPadDb);
      expect(recovered.autoGuardActive, original.autoGuardActive);
      expect(recovered.silenceCountdownActive, original.silenceCountdownActive);
      expect(recovered.peakLCentiDb, original.peakLCentiDb);
      expect(recovered.peakRCentiDb, original.peakRCentiDb);
      expect(recovered.clipCount, original.clipCount);
      expect(recovered.activeFileDurationSecs, original.activeFileDurationSecs);
      expect(recovered.formattedDuration, "01:01:01");
      expect(recovered.activeFileSizeMb, original.activeFileSizeMb);
      expect(recovered.freeDiskGbTenths, original.freeDiskGbTenths);
    });
  });

  group('Control Commands Binary Serialization', () {
    test('Auth command serializes with opcode 0x00 and token bytes', () {
      final cmd = AuthCommand("849201");
      final bytes = cmd.toBytes();
      expect(bytes[0], 0x00);
      expect(String.fromCharCodes(bytes.sublist(1)), "849201");
    });

    test('DropBookmark serializes with opcode 0x01', () {
      final cmd = DropBookmarkCommand();
      final bytes = cmd.toBytes();
      expect(bytes, equals(Uint8List.fromList([0x01])));
    });

    test('SplitFile serializes with opcode 0x02', () {
      final cmd = SplitFileCommand();
      final bytes = cmd.toBytes();
      expect(bytes, equals(Uint8List.fromList([0x02])));
    });

    test('SetHardwarePad serializes with opcode 0x03 and pad byte', () {
      final cmd = SetHardwarePadCommand(-19);
      final bytes = cmd.toBytes();
      expect(bytes[0], 0x03);
      expect(bytes[1], (-19) & 0xFF);
    });
  });

  group('PairingInfo URL and JSON Parsing', () {
    test('Parses from standard QR code URL format', () {
      final info = PairingInfo.fromQrString("http://djm-box.local:8080/?token=987654");
      expect(info.host, "djm-box.local");
      expect(info.port, 8080);
      expect(info.token, "987654");
    });

    test('Parses from direct token string fallback', () {
      final info = PairingInfo.fromQrString("654321");
      expect(info.token, "654321");
    });

    test('Parses from Wi-Fi Hotspot QR format', () {
      final info = PairingInfo.fromQrString("WIFI:T:WPA;S:DJM-Box-Booth;P:secret123;;");
      expect(info.hotspotSsid, "DJM-Box-Booth");
      expect(info.hotspotPassword, "secret123");
      expect(info.host, "10.42.0.1");
    });

    test('Constructs audio stream preview URL accurately', () {
      final info = PairingInfo(host: "10.42.0.1", port: 8080, token: "123456");
      final testUrl = "http://${info.host}:${info.port}/api/audio/test-stream?token=${Uri.encodeComponent(info.token)}";
      final liveUrl = "http://${info.host}:${info.port}/api/audio/live-stream?token=${Uri.encodeComponent(info.token)}";
      expect(testUrl, "http://10.42.0.1:8080/api/audio/test-stream?token=123456");
      expect(liveUrl, "http://10.42.0.1:8080/api/audio/live-stream?token=123456");
    });
  });
}
