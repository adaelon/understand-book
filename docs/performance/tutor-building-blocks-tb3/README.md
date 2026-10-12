# TB3：帮助调整与学习记忆接续

2026-10-12，TB3 已完成工程与 GPT-6.1-Sol 教学内容验收，指导版本 `tb3.v1`，未提交、未部署。四组有效回应及四组重开接续通过真实工具、存储和回答交付路径。四种学习者回应均为脚本夹具，真实读者 0。

## 实施步骤

- [x] 固定 TB2 交付后的难以开始、理由缺失、支撑不适用、局部独立运用四组回应；保存改前指导与实际上下文。
- [x] 完成回应适配指导、事实回读与理解视图分页修复，运行受影响回归。
- [x] 完成 12 次固定实验及用户另行授权的 4 次修复；保存原始失败、最终交付和重开接续，更新方案、链路和 checkpoint。

## 实现与事实链路

`runtime/tutor.rs:SELECT/UNDERSTANDING` 根据当前回应选择补小步示范、补理由、换表示或减少提示；保留先前帮助对本次表现的影响，并要求后续交付采用修订后的理解。普通对话中空的 assistance 列表不能证明独立表现，继续请求本身不产生新的能力证据。

发现并修复两处正常使用中的数据缺口：

- 理解查询原来固定只取前 20 条，且只按对象行数决定能否翻页；仅有来源的理解记录会不可发现。Memory 先过滤已被替代记录再分页，Server 用现有 `after` 同时分页对象与理解；工具摘要限制长文本并标明截断，`evidence_ref` 保留精确分块读取。Web 累积理解页，纠正后重载已浏览范围。
- 用户看过提示后通过带教学引用的普通聊天回应，同时生成 UsageObserved 与 LearnerAction；引用前者形成假设时会遗漏后者已冻结的帮助。Server 从同回合原 LearnerAction 继承当时的 assistance，保留来源、教学会话和聊天归属。用户在求助后切换教法仍保留原帮助。

原话、帮助、尝试、修订和交付仍使用现有记录；普通表达保持 unassessed，不写入掌握投影。重开验证重新加载私人 MemoryStore 和聊天历史，在同一有效 TutorSession 下新建聊天，再实际交付下一步。

## 固定输入与内容结果

[scenarios.json](scenarios.json) 指向 TB2 四份最终输出；[before-tutor.rs](before-tutor.rs) 冻结 `tb2.v4`。改前和改后共用同一 TO3 工具说明、原文、来源回执、请求与实际上下文，只有指导变化；每份配对输入增加 2,175 UTF-8 字节。[summary.json](summary.json) 逐份列出输入文件字节、字段、原始错误和真实交付状态。

| 场景 | 改前观察 | 改后有效交付 | 重开实际采用 |
|---|---|---|---|
| 难以开始 | 已用非 Copy 的 Parcel 示范，再留一个打印判断 | 将“拥有值→能够读取→能够打印”连起来，仍只留一步 | 沿上次 Parcel 支撑继续原判断，没有把未作答当作掌握 |
| 结论对、理由缺失 | 已补类型／所有权关系，再请用户解释 | 明确认可选对的结论，补上“移动不改变类型”后留下因果解释 | 采用已选对但理由未知的记录，接续该因果链 |
| 支撑不适用 | 已按请求换为逐行代码和类型表 | 修订复制误解假设，采用用户能跟上的表示，留下两个打印的解释 | 继续使用逐行／类型表示，没有复活旧误解 |
| 局部表现足够 | 已给 Copy 与共享引用组合的新条件题 | 明确区分已讲解的关系复述和新增 Copy 推论，让用户自选方法，不给结论 | 沿未回答的变式继续，保持少提示、无新能力记录 |

改前已具备上述适配行为，不能据这四次单样本宣称普遍效果提升。TB3 的工程变化解决了事实可发现性和帮助遗漏；教学指导更明确地约束适配判断及独立性的解释。独立组是“此前得到完整解释后作出的局部推论”，不是已证明完全独立迁移。[content-check.json](content-check.json) 保存实际生成 Copy 共享引用写字段代码的 Rust 编译结果 E0594，与所给任务条件一致。

