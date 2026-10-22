# 自动构建补位延迟根因：最优化实际书籍

后续已完成修复与同书实测，见[补位调度修复与实测](automatic-build-scheduling-fix-20261020.md)。下文保留修复前的诊断证据。

2026-10-20。一次真实 invocation 的调度路径，仅执行到首次持久化写入之前，就耗时 **179.3 秒**。主要成本是每次补位重新规划整本书，并在同一调用中重复检查迁移、尝试和预算状态。三个执行器的并发配置无法并行化这条串行控制路径。

## 实际场景

- 输入：`E:\allwork\download\最优化_建模算法与理论.md`。
- 阅读目标：`standard_deep`，Pass2 关闭，并发 3。
- 当前 durable 投影：Pass1 468/468；profile_sidecar 187/6265，待办 6078。
- profile_sidecar 包含 1190 个 discourse 和 5075 个 formula 工作单元。
- 用户已确认继续超过默认任务数预算；本轮只定位调度耗时。
- 当前源码基于 HEAD `33c19c7fa8aec9a4f1031122bc177743b234b9fd`，包含本轮之前已有的未提交修改。诊断没有修改生产实现。
- 日常安装程序：`E:\allwork\Understand Book\understand-book-build.exe`，107200512 字节，修改时间 2026-10-20 14:39:45。相关安装记录见 [preparation-plugin-install-20261020.md](preparation-plugin-install-20261020.md)。

## 测量

使用 Node v24.9.0 和当前源码，在同一本书、同一工作区上进行两次不同范围的测量。计时包装委托真实文件读取，CPU profiler 用于调用链归因。

第一次只调用只读 `automaticBuildPlan`，输出数量和耗时。第二次通过保存的 opaque invocation 调用 `automaticBuildStep`，屏蔽状态写入，在首次写文件前终止；已有目录的递归创建调用直接返回，不实际创建目录。两次都没有调用模型、启动执行器或生成语义候选。

| 测量范围 | 墙钟时间 | 文件存在性检查 | 文件读取 | 存在性检查及读取时间 |
| --- | ---: | ---: | ---: | ---: |
| 单次只读规划 | 49.516 秒 | 34195 次 | 13292 次 | 34.133 秒，68.9% |
| 同一 invocation 的 step，至首次写入前 | 179.330 秒 | 114647 次 | 52585 次 | 128.542 秒，71.7% |

第二次的文件存在性检查用时 82.149 秒，读取用时 46.393 秒，另有 4369 次目录枚举用时 3.029 秒。迁移记录本身占其中 72.170 秒：23904 次存在性检查、47788 次读取。

CPU 采样对第二次调用的主要归因如下。前三行是并列调用范围；子调用表中的时间不能再与它们相加。

| 并列调用范围 | 采样归因时间 |
| --- | ---: |
| `loadDriverState → automaticBuildPlan`，两次整书规划合计 | 116.880 秒 |
| `transitionNow → transitionStateKey`，为状态标识读取待办的尝试记录 | 18.381 秒 |
| `automaticBuildNext → expandAction`，审计、占用检查、预算及派发前处理 | 41.847 秒 |

其中，两次规划中的快照构建合计约 105.7 秒；`automaticBuildNext` 内的旧产物审计约 23.0 秒。以上为采样归因，存在 GC、采样误差和插桩开销。

## 已确认的原因

### 1. 补一个槽位仍重新扫描整书

`skills/build/automatic-build-refill.ts:60` 无条件进入完整 step。`skills/build/automatic-build-driver.ts:955` 每次加载状态调用 `automaticBuildPlan`，再由 `skills/build/automatic-build.ts:1325` 重建快照。

`packages/core/src/build-orchestrator.ts:3051` 重新路由已经完成的 Pass1，随后在 `:3137` 路由全部 profile_sidecar。`:1724` 遍历跳过项，`:1744` 遍历全部语义包；每项进入 `:554` 的迁移读取，并检查产物。

`packages/core/src/automatic-build-policy-generation.ts:1104` 对每条迁移记录执行存在性检查和读取。第一次只读规划中，仅迁移记录就发生了 11947 次检查和 11947 次读取，耗时 21.502 秒。这个数字包含路由过程中检查的迁移记录，不等于待生成任务数。

执行器容量在派发计划最后才用于切片，见 `packages/core/src/automatic-build-dispatch.ts:298`；容量为 1 不能减少前面的整书扫描。

### 2. 已批准预算仍触发完整流程重跑

`skills/build/automatic-build-driver.ts:2352` 每次 step 将 decision effect 初始化为空。`:961` 规划使用 invocation 原有预算；`:1646` 的注释明确说明保留原预算以重放批准边界。

默认 `max_tasks` 是 2000，见 `packages/core/src/automatic-build-budget.ts:79`。当前待办为 6078。因此先经 `automaticBuildNext` 得到预算边界，`:2465` 找到已批准回执，然后 `:2468` 执行 `continue`。循环顶部 `:2409` 随即再次加载整书状态。

测量中 profile 准备文件分别在第 12.801 秒和第 100.203 秒被读取，证实一个 step 内进行了两轮快照构建。批准已经生效；多余成本来自批准重放的控制流程。

