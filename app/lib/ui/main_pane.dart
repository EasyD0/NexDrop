import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../app_model.dart';

/// Main interaction area: file box + text box side by side (desktop v1).
/// While a transfer for that kind is active, the box shows the progress ring.
class MainPane extends StatelessWidget {
  final AppModel model;
  const MainPane({super.key, required this.model});

  @override
  Widget build(BuildContext context) {
    if (model.selectedDevice == null) {
      return const Center(child: Text('从左侧选择一个设备开始传输'));
    }
    return Padding(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text('与「${model.selectedDevice!.displayName}」', style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 12),
          Expanded(
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Expanded(child: _FileBox(model: model)),
                const SizedBox(width: 12),
                Expanded(child: _TextBox(model: model)),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

// ------------------------------------------------------------------ file box

class _FileBox extends StatelessWidget {
  final AppModel model;
  const _FileBox({required this.model});

  Future<void> _pickFiles(BuildContext context) async {
    const typeGroup = XTypeGroup(label: '所有文件');
    final files = await openFiles(acceptedTypeGroups: [typeGroup]);
    if (files.isNotEmpty) {
      model.addFiles(files.map((f) => f.path).toList());
    }
  }

  @override
  Widget build(BuildContext context) {
    final transfer = model.fileTransfer;
    final decision = model.pendingDecision;
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (transfer != null)
              Expanded(child: TransferOverlay(
                transfer: transfer,
                decision: decision,
                onCancel: model.cancelTransfer,
                onDecide: model.decideFile,
              ))
            else ...[
              // File list; tapping empty space opens the file picker.
              Expanded(
                child: GestureDetector(
                  onTap: () => _pickFiles(context),
                  behavior: HitTestBehavior.opaque,
                  child: model.chosenFiles.isEmpty
                      ? const Center(child: Text('点击此处选择文件', style: TextStyle(color: Colors.grey)))
                      : ListView.builder(
                          itemCount: model.chosenFiles.length,
                          itemBuilder: (context, i) {
                            final path = model.chosenFiles[i];
                            return ListTile(
                              dense: true,
                              leading: const Icon(Icons.insert_drive_file_outlined),
                              title: Text(path.split(Platform.pathSeparator).last,
                                  overflow: TextOverflow.ellipsis),
                              subtitle: Text(path, style: const TextStyle(fontSize: 11), maxLines: 1),
                              trailing: IconButton(
                                icon: const Icon(Icons.close, size: 18),
                                onPressed: () => model.removeFile(path),
                              ),
                            );
                          },
                        ),
                ),
              ),
              Row(children: [
                Expanded(
                  child: FilledButton.tonalIcon(
                    onPressed: model.clearFiles,
                    icon: const Icon(Icons.clear_all),
                    label: const Text('清除'),
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: FilledButton.icon(
                    onPressed: model.chosenFiles.isEmpty ? null : () => model.sendFiles(model.chosenFiles),
                    icon: const Icon(Icons.send),
                    label: const Text('发送'),
                  ),
                ),
              ]),
            ],
          ],
        ),
      ),
    );
  }
}

// ------------------------------------------------------------------ text box

class _TextBox extends StatelessWidget {
  final AppModel model;
  const _TextBox({required this.model});

  @override
  Widget build(BuildContext context) {
    final transfer = model.textTransfer;
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            if (transfer != null)
              Expanded(child: TransferOverlay(
                transfer: transfer,
                decision: null,
                onCancel: model.cancelTransfer,
                onDecide: model.decideFile,
              ))
            else ...[
              Expanded(
                child: TextField(
                  controller: TextEditingController(text: model.text)
                    ..selection = TextSelection.collapsed(offset: model.text.length),
                  onChanged: model.setText,
                  maxLines: null,
                  expands: true,
                  textAlignVertical: TextAlignVertical.top,
                  decoration: const InputDecoration(
                    hintText: '输入要传输的文本…',
                    border: OutlineInputBorder(),
                    isDense: true,
                  ),
                ),
              ),
              const SizedBox(height: 8),
              Row(children: [
                _mini(Icons.copy, '复制', () async {
                  await Clipboard.setData(ClipboardData(text: model.text));
                }),
                _mini(Icons.paste, '粘贴', () async {
                  final data = await Clipboard.getData('text/plain');
                  model.setText(data?.text ?? '');
                }),
                _mini(Icons.clear, '清除', () => model.setText('')),
                Expanded(
                  child: FilledButton.icon(
                    onPressed: model.text.isEmpty ? null : model.sendText,
                    icon: const Icon(Icons.send),
                    label: const Text('发送'),
                  ),
                ),
              ]),
            ],
          ],
        ),
      ),
    );
  }

  Widget _mini(IconData icon, String label, VoidCallback onTap) {
    return Padding(
      padding: const EdgeInsets.only(right: 8),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(8),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 8),
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            Icon(icon, size: 18),
            Text(label, style: const TextStyle(fontSize: 10)),
          ]),
        ),
      ),
    );
  }
}

// ------------------------------------------------------------ transfer ring

/// Circular progress with the pairing code in the middle. Buttons depend on
/// direction/verification state, per a.md:
/// - outgoing: 中断发送
/// - incoming file with verification: 接收 / 拒绝
/// - incoming otherwise: 取消
class TransferOverlay extends StatelessWidget {
  final TransferUi transfer;
  final PendingDecision? decision;
  final VoidCallback onCancel;
  final Future<void> Function(bool) onDecide;

  const TransferOverlay({
    super.key,
    required this.transfer,
    required this.decision,
    required this.onCancel,
    required this.onDecide,
  });

  @override
  Widget build(BuildContext context) {
    final failed = transfer.failedReason != null;
    final needsDecision = decision != null && !transfer.outgoing;
    return Center(
      child: Column(mainAxisSize: MainAxisSize.min, children: [
        Stack(alignment: Alignment.center, children: [
          SizedBox(
            width: 140,
            height: 140,
            child: CircularProgressIndicator(
              value: transfer.isText && transfer.outgoing ? null : transfer.progress,
              strokeWidth: 8,
              color: failed ? Colors.red : null,
            ),
          ),
          Column(mainAxisSize: MainAxisSize.min, children: [
            Text(transfer.code, style: Theme.of(context).textTheme.headlineSmall),
            const SizedBox(height: 4),
            Text(
              failed
                  ? transfer.failedReason!
                  : '${transfer.outgoing ? "发送" : "接收"} ${_fmt(transfer.bytes)}'
                    '${transfer.total > 0 ? " / ${_fmt(transfer.total)}" : ""}',
              style: const TextStyle(fontSize: 11, color: Colors.grey),
            ),
          ]),
        ]),
        const SizedBox(height: 16),
        if (failed)
          FilledButton.tonal(onPressed: onCancel, child: const Text('关闭'))
        else if (transfer.outgoing)
          FilledButton.tonal(onPressed: onCancel, child: const Text('中断发送'))
        else if (needsDecision && !transfer.isText)
          Row(mainAxisSize: MainAxisSize.min, children: [
            FilledButton(onPressed: () => onDecide(true), child: const Text('接收')),
            const SizedBox(width: 8),
            FilledButton.tonal(onPressed: () => onDecide(false), child: const Text('拒绝')),
          ])
        else
          FilledButton.tonal(onPressed: onCancel, child: const Text('取消')),
      ]),
    );
  }

  String _fmt(int bytes) {
    if (bytes < 1024) return '$bytes B';
    if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
    return '${(bytes / 1024 / 1024).toStringAsFixed(1)} MB';
  }
}
