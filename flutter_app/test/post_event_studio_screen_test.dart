import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:djm_companion/models/prodj.dart';
import 'package:djm_companion/models/studio.dart';
import 'package:djm_companion/models/telemetry.dart';
import 'package:djm_companion/screens/post_event_studio_screen.dart';
import 'package:djm_companion/services/transport_manager.dart';

class StudioMockTransportManager extends ChangeNotifier implements TransportManager {
  @override
  ActiveTransport activeTransport = ActiveTransport.wifi;

  @override
  Telemetry telemetry = const Telemetry(
    prodj: ProDjTelemetry(
      activeClusters: [
        ProDjCluster(
          clusterId: 1,
          mediaIds: ['USB-A'],
          mediaNames: ['SanDisk 64GB'],
          proposedArtist: 'Headliner',
          trackCount: 15,
          firstOnAirTime: '2026-10-30T22:00:00Z',
          lastOnAirTime: '2026-10-30T23:30:00Z',
        ),
      ],
    ),
  );

  @override
  bool isConnected = true;

  @override
  Future<List<EventSession>> fetchStudioSessions() async {
    return [
      const EventSession(
        id: '2026-10-30',
        mixerModel: 'DJM-A9',
        startTimeRfc3339: '2026-10-30T21:00:00+01:00',
        totalDurationSecs: 7200.0,
        sampleRate: 96000,
        continuityAudit: ContinuityAuditReport(isContinuous: true),
      ),
    ];
  }

  @override
  Future<TimetableResponse> fetchStudioTimetable(String sessionId) async {
    return const TimetableResponse(
      sessionId: '2026-10-30',
      content: '22:00 Resident DJ\n23:30 Headliner',
      slots: [
        ArtistSlot(
          slotIndex: 0,
          artist: 'Resident DJ',
          scheduledStartClock: '22:00',
          targetDate: '2026-10-30',
          scheduledStartRfc3339: '2026-10-30T22:00:00+01:00',
          assignedUsbMedia: 'SanDisk 32GB',
        ),
        ArtistSlot(
          slotIndex: 1,
          artist: 'Headliner',
          scheduledStartClock: '23:30',
          targetDate: '2026-10-30',
          scheduledStartRfc3339: '2026-10-30T23:30:00+01:00',
          assignedUsbMedia: 'SanDisk 64GB',
        ),
      ],
    );
  }

  @override
  Future<TimetableResponse> saveStudioTimetable(String sessionId, String content) async {
    return TimetableResponse(
      sessionId: sessionId,
      content: content,
      slots: const [],
    );
  }

  @override
  String getTransitionPreviewUrl(String sessionId, int boundarySample, {int durationSecs = 15}) {
    return "http://10.42.0.1:8080/api/studio/preview-transition?session=$sessionId&boundary_sample=$boundarySample";
  }

  @override
  Future<String> fetchStudioReport(String sessionId) async {
    return "=== Event Report for $sessionId ===\nMixer: DJM-A9\nDuration: 2h\nStatus: Verified";
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

void main() {
  testWidgets('PostEventStudioScreen loads session, timetable and renders review table', (WidgetTester tester) async {
    final mock = StudioMockTransportManager();

    await tester.pumpWidget(
      ChangeNotifierProvider<TransportManager>.value(
        value: mock,
        child: const MaterialApp(
          home: PostEventStudioScreen(),
        ),
      ),
    );

    // Initial pump and wait for futures to settle
    await tester.pumpAndSettle();

    // Verify Title & Session Picker
    expect(find.text('Post-Event Mastering Studio'), findsOneWidget);
    expect(find.text('Select Event Session'), findsOneWidget);
    expect(find.text('Continuous Seams'), findsOneWidget);

    // Verify Timetable Editor & Content
    expect(find.text('Timetable / Running Order'), findsOneWidget);
    expect(find.text('Save Lineup'), findsOneWidget);
    expect(find.textContaining('Resident DJ'), findsWidgets);

    // Verify Detected USB Clusters
    expect(find.textContaining('Detected Pro DJ Link USB Sticks'), findsOneWidget);
    expect(find.textContaining('SanDisk 64GB'), findsOneWidget);

    // Verify Split Review Table & Artist Slots
    expect(find.text('Artist Set Splits & Boundary Review'), findsOneWidget);
    expect(find.text('Analyze & Resolve'), findsOneWidget);
    expect(find.text('Headliner'), findsOneWidget);

    // Verify Mastering Preset & Export Settings
    expect(find.text('Mastering Preset & Export Settings'), findsOneWidget);
    expect(find.textContaining('De-clip repair'), findsOneWidget);
    expect(find.textContaining('Render 320kbps MP3s'), findsOneWidget);

    // Verify Action Buttons
    expect(find.text('⚡ Start Mastering & Render All Sets'), findsOneWidget);
    expect(find.text('View Event Quality Report'), findsOneWidget);
  });
}
