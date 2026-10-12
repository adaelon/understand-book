# SESSION_CHECKPOINT — 2026-10-11 22:45（Asia/Hong_Kong）

## 新鲜度自检

- 写入时最新 commit：`12b765bb33fe42fcd30fcda743187241ed56d1f3`，`merge: integrate PDF and conversation improvements with reading notes and sharing`。
- 读入时对比 `git log -3`；TC0–TC6、无进展修复及其他工作包含未提交／未跟踪文件，不能只读 HEAD。
- 用户本次授权“阅读 checkpoint，实现 TC6，完成后刷新 checkpoint”。**TC6 已完成，未提交、未部署。** 保留其他在途修改与既有 Computer Use 停止状态。

## 当前在做什么

TC6 交互事实进入讲法、理解证据与画像防污染已完成；下一片为 TC7 全链路与自然请求质量验收。见 [TC6 实施记录](docs/performance/tutor-continuation-tc6.md)、[切片方案](docs/切片方案-Tutor下一步协商与低门槛阅读入口.md)、[ADR-0163](docs/adr/0163-tutor-continuation-and-contextual-reading-entry.md)。

- `teaching_recent` 纳入建议展示／选择；摘要保留原选项、回答引用、有界摘录和助手身份，精确内容沿 trace 回读。当前动作、原回答、来源及真实补充继续冻结在 LearnerContext。
- `TeachingEvent::learner_expression` 提取真实用户表达；active／observing 认知待办只收当前来源版本下未解释的用户表达。纯选择／继续／纠偏、助手交付和曝光不成为认知待办；假设至少需要真实用户表达。
- `AgentChatTurn::user_authored_text` 用于画像审核、历史回填、待审核上下文和教学原话；ReviewTurnInput 单列 input_action，无文字操作保留序号，按钮文案与助手摘录不能充当用户引文。
- 理解假设仍沿原 fact_refs／teaching_implication／supersedes；旧假设被替换后不可 adopted，换来源版本不可沿用。正式表现和帮助条件沿原合同；无新增用户模型。
- 实现前 2 项失败已复现；最终 Server 定向 20 通过／2 默认忽略，Memory 全量 149 通过，Runtime Tutor／画像审核 34 通过。新增确定性测试 8 项；本地／多人选择快照、连续选择、无文字动作、真实补充、历史回填、原题帮助和关闭 Tutor 普通观察均覆盖。
- 单独执行 TC6 实模入口通过：deepseek-v4-flash，6 次采样，累计 usage 135,049 token，completed／incomplete=false。真实补充将旧“等式变形”假设替换为“停顿为何计入总时长”，Move 采用新假设，实际回答改变重点；source-only 对象投影前后均为空。轨迹、一次意图校验失败后的恢复均保留；真实读者仍为 0。

## 下一步（可直接接手）

1. 阅读切片方案 TC7、第 9／11 节，TC6 实施记录与 `tutor-continuation-tc6/trajectory.json`，明确工程矩阵与真实读者验收的剩余项。
2. 沿 `crates/server/src/tests/tutor_continuation_tests.rs` 和 `packages/web/playwright/tutor-continuation*.spec.ts`，按第 9 节矩阵补齐本地／多人、Markdown／PDF、额度／取消及完整首问→选择→纠偏路径的缺项。
3. 在固定真实材料上记录完整接续轨迹与手写 prompt 基线；保存原文版本、实际请求、工具调用、交付、错误和累计费用数据，核对公开内容而非只看 completed。
4. 完成真实读者的默认关闭→开始→继续→纠偏验证；把未验证／失败项写入 `docs/performance/tutor-continuation-tc7-acceptance.md`，不以脚本样本替代读者结果。
5. TC7 通过后进入 TC8：旧二进制读取、同版本撤入口／指导的降级演练及发布准备；实际部署另行授权。

## 未提交 / 未完成

