# OM3：支撑方式与明确请求生效

日期：2026-10-20。状态：完成。代码基线：`33c19c7fa8aec9a4f1031122bc177743b234b9fd` 加本片工作树增量；其他工作线的既有修改保留。本片没有提交、推送或部署。

合同见[切片方案 §5.2、§6 OM3](../../../切片方案-交互教学与学习接续.md)，决策见 [ADR-0172](../../../adr/0172-interactive-teaching-state-and-learning-continuity.md)。

## 实现与入口

- [`tutor_writing_rules.md`](../../../../crates/runtime/src/tutor_writing_rules.md) 的 `plain-explanation.v2` 是主模型与审阅共用的支撑规则：当前明确请求优先；操作困难先给可执行方法；开放任务保留有理由的多种方案；短回复结合活动解释；受助表达不升级为独立掌握。
- [`tutor.rs::instruction_modules/review_instructions`](../../../../crates/runtime/src/tutor.rs) 沿原装配分别向主模型和审阅注入同一写作规约。`teaching-design.v12` 说明如何从 `continuity` 回读原作品、帮助和教学含义，使用实际 `basis_refs`／`interpretation_refs`，并在 `revision_reason` 中说明当前请求如何改变支撑。
- `tutor::spec/input_examples` 补充填行程表的方法、开放行程方案比较、当前要求深入推导三个完整示例。`revision_reason` 与 `teaching_implication` 的字段说明保持请求优先及原证据归属。
- [`orchestrator.rs::teaching_review_learner_view`](../../../../crates/runtime/src/orchestrator.rs) 将已有 `learning_opportunity.continuity` 纳入审阅输入。原来只选取设计、焦点和返回现场，审阅无法直接看到 OM2 的接续视图。原请求与偏好仍由 `frozen_input` 提供，原文与帮助仍沿 `learner_grounding` 回读。
- [`tutor_review_rules.md`](../../../../crates/runtime/src/tutor_review_rules.md) 的 `teaching-delivery.v16` 要求依据同一支撑规则审阅实际正文及原帮助条件；规则出现或引用齐全不自动构成接受理由。

没有新增学习状态、长期偏好存储或按关键词调整难度的逻辑。临时请求改变当前设计与支撑；历史事实、解释和帮助保留各自原身份。

## 固定场景

| 场景 | 链路与持久化断言 |
| --- | --- |
| 不会操作、要求简单示范 | 正式 prepare／design／select／review 交付填表方法，原话成为设计依据，没有新邀请、评分合同或独立能力记录 |
| 开放方案分析 | 当前约束进入主模型和审阅；交付比较框架，保留可兼用的方案，没有强制唯一答案或评分合同 |
| 受助表达后进入新问题 | 实际交付、展示、用户表达和解释存储后，下一轮沿用原作品与帮助；审阅收到原话、原正文和教学含义，原解释不被改写 |
| 历史简单偏好与当前深入请求冲突 | 已确认偏好仍传递；本轮以当前原话选择直接推导，后续 prepare 中原偏好保持原样 |

脚本使用既有 `TutorRequestRecorder` 和 `ChatStubAdapter`，运行真实来源读取、设计接纳、select 审阅、交付与 SQLite 存储。设置 `OM3_OUTPUT` 为本目录绝对路径时，保存实际 prepare、主模型输入、审阅输入、设计和最终交付。

实际输出：[操作门槛](operational-barrier.json)、[开放方案](open-plan.json)、[先前演示](assisted-setup.json)、[受助表达后的新问题](assisted-next-question.json)、[当前深入请求](current-depth.json)。每份同时保留公开交付与由其原因引用读取的私有选择记录；`participation` 等私有字段不会进入公开交付。

## 运行与结果

从仓库根目录复现：

```text
cargo test -p server --lib -- teaching_design_tests tutor_adaptation_tests tutor_response_fit_tests tutor_building_blocks_tests
cargo test -p server --lib -- tutor_to3_
cargo test -p runtime --lib -- tutor::
cargo test -p runtime --lib -- teaching_ math_structure_source_reading_reaches_active_tutor_review_and_delivery
cargo --config profile.test.package.runtime.codegen-units=16 test -p runtime --lib -- tool_registry::contract_tests
pnpm docs:tools
```

| 检查 | 检测目标 | 结果 |
| --- | --- | --- |
| Server 四组场景与相邻回归 | 当前请求、设计引用、原帮助和解释进入实际审阅与交付；既有教学接续行为保持 | 44 passed、0 failed、4 ignored；[日志](server-regression.log) |
| Server 发布示例 | 新示例与原示例走正式分发；UI 投影保持运行时绑定字段；评分与帮助仍使用原记录 | 5 passed、0 failed；[日志](server-examples.log) |
| Runtime Tutor 判定与工作记忆 | 请求原句、交付引文、来源与审阅结果归一仍保持原合同 | 16 passed、0 failed、1 ignored；[日志](runtime-regression.log) |
| Runtime 规则装配与教学循环 | 同一写作规则分别进入主模型和审阅一次；设计、审阅修订、选择与交付保持原顺序 | 40 passed、0 failed；[日志](runtime-loop.log) |
| Runtime 工具合同 | 新示例通过参数校验及 Native／ReAct 投影；各教学状态过滤与来源绑定保持一致 | 21 项先通过，总量断言修正后最后 1 项通过；[集合日志](runtime-contracts.log)、[最终定向日志](runtime-contracts-final.log) |

Server 合计 49 项、Runtime 合计 78 项通过，均按不同测试计数。默认忽略的五项分别需要手动导出、旧冻结材料或外部模型／审阅探针，本片没有启动。工具参考生成结果见[日志](tool-reference.log)，其中重复执行的 Tutor 状态投影不再次累计。

`pnpm docs:tools` 已成功生成 `docs/工具契约参考.md` 与 `docs/tool-contracts.json`，导出 Resident 43、Local MCP 16、Remote MCP 11、Executor 4 项工具和 17 个 Tutor 状态。生成同时通过实际工具导出及 Tutor 状态合同测试。

首轮四组场景均完成正式交付，两个用例错误地从公开交付中寻找私有 `participation`，导致断言失败。已沿交付的 `causal_refs` 读取真实私有选择并检查其内容，公开字段合同保持不变；初轮结果保留在[日志](server-om3.log)。测试源码通过 `include!` 或不同模块名装配，发布示例使用 `tutor_to3_`、运行循环使用 `teaching_` 筛选，不能只按文件名假定覆盖。

首次工具导出由旧示例数量断言阻断，见[初次导出日志](tool-reference-initial.log)。同步三个新增示例的状态期望后，完整合同验证又发现总量断言此前已过期：本片修改前的工具参考有 61 个示例，测试期望仍为 60；本片新增三个后应为 64。已更新 `tool_contract_tests.rs` 的对应期望，保留逐条 schema、实际参数解析和 provider 投影验证；最终只重跑这一失败项。

## 已知限制

固定回答与审阅接受意见由确定性替身提供；这些测试证明输入传递、引用归属和存储行为，不证明模型遵守教学规则。真实模型对照及完整 Reader 教学闭环属于 OM6；教师实时操作仍从 OM4 开始。
