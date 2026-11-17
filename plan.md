# NexDrop（邻传）— 项目计划

> 跨平台局域网文件与文字传输工具
> 工作名：**NexDrop**（中文暂定“邻传”），正式发布前需做一次商标/包名/域名检查。
> 版本：v0.1（2026-11-17）
> 技术栈：**Flutter + Rust**（flutter_rust_bridge）

---

## 1. 项目定位

**一句话**：在同一局域网内，把文件和文字从一台设备快速、安全地送到另一台设备，不经过云、不需要账号。

目标平台（按优先级）：

1. Windows 10/11（桌面端，首发的接收/发送端）
2. Android 8.0+（手机端，首发的另一端）
3. macOS / Linux（第二阶段）
4. iOS（暂不承诺，受限于本地网络与后台限制）

核心场景：

- 手机 → 电脑：分享一条文字/链接，电脑直接粘贴或收到通知
- 手机 ↔ 电脑：拖拽/选择文件互传，显示进度、可取消、校验完整性
- 电脑 ↔ 电脑：同一局域网快速传文件
- 所有传输端到端加密，默认不经过任何服务器

非目标（首个版本不做）：

- 互联网远程传输 / NAT 穿透 / 中继服务器
- 云同步、账号体系、设备云备份
- iOS 首发
- 后台静默、无感知的 Android 剪贴板同步（系统限制，见第 7 节）
- 手机屏幕镜像、通知同步、短信等 Phone Link 类功能

---

## 2. 技术选型

| 层 | 选择 | 理由 |
|---|---|---|
| UI | Flutter（Material 3） | 一套代码覆盖 Windows/Android/macOS/Linux；现代 UI；生态成熟 |
| 核心 | Rust（stable） | 网络、加密、分块、校验性能好；内存安全；跨平台 |
| 桥接 | flutter_rust_bridge 2.x | Flutter ↔ Rust 类型安全调用，支持 async/stream |
| 异步运行时 | tokio | 成熟、生态完整 |
| 传输 | QUIC（quinn）优先，TCP+TLS 兜底 | 多路复用、低延迟、加密内置；UDP 被封时回退 |
| 发现 | mDNS/DNS-SD（mdns-sd） | 局域网零配置；二维码/手动 IP 兜底 |
| 加密 | rustls + 应用层配对 | 端到端；不自签 CA，不依赖公网 |
| 校验 | BLAKE3 | 快、并行、适合大文件 |
| 序列化 | serde + JSON/MessagePack | 控制消息小；调试友好 |
| 日志 | tracing + tracing-subscriber | 结构化日志，便于问题定位 |

为什么不选 C++/Qt：Android 集成仍需 Kotlin/JNI，部署链路重，授权复杂；不如 Flutter + Rust 的开发效率。
为什么不选纯 Rust GUI：手机端支持弱，输入法/无障碍/系统集成体验差。

---

## 3. 总体架构

```text
┌──────────────────────────────────────────────┐
│                Flutter UI                    │
│  Windows: 设备列表 / 拖拽 / 历史 / 设置       │
│  Android: 分享入口 / 设备列表 / 前台服务状态   │
└───────────────────┬──────────────────────────┘
                    │ flutter_rust_bridge
┌───────────────────▼──────────────────────────┐
│                Rust core                     │
│  ┌──────────┐ ┌──────────┐ ┌──────────────┐  │
│  │ Discovery│ │ Pairing  │ │ Transfer     │  │
│  │ mDNS/QR  │ │ QR/TOFU  │ │ QUIC streams │  │
│  └──────────┘ └──────────┘ └──────────────┘  │
│  ┌──────────┐ ┌──────────┐ ┌──────────────┐  │
│  │ Crypto   │ │ File I/O │ │ History/DB   │  │
│  │ rustls   │ │ chunked  │ │ SQLite       │  │
│  └──────────┘ └──────────┘ └──────────────┘  │
└──────────────────────────────────────────────┘
```

