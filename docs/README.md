# NexDrop 文档索引

所有设计、平台与测试文档放在本目录。文档随实现更新，不追求一次写全。

## 目录

| 路径 | 内容 | 状态 |
|---|---|---|
| [architecture.md](architecture.md) | 架构、模块边界、数据流 | 待写（M0） |
| [protocol.md](protocol.md) | 消息格式、状态机、错误码、版本协商 | 待写（M1 前） |
| [ui.md](ui.md) | 页面结构、交互、设计规范 | 待写（M2） |
| [testing.md](testing.md) | 测试矩阵、真机清单、性能基线 | 待写（M2） |
| [release.md](release.md) | 打包、签名、版本号、发布检查表 | 待写（M5） |
| [platform/windows.md](platform/windows.md) | 托盘、自启、防火墙、安装包 | 待写（M1） |
| [platform/android.md](platform/android.md) | 权限、前台服务、分享入口、剪贴板 | 待写（M1 前） |
| [decisions/](decisions/) | 架构决策记录（ADR） | 持续 |

## 约定

- 文件名使用小写 kebab-case，如 `file-transfer.md`。
- 平台相关细节只写在 `platform/` 下，架构文档只引用、不重复。
- 每个 ADR 只记录一个决策，包含：背景、选项、决定、后果；已接受的 ADR 不修改，后续用新 ADR 取代。
- 文档中的命令必须可直接复制执行；路径使用相对仓库根目录的写法。
