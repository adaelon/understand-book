# TB2：使命驱动的文字教学

2026-10-12，**TB2 已完成工程与子代理教学内容验收，未提交、未部署**。最终指导 tb2.v4，TO3 已集成；混合请求意图、前提规则、Copy 条件与来源交付问题已收口。GPT-6.1-Sol 前后四组及一次局部补跑均保存并通过实际交付回放。本轮 DeepSeek 调用为 0，历史失败及未结算预占保留。TB3 尚未开始。

## 实施状态

- [x] 按 checkpoint 读完冷启动入口，采用既有术语、TeachingTarget、事实和控制边界。
- [x] 实现活动教学使命、前置支撑与学习者活动指导，收敛重复参数说明。
- [x] 集成 TO3 Tutor 契约与状态示例，验证最终请求、真实分派、帮助和暂停。
- [x] 保存实模产出、失败与请求版本，修复实验录制器终答输出上限。
- [x] 按用户成本约束改用 GPT-6.1-Sol 完成同输入／工具说明的教学对照、误解组交付及初学者局部补跑，保留失败。
- [x] 修复已知来源后缀归一化，核对九组真实选择与交付，更新方案和 checkpoint。

## 本轮收口

完整内容证据见 [Sol 实验记录](sol/README.md)及 [summary.json](sol/summary.json)。旧指导两个起点都误标 direct_explanation，初学者还缺移动前提；新指导采用 guided_inquiry，复用已知起点。初学者 v3 仍缺旧绑定可用性规则，v4 补全后仅重跑该组。误解组纠正引用等于复制，直接讲解覆盖五点；没有生成学习者回应。

`normalize_bound_source_suffixes` 原先跳过整条含行内代码／引号的正文行，且不接纳句间后缀。现只在标记位置排除字面内容，识别已知 ID 的句尾引用。历史所有权回答的 8 个标记现在形成 5 个可打开来源；代码块、行内字面标记、引文、普通 Markdown 链接和未知 ID 保持原语义。

| 验证 | 结果 | 记录 |
|---|---|---|
| 来源问题 red | 两项失败，历史回答来源数为 0 | [source-red.log](source-red.log) |
| 普通链接边界 red | 复现句号紧邻链接时误归一化，修正后纳入同组回归 | [source-link-red.log](source-link-red.log) |
| Runtime 来源／Tutor／工具契约 | 69 通过 | [closeout-runtime-final.log](closeout-runtime-final.log) |
| Server Tutor（含九组回放） | 108 通过，13 默认忽略 | [closeout-server.log](closeout-server.log) |
| v4 最终请求 | 1 项通过，覆盖五种 Tutor 状态下的指导与工具边界 | [guidance-v4.log](guidance-v4.log) |
| 输入、意图、来源与交付记录 | 9 组核对通过 | [summarize-sol.py](summarize-sol.py)、[summary.json](sol/summary.json) |
| Copy 条件反例 | 实现 Copy 的结构体直接赋值后两个绑定均可打印 | [copy-check.json](copy-check.json) |

Server 回放固定读取和来源注册顺序，真实执行选择、引用编译、交付及持久化；只按原文 quote 重绑当前回合 ID，不修改模型教学输出。源码快照分别为 `sol/before/tutor.rs`（tc6.v1）、`sol/after/tutor.rs`（tb2.v3）和 `sol/prerequisite-repair/tutor.rs`（tb2.v4）。

## 实现与归属

`crates/runtime/src/tutor.rs:COMMON/SELECT` 将使命落到“本步能力—可用支撑—学习者活动”：预测前补足必要概念、规则及操作后果，允许有条件、标明身份的补充解释；复用已知起点，纠正具体错误连接，交付可开始的活动并等待读者。混合请求按完整意图解释，完整解释／答案／总结／翻译请求仍优先于默认教法。指导版本为 `tb2.v4`。

沿原 `instruction_modules` 仅在 active 状态加载 SELECT，关闭、观察、引用和暂停行为保持。操作字段及示例由 TO3 契约维护；没有新增领域对象或数据流边界。

TO3 仅集成 `spec/input_examples/spec_for_context`、`tool_schema::complete_spec` 的评分 submit 条件、`run_context_inner` 的采样时刷新及 Runtime/Server 示例测试。主工作区其他工具和首页修改未带入。工具版本由 [to3-baseline-tutor.rs](to3-baseline-tutor.rs) 与 [to3-integration.patch](to3-integration.patch) 固定；[to3-tb2-tutor.rs](to3-tb2-tutor.rs) 保留此前 DeepSeek 轮的 tb2.v2 源码。

实验侧 `WireRecorder::chat` 给没有输出上限的终答请求补 32,768，保留显式上限和原捕获计划，实际线请求单独保存。生产 Provider 和停止策略不变。

## 输入与运行约定

本节记录已执行的 DeepSeek 实验；后续执行方式见“后续实验方式”。

