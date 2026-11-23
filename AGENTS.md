# 服务端工作约定

## 协作与交付

先看 `git status --short`，保留用户修改。可按需使用子代理；主代理负责分工、验收和提交。日常改动在 `main` 本地提交；推送、发布和部署须另获授权，分别报告结果。

## 改动边界

- 工作区仅含 `server`、`connector`、`crates/social`。云端不依赖 Agent、模型、SQLite 或客户端；协议包不依赖服务端实现或数据库。
- 改公开接口或 DTO 时核对 Web、桌面和 Agent，不复制业务逻辑。各端锁定同一协议提交；验证镜像由主代理切换，临时路径不入库。
- 只在输入、权限、协议与事务边界校验，错误向调用方报告。任务开始和提交时均复核权限、来源、消息修订、节点绑定和材料版本。持久化成功后再更新内存、报告成功；已开始但结果未知的模型调用不自动重做。
- 保留协议 v4、sealedV1 和已有数据身份，加密失败不回退明文。[迁移](docs/migration.md)只读旧库，先备份、预览。已部署 SQL 与 SQLite fixture 保留原始字节及换行，SQL 以[校验清单](scripts/migration-checksums.json)为准。`0008_migration.sql` 的 SHA-256 为 `E7E0726E1BB5B5ED3F3349E0971E13C645C8AF0354F88793B9760C7A52074175`。

## 验证与维护

- 按[开发](docs/development.md)和[联调](docs/validation.md)指南使用隔离数据库、临时目录和模拟模型；真实模型调用须明确授权。联调显式传入程序和产物路径。Web 使用客户端发布的固定版本与摘要，不从兄弟仓取源码构建。
- 私有配置、密钥、会话、恢复材料和测试数据不入库，凭据不入日志。本仓维护 [API、权限、存储与密文契约](docs/design/README.md)和[部署](server/deploy/README.md)；按当前行为写文档。
- 清理或搬移前列全目标的绝对路径与属性（含隐藏项），不跟随链接或 junction。核对清单后逐文件处理，再移除空目录并复核；禁用递归删除和 `-Force`，备份不按日期清除。
- 每批编译后确认 Cargo/rustc 已退出，用[清理脚本](scripts/clean-build-artifacts.py)预览并清理无用产物，保留可复用缓存，不全量 `cargo clean`。Windows 测试复用 MSVC 的 `target/debug`。
