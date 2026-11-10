class SegmentInfo {
  final int index;
  final String flacPath;
  final String? logPath;
  final String startTimeRfc3339;
  final String? endTimeRfc3339;
  final int sampleRate;
  final int totalSamples;
  final double durationSecs;
  final int totalClips;
  final bool isWarmup;
  final String? splitReason;
  final int prerollSamples;
  final String? ambientPath;
  final int? ambientSamples;
  final bool ambientAligned;
  final int ambientOffsetSamples;

  const SegmentInfo({
    required this.index,
    required this.flacPath,
    this.logPath,
    required this.startTimeRfc3339,
    this.endTimeRfc3339,
    required this.sampleRate,
    required this.totalSamples,
    required this.durationSecs,
    required this.totalClips,
    required this.isWarmup,
    this.splitReason,
    this.prerollSamples = 0,
    this.ambientPath,
    this.ambientSamples,
    this.ambientAligned = false,
    this.ambientOffsetSamples = 0,
  });

  factory SegmentInfo.fromJson(Map<String, dynamic> json) {
    return SegmentInfo(
      index: (json['index'] as num?)?.toInt() ?? 0,
      flacPath: json['flac_path'] as String? ?? '',
      logPath: json['log_path'] as String?,
      startTimeRfc3339: json['start_time_rfc3339'] as String? ?? '',
      endTimeRfc3339: json['end_time_rfc3339'] as String?,
      sampleRate: (json['sample_rate'] as num?)?.toInt() ?? 96000,
      totalSamples: (json['total_samples'] as num?)?.toInt() ?? 0,
      durationSecs: (json['duration_secs'] as num?)?.toDouble() ?? 0.0,
      totalClips: (json['total_clips'] as num?)?.toInt() ?? 0,
      isWarmup: json['is_warmup'] as bool? ?? false,
      splitReason: json['split_reason'] as String?,
      prerollSamples: (json['preroll_samples'] as num?)?.toInt() ?? 0,
      ambientPath: json['ambient_path'] as String?,
      ambientSamples: (json['ambient_samples'] as num?)?.toInt(),
      ambientAligned: json['ambient_aligned'] as bool? ?? false,
      ambientOffsetSamples: (json['ambient_offset_samples'] as num?)?.toInt() ?? 0,
    );
  }

  Map<String, dynamic> toJson() => {
    'index': index,
    'flac_path': flacPath,
    'log_path': logPath,
    'start_time_rfc3339': startTimeRfc3339,
    'end_time_rfc3339': endTimeRfc3339,
    'sample_rate': sampleRate,
    'total_samples': totalSamples,
    'duration_secs': durationSecs,
    'total_clips': totalClips,
    'is_warmup': isWarmup,
    'split_reason': splitReason,
    'preroll_samples': prerollSamples,
    'ambient_path': ambientPath,
    'ambient_samples': ambientSamples,
    'ambient_aligned': ambientAligned,
    'ambient_offset_samples': ambientOffsetSamples,
  };
}

class SeamContinuityReport {
  final int fromSegmentIndex;
  final int toSegmentIndex;
  final String splitReason;
  final double durationFromSamplesSecs;
  final double intervalBetweenStartsSecs;
  final double discrepancyMs;
  final bool isWithinTolerance;

  const SeamContinuityReport({
    required this.fromSegmentIndex,
    required this.toSegmentIndex,
    required this.splitReason,
    required this.durationFromSamplesSecs,
    required this.intervalBetweenStartsSecs,
    required this.discrepancyMs,
    required this.isWithinTolerance,
  });

  factory SeamContinuityReport.fromJson(Map<String, dynamic> json) {
    return SeamContinuityReport(
      fromSegmentIndex: (json['from_segment_index'] as num?)?.toInt() ?? 0,
      toSegmentIndex: (json['to_segment_index'] as num?)?.toInt() ?? 0,
      splitReason: json['split_reason'] as String? ?? '',
      durationFromSamplesSecs: (json['duration_from_samples_secs'] as num?)?.toDouble() ?? 0.0,
      intervalBetweenStartsSecs: (json['interval_between_starts_secs'] as num?)?.toDouble() ?? 0.0,
      discrepancyMs: (json['discrepancy_ms'] as num?)?.toDouble() ?? 0.0,
      isWithinTolerance: json['is_within_tolerance'] as bool? ?? false,
    );
  }

