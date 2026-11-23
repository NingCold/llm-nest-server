# 发布

服务端独立发版，版本来自根 Cargo.toml。先完成单仓及 PostgreSQL 检查，再准备指定 Web 版本和 SHA-256 的构建目录，构建镜像并保存镜像摘要。构建步骤见[部署](../server/deploy/README.md)。

发布前用草稿附件的摘要生成兼容清单，随 Release 提供；公开后将同一清单保存在 `server/deploy/releases/`，从 [compatibility.json](../server/deploy/compatibility.json) 的 `publishedRelease` 引用。清单固定各组件的提交、下载地址与 SHA-256，并标明节点协议和加密封装。保留上一组可运行产物；回退服务不默认回退数据库。

[0.1.2 稳定版](https://github.com/NingCold/llm-nest-server/releases/tag/v0.1.2)的镜像内含 Web 0.1.2；桌面、Agent、Android 源码及连接器见[同版兼容清单](../server/deploy/releases/0.1.2.json)。此前的 [alpha.12 镜像](https://github.com/NingCold/llm-nest-server/releases/tag/v0.1.1-alpha.12) 配套 Web alpha.13。[实际内网站点版本](https://github.com/NingCold/llm-nest/blob/main/docs/project/current.md#现在使用的是哪个版本)另行记录。

镜像随附项目 LICENSE、NOTICE 和 cargo-about 0.9.3 按 Linux 依赖生成的 THIRD-PARTY-NOTICES.txt。Web 声明随 Web 包保留，Debian 运行库声明位于镜像的 `/usr/share/doc/`。

候选镜像：在 **Build server image** 填写 `web_version` 和客户端 main 成功 CI 的 `web_ci_run`（run ID），留空 URL 和 tag。该流程校验来源、版本、SHA-256 与归档许可，将镜像作为 `server-image-review` 上传；不创建 tag 或 Release。

正式发布改用已公开 Web 包及清单的固定 HTTPS 地址，填写已存在的 `release_tag`，留空 `web_ci_run`。流程 checkout 该 tag，核对其提交与本仓版本，再保存镜像检查 artifact 并创建或更新同 tag 的草稿 Release。只有草稿上传作业拥有 contents:write；已公开 Release 会被拒绝覆盖，不自动创建 tag、公开或部署。Release 附件包含镜像、image/web manifests、第三方声明、LICENSE、NOTICE，以及覆盖这些文件的 SHA256SUMS。草稿核对完成后附兼容清单，并将它的 SHA-256 加入同一份 SHA256SUMS。

## 源码与协议包

server 和 social 采用 [Apache-2.0](../LICENSE)，共用本仓库的 [NOTICE](../NOTICE)。[连接器](https://github.com/NingCold/llm-nest-connect)在独立仓库以 Apache-2.0 分发，使用方固定 Git 提交。
