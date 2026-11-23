# 发布

服务端独立发版，版本来自根 Cargo.toml。先完成单仓及 PostgreSQL 检查，再准备指定 Web 版本和 SHA-256 的构建目录，构建镜像并保存镜像摘要。构建步骤见[部署](../server/deploy/README.md)。

发布前更新 server/deploy/compatibility.json，记录服务端、Web、Agent、connector 的确切版本/提交及摘要，还有节点 v4 与 sealedV1。不能以 latest 或临时 CI artifact 作为长期依赖。保留上一组可运行产物；回退服务不默认回退数据库。

已公开产物包括 [0.1.1-alpha.12](https://github.com/NingCold/llm-nest-server/releases/tag/v0.1.1-alpha.12)，使用 Web `alpha.13`。这是该次发布组合，不表示当前内网版本；当前试用状态见[产品当前安排](https://github.com/NingCold/llm-nest/blob/main/docs/project/current.md)。手动工作流接收确切 Web 版本及永久下载地址，生成可核对的镜像和草稿 Release；维护者检查产物和部署结果后再公开。工作流只读获取跨仓依赖。

镜像随附项目 LICENSE、NOTICE 和 cargo-about 0.9.3 按 Linux 依赖生成的 THIRD-PARTY-NOTICES.txt。Web 声明随 Web 包保留，Debian 运行库声明位于镜像的 `/usr/share/doc/`。

发布工作流必须填写已存在的 `release_tag`（如 `v0.1.1-alpha.12`）。流程 checkout 该 tag，核对其提交与本仓版本，再保存镜像检查 artifact 并创建或更新同 tag 的草稿 Release。只有草稿上传作业拥有 contents:write；已公开 Release 会被拒绝覆盖，不自动创建 tag、公开或部署。Release 附件包含镜像、image/web manifests、第三方声明、LICENSE、NOTICE，以及覆盖这些文件的 SHA256SUMS。

## 源码与协议包

server、connector 和 social 均采用 [Apache-2.0](../LICENSE)，共用仓库的 [NOTICE](../NOTICE)。connector 和 social 通过固定 Git 提交分发，许可文件随源码提供。
