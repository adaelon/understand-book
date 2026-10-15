# TO12：Book 结果 schema 与副作用注解

日期：2026-10-12。基线：`93b476d` 加主工作区 TO3、TO7–TO11。状态：已完成，未提交、未部署。

## 实施步骤

- [x] 阅读 checkpoint 冷启动入口、TO12 和固定版本 R2/R5/R7，核对实际结果所有者。
- [x] 从领域结果类型生成完整 schema，组合 Resident 预算交付和 MCP 表面结果。
- [x] 发布有执行依据的 annotations，保留远程每次成功调用计费语义。
- [x] 通过实际 tools/list、tools/call 与 Resident 投影验证完整、空集、续页、partial 和 error。
- [x] 更新方案、代码链路和 checkpoint。

## 范围与验证

沿现有 BookToolContractRegistry、结果类型、ToolResultEnvelope 和 MCP 交付路径补充契约。结果类型及候选计数是已有术语，本片为纯技术扩展。输出 schema 由结果所有者生成，分页、候选调用别名、sources 和 usage 由表面组合。

确定性验证将检测真实返回与最终 advertised schema 不一致、续页数组误作完整对象、预算省略误作领域结果，以及重复收费却标为幂等。失败分别修领域定义、表面组合、预算 schema 或注解来源。错误保留既有 isError 和结构化诊断协议，成功 schema 不接纳错误。

## 实现

`read-tools::output_schema` 从现有结果类型生成完整领域 schema，覆盖 manifest、正文／正文页、搜索、Context、Concept、结构／路线及论文投影；嵌套字段、枚举和类型化映射均保留。Runtime 的 QueryOutcome、SynthesizeResponse 和 BookGuideResponse 由 Runtime 自己派生。输入契约继续归 book-tool-contracts，无反向依赖。

| 表面 | 实际交付合同 |
|---|---|
| canonical | 领域结果本身。候选数量描述选择；QueryOutcome 的 partial／insufficient 描述支撑状态 |
| Resident | `ToolRegistration::output_schema()` 发布 ToolResultEnvelope，区分 ok／partial／error；预算 delivery、明确省略、正文页及 null 有独立结构 |
| 本地 MCP | 13 个 Book 定义发布 outputSchema；候选读取调用使用 name／book_text；visitor guide 组合会话开启／更新与关闭两种返回 |
| 远程 MCP | 9 个内容工具及 book_list／mcp_usage 发布 outputSchema；来源、额度和续页留在 Server 组合；完整 data 对象与 data_path 指定的集合数组分别受约束 |

远程集合 schema 沿领域 schema 枚举真实可分页数组，包括候选读取提示、论文引用和数组项内嵌数组。路径与元素类型一起约束，不能用任意对象冒充续页。跨字段数值等式（如交付数量与数组长度）继续由实际分页测试验证。

Resident 的 Provider 函数定义仍只发送输入参数。输出契约通过 Runtime registry API 提供给消费方及后续 TO13 文档生成，不向 Provider 的输入 parameters 混入输出结构。活动回合账本可在结果被模型采样后回收 model_body，保留原 status／truncated／receipt；schema 允许这个既有 null 状态，null 不提供新证据。

| 工具 | 注解依据 |
|---|---|
| 确定性本地／Resident Book 读取 | 既有 ReadOnly effect；无破坏、可重复、闭集 |
| query／synthesize | 既有 ReadOnly effect；会调用模型，保守标为非幂等、开放交互 |
| 本地 visitor guide | guide 开启／更新临时会话并调用模型，close 删除临时会话；整体非只读、非幂等，允许破坏性会话操作 |
| 远程内容读取 | 原执行层每次成功均记录消费并扣减额度，因此 readOnlyHint=false、idempotentHint=false；书籍内容不变 |
| book_list／mcp_usage | 免费只读查询，重复查询不消费内容次数 |

本片为新增发现元数据和 schema API，原 Book 输入、返回 payload、别名及版本策略保留。MCP 的错误仍返回 isError=true 和 structuredContent 诊断／相同 JSON text；它们不属于成功 outputSchema。Query 的 invalid_plan 是原有领域状态，仍按现有成功协议交付。