  Map<String, dynamic> toJson() => {
    'from_segment_index': fromSegmentIndex,
    'to_segment_index': toSegmentIndex,
    'split_reason': splitReason,
    'duration_from_samples_secs': durationFromSamplesSecs,
    'interval_between_starts_secs': intervalBetweenStartsSecs,
    'discrepancy_ms': discrepancyMs,
    'is_within_tolerance': isWithinTolerance,
  };
}

class ContinuityAuditReport {
  final bool isContinuous;
  final int checkedSeams;
  final int failedSeams;
  final List<SeamContinuityReport> seamReports;

  const ContinuityAuditReport({
    this.isContinuous = true,
    this.checkedSeams = 0,
    this.failedSeams = 0,
    this.seamReports = const [],
  });

  factory ContinuityAuditReport.fromJson(Map<String, dynamic> json) {
    return ContinuityAuditReport(
      isContinuous: json['is_continuous'] as bool? ?? true,
      checkedSeams: (json['checked_seams'] as num?)?.toInt() ?? 0,
      failedSeams: (json['failed_seams'] as num?)?.toInt() ?? 0,
      seamReports: (json['seam_reports'] as List<dynamic>?)
              ?.map((s) => SeamContinuityReport.fromJson(s as Map<String, dynamic>))
              .toList() ??
          const [],
    );
  }

  Map<String, dynamic> toJson() => {
    'is_continuous': isContinuous,
    'checked_seams': checkedSeams,
    'failed_seams': failedSeams,
    'seam_reports': seamReports.map((s) => s.toJson()).toList(),
  };
}

class EventSession {
  final String id;
  final String mixerModel;
  final String startTimeRfc3339;
  final String? endTimeRfc3339;
  final List<SegmentInfo> segments;
  final List<SegmentInfo> warmupSegments;
  final List<SegmentInfo> ambientSegments;
  final bool isActiveRecording;
  final int sampleRate;
  final int totalSamples;
  final double totalDurationSecs;
  final ContinuityAuditReport continuityAudit;

  const EventSession({
    required this.id,
    required this.mixerModel,
    required this.startTimeRfc3339,
    this.endTimeRfc3339,
    this.segments = const [],
    this.warmupSegments = const [],
    this.ambientSegments = const [],
    this.isActiveRecording = false,
    this.sampleRate = 96000,
    this.totalSamples = 0,
    this.totalDurationSecs = 0.0,
    this.continuityAudit = const ContinuityAuditReport(),
  });

  String get formattedDuration {
    final s = totalDurationSecs.round();
    final hh = (s ~/ 3600).toString().padLeft(2, '0');
    final mm = ((s % 3600) ~/ 60).toString().padLeft(2, '0');
    final ss = (s % 60).toString().padLeft(2, '0');
    return '$hh:$mm:$ss';
  }

  factory EventSession.fromJson(Map<String, dynamic> json) {
    return EventSession(
      id: json['id'] as String? ?? '',
      mixerModel: json['mixer_model'] as String? ?? 'Pioneer DJM',
      startTimeRfc3339: json['start_time_rfc3339'] as String? ?? '',
      endTimeRfc3339: json['end_time_rfc3339'] as String?,
      segments: (json['segments'] as List<dynamic>?)
              ?.map((s) => SegmentInfo.fromJson(s as Map<String, dynamic>))
              .toList() ??
          const [],
      warmupSegments: (json['warmup_segments'] as List<dynamic>?)
              ?.map((s) => SegmentInfo.fromJson(s as Map<String, dynamic>))
              .toList() ??
          const [],
      ambientSegments: (json['ambient_segments'] as List<dynamic>?)
              ?.map((s) => SegmentInfo.fromJson(s as Map<String, dynamic>))
              .toList() ??
          const [],
      isActiveRecording: json['is_active_recording'] as bool? ?? false,
      sampleRate: (json['sample_rate'] as num?)?.toInt() ?? 96000,
      totalSamples: (json['total_samples'] as num?)?.toInt() ?? 0,
      totalDurationSecs: (json['total_duration_secs'] as num?)?.toDouble() ?? 0.0,
      continuityAudit: json['continuity_audit'] != null
          ? ContinuityAuditReport.fromJson(json['continuity_audit'] as Map<String, dynamic>)
          : const ContinuityAuditReport(),
    );
  }

