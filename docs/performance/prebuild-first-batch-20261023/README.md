# 预构建第一批提速：实现与实测

测量于 2026-10-23 至 2026-10-24 在同一台 Windows x64 / Node v24.9.0 环境完成。基线先于本次生产改动采集，基于当时的实际工作树，保留已有未提交改动；HEAD 为 `df019a38526674f84d3326a20f973ccbcabef5f6`。来源与保存位置见 [baseline-state.json](baseline-state.json)。

本批完成三个改动：同次 Driver 决策复用已验证来源；新认知任务首包直接交付必读原文；记录可用容量到生成就绪的真实区间。最明确的速度收益来自认知首包：固定三重点回放减少三次交接，本地链路中位耗时下降 28.2%。来源复用减少了 37.5% 的源文件读取，但专项样本尚未证明整体 step 稳定加速。计时采集补齐可观测性，同时带来持久化开销。

## 1. 实际前后对比

| 场景与指标 | 修改前 | 修改后 | 解释 |
| --- | ---: | ---: | --- |
| 来源专项：每 step 源文件读取次数 | 8 | 5 | 减少 37.5% |
| 来源专项：读取源文字节 | 3,120,056 | 1,950,035 | 减少 37.5% |
| 来源专项：step 中位耗时 | 1,984.439 ms | 1,973.319 ms | 下降 0.56%，样本区间重叠 |
| 认知三重点：交接次数 | 8 | 5 | 每个重点省去一次申请目标原文的动作 |
| 认知三重点：累计实际输入 | 7,223 B | 5,150 B | 减少 28.7%，UTF-8 字节 |
| 认知三重点：本地链路中位耗时 | 261.538 ms | 187.745 ms | 缩短 28.2% |
| 计时专项：可观测样本 | 0 / 7 | 7 / 7 | 首次容量路径；终态补位另有恢复回归 |
| 计时专项：refill 中位耗时 | 0.139 ms | 19.043 ms | 新增观测与落盘约 18.9 ms |
| 计时专项：refill 到 GENERATE 返回 | 2,289.491 ms | 2,428.135 ms | 这次本地样本增加 6.1%，不作为提速收益 |

认知阶段采用真实 Core `route → freeze → render input → writer → close`，使用固定合法候选。五次计时样本的区间从 255.56–263.72 ms 变为 185.55–197.14 ms。公式仍须读取额外定义，证据缺口仍须完成搜索；所有候选样本发布的材料、引用、条件、缺口与覆盖均与基线完整深比较一致。

原始记录与专项说明：

- [来源复用](source-reuse/README.md)：[baseline](source-reuse/baseline.json)、[candidate](source-reuse/candidate.json)。
- [认知首包](cognitive-prefetch/README.md)：[baseline](cognitive-prefetch/baseline.json)、[candidate](cognitive-prefetch/candidate.json)、[基线材料](cognitive-prefetch/baseline-materials.json)。
- [调度计时](dispatch-timing/README.md)：[baseline](dispatch-timing/baseline.json)、[candidate](dispatch-timing/candidate.json)、[隔离基线重放](dispatch-timing/baseline-replay.json)。
- [机器可读汇总](comparison.json)，由 [summarize.mjs](summarize.mjs) 从原始 JSON 计算。

## 2. 大型控制面回放

沿用已有 B1 夹具：Pass1 的 205 个单元已关闭，画像阶段共 315 个单元、251 个待办，3 个活跃 dispatch。每个样本实际执行七项操作、四次匹配的 MCP 调用，并验证恰好一次持久提交。两个版本各保留首轮和三轮相同磁盘起始状态的暖样本。

下表单位为 ms，取暖样本中位数：

| 操作 | 改前原始基线 | 冻结基线隔离重放 | 最终候选 |
| --- | ---: | ---: | ---: |
| snapshot | 727.606 | 814.395 | 669.952 |
| plan | 850.674 | 873.426 | 697.531 |
| build.step | 1,678.463 | 1,788.209 | 1,429.533 |
| executor.open | 90.757 | 90.862 | 80.938 |
| executor.input.next | 54.852 | 53.387 | 48.000 |
| executor.generation.start | 196.953 | 198.020 | 182.940 |
| executor.submit_candidate | 188.963 | 201.353 | 177.123 |

