# TC4 — 选择接纳与两种宿主恢复

2026-10-11；**TC4 已完成，未提交、未部署。** 沿 ADR-0163 §4–§7 与既有术语实现。HEAD 为 `12b765bb33fe42fcd30fcda743187241ed56d1f3`，工作树包含 TC0–TC3 及其他未提交工作。

## 实施步骤

1. [x] 接通原交付选择、来源／head／控制校验、Goal 关系及 eligible／consumed 投影；用真实存储夹具验证选择与冲突。
2. [x] 补齐本地准备快照和两宿主选择事实恢复；对各持久化边界注入失败，核对同一 turn、选择事件及执行次数。
3. [x] 回归 Tutor、接纳、SessionStore 与 MU10 原失败接点；同步架构、代码链路和 checkpoint。

## 验证目的

检测重复接纳、重新解释原现场、事件漏记和未知执行被重放；出现失败时修正相应持久化边界。MU10 两处旧断言先核对 JSONL 权威输入是否已经迁到独立 inputs 投影，再决定修改实现或测试。

## 实现合同

- `interaction_input::resolve` 从当前读者原聊天的成功交付读取选项，冻结原公开回答、TeachingTarget、source bindings 和被接受的文案；检查原 head、来源版本、TutorSession/control revision 与可选的同版本演示现场。回答接续与纠偏仍核验公开摘录，允许历史回答使用点击时当前 head。
- 选择历史显示“选择：…”，用户补充独立保存；无补充的 explain／compare／observe 由 `tutor::action_intent` 确定 current_action，真实补充保留原优先级。choice 不写 teaching_ref 或 chat_response。
- 两宿主均从原回答解析 Goal；open 时接续，completed 时建立带 related_goal_id 的新 Goal，保留旧完成事实。
- `TurnDomain.choice_event` 随接纳回合保存原选择事实模板，稳定身份为 `chosen:<accepted_turn>`；只由已接纳输入幂等投影至 learning.db。补写无需重新读取当前选项或执行模型。
- 本地复用 `turn.accepted` 保存 client_request_id、normalized 与 ResolvedInteraction，复用 `turn.prepared` 保存 local_preparation。准备快照先落盘再写教学事实；启动及原 key 回查补记原事实，重启后的未知执行保持 INTERRUPTED。
- 多人复用 FrozenTurn、JSONL inputs 和既有准备／领取协议。preparing 已有冻结输入时也检查同 key 载荷冲突；已持久输入按原内容恢复，claimed 不自动重放。
- 公开视图先从已接纳回合判断 consumed 并返回 accepted_turn_id；未消费且原 head／控制／来源有效时 eligible，失效时 stale／unavailable。不同 key 的并发选择受原用户锁和 head 约束，同集合最多接纳一次。
- 正式活动求助沿原 ActivityHelp 分流，实际帮助与作答仍沿原回执。准备、回查和读取不另建执行队列或计费入口。

## 确定性结果

| 验证 | 结果 | 证据 |
|---|---|---|
| Server Tutor、SessionStore／SessionEvent、JL3、MU6b、MU10 真进程中断 | 113 通过，9 默认忽略 | [final.log](tutor-continuation-tc4/final.log) |
| Runtime Tutor 及输入合同 | 26 通过 | [runtime.log](tutor-continuation-tc4/runtime.log) |
| 最终 TC4 与 SessionStore／SessionEvent | 20 通过 | [closure.log](tutor-continuation-tc4/closure.log) |
| MU10 100 个未领取回合重启 | 通过；输入保留、零 Provider 调用 | [regression.log](tutor-continuation-tc4/regression.log) 中该独立测试；同批旧断言失败见下文 |
| 本地请求由按钮改为文字的同 key 冲突，以及直接帮助／两宿主交付回归 | 3 通过 | [identity-final.log](tutor-continuation-tc4/identity-final.log) |

测试分组重叠，不累计为独立测试总数。默认忽略的是实模、浏览器宿主等既有入口；本片没有额外付费模型调用。

[逐操作证据](tutor-continuation-tc4/evidence.json)从收口日志提取 27 条记录，包含请求、接纳 turn、选择事件、执行开始次数、Provider 调用数及终态（已开始后中断的四条记录保留 turn 与计数，原请求见同名测试夹具）。其中：

