import 'dart:async';
import 'package:audioplayers/audioplayers.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';

import '../models/command.dart';
import '../models/telemetry.dart';
import '../services/transport_manager.dart';
import '../widgets/vu_meter.dart';
import 'pairing_screen.dart';

class DashboardScreen extends StatefulWidget {
  const DashboardScreen({super.key});

  @override
  State<DashboardScreen> createState() => _DashboardScreenState();
}

class _DashboardScreenState extends State<DashboardScreen> {
  AudioPlayer? _audioPlayer;
  Timer? _audioTimer;
  int _audioSeconds = 0;
  String? _activeAudioMode; // 'test' | 'live' | null

  @override
  void dispose() {
    _stopAudio();
    super.dispose();
  }

  void _startTestAudio(String streamUrl) async {
    _stopAudio();

    final player = AudioPlayer();
    _audioPlayer = player;
    setState(() {
      _activeAudioMode = 'test';
      _audioSeconds = 15;
    });

    try {
      await player.play(UrlSource(streamUrl, mimeType: 'audio/flac'));
      HapticFeedback.mediumImpact();

      _audioTimer = Timer.periodic(const Duration(seconds: 1), (timer) {
        if (!mounted) return;
        if (_audioSeconds <= 1) {
          _stopAudio();
        } else {
          setState(() {
            _audioSeconds--;
          });
        }
      });

      player.onPlayerComplete.listen((_) {
        _stopAudio();
      });
    } catch (e) {
      if (_audioPlayer != player) return;
      debugPrint("Test audio stream playback failed: $e");
      _stopAudio();
      if (mounted) {
        _showSnackbar(context, "Audio playback failed: $e");
      }
    }
  }

  void _startLiveAudio(String streamUrl) async {
    _stopAudio();

    final player = AudioPlayer();
    _audioPlayer = player;
    setState(() {
      _activeAudioMode = 'live';
      _audioSeconds = 0;
    });

    try {
      await player.play(UrlSource(streamUrl, mimeType: 'audio/mpeg'));
      HapticFeedback.mediumImpact();

      _audioTimer = Timer.periodic(const Duration(seconds: 1), (timer) {
        if (!mounted) return;
        setState(() {
          _audioSeconds++;
        });
      });
    } catch (e) {
      if (_audioPlayer != player) return;
      debugPrint("Live stream playback failed: $e");
      _stopAudio();
      if (mounted) {
        _showSnackbar(context, "Live stream failed: $e");
      }
    }
  }

  void _stopAudio() {
    _audioTimer?.cancel();
    _audioTimer = null;
    final p = _audioPlayer;
    _audioPlayer = null;
    p?.stop();
    p?.dispose();

    if (mounted && _activeAudioMode != null) {
      setState(() {
        _activeAudioMode = null;
        _audioSeconds = 0;
      });
    }
  }

