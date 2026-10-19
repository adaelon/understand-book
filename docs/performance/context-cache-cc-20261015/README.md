# 上下文缓存稳定性：实现与验收

基线为 `402d9be`。本轮按 CC1 → CC2 → CC3 修改 Runtime，并保存同一受控流程的修改前后请求。2026-10-16 00:14 HKT 已部署至正式服务，release `context-cache-20261015`、schema14；Linux 回归590通过、0失败、4忽略，公网验收通过。见[上线与恢复记录](deployment/README.md)。

## 实现结果

1. **CC1：稳定 system 和合法工具集合。** 基础 system 只使用本轮冻结的 profile。工具策略、演示阶段和 Tutor 指导沿现有 sampling ledger 追加，最新 `active_assets` 清单决定有效模块，正文只在新增或完整内容变化时发布。相同 ID／revision 的正文变化也会被识别。`source.present` 有独立 direct slot，仍要求注册、权限和当前来源证据，schema 字节预算保持不变。Open Goal 的 PresentationDelivery requirement 可预启用 author；旧回合来源和 Reader 写权限不恢复。
2. **CC2：取消 48 KiB 滚动逐条淘汰。** 48 KiB 约束尚未采样的新结果批。已采样正文低于实际请求高水位时保留；软压力且已积累一批，或硬预算不足时，一次降为既有回执。尚未采样结果不退休，continuation、receipt、来源范围和持久消息不变。候选 checkpoint 估价使用 ledger 副本。
3. **CC3：完整验证后精简消费投影。** 持久 checkpoint、版本、来源覆盖、历史修订、window identity 的格式与校验保持原样。模型只接收九组语义项的 `text`／`evidence_refs` 与 `pending_effects`。`can_reduce` 使用同一 helper 计算最小消费开销。

最初把全部策略正文重复追加的实现，在四份请求中增加 90,455 字节，已由差量模块发布替换。最终 Native 四份请求总字节从 215,710 到 223,546，增加 7,836（3.63%）；3 次相邻转换中首条 system 稳定、完整历史消息保持的次数均从 2 次到 3 次。工具集合变化仍有 1 次，来自实际激活 author。ReAct 的工具协议位于首条消息，激活工具时它仍会变化；其余已发送历史保持不变。见 [Native 结构对照](native-comparison.json) 与 [ReAct 结构对照](react-comparison.json)。

## 真实检查点测量

样本为 2026-10-13 的实际安装检查点，trace 中 run `01a135cf-4480-7932-b05b-8c68822f48e9` 的模型输入。Rust 生产 helper 的实际输出与独立 Python 投影逐项相等：

| 项目 | 修改前 | 修改后 |
|---|---:|---:|
| 紧凑 checkpoint JSON | 86,934 B | 14,524 B |
| 语义项 | 41 | 41 |
| 语义正文／来源引用／顺序 | 原始 | 完全相同 |
| pending effects | 0 | 0 |

消费 payload 减少 **72,410 B（83.29%）**。208 项来源覆盖与 151 项工具回执仍保存在完整 checkpoint，只从模型消费 JSON 中移除。见 [测量结果](checkpoint-measurement.json)。这项测量没有把字节数当作 token 数。

## 真实 Provider 对照

使用既有 EX13 受控流程的最终 Native 请求，分别重放索引 `0,1,2,5`：激活 author、global → local/state、state → editing。两组各 4 次；每组独立前导标记防止组间文本前缀复用；输出上限 1 token；固定后续对话，不使用这次模型的新输出。

| Provider usage | 修改前 | 修改后 |
|---|---:|---:|
| 输入 tokens | 50,373 | 52,213 |
| 缓存输入 tokens | 28,160 | 28,672 |
| 未命中输入 tokens | 22,213 | 23,541 |
| 加权命中率 | 55.90% | 54.91% |
| 输出 tokens | 4 | 4 |
| 请求耗时合计，不含间隔 | 3,204 ms | 3,114 ms |

