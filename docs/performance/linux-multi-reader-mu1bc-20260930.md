# Linux 多人阅读 MU1b / MU1c 验收

日期：2026-09-30。基线 HEAD：`5e10516`，实施基线为 MU1a 完成后的实时工作树；既有未提交工作保留。设计合同：[ADR-0147](../adr/0147-linux-multi-reader-service-without-redis.md)、[切片方案 §5.3 / §8](../切片方案-Linux原生多人阅读与无Redis首版.md)。

## 实施状态

- [x] 执行 checkpoint 全部冷启动读序，确认 HEAD 一致。
- [x] 保存改动前 Server / Runtime 源码到 `tmp/mu1bc-baseline`，聊天历史基线 6 项通过。
- [x] MU1b 现场归属与选择隔离；all-targets 编译、聊天历史 6 项、双现场隔离 1 项、Host 切书重启 1 项通过。
- [x] MU1c 固定 Run 归属、冻结输入及受限端口；新增 MU1 测试合计 5 项通过。
- [x] Reader / Runtime / Server 回归、失败修复定向复验与最终文档刷新。

## MU1b 归属

`AppState` 组合 ServiceState、唯一 UserRuntime 与一个本地 ReaderWorkspace。现场持有 Book、book_dir、Reader、messages、selected_chat、generation、session_path、工作台加载版本与活动流。Host 显式恢复现场；MCP 保留独立访客权限和空私人根。

`AgentHistory.active_by_book` 沿用旧持久格式，只供本地启动/切书恢复选择；活跃现场通过 selected_chat 投影历史、准备新回合、处理 Goal、画像确认与演示追问。新建/选择/删除在 History 提交成功后更新现场；切书、工作台重载及选择变化使代次更新。删除其他聊天保留本现场原选择。两个内存现场使用同一用户历史，分别验证异书 Reader 和同书不同聊天互不覆盖。

## MU1c 运行边界

RunScope 固定本地用户、原现场 ID/generation、原聊天/turn、原 Book Arc/目录及模型运行配置。ReaderInputSnapshot 冻结视口、选区与地图上下文；RunScope 同时固定已验证问题引用及演示追问回执，Provider 输入中的演示状态来自该次不可变回执。

`ResidentStatePort` 使用三个操作入口：私人提交只借 MemoryStore；实时 Reader 读取校验原现场代次；效果应用校验后才短暂借用 Reader 与导航所需的已读账本。旧的通用 with_state 入口已删除。模型、预览和渲染等待继续在宿主状态锁外。

Reader 的 save_note/save_highlight 负责私人记录，select_annotation 负责现场选择。换代后前者仍保存原书记录，后者不执行；工具返回 reader_effect.status=not_applied，导航/布局返回 not_applied + WORKSPACE_STALE，实时 reader.state 返回 WORKSPACE_STALE。旧现场数据不会伪装成实时 Reader；初始定位与带读上下文明确来自提问快照。

PrivateBookContext 只借用户私人权威和明确材料，不含可变 Reader。Tutor、演示、画像投影与私人成果快照按原 Run 的 Book/目录/聊天解析；Tutor 控制仍从同一用户 Learning 读取。最终 History 与教学交付进入原聊天；仅代次仍匹配时更新现场 messages，SSE 终态投影使用原 Book。

## 确定性验收

新增测试：

- `mu1b_workspaces_keep_independent_book_reader_and_chat_selection`：同一用户的异书现场、同书不同聊天，以及删除非当前聊天保持选择。
- `mu1c_old_run_cannot_touch_replaced_workspace_but_saves_original_note_and_answer`：真实 Host/fake Provider 在等待中受控换代；旧 reader.state 失效、goto 未应用，原书笔记/高亮及原聊天回答落盘，新 Reader/messages 不变。
- `mu1c_chat_switch_back_keeps_old_run_stale_and_input_frozen`：切到新聊天再切回，旧 generation 仍失效；快照不变；换成访客身份拒绝私人提交。
- `mu1c_presentation_author_keeps_original_chat_and_material_after_switch`：换书后原 Run 可创建/读取原聊天版本；当前书接口拒绝跨书读取。
- `mu1c_tutor_material_keeps_original_binding_after_workspace_replacement`：新目录无原教学图时仍能读取原材料；在新现场关闭同一用户 Tutor 后，旧运行即时观察关闭。

受控换代测试绕过本地 Host 的切书 busy 边界，只在持有同一短锁时调用业务入口，不代表开放网络多现场。首次测试串行注入两轮无进展工具后继续发工具，触发现有 FINALIZATION_TOOL_PROTOCOL_VIOLATION；修正测试为同一批工具调用后通过，未修改运行预算或断言。

## 回归结果与修复

`cargo test -p reader -p runtime -p server -- --test-threads=1` 的[首次全量记录](linux-multi-reader-mu1bc-20260930/regression-first.log)：Reader 54 通过；Runtime 库 411 通过/3 原有忽略，集成 6 通过；Server 库 341 通过、3 失败、33 原有忽略。未将这个退出非零的命令记录为全绿。

三个失败分别处理：

- `profile_memory_state_exposes_resident_snapshot_facts_evidence_and_pending_status`：真实遗漏。画像确认入口首次创建聊天后未同步现场选择，导致待确认状态漏显；修复 `route_profile_governance` 在建立聊天时绑定现场。
- `presentation_ownership_follows_book_session_and_pending_turn`、`presentation_follow_up_rejects_receipt_mixup_before_precommit`：测试以修改 active_by_book 模拟实时切聊，已不符合 MU1b 合同；改为调用真实 route_agent_history_select，保留原越权/回执混用拒绝断言。

修复后的定向验证：[画像 14 项通过](linux-multi-reader-mu1bc-20260930/profile-fix.log)，[演示存储 13 项通过/4 原有忽略](linux-multi-reader-mu1bc-20260930/presentation-fix.log)。未改 Reader/Runtime，也未重跑已通过的无关用例。Server 库 344 个不同用例由首次全量与定向修复复验覆盖。

[MU1b 阶段记录](linux-multi-reader-mu1bc-20260930/mu1b-tests.log)、[5 项新增 MU1 测试](linux-multi-reader-mu1bc-20260930/mu1-new-tests.log)。Server 二进制/集成与文档测试补跑见 [remaining.log](linux-multi-reader-mu1bc-20260930/remaining.log)；MCP 5、CLI 1、预览集成 1 通过/10 原有忽略。Reader/Runtime/Server 的 doc-tests 无用例。

按不同用例汇总：**822 项已验证通过，46 项原有忽略**；不是单条全量命令一次全绿。编译保留既有 ts-rs serde 属性解析警告。未调用付费 Provider 或运行原有忽略的真实模型/浏览器验收。

## 已知限制

- 仍为本地单用户、单现场 Host 与原单活动 Run 调度；网络多现场与挂接/回收在 MU5，同聊天占位与取消/删除生命周期在 MU6。
- 当前材料绑定为原 Book Arc 和原目录；不可变 publication_id、发布登记和只读书库在 MU3，本片未伪造发布身份。
- 私人路径、History/Memory/Learning schema 沿用原格式；用户路径注入与服务 SQLite 在 MU2。SQLite 3.46.0 的未来 WAL 版本门槛保留。
- Linux、HTTPS/iPhone、多人容量、故障恢复矩阵 T01–T79 尚未端到端验收；本次未安装、部署或开放服务。T73/T46 仅覆盖本地受控换代核心，不标记多人矩阵完成。
