import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import '../app_model.dart';
import '../src/rust/settings.dart';

/// Settings page: download path, visible device name, file-receive confirm.
class SettingsPage extends StatefulWidget {
  final AppModel model;
  final VoidCallback onClose;

  const SettingsPage({super.key, required this.model, required this.onClose});

  @override
  State<SettingsPage> createState() => _SettingsPageState();
}

class _SettingsPageState extends State<SettingsPage> {
  late TextEditingController nameCtrl;
  late TextEditingController pathCtrl;
  late bool requireConfirm;

  @override
  void initState() {
    super.initState();
    final s = widget.model.settings!;
    nameCtrl = TextEditingController(text: s.deviceName);
    pathCtrl = TextEditingController(text: s.downloadPath);
    requireConfirm = s.requirePairingConfirm;
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(children: [
            IconButton(onPressed: widget.onClose, icon: const Icon(Icons.arrow_back)),
            Text('设置', style: Theme.of(context).textTheme.titleLarge),
          ]),
          const SizedBox(height: 24),
          ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 480),
            child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              TextField(
                controller: nameCtrl,
                decoration: const InputDecoration(
                  labelText: '设备名称',
                  helperText: '其他设备看到的名字',
                  border: OutlineInputBorder(),
                ),
              ),
              const SizedBox(height: 16),
              TextField(
                controller: pathCtrl,
                decoration: InputDecoration(
                  labelText: '默认下载路径',
                  border: const OutlineInputBorder(),
                  suffixIcon: IconButton(
                    icon: const Icon(Icons.folder_open),
                    onPressed: () async {
                      final dir = await getDirectoryPath();
                      if (dir != null) pathCtrl.text = dir;
                    },
                  ),
                ),
              ),
              const SizedBox(height: 16),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('接收文件时需要配对码确认'),
                subtitle: const Text('关闭后，来自已配对设备的文件将自动开始接收'),
                value: requireConfirm,
                onChanged: (v) => setState(() => requireConfirm = v),
              ),
              const SizedBox(height: 24),
              FilledButton.icon(
                onPressed: () async {
                  await widget.model.saveSettings(Settings(
                    deviceName: nameCtrl.text.trim(),
                    downloadPath: pathCtrl.text.trim(),
                    requirePairingConfirm: requireConfirm,
                  ));
                  if (context.mounted) widget.onClose();
                },
                icon: const Icon(Icons.save),
                label: const Text('保存'),
              ),
            ]),
          ),
        ],
      ),
    );
  }
}
