import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';

import '../models/command.dart';
import '../models/telemetry.dart';
import '../services/transport_manager.dart';
import '../widgets/vu_meter.dart';
import 'pairing_screen.dart';

class DashboardScreen extends StatelessWidget {
  const DashboardScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final transport = context.watch<TransportManager>();
    final t = transport.telemetry;

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
                "v0.2.3",
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
            // Mixer Status Card
            _buildMixerCard(t),

            const SizedBox(height: 12),

            // Warning banner if mixer not connected
            if (!t.isMixerConnected) ...[
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
                    peakDb: t.peakLDb,
                    rmsDb: t.rmsLDb,
                  ),
                  const SizedBox(height: 12),
                  VuMeterWidget(
                    label: "RIGHT (Master + Mic)",
                    peakDb: t.peakRDb,
                    rmsDb: t.rmsRDb,
                  ),
                ],
              ),
            ),

            // Red Clip Indicator Banner
            if (t.clipCount > 0) ...[
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
                    t.activeFilename ??
                        (t.isMixerConnected
                            ? "Armed in RAM (Waiting for music to start file)"
                            : "No active file"),
                    style: TextStyle(
                      color: t.activeFilename != null
                          ? const Color(0xFF00FF66)
                          : const Color(0xFFFFD000),
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
                      _buildStatBox("DURATION", t.formattedDuration),
                      const SizedBox(width: 8),
                      _buildStatBox("FILE SIZE", "${t.activeFileSizeMb} MB"),
                    ],
                  ),
                  const SizedBox(height: 8),
                  Row(
                    children: [
                      _buildStatBox("HW PAD", "${t.hwPadDb} dB"),
                      const SizedBox(width: 8),
                      _buildStatBox(
                        "FREE DISK",
                        "${t.freeDiskGb.toStringAsFixed(1)} GB (~${(t.freeDiskGb / 1.1).toStringAsFixed(0)}h)",
                      ),
                    ],
                  ),
                ],
              ),
            ),

            const SizedBox(height: 12),

            // Authenticated Action Buttons with Haptic Feedback
            Row(
              children: [
                Expanded(
                  child: _buildActionButton(
                    context,
                    label: "📍 + Bookmark",
                    color: const Color(0xFF1E2638),
                    onPressed: () {
                      HapticFeedback.mediumImpact();
                      transport.sendCommand(DropBookmarkCommand());
                      _showSnackbar(context, "Dropped bookmark in session log");
                    },
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: _buildActionButton(
                    context,
                    label: "✂️ Split File",
                    color: const Color(0xFF1E2638),
                    onPressed: () {
                      HapticFeedback.mediumImpact();
                      transport.sendCommand(SplitFileCommand());
                      _showSnackbar(context, "Requested zero-gap file split");
                    },
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
                    onPressed: () {
                      HapticFeedback.mediumImpact();
                      transport.sendCommand(SetHardwarePadCommand(-19));
                      _showSnackbar(context, "Hardware pad locked to -19 dB");
                    },
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: _buildActionButton(
                    context,
                    label: "🔄 Reset Peaks",
                    color: const Color(0xFF1E2638),
                    onPressed: () {
                      HapticFeedback.mediumImpact();
                      transport.sendCommand(ResetPeaksCommand());
                      _showSnackbar(context, "Peak hold & clip counter reset");
                    },
                  ),
                ),
              ],
            ),
          ],
        ),
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

  Widget _buildMixerCard(Telemetry t) {
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
    required VoidCallback onPressed,
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
