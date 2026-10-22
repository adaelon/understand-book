# 2026-10-20 画像阶段增量补位

技术书在 Pass1 已关闭、画像阶段已准备完成后，Driver 跨进程复用该 invocation 的派生路由目录。日常补位不再重新读取全部 Pass1 结果和画像任务结果。

## 实现

- `packages/core/src/automatic-build-incremental.ts` 保存任务目录与已观察状态。用现有来源和策略身份绑定目录，通过文件元数据检查可变的来源、策略、迁移目录及发布成果；当前代次的任务产物沿用只创建、不覆盖的写入合同，通过目录新增、删除发现变化。
- `skills/build/automatic-build-driver.ts:loadDriverState` 将增量路由交给原规划、预算与派发路径。同一 invocation 的后续 CLI 进程可以复用索引。租约、尝试、重试、预算与当前空槽继续读取实时持久记录。
- 普通 discourse、公式任务只读取变化结果并调整待办集合。分片与归约结果变化时，`build-orchestrator.ts:refreshAutomaticBuildProfileSidecar` 仅重路由所属段落及其归约依赖，保留其他任务的描述与顺序。
- 新归约任务仍由 Driver 的原准备入口落盘。计算前记录产物目录观察，计算期间到达的其他提交留给下一轮，避免将未读取的结果标记为已消费。
- 初次建立索引、上游或策略变化、遇到未知的新任务，以及画像最后一个任务完成时，回到原完整路由或阶段收口流程。显式只读快照 API 仍保留完整检查。

派生数据位于 `.build/automatic-build/v3/incremental-profile/<invocation_ref>/`。普通结果提交只更新小状态文件；任务结构变化才重写任务目录。该目录不承载语义成果、授权、租约或提交权威，可以由持久成果重新构建。决策见 [ADR-0121 §2](../adr/0121-bounded-build-state-reads-and-executor-lifecycle.md#2-驱动器状态复用)。

## 已通过的验证

Driver、补位、并发和派发运行时的 **112 项回归**通过，包括 8 路补位、恢复、预算批准与阶段收口；新增 **3 项增量场景**通过：

| 场景 | 结果 |
| --- | --- |
| 实际 Driver 连续规划、持久策略变更 | 第二轮复用索引；策略变化后返回原恢复边界 |
| 普通 discourse 与公式各提交一项 | 只读 2 份变化结果；Pass1 产物、迁移回执读取均为 0 |
| 没有新结果 | 任务产物和迁移回执读取均为 0 |
| 删除已有结果、刷新期间其他代理提交 | 待办恢复，晚到提交在下一轮被发现 |
| 删除上游 `base.json` | 索引失效，重新进入完整 Pass1 路径 |
| 长段落分片完成、准备归约任务 | 仅更新所属段落，未重读 Pass1；待办、任务描述、绑定及质量记录与完整路由一致 |

Core TypeScript 检查、插件发布源码契约和 Windows 引擎编译通过。

证据：[调度回归](../../tmp/incremental-refill-20261020/driver-regression.log)、[增量场景](../../tmp/incremental-refill-20261020/incremental-tests-final.log)、[类型检查](../../tmp/incremental-refill-20261020/typecheck-final.log)、[发布契约](../../tmp/incremental-refill-20261020/source-contract.log)、[编译](../../tmp/incremental-refill-20261020/build.log)。

编译版的小型隔离场景进一步确认：不同 CLI 进程复用相同索引，接纳一个 discourse 结果后待办减少一项，普通提交前后任务目录字节和修改时间不变；派发数遵守请求的 8 路上限。该夹具只有 3 个可派发组，未调用模型。证据：[编译版结果](../../tmp/incremental-refill-20261020/compiled-smoke-result.json)、[验证入口](../../tmp/incremental-refill-20261020/compiled-smoke.mts)。

## 本机安装

- 已测试的引擎安装到 `E:\allwork\Understand Book\understand-book-build.exe`，原程序保存在同目录 `understand-book-build.before-incremental-20261020143709.exe`。
- 插件更新为 **`0.1.0+codex.20261020143709`**；本地市场查询确认 `installed=true`、`enabled=true`。首次缓存切换遇到 Windows 拒绝访问，重试后成功，未停止正式构建或其执行器。
- 安装引擎与已测候选、安装插件的 11 个文件与发布源码均逐字节一致。后续新启动的根控制调用使用新引擎，已打开的执行器保持现有会话；本轮未改变其协议。

证据：[引擎安装](../../tmp/incremental-refill-20261020/installation.json)、[插件安装](../../tmp/incremental-refill-20261020/plugin-update-retry.json)、[本地启用状态](../../tmp/incremental-refill-20261020/plugin-list-local.json)、[安装文件比对](../../tmp/incremental-refill-20261020/installed-verification.json)。

## 测量边界

本轮没有有效的实书端到端提速数字。正式构建仍在运行，同机大书复制与测量会争用磁盘、CPU，也会影响计时。2026-10-20 22:36（香港时间）确认正式聊天进行中、进度 471/6265，并报告 8 个执行代理已启动后，停止本轮实书测量。

此前实书测量只操作独立书籍与调度记录副本。第一次因镜像缺少发布资源在兼容性检查处停止；补齐后又遇到副本中的派发投影冲突，未完成正常补位。这些样本保留在 `tmp/incremental-refill-20261020/`，不作为提速证据。后续验证使用小型独立夹具，不调用模型或占用正式构建的执行槽位。

本轮证明了日常补位减少全量读取、变化任务与完整路由结果一致；首次索引构建仍需完整路由。这里不据此推算整本书的完成时间。
