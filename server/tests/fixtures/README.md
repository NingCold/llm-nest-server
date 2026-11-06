# 旧本地数据夹具

`local-human-payloads.json` 是旧本地 SQLite 所保存的 `social::Actor` 和 `social::SocialMessage` JSON 格式的合成样本，不是用户数据库或迁移包。PostgreSQL 测试用共享类型反序列化并核对字段映射，包括账号、窗口和消息 UUID、正文、发送者、时间、修订、附件原字节及相对顺序。

旧窗口内 sequence 按顺序转换为云端 delivery position，不直接复制到全局位置；原内容 part 索引与云端附件索引分别映射。该样本只覆盖真人私聊，不能代替完整导入测试。

`local-v13/` 保留旧 SQLite v13 的十三份 SQL schema，供只读迁移工具生成隔离旧库。夹具和已部署迁移均须保留原始字节，不做换行转换。

正式导入使用 `connector::migration` 版本 1 DTO，操作、范围和限制见[迁移指南](../../../docs/migration.md)，接口见[API](../../../docs/design/api.md#数据迁移接口)。
