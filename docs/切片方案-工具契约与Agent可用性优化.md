# 工具契约与 Agent 可用性优化切片方案

日期：2026-10-12。状态：TO0–TO2 已完成；TO3–TO14 待实施。

本方案以本项目提交 `67efb605f5d45157eeb769f0074b218eacd76dba` 为代码观察基线，以 REA 提交 `2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c` 为外部设计参考。执行时先确认相关代码是否已有后续变化，再按本文的符号和行为定位。

目标：让 Agent 更容易选对工具、一次填对参数、理解结果覆盖范围，并沿工具返回的信息完成读取、引用和真实交付。改进覆盖 Book、Tutor、Presentation、Goal、Reader/Memory 和专用 Build Executor；同时优化已有的契约单源、动态发现、证据分层、结果预算和评测机制。

TO1 已实现共同参数语义回收，TO2 已补共享 Book 示例并完成最终发布输入与真实分派验证，证据见 [TO1 实施记录](performance/tool-usability/to1/README.md) 与 [TO2 实施记录](performance/tool-usability/to2/README.md)。TO3–TO14 待实施，生产服务尚未发布本轮改动。

## 1. 既有基础与本轮改进依据

本方案延续 [全文定位与 Book 契约单源](切片方案-确定性全文定位与Book工具契约单源.md)、[开放能力发现与证据拓扑](切片方案-开放自然语言能力发现与Runtime证据拓扑.md)、[提示词与工具上下文治理](切片方案-Agent提示词与工具上下文治理.md)、[分层评测](切片方案-Agent分层评测与诊断闭环.md)。这些方案已经完成的部分继续作为基础。

| 现有机制 | 本次观察 | 优化目标 | 性质 |
|---|---|---|---|
| `BookToolContractRegistry` 统一输入类型、schema、别名和用途 | 远程 MCP 在 `mcp_read::tool_definitions` 另外补了大量参数说明；Resident 主要使用共同 description 和原始输入 schema | 将共同参数语义、示例和用途说明回收到同一来源，表面只补自己的发布引用、分页和额度说明 | 已确认语义投影差异 |
| `tool_schema::complete_spec` 已限定 operation 分支、必填字段和互斥组合 | TC2A 真实轨迹仍出现 `move` 字符串、顶层错放、漏传和不适用动作 | 为实际暴露的 operation/状态提供可执行示例，测量参数错误是否下降 | 已观察到模型使用错误 |
| Routing Card 已有用途、避免场景、副作用和前置条件 | `ToolSearchMatchV2` 未直接返回用途/避免场景；内部 `CapabilityPlan` 的 unmet/blocked 未作为公共恢复提示序列化 | 发现结果帮助完成下一步，并在可公开的能力层解释缺失前提 | 已确认信息投影缺口 |
| 位置账本、正文证据账本、来源引用各有所有权 | 调用者仍需把候选位置转换成下一次读取参数 | 在候选上提供确定性的读取调用，保留 Agent 对候选的选择 | 可用性优化，收益待测 |
| `ConceptCandidateSet` 已有 `matched_count/returned_count/truncated` | `Context` 只有 `anchor/items`，默认取前 10 项，未说明裁掉多少 | 补候选覆盖信息，区分已知候选数量、交付分页和结构产物可用性 | 已确认结果表达缺口 |
| 工具结果已有预算、投影、回执和 continuation | 通用结构化裁剪先移除 `warnings`，再缩减数组/字符串；通用 `RefineCall` 指导不一定给出具体可执行参数 | 保留会改变解释的限制，给出可执行的续取路径 | 已确认策略局限；逐类验证实际影响 |
| 本地/远程 MCP 都从输入契约发布工具 | Book 的 `result_contract` 是标识字符串；观察的工具定义未发布 `outputSchema` | 由结果所有者描述真实交付结构，连同副作用注解供消费方使用 | 契约完善机会 |
| 已有单元、边界、真实 Agent 与成本证据 | MCP9 已完成一次真实新 Agent 阅读；TC2A 已做固定样本对照 | 在现有轨迹和测试上比较完成率、误调用、正文证据和成本 | 扩展现有评测 |

具体基线证据：

- [TC2A 记录](performance/tutor-continuation-tc2a.md)：六次固定样本观察中，首轮估算输入从 17,250 降为 14,020 token，但仍有工具参数错误、一次教学未完整交付，以及独立的数学条件错误。
- [MCP9 实现与验收](内测MCP-MCP9实现.md)：新 Agent 自行执行目录、定位、两页正文读取，正确得到 37 升变为 19 升、减少 18 升；引用范围与消费记录已有确定性验证。
- [MCP 发布运行单](内测MCP-发布运行单.md)：远程发布、来源版本和额度均为实际在用边界；优化需要保持它们的语义。

## 2. 共同语义的所有权

**决策**：在现有契约所有者中补齐共同语义。

**否决**：
- 新建跨 Rust/TypeScript 总注册表：增加第二份所有权和同步成本。
- 把远程 MCP 全部描述复制给 Resident：混入发布引用、收费和传输细节。

