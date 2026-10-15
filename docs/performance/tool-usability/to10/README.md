# TO10：正文与搜索结果的可执行续取

日期：2026-10-12。基线：`93b476d` 加工作区已完成的 TO3／TO7／TO8／TO9。状态：TO10 已实现并验证，未提交、未部署。

## 实施步骤

- [x] 阅读 checkpoint 冷启动入口及固定版本 R5／R9。
- [x] 保存大单叶、跨叶、原生搜索多页及预算裁剪的改前结果。
- [x] 补共同正文 offset、实际交付范围和搜索原请求游标。
- [x] 验证逐页重建、证据、实际发布参数、远程来源与续页计费。
- [x] 更新方案、代码链路与 checkpoint。

## 范围与验证

沿现有 BookToolContractRegistry、规范正文、搜索 cursor、定位与正文证据账本实施，属于纯技术修正。输入输出术语均沿 CONTEXT 与 TO10 既有定义。正文使用绝对 UTF-16 位置，保留原 LID 范围；搜索使用既有游标协议，保留 query／scope／order／page_size。预算交付与原生分页分别表示。

逐页拼接测试检测遗漏、重复与不可执行参数，失败修结果切点或游标。实际证据账本检测把未交付文本接纳为证据或页内重复文本定位错误，失败修正文观察。服务端测试检测共同输入未被本地／REST／远程消费、发布版本与来源范围变化、成功续页计费变化，失败修具体表面。生成类型与 Web 类型检查检测新增共同参数没有同步消费方。

## 实验模型约定

**决策**：DeepSeek 实验使用 6.1-sol 子代理。

**否决**：
- 继续调用 DeepSeek：用户于 2026-10-12 明确指定替代执行方式。

**命门**：子代理模型为 `gpt-6.1-sol`；固定输入、次数及停止条件，记录实际模型。确定性测试继续由测试运行器判定。

## 改前观察与实现

- 改前实际结果见 [before.json](before.json)，复现见 [red.log](red.log)。大单叶只给 RefineCall 并在正文插入截断标记；搜索预算续取会改 scope／page_size 并移除原 cursor。
- `TextInput.start_utf16` 提升为共同参数，本地 MCP、REST、Resident 使用 `Book::read_text`，远程仍使用同一个 `text_page`。初次完整小结果保持 `{lid,text}`，显式续页读取至多 8,000 Unicode 字符，返回绝对 UTF-16 范围。
- 正文预算投影按 JSON 编码后字节选择连续前缀，能容纳完整叶时优先在叶末切分，大单叶按字符边界切分。保留原 LID 范围并返回具体 offset，不丢章节标题和叶间空隙。
- 搜索预算投影只缩减完整 occurrence 前缀；原查询扫描的 exhaustive 保持原义，envelope 的 partial／truncated 表示模型交付裁剪。聚合 section_counts 被省略时在 delivery 中明确说明。新 cursor 从原结果 ordinal 推得，使用原有游标协议，原 query／scope／order／page_size 不变，不再重跑语义调用。
- 范围校验复用规范正文。新增 offset 路径之外，`Book::text` 的逆序越界切片也改为原范围错误返回，避免直接 panic。

## 回归断言调整

正文前缀现在是可验证证据，旧测试将已读的 X 字符当成未观察并要求整个来源工具不可用，已改为请求真正未交付的 TAIL_SECRET 并断言 SOURCE_NOT_OBSERVED。整叶预算用例从 700 调为 850 字节，覆盖新增坐标元数据且仍只能容纳一叶。TO1 的旧表面测试不再将 start_utf16 视作远程专属字段，其余发布身份／额度字段继续保持表面边界。

正文页按绝对位置与规范正文逐值核对，再接纳到原证据账本。账本条目保存该页精确区间，使跨叶间隙仍属于已观察的连续正文；来源绑定继续使用原 EvidenceRange。重复内容依据坐标定位，同页重读去重，未读尾部不能创建引用。跨叶引用的失败和修复用例见 [gap-red.log](gap-red.log) 与 Runtime 回归。

## 验证结果

