# TO14：真实 Agent 对照与阅读案例

2026-10-12。TO14 的固定对照、证据判定、阅读案例和实现交接已完成。16 个预选任务的结果是 **13 通过、2 失败、1 未决**。本批没有测出工具误调用下降，累计输入增加；保存笔记的失败暴露了选区包装与动作识别之间的真实接线问题。

## 对照与执行

- 基线：`67efb605f5d45157eeb769f0074b218eacd76dba`，TO1 之前。
- 候选：`93b476da49b005def3f52b7b95bdaff4867e6e7b` 加采样前工作区的 TO3–TO13 改动；TO1–TO2 已在该提交历史中。两版源码、候选差异和相同测试适配器见 [source-manifest.json](source-manifest.json)、[candidate.patch](candidate.patch) 和 snapshots/。
- 模型：Codex CLI `0.162.0-alpha.2` 指定 `gpt-6.1-sol`，reasoning effort 为 medium。每次使用隔离、无持久会话的子进程；同一适配器、CLI、材料和任务输入用于两版。
- rainwater 复用原 MCP9 HTTP／SQLite／PublishedBookRef 夹具，Codex 自己调用真实 MCP。Resident 复用 TC2A 原首答和平均速度正文，由测试 ModelAdapter 把实际 AgentRequestPlan 交给 Codex，再把结构化决策送回原 Runtime／Server 执行。
- [冻结计划](PLAN.md)与[schedule.json](schedule.json)在运行前确定两版各8任务：雨水阅读各3次，Tutor继续各3次，保存笔记与纯解释各1次。重复按版本交错，无失败补抽。
- 原批次在13任务后达到1,536,902 token，按阈值停止。随后[单独登记补充批次](coverage-addendum.json)，只执行原计划里尚未启动的3任务，阈值400,000 token，实际217,325；未重跑任何已有样本。原[停止记录](batch-stop.json)保留。

两版编译通过，日志为 [compile-candidate.log](compile-candidate.log) 和 [compile-baseline-final.log](compile-baseline-final.log)。早期构建资产缺失、目录记录器接线和 CLI 恢复误判均保留在[故障记录](preflight-findings.md)，不计作产品参数错误。

## 实际交付

| 固定任务 | 基线 | 候选 | 实际观察 |
|---|---|---|---|
| 雨水阅读，各3次 | 3通过 | 3通过 | 都是目录→检索→两页正文，共4调用；37→19、减少18升、南池未参与；所引页实际读过，发布引用、UTF-16区间、3次消费及12→9余额全部相符 |
| Tutor继续，各3次 | 2通过、1执行器未决 | 3通过 | 每次均合法调用 book.text、tutor.step；5次真实 MessageDelivered，6份 move_selected；未发生工具参数拒绝 |
| 保存选区笔记，各1次 | 失败 | 失败 | 重开 MemoryStore 均无笔记。基线一次 memory_write 发现后无匹配；候选再尝试 reader_write，明确收到 explicit_effect_intent_required，再次发现仍不能保存 |
| 只解释选区，各1次 | 通过 | 通过 | 各一次 source.present；返回来源与已验证选区原文一致；无写调用、无 Reader effect、重开存储无新增 |

逐次判据、工具数量、实际 usage 和活动输入在 [summary.json](summary.json)。四类任务使用完成行为判定；仅有自然语言“完成”、保存了教学目标、或 HTTP 200 都不替代实际交付和持久记录。工具成功响应中的 blocked 能力原因另列，不能只用 `tool_rejections=0` 宣称任务无误。

基线 Tutor 第2次的 CLI 遇503后重连成功，但旧执行器错误地把中途 error 当终态，将已完成的答案丢弃。现已按最后 turn.completed／turn.failed 判终态，并保留重连记录。原样本仍为 unresolved/invalid_harness；[原决策无模型回放](diagnostic-replay/baseline-tutor-continue-2/result.json)可正常交付，不替换原结果。两版有效配对只取第1、3次，不把“3对2”的表面差异解释为产品提升。

数学检查使用独立正时间恒等式和四个有理数算例，并逐份核对真实答案与持久目标。5份真实交付及6份持久动作均未出现TC2A的“等时间是无例外必要条件”错误；[内容核对记录](content-review.md)保留原话和位置。

## 输入与耗时

下表的输入是每个用户任务中全部 CLI 模型请求的累计 input_tokens，包含缓存输入；它不是某一次请求的活动上下文。Tutor只比较两个完整有效配对。

| 任务 | 基线平均累计输入 | 候选平均累计输入 | 变化 | 平均耗时：基线→候选 |
|---|---:|---:|---:|---:|
| 雨水，各3次 | 135,917 | 148,677 | +9.39% | 54.15→57.52秒 |
| Tutor，第1与第3次 | 99,091.5 | 108,707.5 | +9.70% | 81.16→84.60秒 |
| 保存笔记，各1次且均失败 | 49,140 | 112,665 | +129.27% | 44.41→87.42秒 |
| 纯解释，各1次 | 49,116 | 54,321 | +10.60% | 40.40→41.81秒 |

全16任务实际累计为 **1,742,579 input + 11,648 output = 1,754,227 token**，其中704,000为已包含在input中的缓存token。执行器误丢弃的样本也计入消耗。实际货币费用未由宿主返回，保持 `null`。原始合计与配对集合见 [measurements.json](measurements.json)，补充批次消耗见 [coverage-usage.json](coverage-usage.json)。

