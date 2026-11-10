import 'dart:async';
import 'package:audioplayers/audioplayers.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';

import '../models/prodj.dart';
import '../models/studio.dart';
import '../services/transport_manager.dart';

class PostEventStudioScreen extends StatefulWidget {
  const PostEventStudioScreen({super.key});

  @override
  State<PostEventStudioScreen> createState() => _PostEventStudioScreenState();
}

class _PostEventStudioScreenState extends State<PostEventStudioScreen> {
  List<EventSession> _sessions = [];
  EventSession? _selectedSession;
  bool _isLoadingSessions = false;

  final TextEditingController _timetableController = TextEditingController();
  bool _isSavingTimetable = false;
  List<ArtistSlot> _slots = [];

  List<SetBoundary> _boundaries = [];
  bool _isAnalyzing = false;

  // Mastering config
  String _selectedPreset = 'Streaming';
  bool _enableDeclip = false;
  bool _exportMp3 = true;
  bool _exportFullEvent = true;

  // Render job progress
  JobState? _activeJob;
  Timer? _jobPollTimer;
  bool _isRendering = false;

  // Audio preview player
  AudioPlayer? _previewPlayer;
  int? _previewingSlotIndex;
  bool _isPlayingPreview = false;

  @override
  void initState() {
    super.initState();
    _loadSessions();
  }

  @override
  void dispose() {
    _jobPollTimer?.cancel();
    _previewPlayer?.dispose();
    _timetableController.dispose();
    super.dispose();
  }

  Future<void> _loadSessions() async {
    setState(() => _isLoadingSessions = true);
    final transport = context.read<TransportManager>();
    try {
      final list = await transport.fetchStudioSessions();
      if (!mounted) return;
      setState(() {
        _sessions = list;
        _isLoadingSessions = false;
        if (list.isNotEmpty && _selectedSession == null) {
          _selectSession(list.first);
        }
      });
    } catch (e) {
      if (!mounted) return;
      setState(() => _isLoadingSessions = false);
      _showSnackbar("Failed to load sessions: $e");
    }
  }

  Future<void> _selectSession(EventSession session) async {
    setState(() {
      _selectedSession = session;
      _boundaries = [];
      _activeJob = null;
    });
    final transport = context.read<TransportManager>();
    try {
      final tt = await transport.fetchStudioTimetable(session.id);
      if (!mounted) return;
      setState(() {
        _timetableController.text = tt.content;
        _slots = tt.slots;
      });
    } catch (_) {}
  }

  Future<void> _saveTimetable() async {
    if (_selectedSession == null) return;
    setState(() => _isSavingTimetable = true);
    final transport = context.read<TransportManager>();
    try {
      final tt = await transport.saveStudioTimetable(
        _selectedSession!.id,
        _timetableController.text,
      );
      if (!mounted) return;
      setState(() {
        _slots = tt.slots;
        _isSavingTimetable = false;
      });
      _showSnackbar("Timetable lineup saved (${tt.slots.length} artists)");
    } catch (e) {
      if (!mounted) return;
      setState(() => _isSavingTimetable = false);
      _showSnackbar("Failed to save timetable: $e");
    }
  }

  Future<void> _startAnalysis() async {
    if (_selectedSession == null) return;
    setState(() => _isAnalyzing = true);
    final transport = context.read<TransportManager>();
    try {
      final jobId = await transport.startStudioAnalyze(_selectedSession!.id);
      _pollJob(jobId, onComplete: () async {
        // Reload timetable / resolved boundaries
        final tt = await transport.fetchStudioTimetable(_selectedSession!.id);
        if (!mounted) return;
        setState(() {
          _slots = tt.slots;
          _isAnalyzing = false;
        });
        _showSnackbar("Analysis complete! Boundaries resolved.");
      });
    } catch (e) {
      if (!mounted) return;
      setState(() => _isAnalyzing = false);
      _showSnackbar("Analysis failed: $e");
    }
  }

  Future<void> _startRender() async {
    if (_selectedSession == null) return;
    setState(() => _isRendering = true);
    final transport = context.read<TransportManager>();
    try {
      final req = RenderRequest(
        sessionId: _selectedSession!.id,
        preset: _selectedPreset,
        declip: _enableDeclip,
        mp3: _exportMp3,
        fullEvent: _exportFullEvent,
      );
      final jobId = await transport.startStudioRender(req);
      _pollJob(jobId, onComplete: () {
        if (!mounted) return;
        setState(() => _isRendering = false);
        _showSnackbar("Mastering and rendering completed successfully!");
      });
    } catch (e) {
      if (!mounted) return;
      setState(() => _isRendering = false);
      _showSnackbar("Render failed: $e");
    }
  }

