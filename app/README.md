# app/ — Flutter 应用

这一个目录同时是 Android 和 Windows 的客户端，**不拆成两个工程**：

- `lib/` — 共享的 Dart UI 与状态（两端同一套）
- `android/` — Android runner 与 Kotlin 平台通道（前台服务、分享 Intent、剪贴板）
- `windows/` — Windows runner（托盘、自启）
- 网络/加密/文件逻辑全部在 `../core/`（Rust），由两者共同调用

## 尚未生成

当前机器未安装 Flutter SDK，所以 `pubspec.yaml` 和 `android/`、`windows/` 尚未生成（手写一份不可靠的 pubspec 反而会挡住生成器）。安装 Flutter 后执行：

```bash
flutter create --platforms=windows,android --org com.nexdrop --project-name nexdrop .
```

`lib/` 已经在仓库里，`flutter create` 会补齐其余文件。