最终可读交付：

| 场景 | 回应后 | 重开后 |
|---|---|---|
| 难以开始 | [正文](sol/after/cannot-start/answer.md) | [正文](sol/reopen-repair/cannot-start/answer.md) |
| 理由缺失 | [正文](sol/after/missing-reason/answer.md) | [正文](sol/reopen/missing-reason/answer.md) |
| 支撑不适用 | [正文](sol/repair/support-mismatch/answer.md) | [正文](sol/reopen/support-mismatch/answer.md) |
| 局部独立运用 | [正文](sol/repair/independent-step/answer.md) | [正文](sol/reopen-repair/independent-step/answer.md) |

每个目录保留 `input.json / output.json / replay.json`；`answer.md` 从输出原样提取。回放只按相同原句与不可变事实内容重绑隔离夹具的引用 ID，不改写教学文本。四份重开输出均复用既有 interpretation_refs，`evidence=null`。

支撑不适用组的 `interpretation:tb3-prior-copy` 是测试依据 TB2 原始用户表达写入的旧假设，用于检验 supersedes；它不是 TB2 模型自然生成的记录。其余原始交付来自实际 TB2 输出，回应和继续请求均有脚本身份。

## 运行与修复记录

用户依次授权 12 次固定实验、改后 2 次参数修复、重开 2 次参数修复，共 16 次 GPT-6.1-Sol 调用，全部完成。`fork_turns=none`，推理设置沿模型默认，未覆盖；各子代理只读各自冻结输入、只写各自输出。DeepSeek 新增调用 0。

| 批次 | 数量 | 实际交付 |
|---|---:|---|
| before | 4 | 2 份通过；支撑不适用和独立组参数被拒绝 |
| after | 4 | 2 份通过；相同两组参数被拒绝 |
| repair | 2 | 2 份通过，构成改后四份有效交付 |
| reopen | 4 | 理由缺失与支撑不适用通过；难以开始和独立组参数被拒绝 |
| reopen-repair | 2 | 2 份通过，构成四份有效重开交付 |

原始失败包括将 `supersedes` 字符串写成数组，导致随后选择找不到解释；将 `intent_origin` 写成未定义的 `user_request`。修复输入带回实际拒绝，改用 `current_request` 并补原话引用，未增加产品端容错。所有正文、动作 prompt 和教学判断在修复中保持。支撑不适用组还删去了一个原本合法的可选 delivery fact_ref；这不是修复类型错误所必需，单列为修复偏差。用户原话引用保留，原交付仍在上下文和记录中，完整字段差异列于 summary 的 `repair_changed_fields`。

工具未提供本轮模型 token、金额和调用耗时，均记未知；输入文件字节不等于 Provider 活动输入或计费 token。既有 DeepSeek 1,032,768 token 未结算预占保持原值，历史见 [TB2](../tutor-building-blocks-tb2/README.md)。

## 工程验证与复现

| 验证 | 结果／证据 |
|---|---|
| 原缺口回归 | [paging-red.log](paging-red.log)、[help-red.log](help-red.log)、[web-red.log](web-red.log) 先复现丢失，再修复 |
| Runtime Tutor | 29 项通过，[runtime.log](runtime.log)；覆盖最终请求的状态、意图及指导装配 |
| Memory 证据／投影 | 1 项通过，[memory.log](memory.log) |
| Server Tutor | 110 项通过，15 项默认忽略，[server.log](server.log)；包含长事实／多页修订及已展示帮助继承 |
| 理解视图与类型 | 10 项通过，[web-green.log](web-green.log)；[web-types.log](web-types.log) 类型检查通过 |
| 本地冻结输出回放 | [initial-replay.log](initial-replay.log) 捕获原始成败；[response-replay.log](response-replay.log) 验证回应交付；[reopen-initial.log](reopen-initial.log) 捕获重开成败；[reopen-final.log](reopen-final.log) 验证最终重开交付 |
| 内容与配对 | [content-check.json](content-check.json) E0594；[pairing-final.log](pairing-final.log) 配对输入、修复文字及最终八份交付核对通过 |

