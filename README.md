# NexDrop（邻传）

局域网 P2P 文件与文本传输工具（类 LocalSend）。Flutter UI + Rust 核心，不经过云，TLS 加密直连。

- 初版功能规格：[a.md](a.md)
- 技术选型与二期规划（QUIC、Android）：[plan.md](plan.md)
- 代码规范：[AGENTS.md](AGENTS.md)

## 功能

- 自动发现：mDNS 扫描局域网内设备，侧栏展示在线/已配对列表
- 配对：发起方生成 6 位配对码 → 对方确认 → 双方持久化对方指纹（TOFU）
- 传文本：无需确认，直接送达
- 传文件：接收方可要求配对码确认，进度环展示，支持中断/取消
- 冲突处理：接收方正忙时，新请求直接失败

## 技术栈

| 层 | 技术 |
|----|------|
| 核心 | Rust + tokio，TCP + rustls（自签证书，指纹 = 设备 id） |
| 发现 | mdns-sd（`_nexdrop._tcp.local.`） |
| 协议 | 长度前缀 JSON 帧 + 1 MiB 分块文件流 |
| 桥接 | flutter_rust_bridge 2.x（`core/src/api` → `app/lib/src/rust`） |
| UI | Flutter Material 3，Windows 桌面双栏布局 |

## 目录结构

```
NexDrop/
├─ core/                    # Rust 核心（nexdrop-core，FRB 目标 crate）
│  ├─ src/
│  │  ├─ api/               # FRB 对外 API（version/start/pair/send_* 等）
│  │  ├─ crypto/            # 设备身份：自签证书 + 指纹，TLS 配置（TOFU 校验）
│  │  ├─ discovery/         # mDNS 注册与浏览，维护在线设备表
│  │  ├─ model/             # 线协议 Frame 枚举、ALPN、常量、帧读写
│  │  ├─ pairing/           # 发起配对握手，持久化已配对设备
│  │  ├─ settings/          # settings.json / pairs.json 持久化（原子写）
│  │  ├─ state.rs           # 全局单例 CORE：设备表、pending 决策、事件广播
│  │  ├─ transfer/          # 文本/文件收发、分块流、忙状态、取消
│  │  └─ frb_generated.rs   # FRB 生成代码（已 gitignore，重新生成见 scripts/gen_dart）
│  └─ Cargo.toml
├─ app/                     # Flutter 工程（windows）
│  ├─ lib/
│  │  ├─ main.dart          # 应用入口、引导、配对弹窗
│  │  ├─ app_model.dart     # 全局状态 ChangeNotifier，消费 Rust 事件流
│  │  └─ ui/
│  │     ├─ sidebar.dart        # 侧栏设备列表（重命名/删除/配对入口）
│  │     ├─ pairing_sheet.dart  # 配对弹层（选设备 → 显示配对码）
│  │     ├─ main_pane.dart      # 文件框 + 文本框 + 传输进度覆盖层
│  │     └─ settings_page.dart  # 设备名/下载路径/确认开关
│  ├─ lib/src/rust/         # FRB 生成的 Dart 绑定（已 gitignore）
│  └─ windows/CMakeLists.txt # 构建时自动 cargo 编译核心并打包 dll
├─ scripts/                 # 构建/测试/代码生成脚本（见下）
├─ AGENTS.md                # 注释与协作规范
├─ a.md / plan.md           # 功能规格 / 技术规划
└─ Cargo.toml               # workspace 根
```

## 核心代码介绍

### `core/src/state.rs` — 全局状态

`CORE` 单例（`OnceLock`）持有：mDNS 设备表、待决策的配对/文件请求（oneshot channel）、
全局 `busy` 标志（同一时刻只允许一条传输）、`broadcast` 事件通道。
Rust 侧通过 `events` 广播 `CoreEvent`（进度、配对请求、传入文本等），FRB `StreamSink` 桥接到 Dart。

### `core/src/crypto` — 设备身份与 TLS

首次运行生成自签证书（`cert.der`/`key.der` 持久化在数据目录），证书 SHA-256 指纹即设备 id。
TLS 客户端/服务端均使用危险模式跳过系统 CA，改为比对对端证书指纹与配对时钉住的记录（TOFU）。

### `core/src/model` — 线协议

`Frame` 枚举（serde JSON，`t`/`d` 标签）：`Hello`、`PairRequest/Accept/Reject`、`Text`、
`FileOffer/Accept/Reject`、`Done`、`Error`、`Cancel` 等，`read_frame`/`write_frame` 做长度前缀编解码。

### `core/src/transfer` — 传输

- `dial()`：拨号 + Hello 交换 + 指纹校验，返回对端信息
- `send_text` / `send_files`：先 offer，对方 accept 后按 1 MiB 分块流式传输，逐块上报进度
- `handle_incoming`（监听侧）：配对请求挂起等待 UI 决策；文件请求检查 `busy`，忙则回 `Busy`
- 取消：置 `cancelled` 标志，发送/接收循环检测后断开并清理 `.part` 半成品

### `app/lib/app_model.dart` — Flutter 状态

订阅 Rust 事件流并分发到 UI；配对请求弹窗、文件确认弹窗、进度环覆盖层均由此驱动。
FRB 的 u64 在 Dart 侧是 `BigInt`（如 `requestId`、`total`）。

## 脚本介绍

均在 `scripts/` 下，PowerShell 版本，任意工作目录可执行：

| 脚本 | 作用 |
|------|------|
| `gen_dart.ps1` | 在 `app/` 下运行 `flutter_rust_bridge_codegen generate`，把 `core/src/api` 的 Rust API 重新生成到 `app/lib/src/rust/`。修改 `api/` 或相关类型后必须重跑 |
| `build_windows.ps1` | `flutter build windows [debug|release]`（默认 debug）。CMake 会自动 cargo 编译核心并把 `nexdrop_core.dll` 拷到 exe 旁 |
| `test_core.ps1` | 运行 `cargo test -p nexdrop-core`；传参数 `clippy` 时额外执行 `cargo clippy -- -D warnings` |

## 构建与运行

```powershell
# 1. 生成 Dart 绑定（首次或修改 api 后）
scripts\gen_dart.ps1

# 2. 构建 Windows 应用（自动编译 Rust 核心）
scripts\build_windows.ps1

# 3. 运行
app\build\windows\x64\runner\debug\nexdrop.exe
```

> 首次启动时 Windows 防火墙会弹授权提示，需允许（mDNS 与 TCP 直连依赖）。
> 数据目录：`%APPDATA%/NexDrop`（身份证书、settings.json、pairs.json）。

## 测试

```powershell
scripts\test_core.ps1 clippy   # Rust 单测 + clippy
cd app && flutter analyze      # Dart 静态检查
```
