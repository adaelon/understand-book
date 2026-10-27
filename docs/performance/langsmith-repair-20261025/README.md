# LangSmith 用量与生命周期修复，2026-10-25

本轮由近 7 天真实 trace 统计触发，修复新调用的供应商识别、教学用途标签、独立评测用量格式，以及终态导出和持久恢复。修复已于 2026-10-26 合并上线，见[发布记录](../combined-release-20261026/README.md)。本记录描述观测修复切片；后续缓存实现见 [Harness 记录](../harness-cache-20261025/README.md)。

## 数据依据

- 原始窗口：2026-10-18 15:48:50 至 2026-10-25 15:48:50，Asia/Hong_Kong。
- 项目：`understand-book-dev`。原统计按产品、测试和独立评测区分，见 [模型行为报告](../../../output/langsmith-behavior-20261025/report.md)。
- 产品 1,074 条模型 span 中，1,042 条具有完整供应商用量；32 条不按零费用计算。14 个 pending 模型 span 中，9 个父任务已成功、1 个父任务失败、4 个父任务 pending。
- 目标缺失调用 `01a171f7-fcc9-7ec2-b07c-f0fee92e5701` 的父任务实际累计 token 比完整模型回执之和多 191,514，后续工具和 25 个 span 均闭合，支持“结束观测丢失”。缺少当时 exporter HTTP 日志，不能把该历史个例归因于某一次网络错误。

## 代码链路与行为变化

`NativeAdapter / ReActAdapter → ServiceAdapter → LimitedAdapter → ObservedAdapter → CancellableAdapter → RunActivity → ObservationMetadata → LangSmith extra.metadata`

实际官方 `api.deepseek.com` 端点提供 `model_provider=deepseek`，所有包装层保持透传。模型名仍保留实际配置的 `deepseek-v4-flash`；不按模型名称猜测第三方托管商。开始事件发布前已填入模型与供应商，因此缺失终态时仍能识别调用身份。

LangSmith 的 `ls_provider` 与 `ls_model_name` 共同匹配目录价格。用途白名单保留实际使用的 `available_answer`、`tutor_assessment`、`tutor_coach`、`tutor_presentation_plan`、`tutor_delivery_review`、`tutor_explanation_review`，教学辅助调用可以分别统计。可选字段保持旧观测可读，TypeScript 生成契约随 Rust 导出更新。

`RunEvents → ActivityMapper → BoundedQueue → export_loop → LangSmith / Spool`

- 最多三次 HTTP 尝试；原来覆盖整个过程的 1 秒预算改为限制累计退避等待。第一次请求耗时超过 1 秒后失败，仍有重试机会，单次请求与退出总预算保持约束。
- 根结束成功不会再清掉尚未确认的子调用终态。完整 trace 保留到全部待写记录确认；恢复原始用量，不重新执行业务。
- 已确认的精确 revision 记录确认状态；恢复时只重建依赖与本地 revision，避免重发 LangSmith 已经接收的最终 PATCH。
- 重启时，对没有终态的根和子 span 标记 interrupted，时间来源为最后观测，未知用量仍未知；所有子收尾先于根收尾。
- 工具结果投影和 evidence ledger 接纳完成后，在首次工具终态中一并提交输出、证据数量与引用。删除终态后再次 PATCH 证据的路径。

`Python evaluation runner → evals/semantic/langsmith_usage.py → 首次最终 PATCH`

供应商原始 `prompt_tokens / completion_tokens` 映射到 `extra.metadata.usage_metadata`；cached 和 reasoning 分别放入输入／输出细分。思考 token 是输出子集，不另加一次。缺失用量不变成零。两个当前教学实验 runner 已接入，原始实验输入和回执保留。

## 在线用量合同验证

独立项目 `understand-book-observability-canary-20261025` 的 synthetic run `6db739c5-02f3-4eb3-b2d9-4cbc4e1f3ad9` 未调用任何模型，与产品统计分开。首次最终 PATCH 后回读结果：

| 字段 | 实际回读 |
|---|---:|
| 输入 | 1,000 token |
| 输出 | 20 token |
| 总量 | 1,020 token |
| 缓存输入 | 800 token |
| 思考输出子集 | 10 token |
| LangSmith 美元目录费用 | $0.0000444 |
| price_model_id | `d3319aa9-743c-4473-9b26-0896244d058f` |

[完整回执](../../../tmp/langsmith-repair-20261025/new-record-canary.json)。此验证证明供应商、别名及标准 usage 能被当前 LangSmith 正确识别，不代表生产服务已更新。

## 历史评测补写结果