  void _showPinDialog(BuildContext context, TransportManager transport) {
    final controller = TextEditingController();
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: const Color(0xFF131722),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(12),
          side: const BorderSide(color: Color(0xFF232A3B)),
        ),
        title: Text(
          "Connect to ${transport.discoveredHost ?? 'djm-box'}",
          style: const TextStyle(color: Colors.white, fontSize: 16),
        ),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text(
              "Enter the 6-digit PIN displayed on the appliance screen or TUI:",
              style: TextStyle(color: Color(0xFF94A3B8), fontSize: 13),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: controller,
              keyboardType: TextInputType.text,
              autofocus: true,
              style: const TextStyle(
                color: Colors.white,
                fontFamily: "monospace",
                fontWeight: FontWeight.bold,
                fontSize: 18,
                letterSpacing: 2,
              ),
              decoration: const InputDecoration(
                filled: true,
                fillColor: Color(0xFF0A0D14),
                hintText: "125555",
                hintStyle: TextStyle(color: Color(0xFF475569)),
                border: OutlineInputBorder(),
              ),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text("Cancel", style: TextStyle(color: Color(0xFF94A3B8))),
          ),
          ElevatedButton(
            style: ElevatedButton.styleFrom(backgroundColor: const Color(0xFF00F0FF)),
            onPressed: () {
              final pin = controller.text.trim();
              if (pin.isNotEmpty) {
                transport.connectWithDiscoveredHost(pin);
                Navigator.of(ctx).pop();
                _showSnackbar(context, "Connecting to ${transport.discoveredHost}...");
              }
            },
            child: const Text("Connect", style: TextStyle(color: Colors.black, fontWeight: FontWeight.bold)),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final transport = context.watch<TransportManager>();
    final t = transport.telemetry;
    final isConnected = transport.isConnected;

    return Scaffold(
      backgroundColor: const Color(0xFF090A0F),
      appBar: AppBar(
        backgroundColor: const Color(0xFF131722),
        elevation: 0,
        title: Row(
          children: [
            const Text(
              "djm-box",
              style: TextStyle(
                fontWeight: FontWeight.w900,
                fontSize: 18,
                letterSpacing: -0.5,
                color: Color(0xFFE2E8F0),
              ),
            ),
            const SizedBox(width: 8),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
              decoration: BoxDecoration(
                color: const Color(0xFF1E2638),
                borderRadius: BorderRadius.circular(4),
                border: Border.all(color: const Color(0xFF232A3B)),
              ),
              child: const Text(
                "v0.2.6",
                style: TextStyle(
                  color: Color(0xFF94A3B8),
                  fontSize: 10,
                  fontWeight: FontWeight.bold,
                ),
              ),
            ),
          ],
        ),
        actions: [
          _buildTransportBadge(transport.activeTransport),
          IconButton(
            icon: const Icon(Icons.qr_code_scanner, color: Color(0xFF00F0FF)),
            onPressed: () {
              Navigator.of(context).push(
                MaterialPageRoute(builder: (_) => const PairingScreen()),
              );
            },
          ),
        ],
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            // Prominent Top Disconnected / Searching Wi-Fi Banner
            if (!isConnected) ...[
              _buildDisconnectedHeroCard(context, transport),
              const SizedBox(height: 12),
            ],

            // Mixer Status Card
            _buildMixerCard(t, isConnected),

            const SizedBox(height: 12),

            // Warning banner if mixer not connected (ONLY shown when server is online)
            if (isConnected && !t.isMixerConnected) ...[
              Container(
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(
                  color: const Color(0xFF450A0A),
                  borderRadius: BorderRadius.circular(8),
                  border: Border.all(color: const Color(0xFFEF4444)),
                ),
                child: const Row(
                  children: [
                    Icon(Icons.warning_amber_rounded, color: Color(0xFFFECACA), size: 20),
                    SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        "No Pioneer DJM Mixer Detected. Connect mixer via USB-B cable to begin recording.",
                        style: TextStyle(
                          color: Color(0xFFFECACA),
                          fontSize: 12,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 12),
            ],

            // Dual Stereo VU Meters
            Container(
              padding: const EdgeInsets.all(16),
              decoration: BoxDecoration(
                color: const Color(0xFF131722),
                borderRadius: BorderRadius.circular(12),
                border: Border.all(color: const Color(0xFF232A3B)),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  VuMeterWidget(
                    label: "LEFT (Master + Mic)",
                    peakDb: isConnected ? t.peakLDb : -96.0,
                    rmsDb: isConnected ? t.rmsLDb : -96.0,
                  ),
                  const SizedBox(height: 12),
                  VuMeterWidget(
                    label: "RIGHT (Master + Mic)",
                    peakDb: isConnected ? t.peakRDb : -96.0,
                    rmsDb: isConnected ? t.rmsRDb : -96.0,
                  ),
                ],
              ),
            ),

            // Red Clip Indicator Banner
            if (isConnected && t.clipCount > 0) ...[
              const SizedBox(height: 10),
              Container(
                padding: const EdgeInsets.symmetric(vertical: 8),
                decoration: BoxDecoration(
                  color: const Color(0xFFFF3344),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Text(
                  "⚠️ DIGITAL CLIPS: ${t.clipCount}",
                  textAlign: TextAlign.center,
                  style: const TextStyle(
                    color: Colors.white,
                    fontWeight: FontWeight.bold,
                    fontSize: 13,
                  ),
                ),
              ),
            ],

            const SizedBox(height: 12),

            // Dedicated Dual Audio Monitor Card: 15s Test (FLAC) & Live Stream (MP3)
            _buildTestAudioCard(context, transport, t, isConnected),

            const SizedBox(height: 12),

            // Active File Stats Card
            Container(
              padding: const EdgeInsets.all(16),
              decoration: BoxDecoration(
                color: const Color(0xFF131722),
                borderRadius: BorderRadius.circular(12),
                border: Border.all(color: const Color(0xFF232A3B)),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(
                    !isConnected
                        ? "Server offline"
                        : (t.activeFilename ??
                            (t.isMixerConnected
                                ? "Armed in RAM (Waiting for music to start file)"
                                : "No active file")),
                    style: TextStyle(
                      color: !isConnected
                          ? const Color(0xFF94A3B8)
                          : (t.activeFilename != null
                              ? const Color(0xFF00FF66)
                              : const Color(0xFFFFD000)),
                      fontSize: 12,
                      fontFamily: "monospace",
                      fontWeight: FontWeight.w600,
                    ),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  const SizedBox(height: 12),
                  Row(
                    children: [
                      _buildStatBox("DURATION", isConnected ? t.formattedDuration : "--:--:--"),
                      const SizedBox(width: 8),
                      _buildStatBox("FILE SIZE", isConnected ? "${t.activeFileSizeMb} MB" : "-- MB"),
                    ],
                  ),
                  const SizedBox(height: 8),
                  Row(
                    children: [
                      _buildStatBox("HW PAD", isConnected ? "${t.hwPadDb} dB" : "-- dB"),
                      const SizedBox(width: 8),
                      _buildStatBox(
                        "FREE DISK",
                        isConnected
                            ? "${t.freeDiskGb.toStringAsFixed(1)} GB (~${(t.freeDiskGb / 1.1).toStringAsFixed(0)}h)"
                            : "-- GB",
                      ),
                    ],
                  ),
                ],
              ),
            ),

            const SizedBox(height: 12),

            // Action Buttons (Dimmed and disabled when disconnected)
            Opacity(
              opacity: isConnected ? 1.0 : 0.4,
              child: Column(
                children: [
                  Row(
                    children: [
                      Expanded(
                        child: _buildActionButton(
                          context,
                          label: "📍 + Bookmark",
                          color: const Color(0xFF1E2638),
                          onPressed: isConnected
                              ? () {
                                  HapticFeedback.mediumImpact();
                                  transport.sendCommand(DropBookmarkCommand());
                                  _showSnackbar(context, "Dropped bookmark in session log");
                                }
                              : null,
                        ),
                      ),
                      const SizedBox(width: 10),
                      Expanded(
                        child: _buildActionButton(
                          context,
                          label: "✂️ Split File",
                          color: const Color(0xFF1E2638),
                          onPressed: isConnected
                              ? () {
                                  HapticFeedback.mediumImpact();
                                  transport.sendCommand(SplitFileCommand());
                                  _showSnackbar(context, "Requested zero-gap file split");
                                }
                              : null,
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 10),
                  Row(
                    children: [
                      Expanded(
                        child: _buildActionButton(
                          context,
                          label: "🛡️ Lock -19dB",
                          color: const Color(0xFF1E2638),
                          onPressed: isConnected
                              ? () {
                                  HapticFeedback.mediumImpact();
                                  transport.sendCommand(SetHardwarePadCommand(-19));
                                  _showSnackbar(context, "Hardware pad locked to -19 dB");
                                }
                              : null,
                        ),
                      ),
                      const SizedBox(width: 10),
                      Expanded(
                        child: _buildActionButton(
                          context,
                          label: "🔄 Reset Peaks",
                          color: const Color(0xFF1E2638),
                          onPressed: isConnected
                              ? () {
                                  HapticFeedback.mediumImpact();
                                  transport.sendCommand(ResetPeaksCommand());
                                  _showSnackbar(context, "Peak hold & clip counter reset");
                                }
                              : null,
                        ),
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildDisconnectedHeroCard(BuildContext context, TransportManager transport) {
    final discovered = transport.discoveredHost;
    final isConnecting = transport.activeTransport == ActiveTransport.connecting;
    final isScanning = transport.isScanningSubnet;
    final appliances = transport.discoveredAppliances;

    String subtitleText;
    if (isScanning) {
      subtitleText = "Scanning local Wi-Fi subnet (/24) for djm-box on port 8080...";
    } else if (discovered != null) {
      subtitleText = "Discovered djm-box at $discovered! Tap below to connect.";
    } else {
      subtitleText = "Searching booth hotspot (10.42.0.1) & local LAN (djm-box.local)...";
    }

    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: const Color(0xFF451A03),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: const Color(0xFFF59E0B)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(
                isConnecting ? Icons.wifi_find : Icons.wifi_off_rounded,
                color: const Color(0xFFFCD34D),
                size: 20,
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  isConnecting
                      ? "Connecting to djm-box on Wi-Fi..."
                      : "Appliance Disconnected (Offline)",
                  style: const TextStyle(
                    color: Color(0xFFFEF3C7),
                    fontWeight: FontWeight.bold,
                    fontSize: 13,
                  ),
                ),
              ),
              if (isScanning)
                const SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(
                    strokeWidth: 2,
                    valueColor: AlwaysStoppedAnimation<Color>(Color(0xFFFCD34D)),
                  ),
                ),
            ],
          ),
          const SizedBox(height: 6),
          Text(
            subtitleText,
            style: const TextStyle(
              color: Color(0xFFFDE68A),
              fontSize: 12,
            ),
          ),
          if (appliances.length > 1) ...[
            const SizedBox(height: 8),
            Wrap(
              spacing: 6,
              runSpacing: 4,
              children: appliances.map((ip) {
                final isSelected = ip == discovered;
                return ChoiceChip(
                  label: Text(ip, style: const TextStyle(fontSize: 11, fontWeight: FontWeight.bold)),
                  selected: isSelected,
                  selectedColor: const Color(0xFFF59E0B),
                  backgroundColor: const Color(0xFF1E2638),
                  labelStyle: TextStyle(color: isSelected ? Colors.black : const Color(0xFFFEF3C7)),
                  onSelected: (_) => transport.selectDiscoveredAppliance(ip),
                );
              }).toList(),
            ),
          ],
          const SizedBox(height: 10),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              if (discovered != null)
                ElevatedButton.icon(
                  style: ElevatedButton.styleFrom(
                    backgroundColor: const Color(0xFFF59E0B),
                    foregroundColor: Colors.black,
                    padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                    elevation: 0,
                  ),
                  icon: const Icon(Icons.pin, size: 16),
                  label: Text("Connect ($discovered)", style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 12)),
                  onPressed: () => _showPinDialog(context, transport),
                ),
              OutlinedButton.icon(
                style: OutlinedButton.styleFrom(
                  foregroundColor: const Color(0xFFFEF3C7),
                  side: const BorderSide(color: Color(0xFFF59E0B)),
                  padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                ),
                icon: isScanning
                    ? const SizedBox(
                        width: 12,
                        height: 12,
                        child: CircularProgressIndicator(strokeWidth: 2, valueColor: AlwaysStoppedAnimation<Color>(Color(0xFFFEF3C7))),
                      )
                    : const Icon(Icons.network_check, size: 16),
                label: Text(isScanning ? "Scanning..." : "Scan Subnet", style: const TextStyle(fontSize: 12, fontWeight: FontWeight.bold)),
                onPressed: isScanning
                    ? null
                    : () {
                        _showSnackbar(context, "Scanning local Wi-Fi subnet for djm-box...");
                        transport.triggerManualSubnetScan();
                      },
              ),
              OutlinedButton.icon(
                style: OutlinedButton.styleFrom(
                  foregroundColor: const Color(0xFFFEF3C7),
                  side: const BorderSide(color: Color(0xFFF59E0B)),
                  padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                ),
                icon: const Icon(Icons.qr_code_scanner, size: 16),
                label: const Text("Scan QR", style: TextStyle(fontSize: 12, fontWeight: FontWeight.bold)),
                onPressed: () {
                  Navigator.of(context).push(
                    MaterialPageRoute(builder: (_) => const PairingScreen()),
                  );
                },
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildTestAudioCard(
    BuildContext context,
    TransportManager transport,
    Telemetry t,
    bool isConnected,
  ) {
    final isRecording = isConnected && t.state == RecordingState.recording;
    final canPlay = isRecording || _activeAudioMode != null;
    final isTestActive = _activeAudioMode == 'test';
    final isLiveActive = _activeAudioMode == 'live';

    String statusText;
    if (isTestActive) {
      statusText = "Testing 24-bit FLAC... ${_audioSeconds}s";
    } else if (isLiveActive) {
      final mm = (_audioSeconds ~/ 60).toString().padLeft(2, '0');
      final ss = (_audioSeconds % 60).toString().padLeft(2, '0');
      statusText = "🔴 LIVE ($mm:$ss)";
    } else if (!isConnected) {
      statusText = "Server Offline";
    } else if (t.state == RecordingState.armedSilent) {
      statusText = "Armed in RAM (> -65dB)";
    } else if (isRecording) {
      statusText = "Ready";
    } else {
      statusText = "Mixer Idle";
    }

    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: const Color(0xFF131722),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: const Color(0xFF232A3B)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              const Row(
                children: [
                  Icon(Icons.headphones, color: Color(0xFF00F0FF), size: 18),
                  SizedBox(width: 6),
                  Text(
                    "Audio Monitor",
                    style: TextStyle(
                      color: Color(0xFFE2E8F0),
                      fontSize: 13,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                ],
              ),
              Text(
                statusText,
                style: TextStyle(
                  color: (isTestActive || isLiveActive)
                      ? const Color(0xFF00F0FF)
                      : (isRecording ? const Color(0xFF00FF66) : const Color(0xFF94A3B8)),
                  fontSize: 11,
                  fontWeight: FontWeight.bold,
                ),
              ),
            ],
          ),
          if (isTestActive) ...[
            const SizedBox(height: 10),
            ClipRRect(
              borderRadius: BorderRadius.circular(3),
              child: LinearProgressIndicator(
                value: (15 - _audioSeconds) / 15.0,
                backgroundColor: Colors.black,
                valueColor: const AlwaysStoppedAnimation<Color>(Color(0xFF00F0FF)),
                minHeight: 6,
              ),
            ),
          ],
          const SizedBox(height: 10),
          Row(
            children: [
              Expanded(
                child: ElevatedButton.icon(
                  style: ElevatedButton.styleFrom(
                    backgroundColor: isTestActive
                        ? const Color(0xFFDC2626)
                        : (canPlay ? const Color(0xFF16A34A) : const Color(0xFF1E2638)),
                    foregroundColor: Colors.white,
                    padding: const EdgeInsets.symmetric(vertical: 12),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(8),
                      side: BorderSide(
                        color: canPlay ? Colors.transparent : const Color(0xFF232A3B),
                      ),
                    ),
                    elevation: 0,
                  ),
                  icon: Icon(
                    isTestActive ? Icons.stop : Icons.play_arrow,
                    size: 18,
                  ),
                  label: Text(
                    isTestActive ? "Stop (15s)" : "Test 15s (FLAC)",
                    style: TextStyle(
                      fontWeight: FontWeight.bold,
                      fontSize: 12,
                      color: canPlay ? Colors.white : const Color(0xFF64748B),
                    ),
                  ),
                  onPressed: canPlay
                      ? () {
                          if (isTestActive) {
                            _stopAudio();
                          } else {
                            _startTestAudio(transport.testAudioStreamUrl);
                          }
                        }
                      : null,
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: ElevatedButton.icon(
                  style: ElevatedButton.styleFrom(
                    backgroundColor: isLiveActive
                        ? const Color(0xFFDC2626)
                        : (canPlay ? const Color(0xFF0284C7) : const Color(0xFF1E2638)),
                    foregroundColor: Colors.white,
                    padding: const EdgeInsets.symmetric(vertical: 12),
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(8),
                      side: BorderSide(
                        color: canPlay ? Colors.transparent : const Color(0xFF232A3B),
                      ),
                    ),
                    elevation: 0,
                  ),
                  icon: Icon(
                    isLiveActive ? Icons.stop : Icons.radio,
                    size: 18,
                  ),
                  label: Text(
                    isLiveActive ? "Stop Live" : "Live Stream (MP3)",
                    style: TextStyle(
                      fontWeight: FontWeight.bold,
                      fontSize: 12,
                      color: canPlay ? Colors.white : const Color(0xFF64748B),
                    ),
                  ),
                  onPressed: canPlay
                      ? () {
                          if (isLiveActive) {
                            _stopAudio();
                          } else {
                            _startLiveAudio(transport.liveAudioStreamUrl);
                          }
                        }
                      : null,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildTransportBadge(ActiveTransport transport) {
    Color bg;
    Color fg;
    String label;

    switch (transport) {
      case ActiveTransport.wifi:
        bg = const Color(0xFF16A34A);
        fg = Colors.white;
        label = "Wi-Fi";
        break;
      case ActiveTransport.remote:
        bg = const Color(0xFF0284C7);
        fg = Colors.white;
        label = "Remote";
        break;
      case ActiveTransport.connecting:
        bg = const Color(0xFFCA8A04);
        fg = Colors.black;
        label = "Connecting...";
        break;
      case ActiveTransport.disconnected:
      default:
        bg = const Color(0xFF334155);
        fg = const Color(0xFF94A3B8);
        label = "Offline";
    }

    return Container(
      margin: const EdgeInsets.symmetric(vertical: 12, horizontal: 4),
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
      decoration: BoxDecoration(
        color: bg,
        borderRadius: BorderRadius.circular(12),
      ),
      alignment: Alignment.center,
      child: Text(
        label,
        style: TextStyle(
          color: fg,
          fontSize: 11,
          fontWeight: FontWeight.bold,
        ),
      ),
    );
  }

  Widget _buildMixerCard(Telemetry t, bool isConnected) {
    if (!isConnected) {
      return Container(
        padding: const EdgeInsets.all(16),
        decoration: BoxDecoration(
          color: const Color(0xFF131722),
          borderRadius: BorderRadius.circular(12),
          border: Border.all(color: const Color(0xFF232A3B)),
        ),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            const Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  "Appliance Offline",
                  style: TextStyle(
                    color: Color(0xFFE2E8F0),
                    fontSize: 14,
                    fontWeight: FontWeight.bold,
                  ),
                ),
                SizedBox(height: 4),
                Text(
                  "Waiting for Wi-Fi connection to djm-box...",
                  style: TextStyle(
                    color: Color(0xFF94A3B8),
                    fontSize: 12,
                  ),
                ),
              ],
            ),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
              decoration: const BoxDecoration(
                color: Color(0xFF334155),
                borderRadius: BorderRadius.all(Radius.circular(16)),
              ),
              child: const Text(
                "○ OFFLINE",
                style: TextStyle(
                  color: Color(0xFF94A3B8),
                  fontSize: 11,
                  fontWeight: FontWeight.w900,
                ),
              ),
            ),
          ],
        ),
      );
    }

    Color recBg;
    Color recFg;
    String recText;

    switch (t.state) {
      case RecordingState.recording:
        recBg = const Color(0xFF00FF66);
        recFg = Colors.black;
        recText = "● RECORDING";
        break;
      case RecordingState.armedSilent:
        recBg = const Color(0xFFFFD000);
        recFg = Colors.black;
        recText = "⏸ ARMED (> -65dB)";
        break;
      case RecordingState.paused:
        recBg = const Color(0xFFFFD000);
        recFg = Colors.black;
        recText = "⏸ PAUSED";
        break;
      case RecordingState.disconnected:
      default:
        recBg = const Color(0xFF334155);
        recFg = const Color(0xFF94A3B8);
        recText = "○ IDLE";
    }

    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: const Color(0xFF131722),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: const Color(0xFF232A3B)),
      ),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  t.isMixerConnected
                      ? "${t.mixerModel} @ ${t.sampleRate > 0 ? t.sampleRate : 96000} Hz"
                      : "Waiting for USB-B mixer...",
                  style: const TextStyle(
                    color: Color(0xFFE2E8F0),
                    fontSize: 14,
                    fontWeight: FontWeight.bold,
                  ),
                ),
                const SizedBox(height: 4),
                Text(
                  t.isMixerConnected
                      ? "✓ Hardware Pad -19dB Active"
                      : "Mixer disconnected",
                  style: TextStyle(
                    color: t.isMixerConnected
                        ? const Color(0xFF00FF66)
                        : const Color(0xFF94A3B8),
                    fontSize: 12,
                  ),
                ),
              ],
            ),
          ),
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
            decoration: BoxDecoration(
              color: recBg,
              borderRadius: BorderRadius.circular(16),
            ),
            child: Text(
              recText,
              style: TextStyle(
                color: recFg,
                fontSize: 11,
                fontWeight: FontWeight.w900,
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildStatBox(String label, String value) {
    return Expanded(
      child: Container(
        padding: const EdgeInsets.all(10),
        decoration: BoxDecoration(
          color: const Color(0xFF0A0D14),
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: const Color(0xFF1A2233)),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              label,
              style: const TextStyle(
                color: Color(0xFF94A3B8),
                fontSize: 10,
                fontWeight: FontWeight.bold,
              ),
            ),
            const SizedBox(height: 4),
            Text(
              value,
              style: const TextStyle(
                color: Color(0xFF00F0FF),
                fontSize: 13,
                fontWeight: FontWeight.bold,
                fontFamily: "monospace",
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildActionButton(
    BuildContext context, {
    required String label,
    required Color color,
    required VoidCallback? onPressed,
  }) {
    return ElevatedButton(
      style: ElevatedButton.styleFrom(
        backgroundColor: color,
        foregroundColor: Colors.white,
        padding: const EdgeInsets.symmetric(vertical: 14),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(10),
          side: const BorderSide(color: Color(0xFF232A3B)),
        ),
        elevation: 0,
      ),
      onPressed: onPressed,
      child: Text(
        label,
        style: const TextStyle(
          fontSize: 13,
          fontWeight: FontWeight.bold,
        ),
      ),
    );
  }

  void _showSnackbar(BuildContext context, String message) {
    ScaffoldMessenger.of(context).hideCurrentSnackBar();
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(message),
        duration: const Duration(seconds: 2),
        backgroundColor: const Color(0xFF1E2638),
      ),
    );
  }
}
