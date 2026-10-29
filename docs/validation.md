# 验证

单仓检查见[开发](development.md)。PostgreSQL 测试需独立 `CLOUD_TEST_DATABASE_URL`；没有数据库的测试结果不能代替它。

集成流程由本仓组织，使用临时账号、数据库、节点目录和模拟模型。Python cloud/agent 验收脚本通过 `--help` 查看参数；Agent 使用显式 `--binary`。证书验证保持开启，私有输出不得提交。

加密联调先运行 `npm ci --prefix tests/integration`，再运行 `node scripts/crypto_acceptance.cjs`。需要 psql 和以下环境变量：

| 变量 | 内容 |
| --- | --- |
| CLOUD_TEST_DATABASE_URL | 独立 PostgreSQL 测试连接 |
| LLMN_CLOUD_BINARY | 已构建的 cloud-server 程序绝对路径 |
| LLMN_AGENT_BINARY | 指定版本 llmn-agent 程序绝对路径 |
| LLMN_INTEROP_BINARY | 同版本 Agent Rust SDK interop 程序绝对路径 |
| LLMN_WEB_DIR | 解包后的指定版本 Web 产物目录 |
| LLMN_CLIENT_VERSION | 客户端测试产物的确切版本，匹配 manifest.json |
| LLMN_CLIENT_CRYPTO_FIXTURE | 客户端生成的预编译 crypto 测试产物目录 |

测试依赖由 tests/integration/package-lock.json 固定，使用官方 Matrix SDK。客户端通过 `node scripts/package-crypto-fixture.cjs OUTPUT` 打包测试模块，服务端检查 manifest.json 中的客户端版本和逐文件 SHA-256；服务端不读取 TypeScript 源码、不重复客户端迁移实现。版本、提交及 SHA-256 记录在实际联调清单中。此类模拟模型检查不代表日常安装、Linux 实机、真实客户端重启或独立安全审查通过。

## 本地拆仓验收

2026-10-29，三个独立 checkout 的构建及 full/migration 两种组合验收通过；服务端 workspace 77 项测试通过，包含独立 PostgreSQL 上的云端 66 项。联调使用真实 Rust/WASM SDK 和模拟模型，两模式调用分别为 21、0 次；实际构建提交与产物摘要见 [兼容组合](../server/deploy/compatibility.json)。该清单标为本地 debug 验证，不代表已部署或公开发布；后续文档和测试脚本提交没有改变其中的服务端运行源码。Docker/Linux、日常客户端重启和独立安全审查尚未覆盖。
