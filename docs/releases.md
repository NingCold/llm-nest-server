# 发布

服务端独立发版，版本来自根 Cargo.toml。先完成单仓及 PostgreSQL 检查，再准备指定 Web 版本和 SHA-256 的构建目录，构建镜像并保存镜像摘要。构建步骤见[部署](../server/deploy/README.md)。

发布前更新 server/deploy/compatibility.json，记录服务端、Web、Agent、connector 的确切版本/提交及摘要，还有节点 v4 与 sealedV1。不能以 latest 或临时 CI artifact 作为长期依赖。保留上一组可运行产物；回退服务不默认回退数据库。

仓库尚未公开发布，手动工作流接收确切 Web 版本及其永久产物/清单 URL，核验后制作本仓镜像及校验和供检查，不自动公开；推送、标签、公开 Release 及部署需另行授权。许可证与第三方声明需在首次公开产物前由维护者明确。各仓只读获取依赖，不使用本仓 GITHUB_TOKEN 写入其他仓。