  Map<String, dynamic> toJson() => {
    'id': id,
    'mixer_model': mixerModel,
    'start_time_rfc3339': startTimeRfc3339,
    'end_time_rfc3339': endTimeRfc3339,
    'segments': segments.map((s) => s.toJson()).toList(),
    'warmup_segments': warmupSegments.map((s) => s.toJson()).toList(),
    'ambient_segments': ambientSegments.map((s) => s.toJson()).toList(),
    'is_active_recording': isActiveRecording,
    'sample_rate': sampleRate,
    'total_samples': totalSamples,
    'total_duration_secs': totalDurationSecs,
    'continuity_audit': continuityAudit.toJson(),
  };
}

class ArtistSlot {
  final int slotIndex;
  final String artist;
  final String scheduledStartClock;
  final String? scheduledEndClock;
  final String targetDate;
  final String scheduledStartRfc3339;
  final String? scheduledEndRfc3339;
  final int? assignedUsbCluster;
  final String? assignedUsbMedia;

  const ArtistSlot({
    required this.slotIndex,
    required this.artist,
    required this.scheduledStartClock,
    this.scheduledEndClock,
    required this.targetDate,
    required this.scheduledStartRfc3339,
    this.scheduledEndRfc3339,
    this.assignedUsbCluster,
    this.assignedUsbMedia,
  });

  factory ArtistSlot.fromJson(Map<String, dynamic> json) {
    return ArtistSlot(
      slotIndex: (json['slot_index'] as num?)?.toInt() ?? 0,
      artist: json['artist'] as String? ?? '',
      scheduledStartClock: json['scheduled_start_clock'] as String? ?? '',
      scheduledEndClock: json['scheduled_end_clock'] as String?,
      targetDate: json['target_date'] as String? ?? '',
      scheduledStartRfc3339: json['scheduled_start_rfc3339'] as String? ?? '',
      scheduledEndRfc3339: json['scheduled_end_rfc3339'] as String?,
      assignedUsbCluster: (json['assigned_usb_cluster'] as num?)?.toInt(),
      assignedUsbMedia: json['assigned_usb_media'] as String?,
    );
  }

  Map<String, dynamic> toJson() => {
    'slot_index': slotIndex,
    'artist': artist,
    'scheduled_start_clock': scheduledStartClock,
    'scheduled_end_clock': scheduledEndClock,
    'target_date': targetDate,
    'scheduled_start_rfc3339': scheduledStartRfc3339,
    'scheduled_end_rfc3339': scheduledEndRfc3339,
    'assigned_usb_cluster': assignedUsbCluster,
    'assigned_usb_media': assignedUsbMedia,
  };
}

class TimetableResponse {
  final String sessionId;
  final String content;
  final List<ArtistSlot> slots;

  const TimetableResponse({
    required this.sessionId,
    required this.content,
    this.slots = const [],
  });

  factory TimetableResponse.fromJson(Map<String, dynamic> json) {
    return TimetableResponse(
      sessionId: json['session_id'] as String? ?? '',
      content: json['content'] as String? ?? '',
      slots: (json['slots'] as List<dynamic>?)
              ?.map((s) => ArtistSlot.fromJson(s as Map<String, dynamic>))
              .toList() ??
          const [],
    );
  }

  Map<String, dynamic> toJson() => {
    'session_id': sessionId,
    'content': content,
    'slots': slots.map((s) => s.toJson()).toList(),
  };
}

enum BoundaryDetectionMethod {
  silenceMidpoint,
  usbHandover,
  scheduledClock,
  manualAdjust;

  static BoundaryDetectionMethod fromString(String str) {
    switch (str) {
      case 'SilenceMidpoint':
        return BoundaryDetectionMethod.silenceMidpoint;
      case 'UsbHandover':
        return BoundaryDetectionMethod.usbHandover;
      case 'ScheduledClock':
        return BoundaryDetectionMethod.scheduledClock;
      case 'ManualAdjust':
      default:
        return BoundaryDetectionMethod.manualAdjust;
    }
  }

