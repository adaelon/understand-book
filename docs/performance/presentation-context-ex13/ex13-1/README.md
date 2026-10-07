# EX13.1 稳定共同指导与采样位置追加

2026-10-02。**已完成实现及离线验证**。Runtime 全库 440 项通过、3 项按原条件忽略；制作链路最终复验 10 项通过；比较脚本 10 项通过。新增模型调用 0，usage 为 null。合同见 [EX13.1](../../../切片方案-EX13-Goal工作计划与演示修订上下文.md#ex131稳定共同指导把变更移到后部)，设计依据 [ADR-0155 §3](../../../adr/0155-goal-work-plan-and-version-centered-presentation-context.md#3-指导组装与缓存)。

## 请求与生命周期

`agent_prompt::presentation` 分开共同模块和 phase/reference 模块，资源正文继续使用 `ex12.v5`。`build_sample_request` 将共同模块放入 instructions，将当前选择交给 `ContextFragmentLedger::record_sampling`：

```text
完整工具调用/结果组
  → 本次 runtime_state_snapshot
  → 选择实际变化时追加 sampling_guidance（system，固定资源）
  → presentation_guidance_state（system，当前 phase/needs/设计是否有效）
  → 当前 presentation.authoring_context（user，framework/focus）
  → 模型采样
```

旧消息保留原采样位置；最新选择取代旧职责和技术参考。相同选择、needs 去重重排及仅修改设计文字不重新追加指导正文。`instruction_assets` 记录共同模块和当前有效选择，历史指导留在其原消息中。设计数据每次按当前状态投影，历史设计不作为当前记录；它不能授予来源、工具、候选或交付资格。

成功交付清除设计，并将运行内制作标为结束；下一次采样发送失效选择，不回退 global。后续成功的 prepare 或其他合法制作动作重新启用当时选择；无显式上下文时使用默认 global。取消清除设计并结束制作，新 Run 使用空状态。禁用工具的最终收尾采样也停用阶段指导。

中途压缩保留当前回合的完整后缀，指导及状态按原锚点恢复。预算检查和压缩后的同次请求重建复用该采样记录；下一次决策重新记录状态。Goal 拒绝提前终答时，即使原始消息数量未增加，仍得到新预算和交付缺口。这条既有回归曾检出本片的去重错误，修复后通过。

## 受控请求与前缀结果

受控 ModelAdapter 在同一循环执行 discovery → global → local/state → 重复选择 → 只改 framework/focus → local/editing → write → preview → review → deliver → 新 write。`before/` 在生产改动前捕获，`after/` 为实现后的同一输入；各有 11 组 request、Native 和 ReAct JSON 投影。

`request-*.json` 保存计划中的 instructions、当前资产、profile、工具及输入消息；images 从同组 Native 投影末尾的多模态内容取得。`native-*.json` 和 `react-*.json` 由实际适配器序列化函数产生，包含工具名映射和模拟预览图片；没有发送 HTTP。原 EX12.4 录制及 [baseline.json](../baseline.json) 保持不变。

| 变化 | 改动前首差 | 改动后首差 | Native 既有文本消息保留 |
| --- | --- | --- | --- |
| global → local/state | instructions 字符 10,441 / UTF-8 字节 11,582 | messages[8] 新增 | 9/9 |
| 重复选择 | messages[9].content 被替换 | messages[14] 新增 | 15/15 |
| 仅改设计 | messages[12].content 被替换 | messages[19] 新增 | 20/20 |
| local/state → local/editing | instructions 字符 10,960 / 字节 12,971 | messages[24] 新增 | 25/25 |
| 预览后 → review | instructions 字符 10,441 / 字节 11,582 | messages[40] 新增 | 41/41 |
| review → 交付停用 | instructions 字符 10,441 / 字节 11,582 | messages[46] 新增 | 47/47 |
| 新制作动作 | messages[30] 新增 | messages[51] 新增 | 52/52 |

首次能力激活后，9 个相邻采样对的共同 instructions 与工具集合均一致；Native、ReAct 各 9 对全部保留之前的文本消息。旧版首次切阶段时，Native 的文本消息前缀为 0；新版为 9。预览图片是一次采样的尾部观察，下一次不会重复附带；表中将这条图片消息与文本历史分开，完整差异仍保留在统计中。

指导事件数从 global 的 1 增至 local/state 的 2；重复选择和设计修改后仍为 2；needs 切换、review、交付失效、新工作分别增加到 3、4、5、6。工具回执先于对应新指导；设计文字未进入任何 system 消息。

同一 local/state 采样的改动前后比较保存在 [local-before-after.json](local-before-after.json)，剥离阶段指导后的共同 instructions 完全相等。完整逐对比较、分类体积及两种协议结果见 [comparison.json](comparison.json)。

## 重放与测试

EX13.1 的 before/after 已冻结。EX13.2 后测试允许成功保存源码的明确裁剪边界，当前代码的新录制应另存 EX13.2 目录；下面的 summarize 继续读取原冻结材料。在仓库根目录执行：

```powershell
cargo test -p runtime --lib presentation_authoring
python docs/performance/presentation-context-ex13/ex13-1/summarize.py
python docs/performance/presentation-context-ex13/compare.py compare docs/performance/presentation-context-ex13/ex13-1/before/request-02.json docs/performance/presentation-context-ex13/ex13-1/after/request-02.json --output docs/performance/presentation-context-ex13/ex13-1/local-before-after.json
python -m unittest discover -s docs/performance/presentation-context-ex13 -p test_compare.py -v
```

- `presentation_authoring_tests::ex13_guidance_changes_append_after_results_and_keep_provider_prefix`：最终 Native/ReAct 请求前缀、事件次数、每次状态、设计权限、交付失效和新工作启用；改动前先红，改动后通过。
- 原制作链路用例：下一次采样生效、重复 prepare 无进展、框架不授予来源或交付、取消与新 Run、当前候选图片观察。压缩用例加强为“local 指导已经采样后才压缩”，验证指导只恢复一次且当前框架仍在。
- `context_fragment::sampling_rebuild_keeps_guidance_once_and_task_data_at_its_role`：同次压缩重建不重复指导；相同消息位置的新决策仍记录；user 动态数据保持原角色。
- `agent_prompt::presentation::common_modules_are_identical_across_sampling_selections`：共同模块与选择无关，原技术依赖测试保持通过。
- Runtime 全库 `cargo test -p runtime --lib`：440 通过、0 失败、3 忽略，涵盖 Goal 补交付、预算、压缩、工具发现与提供方序列化；最终组装的小幅整理后，制作链路 10 项再次通过。
- 比较脚本 10 项通过，原 EX13.0 冻结统计逐项一致；新增覆盖后部指导分类和没有 source.present 的受支持工具组合。

## 已知限制

本片验证共同前缀与时序，不产生真实缓存命中或费用结论。保留旧指导和逐次状态会增加历史体积：受控样本最后一次请求从 50,547 增至 67,009 个 UTF-8 序列化字节。它们是录制字段的体积，包含模拟图片，不能作为 HTTP 大小或 token。源码裁剪、Goal 工作计划和版本修订输入仍分别由 EX13.2–EX13.4 处理。

工具首次发现、实际结果裁剪、历史压缩和一次性图片仍可能改变请求前缀。EX12.4 最终原生验收仍被提供方 402 阻塞，EX12.5 未开始。本次未部署。
