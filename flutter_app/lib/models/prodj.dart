class ProDjDevice {
  final int playerNumber;
  final String deviceName;
  final String ipAddress;
  final List<int> macAddress;
  final String model;
  final String firmware;
  final bool isMixer;
  final int lastSeenMs;

  const ProDjDevice({
    required this.playerNumber,
    required this.deviceName,
    required this.ipAddress,
    required this.macAddress,
    required this.model,
    required this.firmware,
    required this.isMixer,
    required this.lastSeenMs,
  });

  factory ProDjDevice.fromJson(Map<String, dynamic> json) {
    return ProDjDevice(
      playerNumber: json['player_number'] as int? ?? 0,
      deviceName: json['device_name'] as String? ?? '',
      ipAddress: json['ip_address']?.toString() ?? '',
      macAddress: (json['mac_address'] as List<dynamic>?)?.cast<int>() ?? const [],
      model: json['model'] as String? ?? '',
      firmware: json['firmware'] as String? ?? '',
      isMixer: json['is_mixer'] as bool? ?? false,
      lastSeenMs: (json['last_seen_ms'] as num?)?.toInt() ?? 0,
    );
  }

  Map<String, dynamic> toJson() => {
    'player_number': playerNumber,
    'device_name': deviceName,
    'ip_address': ipAddress,
    'mac_address': macAddress,
    'model': model,
    'firmware': firmware,
    'is_mixer': isMixer,
    'last_seen_ms': lastSeenMs,
  };
}

class ProDjDeckStatus {
  final int playerNumber;
  final bool isOnAir;
  final bool isPlaying;
  final bool isMaster;
  final double? bpm;
  final double? pitchPercent;
  final int? rekordboxId;
  final String? trackTitle;
  final String? trackArtist;
  final String? trackKey;
  final String? usbMediaId;
  final String? usbMediaName;
  final String? usbSlot;
  final double? playheadSecs;
  final double? durationSecs;
  final double cumulativeOnAirSecs;
  final bool degraded;

  const ProDjDeckStatus({
    required this.playerNumber,
    this.isOnAir = false,
    this.isPlaying = false,
    this.isMaster = false,
    this.bpm,
    this.pitchPercent,
    this.rekordboxId,
    this.trackTitle,
    this.trackArtist,
    this.trackKey,
    this.usbMediaId,
    this.usbMediaName,
    this.usbSlot,
    this.playheadSecs,
    this.durationSecs,
    this.cumulativeOnAirSecs = 0.0,
    this.degraded = true,
  });

  String get displayTitle => trackTitle != null && trackTitle!.isNotEmpty ? trackTitle! : 'unavailable';
  String get displayArtist => trackArtist != null && trackArtist!.isNotEmpty ? trackArtist! : 'unavailable';
  String get displayBpm => bpm != null ? '${bpm!.toStringAsFixed(1)} BPM' : 'unavailable';
  String get displayMedia => usbMediaName ?? usbMediaId ?? 'unavailable';

  factory ProDjDeckStatus.fromJson(Map<String, dynamic> json) {
    return ProDjDeckStatus(
      playerNumber: json['player_number'] as int? ?? 1,
      isOnAir: json['is_on_air'] as bool? ?? false,
      isPlaying: json['is_playing'] as bool? ?? false,
      isMaster: json['is_master'] as bool? ?? false,
      bpm: (json['bpm'] as num?)?.toDouble(),
      pitchPercent: (json['pitch_percent'] as num?)?.toDouble(),
      rekordboxId: (json['rekordbox_id'] as num?)?.toInt(),
      trackTitle: json['track_title'] as String?,
      trackArtist: json['track_artist'] as String?,
      trackKey: json['track_key'] as String?,
      usbMediaId: json['usb_media_id'] as String?,
      usbMediaName: json['usb_media_name'] as String?,
      usbSlot: json['usb_slot'] as String?,
      playheadSecs: (json['playhead_secs'] as num?)?.toDouble(),
      durationSecs: (json['duration_secs'] as num?)?.toDouble(),
      cumulativeOnAirSecs: (json['cumulative_on_air_secs'] as num?)?.toDouble() ?? 0.0,
      degraded: json['degraded'] as bool? ?? true,
    );
  }

  Map<String, dynamic> toJson() => {
    'player_number': playerNumber,
    'is_on_air': isOnAir,
    'is_playing': isPlaying,
    'is_master': isMaster,
    'bpm': bpm,
    'pitch_percent': pitchPercent,
    'rekordbox_id': rekordboxId,
    'track_title': trackTitle,
    'track_artist': trackArtist,
    'track_key': trackKey,
    'usb_media_id': usbMediaId,
    'usb_media_name': usbMediaName,
    'usb_slot': usbSlot,
    'playhead_secs': playheadSecs,
    'duration_secs': durationSecs,
    'cumulative_on_air_secs': cumulativeOnAirSecs,
    'degraded': degraded,
  };
}

class ProDjCluster {
  final int clusterId;
  final List<String> mediaIds;
  final List<String> mediaNames;
  final String? proposedArtist;
  final int trackCount;
  final String firstOnAirTime;
  final String lastOnAirTime;

