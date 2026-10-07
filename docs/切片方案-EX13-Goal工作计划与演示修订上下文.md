# EX13：Goal 工作计划与演示修订上下文

日期：2026-10-02。状态：**EX13.0–EX13.6 已完成。** [基线、比较入口与验证](performance/presentation-context-ex13/README.md)、[EX13.2 记录](performance/presentation-context-ex13/ex13-2/README.md)、[EX13.3 记录](performance/presentation-context-ex13/ex13-3/README.md)、[EX13.4 记录](performance/presentation-context-ex13/ex13-4/README.md)与 [EX13.5 记录](performance/presentation-context-ex13/ex13-5/README.md)已落盘；决策见 [ADR-0155](adr/0155-goal-work-plan-and-version-centered-presentation-context.md)。

任务目标是让 Resident 在连续制作和修改演示页时，始终知道用户要求、当前进度、正在修改的确切版本与剩余问题，同时减少重复输入、请求前缀变化和不必要的压缩成本。实施沿用现有 Goal、模型循环、工具协议、内容存储、会话日志与交付合同。

优先顺序为：冻结现有证据 → 稳定指导前缀 → 裁剪当前回合的旧候选源码 → Goal 内加入工作计划 → 收敛确切版本修订上下文 → 优化摘要请求 → 有限的真实接续验收。前五步可以先用录制数据、受控模型和本地浏览器完成验证。

[EX12.4](performance/presentation-staged-authoring-ex12-4.md) 的最终修正补丁已在 [EX13.6](performance/presentation-context-ex13/ex13-6/README.md) 完成原生交付与独立内容验收；同对象 revision 2 已保存。原 25 次对照未重跑。EX12.5 的 Linux Reader 连续使用验收仍未开始。

## 1. 已确认的产品边界

用户接受的方向是：在已有 Goal 内补足 Todo；以 Goal 维持任务，把“先整体后局部”保留为适用时采用的制作方法；按确切版本组织修订上下文，裁剪重复历史，提高缓存利用率。

| 概念 | 负责什么 | 权威与生命周期 |
| --- | --- | --- |
| ResidentGoal 的 interpretation / requirements | 用户要什么、完整范围、必须交付什么 | 用户消息是依据，复用现有 refine / revise 规则；同聊天跨运行保存。 |
| Goal 工作计划 | 目前怎样推进、哪些工作完成、接下来做什么 | Agent 可以根据观察调整；附属于 Goal，复用 Goal 保存与投影。 |
| 演示全局框架 | 怎样解释关系、先建立什么前提、对象和表示如何贯穿全文 | 可修订的设计数据；沿用 EX12 的运行内生命周期。 |
| 演示制作阶段 | 此次采样需要哪类职责与技术指导 | 运行内指导选择；阶段变化不表示任务完成。 |
| 演示确切版本与现场 | 当前修改的内容、资源、来源及用户当时看到的状态 | 已交付版本和保存现场是事实；候选和预览资格仍属于当前运行。 |
| 压缩检查点 | 对可压缩历史的有来源覆盖记录的摘要 | 从历史派生；不能覆盖 Goal、内容版本或来源权限。 |

