# TO9：Context 候选覆盖与结构可用性

日期：2026-10-12。基线：`93b476d` 加工作区已完成的 TO3／TO7／TO8。状态：TO9 已实现并验证，未提交、未部署。

## 实施步骤

- [x] 阅读 checkpoint 全部冷启动入口、TO9、既有可用性与交付路径。
- [x] 保存候选大于 k、等于 k、零候选及缺结构产物的实际结果。
- [x] 在 Context 裁剪前记录候选总数，同步生成类型及共同字段说明。
- [x] 验证本地／Resident／远程交付、候选读取、分页与结构可用性。
- [x] 更新方案、代码链路及 checkpoint。

## 范围与验证计划

沿现有 Context 与 Concept Candidate Set 结果语义工作。Context 的总数属于指定 anchor／granularity 下规则召回并去重的指针项集合，同一 LID 的不同关系仍可为不同候选。BookStructure 继续由原 `available/warning`、远程发布 manifest 能力清单及 `MCP_CAPABILITY_UNAVAILABLE` 表达。

候选超过 k、恰好 k、没有候选与 k=0 分别检验统计位置；失败修 Context。真实 MCP 分派与 Resident 结果投影检验新增字段是否丢失、候选调用是否仍可执行；失败修对应结果投影。远程集合续页检验 complete 与候选 truncated 是否混淆；失败修传输投影。缺失 BookStructure 的读取及发布能力清单检验空结果是否掩盖缺产物；失败修既有可用性传播。最后运行 ts-rs 导出与 Web 类型检查，发现类型不匹配时更新实际消费方。

## 实现

- `read-tools::Context` 增加 `total_candidates / returned_count / truncated`。`Book::context` 在已有规则召回、关系去重和排序完成后、裁到 k 之前取总数；没有改变原候选内容、排序与 `next_calls`。
- `returned_count` 表示规范结果按 k 选出的指针数。远程的集合分页、Resident 的模型输出预算仍在其后执行，各自用原有外层状态说明实际交付。Concept 的 `matched_count / returned_count / truncated` 行为保持原样，字段注释说明相同边界。
- 共同 Context 描述及 k 参数说明发布计数语义，MCP 描述将新引用的结构工具映射到 `book_structure`。生成的 `Context.ts` 与 `ConceptCandidateSet.ts` 已同步；实际消费路径透传字段，无需增加第二份结果类型。
- BookStructure 缺失的既有路径正确：本地／Resident 返回 `available=false` 与原因；远程 `book_list.available_tools` 不宣告结构能力，调用得到 `MCP_CAPABILITY_UNAVAILABLE`。可读取的 Context 树指针仍可使用。没有新增持久状态或运行组件。

## 验证记录

先执行新增测试，保存 [改前结果](before.json) 与 [失败日志](red.log)：真实树规则召回 12 个子节点，旧结果只显示前 10 个，缺少候选覆盖字段。实现后保存 [改后结果](after.json)。

| 场景 | total_candidates | returned_count | truncated |
|---|---:|---:|---|
| 12 个候选，默认 k=10 | 12 | 10 | true |
| 12 个候选，k=12 | 12 | 12 | false |
| 12 个候选，k=20 | 12 | 12 | false |
| 12 个候选，k=0 | 12 | 0 | true |
| 零候选 | 0 | 0 | false |

| 入口 | 结果 | 证据 |
|---|---|---|
| read-tools：`context concept structure` 及类型导出 | 36 通过，含新增计数用例、near/mid/far 关系和原 Concept 计数；缺失／有效结构产物读取沿原测试验证 | [read-tools.log](read-tools.log) |
| Runtime：`to9 to8 tool_registry::contract_tests tool_result_projection` | 27 通过；规范结果和实际下一采样保留覆盖字段，候选调用进入真实读取，候选本身没有正文证据 | [runtime.log](runtime.log)、[resident.json](resident.json) |
| Server：`mcp_read mcp_contract` | 17 通过，2 个原快照导出入口默认忽略；本地分派、远程分页、缺产物能力传播、真实 HTTP 与计费 | [server.log](server.log)、[local.json](local.json)、[remote.json](remote.json)、[http.json](http.json) |
| Web：`pnpm --filter @understand-book/web typecheck` | 通过 | [web-typecheck.log](web-typecheck.log) |
| `python docs/performance/tool-usability/to9/compare.py` | 五种固定结果的原字段及结构可用性逐值不变；四组本地／Resident 覆盖字段一致，四组远程分页可重建，3 次 HTTP 调用计费正确 | [comparison.log](comparison.log)、[sizes-and-results.json](sizes-and-results.json) |

Resident 使用脚本模型从实际 Provider 请求复制候选调用；Book、工具分派、结果投影与账本使用真实实现。Native/ReAct 实际请求均包含覆盖字段，候选仍只提供定位来源。

远程 60 个已知候选分别取 k=0／10／40／60，传输每页最多 20 项：k=10 时 `complete=true` 而候选 `truncated=true`；k=60 时第一页 `complete=false` 而候选 `truncated=false`。续页重建数分别为 0／10／40／60，候选读取调用保持同一发布引用并可执行。实际 HTTP 的 k=0／1／2 三次成功调用各计一次内容调用。

## 结果体积

五种规范结果仅增加 **58–60 字节**。12 个候选取前 10 项时，从 1,296 增至 1,355 字节；两个完整 12 项结果均增加 60 字节；k=0 和零候选均从 25 增至 83 字节。输入契约增加了计数与结构可用性说明；没有测量真实模型费用或选用收益。

## 复现

沿用 Rust `+1.95.0`，`CARGO_TARGET_DIR=D:/codex-build/understand-book-no-progress-195`，dev/test debug=0、incremental=0、并行度 2，TEMP/TMP=`D:/codex-build/to3-temp`。

```text
cargo +1.95.0 test -p read-tools --lib -- context concept structure export_bindings_context export_bindings_concept --test-threads=1
cargo +1.95.0 test -p runtime --lib -- to9 to8 tool_registry::contract_tests tool_result_projection --test-threads=1
cargo +1.95.0 test -p server --lib -- mcp_read mcp_contract --test-threads=1
pnpm --filter @understand-book/web typecheck
python docs/performance/tool-usability/to9/compare.py
```

设置 `TO9_CONTEXT_SNAPSHOT / TO9_RESIDENT / TO9_LOCAL / TO9_REMOTE / TO9_HTTP` 为输出 JSON 路径可重新导出当前结果。复跑使用新路径，保留 before.json。`compare.py` 直接比较原字段及结构可用性，并统计新增字段体积。

## 已知限制

- 计数不能证明规则召回涵盖书内全部语义关系；base.json 的空图谱也不能证明图谱构建完整。
- 结构化结果的预算裁剪与恢复动作沿 TO11 接续；本片不增加模型采样或生产部署。

## 参考

已阅读 REA 固定提交 `2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c` 的 [evidence.ts](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/src/domain/evidence.ts) 和 [relationshipCoverage.ts](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/src/domain/relationshipCoverage.ts)。本项目依据为 [TO9 方案](../../../切片方案-工具契约与Agent可用性优化.md#to9context-候选覆盖与结构可用性)、ADR-0086／0087／0113 及既有结果与发布路径。
