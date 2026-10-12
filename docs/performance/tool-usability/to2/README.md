# TO2：Book 示例与实际发布示例验证

日期：2026-10-12。代码基线：`67efb605f5d45157eeb769f0074b218eacd76dba`，含 TO1 未提交实现。

## 工作步骤

- [x] 读取 checkpoint 全部冷启动入口、CONTEXT 与 REA 固定版本 R1/R2/R3/R7；保存 TO1 after.json 为 before.json。
- [x] 在共同契约旁加入完整输入示例；Runtime 保留来源绑定示例；远程投影发布引用。
- [x] 验证实际 Provider/MCP 定义、schema、typed input、真实分派及来源；记录实际选中测试。
- [x] 比较逐工具字节量，更新方案、代码链路与 checkpoint。

## 范围

术语沿用 BookToolContractRegistry、本轮证据账本与来源绑定，属于纯技术表示扩展。验收目标是完整示例经最终定义仍合法，且夹具实际调用返回预期结果；真实模型收益由 TO14 验收。

Native 与 ReAct 的既有请求投影保留完整 schema，示例只发布在顶层 `examples`。共同输入示例位于 `book-tool-contracts::input_examples`，夹具为该 crate 的 `fixtures/example-book`。字段说明仍要求使用当前书籍返回的位置与发布引用。

## 验证目标

- canonical 示例到最终定义后缺失字段、别名或状态分支：修对应表面投影。
- 最终 schema 接受但 typed input/dispatch 拒绝：修示例或真实契约所有者。
- 候选冒充正文、来源绑定错位、远程计次变化：修本片的夹具/投影，保持既有执行语义。
- 新增示例重复注入或改变无关定义：移除重复或越界改动，并记录输入字节增量。

## 实现

- `book-tool-contracts::input_examples` 提供 8 个完整输入：字面搜索 2 个（首次 exact／范围内之前一次 normalized），Concept、Query、Text、Synthesize、Context、Structure 各 1 个。`input_schema` 只对有示例的工具添加顶层 `examples`。
- 比较查询把 alpha、beta 两端写成独立 targets，并将条件和等号边界写成两个 obligations；综合明确限定输入 LID 集合并要求说明证据缺口。测试实际返回两端引用。
- `orchestrator::declared_tool_specs` 为 `source.present` 提供夹具第一句原文。测试先证明未读原句被拒绝，再读取正文，核对来源精确范围，并经完整 Resident 回合完成带引用的交付。
- `mcp_read::tool_definitions` 仅给远程示例补 `published_book_ref`。引用来自真实 `PublishedLibrary::publish` 产生的记录；HTTP 夹具恢复同一发布记录后，从 `tools/list` 取示例原样调用，包含同一引用、来源和计次断言。未授权账号调用该示例仍被拒绝且不计次。
- `tool_registry::contract_tests` 逐项验证最终 Registry 中的示例、真实参数解析及 Native 名称映射，并比较 Native/ReAct 实际请求投影；未来状态专属示例可以沿用同样入口。MCP 最终 schema 由 `compare.py` 使用 JSON Schema Draft 7 验证。
- 新增反例覆盖发布投影丢 `end_lid`、远程漏发布引用、schema 通过但 typed validator 拒绝重复 targets。错误样例只在测试内。

既有三表面共同字段一致性测试抓住了新投影给无示例工具插入 `examples: null` 的问题；已改为仅访问实际存在的数组，失败证据保存在 [server-projection-red.log](server-projection-red.log)。

## 运行环境与复现

Rust `+1.95.0`；`CARGO_TARGET_DIR=D:/codex-build/understand-book-no-progress-195`；dev/test debug=0、incremental=0、构建并行度 2；TEMP/TMP=`D:/codex-build/to2-temp`。比较脚本使用 Python `jsonschema` 的 Draft 7 validator。

改前定义直接取 TO1 的 [after.json](../to1/after.json)，保存为本片 [before.json](before.json)。最终定义沿原显式入口导出：设置 `TOOL_CONTRACT_SNAPSHOT` 为本片 `after.json` 绝对路径，再运行 `cargo +1.95.0 test -p server --lib mcp_contract_export_tool_definitions -- --ignored`。

