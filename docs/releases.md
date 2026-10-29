# 发布

服务端独立发版，版本来自根 Cargo.toml。先完成单仓及 PostgreSQL 检查，再准备指定 Web 版本和 SHA-256 的构建目录，构建镜像并保存镜像摘要。构建步骤见[部署](../server/deploy/README.md)。

发布前更新 server/deploy/compatibility.json，记录服务端、Web、Agent、connector 的确切版本/提交及摘要，还有节点 v4 与 sealedV1。不能以 latest 或临时 CI artifact 作为长期依赖。保留上一组可运行产物；回退服务不默认回退数据库。

仓库尚未公开发布，手动工作流接收确切 Web 版本及其永久产物/清单 URL，核验后制作本仓镜像及校验和供检查，不自动公开；推送、标签、公开 Release 及部署需另行授权。许可证与第三方声明需在首次公开产物前由维护者明确。各仓只读获取依赖，不使用本仓 GITHUB_TOKEN 写入其他仓。

镜像同时交付根 Apache-2.0 LICENSE、NOTICE 和 cargo-about 0.9.3 按服务端 Linux 依赖生成的 THIRD-PARTY-NOTICES.txt。生成失败会阻止镜像准备；具体命令和镜像内路径见部署说明。Web 与 Debian 运行库保留各自声明。

发布工作流必须填写已存在的 `release_tag`（如 `v0.1.1-alpha.11`）。流程 checkout 该 tag，核对其提交与本仓版本，再保存镜像检查 artifact 并创建或更新同 tag 的草稿 Release。只有草稿上传作业拥有 contents:write；已公开 Release 会被拒绝覆盖，不自动创建 tag、公开或部署。Release 附件包含镜像、image/web manifests、第三方声明、LICENSE、NOTICE，以及覆盖这些文件的 SHA256SUMS。