收口检查通过：本次差异无空白错误；实施记录、方案、ADR 与 checkpoint 的 80 个本地文件链接有效，账本 TB3 入口有效；checkpoint 共 47 行，状态与下一片 TB4 一致。检查回执保存在本目录 `handoff-check.json`。

工程验证使用 Rust +1.95.0，`CARGO_TARGET_DIR=D:/codex-build/understand-book-tb-195`，dev/test debug=0、incremental=0、`-j 2`，TEMP/TMP=`D:/codex-build/tb1-temp`。工程入口：

```powershell
cargo +1.95.0 test -p runtime --lib -j 2 tutor
cargo +1.95.0 test -p memory --lib -j 2 learning_evidence
cargo +1.95.0 test -p server --lib -j 2 tutor_
pnpm --filter @understand-book/web test src/components/UnderstandingSpace.test.ts
pnpm --filter @understand-book/web typecheck
```

Server 宽过滤器会运行既有 TB2 冻结回放并重写其回执；本轮运行前备份、运行后恢复 TB2 原回执。只复验 TB3 时过滤 `tutor_tb3_`。下列冻结回放使用本地模型适配器，外部模型调用为 0：

```powershell
$env:TB3_PHASE='reopen'
cargo +1.95.0 test -p server --lib -j 2 tutor_tb3_sol_replay_and_reopen -- --ignored --nocapture --test-threads=1
python docs/performance/tutor-building-blocks-tb3/summarize.py --final
```

`TB3_PHASE=initial/response/reopen-initial/reopen` 分别捕获原始回应、验收有效回应并导出重开上下文、捕获原始重开、验收最终重开。回放会刷新 TB3 对应回执与上下文，不覆盖原模型输出。`tutor_tb3_export_response_contexts` 导出初始夹具；`prepare-sol.py before/after/reopen` 生成各阶段输入，`prepare-repair.py response/reopen` 捕获原始拒绝。它们是本地准备工具，新增模型实验需另行固定次数与授权；原始冻结输入不应在后续源码上重建后冒充本轮输入。

## 检查目的与失败处置

- `tutor_tb3_` 工程测试：检测帮助事实或原话丢失、修订后仍采纳旧解释、同一有效会话重开后记忆未消费；失败修 Server 原记录／查询或 Runtime 指导。
- 长事实与多解释回读：检测摘要截断不能取得全文、20 条之外的理解记录不可发现、翻页丢失或重现被替代解释；失败修既有查询和分页消费。
- Runtime 最终请求：检测回应指导装配到错误状态、主动教学覆盖关闭／直接请求、无关评估或演示指导重复进入请求；失败修按需装配。
- Sol 教学实验：检测支撑没有根据回应改变、把已展示内容或脚本自评当作独立能力、纠正未生效、重开后重复原支撑；保存失败，只按内容依据判定实际缺口。
- 交付回放：检测引用、解释引用、选择与终答无法通过真实存储路径；失败修生成合同或工程路径，不改写模型输出冒充通过。
- 输入配对与修复核对：检测改前／改后的任务、工具和事实不同，或参数修复偷偷改变教学文本；失败修实验捕获并单列不可比较结果。
- Copy 共享引用变式：编译模型实际给出的完整代码，检测赋予 Copy 后通过共享引用写字段是否真的仍被拒绝；若不是 E0594，重新核对条件并保留内容失败。
- 收口核对：检测本次差异的空白错误、交接文件缺失、checkpoint 遗漏未完成项；失败修对应文件。

## 已知限制

预读原文与固定工具回放用于内容及交付验收，不验证生产 Provider 协议及自主工具循环。工具字段错误在两版和重开中重复出现，最终交付依赖上述授权修复，未证明首次调用稳定性；固定样本数量不用于估计成功率。脚本回应不能支持真人学习收益或整体能力结论，真实读者验收留在 TB6。TB4–TB6 尚未实施，TB2 的 DeepSeek 历史未结项沿原记录。
