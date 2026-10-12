# TB2：GPT-6.1-Sol 教学内容对照

2026-10-12，四组前后各一次，另补初学者一组，共 9 个独立子代理。全部输出已通过真实 Tutor 选择、来源编译、交付及持久化回放。本轮 DeepSeek 调用为 0；既有 DeepSeek 失败及用量保留在 [TB2 记录](../README.md)。

## 输入与执行

使用 `gpt-6.1-sol`，每次 `fork_turns=none`，推理覆盖均省略，采用该模型默认设置；温度未由子代理工具提供。每个子代理只读自己的 `input.json`，只写自己的 `output.json`，没有提供预期答案、其他组输出或父会话历史。调用端统一要求返回 `select` 与实际 `answer`，不生成读者后续回应。

原文、背景和问题逐字沿 TB1。共同工具说明取自已记录 TO3 请求的 `tutor.step`，五条 `source.present` 引用及完整原文已提供；本轮直接比较教学选择与内容。`before` 使用 tc6.v1 的五个活动模块，`after` 使用 tb2.v3；工具说明、来源、初始状态、外层任务指令均相同。[prepare-sol.py](../prepare-sol.py) 可重建输入。

初学者 v3 输出只给出所有权转移，没有说明旧绑定此后的可用性。tb2.v4 只补充“前提规则要说明变化后哪些操作仍可用”的指导；[prerequisite-repair](prerequisite-repair/starting-assignment/input.json) 仅补跑这一组，原 v3 输出保留。其他三组已具备所需前提，沿用其 v3 结果。

各次任务为 `sol_before_assignment`、`sol_before_ownership`、`sol_before_misconception`、`sol_before_direct`、`sol_after_assignment`、`sol_after_ownership`、`sol_after_misconception`、`sol_after_direct`、`sol_repair_assignment`。每组的 `input.json`、`output.json`、`answer.md`、`replay.json` 和每版本 `tutor.rs` 均保存。[summary.json](summary.json) 记录输入配对、意图与来源检查；Codex 实际 token 用量和金额未由此子代理工具返回，记为未知。

## 内容结果

| 场景 | 旧指导 tc6.v1 | 改后与补跑 | 依据 |
|---|---|---|---|
| 只懂整数赋值 | 意图误标直接讲解；邀请判断但没有非 Copy 移动规则 | v3 改为引导并补 Copy 条件，仍缺旧绑定可用性；v4 补全规则，另用 Parcel 示范，原例判断留给读者 | [旧](before/starting-assignment/answer.md)、[v3](after/starting-assignment/answer.md)、[v4](prerequisite-repair/starting-assignment/answer.md) |
| 已懂所有权 | 已能复用 move 规则，但意图误标直接讲解 | 保留已有基础，围绕赋值右侧产生 Foo 还是 &Foo 组织关系；意图为引导，留下类型、所有者和打印行的判断 | [旧](before/starting-ownership/answer.md)、[改后](after/starting-ownership/answer.md) |
| 引用等于复制 | 已能纠正复制推断并邀请复述 | 纠正 `&foo = 复制 Foo` 的具体连接，用一份值和一个引用的关系图支持读者解释两行打印 | [旧](before/misconception-copy/answer.md)、[改后](after/misconception-copy/answer.md) |
| 直接讲解 | 覆盖五点，直接讲解意图正确 | 保留完整解释，说明非 Copy 条件、去掉 & 后第一条打印失效及不能经共享引用写入；无先行答题要求 | [旧](before/direct-explanation/answer.md)、[改后](after/direct-explanation/answer.md) |

这是固定样例的内容观察。旧指导已有成立的所有权支撑、误解纠正和直接讲解，改后保留；测出的改进集中在混合请求意图和初学者缺失的前提规则。

内容事实沿 [TB1 编译结果](../../tutor-building-blocks-tb1/content-check.json)：原例打印两行 Foo(0)，去掉 & 后使用 foo 为 E0382，经共享引用写入为 E0594。新增 [Copy 反例](../copy-example.rs) 编译与执行成功，见 [结果](../copy-check.json)：结构体实现 Copy 后，直接赋值仍可使用两个绑定，说明判断条件是 Copy 能力。

## 工程验证

`tutor_tb2_sol_outputs_replay_through_real_delivery` 为每个样例建全新夹具，真实执行 `book.text → source.present → tutor.step select → 最终交付`。只将记录中的来源 ID 按同一原文 quote 替换为当前回合真实注册的 ID；教学内容与动作字段保持模型原输出。检查一次 MoveSelected、一次 MessageDelivered、有可打开来源、无原始来源标记、无学习能力证据新增。每组 `replay.json` 保存实际结果；回放不调用外部 Provider。

`summarize-sol.py` 检测前后输入或工具不一致、意图与请求不符、未知来源引用、残留标记及交付失败；出现时修相应输入配对、指导或交付路径，再只跑受影响项。9 组的输入、意图（旧指导的失败单列）、引用及回放已核对。

## 已知限制

子代理直接获得原文和已注册来源；读取与引用注册顺序由回放夹具固定，因此本轮不测模型自主工具搜索、实际 Provider 协议或 DeepSeek 表现。它验证当前工程路径能交付这些真实模型内容，不能充作实时 Provider 全链路通过。

每组一次，不推断成功率或稳定成本收益。真实读者为 0，无掌握或迁移结论。前后均保留 Codex 子代理自身的系统环境；与 DeepSeek 历史输出分开报告。
