# LLM-Nest Server

用 LLM Nest Server 搭建邀请制聊天站点，让网页、桌面和 Android 客户端访问同一份聊天记录。服务端管理账号和群成员，保存消息与附件，把 AI 的回复任务交给独立 Agent。开启端到端加密后，服务端同步密文，由可信客户端或节点解密。

当前试用站仅向授权内网开放，由维护者转交验证邮件；Alpha 尚待独立安全审查。[当前安排](https://github.com/NingCold/llm-nest/blob/main/docs/project/current.md)列出站点实际使用的版本。

## 运行一个站点

1. 按[部署指南](server/deploy/README.md)准备 Linux、Docker Compose、HTTPS 证书和私有配置，加载已核验的固定版本镜像。
2. 启动服务，检查 HTTPS 入口，按[邀请与测试邮件](server/deploy/README.md#邀请与测试邮件)发出第一份邀请。
3. 用受邀账号验证登录和收发消息，按[复验与备份](server/deploy/README.md#复验与备份)成套备份 PostgreSQL 与附件目录。

从源码运行见[服务配置](server/README.md)。镜像使用客户端发布的固定版本 Web 包；公开版本与产物摘要见[发布指南](docs/releases.md)和[兼容清单](server/deploy/compatibility.json)。

## 接入自己的 Agent

已有站点时，可用[官方 Agent](https://github.com/NingCold/llm-nest-agent)接入模型。开发自己的 Agent，可引用 Rust [connector](connector/README.md)，也可以直接实现 [HTTP/WSS API](docs/design/api.md)。

节点协议为 **v4**，加密封装为 **sealedV1**。Rust 集成固定协议提交并保留锁文件；参与加密聊天的节点还须按[加密契约](docs/design/crypto-contract.md)建立身份与设备信任。

[开发](docs/development.md) · [设计](docs/design/README.md) · [共用数据类型](crates/social/README.md) · [迁移](docs/migration.md)

代码采用 [Apache-2.0](LICENSE)，第三方组件保留各自许可证和声明。
