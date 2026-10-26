import 'dart:async';
import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:web_socket_channel/web_socket_channel.dart';

import '../models/command.dart';
import '../models/pairing_info.dart';
import '../models/telemetry.dart';

enum ActiveTransport {
  disconnected,
  connecting,
  wifi,
  remote,
}

class TransportManager extends ChangeNotifier {
  ActiveTransport _activeTransport = ActiveTransport.disconnected;
  Telemetry _telemetry = const Telemetry();
  PairingInfo? _pairingInfo;

  WebSocketChannel? _wsChannel;
  StreamSubscription? _wsSubscription;

  bool _isDisposed = false;
  Timer? _reconnectTimer;

  ActiveTransport get activeTransport => _activeTransport;
  Telemetry get telemetry => _telemetry;
  PairingInfo? get pairingInfo => _pairingInfo;

  TransportManager() {
    _loadSavedPairing();
  }

  Future<void> _loadSavedPairing() async {
    final prefs = await SharedPreferences.getInstance();
    final jsonStr = prefs.getString('djm_pairing_info');
    if (jsonStr != null) {
      try {
        final map = json.decode(jsonStr) as Map<String, dynamic>;
        _pairingInfo = PairingInfo.fromJson(map);
        notifyListeners();
        startAutoConnect();
      } catch (e) {
        debugPrint("Failed to load saved pairing info: $e");
      }
    }
  }

  Future<void> savePairing(PairingInfo info) async {
    _pairingInfo = info;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString('djm_pairing_info', json.encode(info.toJson()));
    notifyListeners();
    disconnect();
    startAutoConnect();
  }

  Future<void> clearPairing() async {
    disconnect();
    _pairingInfo = null;
    final prefs = await SharedPreferences.getInstance();
    await prefs.remove('djm_pairing_info');
    _telemetry = const Telemetry();
    notifyListeners();
  }

  void startAutoConnect() {
    _reconnectTimer?.cancel();
    if (_pairingInfo == null) return;

    _connectPriorityChain();
  }

  Future<void> _connectPriorityChain() async {
    if (_isDisposed || _activeTransport == ActiveTransport.wifi) return;

    _activeTransport = ActiveTransport.connecting;
    notifyListeners();

    // Priority 1: Local Wi-Fi WebSocket (mDNS, IP, or direct Hotspot Gateway 10.42.0.1)
    final hosts = <String>[];
    if (_pairingInfo?.mdnsHostname != null) hosts.add(_pairingInfo!.mdnsHostname!);
    if (_pairingInfo?.localIp != null) hosts.add(_pairingInfo!.localIp!);
    if (_pairingInfo?.host != null) hosts.add(_pairingInfo!.host);
    // Add default NetworkManager Wi-Fi Hotspot Gateway fallback
    if (!hosts.contains("10.42.0.1")) {
      hosts.add("10.42.0.1");
    }

    for (final host in hosts) {
      final success = await _tryConnectWebSocket(
        "ws://$host:${_pairingInfo?.port ?? 8080}/ws?token=${_pairingInfo?.token}",
        ActiveTransport.wifi,
      );
      if (success) return;
    }

    // Priority 2: Internet / Tailscale WebSocket (if configured)
    if (_pairingInfo?.remoteUrl != null && _pairingInfo!.remoteUrl!.isNotEmpty) {
      final url = _pairingInfo!.remoteUrl!;
      final wsUrl = url.replaceFirst(RegExp(r'^http'), 'ws');
      final target = "$wsUrl/ws?token=${_pairingInfo?.token}";
      final success = await _tryConnectWebSocket(target, ActiveTransport.remote);
      if (success) return;
    }

    // Retry chain after delay if everything fails
    _activeTransport = ActiveTransport.disconnected;
    notifyListeners();
    _reconnectTimer = Timer(const Duration(seconds: 3), _connectPriorityChain);
  }

  Future<bool> _tryConnectWebSocket(String url, ActiveTransport transportType) async {
    try {
      final uri = Uri.parse(url);
      final channel = WebSocketChannel.connect(uri);

      final completer = Completer<bool>();

      _wsSubscription = channel.stream.listen(
        (data) {
          if (!completer.isCompleted) {
            completer.complete(true);
            _activeTransport = transportType;
            _wsChannel = channel;
            notifyListeners();
          }

          try {
            final map = json.decode(data as String) as Map<String, dynamic>;
            _telemetry = Telemetry.fromJson(map);
            notifyListeners();
          } catch (e) {
            debugPrint("Failed to parse WS telemetry JSON: $e");
          }
        },
        onError: (err) {
          if (!completer.isCompleted) completer.complete(false);
          _handleWsDisconnect();
        },
        onDone: () {
          if (!completer.isCompleted) completer.complete(false);
          _handleWsDisconnect();
        },
        cancelOnError: true,
      );

      // Timeout for WS handshake attempt
      return await completer.future.timeout(
        const Duration(seconds: 2),
        onTimeout: () {
          channel.sink.close();
          return false;
        },
      );
    } catch (_) {
      return false;
    }
  }

  void _handleWsDisconnect() {
    _wsSubscription?.cancel();
    _wsSubscription = null;
    _wsChannel = null;

    if (_activeTransport == ActiveTransport.wifi ||
        _activeTransport == ActiveTransport.remote) {
      _activeTransport = ActiveTransport.disconnected;
      notifyListeners();
      _reconnectTimer?.cancel();
      _reconnectTimer = Timer(const Duration(seconds: 1), _connectPriorityChain);
    }
  }

  Future<void> sendCommand(ControlCommand cmd) async {
    try {
      if (_wsChannel != null) {
        final payload = json.encode(cmd.toJson());
        _wsChannel!.sink.add(payload);
      }
    } catch (e) {
      debugPrint("Failed to send command over active transport: $e");
    }
  }

  void disconnect() {
    _reconnectTimer?.cancel();
    _wsSubscription?.cancel();
    _wsSubscription = null;
    _wsChannel?.sink.close();
    _wsChannel = null;

    _activeTransport = ActiveTransport.disconnected;
    notifyListeners();
  }

  @override
  void dispose() {
    _isDisposed = true;
    disconnect();
    super.dispose();
  }
}