  String toDisplayString() {
    switch (this) {
      case BoundaryDetectionMethod.silenceMidpoint:
        return 'Silence Midpoint';
      case BoundaryDetectionMethod.usbHandover:
        return 'ProDJ USB Handover';
      case BoundaryDetectionMethod.scheduledClock:
        return 'Scheduled Clock';
      case BoundaryDetectionMethod.manualAdjust:
        return 'Manual Adjust';
    }
  }
}

class SetBoundary {
  final int slotIndex;
  final String artist;
  final String boundaryTimeRfc3339;
  final int sampleOffset;
  final int durationSamples;
  final double durationSecs;
  final BoundaryDetectionMethod detectionMethod;
  final double? silenceDurationSecs;
  final String? silenceMidpointRfc3339;
  final String? usbHandoverMedia;
  final bool needsReview;
  final String? reviewReason;

  const SetBoundary({
    required this.slotIndex,
    required this.artist,
    required this.boundaryTimeRfc3339,
    required this.sampleOffset,
    required this.durationSamples,
    required this.durationSecs,
    required this.detectionMethod,
    this.silenceDurationSecs,
    this.silenceMidpointRfc3339,
    this.usbHandoverMedia,
    this.needsReview = false,
    this.reviewReason,
  });

  String get formattedDuration {
    final s = durationSecs.round();
    final hh = (s ~/ 3600).toString().padLeft(2, '0');
    final mm = ((s % 3600) ~/ 60).toString().padLeft(2, '0');
    final ss = (s % 60).toString().padLeft(2, '0');
    return '$hh:$mm:$ss';
  }

  factory SetBoundary.fromJson(Map<String, dynamic> json) {
    return SetBoundary(
      slotIndex: (json['slot_index'] as num?)?.toInt() ?? 0,
      artist: json['artist'] as String? ?? '',
      boundaryTimeRfc3339: json['boundary_time_rfc3339'] as String? ?? '',
      sampleOffset: (json['sample_offset'] as num?)?.toInt() ?? 0,
      durationSamples: (json['duration_samples'] as num?)?.toInt() ?? 0,
      durationSecs: (json['duration_secs'] as num?)?.toDouble() ?? 0.0,
      detectionMethod: BoundaryDetectionMethod.fromString(json['detection_method']?.toString() ?? ''),
      silenceDurationSecs: (json['silence_duration_secs'] as num?)?.toDouble(),
      silenceMidpointRfc3339: json['silence_midpoint_rfc3339'] as String?,
      usbHandoverMedia: json['usb_handover_media'] as String?,
      needsReview: json['needs_review'] as bool? ?? false,
      reviewReason: json['review_reason'] as String?,
    );
  }
}

class SetQualityMetrics {
  final String artist;
  final int slotIndex;
  final double durationSecs;
  final double samplePeakDbfs;
  final double truePeakDbtp;
  final double rmsDbfs;
  final double integratedLufs;
  final double loudnessRangeLu;
  final double crestFactorDb;
  final double flatFactor;
  final int clipCount;
  final double clipPercentage;
  final double recommendedGainDb;
  final String qualityLabel;
  final bool clippingUnrepairedWarning;

  const SetQualityMetrics({
    required this.artist,
    required this.slotIndex,
    required this.durationSecs,
    required this.samplePeakDbfs,
    required this.truePeakDbtp,
    required this.rmsDbfs,
    required this.integratedLufs,
    required this.loudnessRangeLu,
    required this.crestFactorDb,
    required this.flatFactor,
    required this.clipCount,
    required this.clipPercentage,
    required this.recommendedGainDb,
    required this.qualityLabel,
    this.clippingUnrepairedWarning = false,
  });

