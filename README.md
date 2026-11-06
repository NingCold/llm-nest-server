# LLM-Nest Server

LLM-Nest 的云端服务与公开 Rust 协议包。云端负责账号、联系人、私聊与群权限、消息附件、任务调度和密文同步；独立 Agent 负责模型调用与 AI 内部资料。

当前用于**邀请制、仅授权内网的 Alpha 日常试用**，不对外开放。维护者生成邀请码，并通过私有测试收件箱协助受邀者完成邮箱验证。产品范围与三仓总进度以[客户端当前安排](https://github.com/NingCold/llm-nest/blob/main/docs/project/current.md)为准。

## 站点维护者

- [部署服务](server/deploy/README.md)：加载固定版本镜像，或接收指定版本与摘要的 Web 产物构建镜像。
- [服务配置](server/README.md)：环境变量、启动、邀请与邮件。
- [验证](docs/validation.md)、[发布](docs/releases.md)、[旧资料与加密迁移](docs/migration.md)。

PostgreSQL 与附件目录需成套备份。云端不持有模型 Key 或解密秘密；账号登录与邀请码不自动授予解密能力。Alpha 尚未完成独立安全审查。

## 外部 Agent 开发者

- [connector 接入指南](connector/README.md)：独立依赖、节点配对与最小示例。
- [HTTP/WSS API](docs/design/api.md)、[sealedV1 加密契约](docs/design/crypto-contract.md)、[共用数据类型](crates/social/README.md)。
- [开发与检查](docs/development.md)、[设计索引](docs/design/README.md)。

节点协议为 **v4**，加密封装为 **sealedV1**。消费者固定同一协议提交和锁文件；接入已有服务器无需构建服务端或准备数据库。自带模型执行器见[Agent 仓库](https://github.com/NingCold/llm-nest-agent)。

公开产物的固定版本与摘要见[发布指南](docs/releases.md)和[发布兼容清单](server/deploy/compatibility.json)；清单记录该次产物组合，不代表当前内网部署。

项目代码采用 [Apache-2.0](LICENSE)；第三方组件保留各自许可证和声明。
