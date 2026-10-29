# AGENTS.md — LLM-Nest Server

先读 `git status --short`，保留用户修改。普通改动在 main 完成；未经授权不推送、发布或部署。

本仓 workspace 只有 server、connector 和 crates/social。云端不依赖 Agent、模型、SQLite 或客户端；connector 与 social 不依赖服务端数据库和实现。公开协议由本仓维护，消费者固定同一 Git 提交和锁文件。接口或 DTO 变更同时核对 Web、桌面和 Agent 链路，不能复制业务实现。

权限、来源、消息修订、节点绑定和材料版本在开始及提交边界复核。已开始但结果不明的模型调用不得自动重做。加密失败不回退明文；保留 v4、sealedV1 和已有数据身份。迁移只读旧库、先备份预览；全部已部署 SQL 以 `scripts/migration-checksums.json` 核对原始字节，禁止换行转换。`server/migrations/0008_migration.sql` 精确字节不可改写，SHA-256 为 E7E0726E1BB5B5ED3F3349E0971E13C645C8AF0354F88793B9760C7A52074175。

只在外部输入、存储事务、权限和协议边界校验。内部直接使用成立的类型与不变量，不吞错、重复校验或添加假设性备用路径。持久状态成功后才能更新内存与显示成功。

验证见 docs/development.md 与 docs/validation.md。正常测试用隔离数据库、临时目录和模拟模型，真实模型调用需明确授权。集成脚本接收显式程序和产物路径，不读取兄弟仓源码。Web 包由客户端发布，服务端消费指定版本和摘要，不编译前端。

不得提交私有配置、密钥、会话、恢复材料或测试数据；不要打印凭据。已部署的迁移及 SQLite fixture 保持精确字节。文档按当前行为维护，用人话说明用途和操作，不写开发日志。产品总进度在客户端，模型及意识系统说明在 Agent；本仓维护 API、权限、存储、密文契约和部署。

清理或搬移前只读列出全部目标（含隐藏项、属性、解析后的绝对路径），不跟随链接或 junction，不边枚举边删除。禁止递归删除和 Remove-Item -Force；逐文件删除后逐个移除空目录并核验。每批编译后确认无并行 Cargo/rustc，再用 scripts/clean-build-artifacts.py 预览和整理，保留可复用缓存，不能全量 cargo clean。Windows 测试复用默认 MSVC target/debug。安装或资料备份不按日期自动清除。

本地提交、远端推送与发布分别报告。跨仓 Git 依赖固定 rev；本地验证镜像与正式地址的切换由主代理管理，不提交临时路径补丁到最终依赖清单。