| 入口 | 结果 | 证据 |
|---|---|---|
| read-tools：正文、搜索、来源及类型导出 | 35 通过 | [read-tools.log](read-tools.log) |
| book-tool-contracts：共同 schema／解析 | 12 通过 | [contracts.log](contracts.log) |
| Runtime：TO10、投影、TO8／TO9、契约和来源 | 63 通过 | [runtime.log](runtime.log)、[resident.json](resident.json) |
| Server：本地／REST／远程／HTTP | 20 通过，2 个原快照导出入口默认忽略 | [server.log](server.log)、[local.json](local.json)、[http.json](http.json)、[remote-search.json](remote-search.json) |
| Web：生成类型消费 | 类型检查通过 | [web-typecheck.log](web-typecheck.log) |
| 最终 advertised schema 与输入比较 | 18 个实测输入通过 Draft 7，原生搜索样本逐值不变 | [comparison.log](comparison.log)、[sizes-and-results.json](sizes-and-results.json) |

实际 Resident 循环通过脚本模型从下一次 Provider 请求复制 continuation，完整读取 24,022 个 UTF-16 单元，共 4 页；Book、dispatch、预算、定位授权及证据账本均使用真实实现。Native／ReAct 请求均保留续取参数。每页接纳范围与实际交付相等。

HTTP 长正文 2 页逐字符重建，每页成功计 1 次，共 2 次，发布引用和来源 URL 的精确范围保持一致。倒序搜索的原生分页和集合分页共执行 8 次读取，原参数不变，60 个命中无遗漏、无重复；同一页内倒序保持。多分支集合结果按 data_path／offset 归位，不按请求完成先后追加。

| 固定输入（1,800 字节预算，原生搜索除外） | 改前正文体字节 | 改后正文体字节 | 续取 |
|---|---:|---:|---|
| 大单叶 | 994 | 1,795 | RefineCall → 精确 offset |
| 跨叶父节点第一页 | 32 | 128 | 保留父范围与精确 offset |
| 原生搜索第一页 | 1,159 | 1,159 | 原 ToolCursor 不变 |
| 搜索预算裁剪 | 1,030 | 1,785 | 恢复原查询 cursor |

体积属于实际 model_body，不含 envelope／receipt；结果更长是因为交付了更多完整正文或命中，并携带实际范围。没有据此推断模型费用或任务完成率改善。

## 复现

Rust `+1.95.0`；`CARGO_TARGET_DIR=D:/codex-build/understand-book-no-progress-195`，dev/test debug=0、incremental=0、并行度 2，TEMP／TMP=`D:/codex-build/to3-temp`。

```text
cargo +1.95.0 test -p read-tools --lib -- text export_bindings_booktextcall --test-threads=1
cargo +1.95.0 test -p book-tool-contracts --lib
cargo +1.95.0 test -p runtime --lib -- to10 tool_result_projection to8 to9 tool_registry::contract_tests locator_ledger source_presentation --test-threads=1
cargo +1.95.0 test -p server --lib -- mcp_read mcp_contract --test-threads=1
pnpm --filter @understand-book/web typecheck
python docs/performance/tool-usability/to10/compare.py
```

`TO10_SNAPSHOT / TO10_RESIDENT / TO10_LOCAL / TO10_HTTP / TO10_REMOTE_SEARCH` 可指定对应测试输出文件。重跑保留 before.json 和 red.log，另设快照路径。

## 已知限制

- 本片没有真实模型采样或生产部署。脚本模型证明实际调用／交付链路，真实选用收益仍由 TO14 验收。需要模型实验时遵循本页实验模型约定。
- 预算耗尽或不足以容纳一个完整命中时，返回 partial，不接纳省略内容，也不前移位置；等待有足够预算的调用。TO11 接续结构化结果的预算语义和恢复边界。
- 原有结构化候选预算、教学内容数学问题及独立首页改动继续按原分工维护。

## 参考

已阅读 REA 固定提交 `2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c` 的 R5 契约执行／结果交付／MCP Server，以及 R9 测试设计／largeToolResult／工具使用技能，链接见 [TO 方案第 6 节](../../../切片方案-工具契约与Agent可用性优化.md#6-rea-原仓库怎么找怎么读)。本片沿用本项目 ADR-0086／0087／0113 的来源与证据所有权。