## 复现

```powershell
$env:CARGO_TARGET_DIR='D:/codex-build/understand-book-no-progress-195'
$env:CARGO_PROFILE_DEV_DEBUG='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
$env:CARGO_INCREMENTAL='0'
$env:CARGO_BUILD_JOBS='1'
$env:TEMP='D:/codex-build/to3-temp'
$env:TMP=$env:TEMP
$env:TO12_DIR=(Resolve-Path 'docs/performance/tool-usability/to12').Path
cargo +1.95.0 --config 'profile.test.package.runtime.codegen-units=16' test -p runtime -p server --lib -- to12 --test-threads=1
python docs/performance/tool-usability/to12/validate.py
```

## 验证结果

新增 3 项用例通过，见 [tests-fixed.log](tests-fixed.log)。实际 MCP wire 同时核对 structuredContent 与 JSON text；Resident 使用真实 Book 分派、结果投影、回执及活动回合账本，模型边界使用既有 ExampleAdapter。

| 表面 | 最终 schema 数 | 实际样本与判定 |
|---|---:|---|
| 本地 MCP | 13 | 19 个成功结果通过，2 个错误被成功 schema 拒绝；包含正文续取、空搜索、k=0、缺论文产物、invalid_plan 和 visitor close |
| HTTP MCP | 11 | 45 个成功结果通过，2 个错误被成功 schema 拒绝；含 26 个集合续页、原生搜索页、正文页、论文产物和空目录 |
| Resident | 11 | 64 个 envelope 通过：26 ok、31 partial、7 error；含预算省略、极小预算 null 和已采样正文回收 |

共 **132 个交付样本**。完整 raw 另经 canonical schema；投影／错误不能冒充 canonical，任意对象不能代替 model_body／data，错误 data_path 被拒绝。证据：[local.json](local.json)、[remote.json](remote.json)、[resident.json](resident.json)、[schema-validation.log](schema-validation.log)、[validation.json](validation.json)。远程重复正文调用成功两次，实际消费增加 2；失败不计费。

相关 Runtime 80 项加新增 1 项，共 **81 项通过**；Server 累计 **23 项通过，2 个原快照入口忽略**。Server 原表面边界断言把任何 next_calls 都视作远程字段；TO8 已在本地候选返回读取提示，TO12 的 outputSchema 现如实公开它，因此断言调整为顶层传输分页与 published_book_ref／charged_units／source_url 仍属远程。正文、来源和计费行为保持原测试验证。

回归命令：

```text
cargo +1.95.0 --config 'profile.test.package.runtime.codegen-units=16' test -p runtime -p server --lib -- tool_result_projection to11 to8 to9 to10 tool_registry::contract_tests locator_ledger source_presentation tool_search_to7 mcp_read mcp_contract --test-threads=1
cargo +1.95.0 --config 'profile.test.package.runtime.codegen-units=16' test -p server --lib -- mcp_contract_local_and_resident_keep_common_semantics_without_remote_fields --test-threads=1
```

日志：[regression.log](regression.log)、[单项修正后回归](regression-fixed.log)。编译使用项目现有 schemars 0.8，Rust `+1.95.0`；编译日志见 [compile.log](compile.log)。

## 已知限制

- 本片使用确定性模型替身验证 query／synthesis／guide 的真实分派及结果形状，没有真实模型采样、效果提升结论或生产部署。
- 非 Book 工具的动态结果类型化沿原 TO 范围另行处理；其 effect 注解只返回已有依据的值。错误附加诊断字段由实际失败路径维护，成功结果保持完整类型约束。
- outputSchema 不代替原输入验证、证据账本、权限或额度执行。

## 参考

已阅读 REA 固定提交 `2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c` 的 R2 toolContractTypes/toolContracts、R5 contractToolHandler/toolResult/EvidenceMcpServer、R7 mcp-contracts/contractPresentation.test。入口见 [TO 方案第 6 节](../../../切片方案-工具契约与Agent可用性优化.md#6-rea-原仓库怎么找怎么读)。
