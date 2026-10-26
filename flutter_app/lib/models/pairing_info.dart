import 'dart:convert';

class PairingInfo {
  final String name;
  final String host;
  final int port;
  final String token;
  final String? localIp;
  final String? mdnsHostname;
  final String? remoteUrl;
  final String? hotspotSsid;
  final String? hotspotPassword;

  const PairingInfo({
    this.name = "DJM-Box-Recorder",
    this.host = "djm-box.local",
    this.port = 8080,
    required this.token,
    this.localIp,
    this.mdnsHostname,
    this.remoteUrl,
    this.hotspotSsid,
    this.hotspotPassword,
  });

  /// Parse QR code string: handles JSON, direct URL, and Wi-Fi QR formats
  factory PairingInfo.fromQrString(String payload) {
    final trimmed = payload.trim();

    // 1. Wi-Fi QR format: WIFI:T:WPA;S:DJM-Box-Booth;P:secret;;
    if (trimmed.startsWith('WIFI:')) {
      String? ssid;
      String? pass;
      final parts = trimmed.substring(5).split(';');
      for (final p in parts) {
        if (p.startsWith('S:')) ssid = p.substring(2);
        if (p.startsWith('P:')) pass = p.substring(2);
      }
      return PairingInfo(
        host: "10.42.0.1",
        port: 8080,
        token: "",
        hotspotSsid: ssid,
        hotspotPassword: pass,
      );
    }

    // 2. Try parsing JSON format
    if (trimmed.startsWith('{') && trimmed.endsWith('}')) {
      try {
        final map = json.decode(trimmed) as Map<String, dynamic>;
        return PairingInfo(
          name: map['name'] as String? ?? "DJM-Box-Recorder",
          host: map['host'] as String? ?? "djm-box.local",
          port: map['port'] as int? ?? 8080,
          token: map['token'] as String? ?? "",
          localIp: map['local_ip'] as String?,
          mdnsHostname: map['mdns_hostname'] as String?,
          remoteUrl: map['remote_url'] as String?,
          hotspotSsid: map['hotspot_ssid'] as String?,
          hotspotPassword: map['hotspot_password'] as String?,
        );
      } catch (_) {}
    }

    // 3. Try parsing URL format: http://djm-box.local:8080/?token=123456
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
          localIp: (host != "djm-box.local" && !host.endsWith('.local')) ? host : null,
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
        'token': token,
        'local_ip': localIp,
        'mdns_hostname': mdnsHostname,
        'remote_url': remoteUrl,
        'hotspot_ssid': hotspotSsid,
        'hotspot_password': hotspotPassword,
      };

  factory PairingInfo.fromJson(Map<String, dynamic> json) => PairingInfo(
        name: json['name'] as String? ?? "DJM-Box-Recorder",
        host: json['host'] as String? ?? "djm-box.local",
        port: json['port'] as int? ?? 8080,
        token: json['token'] as String? ?? "",
        localIp: json['local_ip'] as String?,
        mdnsHostname: json['mdns_hostname'] as String?,
        remoteUrl: json['remote_url'] as String?,
        hotspotSsid: json['hotspot_ssid'] as String?,
        hotspotPassword: json['hotspot_password'] as String?,
      );
}
