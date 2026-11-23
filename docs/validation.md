# 验证

单仓检查见[开发](development.md)。PostgreSQL 测试需独立 `CLOUD_TEST_DATABASE_URL`；没有数据库的测试结果不能代替它。CI 还会运行 `python scripts/chat-smoke.py target/debug/cloud-server`：从真实服务进程注册、验证两个临时账号，检查双方打开同一个私聊、重试不重复发信、收件人读到消息。该脚本只用于可销毁的测试库，会写入账号和消息。

`cargo test -p cloud-server crypto_directory` 验证设备目录版本匹配、旧客户端兼容、未确认响应重试、同值更新、公钥和签名变化、撤销、绑定、状态变化、改回原值及事务回滚；设备名字、心跳、一次性密钥和收件箱更新不得推进版本。客户端另须验证密钥查询或 SDK 确认失败时不保存新版本，以及重启后复用已确认版本。

集成流程由本仓组织，使用临时账号、数据库、节点目录和模拟模型。Python cloud/agent 验收脚本通过 `--help` 查看参数；Agent 使用显式 `--binary`。证书验证保持开启，私有输出不得提交。

加密联调先运行 `npm ci --ignore-scripts --prefix tests/integration`，再运行 `node scripts/crypto_acceptance.cjs`。需要 psql 和以下环境变量：

| 变量 | 内容 |
| --- | --- |
| CLOUD_TEST_DATABASE_URL | 独立 PostgreSQL 测试连接 |
| LLMN_CLOUD_BINARY | 已构建的 cloud-server 程序绝对路径 |
| LLMN_AGENT_BINARY | 指定版本 llmn-agent 程序绝对路径 |
| LLMN_INTEROP_BINARY | 同版本 Agent Rust SDK interop 程序绝对路径 |
| LLMN_WEB_DIR | 解包后的指定版本 Web 产物目录 |
| LLMN_CLIENT_VERSION | 客户端测试产物的确切版本，匹配 manifest.json |
| LLMN_CLIENT_CRYPTO_FIXTURE | 客户端生成的预编译 crypto 测试产物目录 |

测试依赖由 tests/integration/package-lock.json 固定，使用官方 Matrix SDK。客户端通过 `node scripts/package-crypto-fixture.cjs OUTPUT` 打包测试模块，服务端检查 manifest.json 中的客户端版本和逐文件 SHA-256；服务端不读取 TypeScript 源码、不重复客户端迁移实现。版本、提交及 SHA-256 记录在实际联调清单中。

## 验证结果如何使用

本页说明复验方法；[历史记录](validation-records.md)保留已执行批次的证据。0.1.2 公开镜像及其验证范围见[兼容清单](../server/deploy/releases/0.1.2.json)，当前内网试用与尚待完成的受邀者自助验证见[产品当前安排](https://github.com/NingCold/llm-nest/blob/main/docs/project/current.md)。本页联调使用隔离库和模拟模型；日常安装、Linux 实机、真实客户端重启、受邀者完整使用流程及独立安全审查分别验收。站点复验与备份见[部署指南](../server/deploy/README.md#复验与备份)。