  void _pollJob(String jobId, {required VoidCallback onComplete}) {
    _jobPollTimer?.cancel();
    final transport = context.read<TransportManager>();

    _jobPollTimer = Timer.periodic(const Duration(seconds: 1), (timer) async {
      try {
        final job = await transport.fetchStudioJob(jobId);
        if (!mounted) {
          timer.cancel();
          return;
        }
        setState(() => _activeJob = job);

        if (job.status == JobStatus.completed) {
          timer.cancel();
          onComplete();
        } else if (job.status == JobStatus.failed ||
            job.status == JobStatus.aborted ||
            job.status == JobStatus.interrupted) {
          timer.cancel();
          setState(() {
            _isAnalyzing = false;
            _isRendering = false;
          });
          _showSnackbar("Job ${job.status.name}: ${job.errorMessage ?? 'Unknown error'}");
        }
      } catch (e) {
        debugPrint("Error polling job: $e");
      }
    });
  }

  void _playTransitionPreview(int slotIndex, int boundarySample) async {
    if (_selectedSession == null) return;
    final transport = context.read<TransportManager>();
    final url = transport.getTransitionPreviewUrl(_selectedSession!.id, boundarySample);

    if (_previewPlayer != null) {
      await _previewPlayer!.stop();
      _previewPlayer!.dispose();
      _previewPlayer = null;
    }

    if (_previewingSlotIndex == slotIndex && _isPlayingPreview) {
      setState(() {
        _isPlayingPreview = false;
        _previewingSlotIndex = null;
      });
      return;
    }

    final player = AudioPlayer();
    _previewPlayer = player;
    setState(() {
      _previewingSlotIndex = slotIndex;
      _isPlayingPreview = true;
    });

    try {
      await player.play(UrlSource(url, mimeType: 'audio/mpeg'));
      player.onPlayerComplete.listen((_) {
        if (!mounted) return;
        setState(() {
          _isPlayingPreview = false;
          _previewingSlotIndex = null;
        });
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _isPlayingPreview = false;
        _previewingSlotIndex = null;
      });
      _showSnackbar("Failed to preview transition: $e");
    }
  }

  Future<void> _viewReport() async {
    if (_selectedSession == null) return;
    final transport = context.read<TransportManager>();
    try {
      final report = await transport.fetchStudioReport(_selectedSession!.id);
      if (!mounted) return;
      showDialog(
        context: context,
        builder: (ctx) => AlertDialog(
          backgroundColor: const Color(0xFF131722),
          title: Text(
            "Event Report: ${_selectedSession!.id}",
            style: const TextStyle(color: Colors.white, fontSize: 16),
          ),
          content: SingleChildScrollView(
            child: SelectableText(
              report,
              style: const TextStyle(color: Color(0xFFE2E8F0), fontSize: 12, fontFamily: 'monospace'),
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(ctx).pop(),
              child: const Text("Close", style: TextStyle(color: Color(0xFF00F0FF))),
            ),
          ],
        ),
      );
    } catch (e) {
      _showSnackbar("Failed to fetch report: $e");
    }
  }

