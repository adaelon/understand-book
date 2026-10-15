# TC2A — 按当前任务与状态加载教学指令

日期：2026-10-11（Asia/Hong_Kong）。依据 [TC2A 切片方案](../切片方案-Tutor下一步协商与低门槛阅读入口.md#tc2a按当前任务与状态加载教学指令)及 [ADR-0163 §9](../adr/0163-tutor-continuation-and-contextual-reading-entry.md#9-按需教学指导tc2a)。

状态：**TC2A 装配实现、定向回归和六次固定输入观察完成；真实模型稳定交付与内容质量未通过，继续列为 TC2 未完成项。** 基于 `12b765bb33fe42fcd30fcda743187241ed56d1f3` 的工作树，保留 TC2 及其他既有修改。本片未提交、未部署。

后续复验：2026-10-11 无进展修复后的同一固定输入 **3／3 完整教学交付**，其中两次经过参数纠正。持久目标或正文的条件概括仍有问题，TC2 内容与读者验收继续保留。原六次对照数据保持原样；新增结果见[修复后复验](tutor-continuation-tc2a/recovery-20261011/README.md)。

## 验证目的

请求回归识别未使用流程残留或必要指导不可达，失败时修正装配条件；行为回归识别理解修订、正式评估、导航与暂停受损，失败时修正对应触发或状态更新。真实模型对照分别观察持久目标与公开回答的数学条件，出现错误即保留到 TC2 内容问题，不追加针对算例的提示或重复求通过。

## 指导与状态的装配

- `tutor::instructions` 投影一次冻结数据，移除相同访问路径中的重复值；完整原回答、来源范围、有效理解和必要状态保留。
- `tutor::instruction_modules` 提供共用证据约束、短发现入口、动作选择、理解更新、正式评估及演示三个阶段的教学要求。真实表达／理解操作展开更新；active 展开动作选择；真实 pending open 提交展开评估；成功评估后下一采样撤下评估流程。
- `learner_context` 按原同会话、同来源版本事实及冻结评分合同投影待评估引用。UI 点击、普通聊天、已评估回应和封闭题不产生此入口。
- 导航短入口常驻，既有预激活保证显式带读首轮具备完整流程；延迟发现后下一采样展开。结构读取本身不加载路线流程。
- 原 `ContextFragmentLedger` 在采样位置记录当前指导选择，旧选择随后失效。暂停移除教学指导和工具；演示交付撤下其阶段指导。工具参数及校验、select 先保存目标、最终交付、帮助与理解证据路径保持原合同。

## 实际请求与行为验证

新增的两项请求测试在旧实现上分别失败，见 [red.log](tutor-continuation-tc2a/red.log)。Runtime 最终定向回归 **59 通过、1 默认忽略**，见 [runtime-final.log](tutor-continuation-tc2a/runtime-final.log)。覆盖导航预激活／延迟发现、结构路线与单停靠点 Goto、演示发现／阶段／交付、暂停及冻结上下文不重复。第一次 Runtime 运行仅有旧失效文字断言不匹配，更新为通用工作流语义后通过，原记录保留于 [runtime.log](tutor-continuation-tc2a/runtime.log)。

Server Tutor 回归 **66 通过、9 默认忽略**，见 [server.log](tutor-continuation-tc2a/server.log)。覆盖原来源／权限、理解修订、正式评估、实际交付与本地／多人接纳路径；新增请求验证明确检查无关完整流程缺席、理解回读后指导展开且工具权限不变、真实纠正采用、正式评估后指导失效。固定输入模型测试从默认忽略项中单独运行。

复现定向回归：

```powershell
$env:CARGO_TARGET_DIR='C:/Users/Lenovo/.codex/tmp/understand-book-tc2-target'
$env:CARGO_PROFILE_DEV_DEBUG='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
$env:CARGO_INCREMENTAL='0'
$env:TEMP=(Resolve-Path 'tmp/tutor-continuation-tc2').Path
$env:TMP=$env:TEMP
cargo test -j 1 -p runtime --lib -- tutor agent_prompt guided_read context_fragment --test-threads=1
cargo test -j 1 -p server --lib tutor -- --test-threads=1
```

## 真实模型对照设置

现状版与按需版分别运行三次 `continue_answer`。同一 DeepSeek V4 Flash、temperature=0、原平均速度材料、TC2 最终样本的原首答、空补充和现有工具合同。原首答由替身一次性写入隔离存储；六个接续由真实模型执行。模型仍自行选择合法工具，生产实现仍先 select 保存目标，再公开交付。

每次记录 `request-XX.json` 的实际 instructions、input、指令资产、完整工具定义、运行时配置、单次估算输入 token、输出预留与工具选择模式；`response-XX.json` 保存实际模型回答和工具参数；`result.json` 保存结果、正式事件与交付。现状指导源码和固定原回答位于 [baseline-assets](tutor-continuation-tc2a/baseline-assets/)，现状三次原始运行见 [baseline](tutor-continuation-tc2a/baseline/)。

`summarize.py` 直接比较配置与实际工具定义，不使用摘要替代内容比对；单次输入与累计 provider usage 分开记录。数学核对使用两段、总时间为正时的恒等式：时间加权平均与算术平均相等，当且仅当 `(v₁−v₂)(t₁−t₂)=0`。分别核对等速不等时、等时不等速、原不等时算例及等速等距离。

## 输入量与实际交付

模型、temperature、材料、固定原回答与接续动作配置直接比对一致。读原文前／后的两套业务采样工具定义也完全一致，目标仍先保存后交付。按需版第 1 次失败后多出一次无工具终答，因此全部采样工具集合的总体比对为 false；这次额外路径及成本完整保留，不能把它当正常教学完成。详见 [summary.json](tutor-continuation-tc2a/summary.json)。

所有首个请求的估算输入从 **17,250 降至 14,020 token，减少 3,230（18.72%）**。实际请求中不再出现完整导航、正式评估、理解更新或演示流程；共用边界、发现入口、动作选择、完整来源与原回答仍在。

| 组／次 | 每次采样的估算输入 token | 累计 tokens_spent | 已保存目标／教学交付 | 工具拒绝 | incomplete |
|---|---|---:|---:|---:|---|
| [现状 1](tutor-continuation-tc2a/baseline/run-1/result.json) | 17,250 / 20,970 / 23,292 | 61,633 | 1 / 1 | 0 | false |
| [现状 2](tutor-continuation-tc2a/baseline/run-2/result.json) | 17,250 / 20,347 / 22,544 | 59,693 | 1 / 1 | 0 | false |
| [现状 3](tutor-continuation-tc2a/baseline/run-3/result.json) | 17,250 / 19,888 / 21,272 / 25,239 / 26,872 | 107,793 | 1 / 1 | 1 | false |
| [按需 1](tutor-continuation-tc2a/on-demand/run-1/result.json) | 14,020 / 16,473 / 19,159 / 12,792〔无工具终答〕 | 60,385 | 0 / 0 | 2 | true |
| [按需 2](tutor-continuation-tc2a/on-demand/run-2/result.json) | 14,020 / 16,950 / 19,803 / 20,849 | 70,685 | 1 / 1 | 1 | false |
| [按需 3](tutor-continuation-tc2a/on-demand/run-3/result.json) | 14,020 / 16,749 / 20,696 / 21,851 | 73,871 | 1 / 1 | 1 | false |

现状累计 **229,119**，按需累计 **204,941**，共 **434,060** token；它们是跨请求 usage，不是上下文窗口或货币费用。按需版有一次未完整教学，两组调用轨迹也不同，此累计差值不能作为完成同一任务的成本改善结论。

现状第 3 次把 move 写成字符串，被校验拒绝后重试成功。按需第 1 次先漏传 move，随后尝试评分活动却未开放 submit，两次均被原边界拒绝；聊天保存了终答，但没有 TeachingMove／MessageDelivered，incomplete=true。按需第 2、3 次分别把 move 字段放到顶层、漏传 move，重试后成功。原参数合同未更改，拒绝未被放宽。样本保存后停止，没有为消除失败追加模型采样。

实模运行日志：[现状](tutor-continuation-tc2a/baseline.log)、[按需](tutor-continuation-tc2a/on-demand.log)。两次显式测试入口各执行 3 个接续，保存 HTTP 结果与事实，**测试入口退出成功不代表其中每次教学完整**。当前源码重跑入口为 `continuation_tc2a_live_fixed_input_comparison --ignored --nocapture --test-threads=1`，需设置全新 `TC2A_OUTPUT`；原现状使用改动前已编译的隔离测试程序。模型参数见各组 `configuration.json`，按需指导源码见 [on-demand-assets](tutor-continuation-tc2a/on-demand-assets/)。

## 目标与公开正文的数学核对

确定性算例见 [counterexamples.json](tutor-continuation-tc2a/counterexamples.json)：速度 10／10、时间 1／2 时两种平均均为 10；速度 10／20、时间 1／1 时均为 15；原 60／30、时间 1／2 时为 40 与 45；等速等距离同样取等号。原输出逐字引用与发现见 [content-findings.json](tutor-continuation-tc2a/content-findings.json)，每个运行目录另保存 `answer.md` 与 `persisted-moves.json`。

| 样本 | 持久目标 | 公开正文 |
|---|---|---|
| 现状 1 | 按时间占比解释及比较具体情形，没有误写等时必要条件 | 加权式、40 与 45 算例正确；等权重说明正确，未覆盖等速不等时边界 |
| 现状 2 | 要求时间不等时不用算术平均，漏掉等速情形 | 明确写“只有时间相等时，两者才重合”，被等速不等时反例否定 |
| 现状 3 | 将路程相等概括为简单平均失效 | 把“不必等时”改成“必然不等时”，等速等距离可否定 |
| 按需 1 | 无持久目标，不能用被拒绝参数替代 | 算例正确，仍声称只有等时才成立；先承认等速取等号，又写等时间结果总是更大 |
| 按需 2 | learning_focus 仍写“时间相等时才退化” | 算例、调和平均与等速取等号正确，正文边界比保存的目标完整 |
| 按需 3 | 仍用“等时间时才等于算术平均”概括 | 算例正确，但由时间可以不等就否定直接平均，遗漏等速不等时 |

这些结果没有证明减少指导消除了数学条件错误，也没有证明目标先提交是错误原因。目标提交时序保持原样。

## 已知限制

- 真实模型仍有工具参数失误和一次未完整教学，内容的必要条件概括及目标／正文不一致继续留在 TC2；后续应独立定位具体失败。
- 六次是本场景观察，不是通用准确率、稳定性或学习效果评估；真实读者人数仍为 0。
- 本片未改前端、持久数据类型、证据语义或目标提交时序；TC3 continuation 尚未开放。完整宿主恢复矩阵、移动端与降级演练仍沿原 TC4／TC7／TC8 计划。
- 本轮运行了导航、正式评估和演示状态的确定性回归；独立真实带读模型回放、真实评估模型测试和浏览器宿主测试未重跑。既有 MU10 两项失败不在本片修复范围。
