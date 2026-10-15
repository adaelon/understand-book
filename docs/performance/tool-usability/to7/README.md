# TO7：发现结果的用途和能力恢复提示

日期：2026-10-12。代码基线：`93b476d` 加工作区已完成的 TO3。状态：TO7 已实现、已验证，改动未提交、未部署。

## 实施步骤

- [x] 执行 checkpoint 全部冷启动读序，核对 TO/TB 所有权和 ADR-0113。
- [x] 保存当前发现定义和固定场景的公开 JSON。
- [x] 在 Routing Card 与原能力解析结果上补用途及能力级恢复投影，添加完整输入示例。
- [x] 验证公开 JSON、合法恢复路径、原权限与无副作用行为，记录输入体积。
- [x] 更新方案、代码链路及 checkpoint。

## 范围与验收

本片沿用工具暴露计划、Routing Card、TaskNeed、CapabilityPlan 和本轮定位／证据账本，属于纯技术投影补齐。恢复提示只消费当前权限、profile、证据与暴露状态，发现不执行 Reader/Memory 操作。原内部计划及审计保持内部所有权。

固定场景覆盖已有匹配、缺正文证据、缺位置、只读请求写入、缺用户写入意图、profile 不适用及无能力结果。验证失败时修原公开投影或恢复来源；不通过放宽授权来使恢复请求成立。

## 实现

- `ToolSearchMatchV2` 从原 Routing Card 投影 `use_when/avoid_when`；已有 `effect_mode/preconditions` 保留。
- `ToolSearchOutcomeV2` 增加 `unmet_capabilities` 与能力级 `blocked`。每项包含 capability、公开 reason、message 及可选 `next_searches`，没有被阻塞工具的名称或内部前提细节。`task_need/capability_plan/request_audit` 继续跳过公开序列化。
- `public_capability_recovery` 按未满足能力解释阻塞；当前状态或内部前提不可用统一公开为 `capability_unavailable`。只被 `max_results` 截掉的能力返回 `result_limit`，可单独发现。
- `recovery_search` 用同一权限、内容 profile、证据、采样暴露计划与原能力解析器验证只读后续发现请求。缺正文时优先读取已知位置；无位置时提供结构与字面定位选择；缺 Reader 写入意图时解释用户请求要求，并在允许时提供 Reader 只读检查。建议生成不改变激活集。
- `next_searches` 是完整 `tool.search` 参数，供调用者选择；定位结果中的具体正文读取参数由 TO8 接续。输入 schema 补充结果读法与四个完整示例：全书主线、首次字面出现、读取已选候选、保存用户要求的笔记。

## 验证结果

| 入口 | 结果 | 证据 |
|---|---|---|
| `cargo +1.95.0 test -p runtime --lib -- tool_exposure tool_registry tool_search_to7 sample_request_exclusions_hide_tools_from_discovery_and_provider request_audit_capability_resolution --test-threads=1` | 59 通过，0 失败、0 忽略；包含 7 项 TO7 新测试 | [runtime-final.log](runtime-final.log) |
| `tool_search_to7_public_recovery_is_capability_scoped_and_executable` 与状态测试 | 七类公开 JSON、Hidden 名称隔离、权限拒绝、无恢复入口、已知位置／正文状态、采样排除与 schema 预算；每个建议经真实解析及能力发现 | [after.json](after.json)、`tool_discovery_tests.rs` |
| `tool_search_to7_delivers_recovery_and_never_executes_reader_or_memory_writes` | 普通请求与显式笔记请求走真实 `run`、存储、Reader 和下一采样；Reader 状态、记忆记录及 effects 不变，Native/ReAct 均保留公开字段 | [read-only.json](read-only.json)、[note-request.json](note-request.json) |
| `python docs/performance/tool-usability/to7/compare.py` | 最终 schema、4 个示例、七个输入及恢复请求经 Draft 7；原输入约束、匹配、排序和激活结果保持；七类结果完整经过 4 KiB 交付 | [comparison.log](comparison.log)、[sizes-and-results.json](sizes-and-results.json) |

模型响应采用脚本适配器，工具解析、运行分派、Reader、存储、结果包装与 Provider 请求投影是真实实现。本次没有真实模型采样。初次验证修正了 Reader 状态恢复请求的 operation（原契约为 navigate）；测试夹具改用所需的 CurrentAnchorOnly 状态、读取实际 envelope 的 model_body，并按原合同区分普通请求缺定位与显式意图补定位。业务权限未改。ts-rs 的 serde 属性警告为既有警告。

## 输入成本与复现

`tool.search` schema 从 **1,063 → 2,169 字节**，增加 1,106 字节；按 Runtime 现有字符估算为 **266 → 543 token**（+277），不是 Provider 计费数据。七类完整公开结果为 515–1,050 字节：已有匹配 +178 字节，其他场景 +249–690 字节。两份脚本运行的下一采样活动输入与完整 Native/ReAct 请求保存在上述 JSON 中。

改前实际发现定义与七个实际结果见 [before.json](before.json)，改后见 [after.json](after.json)。`compare.py` 删除新增投影字段后逐值比较原结果，确认匹配、排序、激活和原 description 未改变。schema 除新增 description/examples 外逐值相同。

环境沿用 TO3：Rust `+1.95.0`，`CARGO_TARGET_DIR=D:/codex-build/understand-book-no-progress-195`，dev/test debug=0、incremental=0、并行度 2，TEMP/TMP=`D:/codex-build/to3-temp`。设置 `TO7_SNAPSHOT` 导出固定场景 JSON，设置 `TO7_REQUESTS` 为输出目录导出两个实际请求及预算样本，然后执行上表 Runtime 命令与比较脚本。重跑使用新的输出路径，保留已记录基线。

## 已知限制

- 现有 4 KiB 通用结构化裁剪仍可能缩短发现用途文字。五项匹配样本完整结果 5,122 字节、交付 4,068 字节；移除 TO7 新字段后的等义原结果仍有 4,127 字节，原形状同样超限。当前 continuation 仅给通用缩小提示，见 [budget-observation.json](budget-observation.json)。此问题沿 TO11 修结果语义保留与具体续取；本片的七个固定场景没有裁剪。
- 确定性结果证明合同和权限行为，未证明真实模型选工具、完成率或成本改善；TO14 负责固定模型对照。没有生产发布或真人验收。
- TO3 数学条件、重复建议与 TO1 逆序正文范围问题保持各自待办。本片下一推荐切片为 TO8。

## 参考

已阅读 REA 固定提交 `2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c` 的 R1 工具设计及设计技能、R4 `binary_overview/batch_decompile`、R7 MCP 合同及真实发布测试；原链接见 [TO 方案第 6 节](../../../切片方案-工具契约与Agent可用性优化.md#6-rea-原仓库怎么找怎么读)。
