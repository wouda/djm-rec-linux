import 'package:flutter/material.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import 'package:provider/provider.dart';

import '../models/pairing_info.dart';
import '../services/transport_manager.dart';

class PairingScreen extends StatefulWidget {
  const PairingScreen({super.key});

  @override
  State<PairingScreen> createState() => _PairingScreenState();
}

class _PairingScreenState extends State<PairingScreen> {
  final _tokenController = TextEditingController();
  final _hostController = TextEditingController(text: "djm-box.local");
  final _remoteUrlController = TextEditingController();
  bool _showScanner = true;

  @override
  void initState() {
    super.initState();
    final current = context.read<TransportManager>().pairingInfo;
    if (current != null) {
      _tokenController.text = current.token;
      _hostController.text = current.host;
      if (current.remoteUrl != null) {
        _remoteUrlController.text = current.remoteUrl!;
      }
    }
  }

  @override
  void dispose() {
    _tokenController.dispose();
    _hostController.dispose();
    _remoteUrlController.dispose();
    super.dispose();
  }

  void _handleQrScanned(String raw) {
    try {
      final info = PairingInfo.fromQrString(raw);
      context.read<TransportManager>().savePairing(info);
      Navigator.of(context).pop();
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text("✓ Paired with djm-box recording appliance!")),
      );
    } catch (e) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text("Failed to parse QR code: $e")),
      );
    }
  }

  void _handleManualSave() {
    final token = _tokenController.text.trim();
    if (token.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text("Please enter the 6-digit PIN / auth token")),
      );
      return;
    }

    final info = PairingInfo(
      token: token,
      host: _hostController.text.trim().isNotEmpty
          ? _hostController.text.trim()
          : "djm-box.local",
      remoteUrl: _remoteUrlController.text.trim().isNotEmpty
          ? _remoteUrlController.text.trim()
          : null,
    );

    context.read<TransportManager>().savePairing(info);
    Navigator.of(context).pop();
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(content: Text("✓ Credentials saved. Connecting...")),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: const Color(0xFF090A0F),
      appBar: AppBar(
        backgroundColor: const Color(0xFF131722),
        title: const Text("Phone Pairing"),
        actions: [
          IconButton(
            icon: Icon(_showScanner ? Icons.keyboard : Icons.camera_alt),
            onPressed: () => setState(() => _showScanner = !_showScanner),
            tooltip: _showScanner ? "Enter PIN Manually" : "Scan QR Code",
          ),
        ],
      ),
      body: SingleChildScrollView(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (_showScanner) ...[
              Container(
                height: 280,
                color: Colors.black,
                child: MobileScanner(
                  onDetect: (capture) {
                    final List<Barcode> barcodes = capture.barcodes;
                    for (final barcode in barcodes) {
                      if (barcode.rawValue != null) {
                        _handleQrScanned(barcode.rawValue!);
                        break;
                      }
                    }
                  },
                ),
              ),
              Container(
                padding: const EdgeInsets.all(12),
                color: const Color(0xFF131722),
                child: const Text(
                  "Point your camera at the QR code shown in djm-tui [P] modal.",
                  textAlign: TextAlign.center,
                  style: TextStyle(color: Color(0xFF94A3B8), fontSize: 13),
                ),
              ),
              const SizedBox(height: 16),
            ],
            Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  const Text(
                    "Manual Appliance Pairing",
                    style: TextStyle(
                      color: Color(0xFFE2E8F0),
                      fontSize: 16,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                  const SizedBox(height: 12),
                  TextField(
                    controller: _tokenController,
                    keyboardType: TextInputType.text,
                    style: const TextStyle(
                      color: Colors.white,
                      fontFamily: "monospace",
                      fontWeight: FontWeight.bold,
                    ),
                    decoration: InputDecoration(
                      labelText: "6-Digit PIN or Auth Token",
                      labelStyle: const TextStyle(color: Color(0xFF94A3B8)),
                      hintText: "Shown via 'djm-box pair' or [P] in TUI",
                      hintStyle: const TextStyle(color: Color(0xFF64748B), fontSize: 12),
                      filled: true,
                      fillColor: const Color(0xFF131722),
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(8),
                        borderSide: const BorderSide(color: Color(0xFF232A3B)),
                      ),
                    ),
                  ),
                  const SizedBox(height: 12),
                  TextField(
                    controller: _hostController,
                    style: const TextStyle(color: Colors.white, fontFamily: "monospace"),
                    decoration: InputDecoration(
                      labelText: "Local Host / IP",
                      labelStyle: const TextStyle(color: Color(0xFF94A3B8)),
                      hintText: "djm-box.local or 192.168.x.x",
                      hintStyle: const TextStyle(color: Color(0xFF64748B), fontSize: 12),
                      filled: true,
                      fillColor: const Color(0xFF131722),
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(8),
                        borderSide: const BorderSide(color: Color(0xFF232A3B)),
                      ),
                    ),
                  ),
                  const SizedBox(height: 12),
                  TextField(
                    controller: _remoteUrlController,
                    style: const TextStyle(color: Colors.white, fontFamily: "monospace"),
                    decoration: InputDecoration(
                      labelText: "Remote / Tailscale URL (Optional)",
                      labelStyle: const TextStyle(color: Color(0xFF94A3B8)),
                      hintText: "https://djm-box.tailnet.ts.net:8080",
                      hintStyle: const TextStyle(color: Color(0xFF64748B), fontSize: 12),
                      filled: true,
                      fillColor: const Color(0xFF131722),
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(8),
                        borderSide: const BorderSide(color: Color(0xFF232A3B)),
                      ),
                    ),
                  ),
                  const SizedBox(height: 20),
                  ElevatedButton(
                    style: ElevatedButton.styleFrom(
                      backgroundColor: const Color(0xFF00F0FF),
                      foregroundColor: Colors.black,
                      padding: const EdgeInsets.symmetric(vertical: 14),
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(8),
                      ),
                    ),
                    onPressed: _handleManualSave,
                    child: const Text(
                      "Save and Connect",
                      style: TextStyle(fontWeight: FontWeight.bold, fontSize: 15),
                    ),
                  ),
                  const SizedBox(height: 12),
                  TextButton(
                    onPressed: () {
                      context.read<TransportManager>().clearPairing();
                      Navigator.of(context).pop();
                    },
                    child: const Text(
                      "Disconnect & Clear Credentials",
                      style: TextStyle(color: Color(0xFFFF3344)),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
