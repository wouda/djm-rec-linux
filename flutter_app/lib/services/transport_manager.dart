import 'dart:async';
import 'dart:convert';
import 'dart:io';
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
  String? _discoveredHost;
  bool _isScanningSubnet = false;
  List<String> _discoveredAppliances = [];

  WebSocketChannel? _wsChannel;
  StreamSubscription? _wsSubscription;

  bool _isDisposed = false;
  Timer? _reconnectTimer;

  ActiveTransport get activeTransport => _activeTransport;
  Telemetry get telemetry => _telemetry;
  PairingInfo? get pairingInfo => _pairingInfo;
  String? get discoveredHost => _discoveredHost;
  bool get isScanningSubnet => _isScanningSubnet;
  List<String> get discoveredAppliances => List.unmodifiable(_discoveredAppliances);
  bool get isConnected =>
      _activeTransport == ActiveTransport.wifi || _activeTransport == ActiveTransport.remote;

  String get audioStreamUrl => testAudioStreamUrl;

  String get testAudioStreamUrl {
    final host = _pairingInfo?.host ?? _discoveredHost ?? "10.42.0.1";
    final port = _pairingInfo?.port ?? 8080;
    final token = _pairingInfo?.token ?? "";
    return "http://$host:$port/api/audio/test-stream?token=${Uri.encodeComponent(token)}";
  }

  String get liveAudioStreamUrl {
    final host = _pairingInfo?.host ?? _discoveredHost ?? "10.42.0.1";
    final port = _pairingInfo?.port ?? 8080;
    final token = _pairingInfo?.token ?? "";
    return "http://$host:$port/api/audio/live-stream?token=${Uri.encodeComponent(token)}";
  }

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
      } catch (e) {
        debugPrint("Failed to load saved pairing info: $e");
      }
    }
    // Always start auto-connect and Wi-Fi auto-discovery
    startAutoConnect();
  }

  Future<void> savePairing(PairingInfo info) async {
    _pairingInfo = info;
    _discoveredHost = info.host;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString('djm_pairing_info', json.encode(info.toJson()));
    notifyListeners();
    disconnect();
    startAutoConnect();
  }

  Future<void> connectWithDiscoveredHost(String token, [String? hostOverride]) async {
    final host = hostOverride ?? _discoveredHost ?? "10.42.0.1";
    final info = PairingInfo(
      host: host,
      port: 8080,
      token: token,
      localIp: (host != "djm-box.local" && !host.endsWith('.local')) ? host : null,
      mdnsHostname: host.endsWith('.local') ? host : null,
    );
    await savePairing(info);
  }

  void selectDiscoveredAppliance(String host) {
    _discoveredHost = host;
    notifyListeners();
  }

  Future<void> clearPairing() async {
    disconnect();
    _pairingInfo = null;
    _discoveredHost = null;
    _discoveredAppliances = [];
    final prefs = await SharedPreferences.getInstance();
    await prefs.remove('djm_pairing_info');
    _telemetry = const Telemetry();
    notifyListeners();
  }

  void startAutoConnect() {
    _reconnectTimer?.cancel();
    _connectPriorityChain();
  }

  Future<List<String>> findLocalSubnetPrefixes() async {
    final prefixes = <String>[];
    try {
      final interfaces = await NetworkInterface.list(
        type: InternetAddressType.IPv4,
        includeLoopback: false,
      );
      for (final iface in interfaces) {
        for (final addr in iface.addresses) {
          final ip = addr.address;
          if (ip.startsWith("127.") || ip.startsWith("169.254.")) continue;
          final lastDot = ip.lastIndexOf('.');
          if (lastDot != -1) {
            final prefix = ip.substring(0, lastDot + 1);
            if (!prefixes.contains(prefix)) {
              prefixes.add(prefix);
            }
          }
        }
      }
    } catch (e) {
      debugPrint("Error resolving network interfaces: $e");
    }
    return prefixes;
  }

  Future<List<String>> scanSubnet({int batchSize = 25}) async {
    if (_isScanningSubnet || _isDisposed) return _discoveredAppliances;
    _isScanningSubnet = true;
    notifyListeners();

    final prefixes = await findLocalSubnetPrefixes();
    if (prefixes.isEmpty) {
      prefixes.add("192.168.1.");
    }

    final found = <String>[];
    final port = _pairingInfo?.port ?? 8080;

    for (final prefix in prefixes) {
      final ips = List.generate(254, (i) => "$prefix${i + 1}");
      for (var i = 0; i < ips.length; i += batchSize) {
        if (_isDisposed) break;
        final batch = ips.sublist(i, (i + batchSize > ips.length) ? ips.length : i + batchSize);
        final results = await Future.wait(batch.map((ip) => _probeHost(ip, port, timeoutMs: 500)));
        for (var j = 0; j < batch.length; j++) {
          if (results[j]) {
            final ip = batch[j];
            if (!found.contains(ip)) found.add(ip);
            if (!_discoveredAppliances.contains(ip)) {
              _discoveredAppliances.add(ip);
              _discoveredHost ??= ip;
              notifyListeners();
            }
          }
        }
      }
    }

    _isScanningSubnet = false;
    notifyListeners();
    return found;
  }

  Future<void> triggerManualSubnetScan() async {
    final found = await scanSubnet();
    if (found.length == 1) {
      final host = found.first;
      _discoveredHost = host;
      final token = _pairingInfo?.token;
      if (token != null && token.isNotEmpty) {
        final port = _pairingInfo?.port ?? 8080;
        final success = await _tryConnectWebSocket("ws://$host:$port/ws?token=$token", ActiveTransport.wifi);
        if (success) return;
      }
    }
    notifyListeners();
  }

  Future<void> _connectPriorityChain() async {
    if (_isDisposed || _activeTransport == ActiveTransport.wifi) return;

    _activeTransport = ActiveTransport.connecting;
    notifyListeners();

    // Priority 1: Candidate Wi-Fi / Hotspot hosts
    final hosts = <String>[];
    // Hotspot Gateway 10.42.0.1 is top priority in booth setups
    hosts.add("10.42.0.1");
    if (_pairingInfo?.host != null && !hosts.contains(_pairingInfo!.host)) {
      hosts.add(_pairingInfo!.host);
    }
    if (_pairingInfo?.mdnsHostname != null && !hosts.contains(_pairingInfo!.mdnsHostname)) {
      hosts.add(_pairingInfo!.mdnsHostname!);
    }
    if (!hosts.contains("djm-box.local")) {
      hosts.add("djm-box.local");
    }
    if (_pairingInfo?.localIp != null && !hosts.contains(_pairingInfo!.localIp)) {
      hosts.add(_pairingInfo!.localIp!);
    }

    final token = _pairingInfo?.token;
    final port = _pairingInfo?.port ?? 8080;

    // If an auth token is saved, try WebSocket connection immediately
    if (token != null && token.isNotEmpty) {
      for (final host in hosts) {
        final success = await _tryConnectWebSocket(
          "ws://$host:$port/ws?token=$token",
          ActiveTransport.wifi,
        );
        if (success) {
          _discoveredHost = host;
          notifyListeners();
          return;
        }
      }

      // Priority 2: Remote / Tailscale WebSocket (if configured)
      if (_pairingInfo?.remoteUrl != null && _pairingInfo!.remoteUrl!.isNotEmpty) {
        final url = _pairingInfo!.remoteUrl!;
        final wsUrl = url.replaceFirst(RegExp(r'^http'), 'ws');
        final target = "$wsUrl/ws?token=$token";
        final success = await _tryConnectWebSocket(target, ActiveTransport.remote);
        if (success) return;
      }
    } else {
      // Auto-probe candidate hosts via HTTP to detect djm-box on Wi-Fi without PIN
      for (final host in hosts) {
        final reachable = await _probeHost(host, port);
        if (reachable) {
          _discoveredHost = host;
          debugPrint("Discovered djm-box appliance at $host:$port");
          notifyListeners();
          break;
        }
      }
    }

    // Priority 3: Subnet scan fallback if direct hosts failed to connect
    if (!_isDisposed && _activeTransport != ActiveTransport.wifi) {
      final subnetDevices = await scanSubnet();
      if (subnetDevices.isNotEmpty) {
        if (subnetDevices.length == 1) {
          final host = subnetDevices.first;
          _discoveredHost = host;
          if (token != null && token.isNotEmpty) {
            final success = await _tryConnectWebSocket(
              "ws://$host:$port/ws?token=$token",
              ActiveTransport.wifi,
            );
            if (success) return;
          }
        }
        // Multiple devices or no token saved: leave _discoveredAppliances set for user selection
        notifyListeners();
      }
    }

    // Retry chain after delay if everything fails
    _activeTransport = ActiveTransport.disconnected;
    notifyListeners();
    _reconnectTimer = Timer(const Duration(seconds: 4), _connectPriorityChain);
  }

  Future<bool> _probeHost(String host, int port, {int timeoutMs = 1200}) async {
    try {
      final client = HttpClient()..connectionTimeout = Duration(milliseconds: timeoutMs);
      final request = await client.getUrl(Uri.parse("http://$host:$port/api/status"));
      final response = await request.close().timeout(Duration(milliseconds: timeoutMs));
      client.close();
      return response.statusCode == 200 || response.statusCode == 401;
    } catch (_) {
      return false;
    }
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
