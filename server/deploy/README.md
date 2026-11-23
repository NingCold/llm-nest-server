# 部署云端服务

[服务说明](../README.md) · [验证指南](../../docs/validation.md)

这套 Compose 在一台 Linux 服务器运行 PostgreSQL、聊天服务和 HTTPS 入口。只有 HTTPS 的 8443 端口发布到指定 Tailscale 地址，数据库和应用端口留在容器网络内。本模板使用私有测试收件箱，不发送真实邮件；仅供授权内网 Alpha 试用，不是公网开放部署方案。

服务器需要 Docker Compose。Compose 只启动 `CLOUD_SERVER_IMAGE` 指定的本机镜像，不现场构建或拉取同名应用镜像；未设置变量或尚未加载镜像时直接报错。PostgreSQL、附件、私有环境和证书目录继续沿用。

## 使用发布镜像

下载 [0.1.2 Release](https://github.com/NingCold/llm-nest-server/releases/tag/v0.1.2) 的全部附件，在同一目录核对后加载镜像：

```sh
sha256sum --check SHA256SUMS
docker load --input llmn-server-image.tar.gz
docker image inspect llmn-server:v0.1.2
```

在私有 `cloud.env` 中设置 `CLOUD_SERVER_IMAGE=llmn-server:v0.1.2`，按下文启动。从 Alpha 升级前，备份数据库和附件，阅读[旧设备与恢复文件迁移](../../docs/legacy-device-recovery.md)。保留上一可回退镜像；升级不删除数据。

## 从源码准备镜像

客户端先构建 Web，将明确版本的静态产物和 SHA-256 清单交给服务端。Docker 不直接读取客户端目录，也不编译 Agent。在服务端仓库准备独立构建目录：

```sh
cargo about generate --manifest-path server/Cargo.toml --target x86_64-unknown-linux-gnu --locked --fail --config scripts/licenses/about.toml --output-file /path/to/THIRD-PARTY-NOTICES.txt scripts/licenses/notices.hbs
WEB_VERSION=0.1.2  # 换成待验 Web 包的确切版本
python scripts/prepare-cloud-context.py --third-party-notices /path/to/THIRD-PARTY-NOTICES.txt --web-artifact "/path/to/web-artifact/llmn-web-$WEB_VERSION.tar.gz" --web-manifest /path/to/web-artifact/web-manifest.json --web-version "$WEB_VERSION" --output /path/to/cloud-context
docker build --tag llmn-server:local-reviewed /path/to/cloud-context
```

使用 cargo-about 0.9.3 联网取得上游许可原文；发布工作流校验工具的官方 SHA-256。镜像的 `/usr/share/doc/llmn-server/` 包含项目 LICENSE、NOTICE 和 Rust 依赖声明，Web 声明随 Web 文件保留，Debian 运行库的版权文件在 `/usr/share/doc/`。

确认清单内的 Web 版本正是准备构建的版本；打包和准备上下文各用一个空目录。脚本核对版本、摘要和归档路径，并要求 Rust 声明及 Web 包根目录的 `LICENSE.txt`、`NOTICE.txt`、`THIRD_PARTY_NOTICES.txt` 均非空。

构建上下文保留 Web 清单，只从 `server` 和 `crates/social` 复制受 Git 跟踪的源码，并原样保留 Cargo.toml、Cargo.lock。Docker 构建时按锁文件获取独立的连接器及依赖。将上下文复制到构建机即可构建。部署时把 `CLOUD_SERVER_IMAGE` 设为实际镜像名，并记录服务端、Web 和 Agent 的固定版本与摘要。

## 准备目录与证书

以下路径和地址都是示例，替换为自己的测试目录与 Tailscale 地址：

```bash
mkdir -p /srv/llmn-test/private/tls /srv/llmn-test/data/{attachments,outbox}
chmod 700 /srv/llmn-test/private
sudo chown 10001:10001 /srv/llmn-test/data/{attachments,outbox}
sudo chmod 700 /srv/llmn-test/data/{attachments,outbox}
python3 scripts/cloud-test-certificate.py /srv/llmn-test/private/tls 100.64.0.10
```

脚本生成九十天有效的测试 CA 和三十天有效的服务证书。私钥留在服务器；将 `ca.crt` 提供给测试客户端，只在测试连接中信任它。重新运行脚本会生成新的证书，需要同步客户端使用的 CA。正式使用应换成客户端已经信任的证书。

在 `/srv/llmn-test/private/cloud.env` 填写以下内容，文件权限设为 `600`，不提交到仓库：

```dotenv
POSTGRES_PASSWORD=填写随机生成的十六进制密码
CLOUD_PUBLIC_URL=https://100.64.0.10:8443
CLOUD_DATA_DIR=/srv/llmn-test/data
CLOUD_CERT_DIR=/srv/llmn-test/private/tls
TAILSCALE_IP=100.64.0.10
CLOUD_SERVER_IMAGE=llmn-server:v0.1.2
```

将 `compose.yaml` 与 `nginx.conf` 放入独立部署目录（下例为 `/srv/llmn-test/deploy/`）。保留原私有环境文件和数据，确认 `CLOUD_SERVER_IMAGE` 对应的镜像已加载后启动；部署目录无需源码或构建上下文：

```bash
docker compose --env-file /srv/llmn-test/private/cloud.env -f /srv/llmn-test/deploy/compose.yaml up -d
curl --cacert /srv/llmn-test/private/tls/ca.crt https://100.64.0.10:8443/health
```

`health` 返回 `ok` 表示 HTTP 服务可达；账号、聊天与附件还需要下方的完整测试。检查 `docker compose ps` 和 `ss -lntp`，确认只有指定的内网地址发布 8443，未发布数据库端口。

## 邀请与测试邮件

```bash
docker compose --env-file /srv/llmn-test/private/cloud.env -f /srv/llmn-test/deploy/compose.yaml exec app cloud-server invite 7
docker compose --env-file /srv/llmn-test/private/cloud.env -f /srv/llmn-test/deploy/compose.yaml exec app cloud-server revoke-invite 邀请码
```

第一个命令返回邀请码，交给受邀人使用，也可以组成站点的 `/#/register?invite=邀请码` 链接。邮件写入 `data/outbox` 下的 JSON 文件，包含收件人和验证或重置链接。维护者在服务器上读取对应邮件，不把整个目录公开到网站。

后续切换 SMTP 时，在私有部署目录的 `compose.yaml` 中删除 app 的 `CLOUD_MAIL_OUTBOX` 环境项，不要只改为空字符串；只要该变量存在，服务就选择文件邮件。将 `CLOUD_SMTP_HOST`、`CLOUD_SMTP_FROM` 及所需的 `CLOUD_SMTP_PORT`、`CLOUD_SMTP_USERNAME`、`CLOUD_SMTP_PASSWORD` 显式加入 app 的 environment，通过私有 `cloud.env` 引用它们。设置用户名时必须同时提供密码；仅填写 `cloud.env` 不会自动把新变量传入容器。真实值只保存在权限为 `600` 的私有配置中，不写入仓库、命令行或日志。

用前文的 Compose 启动命令重新创建 app，以专用账号确认能收到注册验证和重置密码两种邮件。变量用途见[服务配置](../README.md)。

桌面测试连接可在启动进程时设置 `CLOUD_CA_CERT_FILE=/绝对路径/ca.crt`，无需关闭证书校验。网页使用浏览器自己的证书信任设置。客户端里填写与证书一致的 HTTPS 地址。

## 复验与备份

`scripts/cloud_acceptance.py` 适用于使用测试收件箱且允许明文的隔离站点。脚本创建三个测试账号，从两台设备检查邀请、验证、消息、附件、重复发送和权限隔离，最后重启应用容器核对数据。输出文件含测试登录资料，请存入忽略目录。已启用强制加密的站点按下节核对加密聊天，保留原加密设置。

```bash
python scripts/cloud_acceptance.py \
  --url https://100.64.0.10:8443 --ca /本机路径/ca.crt \
  --server-host 测试服务器SSH别名 --server-root /srv/llmn-test \
  --client-host 第二台客户端SSH别名 \
  --output target/cloud-work/accounts.private.json
```

脚本假定服务器目录下有 `deploy/compose.yaml`、`private/cloud.env` 和 `data/outbox`，SSH 用户可使用 `sudo -n` 运行测试容器命令。

备份时先停止应用写入，保留 PostgreSQL 导出和同一时刻的附件目录，再恢复应用。恢复时两者一起恢复，核对附件引用。不要用删除持久卷的方式更新服务；日常更新先加载已核验的新镜像、更新 `CLOUD_SERVER_IMAGE`，再重新创建应用容器。

## 切换为强制加密

先更新客户端、Agent 与服务端，保持 `CLOUD_REQUIRE_ENCRYPTION=false` 供现有记录转换。按[迁移说明](../../docs/migration.md#现有云端记录转为加密)完成一致备份、窗口转换、节点资料同步和独立恢复核对；转换工具不会自动建立用户信任，也不会自动清理旧备份。

确认日常账号已有可信设备和恢复文件，使用中的窗口与节点资料都已完成转换后，在私有 `cloud.env` 设置 `CLOUD_REQUIRE_ENCRYPTION=true`，重新创建应用容器。此时旧客户端不能发送明文，新窗口需先准备加密；明文管理转发、旧格式导入和同机明文补传也会被拒绝。升级或切换不会重新执行旧消息和约定。

该开关不改变旧数据，也不是迁移成功证明。切换前后都要以有权账号检查历史、附件、资料应用与新消息；设备使用结果与尚待验证项记录在[产品当前安排](https://github.com/NingCold/llm-nest/blob/main/docs/project/current.md)中。
