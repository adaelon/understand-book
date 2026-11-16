# ADR-0121 有界构建状态读取与 Executor 生命周期

Status: Accepted design, 2026-09-07；D0/B1、L1/L2、P1–P4、V1 实现与候选验收完成（2026-09-08）。P1–P4 的 180 项回归与同规模测量见[有界读取验收](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/understand-book-p1-p4-control-plane.md)；compiled/隔离安装、两轮六单元真实宿主、恢复与计时见 [V1 验收](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/understand-book-v1-release.md)。旧程序宿主 A/B、模型 child 故障注入及正式安装/真实书续跑未验证。
Extends: ADR-0116 的测量与传输优化、ADR-0117 的恢复代际与结果复用。
Revises: 日常状态计算中的准备副作用、V4 单工作单元完成后的 Executor 交接约束；不改变语义结果身份。

B1 的代表性夹具、四轮样本、实际源码快照和验收记录见[可复跑控制面基线](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/understand-book-b1-control-plane.md)。

L1/L2 的六单元持久提交、连接拒绝恢复闭环、发布投影与 compiled stdio 证据见[生命周期验收](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/understand-book-l1-l2-lifecycle.md)。

Mastering Rust 的真实运行已使用四次 MCP 调用，但三条成功任务的工具耗时平均仍为 187.58 秒。真实状态副本的暖快照耗时 17.51 秒，并产生 6,315 次针对已有文件的失败创建；原任务的三槽平均占用率约 38.36%。选择先缩小重复状态工作并修复执行器交接，而不是继续放大 transport batch 或恢复长期 worker。事实与限制见[基线](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/understand-book-20260907-control-plane-baseline.json)，实施见[切片方案](../切片方案-预构建控制面有界读取与执行器补位.md)。

## §1 准备与读取的职责

**决策**:阶段准备写状态，日常快照只读。

**否决**:

- 每个 work unit 重复冻结整个 policy set：已存在的阶段常量反复触发创建、异常和读取。
- 在快照中隐式接管旧产物：读取成本和状态变化无法独立归因。
- 删除迁移校验：会损害既有成果复用和冲突处理。

