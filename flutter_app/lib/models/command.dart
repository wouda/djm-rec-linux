import 'dart:convert';
import 'dart:typed_data';

abstract class ControlCommand {
  static const int opcodeAuth = 0x00;
  static const int opcodeBookmark = 0x01;
  static const int opcodeSplit = 0x02;
  static const int opcodeSetPad = 0x03;
  static const int opcodeToggleGuard = 0x04;
  static const int opcodeResetPeaks = 0x05;
  static const int opcodePairingWindow = 0x06;

  Uint8List toBytes();
  dynamic toJson();
}

class AuthCommand extends ControlCommand {
  final String token;
  AuthCommand(this.token);

  @override
  Uint8List toBytes() {
    final tokenBytes = utf8.encode(token);
    final bytes = Uint8List(1 + tokenBytes.length);
    bytes[0] = ControlCommand.opcodeAuth;
    bytes.setRange(1, bytes.length, tokenBytes);
    return bytes;
  }

  @override
  dynamic toJson() => {"Auth": token};
}

class DropBookmarkCommand extends ControlCommand {
  final String? note;
  DropBookmarkCommand([this.note]);

  @override
  Uint8List toBytes() {
    if (note != null) {
      final noteBytes = utf8.encode(note!);
      final bytes = Uint8List(1 + noteBytes.length);
      bytes[0] = ControlCommand.opcodeBookmark;
      bytes.setRange(1, bytes.length, noteBytes);
      return bytes;
    } else {
      return Uint8List.fromList([ControlCommand.opcodeBookmark]);
    }
  }

  @override
  dynamic toJson() => {"DropBookmark": note};
}

class SplitFileCommand extends ControlCommand {
  @override
  Uint8List toBytes() => Uint8List.fromList([ControlCommand.opcodeSplit]);

  @override
  dynamic toJson() => "SplitFile";
}

class SetHardwarePadCommand extends ControlCommand {
  final int padDb;
  SetHardwarePadCommand(this.padDb);

  @override
  Uint8List toBytes() {
    return Uint8List.fromList([ControlCommand.opcodeSetPad, padDb & 0xFF]);
  }

  @override
  dynamic toJson() => {"SetHardwarePad": padDb};
}

class ToggleAutoGuardCommand extends ControlCommand {
  @override
  Uint8List toBytes() => Uint8List.fromList([ControlCommand.opcodeToggleGuard]);

  @override
  dynamic toJson() => "ToggleAutoGuard";
}

class ResetPeaksCommand extends ControlCommand {
  @override
  Uint8List toBytes() => Uint8List.fromList([ControlCommand.opcodeResetPeaks]);

  @override
  dynamic toJson() => "ResetPeaks";
}