平台胶水（不可避免，但很小）：

- Windows：Flutter 桌面 runner、托盘、开机自启、防火墙提示
- Android：Kotlin 平台通道（前台服务、分享 Intent、剪贴板、通知、MulticastLock）

---

## 4. 仓库结构

```text
NexDrop/
├── plan.md                  # 本文件
├── README.md                # 项目简介与快速开始
├── docs/                    # 所有设计/平台/测试文档（见第 10 节）
│   ├── README.md            # 文档索引
│   ├── architecture.md
│   ├── protocol.md
│   ├── ui.md
│   ├── testing.md
│   ├── release.md
│   ├── platform/
│   │   ├── windows.md
│   │   └── android.md
│   └── decisions/
│       └── ADR-0001-flutter-rust.md
├── app/                     # Flutter 应用
│   ├── lib/
│   ├── windows/
│   └── android/
├── core/                    # Rust crate（cdylib + staticlib）
│   ├── src/
│   │   ├── lib.rs
│   │   ├── discovery/
│   │   ├── pairing/
│   │   ├── transfer/
│   │   ├── crypto/
│   │   └── model/
│   └── Cargo.toml
├── rust_builder/            # flutter_rust_bridge 生成配置（如使用模板）
└── scripts/                 # 构建、生成、发布脚本
```

文档路径约定：

- 设计文档：`docs/*.md`
- 平台细节：`docs/platform/*.md`
- 架构决策记录：`docs/decisions/ADR-XXXX-*.md`（一事一议，不修改历史）
- 测试记录：`docs/testing.md` + `docs/testing/`（后续可加）
- 发布说明：`docs/release.md`

---

## 5. 核心模块设计

### 5.1 设备发现

- mDNS 服务类型：`_nexdrop._tcp.local`
- TXT 记录：`id`（设备 ID）、`name`（显示名）、`platform`、`proto`（协议版本）、`fp`（公钥指纹前 8 字节）
- 监听与发布都要做；Windows 防火墙需放行专用网络
- 兜底：手动输入 IP:port；二维码包含 IP + 公钥 + 一次性 token
- Android 需要 `MulticastLock`；Android 16 需处理本地网络权限（见 platform/android.md）

### 5.2 配对与安全

- 首次配对：一端显示二维码，另一端扫描；二维码含设备 ID、公钥、一次性 token
- 双向确认：两端都显示对方名称与指纹短码，用户确认
- 之后：TOFU（首次信任）保存对方公钥，后续自动连接
- 密钥存储：Windows DPAPI/Credential Manager；Android Keystore
- 传输加密：QUIC + TLS 1.3，ALPN `nexdrop/1`
- 不依赖公网、不上传任何内容；配对 token 一次性、带过期时间

### 5.3 传输协议

- 控制通道：QUIC 双向流，长度前缀消息（JSON 或 MessagePack）
- 消息类型：`hello`、`pair_request`、`pair_result`、`text`、`file_offer`、`file_accept`、`file_chunk`、`file_complete`、`cancel`、`ping`
- 文件：1 MiB 分块，流式读取，边传边算 BLAKE3；完成后校验根哈希
- 断点续传（M3 可选）：接收端记录 offset，重连后从 offset 继续
- 背压：QUIC 流控 + 有界通道，避免内存暴涨
- 超时/重试：连接超时、空闲超时、可取消；错误码统一

### 5.4 文字与剪贴板

- 文字消息走控制通道，UTF-8，限制大小（如 1 MiB）
- 电脑端收到后：写系统剪贴板 + 通知（可配置）
- 手机端收到后：写剪贴板（前台/通知点击时），并保留历史
- 手机 → 电脑的主要入口：
  1. Android 分享菜单 `ACTION_SEND`（`text/plain`）——首选，最稳
  2. App 内“发送剪贴板”按钮（App 在前台时读取）
  3. 快捷设置磁贴/通知动作（后续）