### 3. 同一轮中的多个检查重复读取同一批状态

- `skills/build/automatic-build-driver.ts:885` 为计算调度状态标识，遍历所有待办并调用 `readAutomaticBuildAttemptRecord`。这条路径在测量中累计约 18.3 秒。
- `skills/build/automatic-build.ts:1572` 每次进入提取动作都调用旧产物审计。`packages/core/src/automatic-build-legacy.ts:192` 无条件建立迁移记录索引，`:126` 递归读取全部迁移记录；传入 snapshot 并没有消除这次读取。它在两轮决策中再次读入迁移记录，累计约 23.0 秒。
- `skills/build/automatic-build.ts:1614` 在预算判断和批次选择之前，对全部待办执行占用及重试检查。
- `skills/build/automatic-build.ts:1345` 和 `:1687` 分别在规划与执行决策中进行 preflight。`packages/core/src/automatic-build-budget.ts:756`、`:766` 又分别生成当前待办及整个阶段的派发计划。第二次测量中，preflight 总采样归因约 19.2 秒，已包含在上表规划及动作时间中。

迁移记录共被读取 47788 次，是单次快照读取数 11947 的四倍：两轮快照加两轮旧产物审计。

### 4. 终态收割协议将长补位放大为连续两轮

`skills/build/SKILL.md:352` 要求收到第一个子执行器终态后立即补位；`:425` 要求在该调用期间收到另一个终态时，先再调用一次 refill，然后才能启动返回的新执行器。当前安装技能具有同样规则。

实际构建中，其他执行器经常在第一次长补位期间结束，因此新一轮生成要等待第二次完整补位。每次 refill 又通过 `automatic-build-refill.ts:60` 进入完整 step。这使并发 3 在批次之间出现长时间空槽。

## 修复优先级

1. **先消除预算批准造成的同请求整书重读。** 预算 effect 变化时复用当前 snapshot 和已经读取的状态，仅重新评估受预算影响的决策；保留批准证据的匹配语义。
2. **合并同一次补位中的状态读取。** 复用迁移回执和尝试记录；旧产物审计先确定实际需审计的文件，避免无旧产物时构造整书迁移索引。全局重试及占用判断保留，但同一次调用不重复遍历和读盘。
3. **让稳定补位的读取量跟随变化任务。** 已完成阶段及已准备的任务描述沿用已有准备机制；补位刷新完成、失败和活动租约信息。实现时验证同伴提交、重试、容量变化和输入/策略变化仍能正确推进。
4. **缩短追加终态的处理。** 在不破坏槽位所有权的前提下，让第二次收割复用刚形成的计划，仅更新容量和必要状态。相关 Engine 行为与构建技能应一起调整。

性能验收应使用本书相同规模，主指标为“执行器终态到下一执行器可生成”的间隔，同时记录一次 refill 的规划次数和文件调用数。收益需要在实现后实测。

## 之前的优化为何没有解决

[Profile discourse 准备性能报告](profile-discourse-preparation-20261020/README.md) 使用 96 个普通任务和 36 个 fragment 任务。它明确记录普通场景补位为 2.374 → 2.360 秒，差异处于波动范围；主要收益来自执行器输入接收。该样例没有覆盖本书 6265 个任务、5075 个 formula 及大量迁移回执的负载。

准备缓存已在本轮命中。第一次规划中，46.4 MB 准备文件读取仅用时 0.190 秒；第二次读取两遍合计 0.586 秒。主要瓶颈仍在缓存命中后的整书路由及状态扫描。

## 原始证据与复现

- [规划指标](../../tmp/scheduling-root-cause-20261020/planning-metrics.json)
- [调度指标](../../tmp/scheduling-root-cause-20261020/driver-metrics.json)
- [规划 CPU 归因](../../tmp/scheduling-root-cause-20261020/cpu-summary.json)
- [调度 CPU 归因](../../tmp/scheduling-root-cause-20261020/driver-cpu-summary.json)
- 原始 CPU profile 与测量入口位于 `tmp/scheduling-root-cause-20261020/`。

从仓库根目录运行：

```powershell
node --import tsx tmp/scheduling-root-cause-20261020/probe.mts
node --import tsx tmp/scheduling-root-cause-20261020/driver-probe.mts
node tmp/scheduling-root-cause-20261020/summarize.mjs driver
```

## 测量边界

- 本次时间来自 Node 源码插桩，不能作为已安装编译程序的精确耗时或优化收益。实际调用此前已有数分钟的补位等待，本次测量用于定位同一实现的成本来源。
- 179.3 秒是调度至首次写入前的测量，未包含后续发布、原生子代理创建及模型生成；不是完整端到端时延。
- 真实工作区中已有三份未启动交接，测量没有启动它们。数据和缓存使用当时状态，未清空系统文件缓存。
- 每个测量范围仅取一次昂贵样本，用于根因归因，不提供中位数、p95 或修复后的加速承诺。
- 没有更改生产代码或构建状态。本轮证据支持上述实际路径，尚未验证任何修复候选。