**该短样本未证明缓存命中率或未命中输入改善。** 两组都包含冷启动和必要工具激活，两个同工具阶段转换在旧版已经保持前缀。修改后有效模块清单增加输入；Provider 工具前缀和缓存分配也影响结果。后两次稳定工具调用均有较高命中，不能从整个短序列推导正式长任务的比例。耗时差不足以作延迟结论。

第一次按默认思考模式重放时，第一调用成功，第二调用因脚本样本缺少 `reasoning_content` 返回 HTTP 400；一次显式诊断重现了相同错误，随后两组均改为非思考模式完成以上对照。没有修改应用的模型模式。[DeepSeek 的工具思考续接协议](https://api-docs.deepseek.com/guides/thinking_mode/)要求返回续接内容。失败尝试及其额外 5,861 输入／1 输出 token 单列在 [真实结果](live-results.json)，没有混入上表。

复现有界探针（会产生实际 API 调用）：

```powershell
python docs/performance/context-cache-cc-20261015/cache_probe.py live-replay `
  --baseline-dir docs/performance/context-cache-cc-20261015/requests/baseline `
  --candidate-dir docs/performance/context-cache-cc-20261015/requests/candidate `
  --protocol native --indexes 0,1,2,5 --max-calls 8 --disable-thinking `
  --output-dir tmp/context-cache-new-run `
  --sample-description "Fixed controlled tool activation and phase sequence"
```

脚本不自动重试，不隐式预热，缺少 usage 就停止；记录包含实际请求、原始 usage 和各请求耗时。离线比较使用 `compare-request-directories` 子命令，不调用模型。

## 回归与边界

新增用例均先检出旧行为，再在实现后通过。关键覆盖包括首 system 与 Native/ReAct 历史前缀、工具注册／权限／来源／schema 上限、Open Goal 续接、同 ID 正文变化、空选择撤销、跨 48 KiB 保留、软压力整批回收、持续软压力不碎片化、硬预算回收与未观察结果保护、完整 checkpoint 验证及消费字段逐项一致。真实记录手动测试另行通过。

Runtime 最终全量结果：**590 通过、0 失败、4 忽略**。新增手动记录测试属于忽略项之一，已带真实样本单独执行通过。探针本地测试 4 项通过。完整回归包含 Native/ReAct 实际本地 HTTP、压缩修复、来源门禁及续读路径。

第一次全量运行检出本轮请求位置变化及以下五处基线测试不一致。通过比较 Git HEAD、冻结 before 和当前相关实现定位后，只校正测试场景，保留运行时规则及原测试目的：

- `incident_document_overview_plans_with_read_only_structure_before_source_text` 仍要求 evidence-routing revision v4，基线实际常量已经为 v5。
- `resident_navigation_policy_native_and_react_request_plan_snapshots` 只提供 book.text，却要求只在 route／goto 能力存在时加载的 navigation 模块。
- `resident_model_visible_tool_metadata_is_authored_in_english` 对整个 schema 搜索汉字，基线 presentation.author 的页面输入示例已经包含中文正文。
- `agent_tool_policy_general_no_progress_gate_rejects_reordered_same_arguments` 忽略第一次读取后自动激活 book.text_batch 导致的真实进展状态变化；增加定位预热后，在稳定状态下验证重排参数被拒绝。
- `unbounded_default_continues_past_twelve_samplings_to_completion` 的重叠搜索结果超过既有 16 KiB 单结果上限，脚本却直接读取未取得定位的后续位置；改用小页保留完整章节定位，仍验证 16 次采样完成。

前三项分别更新为当前 v5、在导航 fixture 中提供 book.guide_path、只对 schema 元数据检查英文而允许示例页面内容使用读者语言。最终运行记录：`tmp/context-cache-cc-20261015/runtime-final-fixed-fixtures.log`；Rust 1.95.0，`cargo test -p runtime --lib --config profile.test.package.runtime.codegen-units=16 -- --nocapture`。

本轮改变消费投影，没有减少压缩草稿生成、来源覆盖生成的输出和等待时间。固定记录缓存探针不是自主问答质量测试，也没有覆盖线上长会话的总体命中率。正式服务已更新，线上长会话总体缓存率尚未测量，不能宣称已经从 70% 多提升到 80%。