- 电脑 → 手机：电脑端复制后手动发送，或监听剪贴板变化后发送（需可关闭）

### 5.5 文件传输

- 入口：拖拽（Windows）、文件选择器、Android 分享 `ACTION_SEND`/`ACTION_SEND_MULTIPLE`
- 接收目录可配置；重名策略：覆盖/重命名/跳过
- UI：进度、速度、剩余时间、取消、失败重试
- 大文件：流式，不整文件读入内存；临时文件 + 完成后原子重命名

### 5.6 UI

- Material 3，深/浅色，中文/英文
- 首页：附近设备卡片、拖拽区、最近传输
- 传输页：设备名、方向、进度、速度、取消
- 设置：设备名、接收目录、剪贴板开关、自动接受、开机自启、主题
- Windows：托盘图标、最小化到托盘、单实例
- Android：前台服务常驻通知（传输中）、分享入口、电池优化引导

---

## 6. 里程碑

### M0 — 脚手架（约 1 周）

- [ ] 创建 Flutter 工程（Windows + Android）
- [ ] 创建 `core` Rust crate，导出 `version()` 与 `ping()` 给 Flutter
- [ ] flutter_rust_bridge 代码生成跑通
- [ ] CI：`cargo fmt/clippy/test` + `flutter analyze/test`
- [ ] 日志与错误上报骨架（本地文件）
- 验收：Windows 和 Android 都能显示 Rust 返回的版本号

### M1 — 发现与配对（约 2 周）

- [ ] mDNS 发布/浏览，设备列表实时更新
- [ ] 二维码配对 + 双向确认 + TOFU 存储
- [ ] QUIC 连接建立与断开
- [ ] Windows 防火墙说明与检测
- 验收：同一 Wi-Fi 下，手机与电脑互相发现、配对、重连无需再次确认

### M2 — 文字传输（约 2 周）

- [ ] Android `ACTION_SEND` 分享入口
- [ ] 控制通道发送/接收文字
- [ ] 电脑端写剪贴板 + 通知
- [ ] 手机端写剪贴板（前台/通知点击）
- [ ] 历史记录（SQLite）
- 验收：手机选中文字分享到电脑，电脑 1 秒内可粘贴；电脑发文字到手机可收到

### M3 — 文件传输（约 3 周）

- [ ] Windows 拖拽 + 文件选择器；Android 分享/选择文件
- [ ] 分块传输、进度、速度、取消
- [ ] BLAKE3 校验、失败重试
- [ ] 重名策略、接收目录设置
- [ ] （可选）断点续传
- 验收：1 GB 文件传输完成且哈希一致；中途取消不残留半成品

### M4 — 体验完善（约 2 周）

- [ ] 托盘、开机自启、单实例
- [ ] Android 前台服务与通知、电池优化引导
- [ ] 主题、国际化、空状态/错误状态
- [ ] 设置持久化、传输历史清理
- 验收：日常使用无需命令行；异常有明确提示

### M5 — 发布（约 2 周）

- [ ] Windows 安装包（MSIX 或 Inno Setup），代码签名（可选）
- [ ] Android APK/AAB，签名与隐私说明
- [ ] 端到端测试清单与回归
- [ ] 文档：安装、防火墙、常见问题
- 验收：干净机器安装后 3 分钟内完成首次传输

---

## 7. Android 平台约束（必须提前设计）