66 条评测原先显示 0 token，但保留了供应商原始回执。规范化后的合计为：输入 4,321,945，输出 1,306,161，总量 **5,628,106**，缓存输入 2,550,272，思考输出子集 1,219,438。

仅对第一条旧 run 做过 canary：extra-only 请求返回 202，但回读未落盘；补齐原始身份与时间字段后返回 **HTTP 409：Duplicate run update requests for the same run are not supported**。直接 GET 与 query 均确认旧记录未改变，剩余 65 条没有补写。

[LangChain 官方说明](https://support.langchain.com/articles/9964614152-can-i-reopen-a-run-after-calling-patch-to-write-additional-data)规定最终 PATCH 后 run 不可再修改。历史纠正因此以 [66 条离线用量明细](../../../output/langsmith-repair-20261025/evaluation-usage-corrected.json) 为准；其目录价格估计合计 $1.057098366。按原调用时刻计算峰谷的人民币估计为原报告中的 ¥9.6224494，两者口径不同。没有修改旧时间，也没有复制 run 来改变产品汇总。

## 验证记录

共 **72 项定向测试通过**，类型检查通过。服务端另有 1 项原有手工在线检查保持 ignored，本轮使用上面的独立在线合同验证。

| 检查 | 结果 | 具体覆盖 |
|---|---:|---|
| `cargo test -p server --lib observability::` | 38 通过，1 ignored | 供应商包装链、用途、首次终态完整内容、慢请求后重试、不可重复 PATCH 下的缺失子记录恢复、精确 revision 确认、恢复顺序 |
| `cargo test -p runtime --lib run_events::tests::` | 7 通过 | 开始事件身份、唯一终态、证据和本地活动一致、失败后保留已观察 usage |
| `cargo test -p runtime --lib observation::tests` | 2 通过 | 旧观测与未知用量、生成契约 |
| `cargo test -p runtime --lib orchestrator::tests::tool_result_projection_keeps_raw_trace_but_does_not_admit_omitted_text_as_evidence -- --exact` | 1 通过 | 调整工具结束时机后，真实分发仍保留原始结果、投影边界及证据准入 |
| `pnpm --filter @understand-book/observability test` | 21 通过 | TypeScript 观测、导入与评测合同 |
| `pnpm --filter @understand-book/observability typecheck` | 通过 | 新字段跨语言类型一致 |
| `python -m unittest discover -s evals/semantic -p test_langsmith_usage.py -v` | 3 通过 | 原始 DeepSeek 回执、兼容缓存细分、真实零与未知用量区别 |

两份教学 runner 从仓库外工作目录执行 `--check` 均通过，分别构建 12 / 6 个输入，无模型调用。Rust 日志：[服务端](../../../tmp/langsmith-repair-20261025/server-observability-tests.log)、[运行事件](../../../tmp/langsmith-repair-20261025/runtime-run-events-tests.log)、[观测合同](../../../tmp/langsmith-repair-20261025/runtime-observation-tests.log)、[工具投影](../../../tmp/langsmith-repair-20261025/runtime-tool-projection-test.log)。现有 ts-rs 属性解析、deprecated/unused 警告不影响通过结果。

## 缓存分析

近 7 天产品输入 token 加权命中率 **78.8058%**。本项目已有 22 次调用超过 99%，最高 99.8668%。当前优先级为拆分粗粒度 working-state 片段、稳定高频工具请求族、将已发送大参数的回收放到明确压力边界。

详细依据、最近 24 小时分组、真实相邻请求及三步实验方案见 [DeepSeek 99% 缓存分析](../../../output/langsmith-cache-20261025/cache-analysis.md)。这些是待验证的优化候选，不是本轮已经取得的收益。

## 限制

- 本轮代码已于 2026-10-26 发布；此前生成的记录保持其原始状态。
- LangSmith 当前 `deepseek-v4-flash` 目录价是美元离峰估计，不表示人民币峰谷账单。峰谷按调用时刻另算，不修改供应商回执。
- 已经被旧代码清除、没有原始回执的终态无法凭空恢复。当前 32 条不完整产品调用仍保持未知。
- 持久恢复需要配置 spool。本机 `.env` 与本进程环境未设置 `UB_OBSERVABILITY_SPOOL_DIR`，这只说明本地配置，不代表线上配置；未启用时只能在当前进程内重试。
- 服务端已接收最终 PATCH、客户端在确认写盘前崩溃的歧义，不按成功推断；重复更新被拒绝时保留未确认记录。退出预算耗尽但尚未持久化的队列，也不保证可恢复。
- 原观测修复切片未清理其他工作区改动；后续合并发布的实际范围见发布记录。
