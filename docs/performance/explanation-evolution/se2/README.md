# SE2：正式目录和源码读取

2026-10-16。状态：完成。输入为 SE1 discrete-update@1（run-3），基于 HEAD 402d9be 的工作树。下一入口为 SE3 的 method_refs 与版本追溯。

## 实施步骤

- [x] 从清单编译目录和全部登记文件，接入演示指导与 read_method。
- [x] 验证文件/版本错误、范围完整读取及实际后续 Provider 输入；保留候选组合与正式空目录的证据。
- [x] 更新方案、架构、代码链路及两个 checkpoint，下一入口转为 SE3。

构建默认使用正式 `skills/presentation/methods/catalog.json`。隔离试验通过构建输入 `UNDERSTAND_BOOK_METHOD_CATALOG=docs/performance/explanation-evolution/se1/candidate-catalog.json` 选择同一编译路径中的候选组合；运行中不切换资产。

## 交付与读取合同

- [build.rs](../../../../crates/runtime/build.rs) 根据选定清单生成编译资产，追踪清单和全部登记文件的变化；不存在的文件使构建失败。无需另外手写一份源码文件表。
- [presentation_methods](../../../../crates/runtime/src/presentation_methods.rs) 同时提供短目录与确切版本/文件查找。[指导装配](../../../../crates/runtime/src/agent_prompt/presentation.rs) 只在 Author 暴露时加载目录；共同指导仍为 ex14.v4，目录有独立的 se2.v1.catalog 修订标识。
- [Author 合同](../../../../crates/runtime/src/presentation_author.rs) 与 [schema](../../../../crates/runtime/src/tool_schema.rs) 新增 `read_method`。[Server](../../../../crates/server/src/presentation_author.rs) 复用现有 `presentation_source::read`；[结果投影](../../../../crates/runtime/src/tool_result.rs) 复用连续源码片段与续读合同。

调用示例（仅候选组合登记了此版本）：

~~~json
{"operation":"read_method","method_id":"discrete-update","revision":1}
{"operation":"read_method","method_id":"discrete-update","revision":1,"file":"example/index.html","offset":0,"length":4000}
~~~

省略 file 读取 card.md。返回 method_id、revision、catalog_revision、file、entrypoint、完整 files 清单和 text。offset/length 使用 Unicode 字符；offset 与 chunk_characters 描述实际正文，end_offset 表示此次请求范围的终点，total_characters 是整文件字符数。预算截断时 next_offset 和 continuation 精确指向遗漏部分；请求范围读完后 next_offset=null，需要继续整文件时再从 end_offset 发起新范围。缺失方法/版本返回 PRESENTATION_METHOD_NOT_FOUND，缺失文件返回 PRESENTATION_METHOD_FILE_NOT_FOUND；零长度或越界 offset 明确失败。

## 验证结果

| 检查 | 结果 | 证据 |
|---|---|---|
| 候选编译、目录/源码一致、错误和范围读取、Provider 入口 | Runtime 4 项 + Server 2 项通过；编译清单与选择的候选清单一致 | [候选日志](candidate-tests-verified.log) |
| 后续实际请求包含所读正文 | 8 次请求中完成 6 次 read_method；card、HTML 的三个范围、运行说明、检查合同逐字对照通过 | [候选 Provider 输入](candidate/author-provider-inputs.json) |
| 按需装配 | 发现目录时不含卡片/源码正文；只读卡时不含例子或适配说明；普通回答不暴露 Author、不加载目录 | [普通回答输入](candidate/ordinary-provider-inputs.json)、上述候选输入及 Runtime 指导测试 |
| 小预算续读 | 对真实示例使用 900 字节结果上限，完整源码与限定 1200 字符范围均无遗漏/重复，保持方法身份和请求终点 | `se2_method_projection_reassembles_actual_source_with_exact_range`，候选/正式日志 |
| 默认正式组合 | Runtime 4 项 + Server 2 项通过；编译方法数为 0，发现后为正式空目录，无候选正文 | [正式日志](formal-tests.log)、[正式 Author 输入](formal/author-provider-inputs.json)、[正式普通回答](formal/ordinary-provider-inputs.json) |
| 现有演示指导/采样/投影回归 | 102 项通过 | [Runtime 回归](runtime-regression-final.log) |
| 工具合同与 Provider schema | 17 项通过，含 Native/ReAct 参数投影、全部示例解析和字段合同 | [合同回归](tool-contract-regression-final.log) |
| 原演示源码 read/search/patch | 3 项通过；1 项既有真实浏览器回放保持 ignored | [源码回归](source-read-regression.log) |

Server 验证使用真实 `prepare_agent_chat → execute_prepared → Resident → RuntimeStatePort → AuthorSession → 工具结果投影 → ModelAdapter::chat`。记录适配器只控制下一次工具调用并捕获实际请求，没有替换资产读取、Server 执行、范围切分或运行时投影。四个文件同时经过逐 333 字符的直接工具读取；Provider 流使用 4000 字符范围，最后一段覆盖 HTML 文件末尾。

## 复现

仓库根目录运行候选组合：

~~~powershell
$env:UNDERSTAND_BOOK_METHOD_CATALOG = 'docs/performance/explanation-evolution/se1/candidate-catalog.json'
$env:SE2_OUTPUT = Join-Path (Get-Location) 'tmp/se2-new-candidate'
cargo test -p runtime -p server --lib se2_ -- --nocapture
~~~

清除选择后验证正式组合；本次最后一次构建使用此组合：

~~~powershell
Remove-Item Env:UNDERSTAND_BOOK_METHOD_CATALOG -ErrorAction SilentlyContinue
$env:SE2_OUTPUT = Join-Path (Get-Location) 'tmp/se2-new-formal'
cargo test -p runtime -p server --lib se2_ -- --nocapture
~~~

相关回归可分别运行 `cargo test -p runtime --lib presentation`、`cargo test -p runtime --lib tool_registry::contract_tests`、`cargo test -p server --lib presentation_edit`。本次复用 Cargo 刚输出的 Runtime/Server 测试可执行文件运行相同过滤器，避免在单包与双包命令间重复编译不同的依赖 feature 组合。

## 已知限制与保留记录

- 输入证据证明工程读取与后续采样接通。Provider 返回受控调用，没有付费模型请求，未验证自然选卡、适配质量、跨题收益或正式采用；method_refs 与目录/读取/声明使用的持久追溯属于 SE3。
- 首轮仅有一个候选单元。候选的四个文件来自 SE1 run-3，作品内容未改；本片未重复执行 SE1 的三环境浏览器验收。正式目录仍无已采用条目，未部署或发布。
- [首次运行](candidate-tests.log) 的 Provider 测试在准备阶段因两次运行共用学习存储而失败；测试改用各自临时存储后通过。[首次演示回归](runtime-regression.log) 和 [首次工具合同回归](tool-contract-regression.log) 分别暴露旧操作集合/示例总数断言未包含新增操作；更新合同断言后通过。原日志保留；编译仍有既有 ts-rs serde 属性警告。
