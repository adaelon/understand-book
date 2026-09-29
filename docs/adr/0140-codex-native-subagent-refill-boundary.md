# ADR-0140：Codex 原生子代理自动补位边界

日期：2026-09-27。状态：子代理边界已确认；预规划进度第一切片已实现并验证；Codex 程序自动补位尚未实现，受当前宿主接口限制。

继承 [ADR-0101 §2](0101-deterministic-prebuild-protocol-ownership-and-codex-semantic-boundary.md#2-codex-外部动作面) 的调度权限边界和 [ADR-0138](0138-multi-harness-prebuild-and-deepseek-harness-adapter.md) 的构建宿主定义。

## 决策

**决策**：构建执行器使用同一父任务的原生子代理。
程序补位负责消费终止事件、释放对应槽位、调用 `build.step`、派发下一名子代理，并保持最多 3 名执行器。Build Engine 继续决定可执行工作、预算、接纳与完成；调度程序不生成语义内容。

**约束**：新 handoff 使用新子代理和新连接。
保留现有单 handoff 执行器合同；只释放终止事件实际对应的槽位。`NEEDS_USER`、取消和失败继续遵循现有宿主与引擎边界。

**否决**：独立会话替代原生子代理。
外部 `codex exec` 或 App Server `thread/start` 会话不满足本次父子归属要求；补写父任务标签或使用 `thread/fork` 也不作为原生创建能力的证明。

**何时推进**：宿主公开可调用的子代理生命周期接口。
该接口必须能从程序创建属于当前父任务的子代理，并提供终止与取消能力。满足后再接入控制循环；能力未落实时不新增空适配层。

## 问题证据

原始构建任务 `01a0e213-8c80-75f2-84fa-d537d14de1c7` 在 2026-09-27 17:41:23（UTC+8）收到 `build_exec_7` 的完成消息，等待工具同次返回。主 Agent 到 17:46:39 才完成本地槽位释放，到 17:52:37 才实际开始下一次 `build.step`；该命令约 52 秒后返回 `SPAWN_EXECUTORS`。

这次空槽位等待发生在主 Agent 接收完成信号之后、调用引擎之前。完成消息已经自动送达；引擎在被调用后能够补位。该证据不能进一步区分模型推理、服务排队或其他宿主内部延迟。

当前 [构建技能](../../skills/build/SKILL.md) 已要求首个执行器终止后立即释放、重算容量并调用 `build.step`。增加相同提示词不构成程序自动补位。

## Understand Anything 参考结果

核查的是本机安装所指向的源码提交 `2cda14e89535049e49120198886bc0b82e9e630f`；以下结论限定于该版本。

- [Phase 2](https://github.com/Egonex-AI/Understand-Anything/blob/2cda14e89535049e49120198886bc0b82e9e630f/understand-anything-plugin/skills/understand/SKILL.md#L295-L343) 要求主 Agent 遍历批次、每批派发 file-analyzer，最多并发 5 名。批次完成后由主 Agent 检查输出，全部完成后调用合并脚本。这是技能编排要求，没有程序拥有的子代理补位循环。
- [file-analyzer](https://github.com/Egonex-AI/Understand-Anything/blob/2cda14e89535049e49120198886bc0b82e9e630f/understand-anything-plugin/agents/file-analyzer.md#L13) 明确禁止继续创建子代理，不采用子代理自行启动下一名的接力方式。
- [compute-batches](https://github.com/Egonex-AI/Understand-Anything/blob/2cda14e89535049e49120198886bc0b82e9e630f/understand-anything-plugin/skills/understand/compute-batches.mjs#L311-L378) 将符合条件、少于 3 个文件的小批次合并成每批最多 25 个文件，并预先准备批次输入。这减少派发开销，但不保证完成后的派发时延。
- [基准脚本的并发循环](https://github.com/Egonex-AI/Understand-Anything/blob/2cda14e89535049e49120198886bc0b82e9e630f/scripts/lib/large-repo-benchmark.mjs#L1207-L1226) 自动领取下一批；其实际 worker 调用静态结构解析脚本，[基准范围](https://github.com/Egonex-AI/Understand-Anything/blob/2cda14e89535049e49120198886bc0b82e9e630f/docs/benchmarks/large-monorepo.md#L26) 明确不运行 LLM。因此不能将该循环视为 Codex 子代理创建接口。

可参考确定性准备输入、减少重复上下文与派发开销。Understand Book 的 handoff、work unit 与接纳身份已有合同；本决策不因参考项目使用文件批次而合并这些身份。

## 实现依据与已知限制

本项目 [DSH 控制循环](../../packages/dsh-plugin/src/build-control.ts) 已具备 `terminal → consume → step → launch` 的结构；[执行器宿主](../../packages/dsh-plugin/src/executor-host.ts) 实际通过 `ctx.subagents.start` 传入父 Agent。它证明该设计在具备宿主接口时可行，不证明 Codex 已开放相同接口。

本次核查的 Codex 桌面内置 CLI 为 `0.158.0-alpha.2`。其 `app-server generate-ts --experimental` 导出的客户端请求包含 thread 与 turn 操作，没有供外部程序调用的原生子代理创建请求；当前聊天的 `collaboration` 工具也不能从 `functions.exec` 内调用。这是对当前已验证接口的结论，不断言其他版本永远不支持。

后续验收必须包含真实宿主创建与终止事件：一个执行器结束而另外两个仍运行时，控制程序应在自身下一次循环内派发新子代理；全程维持原生父子归属、最多 3 名执行器且不重复派发。仅有假宿主单测、技能文本断言或独立会话测试不足以证明该目标已实现。

## 追加：预规划与构建进度（第一切片已实现）

预先计算工作、展示剩余量与结束条件不依赖程序创建子代理的能力，应与宿主自动补位分开推进。主 Agent 可以继续派发原生子代理，工作选择与进度统计仍由引擎提供。

当前 [preflight](../../packages/core/src/automatic-build-budget.ts) 已包含当前阶段的总工作量、待办、已提交、剩余派发批次，以及基于历史样本的耗时估计；[派发规划器](../../packages/core/src/automatic-build-dispatch.ts) 先计算待办的完整批次清单，再按空闲槽位选择本次可派发项。第一切片通过 [进度投影](../../packages/core/src/automatic-build-progress.ts) 向 [Driver 响应](../../skills/build/automatic-build-driver.ts) 增加 `build_progress`，保留原有 BookStructure 局部进度。

全局阶段范围由已确认 BuildPlan 决定。当前 [阶段快照](../../packages/core/src/build-orchestrator.ts) 在前序阶段未完成时停止展开后续阶段；依赖已接纳输出的归并任务也需在输入就绪后确定。当前已发现的任务总数不能当作整本固定总数；已知未来阶段不能显示为零任务。

整体进度设计保持以下语义：

- 启动时展示完整阶段路线、目标成果与完成条件，以及当前可确定的任务和批次总量。
- 运行时分别显示已接纳、运行中、待派发和重试；重试增加尝试次数，不冒充新增目标工作。任务数与子代理启动次数分别统计。
- 对尚未展开的阶段标明依赖与估计状态；能可靠估计时提供范围，在依赖就绪后替换为实际数量并说明变化原因。固定阶段进度与动态任务数量分开显示。
- 结束以计划要求的成果发布和引擎 `DONE` 为准。剩余时间需计入执行、主 Agent 派发间隔、收尾和重试；当前 preflight 的任务服务耗时估计不直接冒充全程 ETA。缺少样本时明确标为尚未校准，不能给出确定完成时刻。

第一切片已透传完整阶段路线、已发现工作量、剩余规划批组、私有成果数量和当前阶段服务耗时的校准状态。四种正常控制结果均携带投影，投影复用本次快照，不额外读取或规划。无匹配样本时不输出内部五分钟默认值；有匹配样本时仅输出当前阶段服务耗时。规划批组不是原生子代理启动次数，待办包含运行中但未提交的任务；技能据此解释进度，不从这些数量推断重试次数。

待续切片为依赖阶段估算和包含派发间隔的实测剩余时间。当前代码未提供整本 ETA、全书固定任务总数或逐任务运行/重试统计。这些工作不以自动补位接口开放为前提。

验证：进度投影、Driver 及请求失败回归合计 61 项通过；技能源与发布副本一致性 1 项通过；Core 类型检查、两份技能格式校验通过。先验证旧 Driver 缺少 `build_progress` 的失败，再接通投影。原生执行器协议的六任务三槽位用例验证提交后已完成数递增、待办减少；私有成果用例验证最终 `DONE`。

候选 Engine 已编译，并通过独立临时样例实际调用打包程序的 `build.step` 验证新字段。尚未替换本机安装版，也未改变正在运行的构建会话。

## 追加：一次完成登记与补位请求

用户确认将“登记完成、释放槽位、计算容量、申请下一任务”合成一次操作。采用代码入口 `<build-exe> build.refill`，通过现有执行工具一次提交 `automatic_build_refill_request.v1`；不将计算逻辑放在主 Agent 临时编写的多步模板里。

[automatic-build-refill.ts](../../skills/build/automatic-build-refill.ts) 接收 invocation、总容量上限、原有 `live_by_slot`、`completed_refs` 和本次 `terminal_children`。按 child 身份移除对应所有权，将该 ref 加入已终止集合，再以 `min(宿主容量, invocation.max_parallel) - liveCount` 调用一次既有 `build.step`。响应带回更新的记录、实际空位、原始 step 和经过所有权过滤的 `ready_executors`。登记子代理终止只改变宿主容量；任务提交、重试和完成仍由 Engine 读取持久回执确定。

主 Agent 收到 C 的终止通知后，下一次控制操作即调用 refill，中间不调用 `list_agents`。A、B 保留；内部 step 收到一个空位，返回 D 的引用后，主 Agent 用原生 spawn 创建 D。若 B 在调用期间终止，先用返回记录为 B 再执行一次 refill，再按最新可启动列表派发；未启动的 D 由 Engine 重投影。重复的 C 通知不能移除占据旧槽位的新 child D。失败观察沿用既有队列和请求合同，每次最多传一条。

验收由真实 Driver 与执行器连接协议覆盖六工作单元、三槽位场景，第一位终止的是 C：一次 refill 返回空位 1 和新引用，原有 A、B 的连接对象保持不变。单元测试另覆盖只调用一次 step、重复通知、调用间新增终止、容量限制和排队观察透传。CLI 接入既有 sidecar，不新增 MCP 服务。

### 限制

refill 仍需主 Agent 在收到通知后调用，不保证宿主调度时延，也不在程序内部创建原生子代理。`build.refill` 是打包 Engine 的命令入口，调用它使用现有执行工具；原生 spawn 是后续宿主工具调用。

2026-09-27 经用户授权已替换 `E:\allwork\Understand Book\understand-book-build.exe`，安装版通过独立临时样例的 `build.refill` 实测。旧程序保留为同目录 `understand-book-build.pre-refill-20260927-231511.exe`，已有执行器进程继续运行。插件同名市场来源已切换至当前本地项目，经桌面 CLI 重装为 `0.1.0+codex.20260927151540`；已安装构建技能与发布源逐字节一致。新聊天加载新技能，已有聊天不假定技能上下文自动更新。安装记录见 `tmp/codex-build-refill/installed.json`。
