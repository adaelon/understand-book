# Linux 多人阅读 MU6 运行接口

MU6a–MU6d 在 MU5 的授权现场上接通持久接单、按用户公平并发、恢复、指定取消、聊天删除和连续观察。账号、Cookie、CSRF、Origin 沿 [MU4](Linux多人阅读-MU4候选入口.md)，现场创建和版本沿 [MU5](Linux多人阅读-MU5现场接口.md)。实现与验证见 [MU6a–MU6b 记录](performance/linux-multi-reader-mu6ab-20260930.md)和 [MU6c–MU6d 记录](performance/linux-multi-reader-mu6cd-20260930.md)。

## 接单与请求恢复

先创建或选择聊天。客户端在首次发送前生成并保存 `client_request_id`；网络超时后保留原键及原请求内容。

```json
POST /api/workspaces/WORKSPACE/agent/runs
{
  "client_request_id": "client-generated-request-1",
  "session_id": "CHAT",
  "attachment_id": "page-instance-1",
  "generation": 2,
  "expected_revision": 2,
  "published_book_ref": {"book_id": "BOOK", "publication_id": "PUBLICATION"},
  "expected_chat_head": null,
  "message": "解释这一段"
}
```

版本取最近现场响应；`expected_chat_head` 为上一回合 ID，新聊天为 null。确切发布与聊天头若提交则必须相符；服务总会冻结实际发布和聊天头。引用选区、演示、教学或任务时，沿原问答字段提交 `question_anchor_lid / question_quote / presentation_follow_up / teaching_ref / goal_id / goal_action`。

只有私人 Pending Turn、完整冻结输入、教学准备回执和 `queued` 均持久完成后才返回 202。响应包含 `turn_id / session_id / published_book_ref / dispatch_state / persistence_state`。同 owner、同键、相同规范化内容返回同一回合；比较字段包括问题、显示文本、引用、任务动作、现场及提交版本，直接比较，不计算请求摘要。已接单请求的重试不要求当前页面仍停留在原现场版本。

网络接单在 preparing 提交前分配唯一 turn ID，之后按原键复用。若准备失败时尚未写入私人 History，关闭旧键后在同一聊天提交新键会得到新的 turn ID，不会撞上保留的失败记录。

| 返回 | 含义与后续动作 |
| --- | --- |
| `409 ADMISSION_PREPARING` | 原键仍在准备；保留原键，查询状态或稍后重试 |
| `409 REQUEST_KEY_CONFLICT` | 原键的内容不同；保留原请求，明确新问题才使用新键 |
| `409 ADMISSION_FAILED` | 原键有可比较的失败输入；返回原回合描述，不再次执行 |
| `409 REQUEST_KEY_CLOSED` | 原接单没有可恢复输入，或聊天已删除；此键永久关闭 |
| `409 CHAT_BUSY` | 原聊天或现场仍由 preparing、queued、claimed 或未保存结果占用 |
| `429 RUN_QUEUE_FULL` | 未结接单达到配置上限（默认每用户 4、全局 20） |
| `503` 存储错误 | 未完成接受承诺；查询原键，不能直接换键重发 |

`GET /api/agent/runs?client_request_id=KEY` 在当前 owner 范围查原接单；响应丢失后可由此取回 turn ID。`GET /api/agent/runs/TURN` 返回接单状态与私人历史投影；活动/未保存运行还带既有 RunStream 快照。

兼容入口 `POST /api/agent/runs` 需要请求体额外提供 `workspace_id`。`POST /api/agent/chat` 和 `POST /api/workspaces/WORKSPACE/agent/chat` 使用同一接单核心并等待同一持久回合；前者也必须带 `workspace_id`。成功沿原接口返回 outcome；失败返回状态，未保存返回 `TURN_UNSAVED`。同步等待独立于普通 HTTP 工作线程，默认每用户 4、全局 20 个；等待容量满时返回同一已接受回合的 202，客户端改为查询/观察该 turn。断开 HTTP 连接不取消已接受回合。

## 冻结输入与启动恢复

私人 History 的 Pending Turn 保存版本 1 冻结载荷：原 owner、现场及 generation、聊天及前序头、确切发布、Reader 快照、问题及引用、消息前缀、原教学准备事件和载荷、任务、Provider 模式/地址/模型/运行配置。Provider 密钥、临时敏感候选正文不写入冻结载荷；临时确认只记录原身份。