Resident 首次活动输入的 Runtime 估算：Tutor 15,826→18,815；笔记9,792→12,284；纯解释9,839→12,331。对应首次实际工具定义分别为19,690→31,538、13,071→22,931、13,071→22,931字节。每次 active_context 单独保存在 request-XX.json；CLI计费输入还含执行器包装和宿主上下文，二者不能相减后当作纯工具开销。远程 MCP 没有 Provider 逐次活动输入记录，保持 null。

独立实际 tools/list 的11工具合计由27,809变为149,814字节。其中 output schema 增量116,951字节，占总增量122,005字节的95.9%；inputSchema.examples 的JSON从22变为1,047字节。目录字节不等于 Provider 活动token。完整目录在 [baseline](catalogs/baseline/tools-list.json) 与 [candidate](catalogs/candidate/tools-list.json)。

## 判断与处置

本批没有证明提高完成率、减少参数错误或减少调用。雨水与有效Tutor配对两版均没有参数错误，纯解释各一次，保存任务被真实产品缺陷阻断；在这些样本上增加的信息没有体现为更好的交付，累计输入和耗时增加。

本次是TO整组变更对照，不能把增量归因于某条示例。远程目录的大部分增加来自输出schema，Tutor还含状态及结果投影变化；零基线参数错误也不能识别哪一条示例无用。因此本轮不依据整组差值删除某条示例；保留此前已经验证的契约修正，性能收益维持未证实。若继续收缩投影，应固定出现误用的任务和对应单个示例做消融，然后仅回收已确定无收益的投影。

保存任务的最小修复入口是 `orchestrator::run_outer` 调用 `classify_turn_intent(question)` 的位置：Server 已提供原始 `context.current_user_message`，但该处识别的是选区包装。`action_request_text` 排除引号文本时把 JSON 字符串中的用户指令也排除了。后续使用现有原始用户字段推断动作，再覆盖“带选区明确保存成功”和“引用正文含保存命令但用户只解释时不授权”即可。证据见 [smoke-final](smoke-final/result.json) 和 [candidate-save-note-1](runs/candidate-save-note-1/result.json)。本次冻结产品版本未修改这条行为。

## 验证与复现

11项 Node 评测器测试通过，检测缺失usage、缓存重复计数、503恢复终态、固定分母、未读引用、错误数值、只有目标没有交付、Tool历史去重、无效执行器样本、错误笔记、被拒绝写操作及伪造选区来源。记录为 [evaluator-tests.log](evaluator-tests.log)。[input-verification.json](input-verification.json)逐值确认16份材料按题族一致、远程提示相同、CLI和模型配置相同。其余TO1–TO13未变路径沿用原确定性证据，完整对应关系见 [coverage.md](coverage.md)。

复核现有结果不会调用模型：

```powershell
pnpm eval:tools:test
pnpm eval:tools:report docs/performance/tool-usability/to14/runs
node evals/tool-usability/verify-inputs.mjs docs/performance/tool-usability/to14
python evals/tool-usability/math-oracle.py
```

重新构建任一冻结版本时，在 snapshots/baseline 或 snapshots/candidate 下使用下列相同配置。原试验候选由主工作区编译后复制，基线由快照编译；source-manifest记录的候选生产源码与编译时一致。

```powershell
$env:RUSTUP_TOOLCHAIN='1.95.0'
$env:CARGO_TARGET_DIR='D:/codex-build/to14-rebuild-baseline'
$env:CARGO_PROFILE_DEV_DEBUG='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
$env:CARGO_INCREMENTAL='0'
$env:CARGO_BUILD_JOBS='1'
$env:TEMP='D:/codex-build/to3-temp'
$env:TMP=$env:TEMP
cargo --config 'profile.test.package.runtime.codegen-units=16' test -p server --lib --no-run
```

构建candidate时将CARGO_TARGET_DIR改为 `D:/codex-build/to14-rebuild-candidate`；两个版本分别使用构建目录，避免旧依赖产物混用。将两次输出的 Server test executable 分别复制到不同路径，再使用同一CLI执行新批次。目标目录中的 runs/ 必须不存在；源快照保留，不覆盖本次样本。新批次消耗宿主额度，仍执行同一阈值；需要完成因批次阈值未启动的任务时，`finish-coverage.mjs <新目录>`会先登记独立补充批次且不重跑已存在目录。

```powershell
$env:TO14_BASELINE='D:/codex-build/to14-bin/baseline.exe'
$env:TO14_CANDIDATE='D:/codex-build/to14-bin/candidate.exe'
$env:TO14_CODEX='C:/Users/Lenovo/AppData/Local/OpenAI/Codex/bin/9691020b546a15b2/codex.exe'
node evals/tool-usability/run.mjs tmp/to14-new-batch
```

单任务的完整使用说明见[阅读案例](reading-case.md)。本轮没有部署生产服务、更新安装态插件或提交Git。

## 已知限制

- 真实模型覆盖两个题族、四类任务；Presentation、Executor、复杂Context及其余TO场景只有原确定性证据。本批结果不能外推为所有工具族的真实模型收益。
- Resident 使用结构化决策适配器，且每次采样是新Codex子进程，不等同于原生DeepSeek Provider。温度、后端精确模型修订和货币费用未由CLI暴露；Host缓存与网络重试影响耗时，少量样本不支持统计显著性结论。
- 基线Tutor第2次是执行器无效样本；原始固定分母保留16。补充批次在原预算停止之后登记，不能称为原1,500,000阈值内完成16任务。
- 数学算例与条件已经核对，教学顺序和真人学习效果仍为needs_review。TO5的EXISTING笔记完成判断与本次选区授权识别是两个独立问题。
- 临时reader.example来源用于夹具核对，宿主退出后不能作为在线阅读链接。CLI运行中既有hooks配置解析警告保留在stderr，未改宿主配置。
