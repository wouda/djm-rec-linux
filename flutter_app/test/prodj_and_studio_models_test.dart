import 'package:flutter_test/flutter_test.dart';
import 'package:djm_companion/models/prodj.dart';
import 'package:djm_companion/models/studio.dart';
import 'package:djm_companion/models/telemetry.dart';

void main() {
  group('Pro DJ Link Telemetry Models', () {
    test('ProDjDeckStatus displays "unavailable" in Degraded Mode', () {
      final degradedDeck = ProDjDeckStatus(
        playerNumber: 1,
        isOnAir: true,
        isPlaying: true,
        bpm: null,
        trackTitle: null,
        trackArtist: null,
        usbMediaId: null,
        usbMediaName: null,
        degraded: true,
      );

      expect(degradedDeck.displayTitle, 'unavailable');
      expect(degradedDeck.displayArtist, 'unavailable');
      expect(degradedDeck.displayBpm, 'unavailable');
      expect(degradedDeck.displayMedia, 'unavailable');
      expect(degradedDeck.isOnAir, isTrue);
    });

    test('ProDjDeckStatus displays resolved metadata when available', () {
      final activeDeck = ProDjDeckStatus(
        playerNumber: 2,
        isOnAir: true,
        isPlaying: true,
        bpm: 128.0,
        trackTitle: 'Solar Energy',
        trackArtist: 'Bram & DJM',
        usbMediaId: 'USB-A1',
        usbMediaName: 'Corsair 64GB',
        degraded: false,
      );

      expect(activeDeck.displayTitle, 'Solar Energy');
      expect(activeDeck.displayArtist, 'Bram & DJM');
      expect(activeDeck.displayBpm, '128.0 BPM');
      expect(activeDeck.displayMedia, 'Corsair 64GB');
    });

    test('ProDjTelemetry deserialization from json', () {
      final jsonMap = {
        'devices': [
          {
            'player_number': 1,
            'device_name': 'CDJ-3000',
            'ip_address': '192.168.1.10',
            'mac_address': [0, 160, 240, 1, 2, 3],
            'model': 'CDJ-3000',
            'firmware': '3.14',
            'is_mixer': false,
            'last_seen_ms': 1234567,
          }
        ],
        'decks': [
          {
            'player_number': 1,
            'is_on_air': true,
            'is_playing': true,
            'is_master': true,
            'bpm': null,
            'track_title': null,
            'track_artist': null,
            'degraded': true,
          }
        ],
        'active_clusters': [
          {
            'cluster_id': 1,
            'media_ids': ['STICK1'],
            'media_names': ['SanDisk 32GB'],
            'proposed_artist': 'DJ One',
            'track_count': 12,
            'first_on_air_time': '2026-10-30T22:00:00Z',
            'last_on_air_time': '2026-10-30T23:00:00Z',
          }
        ],
        'virtual_player_number': null,
        'is_settling': false,
        'is_degraded': true,
        'network_device_count': 1,
      };

      final prodj = ProDjTelemetry.fromJson(jsonMap);
      expect(prodj.devices.length, 1);
      expect(prodj.devices.first.model, 'CDJ-3000');
      expect(prodj.decks.length, 1);
      expect(prodj.decks.first.displayTitle, 'unavailable');
      expect(prodj.isDegraded, isTrue);
      expect(prodj.activeClusters.length, 1);
      expect(prodj.activeClusters.first.proposedArtist, 'DJ One');
    });

    test('Telemetry deserializes embedded ProDjTelemetry', () {
      final jsonMap = {
        'state': 'Recording',
        'clip_count': 42,
        'prodj': {
          'devices': [],
          'decks': [
            {
              'player_number': 3,
              'is_on_air': true,
              'is_playing': true,
              'bpm': 130.0,
              'track_title': 'Hypnotic',
              'track_artist': 'Artist B',
              'degraded': false,
            }
          ],
          'active_clusters': [],
          'is_settling': false,
          'is_degraded': false,
          'network_device_count': 1,
        }
      };

      final tel = Telemetry.fromJson(jsonMap);
      expect(tel.prodj, isNotNull);
      expect(tel.prodj!.decks.length, 1);
      expect(tel.prodj!.decks.first.displayTitle, 'Hypnotic');
      expect(tel.prodj!.decks.first.displayBpm, '130.0 BPM');
    });
  });

  group('Studio Models', () {
    test('EventSession and SegmentInfo deserialization', () {
      final jsonMap = {
        'id': '2026-10-30',
        'mixer_model': 'DJM-A9',
        'start_time_rfc3339': '2026-10-30T21:00:00+01:00',
        'sample_rate': 96000,
        'total_samples': 96000 * 3600,
        'total_duration_secs': 3600.0,
        'segments': [
          {
            'index': 1,
            'flac_path': '/recordings/2026-10-30_pt01.flac',
            'start_time_rfc3339': '2026-10-30T21:00:00+01:00',
            'sample_rate': 96000,
            'total_samples': 96000 * 3600,
            'duration_secs': 3600.0,
            'total_clips': 0,
            'is_warmup': false,
          }
        ],
        'continuity_audit': {
          'is_continuous': true,
          'checked_seams': 0,
          'failed_seams': 0,
          'seam_reports': [],
        },
      };

      final session = EventSession.fromJson(jsonMap);
      expect(session.id, '2026-10-30');
      expect(session.mixerModel, 'DJM-A9');
      expect(session.formattedDuration, '01:00:00');
      expect(session.segments.length, 1);
      expect(session.continuityAudit.isContinuous, isTrue);
    });

    test('TimetableResponse and ArtistSlot parsing', () {
      final jsonMap = {
        'session_id': '2026-10-30',
        'content': '22:00 Artist 1\n23:00 Artist 2',
        'slots': [
          {
            'slot_index': 0,
            'artist': 'Artist 1',
            'scheduled_start_clock': '22:00',
            'target_date': '2026-10-30',
            'scheduled_start_rfc3339': '2026-10-30T22:00:00+01:00',
          },
          {
            'slot_index': 1,
            'artist': 'Artist 2',
            'scheduled_start_clock': '23:00',
            'target_date': '2026-10-30',
            'scheduled_start_rfc3339': '2026-10-30T23:00:00+01:00',
          }
        ]
      };

      final resp = TimetableResponse.fromJson(jsonMap);
      expect(resp.sessionId, '2026-10-30');
      expect(resp.slots.length, 2);
      expect(resp.slots[0].artist, 'Artist 1');
      expect(resp.slots[1].artist, 'Artist 2');
    });

    test('SetBoundary and SetQualityMetrics parsing', () {
      final boundaryJson = {
        'slot_index': 0,
        'artist': 'Artist 1',
        'boundary_time_rfc3339': '2026-10-30T22:00:00+01:00',
        'sample_offset': 0,
        'duration_samples': 96000 * 3600,
        'duration_secs': 3600.0,
        'detection_method': 'SilenceMidpoint',
        'needs_review': false,
      };

      final boundary = SetBoundary.fromJson(boundaryJson);
      expect(boundary.artist, 'Artist 1');
      expect(boundary.detectionMethod, BoundaryDetectionMethod.silenceMidpoint);
      expect(boundary.detectionMethod.toDisplayString(), 'Silence Midpoint');
      expect(boundary.formattedDuration, '01:00:00');

      final metricsJson = {
        'artist': 'Artist 1',
        'slot_index': 0,
        'duration_secs': 3600.0,
        'sample_peak_dbfs': -0.1,
        'true_peak_dbtp': 0.4,
        'rms_dbfs': -12.5,
        'integrated_lufs': -11.2,
        'loudness_range_lu': 4.5,
        'crest_factor_db': 11.1,
        'flat_factor': 0.0,
        'clip_count': 120,
        'clip_percentage': 0.001,
        'recommended_gain_db': -2.8,
        'quality_label': 'Good (Redlined)',
        'clipping_unrepaired_warning': true,
      };

      final metrics = SetQualityMetrics.fromJson(metricsJson);
      expect(metrics.artist, 'Artist 1');
      expect(metrics.truePeakDbtp, 0.4);
      expect(metrics.integratedLufs, -11.2);
      expect(metrics.clippingUnrepairedWarning, isTrue);
    });

    test('JobState and RenderRequest serialization', () {
      final jobJson = {
        'id': 'job-1234',
        'session_id': '2026-10-30',
        'status': 'Running',
        'phase': 'Pass 2 Normalizing',
        'progress_percent': 65.5,
        'current_step': 4,
        'total_steps': 7,
        'started_at_rfc3339': '2026-10-30T23:00:00Z',
        'updated_at_rfc3339': '2026-10-30T23:05:00Z',
      };

      final job = JobState.fromJson(jobJson);
      expect(job.status, JobStatus.running);
      expect(job.progressPercent, 65.5);
      expect(job.phase, 'Pass 2 Normalizing');

      final req = RenderRequest(
        sessionId: '2026-10-30',
        preset: 'Club',
        declip: true,
        mp3: true,
      );
      final reqJson = req.toJson();
      expect(reqJson['session_id'], '2026-10-30');
      expect(reqJson['preset'], 'Club');
      expect(reqJson['declip'], isTrue);
    });
  });
}
