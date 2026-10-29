# 跨仓测试依赖

`npm ci --ignore-scripts --prefix tests/integration` 安装本仓集成脚本使用的官方 Matrix WASM SDK、签名、IndexedDB 和压缩依赖。确切版本保存在 package-lock.json，不借用客户端 node_modules。

Agent、Rust SDK interop 程序、Web 与客户端预编译 crypto 测试模块均由调用者显式提供。变量和检查范围见[验证说明](../../docs/validation.md)。