沿 [TB1 scenarios](../tutor-building-blocks-tb1/scenarios.json)、[material](../tutor-building-blocks-tb1/material.json)、[source](../tutor-building-blocks-tb1/source.txt) 的四组输入。两种起点共用问题；全为脚本背景与请求，真实读者 0，不预写后续作答。原文与代码判据沿 [TB1 content-check](../tutor-building-blocks-tb1/content-check.json)：原例打印两行 Foo(0)，去掉 `&` 后使用 foo 报 E0382，经共享引用写入报 E0594；材料未改，不重复编译。

原 Provider、native/deepseek-v4-flash、temperature=0、CatalogMatch，观测上传关闭。每版本四组各一次，每组最多 12 次 HTTP，每版本累计上限 60 次／3,000,000 token；调用前预留 1,000,000 token 窗口及输出上限，未知 usage 停止。金额未知单列，局部补跑继承对应版本预算。Rust +1.95.0，构建与临时目录沿 TB1 run.ps1。

TO2 首轮使用 tb2.v1；TO3 集成后原计划改前／改后各四组，最多新增 96 次 HTTP，沿相同单组预算重新配对。实际顺序为改后再改前，旧轮用量另列累计。

## 检查目的与失败处置

- Runtime `tutor`：检测使命进入关闭／观察状态、请求优先级或回应边界丢失、重复或无关阶段指导；失败修装配。
- Runtime 契约与 Server `tutor`：检测 TO3 状态示例、Native/ReAct 投影、真实分派、帮助／来源／暂停边界回归；失败修集成接点。
- TB1 夹具：检测固定输入、真实源读取、选择、交付或预算记录错误；失败修实验入口，不以替身回答判教学质量。
- `tutor_tb2_terminal_request_is_bounded_before_provider_dispatch`：复现 tools-disabled 终答无上限而在发送前被预算拒绝；修 recorder 转发，保留显式上限。
- 实模：检测缺必要规则、起点仅换篇幅、误解未纠正、直接讲解被覆盖或未交付；保留失败，按指导、工具契约、交付或实验入口归因。
- TB1 summarize.py 与 [compare.py](compare.py)：检测输入／工具不同、捕获遗漏、usage 漏计、指导版本混用；修捕获或配对，并统计实际来源数量及残留标记。
- 最终文件检查：检测冲突标记、空白错误、交接链接失效及 checkpoint 遗漏在途项；失败修相应文件。

## 确定性结果

| 入口（均 Rust +1.95.0） | 结果 | 记录 |
|---|---|---|
| `cargo test -p runtime -j 2 tutor` | 27 通过 | [runtime.log](runtime.log) |
| `cargo test -p server --lib -j 2 tutor_tb1_ -- --test-threads=1` | 3 通过，1 默认忽略 | [fixture.log](fixture.log) |
| `cargo test -p runtime --lib -j 2 -- tool_registry::contract_tests tutor --test-threads=1` | 41 通过 | [integration-runtime.log](integration-runtime.log) |
| `cargo test -p server --lib -j 2 tutor -- --test-threads=1` | 106 通过，13 默认忽略 | [integration-server.log](integration-server.log) |
| `cargo test -p server --lib -j 2 tutor_tb2_terminal -- --test-threads=1` | 预期失败：None != Some(32768) | [terminal-red.log](terminal-red.log) |
| 终答修复后 `-- tutor_tb1_ tutor_tb2_ --test-threads=1` | 4 通过，1 默认忽略 | [terminal-green.log](terminal-green.log) |

集成回归 147 项，不重复计数前面定向运行。新增五状态请求测试核对 active 才装配使命、直接请求可选解释、前提及回应边界保留；TO3 示例、真实源与事实、评估后撤下示例和暂停路径通过。实模入口通过只表示记录落盘。

前轮检查：git diff --check 通过；当时 113 个本地文档链接目标存在，checkpoint 为 45 行，工作区 tutor.rs 与 tb2.v2 快照一致。该结果只对应此前 DeepSeek 轮。

## TO2 首轮与 TO3 集成

[after/summary.json](after/summary.json) 是 tb2.v1 四组各一次，指导变更见 [guidance.patch](guidance.patch)。40 次 HTTP、808,339 token。所有权组完整；初学者和直接讲解组到达 12 次 HTTP 上限，无终答。前者出现 target 字符串、漏 move、能力不一致，后者出现 target 字符串。误解组交付末尾把打印 foo 的第一行写成“第二个 println”，属于内容失败。

[after/comparison.json](after/comparison.json) 确认输入及初始工具与 TB1 相同。tb2.v2 补充缺失前提处理及活动中的准确指代；TO3 从原所有者提供合法示例。两条线一同改变的结果不能证明指导的单独收益。

## TO3 实际结果

[to3-after/summary.json](to3-after/summary.json) 记录 tb2.v2，33 次 HTTP、719,605 token。四组各一次，无未知已发送用量；误解组最后一次无上限请求在发送前被拒绝，未计为 HTTP。

| 输入 | HTTP | 总 token | 秒 | 实际交付 |
|---|---:|---:|---:|---|
| 只懂赋值 | 10 | 192,768 | 103.541 | 完整，4 个来源；预测前有移动规则 |
| 已懂所有权 | 7 | 163,081 | 78.279 | 完整，但遗留 8 处错误来源标记，无可打开来源 |
| 引用等于复制 | 6 | 141,787 | 56.285 | 已选动作，终答被 recorder 拒绝，无教学交付 |
| 直接讲解 | 10 | 221,969 | 60.825 | 完整，3 个来源，direct_explanation/current_request |