发布夹具记录的再生成方式见 [fixture README](../../../../crates/book-tool-contracts/fixtures/example-book/README.md)。首次捕获沿真实发布流程完成，见 [fixture-capture.log](fixture-capture.log)。

REA 阅读范围：固定提交 `2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c` 的 R1 工具设计，R2 契约类型/目录，R3 示例定义、`examplesFor` 和实际发布示例测试，R7 MCP 发布契约与边界测试；出处见[方案第 6 节](../../../切片方案-工具契约与Agent可用性优化.md#6-rea-原仓库怎么找怎么读)。采用实际发布输入验证方法，继续使用现有 Registry 与 Provider 投影。

## 验证结果

下表 Cargo 命令均使用 `+1.95.0`。

| 入口 | 结果 | 证据 |
|---|---|---|
| `cargo test -p book-tool-contracts --lib` | 12 通过，含共享示例 typed input 校验 | [contracts.log](contracts.log) |
| `cargo test -p runtime --lib tool_registry::` | 20 通过，含 13 项 contract_tests；Native/ReAct 保留示例及别名，错误投影与重复 targets 被检出 | [runtime.log](runtime.log) |
| `cargo test -p runtime --lib book_examples::` | 1 通过；8 个 Book 示例真实分派、原搜索续页、完整来源绑定回合 | [runtime-examples.log](runtime-examples.log) |
| `cargo test -p server --lib mcp_contract` | 8 通过；2 个显式捕获入口默认忽略；包含 8 个本地示例、6 个远程 HTTP 示例原样执行及来源/计次 | [server.log](server.log) |
| `cargo test -p server --lib mcp_tool_characterization` | 1 通过，既有本地清单/别名约束保持 | [local-characterization.log](local-characterization.log) |
| `cargo test -p server --lib mcp_contract_export_tool_definitions -- --ignored` | 1 通过，导出 61 个实际工具定义 | [after-capture.log](after-capture.log)、[after.json](after.json) |
| `cargo test -p server --lib mcp_contract_capture_example_publication -- --ignored` | 1 通过，最终再捕获入口写入临时输出目录，原演示引用保持 | [fixture-recapture.log](fixture-recapture.log) |
| `python docs/performance/tool-usability/to2/compare.py` | 61 个最终 schema 及 23 个发布示例通过；参数约束、description、工具顺序保持；重复示例、丢字段、漏发布引用反例生效 | [comparison.log](comparison.log)、[sizes.json](sizes.json) |

正确性测试共 42 项通过；显式夹具/快照捕获单独记录，不把默认忽略计为通过。运行期间的 `ts-rs` serde 属性警告为既有警告。此次只修改输入示例及其表面投影，TO1 的读取行为测试沿用其已有结果。

## 实际输入成本

按紧凑 JSON 的 UTF-8 字节计量，覆盖整个表面；每个工具的增量见 [sizes.json](sizes.json)。

| 表面 | 工具数 | 发布示例数 | schema 改前 → 改后 | 增量 | description 总字节 |
|---|---:|---:|---:|---:|---:|
| Resident | 34 | 9 | 43,235 → 44,279 | +1,044 | 10,552（不变） |
| 本地 MCP | 16 | 8 | 11,182 → 12,131 | +949 | 3,970（不变） |
| 远程 MCP | 11 | 6 | 14,358 → 15,453 | +1,095 | 14,913（不变） |

Resident 的 9 个示例是共同 8 个加 `source.present`；远程按既有纯读取工具集只发布 6 个。示例均只出现于顶层 schema，不复制到 description。输入成本增加已经记录，收益由 TO14 的固定任务对照判断。

## 已知限制

- 本片验证确定性输入契约与执行边界，没有进行真实模型采样或生产部署。Query/Synthesize 的外部模型回复由共享测试替身提供，真实选工具能力、回答质量和成本收益仍属 TO14。
- 示例使用固定演示书籍。对另一份材料调用时，位置、原句及发布引用分别取自该材料的定位结果、已读正文及 `book_list`；示例不授予访问权限。
- 快照覆盖基础工具集及 Native/ReAct 的请求组装，不代表真实 Provider 已接受某次在线采样请求。Tutor 等随状态变化的示例留给 TO3–TO5。
- TO1 记录的本地/Resident 逆序正文范围 panic 继续保留为独立读取修复，见 [TO1 已知限制](../to1/README.md#已知限制)。
