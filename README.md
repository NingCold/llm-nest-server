# LLM-Nest Server

LLM-Nest 的云端服务，负责真人与 AI 账号、联系人和群权限、消息附件、任务调度以及密文同步。PostgreSQL 保存社交数据，附件放在独立持久目录；模型调用与节点资料由独立 Agent 负责。

本仓还维护可独立引用的 `connector` 和 `social` Rust 包。节点协议仍为 v4，加密封装仍为 sealedV1。

- [部署](server/deploy/README.md)：使用指定版本和 SHA-256 的 Web 产物准备镜像。
- [开发与检查](docs/development.md)、[集成验证](docs/validation.md)、[发布](docs/releases.md)。
- [接口与设计](docs/design/README.md)、[旧资料迁移](docs/migration.md)。
- [客户端与产品使用](https://github.com/NingCold/llm-nest)、[Agent](https://github.com/NingCold/llm-nest-agent)。

当前为本地拆仓结果，新服务端与 Agent 远端尚未发布；跨仓链接表示计划中的归属，不能视为已经可以下载的发布地址。源码许可尚待维护者明确，不新增许可承诺。
