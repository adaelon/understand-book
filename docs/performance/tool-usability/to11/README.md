# TO11：结构化结果预算下的语义保留

日期：2026-10-12。基线：`93b476d` 加工作区 TO3／TO7–TO10。状态：已完成，未提交、未部署。

## 实施步骤

- [x] 阅读 checkpoint 全部冷启动入口及固定版本 R5／R6。
- [x] 保存发现、概念、Context、query、结构／profile 的预算失败。
- [x] 在原结果投影中保留必要语义，记录实际交付覆盖并生成可执行恢复参数。
- [x] 验证实际下一采样、来源身份、定位／正文证据边界及相关回归。
- [x] 更新方案、代码链路及 checkpoint，下一推荐切片为 TO12。

## 实现

沿现有 `ToolOutputPolicy`、`ToolContinuation`、候选计数和本轮两类账本实施。`tool_result::project_structured` 按当前结果类型选择可裁剪集合，整项缩减；用途、警告、限制、支持判断、绑定和来源身份保持完整。完整结果不增加投影字段。预算投影通过 `delivery.collections` 的 JSON pointer 分别记录原集合数与交付数；根层候选选择计数保持原含义。

| 路径 | 预算下的行为 | 可执行恢复 |
|---|---|---|
| `tool.search` | 先省略重复描述和匹配诊断，保留完整用途、前置条件、能力、阻塞原因及激活结果；省略字段显式列出 | 使用已有 `next_searches` 或具体能力与 `max_results=1` 再发现 |
| `book.concept` | 先减重复 `next_calls`，再按需减预览、出现位置及候选；读取提示始终有已交付定位来源 | `book.text` 读取返回的位置，包括省略了重复调用提示的位置 |
| `book.context` | 原 `total_candidates/returned_count/truncated` 保持候选选择含义，预算交付另计 | 使用真实 `k` 缩小返回候选集 |
| `book.query/synthesize` | 答案不足以完整交付时整段省略；保留支持判断和绑定，引用整项裁剪，原定位保存在 `delivery.read_lids` | 直接读原文，避免为恢复引用重新执行推理 |
| `book.structure/guide_path` | 集合整项裁剪，警告和可用性保持原值；省略项的原定位可继续读取 | `book.structure(at)` 或 `book.text(lid)`；guide_path 转结构定位，因为其 `at` 不会缩短完整路线 |
| profile／paper／source | 保留 profile 身份、警告、限制和来源引用；可选预览整段省略并说明 | 阅读指导的 critical／creative 可缩至 passive；其余只使用现有接口实际支持的恢复 |

`TurnLocatorLedger::observe_tool_result` 只从 query／synthesize／structure／guide_path 的 `delivery.read_lids` 接纳原定位。它授权后续读取，未交付的引用正文不进入 `TurnEvidenceLedger`。只有真实读取并交付的正文才能成为来源证据。

若必要语义整体仍超预算，返回显式 `semantic_budget_exceeded` 与 omitted 状态，按预算保留能力／错误／profile 身份，避免截短警告后继续呈现结论。错误路径优先省略 details，并保留错误原因、分类和所需能力。

## 实测结果

| 固定输入 | 改前 | 改后 |
|---|---|---|
| TO7 五项发现，原结果 5,122 字节，4 KiB 预算 | 4,068 字节；用途等文字被截短 | 2,525 字节；保留全部 5 项及完整用途、能力和恢复信息 |
| TO8 300 处概念位置，原结果 17,266 字节，16 KiB 预算 | 10,939 字节；189 处位置及调用 | 16,344 字节；全部 300 处位置、276 项读取提示；下一调用 `book.text({lid:"1.277"})` 实际成功 |

字节数仅指序列化 `model_body`，不含 envelope／receipt。概念结果多交付了 111 处位置；重复读取提示的省略由独立集合计数表达。数字与其他预算样本见 [sizes-and-results.json](sizes-and-results.json)，原预算观察见 [TO7](../to7/budget-observation.json)、[TO8](../to8/budget-observation.json)。