| 持久状态 | 启动动作 |
| --- | --- |
| preparing，无私人回合 | 记 admission_failed，关闭原键并释放占位 |
| preparing，有完整冻结载荷 | 复用原教学事件 ID/载荷补齐缺项，完成标记后提交 queued |
| queued，输入及依赖完整 | 使用原 Book、原消息和原绑定，按用户轮转领取 |
| claimed，私人回合仍 pending | 保存 `INTERRUPTED`；恢复原冻结消息前缀和问题，不重放模型/工具 |
| claimed，私人回合已有终态 | 幂等补齐教学交付事实与复核游标，修复接单索引 |
| 已丢失临时敏感确认 | 保存 `SENSITIVE_CONFIRMATION_EXPIRED`，由用户重新确认 |
| 原 Provider 绑定不可用或材料权限撤销 | 明确结束原回合，不换用新 Provider 或新材料 |

切书、选其他聊天或现场换代不改变已接单问题的私人归属。旧 Run 对原 Book 的私人写入保持原归属；失效现场上的 Reader 效果被拒绝。退出登录结束观察权限，已接受执行仍属于原用户；禁用账号或撤销材料权限在执行边界重新检查。

## 取消、未保存与删除

- `POST /api/agent/runs/TURN/cancel`：排队取消与领取共用提交边界，只有一方成功。已领取运行沿协作取消退出；已发生的笔记等私人写入保留，实际退出后释放计算槽。
- `POST /api/agent/runs/TURN/retry-save`：重试原未保存结果。进程内保留确切输出和原上下文，只重做持久化；重启丢失内存结果时保存中断/取消终态。两者都不重新调用模型。存储仍失败则继续占位。
- `DELETE /api/me/chats/CHAT`：同聊天有 preparing/queued/claimed/unsaved 时返回 `409 CHAT_BUSY`。先处理运行与未保存结果，再删除。
- `POST /api/agent/history/delete`：请求体提供 `session_id`，与上述 DELETE 共享实现。

成功删除先保存私人 History，再关闭该聊天的旧请求键，清除所有相关现场的聊天选择、演示和附属状态并换代。若 History 已提交而现场清理中断，后续访问会修复残留选择；旧键不能恢复已删除聊天。

## 连续观察与游标

`GET /api/agent/runs/TURN/events` 使用同一授权 SSE。排队、执行、模型槽等待及最终保存共用一条流；重连只 GET，不重新 POST。每批推送前复核确切登录会话和原材料授权；退出登录关闭该会话观察，取消只影响指定 turn。观察数默认每用户 4、全局 40，超额返回 `429 OBSERVATION_CAPACITY`，断线退出后释放名额。

多人事件 ID 是不透明的 `observation_epoch:sequence`；通过 `Last-Event-ID` 或 `after` 原样传回。收到 `run.snapshot` 时用它替换当前实时投影，并采用该事件 ID。快照具有 `live_buffer_reset: true`，所有事件带 `observation_epoch`。当前流游标仍被有限缓冲覆盖时只补后续事件；游标过期、数字旧游标、服务重启或已释放的终态流重建时返回快照，不拼接旧序号。客户端按流维护游标；本地单用户的数字游标保持原格式。

```text
GET events -> queued snapshot -> run.started -> run.resource / activity / answer patches
           -> run.finalizing -> saved terminal 或 run.persistence_failed
断线 -> 同 turn + 原 Last-Event-ID -> 后续事件或替换快照
重启 -> 新 observation_epoch + 持久 History/接单快照
```

`execution_state=waiting_model` 表示已领取但在等待模型槽，仍可取消。`persistence_state=failed` 与 `TURN_UNSAVED` 表示终态尚未保存；retry-save 后可重新观察原 turn。重启后的 `INTERRUPTED`、`SENSITIVE_CONFIRMATION_EXPIRED` 等错误来自原持久回合；401、404、429 仍是 HTTP 状态。慢观察者只持有有限实时缓冲的独立读写线程，不阻塞 Agent 事件发布；全局缓冲丢弃旧事件后用同一原子序号边界恢复快照。

