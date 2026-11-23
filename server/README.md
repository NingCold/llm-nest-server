# 云端服务配置

[项目首页](../README.md) · [部署](deploy/README.md) · [开发](../docs/development.md) · [API](../docs/design/api.md)

`cloud-server` 使用 PostgreSQL 保存账号、认证、联系人、私聊与群、消息、读态、节点绑定、执行任务和事件；附件字节保存在独立持久目录。它不依赖客户端 Runtime、SQLite 或模型执行器。

## 本机启动

先准备独立 PostgreSQL 数据库和已核验的 Web 静态产物，设置下表的数据库、公开来源、附件与邮件配置，再启动：

```bash
export CLOUD_WEB_DIR=/absolute/path/to/verified-web
cargo run -p cloud-server --locked
```

启动前通过进程环境配置以下内容，不将真实值提交到仓库：

| 环境变量 | 用途 |
| --- | --- |
| `CLOUD_DATABASE_URL` | PostgreSQL 连接串；启动自动运行顺序迁移 |
| `CLOUD_PUBLIC_URL` | 页面公开来源，例如 https://chat.example，不含路径、查询、账号或片段；本机开发允许 localhost HTTP |
| `CLOUD_ATTACHMENTS_DIR` | 附件持久目录，与静态网页和测试收件箱分开 |
| `CLOUD_WEB_DIR` | 静态目录，默认 `web` |
| `CLOUD_BIND` | 监听地址，默认 `127.0.0.1:8787` |
| `CLOUD_REQUIRE_ENCRYPTION` | 拒绝明文写入，默认 `false`；启用前完成存量转换与恢复核对 |
| `CLOUD_SMTP_HOST`、`CLOUD_SMTP_FROM` | 邮件发送配置；可配 `CLOUD_SMTP_PORT`、`CLOUD_SMTP_USERNAME`、`CLOUD_SMTP_PASSWORD` |
| `CLOUD_MAIL_OUTBOX` | 绝对路径的私有测试收件箱；设置后使用文件邮件，不发送 SMTP |

管理员生成邀请：`cargo run -p cloud-server --locked -- invite 7`；撤销使用 `revoke-invite 邀请码`。命令只需要数据库连接。邀请码和邮件链接属于秘密，不应贴入公开日志。

## 存储与安全边界

部署入口为 [Dockerfile](Dockerfile) 和 [Compose](deploy/compose.yaml)。仅向授权内网提供 HTTPS；数据库与应用端口不直接公开，测试收件箱不得置于静态目录。

启用 sealedV1 后，聊天、附件和 AI 资料由授权端加解密；服务器保存密文、公开设备信息、密钥投递和加密恢复资料，不持有解密秘密。`CLOUD_REQUIRE_ENCRYPTION` 拒绝明文路径，不能代替旧记录转换或恢复核对。切换步骤见[部署说明](deploy/README.md#切换为强制加密)。

数据库与附件需配套持久化和备份，单窗口导出不能代替整库备份。旧 v13 导入、现有记录加密转换与更换节点见[迁移指南](../docs/migration.md)；权限、幂等和资料版本以 [API](../docs/design/api.md)与[加密契约](../docs/design/crypto-contract.md)为准。

## 验证

设置仅用于测试的 `CLOUD_TEST_DATABASE_URL` 后运行：

```bash
cargo test -p cloud-server --locked -- --nocapture
```

每例使用隔离 schema；未设置变量会跳过数据库测试，不能当作完整验证。[验证指南](../docs/validation.md)说明跨仓产物与联调条件；[本地数据夹具](tests/fixtures/README.md)用于旧 DTO 映射回归。
