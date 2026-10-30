# 后台重试修复与 200 人候选配置

2026-10-27 23:06:31 HKT 源码修复已上线，200 人候选配置保留待服务器扩容后启用。没有压测或验收模型调用；发布、隔离验证和线上检查见[上线记录](deployment.md)。现网初始基线与容量判断见 [只读调查](README.md)。

## 已实现的行为

- **后台复核收敛**：`memory::ReviewJobStatus` 增加 `Failed`、`Cancelled`；每个任务持久保存尝试次数和最后错误。来源删除或消失后取消，输入归属或区间不匹配后失败，不推进成功复核水位。启动或用户加载用完整会话目录处理既存孤儿。
- **有限重试和恢复**：同一输入范围自动尝试最多 5 次，从失败实际结束时计算退避。前一批未完成时，后一批不提前调用模型。用户产生新的已结束回合后，归属一致且未复核区间完整时，可以建立覆盖尚未复核证据的新任务；旧失败记录保留，成功覆盖后不再把画像标为过期。已删除会话不恢复。
- **删除中的任务**：多人服务与本地两种聊天存储入口均取消对应复核；正在请求模型的任务即使晚到也不能提交结果。未完成、失败和取消不再混作“待执行”。
- **模型请求时限**：前台单次请求总时限默认 600 秒，后台记忆复核专用时限 180 秒，连接时限 15 秒。时限由 HTTP 库实际约束并覆盖响应体；超时不立即再发一次传输重试。翻译和 coach 原有 60 秒专用预算保留。多轮 Agent 回合可超过单次请求时限。
- **用户锁**：暖用户查找先释放注册表锁，再等待该用户私有锁和更新活跃时间，保留同一用户唯一状态、授权复核及闲置回收语义。
- **请求与输出连接**：HTTP 工作线程数改为配置项，默认仍为 4。授权 SSE 在事件到达时立即唤醒并在发送前复核；空闲复核每秒一次，网络保活在连续 10 秒无数据发送后写出。

主要入口：`memory/src/review.rs`；`server/src/{host,multi_user_review,user_runtime,run_admission,session_store,authorization,user_registry,agent_stream,service_limits,multi_user_host}.rs`；`runtime/src/{lib,profile_api}.rs`。

## 200 人候选配置

文件：[`scripts/linux/service-limits.200-users.json.example`](../../../scripts/linux/service-limits.200-users.json.example)。它针对 **200 名读者同时在线，阅读为主、约 10 次提问/分钟的待验证场景**；200 人同一瞬间全部提交 AI 请求属于更大的负载。

| 项目 | 当前默认 | 候选值 | 作用 |
|---|---:|---:|---|
| 常驻用户 | 10 | 300 | 给 200 人及暂时闲置状态留空间 |
| 常驻阅读现场 | 20 | 600 | 允许每人常用 2–3 个现场；每人上限仍为 4 |
| 活动 AI 回合 | 4 | 64 | 包含工具处理及等待模型槽的回合 |
| 未结接单总数 | 20 | 128 | 包含准备、排队、活动和未保存任务，并非另加 128 个排队位 |
| 前台同时请求模型 | 2 | 16 | 每用户仍为 1；后台复核独立槽仍为 1 |
| AI 输出连接 | 40 | 256 | 每用户上限仍为 4 |
| 普通 HTTP 工作线程 | 4 | 8 | AI 流和同步等待沿用独立线程 |
| 预览 / 绘图 / 动画制作 | 各 1 | 各 1 | 限制本机重制作任务同时占用 |

同步等待上限保持 20；名额用满时，已接纳请求沿既有 `202` 异步观察流程继续，不增加长时间同步等待线程。共享书籍预算和单用户资源上限沿用默认值。

配置需与包含新字段的新版本服务一起使用，置于真实服务数据根的 `service-limits.json`，启动时读取。示例文件不参与当前服务启动，也没有替换仓库默认容量。旧版本会拒绝新增字段，不能先把示例拷到旧服务并重启。

## 验证

本地 Windows、Rust 1.96.0，使用临时存储、假模型或回环 HTTP 服务。未运行并发容量、性能基准或生产流量。

最终 98 项定向功能检查通过：Memory 23、Runtime 3、Server 72（首批 71 项通过，修正一项新增测试的契约断言后定向补测通过）。

- [Memory 23 项通过](memory-tests.log)：重试封顶、失败后新证据恢复、原诊断保留、删除已失败来源、孤儿重开、真实水位及原子提交。
- [Runtime 3 项通过](runtime-tests.log)：响应头或流式响应体停住后实际关闭连接，超时不即时重发、部分用量保留，连接预算不错误截断健康长响应；覆盖 Native、ReAct 和 Messages。
- [Server 首批 72 项](server-tests.log)：71 项通过，含用户锁、TTL、授权、SSE 撤销与重连、配置、限额释放、后台计费和聊天删除。新增超时恢复测试的首次断言误用了 `failed`；既有契约是保存 `completed` 的降级答复并标记 `incomplete=true / PROVIDER_ERROR`。测试已更正并加强后续正常答复断言，[定向补测 1 项通过](server-timeout-tests.log)，确认超时后资源归零且下一次真实流程正常完成。

复现环境：`CARGO_INCREMENTAL=0`、`CARGO_PROFILE_DEV_DEBUG=0`、`CARGO_PROFILE_TEST_DEBUG=0`、`-j 1`，`TEMP/TMP` 位于本工作区 E 盘临时目录。主要命令：

```text
cargo test -j 1 -p memory --lib review -- --test-threads=1
cargo test -j 1 -p runtime --lib provider_timeout -- --test-threads=1
cargo test -j 1 -p server --lib -- mu2_ mu4_ multi_user_review review_retry_ stopped_review_ permanent_review_ retry_backoff_ fake_clock_ service_limits::tests agent_stream::tests session_management mu6a_stalled_ mu6d_ --test-threads=1
cargo test -j 1 -p server --lib mu6a_stalled_provider_stream_times_out_and_releases_run_resources -- --test-threads=1
```

## 已知限制与上线条件

- **尚未证明可稳定承载 200 人**。本轮按用户要求不做压测；候选参数是待验证起点。现网约 4 GB 内存且同机还有其他服务，不应直接套用扩大后的容量；沿前次调查，建议先准备 16 GB 内存档，CPU 和模型并发再依后续实际负载决定。
- 首次加载用户及无人持有的闲置用户落盘仍在注册表锁内，集中首次进入的耗时尚未测量。上游账号配额、购买带宽和真实模型并发也尚未核验。
- 正在进行的后台模型请求不会因删除聊天立即中断；它在返回或请求超时后退出，但结果不会提交。HTTP 库的系统 DNS 解析仍不能被请求 deadline 中断；本轮不声明独立的“60 秒无数据超时”。
- 新增任务终态可由新版本读取旧数据；旧版本不识别新写入的 `failed/cancelled`。部署前需保留数据备份，不能在新状态已写入后直接用旧二进制打开同一私人记忆文件。工作区同时已有独立的账号额度变更，完整发布和回退还需遵守其 schema 21 要求。