**命门**:沿用现有策略、迁移记录与授权；初始化、缺失恢复、授权迁移由准备路径执行，正常读取发现未准备状态时返回既有结构化恢复边界。
**展开**:[P1/P2](../切片方案-预构建控制面有界读取与执行器补位.md#p1-阶段策略与迁移登记去重)。

## §2 驱动器状态复用

**决策**:复用稳定路由，实时刷新任务状态。

2026-10-20 修订：标准技术书在 Pass1 已关闭、画像阶段运行时，Driver 按 invocation 保存派生任务目录。
后续补位复用 Pass1 与任务描述，通过当前代次产物目录发现新增、删除的结果，只读取变化任务；
分片变化只重路由所属段落及其归约依赖。源文、代码策略、策略文件、迁移目录或发布结果变化时重新路由。
归约新任务的准备仍经过 Driver 授权门；最后一个画像任务完成后回到完整阶段收口路径。

2026-10-21 修订：派生目录按记录读写，任务描述符在内存中共享；旧目录格式首次使用时重新派生。
工作单元摘要逐项输入原摘要算法，同一次预检复用计算结果，保持既有规范化字节和任务身份。
画像任务目录在同一个原子写入中保存既有计划摘要；后续补位直接复用，普通完成、删除或租约变化不重算。
段落重路由后直接比较任务描述，内容变化才清除摘要并重算；来源、策略与发布失效仍走原路由。
旧目录缺少摘要时从已有描述补齐，不新建 BuildPlan 或 invocation。摘要单独存放在目录头，旧引擎重写目录会丢弃该提示。
性能和恢复证据见[计划摘要复用测量](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/automatic-build-digest-reuse-20261021.md)。

2026-10-24 修订：同一次 Driver 决策内，plan、doctor、next 与派发重放复用 `loadDriverState` 已验证的来源 target。
每个新请求及准备、写入、恢复后的状态刷新仍重新解析来源；不把该 target 保存为跨请求缓存。
来源变化仍触发现有基础恢复或计划变更边界，恢复原文后仍可继续原有 handoff。

**否决**:

- 将整份可变状态作为跨 `build.step` 的权威：child 可能已提交，不能遮蔽当前持久事实。
- 同一次无写入判断反复规划：重复计算不增加可见事实。
- 缓存租约与提交权威：会放行过期或已完成的执行。

**命门**:派生目录可重建，不保存租约、尝试或预算决定；claim、submit 和恢复代际检查仍读取实时记录。
目录观察先于增量读取，计算期间到达的 child 提交留给下一 step，不能提前登记为已经读取。
**展开**:[P3](../切片方案-预构建控制面有界读取与执行器补位.md#p3-driver-单次决策段复用)。

## §3 单任务操作的读取范围

**决策**:Executor 校验当前任务及其依赖。

**否决**:

- 为领取 frozen batch 重建全书：无关工作单元放大每次协议调用成本。
- 只信 opaque ref 或 frozen bytes：不能发现 source、policy、owner 和租约变化。
- 在 MCP 中隐式重规划：扩大了 Executor 的职责和输入边界。

**命门**:复用既有持久 task/descriptor、policy、dispatch、input 与 receipt；完整保留当前身份、依赖和输出合同检查，需重规划时交还 Driver。
**展开**:[P4](../切片方案-预构建控制面有界读取与执行器补位.md#p4-executor-单任务读取)。

2026-10-24 修订：新认知材料任务首包直接交付必读目标原文，最多 2,000 个 UTF-16 单位，
并受剩余读取量、上下文和完整输入预算共同约束。只有最终 packet 实际包含的范围才进入阅读账本。
已冻结的旧首包及已接受步骤保留原输入和任务身份；跨段取证、范围引用与缺口搜索要求不变。

## §4 调度槽与执行器交接

**决策**:复用调度槽，每个新引用创建新执行器。

2026-10-21 修订：MCP 服务完整发出 DONE 或终态错误后关闭输入、输出连接并退出，
不随宿主保留已结束 child 而继续驻留。尚未绑定时的错误 ref 保留连接，支持既有 Root 引导更正。

**否决**:

- 给 terminal child 发送新引用：其连接已绑定旧引用并进入终态。
- 为提速放开 terminal 连接重绑：改变单引用语义上下文生命周期。
- 等整批 child 全结束才补位：让已空闲且有工作可做的槽位持续等待。

**命门**:V4 一个 work unit 提交后结束该 child；Root 按 slot 占位、按 ref 去重，消费已到达的终态后重算容量并用新 child 补位。
**展开**:[L1](../切片方案-预构建控制面有界读取与执行器补位.md#l1-新引用新执行器与即时补位)。

## §5 打开失败的恢复

**决策**:连接错误准确分类，恢复只推进控制代际。

**否决**:

- 把 terminal/ref mismatch 映射为版本不兼容：会误导安装修复。
- 把连接拒绝伪装成语义失败：尚未执行模型任务。
- 原样重复派发已终止引用：与 Root 去重合同冲突。

**命门**:只接收 allowlisted、open 阶段的有界观察；Driver 核验所属 invocation、仍 current 且未 open 后幂等换代，沿用三次暂停与显式重试，已 open/已提交事实优先。
**展开**:[L2](../切片方案-预构建控制面有界读取与执行器补位.md#l2-连接诊断与有界恢复闭环)。

## §6 性能验收口径

**决策**:用当前调度粒度验收端到端收益。

**否决**:

- 把旧 bundle 基线的启动占比用于 V4：一个 child 完成的工作量已不同。
- 以文件调用数或 MCP 次数代替墙钟：减少次数不自动等于更快完成。
- 以更多 attempt 换更短时间：会把重复语义工作藏进性能指标。

**命门**:同模型、同宿主、同程序、同输入与起始状态对照；同时记录 snapshot/step/MCP 耗时、文件操作、slot 区间、attempt 与 durable commits，结果质量和复用集合不退化。
**展开**:[B1/V1](../切片方案-预构建控制面有界读取与执行器补位.md#b1-代表性基线与最小计时夹具)。

2026-10-24 修订：Codex refill 和 DSH 宿主在消费终态、释放 child 占位后记录可用容量，
关联到后续实际派发的 handoff，再由首次 `generation.start` 在 GENERATE 结果持久化就绪后记录终点。
首次容量与终态补位分别计数；多工作单元 handoff 只消费一次容量观察，重放不重置时间，旧数据缺失不填零。
分项记录容量观察到 ready、本轮调度、ready 到 open、open 到 GENERATE 就绪；该端点不代表模型首 token。
观测落盘不参与租约、调度或恢复决定。基线、候选、回归与开销见[首批预构建提速测量](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/prebuild-first-batch-20261023/README.md)。
