import 'dart:typed_data';

enum RecordingState {
  disconnected,
  armedSilent,
  recording,
  paused,
}

class Telemetry {
  final RecordingState state;
  final int hwPadDb;
  final bool autoGuardActive;
  final bool silenceCountdownActive;
  final int peakLCentiDb;
  final int peakRCentiDb;
  final int rmsLCentiDb;
  final int rmsRCentiDb;
  final int clipCount;
  final int activeFileDurationSecs;
  final int activeFileSizeMb;
  final int freeDiskGbTenths;

  final String mixerModel;
  final int sampleRate;
  final String? activeFilename;
  final double? pauseCountdownSecs;
  final List<String> recentLogs;

  const Telemetry({
    this.state = RecordingState.disconnected,
    this.hwPadDb = -19,
    this.autoGuardActive = true,
    this.silenceCountdownActive = false,
    this.peakLCentiDb = -9600,
    this.peakRCentiDb = -9600,
    this.rmsLCentiDb = -9600,
    this.rmsRCentiDb = -9600,
    this.clipCount = 0,
    this.activeFileDurationSecs = 0,
    this.activeFileSizeMb = 0,
    this.freeDiskGbTenths = 0,
    this.mixerModel = "No Mixer",
    this.sampleRate = 0,
    this.activeFilename,
    this.pauseCountdownSecs,
    this.recentLogs = const [],
  });

  bool get isMixerConnected =>
      state != RecordingState.disconnected &&
      mixerModel.isNotEmpty &&
      mixerModel != "No Mixer";

  bool get isRecording => state == RecordingState.recording;

  double get peakLDb => peakLCentiDb / 100.0;
  double get peakRDb => peakRCentiDb / 100.0;
  double get rmsLDb => rmsLCentiDb / 100.0;
  double get rmsRDb => rmsRCentiDb / 100.0;
  double get freeDiskGb => freeDiskGbTenths / 10.0;

  String get formattedDuration {
    final s = activeFileDurationSecs;
    final hh = (s ~/ 3600).toString().padLeft(2, '0');
    final mm = ((s % 3600) ~/ 60).toString().padLeft(2, '0');
    final ss = (s % 60).toString().padLeft(2, '0');
    return "$hh:$mm:$ss";
  }

  static const int packedSize = 20;

  /// Unpack 20-byte packed binary telemetry received over BLE GATT
  static Telemetry fromPackedBytes(Uint8List buf) {
    if (buf.length < packedSize) {
      throw ArgumentError(
          "Invalid payload length: expected at least $packedSize, got ${buf.length}");
    }

    final byteData = ByteData.sublistView(buf);

    final stateByte = byteData.getUint8(0);
    final state = stateByte < RecordingState.values.length
        ? RecordingState.values[stateByte]
        : RecordingState.disconnected;

    final b1 = byteData.getUint8(1);
    final autoGuardActive = (b1 & 0x40) != 0;
    final silenceCountdownActive = (b1 & 0x80) != 0;

    // 6-bit signed pad value
    int rawPad = b1 & 0x3F;
    if ((rawPad & 0x20) != 0) {
      rawPad = rawPad - 64; // sign extend 6-bit negative value
    }
    final hwPadDb = rawPad;

    final peakLCentiDb = byteData.getInt16(2, Endian.little);
    final peakRCentiDb = byteData.getInt16(4, Endian.little);
    final rmsLCentiDb = byteData.getInt16(6, Endian.little);
    final rmsRCentiDb = byteData.getInt16(8, Endian.little);

    final clipCount = byteData.getUint16(10, Endian.little);
    final activeFileDurationSecs = byteData.getUint32(12, Endian.little);
    final activeFileSizeMb = byteData.getUint16(16, Endian.little);
    final freeDiskGbTenths = byteData.getUint16(18, Endian.little);

    return Telemetry(
      state: state,
      hwPadDb: hwPadDb,
      autoGuardActive: autoGuardActive,
      silenceCountdownActive: silenceCountdownActive,
      peakLCentiDb: peakLCentiDb,
      peakRCentiDb: peakRCentiDb,
      rmsLCentiDb: rmsLCentiDb,
      rmsRCentiDb: rmsRCentiDb,
      clipCount: clipCount,
      activeFileDurationSecs: activeFileDurationSecs,
      activeFileSizeMb: activeFileSizeMb,
      freeDiskGbTenths: freeDiskGbTenths,
      mixerModel: state != RecordingState.disconnected ? "Pioneer DJM Mixer" : "No Mixer",
      sampleRate: 96000,
    );
  }

  /// Pack into 20-byte binary frame matching djm_common format
  Uint8List toPackedBytes() {
    final bytes = Uint8List(packedSize);
    final byteData = ByteData.sublistView(bytes);

    byteData.setUint8(0, state.index);

    int padBits = hwPadDb < 0 ? (hwPadDb + 64) & 0x3F : hwPadDb & 0x3F;
    if (autoGuardActive) padBits |= 0x40;
    if (silenceCountdownActive) padBits |= 0x80;
    byteData.setUint8(1, padBits);

    byteData.setInt16(2, peakLCentiDb, Endian.little);
    byteData.setInt16(4, peakRCentiDb, Endian.little);
    byteData.setInt16(6, rmsLCentiDb, Endian.little);
    byteData.setInt16(8, rmsRCentiDb, Endian.little);

    byteData.setUint16(10, clipCount, Endian.little);
    byteData.setUint32(12, activeFileDurationSecs, Endian.little);
    byteData.setUint16(16, activeFileSizeMb, Endian.little);
    byteData.setUint16(18, freeDiskGbTenths, Endian.little);

    return bytes;
  }

  /// Unpack JSON frame received over WebSocket
  static Telemetry fromJson(Map<String, dynamic> json) {
    RecordingState state = RecordingState.disconnected;
    final stateVal = json['state'];
    if (stateVal is String) {
      switch (stateVal) {
        case 'ArmedSilent':
          state = RecordingState.armedSilent;
          break;
        case 'Recording':
          state = RecordingState.recording;
          break;
        case 'Paused':
          state = RecordingState.paused;
          break;
        default:
          state = RecordingState.disconnected;
      }
    } else if (stateVal is int && stateVal < RecordingState.values.length) {
      state = RecordingState.values[stateVal];
    }

    return Telemetry(
      state: state,
      hwPadDb: json['hw_pad_db'] as int? ?? -19,
      autoGuardActive: json['auto_guard_active'] as bool? ?? true,
      silenceCountdownActive: json['silence_countdown_active'] as bool? ?? false,
      peakLCentiDb: json['peak_l_centi_db'] as int? ?? -9600,
      peakRCentiDb: json['peak_r_centi_db'] as int? ?? -9600,
      rmsLCentiDb: json['rms_l_centi_db'] as int? ?? -9600,
      rmsRCentiDb: json['rms_r_centi_db'] as int? ?? -9600,
      clipCount: json['clip_count'] as int? ?? 0,
      activeFileDurationSecs: json['active_file_duration_secs'] as int? ?? 0,
      activeFileSizeMb: json['active_file_size_mb'] as int? ?? 0,
      freeDiskGbTenths: json['free_disk_gb_tenths'] as int? ?? 0,
      mixerModel: json['mixer_model'] as String? ?? "No Mixer",
      sampleRate: json['sample_rate'] as int? ?? 0,
      activeFilename: json['active_filename'] as String?,
      pauseCountdownSecs: (json['pause_countdown_secs'] as num?)?.toDouble(),
      recentLogs: (json['recent_logs'] as List<dynamic>?)?.cast<String>() ?? const [],
    );
  }
}
