# TO8：定位结果到原文读取的参数交接

日期：2026-10-12。基线：`93b476d` 加工作区已完成的 TO3／TO7。状态：TO8 已实现并验证，未提交、未部署。

## 实施步骤

- [x] 阅读 checkpoint 全部冷启动入口、TO8、现有账本与 MCP 分派。
- [x] 保存固定候选结果基线。
- [x] 在结果所有者附候选 `next_calls`，本地／远程 MCP 投影工具名及发布引用。
- [x] 直接执行返回调用，验证范围、版本、证据边界与输入体积。
- [x] 更新方案、代码链路和 checkpoint。

## 范围

沿用 BookToolContractRegistry、Concept Candidate Set、本轮定位来源账本与本轮证据账本。搜索命中附单 LID 或闭区间读取调用，概念候选逐 occurrence 提供选择，Context 项提供该位置的读取调用。候选内 `next_calls` 表示可选正文读取；远程顶层 `next_calls` 保持分页语义。

## 验证计划

固定夹具覆盖单点、跨叶命中、多个概念、Context 关系及空搜索。将结果参数直接交给原读取分派；若范围或表面工具名不合法，修结果／MCP 映射。通过实际交付结果检查账本；若未读候选成为正文证据，修原证据接纳路径。远程通过真实 HTTP 工具调用验证发布引用、来源和计次。比较改前／改后 JSON 和实际预算投影，记录新增参数的成本。

## 实现

- `read-tools::BookTextCall` 的参数复用 `book-tool-contracts::TextInput`，工具名取共同契约的 Resident alias。`TextOccurrence.next_calls` 读取命中所覆盖的叶节点：单 LID 省略 `end_lid`，跨叶命中保留闭区间。`ConceptCandidate.next_calls` 与该候选的离散 occurrences 一一对应；`ContextItem.next_calls` 读取该项 LID。概念和搜索结果保留既有反序列化的缺省数组，使历史结果仍可由原账本解析。
- `mcp::project_candidate_read_calls` 在本地／远程表面将调用投影为 `name: book_text`。远程在原集合分页之前补 `published_book_ref`，因此嵌套续页也保留同一发布身份；顶层分页完成状态、来源与计费仍由原 `McpRead` 执行。
- `tool_result::project_structured` 在概念预算裁剪后同步删除已省略位置的读取提示。300 处位置用例先复现 `1.190` 调用没有定位来源，再通过修复；见 [失败证据](budget-red.log) 和 [修复后样本](budget-observation.json)。定位／正文账本的接纳规则没有扩大。
- `BookTextCall` 和三个候选类型的前端声明由 ts-rs 生成。没有新增工具、持久模型或运行组件。

## 验证结果

| 入口 | 结果 | 证据 |
|---|---|---|
| Runtime：`to8 tool_result_projection locator_ledger source_presentation` | 42 通过；含 4 项 TO8 测试，固定结果、真实分派、定位与正文证据、脚本模型下一采样和 Native/ReAct 投影 | [runtime-server.log](runtime-server.log) 的 Runtime 段 |
| Server：`mcp_read mcp_contract source_presentation` | 15 通过，2 项原有快照入口默认忽略；本地 MCP、远程 HTTP、来源版本、授权／额度和集合分页 | [server-final.log](server-final.log) |
| read-tools：搜索、概念、Context 及相关类型导出 | 35 通过；搜索顺序／范围、概念排序／计数、Context 关系和生成声明 | [read-tools.log](read-tools.log) |
| `python docs/performance/tool-usability/to8/compare.py` | 本地／Resident／远程各 8 个返回调用通过实际发布 schema；五种固定结果移除新增字段后与基线逐值相同，全部完整交付 | [comparison.log](comparison.log)、[sizes-and-results.json](sizes-and-results.json) |
| `pnpm --filter @understand-book/web typecheck` | 通过，生成类型与前端消费方一致 | [web-typecheck.log](web-typecheck.log) |

脚本模型从实际 Provider 请求中的候选 `next_calls` 复制参数，随后走原 Resident 循环；概念场景选择第二个候选。读取前候选没有正文证据，搜索只保留字面出现事实；读取后规范正文及实际范围进入原证据账本。模型边界以脚本控制，Book、结果预算、调用授权、存储与分派均为真实实现。

远程 HTTP 的五次定位加八次读取共计 **13 次**内容调用；调用参数、发布引用、来源 URL 和 UTF-16 范围均已核对。另用 60 处概念出现位置验证嵌套 `next_calls` 分页重建并读取末项。旧集合重建测试的预期已按远程别名和发布引用更新，仍逐值比较完整结果。

## 输入体积

| 固定结果 | 改前字节 | 改后字节 | Runtime 估算 token（前→后） |
|---|---:|---:|---:|
| 三个单点字面命中 | 1,207 | 1,393 | 302→349 |
| 一个跨叶命中 | 602 | 680 | 151→170 |
| 空搜索 | 205 | 205 | 52→52 |
| 两个概念候选 | 685 | 809 | 172→203 |
| 两项 Context | 162 | 284 | 41→71 |

上述为规范结果 JSON，输入 schema 本片未修改。远程包裹结果中新增调用带来 0–513 字节；该比较从同一实测远程结果去掉候选调用字段，未将其称为改前服务实测。300 处概念样本原结果为 17,266 字节；16 KiB 预算下同步删除省略位置的调用后交付 10,939 字节、189 处位置和 189 个合法调用。估算 token 不是 Provider 计费数据。

## 复现

沿用 Rust `+1.95.0`，`CARGO_TARGET_DIR=D:/codex-build/understand-book-no-progress-195`，dev/test debug=0、incremental=0、并行度 2，TEMP/TMP=`D:/codex-build/to3-temp`。

```text
cargo +1.95.0 test -p runtime --lib -- to8 tool_result_projection locator_ledger source_presentation --test-threads=1
cargo +1.95.0 test -p server --lib -- mcp_read mcp_contract source_presentation --test-threads=1
cargo +1.95.0 test -p read-tools --lib -- search_text context concept export_bindings_booktextcall export_bindings_textoccurrence export_bindings_contextitem --test-threads=1
pnpm --filter @understand-book/web typecheck
python docs/performance/tool-usability/to8/compare.py
```

设置 `TO8_CANDIDATES`、`TO8_DELIVERED`、`TO8_REQUESTS`、`TO8_BUDGET`、`TO8_LOCAL`、`TO8_REMOTE` 为各 JSON 输出文件路径可重导出。改前候选保存在 [before.json](before.json)，改后为 [after.json](after.json)；实际交付与运行分别见 [delivered.json](delivered.json)、[requests.json](requests.json)、[local.json](local.json)、[remote.json](remote.json)。重跑应使用新路径保留基线。

## 已知限制

- 300 处概念位置样本的 envelope 正确标为 partial/truncated；候选层计数及通用 `RefineCall` 仍沿既有逻辑，没有精确的缺失 occurrence 续取参数。TO11 接续覆盖表达和语义保留，TO10 负责正文／搜索续取。本片保证实际返回的读取提示具有交付位置来源。
- 本轮没有真实模型采样、生产发布或真人学习验收；TO14 负责实际模型选用与收益比较。确定性结果证明参数、执行与证据边界。

## 参考

已阅读 REA 固定提交 `2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c` 的 R1 工具设计／设计技能及 R6 `evidence.ts`、`relationshipCoverage.ts`；链接见 [TO 方案第 6 节](../../../切片方案-工具契约与Agent可用性优化.md#6-rea-原仓库怎么找怎么读)。本项目依据为 ADR-0086、ADR-0087、ADR-0113 及原结果／读取分派。
