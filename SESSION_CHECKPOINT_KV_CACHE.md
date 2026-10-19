# SESSION_CHECKPOINT — 上下文缓存稳定性 — 2026-10-15

## 当前状态

用户确认按 CC1 → CC2 → CC3 改动，并保留权限、来源验证，小切片验证实际效果。三个实现切片和 CC4 验收已完成；用户随后明确要求上线，2026-10-16 00:14 HKT 已部署 `context-cache-20261015`，PID14712，schema14。源码与报告在当前工作区，尚未提交。开始时基线为 `402d9be`，工作区存在其他任务改动，未回退或纳入本任务。

## 本次交付

- CC1：冻结基础 system；sampling ledger 发布完整有效资产清单与新增/变化正文；source.present 独立合法名额；Open PresentationDelivery Goal 只预启用 author，不恢复旧来源/Reader 权限。
- CC2：48 KiB 只限制新鲜结果批；低于整体高水位不改旧正文，软压力达到一批或硬预算不足时整批退休已采样正文；候选估价用 ledger 副本。
- CC3：完整 checkpoint 先验证，模型仅消费九组语义的 text/evidence_refs 与 pending_effects；can_reduce 使用同一精简 helper。持久格式、来源覆盖、window identity 与 Server 恢复不变。

## 验证证据

- Runtime 最终全量：590 通过、0 失败、4 忽略。原始日志：`tmp/context-cache-cc-20261015/runtime-final-fixed-fixtures.log`。
- 真实 checkpoint 手动 Rust 投影单独通过：86,934 → 14,524 B，减少83.29%；41项正文、引用、顺序逐项一致。完整检查点仍保留208项coverage和151项receipt。
- 探针本地4项通过；8次固定记录真实调用完成，非思考模式、输出1 token。输入50,373→52,213，未命中22,213→23,541，缓存率55.90%→54.91%。该短样本没有测出缓存改善，不能声称正式长会话已达到80%。
- 五处既有测试场景不一致已仅修改fixture：策略版本、导航能力前提、中文页面示例、工具激活预热、搜索结果分页。没有放宽生产门禁。

## 可直接接手

1. 阅读 `docs/performance/context-cache-cc-20261015/README.md`：完整结果、限制、原始请求及复现命令。
2. 阅读 `docs/切片方案-上下文缓存稳定性.md` 与架构/代码链路新增章节；本轮没有剩余实现工作。
3. 上线完成：Linux 正式构建五个程序、Runtime 590/0/4、公网读者与管理员验收通过；停服至本机恢复验收16秒。完整备份及恢复边界见 `docs/performance/context-cache-cc-20261015/deployment/README.md`。旧 release 被资源链接引用，须保留。
4. 线上长会话总体缓存率尚未测量；对真实长任务应同时比较输入、未命中、缓存、输出与交付质量，不单追命中率。本次部署验收模型调用0。

## 关键路径

- `crates/runtime/src/orchestrator.rs`：统一请求builder、Goal预启用、候选估价副本。
- `crates/runtime/src/context_fragment.rs`：差量指导manifest及历史定义。
- `crates/runtime/src/tool_exposure.rs`：来源呈现独立slot。
- `crates/runtime/src/tool_result.rs`、`cache_pressure_tests.rs`：新鲜批与整体压力回收。
- `crates/runtime/src/compaction.rs`：验证后消费投影、相同预算下限、真实样本手动记录。

## 历史沿革

2026-09-28 的 DeepSeek 用量、续接持久化、尾部采样状态和压缩独立预算工作已在此前集成；原记录详见 `docs/计划-调用成本与DeepSeek适配.md`、CONTEXT.md 和架构中的“调用成本与模型续接”。本次保留这些既有合同。
