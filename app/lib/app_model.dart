import 'package:flutter/foundation.dart';
import 'src/rust/api.dart';
import 'src/rust/api.dart' as api;
import 'src/rust/frb_generated.dart';
import 'src/rust/settings.dart';
import 'src/rust/state.dart';

/// One active transfer, in either direction. The UI overlays the matching
/// box (file or text) with a progress ring whenever this is non-null for it.
class TransferUi {
  final bool outgoing;
  final bool isText;
  final String deviceName;
  final String code;
  final int total;
  int bytes;
  String? failedReason;

  TransferUi({
    required this.outgoing,
    required this.isText,
    required this.deviceName,
    required this.code,
    required this.total,
    this.bytes = 0,
  });

  double get progress => total > 0 ? (bytes / total).clamp(0.0, 1.0) : 0.0;
}

/// Pending incoming request that needs a user decision.
class PendingDecision {
  final BigInt requestId;
  /// Empty name = incoming *file* confirm; otherwise a pairing request.
  final String peerName;
  final String code;

  PendingDecision({required this.requestId, required this.peerName, required this.code});

  bool get isFileConfirm => peerName.isEmpty;
}

class PairingAttempt {
  final String deviceId;
  final String deviceName;
  String code;
  bool done;
  bool accepted;

  PairingAttempt({required this.deviceId, required this.deviceName, required this.code})
      : done = false,
        accepted = false;
}

/// App-wide state fed by the core event stream.
class AppModel extends ChangeNotifier {
  List<DeviceView> devices = [];
  Settings? settings;
  DeviceView? selectedDevice;

  TransferUi? fileTransfer;
  TransferUi? textTransfer;
  PendingDecision? pendingDecision;
  PairingAttempt? pairing;

  /// Incoming text buffered until TransferDone(text) flips it into the box.
  String _incomingTextBuffer = '';
  final incomingText = ValueNotifier<String?>(null);

  String? lastError;

  /// Public wrapper for the protected `notifyListeners`, for UI code that
  /// mutates public fields directly (e.g. closing the pairing sheet).
  void notifyExternal() => notifyListeners();

  DeviceView? deviceById(String id) {
    for (final d in devices) {
      if (d.id == id) return d;
    }
    return null;
  }

  Future<void> bootstrap() async {
    await RustLib.init();
    await api.start();
    settings = await api.getSettings();
    await _refreshDevices();
    subscribeEvents().listen(_onEvent);
  }

  Future<void> _refreshDevices() async {
    devices = await api.listDevices();
    // Keep selection fresh after list changes.
    if (selectedDevice != null) {
      selectedDevice = deviceById(selectedDevice!.id);
    }
    notifyListeners();
  }

  void _onEvent(CoreEvent event) {
    switch (event) {
      case CoreEvent_DevicesChanged():
        _refreshDevices();
      case CoreEvent_PairRequested(:final requestId, :final name, :final code):
        pendingDecision = PendingDecision(requestId: requestId, peerName: name, code: code);
      case CoreEvent_PairCompleted(:final deviceId, :final accepted):
        if (pairing?.deviceId == deviceId) {
          pairing!.done = true;
          pairing!.accepted = accepted;
        }
        if (!accepted) lastError = '对方拒绝了配对请求';
      case CoreEvent_TransferStart(
          :final outgoing, :final isText, :final deviceName, :final code, :final total
        ):
        final t = TransferUi(
          outgoing: outgoing, isText: isText, deviceName: deviceName, code: code, total: total.toInt(),
        );
        if (isText) { textTransfer = t; } else { fileTransfer = t; }
        if (!outgoing && isText) incomingText.value = null;
      case CoreEvent_TransferProgress(:final outgoing, :final isText, :final bytes):
        final target = isText ? textTransfer : fileTransfer;
        if (target != null) target.bytes = bytes.toInt();
      case CoreEvent_TransferDone(:final outgoing, :final isText):
        if (isText) {
          if (!outgoing) text = _incomingTextBuffer;
          textTransfer = null;
        } else {
          fileTransfer = null;
        }
      case CoreEvent_TransferFailed(:final outgoing, :final isText, :final reason):
        (isText ? textTransfer : fileTransfer)?.failedReason = reason;
      case CoreEvent_IncomingText(:final text):
        _incomingTextBuffer = text;
    }
    notifyListeners();
  }

  // -------------------------------------------------------------- actions

  Future<void> startPairing(DeviceView device) async {
    // The code returns once the peer-side dialog resolves; meanwhile the
    // progress dialog shows nothing until PairRequested/PairCompleted land.
    pairing = PairingAttempt(deviceId: device.id, deviceName: device.displayName, code: '······');
    notifyListeners();
    try {
      final code = await api.pair(deviceId: device.id);
      pairing?.code = code;
    } catch (e) {
      pairing = null;
      lastError = e.toString();
      notifyListeners();
      rethrow;
    }
    notifyListeners();
  }

  Future<void> decidePair(bool accept) async {
    final d = pendingDecision;
    if (d == null) return;
    pendingDecision = null;
    notifyListeners();
    await api.decidePair(requestId: d.requestId, accept: accept);
    await _refreshDevices();
  }

  Future<void> decideFile(bool accept) async {
    final d = pendingDecision;
    if (d == null) return;
    pendingDecision = null;
    notifyListeners();
    await api.decideFile(requestId: d.requestId, accept: accept);
  }

  Future<void> selectDevice(DeviceView? d) async {
    selectedDevice = d;
    notifyListeners();
  }

  Future<void> removeDevice(DeviceView d) async {
    await api.removeDevice(id: d.id);
    if (selectedDevice?.id == d.id) selectedDevice = null;
    await _refreshDevices();
  }

  Future<void> renameDevice(DeviceView d, String alias) async {
    await api.renameDevice(id: d.id, alias: alias);
    await _refreshDevices();
  }

  Future<void> saveSettings(Settings s) async {
    await api.setSettings(settings: s);
    settings = s;
    notifyListeners();
  }

  Future<void> sendText() async {
    if (selectedDevice == null) return;
    await api.sendText(deviceId: selectedDevice!.id, text: text);
  }

  Future<void> sendFiles(List<String> paths) async {
    if (selectedDevice == null) return;
    await api.sendFiles(deviceId: selectedDevice!.id, paths: paths);
  }

  Future<void> cancelTransfer() => cancelTransfer();

  // ---------------------------------------------------------- local state

  /// Text currently in the text box.
  String text = '';
  final chosenFiles = <String>[];

  void addFiles(List<String> paths) {
    chosenFiles.addAll(paths);
    notifyListeners();
  }

  void removeFile(String path) {
    chosenFiles.remove(path);
    notifyListeners();
  }

  void clearFiles() {
    chosenFiles.clear();
    notifyListeners();
  }

  void setText(String value) {
    text = value;
    notifyListeners();
  }
}