  factory SetQualityMetrics.fromJson(Map<String, dynamic> json) {
    return SetQualityMetrics(
      artist: json['artist'] as String? ?? '',
      slotIndex: (json['slot_index'] as num?)?.toInt() ?? 0,
      durationSecs: (json['duration_secs'] as num?)?.toDouble() ?? 0.0,
      samplePeakDbfs: (json['sample_peak_dbfs'] as num?)?.toDouble() ?? -96.0,
      truePeakDbtp: (json['true_peak_dbtp'] as num?)?.toDouble() ?? -96.0,
      rmsDbfs: (json['rms_dbfs'] as num?)?.toDouble() ?? -96.0,
      integratedLufs: (json['integrated_lufs'] as num?)?.toDouble() ?? -96.0,
      loudnessRangeLu: (json['loudness_range_lu'] as num?)?.toDouble() ?? 0.0,
      crestFactorDb: (json['crest_factor_db'] as num?)?.toDouble() ?? 0.0,
      flatFactor: (json['flat_factor'] as num?)?.toDouble() ?? 0.0,
      clipCount: (json['clip_count'] as num?)?.toInt() ?? 0,
      clipPercentage: (json['clip_percentage'] as num?)?.toDouble() ?? 0.0,
      recommendedGainDb: (json['recommended_gain_db'] as num?)?.toDouble() ?? 0.0,
      qualityLabel: json['quality_label'] as String? ?? 'Good',
      clippingUnrepairedWarning: json['clipping_unrepaired_warning'] as bool? ?? false,
    );
  }
}

enum JobStatus {
  idle,
  running,
  completed,
  failed,
  aborted,
  interrupted;

  static JobStatus fromString(String str) {
    switch (str.toLowerCase()) {
      case 'running':
        return JobStatus.running;
      case 'completed':
        return JobStatus.completed;
      case 'failed':
        return JobStatus.failed;
      case 'aborted':
        return JobStatus.aborted;
      case 'interrupted':
        return JobStatus.interrupted;
      case 'idle':
      default:
        return JobStatus.idle;
    }
  }
}

class JobState {
  final String id;
  final String sessionId;
  final JobStatus status;
  final String phase;
  final double progressPercent;
  final int currentStep;
  final int totalSteps;
  final String? errorMessage;
  final List<String> outputFiles;
  final String startedAtRfc3339;
  final String updatedAtRfc3339;

  const JobState({
    required this.id,
    required this.sessionId,
    required this.status,
    required this.phase,
    required this.progressPercent,
    required this.currentStep,
    required this.totalSteps,
    this.errorMessage,
    this.outputFiles = const [],
    required this.startedAtRfc3339,
    required this.updatedAtRfc3339,
  });

  factory JobState.fromJson(Map<String, dynamic> json) {
    return JobState(
      id: json['id'] as String? ?? '',
      sessionId: json['session_id'] as String? ?? '',
      status: JobStatus.fromString(json['status']?.toString() ?? 'Idle'),
      phase: json['phase'] as String? ?? '',
      progressPercent: (json['progress_percent'] as num?)?.toDouble() ?? 0.0,
      currentStep: (json['current_step'] as num?)?.toInt() ?? 0,
      totalSteps: (json['total_steps'] as num?)?.toInt() ?? 0,
      errorMessage: json['error_message'] as String?,
      outputFiles: (json['output_files'] as List<dynamic>?)?.cast<String>() ?? const [],
      startedAtRfc3339: json['started_at_rfc3339'] as String? ?? '',
      updatedAtRfc3339: json['updated_at_rfc3339'] as String? ?? '',
    );
  }
}

class RenderRequest {
  final String sessionId;
  final String preset;
  final double? targetLufs;
  final double? ceilingDbtp;
  final bool declip;
  final bool mp3;
  final bool fullEvent;
  final bool force;

  const RenderRequest({
    required this.sessionId,
    this.preset = 'Streaming',
    this.targetLufs,
    this.ceilingDbtp,
    this.declip = false,
    this.mp3 = true,
    this.fullEvent = true,
    this.force = false,
  });

  Map<String, dynamic> toJson() => {
    'session_id': sessionId,
    'preset': preset,
    if (targetLufs != null) 'target_lufs': targetLufs,
    if (ceilingDbtp != null) 'ceiling_dbtp': ceilingDbtp,
    'declip': declip,
    'mp3': mp3,
    'full_event': fullEvent,
    'force': force,
  };
}