[初学者回答](to3-after/starting-assignment/answer.md) 从整数赋值接到值与引用，明确所有权含义，补移动规则，再让读者定位打印行。[所有权回答](to3-after/starting-ownership/answer.md) 复用已知移动规则，聚焦赋过去的是 Foo 还是 &Foo，留下打印行与类型判断。[直接讲解](to3-after/direct-explanation/answer.md) 覆盖五点，没有先要求用户作答。这些是内容观察，不构成学习者表现。

[to3-before/summary.json](to3-before/summary.json) 的旧指导首次请求失败，transport 为 response、succeeded=false、usage=null，没有 Provider 错误正文。保留 1,032,768 token 预占，停止后续发送；其余三组只记录预算拒绝，HTTP 为 0。不能判定 Provider 具体原因或记为零费用，本组不是完成的旧指导内容对照。

[to3-after/comparison.json](to3-after/comparison.json) 核对输入、初始工具、实际指导版本与改后 Native 请求。相同工具均为 30,331 UTF-8 字节，指导从 7,820 增至 9,153 字节，首采样估算输入各增加 334 token。无重复使命或无关评估／演示阶段；改前交付缺席，暂不能判断改善或稳定成本收益。

TB2 合计 **74 次 HTTP 尝试，已知 1,527,944 token，另有 1,032,768 token 未结算预占**。金额未知。usage 缺失后没有追加调用或重置额度。

## 实验入口修复与复现

初次切换用 Copy-Item，旧时间戳使 Cargo 使用已编译 tb2.v2。实际请求核对后，第一轮按改后版本保存，第二轮运行旧指导；最终目录及 configuration 按实际版本标注，原请求未改写。现 [run-paired.ps1](run-paired.ps1) 写入源码字节刷新时间戳，结束恢复原源码；compare.py 验证旧指导不含使命。

误解组 tools-disabled 请求缺 max_tokens 是 recorder 遗漏。回归先复现，再由 `WireRecorder::chat` 补上限；实模补跑受后续 Provider 未结算请求阻断，没有用工程通过代替交付。

DeepSeek 专项调用须由用户明确要求，且核对未结算用量后才能恢复。历史完整配对入口为 `run-paired.ps1 -OutputRoot <新的绝对目录>`；局部补跑沿 TB1 `run.ps1 -Case <原 case> -PriorOutput <对应版本上一目录>`，继承已用预算。不要用新目录清零预占。输出先用 TB1 summarize.py 汇总，再用 `compare.py <改后目录> <改前目录>` 核对。

## 后续实验方式

**决策**：教学实验优先使用 GPT-6.1-Sol 子代理。

**否决**：
- 日常指导迭代默认全量调用 DeepSeek：额外 API 成本不符合用户约束。
- 直接拿 Sol 改后输出对比 DeepSeek 改前输出：模型与执行环境变化，不能归因于指导。

**命门**：前后版本使用相同模型、推理设置、材料与工具说明；子代理生成真实输出，内容依据沿原文与代码判据。
**何时回头**：用户明确要求验证 DeepSeek 接入或该模型特有表现时，先说明具体问题与调用上限，再执行最小必要实验。

复用 TB1 四组输入，为前后指导各开独立、无父会话历史的子代理，每组每版一次；仅提供冻结的材料、背景、任务与该版指导，保存输入、模型、设置、输出和失败。子代理不得运行现有 DeepSeek 调用脚本。已完成八组配对及一次初学者补跑，详情见 [Sol 记录](sol/README.md)。

参数解析、状态变化、来源交付和最终请求装配继续由现有 Runtime/Server 测试验证。子代理内容对照不能证明生产 Provider 的协议、工具调用循环或最终交付链路通过，也不能充作真实学习者表现；原 DeepSeek 证据与未结算预占保留，未结算事项仅阻塞进一步 DeepSeek 调用。

## 已知限制与历史未结项

- DeepSeek 首次旧指导请求缺 usage，保留 1,032,768 token 未结算预占；该模型同工具旧指导对照及误解组补跑没有恢复。用户要求专项验证时，先核对历史用量再安排最小调用。
- 子代理已提供原文与来源；回放验证这些内容的工程交付，不代表实时 Provider 协议、模型自主工具循环或 DeepSeek 表现已复验。单次固定样例不支持成功率或稳定成本收益结论；Codex 用量与金额未知。
- 真实读者 0，无能力提升或迁移结论。TC2 内容问题、TC7–TC8、TO1 逆序范围问题沿原记录保留。
- 文档核对发现两处 HEAD 已有断链：CONTEXT.md 引用的 `docs/adr/0109-browser-owned-native-prose-flow.md`、代码链路引用的 `performance/account-layout-20261009/live-verification.json`，对应文件缺失。本轮新链接目标存在，详情见 [final-check.json](final-check.json)。