- TC6 源码、8 项确定性测试、1 项实模入口、实施证据、架构／代码链路／ADR／切片状态及本 checkpoint 已落盘，未提交。
- TC0–TC5 与其他任务既有修改保持。TC2 真实模型稳定交付、公开内容质量与真实读者验收仍未完成；TC5／TC6 局部成功不关闭这些问题。
- TC7–TC8 未实施。真实模型本次仅一条纠正轨迹，输出仍偏长；不能声明通用教学质量或学习收益通过。
- 本地原请求恢复限原标签页会话存储；关标签页后沿服务端历史。外部 Provider 未知结果不保证 exactly-once。旧二进制对新增事件／Move／本地准备记录的读取不可假定兼容。

## 冷启动读序

1. 本 checkpoint → 切片方案 TC7、第 7／9／11 节 → ADR-0163 §4–§7；CONTEXT 的 TeachingChoice／回答接续与纠偏／ResolvedInteraction／LearnerContext／LearningEvidence。
2. `docs/performance/tutor-continuation-tc6.md`、`tutor-continuation-tc6/trajectory.json`、`live/learner-context.json` 与 `live/answer.md`；按需回读 `live/result.json` 的 facts／evidence／projection。
3. `crates/server/src/teaching.rs:trace_summary/uninterpreted_facts/usage_facts/accept_hypothesis`、`crates/memory/src/teaching.rs:learner_expression/teaching_recent`、`learning_evidence.rs:append_evidence/current_interpretations`；Runtime `tutor.rs:COMMON/instruction_modules`。
4. `crates/server/src/lib.rs:user_authored_text/pending_review_context`、`host.rs:copy_user_review_input/copy_historical_backfill_input`、`crates/runtime/src/memory_review.rs`；TC6 测试及既有 T17 修订、原题帮助测试；架构／代码链路 TC6 条目。
5. TC7 涉及界面时读 TC5 实施记录及 `TutorContinuation.vue`、`RightRail.vue`、`AgentPresentation.vue`、`App.vue`、`local-reading-action.ts`、`network-client.ts` 与两份 `playwright/tutor-continuation*.spec.ts`；接纳故障回归读 TC4 实施记录及 evidence.json。

## 环境与前序证据

- Rust `+1.95.0`，`CARGO_TARGET_DIR=D:/codex-build/understand-book-no-progress-195`，dev/test debug=0、incremental=0，TEMP/TMP=`D:/codex-build/tmp`，`-j 2`。源码变化后重新编译；默认 1.96 旧 Windows 构建问题未处理。
- TC6 指导 revision=`tc6.v1`；当前 TC6 实模使用已有 `.env` Provider 配置和显式模型，观测上传关闭。精确测试命令见 TC6 记录；默认忽略实模入口须显式执行，不能将 ignored 计为通过。
- [TC5](docs/performance/tutor-continuation-tc5.md)：17 项浏览器、3 条真实 Rust API 路径；两次脚本化实模纠偏，累计 usage 261,827 token。普通／演示唯一建议宿主，草稿和原请求恢复保持。
- [TC4](docs/performance/tutor-continuation-tc4.md)：27 条接纳／恢复操作证据；chosen 从原接纳幂等投影；历史回答用当前 head，新选择用原 head。
- [TC3](docs/performance/tutor-continuation-tc3.md)：五次实模累计 usage 435,587 token；旧 offered 投影现为 eligible／consumed。[TC2A](docs/performance/tutor-continuation-tc2a.md)和[无进展修复](docs/performance/agent-no-progress-20261011.md)保留原对照与失败输出。
- 用户确认口径：公开回答正确性优先；内部目标错误、公开回答正确且无后续影响证据时记内部质量观察。两段平均相等条件为 `(v1-v2)*(t1-t2)=0`；正式评分按冻结 assessment 合同。
- 其他入口：[对话理解验收](docs/验收-对话理解与多人后台记忆.md)、[RN4](docs/performance/reading-notes-rn4-20261010/README.md)、[Tutor 原方案](docs/切片方案-Tutor预构建不阻塞教学.md)、[回合失败](SESSION_CHECKPOINT_TURN_FAILURES.md)。其他 checkpoint 原入口保持。