  const ProDjCluster({
    required this.clusterId,
    required this.mediaIds,
    required this.mediaNames,
    this.proposedArtist,
    required this.trackCount,
    required this.firstOnAirTime,
    required this.lastOnAirTime,
  });

  factory ProDjCluster.fromJson(Map<String, dynamic> json) {
    return ProDjCluster(
      clusterId: (json['cluster_id'] as num?)?.toInt() ?? 0,
      mediaIds: (json['media_ids'] as List<dynamic>?)?.cast<String>() ?? const [],
      mediaNames: (json['media_names'] as List<dynamic>?)?.cast<String>() ?? const [],
      proposedArtist: json['proposed_artist'] as String?,
      trackCount: (json['track_count'] as num?)?.toInt() ?? 0,
      firstOnAirTime: json['first_on_air_time'] as String? ?? '',
      lastOnAirTime: json['last_on_air_time'] as String? ?? '',
    );
  }

  Map<String, dynamic> toJson() => {
    'cluster_id': clusterId,
    'media_ids': mediaIds,
    'media_names': mediaNames,
    'proposed_artist': proposedArtist,
    'track_count': trackCount,
    'first_on_air_time': firstOnAirTime,
    'last_on_air_time': lastOnAirTime,
  };
}

class ProDjUsbMedia {
  final String mediaId;
  final String mediaName;
  final int playerNumber;
  final String slot;
  final int tracksPlayed;
  final String firstSeenRfc3339;
  final String lastSeenRfc3339;

  const ProDjUsbMedia({
    required this.mediaId,
    required this.mediaName,
    required this.playerNumber,
    required this.slot,
    required this.tracksPlayed,
    required this.firstSeenRfc3339,
    required this.lastSeenRfc3339,
  });

  factory ProDjUsbMedia.fromJson(Map<String, dynamic> json) {
    return ProDjUsbMedia(
      mediaId: json['media_id'] as String? ?? '',
      mediaName: json['media_name'] as String? ?? '',
      playerNumber: (json['player_number'] as num?)?.toInt() ?? 0,
      slot: json['slot'] as String? ?? '',
      tracksPlayed: (json['tracks_played'] as num?)?.toInt() ?? 0,
      firstSeenRfc3339: json['first_seen_rfc3339'] as String? ?? '',
      lastSeenRfc3339: json['last_seen_rfc3339'] as String? ?? '',
    );
  }

  Map<String, dynamic> toJson() => {
    'media_id': mediaId,
    'media_name': mediaName,
    'player_number': playerNumber,
    'slot': slot,
    'tracks_played': tracksPlayed,
    'first_seen_rfc3339': firstSeenRfc3339,
    'last_seen_rfc3339': lastSeenRfc3339,
  };
}

class ProDjTelemetry {
  final List<ProDjDevice> devices;
  final List<ProDjDeckStatus> decks;
  final List<ProDjCluster> activeClusters;
  final int? virtualPlayerNumber;
  final bool isSettling;
  final bool isDegraded;
  final List<ProDjUsbMedia> activeMedia;
  final int networkDeviceCount;

  const ProDjTelemetry({
    this.devices = const [],
    this.decks = const [],
    this.activeClusters = const [],
    this.virtualPlayerNumber,
    this.isSettling = false,
    this.isDegraded = true,
    this.activeMedia = const [],
    this.networkDeviceCount = 0,
  });

  factory ProDjTelemetry.fromJson(Map<String, dynamic> json) {
    return ProDjTelemetry(
      devices: (json['devices'] as List<dynamic>?)
              ?.map((d) => ProDjDevice.fromJson(d as Map<String, dynamic>))
              .toList() ??
          const [],
      decks: (json['decks'] as List<dynamic>?)
              ?.map((d) => ProDjDeckStatus.fromJson(d as Map<String, dynamic>))
              .toList() ??
          const [],
      activeClusters: (json['active_clusters'] as List<dynamic>?)
              ?.map((c) => ProDjCluster.fromJson(c as Map<String, dynamic>))
              .toList() ??
          const [],
      virtualPlayerNumber: (json['virtual_player_number'] as num?)?.toInt(),
      isSettling: json['is_settling'] as bool? ?? false,
      isDegraded: json['is_degraded'] as bool? ?? true,
      activeMedia: (json['active_media'] as List<dynamic>?)
              ?.map((m) => ProDjUsbMedia.fromJson(m as Map<String, dynamic>))
              .toList() ??
          const [],
      networkDeviceCount: (json['network_device_count'] as num?)?.toInt() ?? 0,
    );
  }

  Map<String, dynamic> toJson() => {
    'devices': devices.map((d) => d.toJson()).toList(),
    'decks': decks.map((d) => d.toJson()).toList(),
    'active_clusters': activeClusters.map((c) => c.toJson()).toList(),
    'virtual_player_number': virtualPlayerNumber,
    'is_settling': isSettling,
    'is_degraded': isDegraded,
    'active_media': activeMedia.map((m) => m.toJson()).toList(),
    'network_device_count': networkDeviceCount,
  };
}
