# SR4b Action 复用与当前 Ledger 恢复 — 2026-10-01

状态：**已完成，下一步 SR5。** 合同：[ADR-0149 §5](../adr/0149-构建期语义候选召回与Core-owned-Embedding-Provider.md)、[SR4b](../切片方案-构建期语义候选召回.md)。

## 实施

`teaching-build.ts:accepted` 从当前 accepted fragments 与 previous 重建 ledger，先取得当前有效检索准备并渲染实际模型输入，再以原有 input/prompt/semantic-contract 绑定查找 action。非终态 action 在当前 ledger 上重新执行 `advanceObjectAlignment`，读取、inspect、来源及身份接纳 gate 继续生效。Search 重新产生请求；下一步从当前依赖准备结果，不恢复历史搜索页。旧 action 在当前 gate 下失效时，其原始 receipt 保留，替代动作使用独立的 `-replay` 任务槽。

接纳 artifact 增加 `accepted_action`，保存模型原始输出。`ObjectAlignmentWork` 内部版本升为 `formal_object_alignment.v3`，alignment 任务使用 `align-v3-…` 标识以隔离没有 action 记录的旧 artifact。模型 action schema 仍为 `formal_objects.v2`，policy/generation 仍为 `formal_objects.v3` / `formal_objects.full.v3`，正式成品仍为 `formal_objects.v1`；本轮没有改变模型动作语义或 prompt。旧 generation、冻结任务和接纳记录保留。

`freezeTeachingTask` 对 alignment 分别处理冻结模型任务与当前 Core 执行状态。原 `.task.json` 永远保留当时模型输入；同模型任务的新 ledger、source、previous 和检索准备写入 `.task.json.current.json`。Input bridge 读取原任务，writer bridge 读取当前执行状态。Provider/config、诊断和向量缓存路径不加入模型身份；相同实际输入不会因这些字段变化产生 frozen-task conflict。

非终态动作始终重放，不恢复整份旧后状态。Finish 的 artifact 另存 `finish_dependencies`，直接比较完整 proposal（包括引用与覆盖）、previous 和 source。相同依赖恢复原 FormalObjects，并在当前 work 上完成收尾；检索或诊断变化本身不会分配新 ID。依赖变化时，普通路由只返回已接纳 finish、等待物化的状态；`closeTeachingStage` 沿原接纳规则重新物化，先保存 `.task.json.materialized.json` 中的依赖/结果，再发布正式文件。重复关闭或发布重试恢复同一结果，普通路由不分配身份、不写准备记录。

向量仍在可删除的 `.build/semantic-retrieval/cache.json`；action、冻结输入、当前执行状态及 finish 物化记录由 teaching 路径保存。完整依赖直接比较，没有增加总状态摘要。

## 验证

运行前确定检测目标：教学集成检测旧 proposal 覆盖当前目录、显式 query 重检索缺失、配置变更误改冻结输入、当前 gate 被跳过、finish 重复分配身份；alignment/map/fragments 检测来源读取、引用、拆并、覆盖和发布回归；retrieval/hybrid 与 orchestrator 检测准备失败被变成模型重试、旧搜索页被复用或普通读取产生调用。失败时修对应实现，保留原门禁。类型检查检测新内部版本和 writer 接点的调用不兼容。

新增恢复用例通过真实 task/input/writer/artifact 路径覆盖：

- 自动 focus 输入相同、同一显式 query 在 provider/config 变化后排序改变：复用旧 search，下一步采用新结果，旧 read 决策失效。
- Provider/config 改变但实际输入相同：复用已接纳 read，累计范围保持，待处理任务使用当前 Core 状态提交，原冻结文件与 receipt 字节不变。
- Resolve 改变含义：独立对象及引用它的 relation/composite 共三条投影重算，其余文档向量命中缓存。
- 当前 fragment 的未展示对象改变：bounded input 不变，inspect action 重放保留新对象；原 SR0 缺口用例已改为回归保证，累计 read 与 32 步预算门禁继续验证。
- 当前目录不再包含旧 inspect 目标：当前 gate 拒绝复用，新的有效动作独立接纳，旧 receipt 保留。
- Finish 后删除缓存、provider 离线：准备记录足以恢复原正式身份；覆盖义务变化只在关闭时重新物化，再次关闭保持新结果的身份与字节一致。
- SR4a 既有失败/取消/恢复用例继续检测 search receipt、attempt 与已读范围保留；旧 v1/v2 alignment 提交不能进入 v3 接纳。

目录变化测试在 fixture 中以有效 envelope 替换受控上游 fragment 结果，用于隔离当前 ledger 恢复行为；内容改版与跨版重绑仍由 U3–U10 验收。

```powershell
pnpm --filter @understand-book/core exec vitest run test/teaching-build.test.ts test/teaching-object-alignment.test.ts test/teaching-map.test.ts test/teaching-object-fragments.test.ts test/automatic-build-retrieval.test.ts test/semantic-retrieval-hybrid.test.ts test/build-orchestrator.test.ts --testTimeout=60000 --maxWorkers=1 --no-file-parallelism
pnpm --filter @understand-book/core typecheck
```

**最终 7 文件、83 项测试通过，进程退出码 0；Core 类型检查通过。** 教学集成由 13 项增至 18 项，既有 SR0 缺口用例改为 SR4b 回归。原完整目录、来源读取、拆并、旧 generation、阶段发布、Pass2 两种计划及工作区搬迁路径保持通过。

首轮修改后的目录替换夹具因没有重建 envelope 的既有完整性字段而未进入重放路径，修正为有效 envelope 后通过。随后两轮整组运行均为 83 项断言通过，但长同步教学用例连续执行导致 Vitest 的 `onTaskUpdate` 报告通信超时，串行本身不能解决。测试文件在每项结束后通过 `setImmediate` 让出一次事件循环；最终同组串行运行约 137 秒，全部通过且无未处理错误，没有提高超时或忽略错误。

## 已知限制

- SR5 的真实 adapter、配置/确认面与 Codex/DSH host/driver 异步注入待实施，生产 semantic mode 尚未开启。
- 本轮仅使用计数 fake provider，无真实 embedding 或语义模型调用；既有真实样本额度已用完。真实身份质量、长材料与固定 Gold 的三组对照仍由 SR6/T5A 验收。
- 未提交 Git、未打包、安装或部署。工作树中其他工作线改动保留，现有安装包不代表本次源码。
