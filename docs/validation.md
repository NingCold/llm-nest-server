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

2026-10-29，三个独立 checkout 的构建及 full/migration 两种组合验收通过；服务端 workspace 77 项测试通过，包含独立 PostgreSQL 上的云端 66 项。联调使用真实 Rust/WASM SDK 和模拟模型，两模式调用分别为 21、0 次；实际构建提交与产物摘要见 [当时的兼容组合](https://github.com/NingCold/llm-nest-server/blob/c4180d7/server/deploy/compatibility.json)。当时清单标为本地 debug 验证，不代表已部署或公开发布；后续文档和测试脚本提交没有改变其中的服务端运行源码。当时未覆盖 Docker/Linux 部署；当前镜像检查见下一节，日常客户端解锁验读和独立安全审查仍待补齐。

## 首次公开镜像与内网更新

2026-10-30，`alpha.12` 固定镜像搭配 Web `alpha.13`，已在 Debian 13 通过全新隔离库和既有备份实际还原后的验收。覆盖邀请注册、邮箱验证、登录退出、唯一私聊、账号权限、Web 和许可证文件，以及强制加密下拒绝新明文且不落库。还原副本以新镜像启动前后，15 张关键表指纹一致。临时库、容器和运行文件已按清单清理。

第一次切换发现 9 份迁移文件的换行被 Git 规范化，服务已回退，再从部署原件恢复与 SQLx 校验值一致的字节。SQL 内容未变，全部迁移现由 Git 属性和发布摘要检查保护，并补充换行转换回归。最终切换 `alpha.12` 后，15 张关键表与既有调用数保持，16 个附件已一致备份，原私有配置和强制加密设置沿用。应用和 HTTPS 入口均已改用独立部署目录，原代理镜像、TLS 和内网端口保持。实际组合见[当前兼容清单](../server/deploy/compatibility.json)。

本轮真实模型调用为 0；上述服务器验收不代表日常客户端已登录解锁或逐条读回历史，也不代替独立安全审查。客户端 Windows 已覆盖安装、Linux deb/AppImage 已做隔离启动；干净 Windows、MSI 实装及 GNOME 人工验收继续保留。