最终 step 比最初基线少 248.930 ms，约 14.8%；未改动的操作也出现较明显变化，因此不把这个百分比全部归因于来源复用，也不外推为整书加速。专项源读取计数给出了更明确的机制证据。

为核查跨运行差异，另在冻结改前源码的隔离目录中重放基线，复用同一依赖安装和未改安装静态文件。隔离重放的安装布局、路径及运行时刻不同，作为波动背景保留，不替换最初在原工作区采集的基线。单段计时专项的隔离重放总耗时为 3,344.626 ms，而最初为 2,289.491 ms；现有样本不足以把完整链路差异拆成纯观测开销和环境变化。直接可测的 refill 新增开销约为 18.9 ms。

数据：[原始基线](control-plane-baseline.json)、[隔离基线重放](control-plane-baseline-replay.json)、[最终候选](control-plane-candidate.json)。首次候选 [control-plane-candidate-initial.json](control-plane-candidate-initial.json) 保留作过程记录，最终表使用修正计时终点后的候选。

## 3. 实现及证据、恢复边界

### 同次调用复用已验证来源

`automatic-build.ts` 的 plan、next、engine/protocol contract 可以接收当前决策已验证的 target。`automatic-build-driver.ts` 在 `loadDriverState` 校验后，把它传给同一状态下的规划、doctor、派发与重放。

每个新请求以及准备、writer、恢复后的状态刷新仍重新解析来源。专项测试在两次 step 之间修改源文件，确认进入既有 `foundation_required`；恢复原文字节后仍可拿到原 handoff。未引入跨请求来源缓存，也没有复用租约或提交权限。

### 认知首包交付必读原文

`prepareCognitiveTaskInput` 用既有 read 操作生成候选阅读状态。短段完整交付，长段最多交付 2,000 个 UTF-16 单位，并同时满足剩余读取量、上下文和完整模型输入预算。只有最终 packet 中实际存在、文本与起止范围一致的原文才进入 writer context 的阅读账本。

`teaching-build.ts` 先重建旧首包身份并检查冻结任务或已接受产物；旧任务继续使用原身份和字节，新任务才采用首包交付。未交付的范围仍不能引用，重复读取同一范围不重复扣费，跨段取证与缺口搜索约束继续生效。现有 prompt、policy 身份和发布要求保持。

### 容量观察到生成就绪

Codex 在 `build.refill` 消费 `terminal_children` 后观察空位；DSH 在实际 drive 循环消费终态、删除 live ownership 后观察空位。终点为首次 `generation.start` 的权威 GENERATE 结果已持久化、可以返回时。候选提交或租约到期不代表宿主已释放槽位。

| 指标 | 含义 |
| --- | --- |
| `dispatch_wait_ms` | 容量被观察为可用 → GENERATE 就绪 |
| `slot_observation_to_ready_ms` | 容量观察 → handoff ready，保留跨 WAIT 的时间 |
| `refill_step_ms` | 返回该 handoff 的本次调度耗时 |
| `handoff_ready_to_open_ms` | ready → executor.open 的记录时点 |
| `open_to_generation_start_ms` | open → GENERATE 就绪，含输入交付与本地生成准备 |

阶段汇总同时给出首次容量、终态补位、覆盖率和缺失样本数。未分配空位持久保存；同一 ref 重放不重置时刻，同一 handoff 的多个工作单元仅首次消耗容量观察。事件记录绑定到具体 attempt、semantic attempt 和 lease epoch，可从原始文件重建汇总；旧样本缺失保持 unavailable，采集失败不阻断构建。

## 4. 验证与复跑

已完成的相关验证：

