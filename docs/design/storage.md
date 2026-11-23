# 存储与迁移

[总纲](https://github.com/NingCold/llm-nest/blob/main/docs/design/overview.md) · [聊天与消息](conversations.md) · [API 与事件](api.md)

私聊和群聊可显式启用 sealedV1：新消息以密文封装保存，角色、模型、记忆和每 AI 摘要以分范围密文资源 CAS 同步；节点仍保存解密后的执行资料。设备公开密钥、Olm 信箱、恢复密文及版本元数据保存在云端，SDK 秘密与恢复解密材料不上传。旧未启用窗口和未转换历史仍保存明文。历史转换通过独立清单与指纹回执保留原记录标识，附件先写新密文目录再切换索引，旧附件留作受限备份；完成后 `crypto_history_complete` 不再允许旧位置明文。接口见[加密契约](crypto-contract.md)，联调见[当前安排](https://github.com/NingCold/llm-nest/blob/main/docs/project/current.md)。

## 当前云端存储

`server` 直接使用 PostgreSQL，不经过旧 SocialStore。`0001_accounts.sql` 保存 accounts、credentials、邀请、待验证注册、会话、密码重置和认证限流；`0002_chat.sql` 保存 cloud_contacts、cloud_conversations、cloud_list、cloud_messages、cloud_receipts、cloud_events。`0003_nodes.sql` 增加 AI 管理员、节点绑定、配对、任务及节点事件；`0004_groups.sql` 增加真实群与成员、结构化提及/回复、多 AI 任务回执、联系人屏蔽及账号启停。启动自动运行顺序迁移，不改写旧迁移。

双方 UUID 有序配对和唯一约束保证唯一私聊；群的 kind 为 group，first_id/second_id 均为 null，以 name 和 cloud_members 表达真实群，不使用 AI 账号占位。账号、窗口和消息 ID 均可沿用稳定 UUID。消息 position 是全局交付顺序，个人 read_position 不回退；消息发送、编辑/撤回、列表修改与相关账号的持久事件及回执在同一事务提交。写事务在分配事件 ID 前取得同一 advisory lock，保证可重放游标的提交顺序。init 和历史页的消息/读位置使用一致快照。

`cloud_members(conversation,account_id,role)` 是私聊与群的当前成员授权表，role 为 owner/admin/member；0004 为既有私聊补入双方 member。每群至多一位 owner 的部分唯一索引配合群管理事务保持群主不变量。成员变化提升窗口 revision，保留已提交聊天，取消未完成任务及其失效派生片段，并向旧成员和新成员提交事件；移出成员后每次读取均检查当前成员表。

0004 为 accounts 增加 status/status_revision，为 cloud_contacts 增加 listed/blocked/revision；通讯录列出与私聊屏蔽独立，移除联系人保留条目。cloud_messages 增加 sender_name（既有消息按发送者账号补齐）、reply_to、mentions UUID 数组与 ai_responses JSONB 数组；旧 ai_response 合并 aiId 迁为数组后删除旧列。历史署名不要求发送者仍是成员，mentions 保留明确选择的顺序，reply_to 由业务层复核同窗 committed 来源。ai_responses 按对应任务的 queue_position 重建，不将多个 AI 的状态覆盖成一个值。

cloud_node_tasks 新增 queue_position（唯一 bigserial）与 conversation_revision；窗口未完成任务的索引按 `(conversation,queue_position)` 支持跨节点 FIFO。材料选择限定于触发位置以内原文，另允许本窗更早 queue_position 的 completed 任务输出，且该任务的触发位置不得晚于当前触发；排除后续无关真人输入，最近 100 条之外仍补足必要触发/回复来源。开始事务复核队首、成员/账号/屏蔽、窗口 revision 与材料来源，持久保存 sources 后，后续 delta/result 逐条检查冻结来源，不重新选最近历史。`cloud_group_receipts` 按实际 owner/request 保存群与关系命令的 fingerprint 和窗口引用；不保存命令正文。AI 启停仍使用账号状态版本，取消只影响目标 AI 的未完成任务；屏蔽只影响对应私聊，二者保留有效片段为 interrupted，停用期间仍保存真人私聊消息但不为目标 AI 排任务，恢复/解除不重排。

附件元数据在消息 JSONB，原字节位于独立受管理目录的 `{messageId}/{attachmentIndex}`。先校验、独占创建文件、同步落盘，成功后提交数据库引用；数据库失败可能留下不可见的无引用文件，当前没有自动孤儿清理器。正文编辑保留原字节，撤回事务清空正文及元数据后删除文件。下载持有消息共享锁直至原字节读取完成；导出在一致快照中锁定完整消息及读取所选附件，避免撤回与读取拼出不一致记录。备份操作见[提交与备份边界](#提交与备份边界)。

cloud_receipts 只保存命令 SHA256 与实体 ID，不重复保存正文或 base64；重复请求返回当前权威状态，旧回执不能恢复撤回内容。密码使用 Argon2 哈希，邀请、验证/重置链接、会话只存校验值和有效性信息。SMTP 秘密属于部署环境。云端保存 AI 社交任务、来源修订、开始/结果指纹与确认 ID、累计草稿及终态、用量报告和节点事件；配对/绑定只保存节点 secret 的哈希。云端不保存模型 Key；启用加密后的 persona、模型参数、记忆和摘要以密文资源保存，未启用加密时只经内存转发。

[本地 payload 映射探针](../../server/tests/fixtures/README.md) 用合成数据和真实共享类型核对 UUID、公开资料、消息相对顺序及图片/文件原字节。旧 per-window sequence 需映射到云端 global position，旧正文 part index 需映射到云端附件索引。该探针只检查样本映射；正式迁移工具负责备份、预览、云端导入及节点资料包，范围见下文及[迁移指南](../migration.md)。

`0005_consciousness.sql` 在云端增加群话题引用及其来源索引、按稳定 AI 账号补齐的来源变更、节点材料版本和独立摘要调用确认。正式消息只保存 `context_reference` 的 ID、版本和失效标记，不复制群原文；摘要调用表只有来源、配置版本、状态和用量。在线管理请求与响应只经内存转发，不写入上述表或日志。

`0006_participation.sql` 增加云端窗口规则与状态、每日自动次数、一次性约定及其授权设置。社交任务区分消息和约定触发，保存合并输入、最早执行时间及规则版本。真人消息、任务与事件同事务；开始确认与自动次数同事务；成功结果、约定终态及后继接话同事务。约定命令回执只保存指纹和目标 ID，授权正文保存在意图记录中。

## 云端管理与桌面托管存储

`0007_management.sql` 为 accounts 增加 profile_revision、administrators_revision，分别保护公开资料与管理员名单修改；cloud_ai_bindings.capabilities 保存当前节点声明。在线档案正文不写入这些字段，能力声明不能代替当前在线连接与管理员授权复核。云端管理回执保存命令指纹和目标标识，重试返回当前结果。

桌面托管目录为 `desktop-agents/<serverOriginSha256>/<humanUuid>`，配置包含本机秘密，不进入业务导出；目录与进程管理由[客户端界面与桌面设计](https://github.com/NingCold/llm-nest/blob/main/docs/design/frontend.md)维护；在线管理事务与请求回放见 [API](api.md#云端账号管理与只读查看)。

## 当前节点存储

自带 Agent 使用独立 `node.sqlite`，按 AI 保存角色、模型参数、记忆、摘要、来源版本、执行检查点和预算；节点凭据留在私有目录。模型 Key 仅从环境引用解析。外部 Agent 自选存储，不必采用 SQLite。

节点开始意图与预算预留同事务保存。只有当前进程收到开始接口明确的 409 `task_not_ready` / `conversation_busy` 未开始响应，才能撤销本次 intent 与调用数/token 预留，并保持日期单调；已开始、确认丢失或重启后的不确定调用不退还、不重做。执行恢复见[Agent 执行设计](https://github.com/NingCold/llm-nest-agent/blob/main/docs/design/execution.md#当前云端私聊与群聊执行)。

## 同机文字私聊存储

`0009_direct.sql` 为 PostgreSQL 增加 `cloud_direct_grants`、`cloud_direct_receipts` 和 `cloud_direct_runs`。授权只保存 token 哈希，并绑定真人、AI、窗口、nodeId 与绑定版本；接纳回执只保存请求指纹和消息 ID，运行记录保存配置版本、用量与错误。已接纳正文仍进入原有 `cloud_messages`、来源索引和事件事务，不另建第二套云端聊天历史，也不为补传建立新的模型任务。

自带 Agent 的 `node.sqlite` 增加 `direct_windows` 和 `direct_turns`。前者保存获授权真人/AI/窗口、节点 grant、最近最多 64 条云端文字原文、末条 base、同步状态和错误；它是同机窗口的缓存，不代表完整历史。后者保存固定请求 ID、内容指纹、真人与 AI 消息 UUID/正文、排队与执行终态，以及实际使用的来源、材料版本、档案版本、用量和错误。以上内容保存在受限节点目录，没有加密或安全擦除承诺，不进入现有 AI 资料迁移包。

Send 先在 SQLite 事务保存真人消息和 queued 回合，再返回确认。模型调用使用共同的 `model_reservations`、节点整体次数/token 预算与并发槽；本机调用没有伪造的云端 start 确认。同一窗口的本机与云端执行共用串行控制。开始前持久记录 running 和执行材料，生成期间保存片段；停止或权限变化的中断状态不能被迟到结果覆盖。启动将遗留 running 标为 interrupted，不重复模型调用；尚未开始的 queued 可继续领取。

窗口同步状态分别为 pending、synced、conflict。每次只补传一个已结束回合；首次发送前把完整同步命令保存为 `direct_windows.sync_json`，网络结果不明时原样重试。确认后更新云端缓存和 head，并把该回合标为已同步；仍有后续本机回合时继续 pending。网络失败保留正文与待同步状态；当前权限、绑定、来源或云端历史不符则记录冲突并停止对应未完成回合，不丢弃本机记录或自动换节点。

同机路线的待确认发送保留 `route:"local"`、原 requestId 与完整命令，不能在刷新后改发云端。桌面从原生登录会话确定操作者，退出、401 或切服清除内存身份；重启须联网核对会话，不提供离线重新登录。

`run-managed` 在 `node.lock` 内建立私有通道，`direct-endpoint.json` 只含通道地址、实例 ID 和认证 token；普通 `run` 不创建。原生层不把 token、grant、模型 Key 或节点凭据交给网页。启用加密的窗口沿用 sealedV1 与 Olm 密钥投递；不提供局域网连接。

## 迁移与备份

`0008_migration.sql` 增加 `cloud_imports` 的来源、内容指纹和 ID 映射，`cloud_messages.import_origin` 的历史来源标记，以及窗口 `read_only`。旧完成顺序与消息顺序可能不同，因此 `cloud_import_reads` 按账号保存迁移时已经读过的消息集合；后续新消息继续按正常读位置处理，不因位置转换把旧未读误标已读。

导入在同一事务新建身份、关系、窗口、消息、映射和通知，不走发送或任务入队流程。附件先在本批临时目录校验保存，再发布并提交数据库。提交结果不明时，等待原事务释放同一把锁后查询持久回执；确认未提交才清理本批文件，查询失败则保留文件等待确认。

节点 `transfer_receipts` 保存每 AI 的包 ID 和指纹。导入先同步来源、核对当前成员与消息版本，原子保存档案和材料版本后再次同步；同包重试不覆盖后续编辑或遗忘。未完成摘要保留中断状态，失效条目不恢复正文，自动摘要关闭。节点凭据、任务、预算、调用确认和同步游标不迁。

### 旧数据迁移与节点迁移

旧 v13 迁移先锁定旧目录，用 SQLite backup 保存完整备份，再预览账号与来源映射。真人映射到登录账号，AI 新建而不按同名合并；聊天、附件、关系、成员、已读置顶、屏蔽和停用迁入云端，角色、记忆和摘要迁入对应 AI。源库与十三份 `server/tests/fixtures/local-v13` SQL 保持原字节。头像、旧审计与修订资料留在备份。

仅向仍有来源访问权的 AI 分发有效摘要，并标明导入来源；旧失效条目不启用、不付费重新概括。旧调用不重放，未来约定须重新确认。更换节点沿用云端身份和历史，新节点重新配置秘密并授权，资料包类型为 `connector::node::transfer::AiTransfer`。

验收核对预览与导入范围、重复导入幂等、附件完整性、来源映射、摘要权限、备份恢复及新节点接续。正式记录单向切换，不长期双写；失败时保留旧库和导入记录供核对。命令与具体映射限制见[迁移指南](../migration.md)。

## 云端与 Agent 的数据归属

### 数据归属

| 位置 | 保存内容 |
| --- | --- |
| 云端 PostgreSQL | 账号、认证、管理员、联系人与屏蔽、成员与权限、消息及修订、附件元数据、读态、事件及回执；节点绑定、任务、调度规则、执行终态；设备公钥、密钥投递与恢复密文、AI 密文资源 |
| 附件持久目录 | 正式图片、文件或密文字节；数据库记录消息、索引、版本、格式、大小和完整性信息，不保存用户设备路径 |
| Agent 私有目录 | 解密后的角色、模型参数、记忆、摘要、来源版本、检查点和预算；同机缓存及待同步消息；节点凭据与 SDK 秘密 |
| 客户端本机 | SDK 密钥库、恢复材料、缓存与草稿，按服务器和实际账号隔离 |

未加密的角色、记忆、摘要和模型参数仅在线转发，节点保存后才确认成功，离线不排队。加密资料由云端保存密文并单独记录节点应用状态。模型 Key 留在节点，邮件凭据属于服务端私有部署配置；两者与业务导出分开。

### 提交与备份边界

云端在各用例事务内复核权限、来源、绑定，提交消息/修订、附件引用、任务、事件与回执；领取任务和提交结果各自事务化，不跨模型请求持有事务。节点以本地事务保存资料与检查点，两端通过协议同步版本和状态。

数据库与附件没有跨存储原子提交，附件写入与失败处理见[当前云端存储](#当前云端存储)。备份时暂停写入和附件清理、排空正在提交的操作，保存 PostgreSQL 一致快照及其引用的附件，记录配套标识和完整性清单后恢复写入。恢复成套使用，并检查全部正式附件引用。清理只处理已结束上传且无有效引用的文件。

`cloud_restore_acceptance.py` 将指定测试服务的快照还原到独立临时数据库和私有附件目录，逐表逐文件核对，不覆盖原库。定期在线备份工具尚未提供。站点操作见[部署指南](../../server/deploy/README.md#复验与备份)，节点工作资料与秘密另行私密备份。

## 本机未发送草稿

草稿使用 IndexedDB，按服务器、实际真人账号和窗口隔离，默认本机明文。发送前保存完整命令与 requestId；未知结果手动重试原命令。清理浏览器/WebView 资料会删除草稿。恢复、多页面冲突和关闭协调由[客户端界面与桌面设计](https://github.com/NingCold/llm-nest/blob/main/docs/design/frontend.md)维护。

## 备份、遗忘与回退

遗忘和撤回不修改已导出的文件、备份或供应商已接收的内容。加密身份及恢复材料按原保护方式单独保存，不按可再生产物清理。发布或拆仓不触发清库、换身份或目录迁移；回退应用不默认回退数据库。旧 JSON/SQLite 仅通过明确迁移导入，不随客户端启动自动导入。