## 验证结果

Runtime **80 项通过，0 失败**，其中新增 TO11 **10 项**。覆盖 TO7–TO10、结果投影、契约、定位账本和来源交付，见 [runtime.log](runtime.log)。新增用例分布在 `structured_result_tests.rs`（6）、`book_handoff_tests.rs`（3）及 `tool_discovery_tests.rs`（1）。

- [预算样本](after/) 验证合法 JSON、实际字节预算、完整用途／警告／支持判断／来源身份、整项集合和显式省略。大错误 details 不挤掉必要错误原因；缺结构的警告无法容纳时返回明确省略。
- [resident.json](resident.json) 来自实际 Resident 循环和 Provider 下一次请求。Native／ReAct 可见恢复参数，全部 300 处概念定位可读；恢复前没有正文证据，实际读取后才接纳 1 项。
- query 的省略引用仍授权原叶读取，不授权其父范围；未交付引用不能成为来源证据。实际读取后仅接纳读到的叶。
- [collections.json](collections.json) 验证真实 60 段结构／Context 集合的缩小范围调用，经过 registry 校验和真实分派。
- [discovery.json](discovery.json) 验证 4,096／2,500／1,800 字节预算下完整阻塞原因及发现恢复，经过真实 schema、parser 和发现执行。paper 指导缩阶段也经实际分派。

改前失败保存在 [red.log](red.log) 和 [before/](before/)；最终通过结果以 runtime.log 为准。旧投影测试中 profile 与默认阅读指导的通用 RefineCall 期待已移除，这些参数不能缩小实际结果。

## 复现

在仓库根目录运行；Rust `+1.95.0`，使用既有 D 盘构建目录。本机首次编译遇到内存不足，最终运行将构建并行度降为 1、Runtime 测试 codegen units 降为 16，见 [build-memory-failure.log](build-memory-failure.log)。

```powershell
$env:CARGO_TARGET_DIR='D:/codex-build/understand-book-no-progress-195'
$env:CARGO_PROFILE_DEV_DEBUG='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
$env:CARGO_INCREMENTAL='0'
$env:CARGO_BUILD_JOBS='1'
$env:TEMP='D:/codex-build/to3-temp'
$env:TMP=$env:TEMP
cargo +1.95.0 --config 'profile.test.package.runtime.codegen-units=16' test -p runtime --lib -- to11 tool_result_projection to8 to9 to10 tool_registry::contract_tests locator_ledger source_presentation tool_search_to7 --test-threads=1
```

可选 `TO11_SNAPSHOTS` 指定预算样本目录，`TO11_RESIDENT`／`TO11_COLLECTIONS`／`TO11_DISCOVERY` 指定对应 JSON 文件路径。重跑时另选输出路径，保留改前证据。

## 已知限制

- 本片没有真实模型采样或生产部署。脚本模型验证实际运行链路，模型选择及任务收益由 TO14 验收；实验沿 [已确认模型约定](../to10/README.md#实验模型约定)。
- 集合计数表达当前规范结果的交付覆盖，不能证明图谱覆盖书中全部关系。Context 的 k 恢复是缩小选择，概念正文读取是读取已知位置，两者均不表示候选尾部分页。
- 未增加通用分页或结果保存服务。manifest、metadata、lexicon 没有更小范围的读取接口；必要语义仍放不下时明确省略。未列入本片的工具沿原投影策略。
- 预算针对 model_body。极小预算连最小诊断也放不下时保留原 JSON `null` 下限及 envelope partial；少于 4 字节的预算无法容纳该既有下限。

## 参考

已阅读 REA 固定提交 `2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c` 的 R5 `contractToolHandler.ts`、`toolResult.ts`、`EvidenceMcpServer.ts` 与 R6 `evidence.ts`、`relationshipCoverage.ts`，路径见 [TO 方案第 6 节](../../../切片方案-工具契约与Agent可用性优化.md#6-rea-原仓库怎么找怎么读)。本项目沿 ADR-0086／0087／0113 的来源和交付约束。