1. **剪贴板读取限制**：Android 10+ 只有前台应用或默认输入法能读剪贴板。不要设计“后台自动同步手机剪贴板”；用 `ACTION_SEND`、前台按钮、快捷磁贴解决。
2. **前台服务**：传输期间必须使用 foreground service；Android 14+ 声明 `foregroundServiceType="dataSync"`，Android 13+ 申请 `POST_NOTIFICATIONS`。
3. **本地网络**：mDNS 需要 `MulticastLock`；Android 16 起注意本地网络访问权限/系统弹窗，首次连接要引导用户允许。
4. **电池优化**：引导用户把 App 加入“不限制”，否则传输/发现会被杀。
5. **存储**：使用 SAF（`ACTION_OPEN_DOCUMENT` / `ACTION_CREATE_DOCUMENT`）处理接收目录，避免 MANAGE_EXTERNAL_STORAGE。
6. **分享入口**：`ACTION_SEND`（text/plain、*/*）与 `ACTION_SEND_MULTIPLE`，冷启动也要能接住。

以上细节写入 `docs/platform/android.md`，实现前先评审。

---

## 8. 测试策略

- Rust：单元测试（协议编解码、分块、哈希、配对状态机）+ 集成测试（本地回环传输）
- Flutter：widget 测试（设备列表、传输状态）+ 关键流程手工测试
- 端到端：Windows ↔ Android 真机；手机热点、路由器 AP 隔离、防火墙开启等场景
- 性能基线：1 GB 文件在千兆局域网 ≥ 80 MB/s；文字消息端到端 < 1 s
- 异常：断网、切 Wi-Fi、锁屏、后台、取消、磁盘满、重名

---

## 9. 风险清单

| 风险 | 影响 | 应对 |
|---|---|---|
| Android 后台剪贴板限制 | 无法做“无感同步” | 产品改为分享入口/前台触发，文档说明 |
| 路由器开启 AP 隔离/多播过滤 | 发现失败 | 提供手动 IP/二维码配对 |
| Windows 防火墙拦截 | 连接失败 | 首次引导放行专用网络；检测并提示 |
| QUIC/UDP 被封 | 传输失败 | M3 增加 TCP+TLS 兜底 |
| Flutter 桌面插件缺口 | 功能受限 | 关键能力放 Rust core，平台通道只做薄封装 |
| 代码签名/商店审核 | 发布延迟 | 先内测侧载，签名后置 |
| 名称/商标冲突 | 发布风险 | 发布前查商标、域名、包名 |

---

## 10. 文档计划

| 路径 | 内容 | 何时写 |
|---|---|---|
| `docs/README.md` | 文档索引与约定 | M0 |
| `docs/architecture.md` | 架构、模块边界、数据流 | M0 |
| `docs/protocol.md` | 消息格式、状态机、错误码、版本协商 | M1 前 |
| `docs/platform/windows.md` | 托盘、自启、防火墙、安装包 | M1 |
| `docs/platform/android.md` | 权限、前台服务、分享入口、剪贴板 | M1 前 |
| `docs/ui.md` | 页面结构、交互、设计规范 | M2 |
| `docs/testing.md` | 测试矩阵、真机清单、性能基线 | M2 |
| `docs/release.md` | 打包、签名、版本号、发布检查表 | M5 |
| `docs/decisions/ADR-*.md` | 架构决策记录 | 每做一次不可逆选择 |

---

## 11. 开放问题

- [ ] 正式名称与商标检查（NexDrop 为工作名）
- [ ] 是否支持 macOS/Linux 首发（建议 M4 后）
- [ ] 手机端是否要做“电脑剪贴板自动写入手机”（需前台/通知点击，体验取舍）
- [ ] 是否需要断点续传（大文件场景）
- [ ] 是否开源、许可证选择（MIT/Apache-2.0 或 GPL）
- [ ] Windows 安装包形式（MSIX vs Inno Setup）

---

## 12. 第一步

```bash
# 1. 创建 Flutter 工程（Windows + Android）
flutter create --platforms=windows,android app

# 2. 创建 Rust core
cargo new --lib core

# 3. 安装并初始化 flutter_rust_bridge
cargo install flutter_rust_bridge_codegen
flutter_rust_bridge_codegen integrate

# 4. 跑通 M0 验收
flutter run -d windows
flutter run -d <android-device>
```

先完成 M0，再按 M1 → M2 的顺序推进；M2 的文字分享是最小可用价值，建议优先打通。