- 本地 4 种动作 × 4 个故障点（accepted／snapshot／teaching_receipt／prepared）共 16 条：同 key 回查两次仍是原 turn，重启后为 INTERRUPTED，执行开始数 0。只有 choice 产生一个 ContinuationChosen。
- 本地 4 种动作在真实运行入口进入替身 Provider 后模拟宿主消失：重启后保持原 turn，执行开始数与 Provider 调用数仍为 1，无第二次执行。
- 多人 choice 在 pending／chosen_receipt／teaching_receipt／prepared／queued 后恢复共 5 条：领取次数各为 1，同 key 多次查询不增加 Provider 调用。该组故意使用不执行 tutor.step 的简单替身，实际终态 failed、各 2 次采样，仍保持消费及原选择事实。
- 两宿主各一条完整成功路径：`book.text → tutor.step(select compare) → answer`，一条接纳、一条 chosen、一条教学交付，3 次替身采样，incomplete=false；重复原请求不增加采样。

此外自动用例覆盖：两宿主双标签不同选项仅一项接纳；preparing 尚无输入时原 key 关闭；同 key 不同选项、动作、回答、摘录冲突；本地按钮 key 不可换成普通文字再执行；历史回答在当前 head 接续而旧建议失效；原 Goal open／completed 分流；来源／控制／聊天／演示不匹配；多人重新打开权威存储后选择快照不变，撤销授权后零执行；claimed 恢复后中断，问题计数保持一条。

MU10 原进程中断测试的八个窗口已全部通过：preparing、pending、teaching_receipt、prepared、queued、claimed、note、terminal。claimed／note／terminal 恢复后的新增模型调用均为 0。题内帮助、取消、部分写入及原交付回执修复由同批 Tutor、JL3、MU6b 原用例覆盖。

## 失败记录与修正

- [red.log](tutor-continuation-tc4/red.log)：旧实现仍返回 offered，新增选择消费测试失败。
- [initial.log](tutor-continuation-tc4/initial.log)：测试夹具没有保存回答来源，误把完成 Goal 当作 open，且简单模型替身没有实际交付建议；分别补真实来源、按原 Goal 状态断言、用已持久交付建立准入夹具。
- [targeted.log](tutor-continuation-tc4/targeted.log)：多人恢复只执行了一个 Run，但其中发生两次采样；改为检查持久 attempt=1，并分别记录 Provider 调用，保留失败终态。
- [regression.log](tutor-continuation-tc4/regression.log)：57 通过、1 失败、4 忽略。MU10 的首处旧输入断言修复后，执行到后续同类旧字段断言再次失败。两处都改读 `SessionLog.projection.inputs`；100 回合测试同样检查权威投影，不再检查已移走的 turn.admission_input。随后八窗口全部通过，未放宽恢复断言。

## 复现

环境沿 checkpoint：Rust `+1.95.0`，`CARGO_TARGET_DIR=D:/codex-build/understand-book-no-progress-195`，dev/test debug=0、incremental=0，TEMP/TMP=`D:/codex-build/tmp`，`-j 2`。

```text
cargo +1.95.0 test -p server --lib -j 2 -- tutor session_store session_event jl3 mu6b mu10_real_host_kill --test-threads=1 --nocapture
cargo +1.95.0 test -p runtime --lib tutor -j 2 -- --test-threads=1
cargo +1.95.0 test -p server --lib -j 2 -- continuation_tc4 session_event session_store --test-threads=1 --nocapture
cargo +1.95.0 test -p server --lib -j 2 -- continuation_tc4_local_answer_identity continuation_tc2_selection continuation_tc4_both_hosts --test-threads=1 --nocapture
```

## 已知限制

本片验证接纳、事实恢复与确定性执行；成功路径使用替身模型，不代表真实模型的建议兑现质量或真实读者效果。外部 Provider 的未知结果不具备 exactly-once 保证。

选择／纠偏服务端合同已开放，前端建议宿主、展示事实与交互体验归 TC5／TC6。完整 Markdown／PDF 界面、真实材料兑现及纠偏对照归 TC7；旧二进制对新本地 turn.prepared 的读取与降级演练归 TC8。TC2 既有公开回答内容问题及真实读者验收仍保留。
