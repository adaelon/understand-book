# SR4a BuildPlan 与获授权异步准备 — 2026-10-01

状态：**已完成，下一步 SR4b。** 合同：[ADR-0149](../adr/0149-构建期语义候选召回与Core-owned-Embedding-Provider.md)、[SR4a/SR4b](../切片方案-构建期语义候选召回.md)。

## 实施

`BuildPlanV1.retrieval` 绑定 selection、estimate 和 budget。Selection 包含 `retrieval_mode`、provider/model/revision/config/dimensions、local/remote 与 `current_and_previous_formal_object_projections_and_queries` 数据范围；estimate 的 records/queries 可以为 null，并注明 `unknown_until_fragments`。预算必须提供 documents/queries/calls 上限，可追加 input tokens 上限；全部进入既有 plan identity。`compileBuildMode` 可接收并保留这些字段。无该字段的既有计划保持原路径。

`prepareAutomaticBuildSnapshot(target, stage, {authorization})` 为显式异步重载。先走既有确认、目标、来源、profile、closure 与 freshness gate，再比较运行配置和已确认 selection；只有当前可执行阶段可以准备。未确认或漂移返回 `needs_user`，不发 provider 请求。旧同步入口与普通 snapshot 保持同步读取行为。

`automatic-build-retrieval.ts` 持久化 `.build/teaching/retrieval.json`：已选择配置、各步骤的准备依赖/结果、逐次调用记录和失败原因。该文件独立于可删除的 `.build/semantic-retrieval/cache.json`。普通 teaching router 直接比较当前目录、focus/search 和配置依赖；缺失时保留 `preparation_required`，不创建下一 alignment 模型任务。Remaining-work 展示当前 records、缺失 documents/queries；没有 adapter 批量配置时 calls 为 null，执行准备时展开精确批数。

获授权准备先以缓存 miss、query 和实际 tokenizer 计算本次工作量，整体不足时零调用。每次 adapter 调用前持久化 records/query/call 和输入 token 预留，结束后记录成功/失败/取消、耗时及可获得 usage/cost。失败调用和后续重试均消耗额度；同 plan ID 升 revision 不抹掉此前消耗。未知 usage 保持缺省，并记录 unknown usage calls。预留用于额度判断，不冒充实际用量。

检索 token 额度同时受 BuildPlan 总 token 上限约束；后续模型预算计入已知 embedding tokens，并为未知部分保留预测占用。墙钟上限结合现有计划时长预测和累计 provider 耗时，本次调用用 AbortSignal 传递取消；结束时清除计时器和监听。Adapter 的每次实际尝试应经过 Core 的单次调用边界，本切片不提供隐式重试。

`teaching-build.ts` 将有效 PreparedRetrieval 传入真实 task/input/writer 路径。Search 接纳仅保存 query/offset；inspect/read 保持搜索和累计原文读取；resolve 后下一 focus 重新比较目录投影，只重算变化文本。有效准备页可脱离向量缓存及在线 provider 使用。失败/取消记录为 retrieval 原因，不转成 `semantic_output_invalid`，恢复只补准备，不创建或增加模型 attempt。

`skills/build/automatic-build.ts:automaticBuildNextWithPreparation` 是异步控制入口，CLI `next` 已等待该入口；host 可显式注入独立于 Harness 的 runtime/provider。原计划预览及同步读取不请求 provider。配置入口、生产 host/driver adapter 注入由 SR5 完成。

## 验证

运行前确定检测目标：授权测试检测未确认/漂移/超额调用；用量测试检测失败和重试漏计；教学集成检测 search 被重生成、读取范围丢失、提前创建下一任务或 resolve 漏重算。原 snapshot、预算、计划 gate 与 cache 回归检测新增接点改变既有行为；类型检查检测同步/异步调用合同不兼容。失败时修对应实现，保留原接纳门禁。

首轮新增授权与教学集成共 23 项通过；补齐未知 token 的后续预算占用、resolve 投影更新与准备失败后的配置保留后，执行最终回归：

```powershell
pnpm --filter @understand-book/core exec vitest run test/automatic-build-retrieval.test.ts test/teaching-build.test.ts test/embedding-provider.test.ts test/semantic-retrieval-cache.test.ts test/semantic-retrieval-hybrid.test.ts test/build-intent.test.ts test/automatic-build-budget.test.ts test/automatic-build-plan-gate.test.ts test/automatic-build-observation.test.ts test/build-orchestrator.test.ts --testTimeout=60000
pnpm --filter @understand-book/core typecheck
```

**10 文件、83 项通过；Core 类型检查通过。** 新增 11 项用例（授权/预算 10 项，真实教学 input/writer 集成 1 项），使用计数 fake：

- 未确认、location/provider 漂移、计划预览零调用；selection/data scope/额度绑定既有 plan digest。
- documents、queries、calls、input tokens、BuildPlan total tokens 与 wall-clock 不足时零调用。
- 一批成功、一批失败后，已完成向量保留；剩余文档为 20，失败的 8 条仍计额度，恢复后累计 36 documents/1 query/6 calls。
- lexical_only 与空 query 零 embedding；两种 Harness 的计划读取共用 Core 状态；缓存删除和 adapter 缺失时复用有效准备页。
- 教学 fragment 接纳后返回待准备；search artifact 在失败/取消/恢复期间字节不变，原 attempt=1 保留；inspect/read 不增加 query 调用，累计读取范围保留；resolve 改一条独立定义只新增一条 document embedding。
- 既有 generation/read gate、Pass2 两种计划、小材料发布、定向修复、普通快照零写入及原预算 gate 通过。

## 已知限制

- SR4b 的模型 action 重放、当前 ledger 依赖判断和冻结任务表示尚未实施；当前 accepted artifact 仍保存完整 alignment 后状态，原 SR0 缺口测试保留。
- 生产默认未开启 semantic mode。SR5 仍须实现真实 adapter、配置/确认面，以及 Codex/DSH host/driver 的异步 adapter 注入；同步 driver 遇到待检索准备会保持 `needs_user/preparation_required`。SR4a 已提供可调用的 Core 与 CLI 异步边界。
- 本轮没有新增真实 embedding 或语义模型调用，SR1–SR3 的 40 documents/10 queries 额度仍已用完；真实 Agent 身份质量、长材料与发布门槛留给 SR6/T5A。
- 未提交 Git、未打包、安装或部署；保留工作树中其他工作线改动。
