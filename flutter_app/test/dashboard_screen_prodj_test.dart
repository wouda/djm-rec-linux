import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:djm_companion/models/prodj.dart';
import 'package:djm_companion/models/telemetry.dart';
import 'package:djm_companion/screens/dashboard_screen.dart';
import 'package:djm_companion/services/transport_manager.dart';

class MockTransportManager extends ChangeNotifier implements TransportManager {
  @override
  ActiveTransport activeTransport = ActiveTransport.wifi;

  @override
  Telemetry telemetry = const Telemetry();

  @override
  bool isConnected = true;

  @override
  List<String> discoveredAppliances = [];

  @override
  String? discoveredHost = "10.42.0.1";

  @override
  bool isScanningSubnet = false;

  @override
  dynamic pairingInfo;

  @override
  String get audioStreamUrl => "http://10.42.0.1:8080/api/audio/test-stream";

  @override
  String get testAudioStreamUrl => "http://10.42.0.1:8080/api/audio/test-stream";

  @override
  String get liveAudioStreamUrl => "http://10.42.0.1:8080/api/audio/live-stream";

  @override
  String get apiBaseUrl => "http://10.42.0.1:8080";

  @override
  String get authToken => "secret123";

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

void main() {
  testWidgets('Dashboard renders Pro DJ Link cards in Degraded Mode', (WidgetTester tester) async {
    final mockTransport = MockTransportManager();
    mockTransport.telemetry = const Telemetry(
      state: RecordingState.recording,
      mixerModel: 'Pioneer DJM-A9',
      prodj: ProDjTelemetry(
        devices: [
          ProDjDevice(
            playerNumber: 1,
            deviceName: 'CDJ-3000',
            ipAddress: '192.168.1.10',
            macAddress: [0, 1, 2, 3, 4, 5],
            model: 'CDJ-3000',
            firmware: '3.0',
            isMixer: false,
            lastSeenMs: 1000,
          )
        ],
        decks: [
          ProDjDeckStatus(
            playerNumber: 1,
            isOnAir: true,
            isPlaying: true,
            bpm: null,
            trackTitle: null,
            trackArtist: null,
            usbMediaId: null,
            usbMediaName: null,
            degraded: true,
          ),
        ],
        isDegraded: true,
        networkDeviceCount: 1,
      ),
    );

    await tester.pumpWidget(
      ChangeNotifierProvider<TransportManager>.value(
        value: mockTransport,
        child: const MaterialApp(
          home: DashboardScreen(),
        ),
      ),
    );

    await tester.pump();

    // Verify Pro DJ Link header and Degraded badge
    expect(find.textContaining('PRO DJ LINK'), findsOneWidget);
    expect(find.text('DEGRADED (PASSIVE)'), findsOneWidget);

    // Verify Player 1 card
    expect(find.text('PLAYER 1'), findsOneWidget);
    expect(find.text('ON AIR'), findsOneWidget);

    // Verify "unavailable" strings appear for title, artist, bpm, and media in Degraded Mode
    expect(find.text('unavailable'), findsNWidgets(4));

    // Verify Post-Event Studio Launcher is rendered
    expect(find.text('POST-EVENT MASTERING STUDIO'), findsOneWidget);
    expect(find.text('Open Mastering Studio'), findsOneWidget);
  });

  testWidgets('Dashboard renders Pro DJ Link cards with resolved metadata in Active Mode', (WidgetTester tester) async {
    final mockTransport = MockTransportManager();
    mockTransport.telemetry = const Telemetry(
      state: RecordingState.recording,
      mixerModel: 'Pioneer DJM-V10',
      prodj: ProDjTelemetry(
        devices: [],
        decks: [
          ProDjDeckStatus(
            playerNumber: 2,
            isOnAir: true,
            isPlaying: true,
            isMaster: true,
            bpm: 126.0,
            trackTitle: 'Midnight Express',
            trackArtist: 'DJ Pulse',
            usbMediaName: 'Corsair Stick',
            degraded: false,
          ),
        ],
        isDegraded: false,
        networkDeviceCount: 1,
      ),
    );

    await tester.pumpWidget(
      ChangeNotifierProvider<TransportManager>.value(
        value: mockTransport,
        child: const MaterialApp(
          home: DashboardScreen(),
        ),
      ),
    );

    await tester.pump();

    // Verify Active Sync badge
    expect(find.text('ACTIVE SYNC'), findsOneWidget);

    // Verify Player 2 and Master badge
    expect(find.text('PLAYER 2'), findsOneWidget);
    expect(find.text('MASTER'), findsOneWidget);

    // Verify resolved track title, artist, BPM, and USB stick
    expect(find.text('Midnight Express'), findsOneWidget);
    expect(find.text('DJ Pulse'), findsOneWidget);
    expect(find.text('126.0 BPM'), findsOneWidget);
    expect(find.text('Corsair Stick'), findsOneWidget);
  });
}
