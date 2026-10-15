# TO6：Executor 生命周期示例

日期：2026-10-12。基线：`93b476d` 加主工作区 TO3–TO5、TO7–TO12。状态：已完成，未提交、未部署。

## 实施步骤

- [x] 阅读 checkpoint 冷启动读序、R1–R3/R7 固定源码及现有恢复合同。
- [x] 保存改前实际工具定义、固定合成输入与正常／恢复生命周期请求。
- [x] 在原 TypeScript 契约旁补四工具示例、引用来源及结果处置说明。
- [x] 验证最终 schema、真实 MCP/session、候选接纳与原相关回归。
- [x] 更新方案、代码链路和 checkpoint。

## 范围与判据

沿用 Executor open、opaque handoff、恢复代际、提交修订和候选传输合同；属于纯技术扩展，无新领域术语。示例由真实测试 session 签发引用，发布在原 input schema，版本取各工具现有常量。正常调用、重放及租约恢复沿现有代码执行，Reader Book/Memory 工具所有权保持。

确定性测试检测错误请求版本、输入未确认即生成、恢复后继续提交旧 sink、发布示例与真实调用不一致。失败先定位契约／示例／测试绑定，真实协议问题按最小原因修复。类型检查检测 Executor 契约与原消费者不匹配。真实模型收益由 TO14 验收。

## 实现

`build-executor-tool-contract.ts:BUILD_EXECUTOR_TOOL_CONTRACTS_V3` 在四个原 input schema 中发布五个完整输入；`closedSchema` 从各工具现有版本常量补入 version，适配器和 MCP 原样发布。`input.next` 请求仍为 v4，其余三个请求为 v3；handoff 记录和响应信封的版本分别按原合同解释。

| 工具 | 示例 | 引用来源与结果含义 |
|---|---|---|
| executor.open | 打开分派的 handoff；恢复时重新使用当前分派 | DELIVER_INPUT 带 next_request；WAIT 带等待时间；DONE 为代码判定终态 |
| executor.input.next | 首批读取、确认非最终批次后续读 | delivery session 与 input ref 成对使用；ack 取已消费非最终 batch.last_ordinal |
| executor.generation.start | 确认最后一批后取得 GENERATE | confirmed_through_ordinal 取最终 batch.last_ordinal；生成 session 可以不同于交付 session |
| executor.submit_candidate | 向当前 GENERATE 的 sink 提交空图合成候选 | 使用生成 session／sink 成对引用及 output_contract；接纳以代码回执为准 |

示例引用来自 [改前正常链路](before/normal.json) 与 [改前 DSH 链路](before/dsh.json) 的成功请求，独立验证逐值核对其来源。它们是合成夹具的示例引用；实际调用以当次 handoff／工具返回值替换 ref 和 ordinal。候选示例是该 Pass1 夹具允许的结构，其他任务依据实际语义输入和 output_contract 生成。测试消费最终发布示例，仅重绑引用和序号，原候选不替换。

`input.next` 的既有 MCP 注解错误标为只读，但 `inputBatchResponse` 持久化 offer，`confirmDeliveryBatch` 持久化确认。`build-executor-mcp.ts:createBuildExecutorMcpSession` 将该注解更正为 `readOnlyHint=false`，并在描述中说明 session 写入；其他注解、输入约束和执行行为保持。改前失败见 [annotation-red.log](annotation-red.log)。

## 验证结果

相关六个文件累计 **90 个不同测试通过**，包括本片新增 6 个。完整回归 89 项见 [regression-tests.log](regression-tests.log)；最终元数据修正后重跑全部 MCP 测试与 TO6 路径，共 19 项见 [final-tests.log](final-tests.log)。Core [最终类型检查](typecheck-final.log) 通过。

- [正常流程](after/normal.json)：提前生成被拒绝且 attempt=0；输入请求可重放；新连接从首个未确认输入恢复；已启动后新连接重读输入，再次 start 返回相同 generation。真实 mailbox 与提交值相同，最终 `DONE committed`，semantic_attempt=1、lease_epoch=1、failures=0。
- [恢复流程](after/recovery.json)：沿既有 Driver 发行当前恢复 handoff，lease_epoch=2、semantic_attempt=1；旧 start 返回 `stale_generation_session`，旧 sink 返回 `lease_expired`；新连接取得新的生成 session／sink，候选成功接纳，failures=0。
- [DSH 续读](after/dsh.json)：真实 V4 session 按现有单 chunk 批次交付，第一批后带 ack 请求下一批，最终序号只交给 generation.start；共同请求版本与 Codex 工具定义一致，最终提交成功。
- [真实 stdio](after/stdio.json)：独立 Node 进程运行原 MCP 入口，经过 stdin/stdout 的 initialize、tools/list、tools/call；错误 open 版本被拒绝，五个发布示例保持，正常读取／生成／提交得到 `DONE committed`。
- [独立 Draft 7](validation.json)：5 个发布示例、31 次成功调用通过；39 个必填／封闭字段／版本／引用／序号反例被拒绝；4 次 schema 合法但状态不允许的调用经真实 session 拒绝，1 次 stdio 错误版本拒绝。去掉说明和 examples 后，原输入约束逐值保持。

四份实际 MCP 定义合计 **2,930 → 7,570 字节（+4,640）**，包含工具描述、schema 和注解。该数字是序列化定义大小，不是 billed token 或模型效果。

基线请求已保存于 `before/`；缺少示例的红测见 [red.log](red.log)。最初的多批次夹具断言失败见 `baseline-tests.log`、`baseline-final-tests.log`、`baseline-multiparagraph-tests.log`：现有 Codex 路由使首个工作单元在一批内完成，后续改用受支持的 DSH 单 chunk 批次验证 ack。冻结后基线三项通过见 [baseline-accepted-tests.log](baseline-accepted-tests.log)。初次类型检查中测试接口缺少索引签名，改为对象展开后通过；没有调整生产传输上限。

## 复现

```powershell
$env:TO6_CAPTURE_DIR=Join-Path (Get-Location) 'docs/performance/tool-usability/to6/after'
pnpm --filter @understand-book/core exec vitest run test/build-executor-tool-adapter.test.ts test/build-executor-mcp.test.ts test/automatic-build-executor-session.test.ts test/executor-transport.test.ts test/build-executor-version-boundary.test.ts test/dsh-build-session.test.ts
pnpm --filter @understand-book/core typecheck
python docs/performance/tool-usability/to6/validate.py
```

仅重跑示例及 MCP 投影时，在四个对应文件后使用 `-t 'TO6|Build Executor root-shared MCP boundary'`。`before/` 是冻结证据，重跑写入 `after/`。

## 已知限制

- 本片验证代码契约、真实 session、MCP 与候选接纳；没有调用真实模型，工具选用和完成率收益由 TO14 验收。
- 本轮没有生产发布或更新安装态插件。

## 参考

已按固定提交 `2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c` 阅读 R1 tool-design／rea-tool-design、R2 toolContractTypes／toolContracts、R3 toolContractHelpers／toolContractExamples／advertisedInputExamples.test、R7 mcp-contracts／contractPresentation.test；后两组通过既有固定版本源码缓存读取。借鉴同一所有者维护示例、验证实际发布 schema 与实际调用；入口见 [TO 方案第 6 节](../../../切片方案-工具契约与Agent可用性优化.md#6-rea-原仓库怎么找怎么读)。

