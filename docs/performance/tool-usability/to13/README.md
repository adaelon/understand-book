# TO13：从实际契约生成工具参考文档

日期：2026-10-12。基线：`93b476d` 加主工作区 TO3–TO12。状态：已完成，未提交、未部署。

## 实施步骤

- [x] 阅读 checkpoint 全部冷启动入口、TO13 及 REA R8 固定源码。
- [x] 扩展现有 Rust 导出，保留完整 Resident 元数据和 Tutor 状态投影；远程目录由 HTTP 与导出共用。
- [x] 增加显式生成入口，直接消费 Rust 导出和 TypeScript Executor 实际 tools/list。
- [x] 生成参考文档，验证公开集合、示例、字段变化传播与文件漂移。
- [x] 更新方案、代码链路和 checkpoint。

## 范围与判据

沿用现有工具契约、工具暴露计划、结果交付与 Executor 定义，属于纯技术变更，无新领域术语。目录按 Resident、本地 MCP、远程 MCP、专用 Executor 分开，Tutor 使用既有 TO3 状态夹具。输入、效果、输出 schema 的所有者保持；生成产物由命令重建。

生成与验证检测漏工具、HTTP 发布集合和导出不一致、状态示例失真、共同字段或示例未进入产物，以及生成文件陈旧。失败修导出或模板。生成仅读取契约元数据，不调用模型、不打开材料或生产账号；HTTP 集合测试使用现有合成夹具。

## 实现

入口为 `pnpm docs:tools`，产出 [工具契约参考](../../../工具契约参考.md) 和完整 [tool-contracts.json](../../../tool-contracts.json)。`pnpm docs:tools:check` 从当前源码重新导出并直接比较两份文件，陈旧或缺失时失败且不改写文件。生成顺序固定，文件不带生成时间；Windows CRLF 与 LF 等价。

`scripts/generate-tool-reference.mjs` 扩展根 package scripts，按工具表面渲染用途、输入字段／分支、原始示例、结果覆盖、effects／annotations、预算策略和来源符号。完整 schema 保存在 JSON，由文档中的 JSON Pointer 定位；任务说明链接到生成示例。工具目录由实际定义枚举，模板不维护另一份工具名单。

| 来源 | 导出方式 | 所有权 |
|---|---|---|
| Resident | 扩展原 `mcp_contract_export_tool_definitions`，使用 `ToolRegistry::try_new(tool_specs())` 的全部 registration | 原输入、routing_card、output_schema、annotations、output_policy |
| 本地 MCP | 原 `mcp::tools_list_result` | 原发布定义，含 13 个 Book 与 3 个 Artifact |
| 远程 MCP | `remote_mcp::tools_list_result` 供 HTTP dispatch 与原导出测试共用 | 原 mcp_read 定义加 mcp_usage；不创建用户或出版物 |
| Executor | 从原 `createBuildExecutorMcpSession` 调用 tools/list，逐值核对 `BUILD_EXECUTOR_TOOL_CONTRACTS_V3` | TypeScript 仍为唯一输入／示例来源，注解取实际 MCP |
| Tutor | 原状态契约测试新增 `TOOL_TUTOR_SNAPSHOT` 输出 | 复用 TO3 的 14 个可用场景及 3 个不可用状态；context／最终 schema／示例成对导出 |

生成器只建立临时导出目录；导出结束后移除本次临时文件。生产工具输入、输出与权限语义保持。原 TO3 快照格式继续保留，文档导出省去重复的 Native／ReAct 请求。

## 验证结果

| 表面 | 工具数 | 发布输入示例数 | 已发布输出 schema 数 |
|---|---:|---:|---:|
| Resident | 34 | 47 | 11 |
| 本地 MCP | 16 | 8 | 13 |
| 远程 MCP | 11 | 6 | 11 |
| Executor | 4 | 5 | 0 |
| Tutor 状态投影 | 14 个可用场景、3 个不可用状态 | 86 | 沿原定义 |

- [生成记录](generate-final.log)：实际 Rust 导出 1 项、Tutor 状态契约测试 1 项通过；生成 **65 个表面工具条目**，17 个 Tutor 场景。
- [Server 回归](server-tests.log)：2 项通过；HTTP tools/list 与导出目录逐值一致，Resident／本地／远程的共同输入字段保持原契约、传输字段留在所属表面。
- [生成器回归](generator-tests.log)：7 项通过；实际公开工具集合与 Markdown 一一对应、发布示例保持、状态与示例配对；变更导出中的共同字段和示例可反映在三个 Book 表面；新注册条目无需修改模板名单；Executor MCP 与 TypeScript 定义一致；链接、JSON 示例与只读漂移检查有效。
- [Draft 7](validation.json)：**152 个发布输入示例**通过对应最终 schema，**35 份输出 schema**结构合法。原 TO12 的实际结果执行验证沿用原记录，本片没有重复声称运行那些样本。
- [完整漂移检查](check.log)：重新执行源码导出后，两份生成文件逐文本一致。

首次生成在实际输出契约的布尔 schema `true` 处失败，见 [generate.log](generate.log)。摘要函数补齐布尔 schema 后通过；生成器测试直接消费完整实际契约，持续覆盖该情况。检查过程中未修改原业务 schema 来迎合模板。

## 复现

```powershell
$env:RUSTUP_TOOLCHAIN='1.95.0'
$env:CARGO_TARGET_DIR='D:/codex-build/understand-book-no-progress-195'
$env:CARGO_PROFILE_DEV_DEBUG='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
$env:CARGO_INCREMENTAL='0'
$env:CARGO_BUILD_JOBS='1'
$env:TEMP='D:/codex-build/to3-temp'
$env:TMP=$env:TEMP
pnpm docs:tools
pnpm docs:tools:check
pnpm docs:tools:test
python docs/performance/tool-usability/to13/validate.py
cargo --config 'profile.test.package.runtime.codegen-units=16' test -p server --lib -- mcp_contract_initialize_and_all_tool_schemas_explain_the_reading_path mcp_contract_local_and_resident_keep_common_semantics_without_remote_fields --test-threads=1
```

生成命令内部保留既有 Runtime `codegen-units=16` 设置；目标目录、工具链、临时目录及构建并行度由环境提供。普通测试不会无条件写入文档；仅显式命令生成产物。`validate.py` 需要现有 Python `jsonschema`。

## 已知限制

- 目录代表当前源码，实际可用工具仍受每轮工具暴露计划、权限、材料与状态影响；新增 Tutor 状态分支应扩展原状态契约场景。
- 没有发布示例、annotations 或动态 outputSchema 的条目如实标出；未额外发明结果合同。
- 没有真实模型采样、模型收益结论、生产部署或安装态插件更新；TO14 负责真实 Agent 对照。
- TO5 已记录的 EXISTING 笔记／Goal 完成判断问题保持独立，见 [原记录](../to5/README.md#已知限制)。

## 参考

已阅读 REA `2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c` 的 [generate-product-catalog.mjs](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/scripts/generate-product-catalog.mjs) 与 [product-catalog.mjs](https://github.com/morluto/rea/blob/2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c/scripts/lib/product-catalog.mjs)：消费当前运行时定义、显式生成和检查产物。采用结构／文本直接比较。
