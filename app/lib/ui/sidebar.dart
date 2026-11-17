import 'package:flutter/material.dart';
import '../app_model.dart';
import '../src/rust/api.dart' show DeviceView;
import 'pairing_sheet.dart';

/// Collapsible device sidebar: list of paired/discovered devices, an add
/// button (pairing sheet) and a settings button at the bottom.
class Sidebar extends StatelessWidget {
  final AppModel model;
  final bool open;
  final VoidCallback onToggle;
  final VoidCallback onOpenSettings;

  const Sidebar({
    super.key,
    required this.model,
    required this.open,
    required this.onToggle,
    required this.onOpenSettings,
  });

  @override
  Widget build(BuildContext context) {
    return AnimatedContainer(
      duration: const Duration(milliseconds: 200),
      width: open ? 240 : 56,
      child: Column(
        children: [
          ListTile(
            leading: Icon(open ? Icons.chevron_left : Icons.chevron_right),
            title: open ? const Text('NexDrop', style: TextStyle(fontWeight: FontWeight.bold)) : null,
            onTap: onToggle,
          ),
          const Divider(height: 1),
          Expanded(
            child: model.devices.isEmpty
                ? (open ? const Center(child: Text('暂无设备\n点击下方 ＋ 添加', textAlign: TextAlign.center)) : const SizedBox())
                : ListView.builder(
                    itemCount: model.devices.length,
                    itemBuilder: (context, i) {
                      final d = model.devices[i];
                      final selected = model.selectedDevice?.id == d.id;
                      return ListTile(
                        dense: !open,
                        selected: selected,
                        leading: Icon(
                          d.paired ? Icons.computer : Icons.devices_other,
                          color: d.online ? Colors.green : Colors.grey,
                        ),
                        title: open
                            ? Text(d.displayName, overflow: TextOverflow.ellipsis)
                            : null,
                        subtitle: open
                            ? Text(d.paired ? (d.online ? '在线' : '离线') : '未配对', style: const TextStyle(fontSize: 12))
                            : null,
                        onTap: () {
                          model.selectDevice(d);
                        },
                        trailing: open ? _itemMenu(context, d) : null,
                      );
                    },
                  ),
          ),
          const Divider(height: 1),
          ListTile(
            dense: !open,
            leading: const Icon(Icons.add),
            title: open ? const Text('添加设备') : null,
            onTap: () => showPairingSheet(context, model),
          ),
          ListTile(
            dense: !open,
            leading: const Icon(Icons.settings),
            title: open ? const Text('设置') : null,
            onTap: onOpenSettings,
          ),
        ],
      ),
    );
  }

  Widget _itemMenu(BuildContext context, DeviceView d) {
    return PopupMenuButton<String>(
      icon: const Icon(Icons.more_vert, size: 18),
      onSelected: (action) async {
        if (action == 'rename') {
          final controller = TextEditingController(text: d.displayName);
          final alias = await showDialog<String>(
            context: context,
            builder: (ctx) => AlertDialog(
              title: const Text('重命名设备'),
              content: TextField(controller: controller, autofocus: true,
                  decoration: InputDecoration(hintText: d.deviceName)),
              actions: [
                TextButton(onPressed: () => Navigator.pop(ctx), child: const Text('取消')),
                FilledButton(onPressed: () => Navigator.pop(ctx, controller.text.trim()), child: const Text('保存')),
              ],
            ),
          );
          if (alias != null) await model.renameDevice(d, alias);
        } else if (action == 'delete') {
          final ok = await showDialog<bool>(
            context: context,
            builder: (ctx) => AlertDialog(
              title: const Text('删除设备'),
              content: Text('确定删除「${d.displayName}」吗？将断开配对关系。'),
              actions: [
                TextButton(onPressed: () => Navigator.pop(ctx, false), child: const Text('取消')),
                FilledButton(onPressed: () => Navigator.pop(ctx, true), child: const Text('删除')),
              ],
            ),
          );
          if (ok == true) await model.removeDevice(d);
        }
      },
      itemBuilder: (ctx) => const [
        PopupMenuItem(value: 'rename', child: ListTile(leading: Icon(Icons.edit), title: Text('重命名'))),
        PopupMenuItem(value: 'delete', child: ListTile(leading: Icon(Icons.delete_outline), title: Text('删除'))),
      ],
    );
  }
}
