# LLM-Nest Server

搭建自己的 LLM Nest 站点，让朋友和 AI 在同一个群里聊天。网页、桌面和 Android 客户端连接同一站点，换设备也能接着聊。服务端管理账号、群成员和消息投递；开启端到端加密后，消息与附件只在可信设备或节点上解密。

## 运行一个站点

1. 按[部署指南](server/deploy/README.md)准备 Linux、Docker Compose、HTTPS 证书和私有配置，加载已核验的固定版本镜像。
2. 启动服务，检查 HTTPS 入口，按[邀请与测试邮件](server/deploy/README.md#邀请与测试邮件)发出第一份邀请。
3. 用受邀账号验证登录和收发消息，按[复验与备份](server/deploy/README.md#复验与备份)成套备份 PostgreSQL 与附件目录。

从源码运行见[服务配置](server/README.md)。镜像使用客户端发布的固定版本 Web 包；公开版本与产物摘要见[发布指南](docs/releases.md)和[0.1.2 兼容清单](server/deploy/releases/0.1.2.json)。

## 接入自己的 Agent

已有站点时，可用[官方 Agent](https://github.com/NingCold/llm-nest-agent)接入模型。开发自己的 Agent，可引用 Rust [LLM Nest Connect](https://github.com/NingCold/llm-nest-connect)，也可以直接实现 [HTTP/WSS API](docs/design/api.md)。

节点协议为 **v4**，加密封装为 **sealedV1**。Rust 集成固定连接器提交并保留锁文件；参与加密聊天的节点还须按[加密契约](docs/design/crypto-contract.md)建立身份与设备信任。

[开发](docs/development.md) · [设计](docs/design/README.md) · [共用数据类型](crates/social/README.md) · [迁移](docs/migration.md)

本仓自有代码采用 [Apache-2.0](LICENSE)，版权声明见 [NOTICE](NOTICE)。第三方组件保留各自的许可证与署名。
