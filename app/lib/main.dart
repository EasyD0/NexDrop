import 'package:flutter/material.dart';
import 'app_model.dart';
import 'ui/main_pane.dart';
import 'ui/settings_page.dart';
import 'ui/sidebar.dart';

void main() {
  runApp(const NexDropApp());
}

class NexDropApp extends StatefulWidget {
  const NexDropApp({super.key});

  @override
  State<NexDropApp> createState() => _NexDropAppState();
}

class _NexDropAppState extends State<NexDropApp> {
  final model = AppModel();
  bool ready = false;
  String? error;

  @override
  void initState() {
    super.initState();
    model.bootstrap().then((_) {
      if (mounted) setState(() => ready = true);
    }).catchError((e) {
      if (mounted) setState(() { error = e.toString(); });
    });
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'NexDrop',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xFF4A7BD0)),
        useMaterial3: true,
      ),
      home: !ready
          ? Scaffold(body: Center(child: error == null
              ? const Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [CircularProgressIndicator(), SizedBox(height: 16), Text('NexDrop 启动中…')],
                )
              : Text('启动失败：$error', textAlign: TextAlign.center)))
          : ListenableBuilder(
              listenable: model,
              builder: (context, _) => const HomePage(),
            ),
    );
  }
}

class HomePage extends StatefulWidget {
  const HomePage({super.key});

  @override
  State<HomePage> createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  bool sidebarOpen = true;
  bool showingSettings = false;
  bool _pairDialogOpen = false;

  @override
  Widget build(BuildContext context) {
    final model = context.findAncestorStateOfType<_NexDropAppState>()!.model;
    _maybeShowPairDialog(context, model);
    return Scaffold(
      body: Row(
        children: [
          Sidebar(
            model: model,
            open: sidebarOpen,
            onToggle: () => setState(() => sidebarOpen = !sidebarOpen),
            onOpenSettings: () => setState(() => showingSettings = true),
          ),
          const VerticalDivider(width: 1),
          Expanded(
            child: showingSettings
                ? SettingsPage(
                    model: model,
                    onClose: () => setState(() => showingSettings = false),
                  )
                : MainPane(model: model),
          ),
        ],
      ),
    );
  }

  /// Incoming *pairing* request (name non-empty): modal accept/reject.
  /// Incoming *file* confirms are rendered inside the file box overlay.
  void _maybeShowPairDialog(BuildContext context, AppModel model) {
    final d = model.pendingDecision;
    if (d == null || d.isFileConfirm || _pairDialogOpen) return;
    _pairDialogOpen = true;
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted) return;
      final accept = await showDialog<bool>(
        context: context,
        barrierDismissible: false,
        builder: (ctx) => AlertDialog(
          title: const Text('配对请求'),
          content: Column(mainAxisSize: MainAxisSize.min, children: [
            Text('「${d.peerName}」请求与你配对'),
            const SizedBox(height: 12),
            Text('配对码：${d.code}', style: Theme.of(context).textTheme.titleMedium),
          ]),
          actions: [
            TextButton(onPressed: () => Navigator.pop(ctx, false), child: const Text('拒绝')),
            FilledButton(onPressed: () => Navigator.pop(ctx, true), child: const Text('同意')),
          ],
        ),
      );
      _pairDialogOpen = false;
      await model.decidePair(accept ?? false);
    });
  }
}