领域术语见 [CONTEXT](../CONTEXT.md#goal-工作计划)。本次扩展 Goal 的工作状态，不改变用户要求的所有权，不建立独立 Todo 服务、计划调度器或第二套持久 PresentationBrief。

复杂解释仍可采用“理解路径 → 关键局部原型 → 观察 → 展开全文 → 串读”的方法；工作计划可以表达这几个工作项。一个已有页面的小修订可以只包含定位、修改、验证与交付，不强制重新做全局设计。阶段是按需指导，不成为必须走完的固定状态机。

## 2. 已有实现与问题证据

本节表格为 EX13.0 冻结时的实现与问题证据；EX13.1 的当前实现见第 6 节及[验证记录](performance/presentation-context-ex13/ex13-1/README.md)。以下事实来自 2026-10-02 工作树和已经保存的 EX12.4 请求。基准提交为 `5e10516`，但 EX12 实现包含未提交修改，因此不能仅凭该提交复原本次基线；接手时还需使用本工作树与相应录制材料。

| 当前实现 | 已经解决的事 | EX13 需要补的部分 |
| --- | --- | --- |
| `goal.rs:ResidentGoal / GoalWorkingState` | 保存原要求、focus、open_questions、next_move、结果引用与状态 | 没有逐项工作状态；不能区分“关键局部做好了”和“整项任务完成了”。 |
| `ResidentGoal::objective_gap` 与 Server 完成提交 | 用实际页面交付和 Reader 操作核对客观缺口 | 不自动核对全部语义范围；工作计划不能被包装成内容完整性证明。 |
| `context_fragment.rs:record_sampling` | 动态运行状态在采样位置追加，保留旧快照，最后快照生效 | presentation 阶段指导仍走另一条前部 instructions 组装路径。 |
| `orchestrator.rs:build_sample_request`、`agent_prompt/presentation.rs` | 按 phase / needs 选择实际指导资源 | phase / needs 改变会改变前部 instructions；交付后回退默认 global 也可能改变它。 |
| `tool_result.rs:ActiveToolResultLedger` | 当前回合工具结果正文有 48 KiB 预算，旧结果可投影成回执 | 不覆盖 assistant 工具调用参数里的整段 HTML；改写旧结果也会改变已发送前缀。 |
| `provider_history_projection`、Server `redact_history` | 已完成回合的制作参数可以收敛为定位信息 | 当前回合的多次 write 仍可能重复携带原型和完整页面源码。 |
| `presentation_api.rs:follow_up_context` | 按提交的 receipt 读取确切版本与保存现场 | 每次同时塞入完整 readable_content；连续修订还会叠加历史制作和失败过程。 |
| `presentation.author.read/search/patch` | 可按确切版本或当前候选定位、读取、精确修改 | 修订入口尚未统一围绕该版本和当前缺陷组织最小材料；write 漏 based_on 可以另建对象。 |
| `compaction.rs:call_generator` | 十节结构摘要、逐 source 覆盖、严格验证、一次修复 | 摘要使用独立请求；当前修复通过改 system 增补错误，会破坏该摘要请求的稳定前缀。 |

正确 CatalogMatch 实验保存于 [mechanism-staged-catalog](performance/presentation-staged-authoring-ex12-4/mechanism-staged-catalog/)。其中以下数据已经存在，EX13.0 只做离线提取：

| 采样请求 | 指导状态 | 输入 token | 缓存输入 token | 缓存比例 |
| --- | --- | ---: | ---: | ---: |
| request-12 | global | 70,791 | 68,992 | 97.46% |
| request-13 | 切到 local | 74,406 | 9,984 | 13.42% |
| request-28 | local | 149,450 | 146,304 | 97.90% |
| request-29 | 切到 review | 150,402 | 9,472 | 6.30% |
| request-30 | 交付后回到默认 global | 150,769 | 24,960 | 16.55% |

在这些相邻请求上，前部指导确实发生变化；缓存下降与之同时出现。提供方缓存保留、工具集合和图片也会影响命中，不能把所有未命中都归给阶段切换。离线验证能证明前缀布局改变，真实命中率和费用收益只能由后续提供方 usage 证明。

该实验后段仍携带两次 write 的 HTML，分别为 18,486 和 31,850 字符，共 50,336 字符。这是字符数，不是 token 数。完整页面候选已经保存后，旧原型源码仍随多次预览采样重复发送，是本次裁剪的具体对象。

EX12.4 共出现 19 次结构化压缩调用，已知输入 395,960 token、输出 343,752 token；输出包含 reasoning。原最终接续中的一次 37,269 输出因引用输入中不存在的 source ID 被拒绝，一次修复输出 47,810 后通过。EX13.0 已按原始录制更正此前“重复 source ID”的误记，详见[离线基线](performance/presentation-context-ex13/README.md)。继续放大输出上限不能消除输入重复、结构冗余和修复成本。

已有实现、实验条件与原始消耗以 [EX12.4 报告](performance/presentation-staged-authoring-ex12-4.md) 为准。v5 已实际进入 review；发现完整范围仍有遗漏，不能归因为“根本没串读”。

## 3. 目标请求结构

```text
稳定的宿主规则、共同解释原则、工程合同、工具协议
  + 已冻结的用户输入与可复用历史前缀
  + 随实际工具循环追加的调用、结果和观察
  + 在原采样位置保留的宿主状态快照 / 指导变更
  + 当前 Goal 要求、工作计划与真实成果
  + 当前框架、确切版本、当前缺陷和必要材料
  → 下一次采样
```

此图表达职责与稳定性，不要求把不同权限层的内容拼成一个字符串。固定工程规则和编译进 Runtime 的阶段指导属于宿主指导；框架、页面数据、用户现场和来源摘录仍是任务数据。移动注入位置时保持这个边界。

缓存优化遵守三项具体取舍：

1. 共同规则在工具可见且内容不变时保持原位置和原内容；phase、needs、进度和读数的变化在后部表达。
2. 同一大段指导只在当前选择实际变化时追加；每次采样仍投影当前状态，不能为了缓存降低状态更新频率。
3. 裁剪旧源码会改变历史前缀，应集中在已经观察到保存成功的候选边界，完成后保持该投影，避免每轮反复改写历史。允许为节省后续多轮输入付出一次前缀失效，并记录它。

工具按需发现和首次激活仍可能改变工具集合与前缀。此次不为提高命中预先曝光所有工具，也不以提高缓存比例为由保留持续无用的大块输入。

## 4. Goal 工作计划的最小合同

`GoalWorkingState` 已增加 `items`，保持现有 `working` 更新入口。数据形状：

```rust
GoalWorkItem {
    id: String,
    description: String,
    status: Pending | InProgress | Completed,
}

GoalWorkingState {
    focus: String,
    open_questions: Vec<String>,
    next_move: String,
    items: Vec<GoalWorkItem>,
}
```

`id` 只用于同一 Goal 中识别、更新和引用工作项；不从内容计算摘要或指纹。`items` 按当前推进顺序排列。多项同时进行无需新增并发调度语义；普通串行任务以一个当前项表达工作重点。

**更新语义**：`goal.update(operation="working")` 提供 `items` 时原子替换当前列表；省略时保留已有列表；显式空列表清空工作计划。保存旧 Goal 缺少该字段时读取为空列表。只增加字段默认值与相应用例，复用现有 Goal revision、事件与恢复路径。

工作项的非空 id、非空描述和 id 唯一性用于避免错误更新，复用 Goal 的参数校验方式。相同更新不增加 revision。Agent 可以重排、拆合或删除已放弃的方法步骤，也可以因验证失败将 completed 改回 in_progress；这些调整不修改 requirements。

三个既有字段保持明确分工：`focus` 是当前要解决的问题，`open_questions` 是会影响下一步选择的未决问题，`next_move` 是眼下要采取的动作；`items` 表示多步进展。投影不要求在四个地方重复同一句话。没有多步工作时，空列表与现有短工作判断足够。

**要求与完成语义**：

- 工作计划不能扩大或缩小用户要求。范围变化继续通过有用户消息依据的 refine / revise 处理。
- 所有工作项标 completed 不能自动完成 Goal；页面必须沿用实际预览、交付及历史提交合同。
- 剩余工作项是下一次决策的提醒，不新增一套强制执行流程或内容自评分器。准备结束时 Agent 应使计划与真实成果一致，保留尚未解决的内容缺口。
- 串读必须同时回看用户要求、对应材料和当前成果，不能仅拿自己写的框架或 completed 列表证明完整。

例如原问题的复杂制作可以记录“厘清完整范围、试做搬运/计算关系、依据观察调整、补齐其余内容与练习、串读并核值、交付”；最终局部修订只需记录“读取指定版本和待改片段、应用纠错、验证数值与短屏布局、交付同一对象的新版本”。这些是例子，不进入通用工具 schema 成为固定模板。

`prepare` 继续保存 phase / framework / focus / needs。framework 不再抄录工作项状态；authoring focus 表达当前局部的解释关系，Goal focus 表达当前任务问题。两者相同时只需简短指明当前局部，不额外制造必须填满的文字。

## 5. 按确切版本组织修订

修订的最小输入由以下已有事实组成：当前用户要求、关联 Goal 的要求和未完成工作、确切 `PresentationRef`、已保存现场、当前明确缺陷、可按需读取的内容和来源定位。历史失败过程只保留会改变本次修法的结论。

`follow_up_context` 保留“选定旧版本就读取旧版本”，不能使用 latest 代替用户发送时的 reference。EX13.4 已将全文自动注入调整为“标题、reference、现场、必要假设和内容读取入口”；`read/search(file="readable_content")` 从同一版本按需读取或定位正文，长正文沿现有结果预算分页取回。源码继续使用 search / read 的 file 与范围能力。

现场中的参数、选项、步骤与显示结果保留原义，标明它们是页面观察值。它们不能自动成为独立算值、原文证据或新的模型指令。页面存在的来源绑定可以沿原规则继承；新书源断言仍按现有证据取得与来源附着规则处理。

确切版本能回答“内容是什么”，Goal 能回答“还要做到什么”。选择一个版本开始修订不等于删掉原任务尚未完成的要求，也不必重新发送先前所有 HTML、截图描述、压缩修复失败和中间设计。

**写入语义需要覆盖已发生的错误**：EX12.4 的 mechanism feedback3 漏掉 `based_on` 后创建了新对象。对于携带 follow-up receipt 的 `write`，有 `based_on` 时必须匹配该 receipt；两者皆无时保留普通新建行为。携带 receipt 却既不写 `based_on`、也不显式声明新建时，返回可修正的参数错误，避免默默另建对象。

EX13.4 已增加 `write.new_object: true` 表达明确的新建选择，仅在未提供 based_on 时使用。带 receipt 时缺省不代表新建；同时提供 new_object=true 与 based_on 是冲突。它是内容操作的意图参数，指导要求 Agent 根据用户实际请求选择。`patch(reference / candidate_id)` 保留确切基底语义；显式新建的当前运行候选可继续 patch。

新运行重新获取所需工具、来源观察和候选预览资格。已交付版本可以跨运行读取；旧 run 的 candidate_id 不能因存在于历史或工作计划中而重新取得资格。页面候选源码的恢复读取仅在当前 run 的合法候选存续期间有效。

## 6. 实施切片与依赖

| 切片 | 交付 | 前置 | 状态 |
| --- | --- | --- | --- |
| EX13.0 | 不调用模型的录制基线与比较入口 | 已保存 EX12.4 材料 | 已完成；8 项离线测试通过 |
| EX13.1 | 固定共同指导，阶段与技术指导在采样位置追加 | EX13.0 | 已完成；Runtime 440 项通过，比较脚本 10 项通过 |
| EX13.2 | 当前回合已保存旧候选的源码投影裁剪 | EX13.0、EX13.1 的请求观察入口 | 已完成；Runtime 443 项通过，Server 编辑/存储 4 项通过，离线录制测试 1 项通过 |
| EX13.3 | Goal 内的工作项、更新、恢复和投影 | EX13.1 | 已完成；Runtime 447 项通过，Server/Web 验证见 EX13.3 记录 |
| EX13.4 | 确切版本修订输入与显式新建选择 | EX13.2、EX13.3 | 已完成；Runtime 449 项、Server 23 项通过，见 EX13.4 记录 |
| EX13.5 | 保持覆盖合同的摘要请求与修复前缀优化 | EX13.2、EX13.4 | 已完成；Runtime 453 项通过，离线来源/前缀及 Native/ReAct 本地 HTTP 验证见 EX13.5 记录 |
| EX13.6 | 有限的真实接续与效果记录 | EX13.1–EX13.5 | 已完成；同对象 revision 2、三环境七路径通过，累计 22 次调用，费用见 EX13.6 记录 |

依赖表示所需输入，并不要求增加 Agent 或另建运行服务。每片完成后更新本表、相应验证记录与 checkpoint，再开始下一片。

### EX13.0：从现有请求建立可复用的离线基线

**实施结果（2026-10-02）**：[compare.py](performance/presentation-context-ex13/compare.py) 支持固定样本重放与任意两份同类型录制比较；[baseline.json](performance/presentation-context-ex13/baseline.json) 保存五个请求的分类体积、usage、有序首差、阶段/工具与裁剪变化、两次 write 的成功回执及源码落点、feedback5 摘要修复与确切版本补丁关联。8 项测试通过，新增模型调用 0。三个阶段切换首差均在 instructions 字符 10,846；13→28 区间有 9 个旧结果裁剪，后段仍携带两份共 50,336 字符 HTML。测量口径与证据更正见[说明](performance/presentation-context-ex13/README.md)。

**交付**：新增 `docs/performance/presentation-context-ex13/` 的基线说明与紧凑统计，配套可重放录制输入的测试 fixture 或脚本。保留原始 EX12.4 目录，只引用或抽取必要样本。

输入选择：正确 Catalog 的 request / response 12、13、28、29、30；两次 write 和成功回执；最终 feedback5 的压缩请求、拒绝与修复；已经验证的 `local-correction/patch.json`。比较采用原 CatalogMatch，不把 ExplicitOverride 的旧组混为同一基线。

分别统计固定 instructions、phase / needs 指导、动态状态、assistant 调用参数、tool 结果、图片引用、其他历史的序列化大小，并标出真实 usage。直接比较请求的有序内容和首个差异位置，不新增哈希文件。记录 phase、工具集合和裁剪事件，解释每次前缀变化来源。

**检查要发现的失败**：录制样本的 profile、上下文或前后请求不匹配，会让后续“节省”比较失去意义；发现后更换为已验证的匹配样本。用离线解析断言，不重新调用模型取得同一基线。

**完成条件**：接手者可用相同录制输入重复得到上述关键体积、调用类型与差异位置；真实 usage 与离线字符/字节统计分开记录。对没有 usage 的离线请求不生成虚构的费用或缓存命中。

### EX13.1：稳定共同指导，把变更移到后部

**实施结果（2026-10-02）**：共同模块保持前部 instructions；固定阶段/技术资源在采样锚点以 system 指导事件追加，重复选择不重复正文。每次采样投影当前 phase/needs，framework/focus 保持 user 数据；交付与禁用工具的收尾采样显式失效，后续合法制作重新启用。压缩保留原锚点，同次重建去重与 Goal 补交付的新采样分别处理。受控 9 对 Native/ReAct 请求均保留既有文本前缀，Runtime 440 项通过、3 忽略，最终制作链路 10 项通过；离线脚本 10 项通过。录制、体积增长和验证边界见[EX13.1 记录](performance/presentation-context-ex13/ex13-1/README.md)。新增模型调用 0。

**代码入口**：

- `crates/runtime/src/agent_prompt/presentation.rs`：共同方法、工程合同、能力目录与阶段/技术资源的组装边界。
- `crates/runtime/src/orchestrator.rs:build_sample_request`：从所有模块均进入前部 instructions，调整为稳定模块加有采样锚点的指导事件。
- `crates/runtime/src/context_fragment.rs`、`run_context.rs`：复用采样位置和运行内状态；阶段指导与框架任务数据分别投影。
- `crates/runtime/src/model_runtime.rs`：检查最终请求序列化，避免适配层再次把尾部指导提升或合并到前部。
- `crates/runtime/src/presentation_authoring_tests.rs`、`agent_prompt/presentation/tests.rs`：受控循环验证时序、权限层次和实际请求。

首次制作能力可见时加载共同规则，并追加默认职责；prepare 更新后，下一次采样在工具结果之后追加新的 phase / needs 指导。历史指导保持其原采样位置，最新选择支配当前工作。重复相同选择不追加大段相同技术正文；当前 phase / needs 标识仍随每次状态投影出现。

编译自固定资源的指导可以使用宿主规则消息；Agent 的 framework / focus 仍按任务数据发送，不能直接拼进 system 快照。沿用相同 Agent 与工具循环，不因“切阶段”单独调用模型。

交付或取消时清除运行内设计状态，并使旧阶段指导失效；交付后的收尾采样不因默认 global 又改写共同前缀。若同一运行有后续新的合法制作动作，再按当时状态追加新选择。

发生历史压缩后，恢复当前必要指导与设计数据，不能因其原注入位置被摘要覆盖而消失。候选、图片和来源资格仍从现有运行事实取得。

**检查要发现的失败**：同工具集合下 phase / needs 切换仍改写共同 instructions；旧 local 指导在 review/交付后继续被当成当前职责；framework 被提升为规则；压缩后当前指导丢失。相应修正请求拼装、失效表达或恢复位置。

**完成条件**：受控 global→local→review→deliver 路径中，前部共同 instructions 保持一致，指导只在实际切换点追加，技术参考按需进入；现有下一次采样生效与交付资格测试保持通过。记录最终提供方请求的比较结果，不只检查模块选择函数。

### EX13.2：裁剪当前回合中已经有可靠落点的旧源码

**实施结果（2026-10-02）**：`ActiveToolResultLedger::insert_call` 绑定成功保存回执与调用，首次采样保留完整源码，之后只在模型投影移除 html/readable_content，保留当前运行候选的读取入口及原元数据。最近 patch 的 edits 保留到明确以它为基底的成功后继被采样；独立 write 不推断替代关系。结果正文淘汰后定位仍在，压缩后工具配对、原始消息和 required source 覆盖通过验证。使用原两份 HTML 的受控末次请求 128,457→52,654 字节（−59.01%）；Native/ReAct 序列化也已核对。Runtime 443 项、Server 编辑/真实存储 4 项、离线录制测试 1 项通过，新增模型调用 0；详见 [EX13.2 记录](performance/presentation-context-ex13/ex13-2/README.md)。

**代码入口**：`orchestrator.rs:provider_history_projection / history_projection_through / messages_with_context_fragments`，`tool_result.rs:ActiveToolResultLedger`，Server `presentation_author.rs` 的 write / patch 成功回执和 `redact_history`。

先覆盖确定的关系：write / patch 返回候选保存成功，相关结果已进入后续采样；旧的整段写入源码可通过当前 run 内的 candidate_id 重新取得。该 assistant 工具调用仍保留 id、操作、候选关联、based_on、文件定位与必要说明，移除重复的大正文。工具调用与结果必须成对存在。

首次发送的调用、尚未观察的结果、保存失败的调用、没有可重读落点的内容不裁剪。只有与该调用成功回执对应的源码才能按此规则收敛；不能因“后面还有一次 write”就认定它替代前一个独立作品。

同一作品出现新候选时，优先收敛已经保存的早期原型；保留当前工作所需的最近修改上下文。历史持久化和审计仍使用原有原始记录规则，本片只改变模型请求投影，不覆盖内容文件或伪造历史工具结果。

裁剪发生在语义明确的保存/采样边界，并保持后续投影稳定；接入现有结果预算时避免对同一调用重复改写。source ID、证据回执、source_ref_ids、最终结果引用和用户要求保留原有身份。

**检查要发现的失败**：模型收到孤立 tool result；误裁尚未保存源码；裁剪回执无法定位实际候选；新采样仍重复携带旧原型全文；裁剪导致摘要遗漏 required source。发现后修正候选关联、投影时机或压缩输入映射，不能靠放宽来源验证通过。

**完成条件**：EX13.0 录制中的旧原型不再在每次后续预览中携带整段 HTML；可从回执重读对应合法候选。失败写入、两个独立作品、patch 链和压缩后投影均有针对性的受控用例，原始事实记录和交付行为不变。

### EX13.3：Goal 增加可恢复的工作计划

**实施结果（2026-10-02）**：`GoalWorkItem` 与 working.items 实现原子替换、省略保留、空列表清空、完整参数校验及同值不增 revision；最新投影包含工作项与未决问题。复用 GoalUpdated / TurnFinished、changed_goals 及新运行 Goal 装载，压缩不改写计划或 requirements。Web 类型同步，制作指导升级为 `ex13.v1`。Runtime 447 项通过、3 忽略；完整持久化和 Web 结果、Native/ReAct 请求见 [EX13.3 记录](performance/presentation-context-ex13/ex13-3/README.md)。新增模型调用 0。

**代码入口**：

- `crates/runtime/src/goal.rs`：上述类型、apply_update 与 projection。
- `crates/runtime/src/orchestrator.rs:resident_tools` 的 `goal.update` schema、处理分支及 `agent.resident_goal` 投影。
- `crates/server/src/session_store.rs:changed_goals` 与 `session_log` 的 `GoalUpdated / TurnFinished` 记录和恢复测试。
- `packages/web/src/api.ts:ResidentGoal`：保持前后端数据类型一致；沿现有 Goal 状态显示，不新增独立任务面板。
- `skills/presentation/SKILL.md` 及 `phases/`：明确用户要求、工作项和解释框架的分工；实际发布指导变更时更新方法版本。

按第 4 节实现完整合同。压缩摘要可记录工作的语义进展，但每次采样仍以聊天持有的 Goal 最新状态为准；summary 的 active_goal 不能覆盖它。Goal 完成、取消、替换和新建关联 Goal 延用既有生命周期。

**检查要发现的失败**：一次普通 working 更新清空已有工作项；重启/压缩后列表丢失；旧数据缺字段导致读取失败；同一更新重复增加 revision；工作项完成触发缺少页面的 Goal 完成；框架修改擦掉要求。分别用更新、序列化、日志重放、受控循环和既有交付缺口用例验证。

**完成条件**：一个复杂任务的未完成项能跨采样、压缩和同聊天新运行恢复；局部候选完成时整体要求仍在；简单任务可保留空列表；不新增额外监督模型调用和独立计划存储。

### EX13.4：修订只装入确切基底、当前问题与可恢复材料

**实施结果（2026-10-02）**：追问保留确切 receipt、标题、完整现场、假设和实际内容入口；正文通过 `file="readable_content"` 按需读取/搜索并支持预算续读。write 实现显式新建与 receipt 基底匹配，遗漏或冲突返回可修正参数错误；显式新建候选可继续 patch，来源和跨运行资格沿原合同。制作指导更新为 ex13.v2。受控追问 37,142→1,706 字节，Goal 要求及未完成项保留；Runtime 449 项、Server 版本/编辑 23 项通过，真实模型调用 0。见 [EX13.4 记录](performance/presentation-context-ex13/ex13-4/README.md)。

**代码入口**：`crates/server/src/presentation_api.rs:follow_up_context`，Server `presentation_author.rs` 的 read / search / write / patch，Runtime `presentation_author.rs:AuthorRequest / spec`，`agent_run.rs` 的预备上下文与相关 Server 用例。

按第 5 节收敛 follow-up 投影，并实现显式新建参数。已保存现场与 exact reference 继续在 admission 时验证。目录或正文索引从现有内容元数据取得；不得为了省输入额外调用模型摘要，也不新建一个可能与版本脱节的缓存正文。

修订开始先读取相关版本和范围。改动只涉及某段数值或布局时，复用有效框架与资源，通过 patch 修改；需要重做整体表示时可 write，但仍明确 based_on。正文解释类追问可以按需读完整 readable_content，不能因为裁剪入口而失去回答内容的途径。

将“仍要解决的缺陷”放在当前工作计划和本轮材料内：缺陷描述、对应位置/操作、预期可观察结果。历史失败保留结论与必要定位；失败次数、旧提示迭代和已经失效的候选不持续展开为任务输入。

**检查要发现的失败**：用户选旧版本却读了新版本；参数现场丢失；省略 based_on 悄悄另建对象；有意新建被一律阻止；元数据缺少可用读取入口；重启后复用旧候选资格。发现后调整入口投影、write 语义或版本解析，不增加全局确认步骤。

**完成条件**：受控修订在同一 presentation id 上产生新 revision，内容与来源按预期继承；另建对象必须显式选择；旧版本解释、保存现场、普通新建与局部 patch 路径通过。请求中不再无条件附带整篇 readable_content，相关内容可按需准确取回。

### EX13.5：优化摘要请求，保持来源覆盖合同

**实施结果（2026-10-02）**：固定摘要规则，诊断追加在原输入之后；`generation_input` 保留来源材料、顺序、覆盖与证据约束，只移除四个运行时管理字段。预算与实际发送复用同一投影，修复尾部纳入输入预算。原 feedback5 的 237 个来源、84 个 required source 保持；原错误摘要拒绝，原修复摘要通过，最多一次修复。首轮 system + user 156,803→156,655 UTF-8 字节，修复保持全部规则与材料前缀；Native/ReAct 本地 HTTP 已验证。Runtime 453 通过、3 忽略，新增真实模型调用 0；详见 [EX13.5 记录](performance/presentation-context-ex13/ex13-5/README.md)。

**代码入口**：`crates/runtime/src/compaction.rs:CompactionRequest / call_generator / validate_draft / request_input_tokens`，`auto_compaction.rs`，`orchestrator.rs:maybe_auto_compact`，现有模型适配器的 `complete_structured(CompletionRequest)`。

先使用 EX13.2/EX13.4 已去除重复的投影确认压缩输入和触发次数，再处理摘要请求自身。首片采用现有 system + user 结构即可完成的改动：固定摘要规则，保持原输入正文不变，将一次修复的诊断追加在原输入之后，由固定规则说明如何使用该诊断。当前通过修改 system 触发整段前缀变化的做法退出。

摘要输入的稳定部分与变动元数据分开组织，保持来源项的原顺序和确切 source ID。删除机器已经持有、且不影响模型完成语义归纳的重复包装；任何精简都需指出被删部分不再承担什么实际用途。token 预估与实际序列化使用同一投影，包含尾部修复提示与输出预留。

维持现有十节 CompactionDraft、逐 eligible source 的覆盖、required source 限制、证据链接检查和最多一次验证驱动修复。非法摘要不得通过“清洗”、补造 source ID 或降级为任意文本安装。

本片不直接把完整主线消息重新塞入摘要请求。当前适配器是独立结构化请求，主线前缀能否复用需先离线比较最终 wire 消息；若必须为此重做适配器或重新发送明显更多无关内容，保留为后续独立方案。本片至少完成摘要首轮与其修复之间可验证的稳定前缀。

当前 CatalogMatch Flash 的 65,536 输出上限和应用压缩水位先保持不变。离线 token/字符减少不自动成为提高水位、扩大预算或降低输出上限的依据；只有真实剩余成本与失败证据支持时再单独调整。

**检查要发现的失败**：修复仍改变输入前部；required source 被漏掉或误判 non_task；重复覆盖被接受；错误预算使修复请求超出可用输入；裁剪后的 source 映射与原始证据脱节。复用严格验证的既有失败 fixture，并为实际改动补充最小反例。

**完成条件**：错误摘要仍被同样拒绝；一次修复仍有界；初次/修复请求共享未改变的规则与原材料前缀；成本记录区分摘要输入、缓存、输出、reasoning 与失败重试，不将 reasoning 再次加到总输出。

### EX13.6：有限真实接续与整体收口

**实施结果（2026-10-02）**：用户授权沿用原 DeepSeek 配置、总支出不超过 9 元。恢复原聊天、Goal、检查点与确切现场，原生应用原 11 项补丁及一项公式补充；三环境 preview、后续图片观察、local→review、同对象 revision 2 交付完成，独立三环境七路径通过。真实接续检出的旧历史被补入 new_object:null 导致检查点失效已修复，Runtime 454 项通过。累计提供方调用 22 次，按空闲价估算 0.93951468 元、高峰价保守累计 1.87902936 元，余额净减 0.59 元单列；一次自然压缩首轮通过。请求、失败、费用限制与验收见 [EX13.6 记录](performance/presentation-context-ex13/ex13-6/README.md)。

**开始条件**：相关确定性测试通过，提供方可用，真实调用范围与最高支出明确；本次已满足并完成，成功后停止调用。

本片复用 EX12.4 的最终真实接续，基底为 `presentation-1790920818511542500-6` revision **1**，补丁为 [local-correction/patch.json](performance/presentation-staged-authoring-ex12-4/local-correction/patch.json)。这是已交付的新对象 revision 1，不是旧对象 revision 2。

以一个冻结输入验证：恢复 Goal / 当前工作 → 读取确切版本和局部内容 → 修改 → 当前运行中的三环境预览 → 后续采样检查图片 → 交付同一对象的新 revision。已离线验证的数值和七条操作路径作为预期结果；独立截图不能代替原生候选的预览资格。

若这次短修订不足以出现 phase 切换或自动压缩，真实记录只支持修订链路结论。不能为凑齐指标不断续跑；只有尚待验证的机制确实需要且仍在约定费用范围内，才增加一个明确触发该机制的固定样本。

费用边界直接用于现有实验入口的启动与停止：记录模型/profile、允许运行数及调用/输出边界；成功后停止，遇到无法在剩余预算内处理的失败便保留材料和缺口。若现有入口不能落实本次约定边界，先补实验入口的具体限制，不能把任意 token 数写成货币硬保证，也不扩成产品计费系统。

**结果记录**：实际任务完整性、同对象版本结果、未解决缺陷、主线/摘要各自 usage、cached / uncached input、输出（注明 reasoning 是其中子集）、总耗时和新增调用数。使用当次提供方价格核算估计费用，账单存在时单列实际账单。

**完成条件**：目标修订通过原生交付与内容验收；没有因工作项、裁剪或阶段变更失去要求、来源和状态。前缀与体积优化已由离线对比证明；真实缓存和费用仅报告已实际发生的样本结果。EX12.4 只有在其原验收要求也通过后才更新为完成，再单列 EX12.5 的部署和 Reader 连续使用。

## 7. 验证范围与接手方式

实施每片前先说明此次验证会发现哪种具体失败，以及失败后改哪个入口；上文各片已列出对应对象。文档落档阶段只检查路径、术语、依赖与状态一致性，不运行应用测试或模型实验。

改运行代码后使用受控 ModelAdapter 验证组装与行为，用录制请求比较输入体积与前缀，用现有严格解析验证摘要。只有修改页面运行或预览行为才运行对应浏览器用例；不为纯请求排布变化重新运行全部数值、整套端到端和 25 次付费矩阵。

EX13.0 的[离线基线](performance/presentation-context-ex13/README.md)已成为 EX13.1–EX13.5 的共同观察入口。每片记录“改动前后的实际请求、具体失败是否消失、仍未验证什么”；通过后进入下一片，不反复扩大已定论检查。

冷启动读序：本方案第 2、6 节 → [ADR-0155](adr/0155-goal-work-plan-and-version-centered-presentation-context.md) → [EX12.4 报告](performance/presentation-staged-authoring-ex12-4.md) → 对应小片代码入口。需要既有合同才读 [Goal 原方案](切片方案-Resident-Goal目标维持与交付闭环.md)、[EX12 方案](切片方案-演示页全局框架与分阶段制作.md) 与 [调用成本计划](计划-调用成本与DeepSeek适配.md)。

**下一步为 EX12.5**：读取 EX13.6 的 revision 2、验收记录与原 EX12.5 合同，在目标 Linux Reader 推进长页操作、保存重开和后续局部修订。

## 8. 已知限制

工作计划改善任务连续性，不提供自动语义完整性证明。原文范围、数学关系和交付内容仍需要 Agent 的实际核对、具体数值验证及用户反馈。

稳定前缀提高可复用机会，实际缓存由提供方决定。压缩安装、工具集合变化和有意裁剪都会改变前缀；不能承诺某个固定命中比例或费用降幅。

运行内框架仍不随页面版本持久保存。需要跨运行恢复设计时，从已交付内容、Goal 与必要历史结论重新建立简短框架；完整持久 PresentationBrief 仍需独立真实需求。

EX13.0–EX13.6 及 EX12.4 最终原生内容验收已完成。EX13.6 记录了真实缓存、费用和一次成功压缩，但没有新旧同输入实测对照，不能推断一般性费用降幅；原模型收尾文字的练习编号误指保留在记录的已知限制中。Linux 已上线 `ex13-jl-20261002`，长页、保存重开与现场追问通过，见 [发布记录](Linux上线-EX12-EX13-JL.md)；同对象后续局部修订、长期连续使用、实体手机和学习效果仍未验收。
