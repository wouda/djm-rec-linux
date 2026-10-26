import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import 'screens/dashboard_screen.dart';
import 'services/transport_manager.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(
    ChangeNotifierProvider(
      create: (_) => TransportManager(),
      child: const DjmCompanionApp(),
    ),
  );
}

class DjmCompanionApp extends StatelessWidget {
  const DjmCompanionApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'djm-box remote',
      debugShowCheckedModeBanner: false,
      theme: ThemeData.dark().copyWith(
        scaffoldBackgroundColor: const Color(0xFF090A0F),
        colorScheme: const ColorScheme.dark(
          primary: Color(0xFF00F0FF),
          secondary: Color(0xFF00FF66),
          surface: Color(0xFF131722),
          error: Color(0xFFFF3344),
        ),
      ),
      home: const DashboardScreen(),
    );
  }
}
