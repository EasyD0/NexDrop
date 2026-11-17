# 开发环境记录

> 记录一次实际配置的结果，供新机器复现和排查使用。更新于 2026-11-17。

## 目标平台

- Windows 10/11（桌面端）
- Android 8.0+
- 技术栈：Flutter UI + Rust core（flutter_rust_bridge）

## 版本清单

| 组件 | 版本 | 位置 |
|---|---|---|
| Flutter | 3.50.0 stable | `C:\src\flutter` |
| Dart | 3.14.0 | 随 Flutter |
| Rust | 1.99.1 | rustup |
| Visual Studio | 2026 Community 18.5.11723.231 | `C:\Program Files\Microsoft Visual Studio\18\Community` |
| MSVC 工具集 | 14.50.35717 | 随 VS2026 |
| Windows SDK | 10.0.26100.0 | 随 VS2026 |
| Android SDK | platform 36.1 / build-tools 36.0.0 | `C:\Users\<user>\AppData\Local\Android\Sdk` |
| Android NDK | 30.0.16248370 | 同上 `ndk\30.0.16248370` |
| Android cmdline-tools | latest | 同上 `cmdline-tools\latest` |
| JDK | OpenJDK 21.0.10（Android Studio JBR） | `C:\Program Files\Android\Android Studio\jbr` |
| cargo-ndk | 4.1.2 | `~/.cargo/bin` |
| flutter_rust_bridge_codegen | 2.13.0 | `~/.cargo/bin` |

## 环境变量（用户级）

| 变量 | 值 |
|---|---|
| `JAVA_HOME` | `C:\Program Files\Android\Android Studio\jbr` |
| `PATH`（追加） | `C:\src\flutter\bin` |

`JAVA_HOME` 必须指向 JDK 21。系统里另装有 Oracle JDK 24，AGP/Gradle 对其支持滞后，不要使用。

## 安装步骤（复现用）

```bash
# 1. Flutter SDK：从官方 releases_windows.json 取 stable 版本，解压到 C:\src
curl -L -o C:\src\flutter_windows_3.50.0-stable.zip \
  https://storage.googleapis.com/flutter_infra_release/releases/stable/windows/flutter_windows_3.50.0-stable.zip
tar -xf C:\src\flutter_windows_3.50.0-stable.zip -C C:\src

# 2. Rust Android 目标
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android

# 3. Rust 工具
cargo install cargo-ndk
cargo install flutter_rust_bridge_codegen

# 4. 验证
flutter doctor -v
```

Android 侧组件（NDK、cmdline-tools）通过 Android Studio 的 SDK Manager 安装，之后可用 `sdkmanager` 精确指定版本。

### Rust target 命名说明

target 里带 `linux` 不代表需要 Linux 机器 —— Android 内核基于 Linux，Rust 的 triple 命名是
`<架构>-<内核>-<系统>`，所以 Android 目标统一写作 `*-linux-android`。

| target | 用途 | 必要性 |
|---|---|---|
| `aarch64-linux-android` | 64 位 ARM 真机 | 必需 |
| `armv7-linux-androideabi` | 32 位 ARM 老设备 | 可选 |
| `x86_64-linux-android` | x86_64 模拟器镜像 | 用模拟器时需要 |
| `i686-linux-android` | 32 位 x86 模拟器 | 基本淘汰，可删 |

每个 target 只是对应的 Rust 标准库，四个合计约几十 MB。

## 当前验证结果

`flutter doctor -v` 输出 **No issues found!**，逐项状态：

- Flutter 3.50.0 stable，Windows 11 (26H2)
- Android toolchain：SDK 36.0.0，licenses 全部接受
- Visual Studio 2026 18.5.2，Windows SDK 10.0.26100 被正确识别
- Connected devices：Windows desktop、Chrome、Edge

## 已知问题

### 1. adb 重复

`flutter doctor` 报告两个 adb：

- `C:\Users\<user>\AppData\Local\Android\Sdk\platform-tools\adb.exe`
- `D:\Program protable\platform-tools\adb.exe`

两者会导致设备检测不一致。建议从 PATH 中移除 D 盘那份，只保留 SDK 自带的。

### 2. NDK 版本可能不匹配

已安装 NDK **30.0.16248370**，新版 Flutter/Gradle 通常要求某一具体版本。若构建报 `ndkVersion` 不匹配，按提示补装：

```bash
sdkmanager "ndk;<required-version>"
```

### 3. VS2026 识别

Flutter 3.39 之前的版本不支持 VS2026（18.x），会误报 C++ 工具链缺失。本机 3.50.0 无此问题。若降级 Flutter，需要装 VS2022 Build Tools 并用 `CMAKE_GENERATOR=Visual Studio 17 2022` 回退。

## 相关文档

- 项目计划：[../plan.md](../plan.md)
- 文档索引：[README.md](README.md)
