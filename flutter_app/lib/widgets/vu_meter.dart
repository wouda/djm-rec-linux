import 'package:flutter/material.dart';

class VuMeterWidget extends StatelessWidget {
  final String label;
  final double peakDb;
  final double rmsDb;

  const VuMeterWidget({
    super.key,
    required this.label,
    required this.peakDb,
    required this.rmsDb,
  });

  // Map dB (-60 to 0) to percentage (0.0 to 1.0)
  double _dbToPct(double db) {
    if (db <= -60.0) return 0.0;
    if (db >= 0.0) return 1.0;
    return (db + 60.0) / 60.0;
  }

  @override
  Widget build(BuildContext context) {
    final pct = _dbToPct(peakDb);
    // Reference marker at -19 dBFS: (-19 + 60) / 60 = 41 / 60 = 0.6833
    const refPosition = 41.0 / 60.0;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            Text(
              label,
              style: const TextStyle(
                color: Color(0xFF94A3B8),
                fontSize: 12,
                fontWeight: FontWeight.bold,
              ),
            ),
            Text(
              peakDb <= -60.0 ? "-∞ dBFS" : "${peakDb.toStringAsFixed(1)} dBFS",
              style: TextStyle(
                color: peakDb >= -0.5
                    ? const Color(0xFFFF3344)
                    : (peakDb >= -19.0
                        ? const Color(0xFFFFD000)
                        : const Color(0xFF00FF66)),
                fontSize: 12,
                fontFamily: "monospace",
                fontWeight: FontWeight.bold,
              ),
            ),
          ],
        ),
        const SizedBox(height: 4),
        Container(
          height: 24,
          decoration: BoxDecoration(
            color: const Color(0xFF000000),
            borderRadius: BorderRadius.circular(4),
            border: Border.all(color: const Color(0xFF232A3B)),
          ),
          child: LayoutBuilder(
            builder: (context, constraints) {
              final w = constraints.maxWidth;
              return Stack(
                children: [
                  // Active VU fill bar
                  AnimatedContainer(
                    duration: const Duration(milliseconds: 50),
                    width: w * pct,
                    height: constraints.maxHeight,
                    decoration: BoxDecoration(
                      borderRadius: BorderRadius.circular(3),
                      gradient: const LinearGradient(
                        colors: [
                          Color(0xFF00FF66), // Green (-60 to -19 dB)
                          Color(0xFFFFD000), // Yellow (-19 to -3 dB)
                          Color(0xFFFF3344), // Red (-3 to 0 dB)
                        ],
                        stops: [0.0, 0.68, 0.95],
                      ),
                    ),
                  ),

                  // -19 dBFS Physical 0dB reference marker line
                  Positioned(
                    left: w * refPosition,
                    top: 0,
                    bottom: 0,
                    child: Container(
                      width: 2,
                      decoration: const BoxDecoration(
                        color: Colors.white,
                        boxShadow: [
                          BoxShadow(
                            color: Colors.white,
                            blurRadius: 4,
                          ),
                        ],
                      ),
                    ),
                  ),

                  // -19 dBFS reference label
                  Positioned(
                    left: (w * refPosition) - 24,
                    top: 4,
                    child: const Text(
                      "0dB MIXER",
                      style: TextStyle(
                        color: Colors.white,
                        fontSize: 8,
                        fontWeight: FontWeight.w900,
                        shadows: [
                          Shadow(
                            color: Colors.black,
                            blurRadius: 2,
                          ),
                        ],
                      ),
                    ),
                  ),
                ],
              );
            },
          ),
        ),
      ],
    );
  }
}
