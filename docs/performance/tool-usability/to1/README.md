# TO1：共同参数语义回收

日期：2026-10-12。代码基线：`67efb605f5d45157eeb769f0074b218eacd76dba`。状态：TO1 已完成，改动尚未提交。

## 范围与步骤

1. 已保存改前实际定义：[before.json](before.json)，Resident 34、本地 MCP 16、远程 MCP 11 个工具。
2. 已补共同输入语义及 MCP 投影：按工具区分 query，说明 source 范围、相对过滤、游标绑定参数、默认值和单位。
3. 已验证共同字段完整一致、远程字段隔离和已有输入合法性；改后定义见 [after.json](after.json)，逐工具体积见 [sizes.json](sizes.json)。
4. 已更新切片方案、代码链路和 checkpoint，下一片为 TO2。

## 实现依据

冷启动读序全部已读；HEAD 与 checkpoint 一致。术语和边界沿用 CONTEXT、ADR-0088 与 TO0，无领域模型变更。

REA 参考固定提交 `2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c`，实际阅读 R1 的 `docs/tool-design.md`、`.agents/skills/rea-tool-design/SKILL.md`，R2 的 `src/contracts/toolContractTypes.ts`、`src/contracts/toolContracts.ts`；链接见[方案第 6 节](../../../切片方案-工具契约与Agent可用性优化.md#6-rea-原仓库怎么找怎么读)。采用既有契约所有者和实际发布表面验证的方式。

## 实现

- `book-tool-contracts/src/lib.rs`：字段说明随 typed input 生成并内联进 schema，覆盖嵌套 scope/relative_to/obligations；Context schema 新增既有执行默认值 `granularity=near`、`k=10`，保留 nullable 和 `k=0`。`DEFAULT_CONTEXT_*` 同时供读取实现与 schema 使用。
- 同一 LID 工具按自己的用途解释参数：Text 说明 start_lid/end_lid 到包含末节点的 source 范围映射；Context 是指针集合；Concept 的 anchor 影响排序、不限制整书召回；Query 的 anchor 用于指代消歧。
- 搜索说明区分 exact/normalized，明确 4,096 Unicode scalar values、范围交集、before/after 排除参考节点、canonical source order，以及续页必须保留 page_size 在内的原查询参数。
- `BookToolContract::mcp_description` 将共同用途/避免场景投影为 MCP 描述，并将 `book.text` 转为该表面的 `book_text`；Manifest 的避免场景改为元数据不能代替正文证据。Resident 仍用短 description + 原 Routing Card 的用途字段，无额外重复注入。
- `mcp_read::tool_definitions` 删除共同字段的第二份说明。published_book_ref、start_utf16、集合 page、来源、额度仍归远程表面；快照比较确认这些字段及远程工具整体描述保持原样。

## 验证目标

- 共同契约：字段缺说明、同名 query 语义混淆、默认值/范围描述错误，失败修 typed input/schema。
- Runtime：共同 schema 或 Routing Card 用途漂移、远程传输字段混入，失败修 Resident 投影。
- MCP：本地/远程共同字段（包括嵌套 description）不同、别名错误、计次/分页说明丢失，失败修对应表面。
- 体积记录：逐工具直接比较 JSON 和 UTF-8 字节；检查非 Book 工具及原有约束是否被意外修改，发生则恢复越界改动。

## 运行环境

Rust `+1.95.0`，`CARGO_TARGET_DIR=D:/codex-build/understand-book-no-progress-195`，dev/test debug=0、incremental=0、构建并行度 2，TEMP/TMP=`D:/codex-build/to1-temp`。

`mcp_contract_export_tool_definitions` 是显式测量入口，运行时指定 `TOOL_CONTRACT_SNAPSHOT` 并加 `--ignored`。Resident 来自 `tool_specs()`；本地 MCP 来自 `tools_list_result()`；远程 MCP 来自真实 HTTP fixture 的 `tools/list`，包含 `mcp_usage`。该 fixture 不收费、不调用模型。

原方案的 `cargo test -p runtime --lib tool_contract_tests` 实际选中 0 项，因为文件被注册为 `tool_registry::contract_tests`。本片改用 `tool_registry::` 覆盖真实契约模块与相邻 Registry 测试，后续只测契约时使用 `tool_registry::contract_tests`。0 用例运行不计通过。

## 验证结果

| 命令（Cargo 均使用 `+1.95.0`） | 结果 | 证据 |
|---|---|---|
| `cargo test -p book-tool-contracts --lib` | 11 通过 | [contracts.log](contracts.log) |
| `cargo test -p runtime --lib tool_registry::` | 18 通过，含 11 项 contract_tests | [runtime.log](runtime.log) |
| `cargo test -p server --lib mcp_contract` | 6 通过；1 个显式快照导出用例默认跳过 | [server.log](server.log) |
| `cargo test -p server --lib mcp_tool_characterization` | 1 通过，验证本地工具清单、schema 与 MCP 别名 | [local-characterization.log](local-characterization.log) |
| `cargo test -p read-tools --lib tests::context_` | 5 通过，验证树邻接、near/mid/far 累积、关系投影与错误路径 | [context.log](context.log) |
| `cargo test -p server --lib mcp_contract_export_tool_definitions -- --ignored` | 改前、改后各 1 通过，输出真实定义 | [baseline.log](baseline.log)、[after-capture.log](after-capture.log) |
| `python docs/performance/tool-usability/to1/compare.py` | 工具清单/顺序、原有输入约束、远程专属字段与整体描述、无关工具均保持原样 | [comparison.log](comparison.log) |

合计 41 项正确性测试通过，2 次显式快照导出通过。新增字段说明测试先在改前代码上因 `Text.end_lid` 无说明而失败，见 [contract-red.log](contract-red.log)。新测试曾将 schema 的 `0.0` 与 JSON 整数 `0` 按表示比较；已改为按数值比较，不改变生产代码的边界。

MCP 的公共字段比较现在包括所有嵌套 description；只移除远程专属字段后，与共同 schema 完整相等。本地 MCP 与 Resident 也直接比较共同 schema，Resident 同时验证原 Routing Card 的用途/避免场景。Context 新增的 default 注解描述原行为，nullable、required、enum、范围、Query/Guide 分支等既有约束均未改变。

## 实际定义体积

schema 按紧凑 JSON 的 UTF-8 字节计量；description 是工具整体说明的 UTF-8 字节，schema 内的字段说明已计入 schema。表中覆盖整个导出工具面，逐工具数据见 [sizes.json](sizes.json)。

| 表面 | 工具数 | schema 改前 → 改后 | 整体 description 改前 → 改后 |
|---|---:|---:|---:|
| Resident | 34 | 37,960 → 43,235 | 10,552 → 10,552 |
| 本地 MCP | 16 | 5,207 → 11,182 | 2,335 → 3,970 |
| 远程 MCP | 11 | 12,260 → 14,358 | 14,913 → 14,913 |

新增说明增加了输入体积；本片只证明语义一致及行为约束保持。TO2 可直接使用本片 after.json 作为引入示例前的基线，TO14 再判断真实任务收益。

## 已知限制

本片不做真实模型收益判断或生产部署；TO2 和 TO14 分别负责可执行示例及真实 Agent 对照。

Resident 捕获的是 `tool_specs()` 基础注册表，没有声称这是某次真实模型回合的动态激活工具集。特定 Tutor/Presentation 状态与实际请求验证按后续切片执行。

阅读实现时发现一个既有范围错误：本地 MCP `route_book_text` 和 Resident 的 Text 分派直接调用 `Book::text`；当两个真实 LID 的末节点 span.end 小于首节点 span.start 时，该函数直接切片会 panic。共同输入校验只校验非空，不校验 source 顺序；远程 `text_page` 已拒绝逆序范围。此结论来自调用链与切片代码，未专门运行复现。TO1 描述明确要求 source reading order；行为修复应在读取所有者单独补错误返回及最小回归。