**命门**：输入语义留在 `book-tool-contracts`；结果 schema 留在结果类型所属模块；服务层组合表面投影。
**展开**：[ADR-0088](adr/0088-deterministic-text-occurrence-search-and-canonical-book-tool-contracts.md)、[本方案 TO1](#to1共同参数语义回收)。

## 3. 工具边界与调用者选择权

**决策**：改善现有工具的可选性与可用性。

**否决**：
- 将定位、读取、综合合成默认自动工作流：会提前替调用者决定候选和证据。
- 为消除参数错误拆出大量工具：先验证当前状态投影和示例的效果。

**何时回头**：固定任务中仍有稳定的分支混淆，或同类离散读取确有主要串行等待成本。
**展开**：[ADR-0113](adr/0113-open-natural-language-capability-routing-and-runtime-evidence-topology.md)、[后续条件项](#10-后续条件项)。

## 4. 证据与结果交付

**决策**：以实际读到的范围和可执行续取为准。

**否决**：
- 将候选、预览或目录直接当正文证据：改变既有证据语义。
- 用裁剪后的短结果代表完整结果：会丢失覆盖状态与解释条件。

**命门**：定位候选、规范正文、已接纳证据与用户来源引用沿现有账本流转。
**展开**：[ADR-0086](adr/0086-runtime-owned-user-visible-source-references.md)、[ADR-0087](adr/0087-provenance-aware-answer-delivery-and-compact-provider-history.md)。

## 5. 效果判断

**决策**：以任务交付和确定性证据判断收益。

**否决**：
- 仅比较 schema token 或工具次数：可能以漏读、漏引用或未交付换取下降。
- 重复采样直至成功：会抹掉本来要观察的使用失败。

**命门**：冻结任务、模型设置、材料版本和采样预算；参数正确与教学内容正确分别验收。
**展开**：[TO14](#to14真实-agent-对照与可复用阅读案例)、[分层评测方案](切片方案-Agent分层评测与诊断闭环.md)。

## 6. REA 原仓库怎么找、怎么读

原仓库：[morluto/rea](https://github.com/morluto/rea)。本节所有源码链接固定到提交 `2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c`，后续 Agent 可以直接打开，不依赖本次会话的临时下载目录。

| 代号 | 原仓库入口 | 要读出的设计 | 本项目对应工作 |
|---|---|---|---|
| R1 | [docs/tool-design.md](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/docs/tool-design.md)、[rea-tool-design/SKILL.md](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/.agents/skills/rea-tool-design/SKILL.md) | 按调用者目标设计工具；保留会改变下一步的选择；同类批处理、结果覆盖和真实消费方验证 | 全方案的设计取舍 |
| R2 | [toolContractTypes.ts](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/src/contracts/toolContractTypes.ts)、[toolContracts.ts](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/src/contracts/toolContracts.ts) | 输入、输出、用途、effects、annotations、examples 共同构成可执行契约 | TO1、TO2、TO12 |
| R3 | [toolContractHelpers.ts](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/src/contracts/toolContractHelpers.ts)、[toolContractExamples.ts](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/src/contracts/toolContractExamples.ts)、[advertisedInputExamples.test.ts](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/src/contracts/advertisedInputExamples.test.ts) | 示例经过输入解析；检查实际发布示例是否合法、是否有区别、是否携带多余大记录 | TO2–TO6 |
| R4 | [enhancedToolContracts.ts](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/src/contracts/enhancedToolContracts.ts) | `binary_overview` 与精确工具的选择；`batch_decompile` 按项返回成功/失败和顺序 | TO7；同类批读取的条件项 |
| R5 | [contractToolHandler.ts](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/src/server/contractToolHandler.ts)、[toolResult.ts](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/src/server/toolResult.ts)、[EvidenceMcpServer.ts](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/src/server/EvidenceMcpServer.ts) | 契约执行、记录、实际交付的边界；有界结果和后续读取；成功/错误的协议差异 | TO10–TO12 |
| R6 | [evidence.ts](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/src/domain/evidence.ts)、[relationshipCoverage.ts](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/src/domain/relationshipCoverage.ts) | observed/derived/inferred、来源、限制；complete/partial/unknown 相对于已观察集合的含义 | TO8、TO9、TO11 |
| R7 | [docs/mcp-contracts.md](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/docs/mcp-contracts.md)、[contractPresentation.test.ts](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/tests/boundary/mcp/contractPresentation.test.ts) | canonical schema 与 advertised schema 的区别；真实 MCP 结构化内容、错误和能力恢复提示 | TO7、TO12 |
| R8 | [generate-product-catalog.mjs](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/scripts/generate-product-catalog.mjs)、[product-catalog.mjs](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/scripts/lib/product-catalog.mjs) | 从运行时契约产生目录和参考文档，减少手写副本漂移 | TO13 |
| R9 | [docs/testing.md](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/docs/testing.md)、[largeToolResult.test.ts](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/tests/boundary/mcp/largeToolResult.test.ts)、[工具使用 skill](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/.agents/skills/reverse-engineer-anything/SKILL.md) | 区分真实边界、替身测试、模型行为；已知目标直达、先看有界结果、实际失败后诊断 | TO10、TO14 |

建议读序：先 R1 明确设计标准，再读本切片引用的 R2–R9；用测试确认实际行为。需要看工具选择时，在 R4 搜索 `binary_overview`、`batch_decompile`；需要看示例时，在 R3 搜索 `examplesFor`；需要看结果时，从 R5 的执行入口沿返回值找到 R7 的实际发布结构。

需要本地全文搜索时，在本项目目录之外建立独立参考 checkout。下面的 PowerShell 命令只用于获取设计源码：

```powershell
$reaStudy = Join-Path $env:TEMP ('rea-tool-study-' + [guid]::NewGuid().ToString('N'))
git clone https://github.com/morluto/rea.git $reaStudy
git -C $reaStudy checkout --detach 2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c
rg -n 'examplesFor|outputSchema|annotations|batch_decompile|binary_overview' $reaStudy/src/contracts
```

源码阅读不需要安装 REA、连接逆向服务或运行 provider。研究新版本时记录新的 commit 和变更依据；上游 `main`、npm 包、当前机器安装的 MCP 可能不是同一版本。项目行为的依据是本项目代码、既有 ADR 和真实请求，REA 用于借鉴设计。

## 7. 本项目入口与改动所有权

| 范围 | 阅读/改动入口 | 所有权 |
|---|---|---|
| 共同 Book 输入语义 | [book-tool-contracts/src/lib.rs](../crates/book-tool-contracts/src/lib.rs)：`BookToolContract`、`input_schema`、typed input | 输入、共同参数解释、用途、共享输入示例 |
| Resident 声明与 operation 约束 | [orchestrator.rs](../crates/runtime/src/orchestrator.rs)：`declared_tool_specs`；[tool_schema.rs](../crates/runtime/src/tool_schema.rs)：`complete_spec` | Resident 组装和既有 operation 约束 |
| 状态专属工具面 | [tutor.rs](../crates/runtime/src/tutor.rs)：`spec_for_context`；[presentation_author.rs](../crates/runtime/src/presentation_author.rs)：`spec` | 当前 Tutor 状态、演示候选与交付语义 |
| 发现与副作用 | [tool_registry.rs](../crates/runtime/src/tool_registry.rs)：`ToolRoutingCard`；[tool_exposure.rs](../crates/runtime/src/tool_exposure.rs)：`search_and_activate`、`CapabilityPlan` | 工具完整清单、路由信息、可见性、能力前提 |
| Book 结果 | [read-tools/src/lib.rs](../crates/read-tools/src/lib.rs)：`Context`、`ConceptCandidateSet`、`SearchTextResult`、`context` | 领域结果、覆盖信息和结果类型 schema |
| 结果交付与证据 | [tool_result.rs](../crates/runtime/src/tool_result.rs)：`project_text/project_search/project_structured`；`orchestrator::observe_tool_evidence` | 模型实际收到的结果、续取、接纳证据范围 |
| MCP 表面 | [mcp.rs](../crates/server/src/mcp.rs)：`tools_list_result`；[mcp_read.rs](../crates/server/src/mcp_read.rs)：`tool_definitions`、`McpRead` | 名称映射、发布版本、传输分页、额度和 MCP 输出结构 |
| Executor | [build-executor-tool-contract.ts](../packages/core/src/build-executor-tool-contract.ts)、[build-executor-mcp.ts](../skills/build/build-executor-mcp.ts) | 专用 Executor 四工具契约、传输和不透明引用 |
| 验证 | [tool_contract_tests.rs](../crates/runtime/src/tool_contract_tests.rs)、[mcp_contract_tests.rs](../crates/server/src/mcp_contract_tests.rs)、[Core 工具适配测试](../packages/core/test/build-executor-tool-adapter.test.ts) | 原测试继续使用，按具体缺口补最小用例 |

`read-tools` 已依赖 `book-tool-contracts`。结果 schema 应在 `read-tools` 或结果实际所有者处生成，再由 Runtime/Server 组合；不要反向引入依赖环。Executor 的 TypeScript 契约继续独立维护。

术语沿用 [CONTEXT](../CONTEXT.md) 中的 BookToolContractRegistry、工具暴露计划、本轮证据账本、历史 Tool 回执、Provider 历史投影、Concept Candidate Set、Reader UI Control Plane。下文“示例”“覆盖字段”“后续调用”是现有契约的表示与说明，不增加领域实体。

## 8. 切片顺序与全工具覆盖

| 切片 | 交付 | 前置 | 状态 |
|---|---|---|---|
| TO0 | 基线、源码导航、切片方案和文档入口 | 无 | 已完成 |
| TO1 | 共同参数语义回收 | TO0 | 已完成，见 [实现与验证](performance/tool-usability/to1/README.md) |
| TO2 | Book 示例与实际发布示例验证 | TO1 | 已完成，见 [实现与验证](performance/tool-usability/to2/README.md) |
| TO3 | Tutor 当前状态的有效示例 | TO2 | 待实施 |
| TO4 | Presentation 操作示例 | TO2 | 待实施 |
| TO5 | Goal、Reader、Memory 的用途和动作示例 | TO2 | 待实施 |
| TO6 | 专用 Executor 四工具生命周期示例 | TO0 | 待实施 |
| TO7 | 发现结果的用途和能力恢复提示 | TO1 | 待实施 |
| TO8 | 定位结果到原文读取的参数交接 | TO1 | 待实施 |
| TO9 | Context 候选覆盖与结构可用性 | TO0 | 待实施 |
| TO10 | 正文/搜索结果的可执行续取 | TO8 | 待实施 |
| TO11 | 结构化结果预算下的语义保留 | TO7、TO9、TO10 | 待实施 |
| TO12 | Book 结果 schema 与副作用注解 | TO2、TO11 | 待实施 |
| TO13 | 从实际契约产生工具参考文档 | TO3、TO4、TO5、TO6、TO7、TO12 | 待实施 |
| TO14 | 真实 Agent 对照与可复用阅读案例 | TO13 | 待实施 |

推荐先完成 TO1→TO2→TO3，直接覆盖已有 TC2A 参数错误；随后推进 TO7→TO8→TO9→TO10→TO11。TO4、TO5 和独立的 TO6 按各自所有权接续，最后完成输出契约、文档与整体验收。每片独立记录实际影响，不以等待全方案结束为由延后本片验证。

| 工具族 | 示例必须教会什么 | 主切片 |
|---|---|---|
| `book.search_text` | 字面定位、首次/前后出现、范围过滤、原查询续页 | TO2、TO8、TO10 |
| `book.concept` | 名称/别名召回，多个候选由调用者选择，候选数量的含义 | TO2、TO8、TO9 |
| `book.query` | 语义问题的 intent、独立 targets、逐项 obligations 与证据需求 | TO2 |
| `book.synthesize` | 使用已确定的证据范围完成综合，明确证据不足 | TO2 |
| `book.context/structure` 与 profile 读取 | 结构视图和位置用于规划；覆盖状态与原文读取各自有含义 | TO2、TO8、TO9、TO11 |
| `book.text`、`source.present` | 读取规范正文，再从实际观察到的原句绑定用户来源 | TO2、TO8、TO10 |
| `tutor.step` | 当前状态允许什么 operation；`move` 的真实嵌套结构和证据绑定 | TO3 |
| `presentation.author` | 九种 operation、已交付 reference 与 candidate 的区别、编辑到交付 | TO4 |
| `goal.update` | working、refine、revise 的职责与不同字段集 | TO5 |
| `reader.note/highlight`、`memory.save` | 笔记、高亮、记忆各自的对象、授权意图和成功回执 | TO5 |
| `tool.search` | 用户任务如何填能力、scope、operation、effect；怎样理解匹配和阻塞 | TO7 |
| Executor 四工具 | open/resume、顺序输入、generation 授予、候选提交 | TO6 |

## TO0：方案与基线入口

**产出**：本文及既有方案的后续入口；以固定 commit 链接保存上游出处，以本地符号保存改动入口。

**完成判据**：所有必选切片有范围、所有者、完成行为和失败后的处置；本地链接、上游固定版本路径和切片前置关系可解析。

**验证**：只检查本次文档的链接、路径、依赖和改动格式。发现不存在的入口则纠正路径；发现跨模块任务无法单片验收则重新划分本片。TO0 不执行应用测试或真实模型采样。

**本次结果**：35 个本地文件链接、20 个 REA 固定版本源码路径、文内锚点与 TO0–TO14 的前置关系检查通过；新增文档及既有入口改动的格式检查通过。

## TO1：共同参数语义回收

**目标**：Resident、本地 MCP 和远程 MCP 对同一 Book 参数给出相同的含义、默认值、单位和上下游映射。

**输入/所有者**：R1、R2；`book-tool-contracts` 的输入定义；`mcp_read::tool_definitions` 的现有补充说明；`orchestrator::declared_tool_specs`。

**改动**：把 `lid/end_lid`、`query`、`scope/relative_to`、`cursor/page_size`、`granularity/k` 等共同语义移到现有共同契约；按工具解释同名字段，例如字面搜索的 query 与概念召回的 query。将 `use_when/do_not_use_when` 用于各表面的适量说明。远程表面继续补 `published_book_ref`、UTF-16 分页、来源链接和额度。

**产出**：共同字段说明只有一个权威来源；每个表面保留清晰的差异。仓库编写的模型描述使用既有英文约定，用户原文与书内原文保持原语言。

**完成判据**：比较同一逻辑工具的公共字段时，required、enum、default、单位、范围边界和参数来源一致；远程专属字段只出现在其表面。既有输入合法性和分支条件保持成立。

**验证及处置**：扩展共同契约与 MCP schema 测试，识别字段说明漂移或运输字段混入；失败则修正共同定义或对应投影。记录每个实际发布工具的 schema/description 字节量，为示例引入后的比较提供基线。

**本次结果（2026-10-12）**：共同 typed input/schema 已覆盖字段及嵌套范围说明，Context 默认 near/10 与读取实现共用常量；本地 MCP 使用共同用途说明与正确别名，远程仅保留自身投影。41 项正确性测试通过，改前/改后工具定义各导出一次；完整公共字段一致，原有输入约束与远程专属语义保持。逐工具字节量、日志及已知逆序正文范围问题见 [TO1 实施记录](performance/tool-usability/to1/README.md)。下一片为 TO2。

## TO2：Book 示例与实际发布示例验证

**目标**：为高频选择与参数误区提供少量可执行示例，并验证模型最终看到的版本。

**输入/所有者**：R2、R3、R7；共同 Book 契约、Runtime 既有 Registry/ToolSpec 组装、Server MCP 投影；现有 `tool_contract_tests`。

**改动**：在共同输入契约旁维护 Book 示例，至少覆盖字面搜索、语义 query、概念候选、正文范围、综合和来源绑定的不同用途。Resident 专有的 `source.present` 示例留在 Runtime。示例采用同一小型确定性书籍夹具中的真实 LID；远程示例由投影补实际夹具发布引用。顶层示例展示能够直接提交的完整 input，字段内说明解释值的来源。

**关键样例**：

- “找这个词第一次出现/当前位置之前出现”：`book.search_text`，使用公开的匹配模式、scope 和 order。
- “这个概念可能有哪些含义”：`book.concept` 返回多个候选后由调用方选择。
- “比较两个位置的条件”：`book.query` 的 targets 分开表达两端，obligations 写清要比较的条件；后续覆盖两端证据。
- “依据这些已确定段落总结”：按 `book.synthesize` 当前真实输入类型表达证据集合。
- “引用刚读到的原句”：`source.present.quote` 来自可验证的原文；来源身份由 Runtime 绑定。

**示例装配规则**：canonical 示例 → 当前表面名称/字段映射 → 当前状态 schema 投影 → 选取适用示例 → 实际 Provider/MCP 工具定义。若现有 Provider 不保留 schema 的 examples，沿现有 description 生成精简示例；先查实际请求，避免把同一个示例同时塞进多个位置。

**完成判据**：示例通过最终 advertised schema 和真实输入解析；夹具调用走现有 dispatch，返回对应类型/来源。多操作工具只附当前有效分支的示例，所需数量由分支差异决定；简单单操作不强制重复示例。

**验证及处置**：测试应抓住“canonical 合法但发布后字段被删”“别名未转换”“有效示例在真实解析时被拒绝”；失败修相应投影或示例。错误样例放测试中，模型侧只发布可用调用。该片建立后续 TO3–TO5 复用的最小示例校验能力，沿用现有 Registry。

**本次结果（2026-10-12）**：共同契约发布 8 个完整输入，Resident 增加 1 个来源原句示例，远程在适用的 6 个示例中补真实发布夹具引用。Native/ReAct 实际请求投影保留 schema 示例；本地/远程 MCP 和 Resident 的示例走真实分派，来源读取、精确绑定与计次通过。42 项正确性测试通过；最终 61 个工具 schema、23 个发布示例验证通过，原输入约束与 description 保持。三个表面 schema 分别增加 1,044／949／1,095 字节（Resident／本地／远程）。实现、快照和复现入口见 [TO2 实施记录](performance/tool-usability/to2/README.md)。下一片为 TO3。

## TO3：Tutor 当前状态的有效示例

**目标**：降低已观察到的 `tutor.step` 参数和动作选择错误。

**输入/所有者**：R3；`tutor::spec/spec_for_context`、`tool_schema::complete_spec`、Tutor 现有上下文夹具；[Tutor 接续方案](切片方案-Tutor下一步协商与低门槛阅读入口.md)、[ADR-0163](adr/0163-tutor-continuation-and-contextual-reading-entry.md)。

**改动**：覆盖 material、trace、select、outside、assess、understanding、evidence 的不同输入；select 示例突出 `move` 对象嵌套。根据当前观察/教学状态选取有效示例，target/current action 和 Runtime 绑定字段的要求与实际投影一致。提示/reveal 示例具备对应 hint/explanation；evidence 的用户表达证据与 hypothesis 两分支分别演示。

**产出**：普通源码契约、状态投影和模型侧示例一起变化；示例中的 source binding、action ref、事实引用来自当前测试上下文。普通入口只示范当前可执行的教学动作。

**完成判据**：每种受支持状态至少有一个能被真实分派接受的示例；选择、提示、正式评估、理解更新和暂停的实际权限一致。TC2A 四种已观察误用有稳定的拒绝/正确输入对照。

**验证及处置**：用既有 Tutor/工具契约测试抓“示例字段被状态投影删掉”“正式评估条件未满足”“无原话形成理解事实”；失败修示例选择或契约组装，保留原业务拒绝。该片真实模型对照遵循 TO14 的固定采样办法，单独报告参数错误和教学内容错误。

## TO4：Presentation 操作示例

**目标**：让 Agent 能准确完成创建、修改、预览和交付演示。

**输入/所有者**：R3；`presentation_author::spec`、`tool_schema::complete_spec`、现有 authoring/preview 测试。

**改动**：维护 prepare、render_plot、render_animation、read、search、patch、write、preview、deliver 九种操作的可执行示例。read/search/patch 展示 reference 或 candidate_id 的正确选择；write 展示基于已有对象修改与显式新建的区别；preview 使用 width 或 viewport 的一个有效组合，演示必要交互动作。

**关键链路**：已有演示 reference → read/search 定位 → patch 产生候选 → preview 检查 → deliver 得到交付回执。新建链路使用 write 的真实必填字段；正文来源引用与图形资产引用沿当前绑定规则传递。

**完成判据**：九种 operation 的示例分别通过实际 schema 和解析；至少一条已有演示修改链路按真实 fixture 完成，最终回执指向交付对象。候选与已交付引用不会在后续示例中混用。

**验证及处置**：以真实 authoring 测试抓 reference/candidate 混用、预览参数冲突、遗漏 readable_content、提前声称交付；失败改示例或与实际约束不一致的描述。保留当前演示协议和对象生命周期。

## TO5：Goal、Reader、Memory 的用途和动作示例

**目标**：澄清已有有副作用工具的职责，减少选错工具和字段被放进错误 operation。

**输入/所有者**：R1–R3；`declared_tool_specs`、`tool_schema::complete_spec`、既有 Reader/Memory handler 和测试。

**改动**：

- `goal.update`：working 使用 focus/next_move，可带计划项；refine 使用 interpretation/requirements；revise 另需真实用户回合和原话依据。示例按各自封闭字段集生成。
- `reader.note`：表达用户要保存的笔记及其原文落点，成功由笔记持久化回执确认。
- `reader.highlight`：表达当前实际支持的高亮动作和范围，明确与笔记内容的关系。
- `memory.save`：展示现有支持的记忆类型与真实输入，使用现行事实来源；笔记仍由 reader.note 拥有。

**产出**：各工具在既有定义旁拥有用途、有效输入和关键成功回执的例子。发现结果可以复用这些用途说明。

**完成判据**：goal 三分支字段不会混入；用户要求保存笔记时，执行和验证的是笔记记录；只解释选中文本的任务不产生写入；Memory 示例符合实际公开类型和证据要求。

**验证及处置**：扩展现有 `goal_operations_never_silently_discard_plan_fields`、`note_is_only_exposed_through_reader_note` 及对应真实持久化测试。发现错误工具被选中则修用途/示例，发现假成功则修实际回执判定；两类原因分别记录。

## TO6：专用 Executor 四工具生命周期示例

**目标**：让 Build Executor 明确每一步消费什么引用、确认到哪里、返回什么下一步信息。

**输入/所有者**：R2、R3、R7；TypeScript 的 `build-executor-tool-contract.ts`、现有 tool adapter、session、transport 和 stdio MCP；[有界语义传输方案](切片方案-executor有界语义传输与候选提交闭环.md)、[恢复与复用方案](切片方案-executor恢复代际与语义结果复用闭环.md)。

**改动**：为 `executor.open`、`executor.input.next`、`executor.generation.start`、`executor.submit_candidate` 补齐生命周期示例与必要结果说明。实例中的 opaque handoff/session/input/sink ref 由测试 session 产生；示范顺序输入确认、获得 generation 授予、向私有 sink 提交候选及既有恢复路径。

**版本依据**：观察基线的 contract 数组名为 V3，但 input.next 的请求版本为 v4；示例从当前实际常量/契约读取版本，按每个工具生成。

**完成判据**：示例可通过当前适配器与真实 session 流转；恢复示例沿既有授权代际，提交候选后由代码侧返回真实处置。专用 Executor 的工具面与 Reader 的 Book/Memory 工具面继续各自独立。

**验证及处置**：运行已有 tool-adapter、MCP、session、transport 和 version-boundary 中受影响测试。抓取错误版本、未确认输入直接生成、旧 sink 被误当当前目标等受支持恢复流程中的错误；失败修本片示例/适配，实际业务行为问题单列最小修复。

## TO7：发现结果的用途和能力恢复提示

**目标**：让 `tool.search` 的返回结果同时回答“哪个工具适合”和“当前缺什么才能继续”。

**输入/所有者**：R1、R4、R7；`ToolRoutingCard`、`ToolSearchMatchV2`、`ToolSearchOutcomeV2`、`CapabilityPlan`。

**改动**：匹配项从 Routing Card 投影精简 `use_when/avoid_when`、副作用和前提说明；增加请求能力层的 unmet/blocked 公开投影。能力缺证据时，指向现有定位/读取动作；缺用户写入意图时，解释当前允许的只读路径或需要的用户意图；确无可用能力时明确不能完成该动作。

**边界**：内部 block 含 tool 名、Runtime 隐藏原因和审计状态；公共输出仅包含当前调用者可获知的能力与恢复信息。恢复动作只引用当前可见或可合法发现的工具。`task_need/capability_plan/request_audit` 继续保留内部所有权。

**输入示例**：为几种真实用户任务提供完整 task、required_capabilities、scope、operation、effect_mode、max_results 组合；复杂中文任务可以检验发现效果。`task` 仍是自然语言，结构字段继续表达当前 ADR-0113 的权威边界。

**完成判据**：已有匹配时返回有用用途；未满足时能理解原因及合法下一步；Hidden 工具名不会经新字段泄露；发现动作本身不会执行 Reader/Memory 副作用。

**验证及处置**：用能力解析/暴露测试分别覆盖 evidence_required、locator_required、effect_not_authorized、profile 不适用及无结果；检查序列化后的真实公共 JSON。若出现内部计划泄漏则收窄投影；若恢复建议在当前状态不可执行则修建议来源。

## TO8：定位结果到原文读取的参数交接

**目标**：Agent 选定候选后可以复制准确参数读取原文。

**输入/所有者**：R1、R6；search_text/concept/context 的结果所有者与各表面适配，现有 `next_calls`、位置账本和正文读取。

**改动**：为候选证据范围附最小后续调用信息，沿已有 NextCall/next_calls 表达可执行 `book.text` 或 `book_text` 参数。单 LID 与连续范围分别保留；远程调用补同一 `published_book_ref`。语义候选先由 Agent 选择，随后消费对应读取调用。

**拟定交接语义**：以下是表达形状，实际字段名优先复用现有定义；尖括号处由结果中的真实值构造。

```text
selected_candidate.read_call:
  tool: book.text
  arguments: { lid: <start_lid>, end_lid: <inclusive_end_lid> }

remote projection:
  name: book_text
  arguments: { published_book_ref: <selected_publication>, ...range_arguments }
```

**完成判据**：按返回调用可以读到该候选的规范正文；两端范围和发布版本一致；候选/预览本身只保留当前被允许的证据含义。字面命中范围可证明出现事实，扩展解释仍需相应正文。

**验证及处置**：从夹具结果提取返回参数直接走读取 dispatch，并核对原文范围和证据账本。若参数不合法修映射；若未读候选被接纳为完整证据修接纳路径；若增加大量重复参数则按实际候选数量和预算调整投影。

## TO9：Context 候选覆盖与结构可用性

**目标**：调用方知道当前看到了多少已知关系，以及为什么不能声称完整。

**输入/所有者**：R6；`Context`、`context`、`ConceptCandidateSet`；既有 manifest/产物 availability。

**改动**：在 `context` 排序裁到 k 之前记录候选总数，返回 `total_candidates`、`returned_count`、`truncated` 或项目已有等义字段；字段说明明确总数属于本次规则召回的已知候选集合。同步所有实际消费方的序列化类型与结果投影。

**三层含义**：

| 层次 | 判据来源 | 可以说明什么 |
|---|---|---|
| 交付分页 | 当前结果页、next_calls、complete | 已生成结果是否还有未发送部分 |
| 候选裁剪 | 排序前候选集合及 k/limit | 本次已知候选是否全部返回 |
| 结构产物可用性 | 已有 manifest、构建状态、availability | 当前是否具备某种已构建关系能力 |

**完成判据**：候选数大于 k、等于 k、零候选三类结果有正确计数；缺失关系产物通过现有可用性路径表达。上述任一“完整”都不能推导出书中不存在其他语义关系。

**验证及处置**：准备真实规则可召回超过 k 个候选的小夹具以及缺产物夹具。计数错误修裁剪前统计；空结果掩盖产物不可用则修可用性传播。已有 Concept 的 matched_count 语义继续成立。

## TO10：正文/搜索结果的可执行续取

**目标**：对受支持的大结果，Agent 按返回信息能继续读取，覆盖范围可以核对。

**输入/所有者**：R5、R9；`project_text/project_search`、`ToolContinuation`、远程正文 UTF-16 和集合分页、现有规范正文存储。

**改动**：先区分工具原生 cursor 与模型上下文预算裁剪。连续正文优先沿真实叶节点或已有范围/offset 续取；搜索保持原查询、scope、order 和合法 cursor。单个受支持的大叶节点不能再细分时，使用既有规范正文的范围读取能力给出确定续取；需要扩展 Resident 参数时同步共同契约和具体消费面。

**完成判据**：串接当前页与所有所需续页可恢复请求范围，既不漏段也不重计证据；每次 continuation 都可直接调用。大单叶节点、跨叶范围、原生搜索多页和预算裁剪分别有明确信号。原文证据只接纳模型实际收到的部分。

**验证及处置**：沿用 `tool_result_projection` 与 MCP 分页边界测试，新增当前受支持场景中缺失的一例。确定性比较拼接正文/命中集、范围和实际账本；若漏页修游标/切点，若重复暴露当前页修续取参数。读取后续页应利用既有正文或保留结果，避免为恢复已生成的结果重跑昂贵语义调用。

**远程约束**：一次成功续页当前计一次内容调用；实现不得在这片中隐式改变额度、来源链接和发布版本绑定。

## TO11：结构化结果预算下的语义保留

**目标**：结果变短以后仍能被正确解释，并保留真实可用的后续动作。

**输入/所有者**：R5、R6；`project_structured`、`ToolOutputPolicy`、查询/概念/结构/profile 返回类型，以及 TO7/TO9 新投影。

**改动**：按当前工具结果的真实语义区分必要说明和可分页主体。优先保留 coverage、影响结论的 warning/limitation、来源身份、错误原因及可执行 continuation；先缩减正文/候选等大集合。重复状态文案可以精简。让已有 `RefineCall` 使用当前工具真实支持的缩小范围参数，能够续取时返回具体 NextCall。

**完成判据**：预算内结果仍是合法 JSON；partial/truncated 与实际交付一致；缺失 graph、截掉候选、只有部分证据等状态可辨；来源引用保持原身份。纯粹重复的说明可以省去，改变结论适用范围的信息继续可见。

**验证及处置**：给已存在的 query、concept、structure/profile 路径构造会触发预算的正常规模结果，检查必要字段和续取。若通用裁剪破坏某工具语义，将该工具的最小投影规则放在现有结果分派处。若最小语义仍超过预算，返回明确的有界 partial/error 和恢复动作，记录其支持边界。

## TO12：Book 结果 schema 与副作用注解

**目标**：消费方能从工具定义知道成功结果、部分结果和副作用的真实含义。

**输入/所有者**：R2、R5、R7；Book 结果类型、`ToolResultEnvelope`、本地/远程 MCP 定义、Runtime 现有 effect 元数据。

**改动**：首先为共同 Book 工具补结果 schema，由结果类型所有者维护；服务层组合本表面的实际返回形状。本地 MCP、远程 MCP、Resident 的 delivered 结构各按当前实现描述，远程分页元数据、sources、usage 继续属于远程结果。Runtime 可从已有 effect 元数据投影 annotation；副作用语义由原执行层决定。

**必须区分**：canonical 完整领域结果、Resident 投影后的 model body/envelope、MCP 实际 structuredContent。成功/partial 的 schema 必须匹配各自实际返回；错误按现有协议的 isError/诊断结果表达。示例字段完整与结果全量是两个不同概念。

**注解判定**：readOnly/destructive/idempotent 等只取有代码依据的值。混合读写 operation 工具使用整体保守语义并在分支描述中解释；会计费的远程读取应说明重复调用的实际消费，不能从“读取内容”直接推导“重试无额外影响”。

**完成判据**：Book 所有本次公开的成功和分页样本通过最终 advertised output schema；部分结果不冒充完整领域结果，错误不强塞进成功结构。无法准确描述的动态字段保留其真实边界，不用任意 JSON 掩盖缺少结果约束。

**验证及处置**：从真实 MCP tools/list 与 tools/call 取得 schema/返回值，验证完整、空集、续页、partial、error。失败时定位为领域 schema、表面包装或运行返回不一致，再修相应所有者。各表面使用已有版本策略；若需要变更调用协议，在本片明确记录具体接口变化和现有消费方更新范围。

## TO13：从实际契约产生工具参考文档

**目标**：后续开发者、Agent 和使用者看到的参数、示例、效果与实际发布工具一致。

**输入/所有者**：R8；TO1–TO7 的契约/示例和 TO12 结果定义；现有 docs 与构建脚本组织。

**改动**：提供一次明确调用的文档生成入口，消费现有 Rust 契约导出与 TypeScript Executor 契约。生成按表面区分的工具目录，包含用途、输入、有效示例、结果覆盖、效果及来源符号；教程部分引用生成示例并保留简短任务解释。

**产出**：计划新增 `docs/工具契约参考.md` 及必要的既有脚本入口扩展。工具清单从运行时权威来源获得，生成结果标明生成方式和源定义；新工具有唯一维护入口。

**完成判据**：生成目录与实际公开集合一致；Resident、远程 MCP、专用 Executor 的能力边界清楚；修改一个共同字段或示例后，生成文档反映相应变化。状态专属 Tutor 示例说明对应状态。

**验证及处置**：执行生成入口并比较输出，检测漏工具、过期示例和生成漂移；失败修导出或模板。直接比较结构/文本即可。生成过程不调用模型、不需要生产账号，也不扫描材料正文。

## TO14：真实 Agent 对照与可复用阅读案例

**目标**：判断工具改进是否让真实任务更容易完成，并沉淀可重复的使用案例。

**输入/所有者**：R1、R9；[分层评测方案](切片方案-Agent分层评测与诊断闭环.md)、TC2A/MCP9 原记录、现有请求审计与评测执行器。

**基线安排**：各工具族开始改动前固定本族的任务输入、材料版本和期望行为，保留基线实际请求。真实对照使用冻结的基线代码与改后代码、相同模型设置和预算；可以在最终验收时执行两个版本。本文的既有历史样本用于选问题，跨版本模型配置不同的历史结果不直接充当新实验对照。

| 场景 | 必须观察的行为 | 客观判据 |
|---|---|---|
| 解释已有正文选区 | 使用已有证据完成局部解释 | 来源落在已验证选区；无无关写动作 |
| 字面首次/之前出现 | 选择全文定位及正确排序/范围 | 与确定性 occurrence 结果一致 |
| 语义歧义与两处比较 | 候选由调用方选择；比较两端确有证据 | 两侧实际读取范围、目标条件齐全 |
| 查询/综合证据不足 | 保留限制，采取有效补读或明确不足 | 未读片段没有变成引用 |
| Context 候选被裁/结构不可用 | 区分分页、候选覆盖和可用性 | 结果字段与夹具真实状态一致 |
| 超预算正文/结构化输出 | 跟随有效 continuation | 请求范围完整重建，来源版本一致 |
| Tutor 继续/提示/理解更新 | 发出当前状态的合法 operation | 参数拒绝次数、真实 TeachingMove/交付记录 |
| 修改已有演示 | 读取/修改候选/预览/交付 | 正确对象发生预期修改，交付回执有效 |
| 保存笔记与纯解释 | 对前者保存真实记录，对后者保持只读 | 持久记录及执行副作用与用户意图相符 |
| Executor 正常/恢复 | 使用当前授权引用推进完整生命周期 | 现有 session/候选接纳记录一致 |

**执行方式**：确定性用例先检查 schema、参数、结果、证据、持久化和额度；真实模型使用覆盖不同表达的固定任务样本。每组预先写明模型、重复次数、token/费用上限和停止条件，受现有项目费用约定约束。材料、请求、工具返回、最终交付和确定性判定保留为同一运行证据。

**比较指标**：实际任务完成、参数校验拒绝/重试、选错工具、无效探索、引用/持久化正确性、端到端耗时、每次请求活动输入量、累计 usage 和实际费用。活动上下文与累计 billed token 分别报告。表面更短但任务失败的样本按失败保留。

**产出**：计划将结果保存为 `docs/performance/tool-usability/` 下按切片/场景区分的记录，并写一份实现汇总；先复用 MCP9 的完整阅读任务，形成可从目录发现、定位、续读到可核对引用的教程。案例描述已实际运行的行为，演示材料可以使用现有确定性夹具。

**完成判据**：每项变化都有对应的正确性证据；真实模型结果同时保留成功和失败，报告适用范围。若没有测出减少参数错误/调用耗时等预期收益，则在汇总中明确，并回退无收益且明显增加输入成本的示例投影。内容数学错误由独立客观算例判定，不能用 schema 通过代替。

## 9. 验证入口与运行原则

每次执行验证前写清楚本次要抓的失败和失败后的改动位置。以下是已存在的套件入口；具体切片只运行受影响的子集。新增用例名应在实现后记录，测试过滤结果必须实际包含目标用例。

| 验证入口 | 识别的具体失败 | 失败后的处置 |
|---|---|---|
| `cargo test -p book-tool-contracts --lib` | 公共字段、解析和 schema 不一致 | 修共同输入契约 |
| `cargo test -p runtime --lib tool_registry::contract_tests` | operation 必填/互斥、示例与真实解析不一致 | 修定义、状态投影或示例；文件名过滤 `tool_contract_tests` 会选中 0 项 |
| `cargo test -p runtime --lib tool_exposure` | 发现/授权/可见性投影错误 | 修路由卡和公开结果映射 |
| `cargo test -p runtime --lib tool_result_projection` | 结果预算破坏范围、格式或续取 | 修对应结果投影 |
| `cargo test -p server --lib mcp_contract` | 实际 MCP 参数/结果/来源/额度边界不一致 | 修具体传输投影或执行路径 |
| `pnpm --filter @understand-book/core exec vitest run test/build-executor-tool-adapter.test.ts test/build-executor-mcp.test.ts test/automatic-build-executor-session.test.ts test/executor-transport.test.ts test/build-executor-version-boundary.test.ts` | Executor 版本、输入确认、候选提交和恢复示例不成立 | 修 TypeScript 契约或本片示例 |
| `pnpm --filter @understand-book/core typecheck` | Executor 类型与调用方不匹配 | 修受影响类型和消费方 |

Tutor、Presentation、Reader 的行为验证继续使用现有对应模块测试，定位到本片受影响用例后记录命令。TO9/TO12 涉及生成类型时运行其实际生成和消费方检查。确定性验证通过后，再按预先固定计划做真实模型验证；已经通过的检查在输入未变时沿用结果。

## 10. 后续条件项

以下两项保留为有证据后启动的独立工作，TO1–TO14 的完成不依赖它们。

| 条件项 | 启动证据 | 设计方向 | 验收 |
|---|---|---|---|
| 同类离散原文批读取 | 实际轨迹显示多个已选定离散范围，串行调用等待占主要耗时；已有并行读取仍不能解决 | 借 R4 的同类批处理：按输入顺序返回每项成功/失败，保留每段来源身份；先限定规范正文读取 | 在同一任务/材料上比较等待时间、输入成本、部分失败处理和原额度语义 |
| 拆分多 operation 工具 | TO3/TO4 等状态投影与示例改进后，固定样本仍稳定混淆特定动作 | 只拆发生混淆且任务意义独立的动作，复用原服务实现和权限来源 | 误用下降，工具发现和活动输入成本可接受，旧消费方按项目真实需要更新 |

需要批读取时，额度是否按项计算必须与现有计费约定明确衔接；需要调整 `tool.search` 结构化必填字段时，先提供当前使用失败证据并修订 ADR-0113。两者都属于另行冻结的具体接口变化。

## 11. 已知限制与范围

- REA 的领域是逆向分析。本项目借鉴契约、选择权、覆盖表达和验证方法；其 Provider 体系、证据存储身份机制和工具规模不能直接说明本项目也需要同样实现。
- 本方案给出具体设计和验证路径，尚未证明示例、output schema 或更丰富的发现结果会提高模型完成率。它们也可能增加每轮输入成本，TO14 负责给出实测结论。
- 参数示例能够针对已观察的格式错误；TC2A 的数学条件错误需要内容证据和独立判定。两类结果分别记录。
- coverage 的 complete 只说明约定集合/交付范围的完整性，不能证明知识图谱涵盖书内全部真实关系；无法从既有构建状态判断的能力明确保持 unknown。
- 结果 schema 第一批覆盖共同 Book 结果及其表面投影。Tutor、Presentation、Goal、Reader/Memory 的重点是现有 operation/状态约束与可执行示例；需要进一步类型化其动态输出时，以实际消费问题另列小片。
- 本轮复用现有账本、注册表、存储和版本策略。新增持久模型、跨语言统一框架、独立迁移层或新的内容摘要机制均需独立的具体需求依据。
- 真实模型服务不可用、费用预算未具备或某表面尚无真实运行条件时，记录缺失的验收，不将确定性测试通过写成真实 Agent 通过。生产部署按既有发布约定处理。

## 12. 给后续执行 Agent 的交接

1. 阅读本文的基线表、所有权表和目标切片。通过本项目符号定位当前实现，确认该片是否已有后续提交完成；每次只推进一个可独立验收的切片。
2. 打开第 6 节中该片标注的 R 编号链接。链接已固定 REA 版本；需要全文搜索时使用独立 checkout，记录实际阅读的路径、符号和提交。
3. 在目标切片开始前保留当前真实工具定义/请求和相关任务输入。示例应由项目现有夹具生成引用，再经过最终暴露 schema 和真实解析验证。
4. 只在该片的现有所有者内补齐设计，按第 9 节选择能检测具体失败的检查。沿项目公开支持的用法验证当前缺口。
5. 记录改动、测试和实际模型证据，更新本表状态与 [代码链路](代码链路.md)；结构边界确实变化时再同步架构/ADR。尚未运行的验收明确保留待办。

TO1、TO2 已完成。接手 TO3 时从 [TO2 实施记录](performance/tool-usability/to2/README.md)、[TO2 改后定义](performance/tool-usability/to2/after.json) 和本文 TO3 开始，按 Tutor 当前状态补有效示例；复用 `tool_registry::contract_tests` 中的最终示例/schema/真实解析验证与 Native/ReAct 请求投影检查，并保持三表面契约一致性。
