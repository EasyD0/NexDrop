import 'package:flutter/material.dart';
import '../app_model.dart';
import '../src/rust/api.dart';

/// "Add device" flow: scan results -> pick one -> show pairing code and wait
/// for the peer to accept/reject.
Future<void> showPairingSheet(BuildContext context, AppModel model) async {
  await showModalBottomSheet(
    context: context,
    isScrollControlled: true,
    builder: (ctx) => _PairingSheet(model: model),
  );
  // Drop any finished pairing attempt so the dialog state resets.
  model.pairing = null;
  model.notifyExternal();
}

class _PairingSheet extends StatefulWidget {
  final AppModel model;
  const _PairingSheet({required this.model});

  @override
  State<_PairingSheet> createState() => _PairingSheetState();
}

class _PairingSheetState extends State<_PairingSheet> {
  @override
  Widget build(BuildContext context) {
    final model = widget.model;
    final attempt = model.pairing;

    return Padding(
      padding: const EdgeInsets.all(24),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(attempt == null ? '附近的设备' : '正在配对', style: Theme.of(context).textTheme.titleLarge),
          const SizedBox(height: 16),
          if (attempt == null) ...[
            ...model.devices.where((d) => d.online).map(
              (d) => ListTile(
                leading: const Icon(Icons.computer),
                title: Text(d.displayName),
                subtitle: Text(d.paired ? '已配对' : '未配对'),
                onTap: () async {
                  try {
                    await model.startPairing(d);
                  } catch (_) {}
                  if (context.mounted) setState(() {});
                },
              ),
            ),
            if (model.devices.where((d) => d.online).isEmpty)
              const Padding(
                padding: EdgeInsets.symmetric(vertical: 32),
                child: Center(child: Text('正在扫描局域网…', style: TextStyle(color: Colors.grey))),
              ),
          ] else ...[
            // Show the pairing code big, wait for the peer's decision.
            Center(
              child: Column(children: [
                const SizedBox(height: 8),
                Text('请在对方设备上确认', style: Theme.of(context).textTheme.bodyMedium),
                const SizedBox(height: 12),
                Text(attempt.code, style: Theme.of(context).textTheme.displayMedium),
                const SizedBox(height: 16),
                if (!attempt.done)
                  const CircularProgressIndicator()
                else if (attempt.accepted)
                  const Row(mainAxisSize: MainAxisSize.min, children: [
                    Icon(Icons.check_circle, color: Colors.green), SizedBox(width: 8), Text('配对成功'),
                  ])
                else
                  const Row(mainAxisSize: MainAxisSize.min, children: [
                    Icon(Icons.cancel, color: Colors.red), SizedBox(width: 8), Text('配对失败'),
                  ]),
              ]),
            ),
          ],
          const SizedBox(height: 16),
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('关闭'),
          ),
        ],
      ),
    );
  }
}
