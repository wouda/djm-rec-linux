import 'dart:convert';

class PairingInfo {
  static const String defaultBleServiceUuid = "0000d900-0000-1000-8000-00805f9b34fb";
  static const String bleCharTelemetry = "0000d901-0000-1000-8000-00805f9b34fb";
  static const String bleCharMetadata = "0000d902-0000-1000-8000-00805f9b34fb";
  static const String bleCharControl = "0000d903-0000-1000-8000-00805f9b34fb";

  final String name;
  final String host;
  final int port;
  final String bleServiceUuid;
  final String token;
  final String? localIp;
  final String? mdnsHostname;
  final String? remoteUrl;

  const PairingInfo({
    this.name = "DJM-Box-Recorder",
    this.host = "djm-box.local",
    this.port = 8080,
    this.bleServiceUuid = defaultBleServiceUuid,
    required this.token,
    this.localIp,
    this.mdnsHostname,
    this.remoteUrl,
  });

  /// Parse QR code string: handles both JSON payload and HTTP URL formats
  factory PairingInfo.fromQrString(String payload) {
    final trimmed = payload.trim();

    // 1. Try parsing JSON format
    if (trimmed.startsWith('{') && trimmed.endsWith('}')) {
      try {
        final map = json.decode(trimmed) as Map<String, dynamic>;
        return PairingInfo(
          name: map['name'] as String? ?? "DJM-Box-Recorder",
          host: map['host'] as String? ?? "djm-box.local",
          port: map['port'] as int? ?? 8080,
          bleServiceUuid: map['ble_service_uuid'] as String? ?? defaultBleServiceUuid,
          token: map['token'] as String? ?? "",
          localIp: map['local_ip'] as String?,
          mdnsHostname: map['mdns_hostname'] as String?,
        );
      } catch (_) {}
    }

    // 2. Try parsing URL format: http://djm-box.local:8080/?token=123456
    if (trimmed.startsWith('http://') ||
        trimmed.startsWith('https://') ||
        trimmed.contains('?token=')) {
      try {
        final uri = Uri.parse(trimmed);
        final token = uri.queryParameters['token'] ?? "";
        final host = uri.host.isNotEmpty ? uri.host : "djm-box.local";
        final port = uri.port != 0 ? uri.port : 8080;
        return PairingInfo(
          host: host,
          port: port,
          token: token,
          mdnsHostname: host.endsWith('.local') ? host : null,
        );
      } catch (_) {}
    }

    // Fallback: treat the entire string as the auth token
    return PairingInfo(token: trimmed);
  }

  Map<String, dynamic> toJson() => {
        'name': name,
        'host': host,
        'port': port,
        'ble_service_uuid': bleServiceUuid,
        'token': token,
        'local_ip': localIp,
        'mdns_hostname': mdnsHostname,
        'remote_url': remoteUrl,
      };

  factory PairingInfo.fromJson(Map<String, dynamic> json) => PairingInfo(
        name: json['name'] as String? ?? "DJM-Box-Recorder",
        host: json['host'] as String? ?? "djm-box.local",
        port: json['port'] as int? ?? 8080,
        bleServiceUuid: json['ble_service_uuid'] as String? ?? defaultBleServiceUuid,
        token: json['token'] as String? ?? "",
        localIp: json['local_ip'] as String?,
        mdnsHostname: json['mdns_hostname'] as String?,
        remoteUrl: json['remote_url'] as String?,
      );
}