- Driver 83 项、recovery 4 项、来源新鲜度专项 1 项通过。
- 认知材料与教学构建 42 项通过，覆盖真实交付范围、预算、越界拒绝、旧冻结输入与已接受步骤恢复。
- metrics / refill / executor 恢复定向 21 项通过，覆盖多槽、WAIT 后重启、重放、旧代次缺失、多工作单元去重和观测存储失败。恢复用例验证 epoch 2 对应一次终态补位，确定性时钟为 3,000 ms，分段为 500 / 500 / 2,000 ms。
- DSH 实际 drive 循环的本地生命周期测试通过：宿主和 Engine 使用替身，A 持续运行，B/C 依次结束补位 C/D；A 的两次 live 重放均未重复消费空位。
- 来源性能基准 1 项、大型控制面性能基准 2 项通过；认知与计时回放脚本完成。
- 对冻结的来源复用与认知首包改动进行了独立只读审查，没有发现有依据的正确性、证据或恢复回归。
- Core 与 DSH 两包最终类型检查均通过，相关已跟踪改动的 `git diff --check` 通过。

验证明细见 [verification.json](verification.json)、[认知验证](cognitive-prefetch/verification.json)、[计时验证](dispatch-timing/verification.txt)。

候选源码可在仓库根目录复跑：

```powershell
$env:UNDERSTAND_BOOK_SOURCE_REUSE_REPORT = Join-Path (Get-Location) 'tmp/source-reuse-repeat.json'
pnpm --filter @understand-book/core exec vitest run test/automatic-build-source-reuse-performance.test.ts --maxWorkers=1 --minWorkers=1

node --import tsx docs/performance/prebuild-first-batch-20261023/cognitive-prefetch/benchmark.ts candidate tmp/cognitive-repeat.json
node --import tsx docs/performance/prebuild-first-batch-20261023/dispatch-timing/measure.ts candidate-repeat

$env:UNDERSTAND_BOOK_B1_REPORT = Join-Path (Get-Location) 'tmp/control-plane-repeat.json'
pnpm --filter @understand-book/core exec vitest run test/automatic-build-control-plane-performance.test.ts --maxWorkers=1 --minWorkers=1
```

脚本标签不自动切换源码版本；认知 candidate 模式还会复用保存的基线正式对象回执并比较最终材料。改前数据必须用已保存的改前源码执行。性能命令依次运行，不与其他重负载测试并行；首轮为已有文件缓存状态下的首个观测，不声称是清空操作系统缓存的冷启动。

## 5. 限制与已知问题

- 本批性能回放使用固定合法候选，没有调用真实模型，未测 provider 延迟、token 计费、真实 child 启动或整书完成时间。输入统计为实际 UTF-8 字节，不是 token 估算。认知输出相同证明固定候选路径等价，不能替代真实模型质量对照。
- DSH 当前安装宿主的端到端测试未通过：原测试配置的 `both` 工具模式被拒绝；临时改用 `native` 后，既有能力检查仍报告缺少 `subagents` 和 `spawn`。临时测试配置已恢复，生产能力检查未修改。DSH 本批只确认本地实际 drive 生命周期和类型检查，未宣称安装宿主验证通过。失败日志在 [DSH 测试日志](dispatch-timing/dsh-tests.log) 与 [能力检查](dispatch-timing/dsh-capability.log)。
- 计时起点是宿主观察到可用容量的时刻，无法还原真实 child 退出到宿主观察之间的时间；终点也不是模型服务首 token。旧任务及缺少相应观察的调用保留缺失，后续不能把低覆盖率解释为零等待。
- 完整区间归入现有公共任务的 attempt 和阶段汇总；私有控制产物的生成分支尚未纳入这一汇总。因此 7/7 代表该公共任务夹具的覆盖率，不代表全部预构建分支均有完整计时。
- 首次认知候选回放因前置正式对象随机 UUID 不同而中止。随后候选复用基线实际已接受的正式对象回执，在保持完整深比较的前提下取得有效结果。该次失败单独保留，未纳入性能样本。
- 首次计时候选使用请求入口的协议时钟，遗漏 generation.start 函数内部准备时间；审核后改为 GENERATE 持久化就绪时记录。早期结果保存在 [candidate-initial.json](dispatch-timing/candidate-initial.json)，最终结果重新采集。

架构决策同步至 [ADR-0121](../../adr/0121-bounded-build-state-reads-and-executor-lifecycle.md)。
