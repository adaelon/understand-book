# 持续执行器真实书籍记录

2026-10-29 最新执行方式：用户要求不重跑对照，直接让[用 understand-book 构建深度学习书](codex://threads/01a182f6-204a-79e2-95d7-10b5b1cc69dd)使用新版接续原进度。最终程序于香港时间15:29:01安装，原invocation、计划、质量档、Pass2关闭与并发上限8继续沿用。该聊天继续拥有构建，本性能任务只观察无正文的时间和生命周期记录。

安装前15:28:10观察：主控结构进度181/732；24个已注册worker中6个已经完成多批任务，累计完成26份assignment。说明该聊天此前已使用最初安装的跨批次版本；15:29的最终更新主要补齐并发settled恢复、工具内有限等待和WAIT上下文计费，不能把之前所有任务标为旧批次路径。原始观察见[d2l-before-final-activation.json](d2l-before-final-activation.json)，安装证据见[activation.json](activation.json)。

目标聊天已确认使用新版继续原构建。首个后续观察为香港时间15:35:15，记录见[d2l-after-final-activation.json](d2l-after-final-activation.json)：

| 指标 | 15:28:10 | 15:35:15 | 变化 |
| --- | ---: | ---: | ---: |
| 主控结构已验收投影 | 181/732 | 186/732 | +5 |
| 完成过多批任务的worker | 6 | 8 | +2 |
| worker累计完成assignment | 26 | 33 | +7 |
| 已记录成功提交尝试 | 182 | 188 | +6 |
| 已记录可重试失败 | 83 | 85 | +2 |

观察窗口为425.087秒，主控投影平均新增约0.706项/分钟。成功提交记录与主控投影刷新时点不同，不能把两种计数互换；worker assignment也不等于所有阶段通用的语义工作单元。窗口包含旧连接继续执行、最终程序激活和重试，仅证明真实续建持续推进及复用正在发生，不能用于宣称最终补丁的稳定提速倍数。后续由原聊天按同一阶段实际记录继续观察。

下面保留最初的隔离实验设计与原始记录。该实验运行到94项已接纳后被中断，未完成BookStructure关闭；candidate-run从未启动。按用户最新要求不重新开始，也不据此报告整阶段加速倍数。

## 原隔离实验设计

本实验比较同一本真实书籍的完整 BookStructure 阶段。两次都复用同一份已通过质量门的全书 Pass1 与 profile sidecar，从没有任何 BookStructure 成果的状态开始，重新生成全书结构直到 Engine `DONE`。保持 `quality_profile=full`、并发上限 8、Pass2 关闭，以及相同专用执行器角色与模型设置。正式学习对象、认知素材、教学发布不属于这次测量；原书工作区不被修改。

## 比较对象

| 项目 | 基线 | 候选 |
| --- | --- | --- |
| 主控程序 | 修改前冻结源码编译的 `baseline-engine.exe` | 本次完成后的候选程序 |
| 执行器会话 | 未注册为常驻 worker 的原始 handoff，原有每批退出路径 | 注册为常驻 worker 的 handoff，跨批次领取 |
| 执行器 MCP 程序 | 同一个候选程序中未修改的原始 session 路径 | 同一程序中的 worker 包装与续领路径 |
| 输入与已有成果 | 同源的整本书及已接纳 Pass1/profile | 逐字节一致的副本 |
| 并发与质量 | `8`、`full` | `8`、`full` |

采用同一执行器 MCP 程序，可以避免每次更换共享安装时影响其他正在构建的书。候选的 worker 包装在未发现 worker 注册记录时直接委托原始 session 命令；原来的语义输入渲染、候选校验、writer 与质量门没有改变。因此这是一项**旧批次生命周期与新常驻生命周期的受控路径比较**，不是两套完全独立安装的整体发行版比较。

冻结源码位于 `tmp/continuous-workers-20261029/baseline-source`，包含开始实施前已有的未提交修改，不能用 Git HEAD 代替。基线可执行文件位于 `tmp/continuous-workers-20261029/benchmark/baseline-engine.exe`，由该源码入口打包。

## 输入范围与计划

来源为 `.understand-book/logic-of-failure`。复制操作只读原工作区，目标是测量目录内新建的书籍副本。复制清单由 [准备脚本](real-book-benchmark.mjs) 中 `foundation` 管理：源文件、已发布 Pass1/profile 文件、对应已接纳 artifacts、shadow tasks、质量报告、policy 与 publication/close receipts。已有的 `relocateGenerationTask` 与 `sameBuildContent` 负责移动后绑定，测量脚本不改写语义成果。

不复制 BookStructure 已发布文件、tasks、artifacts、policies、close/quality/publication receipts、输入预计算缓存、章节或主题 continuation，也不复制教学阶段。当前冻结版路由在空 BookStructure 状态下首先生成一个 outline 任务，再根据被接纳结果动态生成章节与主题工作。

两份计划均由代码的 `prepareExplicitLegacyBuildPlan(..., { pass2: 'disabled', build_tutor: false })` 签发，投影如下：

- 完整闭包：`pass1 → profile_sidecar → book_structure`。
- 复用：全书 Pass1（35 个已接纳任务）与 profile sidecar（187 个已接纳任务）。
- 新建：仅 `public.book_structure`。
- 不预先承诺 token/墙钟估计；计划在运行时按已有引擎质量和预算规则推进。

已准备的控制文件：

- `tmp/continuous-workers-20261029/benchmark/baseline-run/control-state.json`
- `tmp/continuous-workers-20261029/benchmark/candidate-run/control-state.json`

`baseline` 目录是准备脚本首次漏复制质量报告时留下的失败准备记录，没有创建执行 invocation 或启动语义生成；它不属于测量结果。正式两次运行以 `*-run` 目录为准。

## 执行

在候选实现与相关测试完成后执行。八个并发槽应由专用执行器使用，先结束实施代理；其他聊天的正常构建若争抢模型资源，在结果中记录。不得使用固定候选、历史候选重放或模拟模型替代真实语义调用。

首次准备已经完成，恢复时不要重复创建计划或 invocation。候选程序编译好后，只切换候选测量控制文件中的可执行路径：

```powershell
& 'D:/fate/node/node.exe' --import tsx docs/performance/continuous-workers-20261029/real-book-benchmark.mjs engine --run tmp/continuous-workers-20261029/benchmark/candidate-run --engine <candidate-engine.exe>
```

每次运行的首次控制调用：

```powershell
& 'D:/fate/node/node.exe' --import tsx docs/performance/continuous-workers-20261029/real-book-benchmark.mjs step --run <run-directory>
```

Root 仅将 `SPAWN_EXECUTORS` 返回的完整 opaque ref 交给专用 `understand_book_executor` 子代理。返回 `dispatch_slot_ref` 只保存在 Root 的 ownership map 中。将实际启动结果写成一个数组文件，每项仅含 `{ dispatch_slot_ref, child, opaque_handoff_ref }`，随后执行 `spawned --run <run-directory> --launches <array-file>`，记录真实 child starts。首次 BookStructure launch 后执行 `prepare-inputs`；如返回 `outline_pending`，按技能在下一次有工作可启动时再准备一次。

子代理结束时，写入 `{ terminal_children: [<owned-child>] }` 请求文件，使用 `refill --run <run-directory> --request <request-file>`。脚本把现有 ownership map 和 completed refs 一并交给唯一 `build.refill` 调用，保存返回状态，并把完整控制结果落入 `control-events.jsonl`。Root 继续即时启动 ready executors；不等待全部 child 才补位。候选阶段无任务可领取的 worker 可以保持 WAIT；只有引擎返回的终态、上下文轮换或实际错误才释放它。

保持两个运行在同样模型和同样角色下完成，不引入手动等待作为基线负担。运行中遇到候选质量拒绝仍使用原有纠错/重试路径并计入耗时；不删掉慢样本或失败记录。

## 指标与判定

运行结束后：

```powershell
& 'D:/fate/node/node.exe' --import tsx docs/performance/continuous-workers-20261029/collect-real-book-metrics.mjs <run-directory>
```

[指标脚本](collect-real-book-metrics.mjs) 只读取引擎的时间、结果、计量与质量回执，不读取任务输入或 candidate 内容。指标包括：

- 主要指标：首个 `generation.start`（执行器已确认全部输入）到 Engine `DONE` 的完整阶段时间；另报首次 open 到 DONE，保留输入传输影响。
- 动态接续：同一 chapter 上一步引擎记录的提交时刻到下一步 generation start 的间隔，报告样本数、p50、p95、平均值。提交记录采用请求入口时间，因此间隔包含提交内部校验、写入、续领和下一项输入传输，不能解释为纯调度空等。
- 工作量与运行开销：成功任务数、物理尝试数、重复尝试数、状态分布、实际子代理启动数、主控持有的子代理峰值。worker 分别统计领取过多批任务和完成过多批任务，后者才证明跨批次完成。
- 本步生成时间：输入确认到提交请求进入引擎，不含随后提交内部处理；与完整阶段时间分别报告。
- 模型与 token：回执有值时报告；当前可能只有 `unavailable`，不能将已知 token 小计冒充总消耗。
- 正确性：必须同时达到 Engine `DONE` 且 `readAcceptedBookStructureClose(..., 'full')` 成功，才记为完整成功运行。

真实模型可能生成不同数量的动态步骤，报告须并列成功任务量与重试量，并结合接续间隔判断调度收益。一次两路实测不能给出稳定统计置信区间；如果总时间被模型调用波动或重试差异支配，不能仅凭总时长声称显著加速。

本实验能支持“同一本实际书籍的完整全书结构阶段”结论，不外推为 Pass1、profile、正式知识对象和教学材料全部运行的整本预构建加速倍数。历史约 95 分钟包含停顿/修复，保留作问题背景，不用作对照分母。

## 当前结果

两份副本的6,163个prerequisite文件、23,963,634字节已直接逐字节比较一致。旧路径对照记录保留在baseline-run：58个发现任务全部完成，随后章节链继续展开，累计94项已接纳；未达到Engine DONE。candidate-run只有准备记录，没有语义执行。它们提供旧调度行为和恢复诊断，不构成已完成的两路性能比较。

基线前段与本机 Core 回归测试存在重叠：Driver/refill/adapter 自香港时间13:02:13运行约585.91秒，跨派发专项测试约13:12–13:14运行。后续基线及候选测速期间不再运行重型回归。完整阶段时间需要保留这一负载混杂因素；后段任务的接续间隔和原生启动次数用于独立判断调度收益。
