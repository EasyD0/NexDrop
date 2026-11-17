# NexDrop — 代码规范手册

> 本文件面向人类开发者与 AI Agent，定义 NexDrop 的注释风格与协作约定。
> 原则：注释解释"为什么"，能用代码自解释的地方不写。

---

## 1. 语言约定

- 所有注释使用中文；代码标识符、类型名、枚举变体保持英文原文。
- 专有名词与 API 名保留英文写法，如 `TOFU`、`ALPN`。
- 注释说明意图与约束，不翻译代码本体。

---

## 2. 注释风格：rustdoc / dartdoc

本仓库是 Rust + Dart 双语言项目，按各自官方风格注释：

| 语言 | 风格 | 符号 |
|------|------|------|
| Rust | rustdoc | `///`（项）、`//!`（模块） |
| Dart | dartdoc | `///` |

---

## 3. Rust 注释规范

### 3.1 模块级 — `//!`

在模块文件顶部说明模块存在的目的与关键约束，一两句话。

```rust
//! Device identity: a self-signed TLS certificate created on first run,
//! reused forever; its SHA-256 fingerprint is the stable device key.
```

### 3.2 结构体 / 字段 / 函数 — `///`

```rust
/// 设备长期身份，以 DER 形式持久化在 settings 旁。
#[derive(Clone)]
pub struct Identity {
    pub cert_der: Vec<u8>,
    /// `cert_der` 的 SHA-256 十六进制值，用作设备 id 与 pin。
    pub fingerprint: String,
}

/// 从 `dir` 加载 `cert.der`/`key.der`，首次运行则自动创建。
pub fn load_or_create(dir: &std::path::Path) -> anyhow::Result<Self> { /* ... */ }
```

### 3.3 复杂函数：用 Markdown 章节

参数、返回、错误无法一句话说清时，用以下三段式：

```rust
/// 建立与远端设备的 TLS 会话。
///
/// # Arguments
///
/// * `device_id` — 远端设备指纹（TOFU 固定依据）
/// * `text` — 要发送的明文
///
/// # Returns
///
/// 成功传输的字节数。
///
/// # Errors
///
/// 设备离线、配对失败或底层 I/O 错误时返回 `anyhow::Error`。
pub async fn send_text(device_id: String, text: String) -> anyhow::Result<()> { /* ... */ }
```

> 语义显然时用一句话注释即可，不强制拆章节。

### 3.4 私有辅助 / 辅助 trait

短小、局部、语义分明时，用简短 `///` 说明用途：

```rust
/// proxy trait：让 `tx.send(...)` 的失败统一带上下文
trait ContextSend {
    fn context_send(self) -> anyhow::Result<()>;
}
```

---

## 4. Dart 注释规范

Dart 用 `///`，支持段落与 Markdown。

```dart
/// 取消当前正在进行的文件传输。
///
/// 由 Rust 核心经桥接层调用；传输不存在时静默返回。
Future<void> cancelTransfer() => RustLib.instance.api.crateApiCancelTransfer();
```

### 字段 / 类

```dart
/// 全局应用状态模型，驱动 UI 响应式刷新。
class AppModel extends ChangeNotifier {
  /// 是否已完成核心引导（Rust 桥接就绪）。
  bool ready = false;
}
```

---

## 5. 通用约定

1. 优先写文档注释（`///`），而非行内散装注释（`//`）。
2. 用 `# Arguments` / `# Returns` / `# Errors` 章节组织复杂函数的参数说明。
3. 一行注释不超过 100 字符，超长换行。
4. 注释需通过格式检查：Rust 用 `cargo fmt`，Dart 用 `dart format`。
5. 待办统一带编号或链接，如 `/// TODO(#42): 补充断点续传`。

---

## 6. 快速参考卡片

| 场景 | Rust | Dart |
|------|------|------|
| 模块说明 | `//! ...` | 类注释 `///` |
| 类型/函数 | `/// ...` | `/// ...` |
| 参数/返回/错误 | `# Arguments` / `# Returns` / `# Errors` | 列表 |
| 内联补充 | `//` | `//` |