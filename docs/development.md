# 开发

需要 Rust 1.99.1、Python 3.12、Node 24.18.0 和 PostgreSQL 17。服务端与协议包在本仓即可构建，无需客户端或 Agent 源码。

```sh
cargo build --workspace --locked
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked
python scripts/check-boundaries.py
python scripts/test_migrate_local.py
python scripts/test_build_artifacts.py
python scripts/test_clean_build_artifacts.py
```

数据库测试设置 `CLOUD_TEST_DATABASE_URL`，仅指向可销毁的独立测试库。普通 Rust 测试通过不代表 PostgreSQL 链路已执行；详见[验证](validation.md)。

运行服务所需变量和创建邀请的方法见[服务说明](../server/README.md)。部署接收客户端发布的 Web 压缩包及版本摘要清单，见[部署说明](../server/deploy/README.md)。