## 运行容量与模型用量

服务启动读取既有 Provider 环境配置；未配置时阅读可用，接单返回 `PROVIDER_NOT_CONFIGURED`。管理员可在服务根放置 `service-limits.json`，启动时一次读取；省略字段使用下表默认值。字段未知、非正值或无效 JSON 返回 `SERVICE_LIMITS_INVALID`，不带着错误配置启动。配置只影响该多人服务实例。

| 字段 | 默认值 | 含义 |
| --- | --- | --- |
| `active_runs / user_active_runs` | 4 / 1 | 全局/单用户活动份额；领取准备也占份额 |
| `queued_runs / user_queued_runs` | 20 / 4 | 全局/单用户未结接单，含 preparing、queued、claimed、unsaved |
| `model_slots / user_model_slots` | 2 / 1 | 实际 Provider 调用并发，等待者按用户轮转 |
| `preview_slots / plot_slots / animation_slots` | 1 / 1 / 1 | 三个独立制作资源池，每用户各最多 1；网络制作需通过 MU7 执行器探针 |
| `presentation_bytes / presentation_files` | 536870912 / 4096 | MU7 每用户候选、版本和状态的累计落盘字节/文件上限，详见 [MU7 接口](Linux多人阅读-MU7受限制作.md) |
| `sync_waiters / user_sync_waiters` | 20 / 4 | 同步 chat 等待响应 |
| `sse_connections / user_sse_connections` | 40 / 4 | 观察连接 |
| `request_bytes / event_bytes` | 65536 / 262144 | HTTP 请求体/每个实时事件缓冲字节数 |
| `resident_users` | 10 | 常驻私人权威实例 |
| `workspaces / user_workspaces` | 20 / 4 | 常驻现场 |
| `books / book_bytes` | 20 / 2147483648 | 所有存活 Book 的数量和既有保守计费字节 |

例如 Linux 冻结容量验收负载可设 `{"user_queued_runs":2}`，沿 MU0 的每用户 2 未结接单、2 个全局模型槽运行。闲置回收沿原用户 30 分钟、现场 15 分钟，现场维护周期 60 秒。

运行领取按用户轮转，同用户内部按接单顺序。模型许可只覆盖一次实际调用，调用退出后才释放；工具、画像判断、教学评估、嵌套查询、压缩和修复使用相同受限 adapter，不在整个 Run 期间占住模型槽。暂停/取消不会提前释放仍在网络调用中的槽。每次真正发起新调用前复验原用户与原材料授权。

`turn.usage` 返回各次 Provider 实际 usage，未知项为 null；累计字段 `known_total_tokens` 只累计已知总数。重复的累计 usage 帧取最后一帧，超时若没有 usage 就保持未知。调用次数和累计 Token 只用于用量记录，不触发停机。单次请求保留显式输出上限，未指定时使用模型运行配置的输出预留；累计用量不挤占后续请求的输出额度。Resident 按任务进展持续执行，完成、用户取消、实际无进展及不可恢复错误沿 [ADR-0150](adr/0150-resident-unbounded-tool-loop-and-progress-stops.md) 处理。

停止时先停接单/领取，再协作取消活动回合、结束同步等待、冲刷常驻用户 pending reads；未领取 queued 留待下次启动。断开的观察不取消执行。事件分片只在有界内存中，usage 随私人 History 终态保存，不逐 token 写 control.sqlite。

`control.sqlite` 仍为 schema 4；本片没有新的数据库迁移。存在新格式接单时，旧恢复代码不能直接读取该根。

## 已知限制

生成代码执行与私人产物/临时磁盘隔离归 MU7，完整 Vue 页面及设备生命周期归 MU8。当前网络端演示制作、翻译、本地化和独立后台复核入口保持能力拒绝；实际可达的嵌套模型调用已进入共享预算。三个制作资源池已定义和验证独立容量，实际执行器接线与 Linux 资源清理在 MU7 验收。

本轮证据来自 Windows 隔离服务根、本地 fake Provider、真实 HTTP/SSE 和服务根重开。Linux kill/掉电、真实 HTTPS/iPhone、生产容量和沙箱验收仍按后续切片执行。MU0 的完整五分钟 Linux 性能门槛不由本轮定向并发测试代替。
