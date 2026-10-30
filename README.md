# LLM-Nest Server

LLM-Nest 的云端服务，负责真人与 AI 账号、联系人和群权限、消息附件、任务调度以及密文同步。PostgreSQL 保存社交数据，附件放在独立持久目录；模型调用与节点资料由独立 Agent 负责。

本仓还维护可独立引用的 `connector` 和 `social` Rust 包。节点协议仍为 v4，加密封装仍为 sealedV1。

- [部署](server/deploy/README.md)：使用指定版本和 SHA-256 的 Web 产物准备镜像。
- [开发与检查](docs/development.md)、[集成验证](docs/validation.md)、[发布](docs/releases.md)。
- [接口与设计](docs/design/README.md)、[旧资料迁移](docs/migration.md)。
- [客户端与产品使用](https://github.com/NingCold/llm-nest)、[Agent](https://github.com/NingCold/llm-nest-agent)。

当前可下载 [0.1.1-alpha.12 镜像](https://github.com/NingCold/llm-nest-server/releases/tag/v0.1.1-alpha.12)，搭配 Web/桌面和 Agent `alpha.13`。确切文件摘要与验收范围见[兼容组合](server/deploy/compatibility.json)。这是供试用的 Alpha，尚未完成独立安全审查。

项目代码采用 [Apache-2.0](LICENSE) 许可证；第三方组件保留各自的许可证和声明。