  void _showSnackbar(String msg) {
    ScaffoldMessenger.of(context).hideCurrentSnackBar();
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(msg),
        backgroundColor: const Color(0xFF1E2638),
        duration: const Duration(seconds: 3),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final transport = context.watch<TransportManager>();
    final prodj = transport.telemetry.prodj;

    return Scaffold(
      backgroundColor: const Color(0xFF090A0F),
      appBar: AppBar(
        backgroundColor: const Color(0xFF131722),
        elevation: 0,
        title: const Text(
          "Post-Event Mastering Studio",
          style: TextStyle(fontSize: 16, fontWeight: FontWeight.bold, color: Colors.white),
        ),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh, color: Color(0xFF00F0FF)),
            onPressed: _loadSessions,
          ),
        ],
      ),
      body: _isLoadingSessions
          ? const Center(child: CircularProgressIndicator(color: Color(0xFF00F0FF)))
          : SingleChildScrollView(
              padding: const EdgeInsets.all(12),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  // Session Picker Card
                  _buildSessionPickerCard(),

                  const SizedBox(height: 12),

                  // Timetable Editor & USB Mapping Card
                  _buildTimetableEditorCard(prodj),

                  const SizedBox(height: 12),

                  // Split Review & Boundary Table Card
                  _buildSplitReviewCard(),

                  const SizedBox(height: 12),

                  // Mastering Presets & Render Options Card
                  _buildRenderOptionsCard(),

                  const SizedBox(height: 12),

                  // Progress & Status Card
                  if (_activeJob != null) ...[
                    _buildProgressCard(),
                    const SizedBox(height: 12),
                  ],

                  // Action Buttons Card
                  _buildBottomActionsCard(),
                ],
              ),
            ),
    );
  }

  Widget _buildSessionPickerCard() {
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: const Color(0xFF131722),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: const Color(0xFF232A3B)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            "Select Event Session",
            style: TextStyle(fontSize: 12, fontWeight: FontWeight.bold, color: Color(0xFF94A3B8)),
          ),
          const SizedBox(height: 8),
          if (_sessions.isEmpty)
            const Text(
              "No recorded sessions found in recordings directory.",
              style: TextStyle(color: Color(0xFF94A3B8), fontSize: 13),
            )
          else
            DropdownButtonFormField<EventSession>(
              value: _selectedSession,
              dropdownColor: const Color(0xFF1E2638),
              style: const TextStyle(color: Colors.white, fontSize: 13),
              decoration: InputDecoration(
                filled: true,
                fillColor: const Color(0xFF0A0D14),
                contentPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(8),
                  borderSide: const BorderSide(color: Color(0xFF232A3B)),
                ),
              ),
              items: _sessions.map((s) {
                return DropdownMenuItem<EventSession>(
                  value: s,
                  child: Text(
                    "${s.id} (${s.segments.length} segs, ${s.formattedDuration}, ${s.mixerModel})",
                    overflow: TextOverflow.ellipsis,
                  ),
                );
              }).toList(),
              onChanged: (val) {
                if (val != null) _selectSession(val);
              },
            ),
          if (_selectedSession != null) ...[
            const SizedBox(height: 8),
            Row(
              children: [
                _buildSmallBadge(
                  _selectedSession!.continuityAudit.isContinuous ? "Continuous Seams" : "Discontinuous",
                  _selectedSession!.continuityAudit.isContinuous ? const Color(0xFF16A34A) : const Color(0xFFDC2626),
                ),
                const SizedBox(width: 8),
                Text(
                  "${(_selectedSession!.sampleRate / 1000).toStringAsFixed(1)} kHz · ${_selectedSession!.segments.length} Flac Segments",
                  style: const TextStyle(color: Color(0xFF94A3B8), fontSize: 11),
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildTimetableEditorCard(ProDjTelemetry? prodj) {
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: const Color(0xFF131722),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: const Color(0xFF232A3B)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              const Text(
                "Timetable / Running Order",
                style: TextStyle(fontSize: 13, fontWeight: FontWeight.bold, color: Colors.white),
              ),
              ElevatedButton.icon(
                style: ElevatedButton.styleFrom(
                  backgroundColor: const Color(0xFF0284C7),
                  foregroundColor: Colors.white,
                  padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                  shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(6)),
                ),
                icon: _isSavingTimetable
                    ? const SizedBox(width: 12, height: 12, child: CircularProgressIndicator(strokeWidth: 2, color: Colors.white))
                    : const Icon(Icons.save, size: 14),
                label: const Text("Save Lineup", style: TextStyle(fontSize: 11, fontWeight: FontWeight.bold)),
                onPressed: _isSavingTimetable ? null : _saveTimetable,
              ),
            ],
          ),
          const SizedBox(height: 8),
          TextField(
            controller: _timetableController,
            maxLines: 4,
            style: const TextStyle(color: Colors.white, fontFamily: 'monospace', fontSize: 12),
            decoration: InputDecoration(
              hintText: "22:00 Artist 1\n23:00 Artist 2\n00:30 Headliner",
              hintStyle: const TextStyle(color: Color(0xFF64748B), fontSize: 12),
              filled: true,
              fillColor: const Color(0xFF0A0D14),
              border: OutlineInputBorder(
                borderRadius: BorderRadius.circular(8),
                borderSide: const BorderSide(color: Color(0xFF232A3B)),
              ),
            ),
          ),

          // USB Clusters & B2B Mapping Section
          if (prodj != null && prodj.activeClusters.isNotEmpty) ...[
            const SizedBox(height: 10),
            const Text(
              "Detected Pro DJ Link USB Sticks & Clusters:",
              style: TextStyle(color: Color(0xFF94A3B8), fontSize: 11, fontWeight: FontWeight.bold),
            ),
            const SizedBox(height: 6),
            ...prodj.activeClusters.map((c) => Container(
                  margin: const EdgeInsets.only(bottom: 4),
                  padding: const EdgeInsets.all(8),
                  decoration: BoxDecoration(
                    color: const Color(0xFF0A0D14),
                    borderRadius: BorderRadius.circular(6),
                    border: Border.all(color: const Color(0xFF1E2638)),
                  ),
                  child: Row(
                    children: [
                      const Icon(Icons.usb, color: Color(0xFF00F0FF), size: 16),
                      const SizedBox(width: 6),
                      Expanded(
                        child: Text(
                          "Cluster #${c.clusterId}: ${c.mediaNames.join(', ')} (${c.trackCount} tracks, proposed: ${c.proposedArtist ?? 'Unassigned'})",
                          style: const TextStyle(color: Color(0xFFE2E8F0), fontSize: 11),
                        ),
                      ),
                    ],
                  ),
                )),
          ],
        ],
      ),
    );
  }

  Widget _buildSplitReviewCard() {
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: const Color(0xFF131722),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: const Color(0xFF232A3B)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              const Text(
                "Artist Set Splits & Boundary Review",
                style: TextStyle(fontSize: 13, fontWeight: FontWeight.bold, color: Colors.white),
              ),
              ElevatedButton.icon(
                style: ElevatedButton.styleFrom(
                  backgroundColor: const Color(0xFF1E2638),
                  foregroundColor: const Color(0xFF00F0FF),
                  padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(6),
                    side: const BorderSide(color: Color(0xFF232A3B)),
                  ),
                ),
                icon: _isAnalyzing
                    ? const SizedBox(width: 12, height: 12, child: CircularProgressIndicator(strokeWidth: 2, color: Color(0xFF00F0FF)))
                    : const Icon(Icons.analytics, size: 14),
                label: const Text("Analyze & Resolve", style: TextStyle(fontSize: 11, fontWeight: FontWeight.bold)),
                onPressed: _isAnalyzing ? null : _startAnalysis,
              ),
            ],
          ),
          const SizedBox(height: 8),
          if (_slots.isEmpty)
            const Padding(
              padding: EdgeInsets.symmetric(vertical: 12),
              child: Center(
                child: Text(
                  "No artist slots resolved yet. Enter lineup and click Analyze.",
                  style: TextStyle(color: Color(0xFF64748B), fontSize: 12),
                ),
              ),
            )
          else
            ListView.separated(
              shrinkWrap: true,
              physics: const NeverScrollableScrollPhysics(),
              itemCount: _slots.length,
              separatorBuilder: (_, __) => const Divider(color: Color(0xFF1E2638), height: 12),
              itemBuilder: (ctx, idx) {
                final slot = _slots[idx];
                final isPlayingThis = _previewingSlotIndex == idx && _isPlayingPreview;
                return Row(
                  children: [
                    Container(
                      width: 24,
                      height: 24,
                      decoration: const BoxDecoration(
                        color: Color(0xFF1E2638),
                        shape: BoxShape.circle,
                      ),
                      alignment: Alignment.center,
                      child: Text(
                        "${idx + 1}",
                        style: const TextStyle(color: Colors.white, fontSize: 11, fontWeight: FontWeight.bold),
                      ),
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            slot.artist,
                            style: const TextStyle(color: Colors.white, fontWeight: FontWeight.bold, fontSize: 13),
                          ),
                          Text(
                            "${slot.scheduledStartClock} · ${slot.assignedUsbMedia ?? 'Pro DJ Handover / Silence'}",
                            style: const TextStyle(color: Color(0xFF94A3B8), fontSize: 11),
                          ),
                        ],
                      ),
                    ),
                    IconButton(
                      icon: Icon(
                        isPlayingThis ? Icons.stop_circle : Icons.play_circle_outline,
                        color: isPlayingThis ? const Color(0xFFFF3344) : const Color(0xFF00FF66),
                        size: 24,
                      ),
                      onPressed: () {
                        // Sample offset mock or slot sample calculation
                        final sampleOffset = idx * 96000 * 3600;
                        _playTransitionPreview(idx, sampleOffset);
                      },
                    ),
                  ],
                );
              },
            ),
        ],
      ),
    );
  }

  Widget _buildRenderOptionsCard() {
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: const Color(0xFF131722),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: const Color(0xFF232A3B)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            "Mastering Preset & Export Settings",
            style: TextStyle(fontSize: 13, fontWeight: FontWeight.bold, color: Colors.white),
          ),
          const SizedBox(height: 8),
          Row(
            children: [
              Expanded(
                child: DropdownButtonFormField<String>(
                  value: _selectedPreset,
                  dropdownColor: const Color(0xFF1E2638),
                  style: const TextStyle(color: Colors.white, fontSize: 12),
                  decoration: InputDecoration(
                    labelText: "Loudness Preset",
                    labelStyle: const TextStyle(color: Color(0xFF94A3B8), fontSize: 11),
                    filled: true,
                    fillColor: const Color(0xFF0A0D14),
                    contentPadding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(8),
                      borderSide: const BorderSide(color: Color(0xFF232A3B)),
                    ),
                  ),
                  items: const [
                    DropdownMenuItem(value: 'Streaming', child: Text('Streaming (-14 LUFS)')),
                    DropdownMenuItem(value: 'Club', child: Text('Club (-11 LUFS)')),
                  ],
                  onChanged: (v) {
                    if (v != null) setState(() => _selectedPreset = v);
                  },
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          CheckboxListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            title: const Text("De-clip repair (interpolate clipped peaks)", style: TextStyle(color: Color(0xFFE2E8F0), fontSize: 12)),
            value: _enableDeclip,
            activeColor: const Color(0xFF00F0FF),
            checkColor: Colors.black,
            onChanged: (v) => setState(() => _enableDeclip = v ?? false),
          ),
          CheckboxListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            title: const Text("Render 320kbps MP3s in addition to 24-bit FLACs", style: TextStyle(color: Color(0xFFE2E8F0), fontSize: 12)),
            value: _exportMp3,
            activeColor: const Color(0xFF00F0FF),
            checkColor: Colors.black,
            onChanged: (v) => setState(() => _exportMp3 = v ?? true),
          ),
          CheckboxListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            title: const Text("Export Full Event Continuous Mix (with .cue & .txt tracklist)", style: TextStyle(color: Color(0xFFE2E8F0), fontSize: 12)),
            value: _exportFullEvent,
            activeColor: const Color(0xFF00F0FF),
            checkColor: Colors.black,
            onChanged: (v) => setState(() => _exportFullEvent = v ?? true),
          ),
        ],
      ),
    );
  }

  Widget _buildProgressCard() {
    final job = _activeJob!;
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: const Color(0xFF131722),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: const Color(0xFF232A3B)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(
                "Processing: ${job.phase}",
                style: const TextStyle(color: Colors.white, fontWeight: FontWeight.bold, fontSize: 12),
              ),
              Text(
                "${job.progressPercent.toStringAsFixed(0)}%",
                style: const TextStyle(color: Color(0xFF00F0FF), fontWeight: FontWeight.bold, fontSize: 12),
              ),
            ],
          ),
          const SizedBox(height: 8),
          LinearProgressIndicator(
            value: job.progressPercent / 100.0,
            backgroundColor: const Color(0xFF0A0D14),
            color: const Color(0xFF00F0FF),
            minHeight: 8,
            borderRadius: BorderRadius.circular(4),
          ),
          const SizedBox(height: 6),
          Text(
            "Step ${job.currentStep} of ${job.totalSteps}",
            style: const TextStyle(color: Color(0xFF94A3B8), fontSize: 11),
          ),
        ],
      ),
    );
  }

  Widget _buildBottomActionsCard() {
    return Column(
      children: [
        SizedBox(
          width: double.infinity,
          child: ElevatedButton.icon(
            style: ElevatedButton.styleFrom(
              backgroundColor: const Color(0xFF00F0FF),
              foregroundColor: Colors.black,
              padding: const EdgeInsets.symmetric(vertical: 14),
              shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
              elevation: 0,
            ),
            icon: _isRendering
                ? const SizedBox(width: 16, height: 16, child: CircularProgressIndicator(strokeWidth: 2, color: Colors.black))
                : const Icon(Icons.flash_on, size: 18),
            label: Text(
              _isRendering ? "Rendering in Progress..." : "⚡ Start Mastering & Render All Sets",
              style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 13),
            ),
            onPressed: (_isRendering || _selectedSession == null) ? null : _startRender,
          ),
        ),
        const SizedBox(height: 8),
        SizedBox(
          width: double.infinity,
          child: OutlinedButton.icon(
            style: OutlinedButton.styleFrom(
              foregroundColor: const Color(0xFFE2E8F0),
              side: const BorderSide(color: Color(0xFF232A3B)),
              padding: const EdgeInsets.symmetric(vertical: 12),
              shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
            ),
            icon: const Icon(Icons.description, size: 16),
            label: const Text("View Event Quality Report", style: TextStyle(fontSize: 12, fontWeight: FontWeight.bold)),
            onPressed: _selectedSession == null ? null : _viewReport,
          ),
        ),
      ],
    );
  }

  Widget _buildSmallBadge(String text, Color color) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: color,
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        text,
        style: const TextStyle(color: Colors.white, fontSize: 10, fontWeight: FontWeight.bold),
      ),
    );
  }
}
