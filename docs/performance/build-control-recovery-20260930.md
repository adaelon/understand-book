# AI Infra Book 构建控制记录恢复

2026-09-30，工作区：`E:\allwork\download\agent\lifebook\.understand-book\ai-infra-book-complete`。

## 故障与保留成果

旧构建调用失败于读取 invocation 控制记录：本地诊断为 `ENOENT`，调用链是 `readJsonRecord → readInvocation → automaticBuildStep`。控制记录默认位于系统临时目录。临时注册目录在 10:39:36 重建，10:49:29 读取旧调用记录失败。清除该目录的具体进程尚未确定。

工作区保留了已接纳成果。恢复前通过引擎的只读阶段投影确认：Pass1 198/198，profile_sidecar 1323/1323，BookStructure 470/530；前两阶段已关闭，BookStructure 待完成和发布。目录数量不作为已接纳成果数量。

任务总数表示当时已发现的工作单元。续建过程中后续任务继续展开，总数已从 530 增至 595；不将当时的剩余数当作固定最终范围。

用户已确认原计划：标准深读、排除 Pass2、并发 3、无额外 token 上限。恢复复用同一计划，没有重新定义阅读范围。原始启动请求中的精确创建时间未留存，因此建立新调用；新调用的完整创建请求和控制引用另存于仓库 `.codex-recovery`，不包含书籍语义正文或候选输出。

## 修复与安装

默认控制目录改为 `C:\Users\Lenovo\.understand-book\automatic-build-driver-v1`。Driver 使用 executor session 的同一目录解析入口。显式 `UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT` 覆盖继续生效。

调用记录缺失时，step/refill 返回有界的 `invocation_record_missing` 诊断，明确可通过原创建请求或同一已确认计划重建调用并续用工作区。其他内部错误继续使用原诊断路径。

从已提交代码隔离打包本次两处修复。首次快照导出改变了发布提示文件的换行格式，发布校验拒绝该包；立即回滚原程序继续构建。重新打包保留原 Windows 文件格式后，协议 doctor 的八项检查全部 compatible。

新程序已安装至 `E:\allwork\Understand Book\understand-book-build.exe`，原程序保留为同目录的 `understand-book-build.before-persistent-registry-20260930.exe`。同一恢复调用已在持久目录内创建成功，正式 refill 正常返回三个执行器。

## 验证

- 新增回归测试在修复前失败、修复后通过：系统临时目录变化仍能访问 Driver 写入的记录；step/refill 对缺失调用记录给出明确恢复提示。
- 故障、补位、Driver 和 executor session 四组回归合计 109 项通过。
- Core 类型检查通过。
- 打包程序实际将诊断写入用户持久目录，并通过全部安装兼容性检查。
- 工作区在 482/530 项时完成安装切换，剩余构建继续按原计划、并发 3 执行。

## 已知问题

尚不能确认本次是谁清除了旧临时目录。构建完成必须以引擎返回 `DONE` 为准；本记录写入时仍在续建。

## 进度分母反复增加的根因

2026-09-30 用户授权直接核对全部相关文件后，对照实际安装包的隔离源码快照、任务租约、提交回执和已接纳产物确认：BookStructure 的分母统计的是已经展开的工作单元，不是全书最终工作量。`automatic-build-progress.ts` 将 `state.work_units.length` 直接投影为 `total`，并标记 `scope=discovered`。

章节归并在上一层全部产物就绪后，才按实际输出大小生成下一层任务。每组最多接收 8 个子产物，同时执行程序内的 6000 token 正文预算；超出时进一步缩小分组。此内部单任务路由预算与用户未设置额外总 token 预算是两个层面。

最近两次增加有明确的一次执行记录：第 14 单元的 29 个片段完成后，产生 9 个第一层归并任务，分母 581→590；这 9 项完成后，产生 5 个第二层归并任务，分母 590→595。已检查的这些任务 attempt 均为 1。它们的身份分别包含 `fragment`、`reduce:001`、`reduce:002`，不是同一任务反复重试。

检查时 14 个单元中有 13 个最终单元产物；第 14 单元已完成 29/29 个片段、9/9 个第一层归并及 3/5 个第二层归并。引擎投影为 593/595。全书局部拼接、关系选择、关系整合均尚未开始，publication=pending。因此 593/595 不能解释为整本构建接近 100%。此前将已展开任务的剩余数说成“最后几项”的沟通不准确。

上一段长等待对应 `unit:14:fragment:0028`：生成启动完成于香港时间 17:01:44，提交完成于 17:20:08，间隔约 18 分 24 秒。回执确认第一次尝试成功提交；仅凭完成时间无法拆分生成与提交的各自耗时。

后续应按“已完成单元数／单元总数、当前归并层、全书拼接与关系整合是否开始、发布状态”说明进度，同时把任务分母明确称为已展开任务数。最终任务总量要等后续依赖和关系选择产物确定。

## 全书拼接启动生成超时（2026-09-30）

14 个单元的最终结果已经完成，引擎展开 5 项全书局部拼接。最后已确认的投影为 BookStructure 601/606、local 0/5、selection 0/0、relation 0/0、publication=pending。执行器 352、353、354 对应 `stitch:fragment:0000`、`0001`、`0002`，三者都成功完成 open 和 input.next，随后 generation.start 各重试三次，九次均被客户端以 `timed out awaiting tools/call after 120s` 终止等待；没有 candidate 提交。

实际服务端已完成第一次启动生成，并写出 GENERATE 记录。按请求进入函数时保存的 accepted_at 与 generation-start 记录文件实际写入时间比较（香港时间）：

| 拼接任务 | 进入启动生成 | 写出 GENERATE 记录 | 经过时间 |
| --- | --- | --- | --- |
| 0000 | 18:38:43.551 | 18:41:05.378 | 141.827 秒 |
| 0001 | 18:39:31.141 | 18:42:02.314 | 151.173 秒 |
| 0002 | 18:39:56.030 | 18:42:16.742 | 140.712 秒 |

这些是持久化完成时间，不是模型生成耗时。客户端的 120 秒期限比服务端完成时间早 21–31 秒，因而丢失了成功的启动回复。

实际安装版本的调用链显示，同一次 generation.start 会至少三次解析当前任务描述：`resolveDeliverySessionContext → taskDescriptor`，`materialFromFrozenGenerationInput → renderDeliveryMaterial → taskDescriptor`，以及 `startAutomaticBuildExecutorGeneration → taskDescriptor`。BookStructure 的 taskDescriptor 通过 readAutomaticBuildTaskStage 重建任务投影；普通单元按 parent_lid 限定范围，而 parent 为 stitch 时不限定单元，重复重建整本书的章节路由及拼接路由。已保存的 generation-start 重放回复也要经过这些读取和重建，成功记录并未让重试成为快速路径。三路并发下，启动调用越过 120 秒上限。

另有两项恢复链路问题。执行器 352 在已经成功 open 和 input.next 后误报 bootstrap_unavailable，违反零次成功调用才允许此分类的执行器契约。Driver 的 recordAutomaticBuildExecutorBootstrapFailure 发现 executor-opens 已存在后直接返回，因此该报告不会生成新恢复引用。353、354 仅返回 interrupted，未保留工具超时原因。三者都已成为终态，但服务端持有已启动的任务记录；随后 refill 仍投影原来的三个引用，已完成子执行器引用过滤后 ready_executors 为空，因此不能靠重复派发继续。

修复方向：在一次请求内复用已经完成的任务描述校验；为全书拼接使用与已冻结任务及其实际依赖对应的校验路径，避免反复构造整本书路由；保留协议要求的当前状态和依赖验证。对 accepted 后的工具传输超时建立准确的终态与恢复路径，避免误归为 bootstrap。必须用实际全书拼接工作量验证启动延迟和超时重放，不能只提高等待上限或重复相同请求。

### 启动与恢复修复

2026-09-30 用户同意修复后，回归测试进一步确认了恢复时间问题：同一 invocation 的 transition clock 按未变化的 pending task 状态保存，租约判断却沿用该历史时间。即使现实时间已过运行租约期限，active dispatch 重放仍返回旧引用。新 invocation 的原有测试没有覆盖同一 invocation 持续运行的情况。

修复让 active dispatch 的租约观察及恢复引用发布使用当前时间，初始任务派发继续使用原稳定时间。恢复沿用既有 lease epoch 机制，保留 dispatch slot 和 semantic attempt，不新增恢复协议，不删除或重写工作区租约。

executor session 在单次请求内传递已验证的 task stage；open 的材料构造、input.next、generation.start 与 submit 共用该请求内的解析结果。generation.start 及其重放均从三次任务投影降为一次。每次新请求仍重新验证源、当前 policy、冻结任务及依赖，未增加跨请求缓存。全书投影的一次解析暂时保留，是否需要进一步缩减将由实际三并发启动耗时决定。

修复前新增回归已失败并确认两处原始行为：全书 stitch 启动读取 task stage 三次；同一 invocation 过期后返回原 ref。执行器误报 bootstrap 的事实仍保留；Driver 已有的 durable open 判断正确拒绝此误报。

修复验收：Driver 52 项、executor session 44 项、refill 4 项全部通过，Core 类型检查通过。全书集成回归新增了 stitch fragment 的真实 session 启动/重放与提交，断言每次 generation.start 只读取一次任务投影；测试候选控制在传输预算内，最终全书输出仍超过单任务预算。主工作区的最终 DONE 断言被另一批未完成教学阶段改动影响，因此在实际打包的隔离源码中再次验证全书流程与两种过期恢复，3 项全部通过，包含最终 DONE。隔离测试环境补齐 node_modules 引用及原始发布边界资源后，八项协议检查通过。

香港时间 19:40 安装修复版至 `E:\allwork\Understand Book\understand-book-build.exe`，安装前版本保存在 `understand-book-build.before-generation-recovery-20260930.exe`。打包排除了其他未完成改动。

沿用原 invocation 的第一次 refill 即返回原三个 dispatch slot 的新 handoff ref；新执行器 355、356、357 接续实际拼接任务。三者 semantic_attempt=1、lease_epoch=2。按与故障诊断相同的服务端持久化计时口径，实际启动耗时如下：

| 拼接任务 | 修复前 | 修复后 | 减少 |
| --- | ---: | ---: | ---: |
| 0000 | 141.827 秒 | 45.296 秒 | 68.1% |
| 0001 | 151.173 秒 | 45.577 秒 | 69.9% |
| 0002 | 140.712 秒 | 43.114 秒 | 69.4% |

三并发实际启动均低于 MCP 的 120 秒限制，已写出新的 GENERATE 记录。一次完整投影的解析目前满足超时预算，本次未扩展为新的 BookStructure 依赖校验实现。构建继续运行；整书完成仍以引擎 DONE 为准。


实际续建的五项局部拼接已全部通过，执行器 355–359 均提交，Engine 确认 local=5/5。已展开工作为 606/607，新增的一项为 BookStructure 关系选择；selection=0/1，relation=0/0，publication=pending。原 scope、Pass2 禁用与并发上限 3 保持原计划授权。


关系选择完成后，Engine 投影 selection=1/1、relation=0/95。95 是选择组展开并去重后的两两关系对数量（bookStructureSelectedPairs）；routeBookStructureProductionStage 逐对应用已接纳增量，遇到首个未完成任务即停止展开，所以当前已展开任务仍仅为 607/608。下一对依赖上一对整合后的 candidate，关系整合按顺序执行。此阶段总数应以 relation 的 95 为准，不能用 discovered 的 608 作全书最终分母。

### 关系任务并发切片（2026-09-30）

目标是兑现既有并发 3 的构建约定：保留已接纳结果与已经派出的输入绑定，向空闲执行器发布独立关系任务。任务、主题、关系增量、依赖与发布均沿用现有含义；这是纯技术调度修改。

原串行链把每个关系任务绑定到所有前序关系产物，并在第一个未完成任务处停止展开。实际上，章节摘要在关系阶段保持不变，新增主题和章节依赖使用固定身份与集合合并；共享章节的两个关系任务不构成写冲突。会修改已有主题的任务才需要按序执行。调度按选定关系的固定顺序计算主题写依赖，并把可能合并的主题别名作为后续冲突资源，完整保留祖先依赖。只在这些祖先均接纳后发布下一项任务，输入仅从其祖先增量重建。

已经冻结的串行任务在当前关系输入、父产物及策略仍相同，且原绑定的依赖产物全部有效时复用原任务。新主题身份从固定关系任务序号派生，避免并发完成形成空洞时主题编号变化。没有修改抽取提示或候选协议，也没有新增调度状态文件。

回归针对三个具体失败：独立关系未同时派发；后序任务先提交导致其他输入失效；主题合并后对别名的修改乱序。隔离安装源码集成测试同时派出 3 项共享章节的关系任务，逆序提交并验证其余任务绑定不变，最终返回 DONE 并发布 82,514 字节结果。路由与关系应用测试连同集成共 9 项通过；主源码类型检查通过。

真实书籍只读投影仍为 local 5/5、selection 1/1、relation 5/95，并展开 56 项当前可运行关系任务。原有 5 项已接纳成果全部复用。其余关系依赖具体主题前序完成，最终发布仍需全部 95 项接纳。

新包安装协议 8 项检查 compatible；完成与补位、同 invocation 租约恢复等另外 10 项针对性测试通过。已备份旧运行包并替换安装入口，继续原 invocation 后 Engine 实际返回 3 个独立派发引用，执行器 366、367、368 已同时启动。此时 relation 5/95，当前展开任务 612/668，pending 56。并发上限仍为用户原先指定的 3。

首轮实际并发验证：执行器 366/367/368 全部 committed，Engine 确认 relation 8/95，并发布下一轮 3 项。三个 generation.start 的服务端耗时为 38.524、35.965、36.663 秒，semantic_attempt 均为 1，低于客户端 120 秒期限。后续任务仍需执行与发布。

### 并发补位时重复打开已占用任务

真实续建中执行器 382 返回 committed，但 relation 仅从 21 增至 22，而不是按执行器终态数量推断的 23。只读运行元数据确认：382 的交付目标是 `stitch:relation:000010`，与执行器 381 相同，且没有新的 generation-start 记录；打开时该任务已经由 381 接纳。构建成果没有减少，但一个执行器槽位被无效补位占用。

根因是动态关系依赖释放使 accepted plan digest 与 dispatch run 改变，`selectAutomaticBuildDispatchHandoff` 对新计划中尚未发布的 manifest 没有检查旧运行持有的任务租约。因此同一未完成任务在两个运行中被分别发布；底层任务租约防止重复生成，新执行器仍会读取输入后直接结束。

修复在选择新 manifest 之前读取现有任务租约，跳过被旧运行占用的任务，继续从该计划后续 manifest 补位；当前已发布 manifest 及其恢复路径保留。原租约过期后仍可重新派发。只改变当次可选任务集合，没有新增持久状态或摘要文件。

针对性回归先复现了原函数选择旧占用批次的失败，再验证修复选择后续未占用批次，以及租约过期后的恢复。派发运行与规划共 27 项测试通过，类型检查通过。补丁已加入同一隔离安装源码快照并替换运行包，继续原 invocation；此时 Engine 确认为 relation 25/95。

跨计划补位修复后的下一轮实际续建：执行器 387、388、389 三者 committed，Engine 确认 relation 25→28/95，发布新的 3 个任务引用；三个槽位继续运行。


## 关系阶段再次超过 120 秒：输入传输测量热路径

并发执行器 396、397、398 对关系任务 21、51、52 的首次 generation.start 服务端耗时分别为 131.764、165.921、151.770 秒。任务 51 的调用端在 120 秒得到非 JSON 的工具调用错误，随后 JSON.parse 抛错，执行器误报 bootstrap_unavailable；服务端仍已接受该次启动。396、398 已提交，关系进度保留为 36/95。该失败应由原 invocation 的租约恢复继续处理。

无运行执行器时，聚焦关系任务的完整投影读取耗时 52.398 秒。CPU 采样显示输入传输分块计算占 34.39 秒；estimateTokens 自身占 17.64 秒，字符串 join 占 7.43 秒。每次读取会重新路由完整 BookStructure 输入，并在多个候选切点重复估算响应预算，三路并发放大了启动开销。

修复保留原有二分切块顺序，只改变两个纯计算实现：token 估算用字符码判断代替逐码点正则调用，仍将补充字符计为一个码点；传输候选字符串通过预计算 UTF-16 偏移直接截取，代替码点数组切片再拼接。实际同一书籍任务读取降至 14.668 秒，36/95 个已提交关系仍全部匹配。

曾尝试先测完整剩余输入，但它改变了已有分块选择结果，因此已撤回。当前补丁不包含该捷径。传输边界、317247 字节混合字符输入、窗口计数测试共 16 项通过，TypeScript 检查通过。静态执行器将非 JSON 传输错误误分类的问题仍存在；本次修复降低了触发 120 秒超时的具体热路径，后续生命周期观察仍以 Engine 持久状态为准。

部署 fast-transport 版本后，协议 doctor 的 8 项兼容性检查通过；399、400、401 三个执行器均成功提交，关系进度从 36/95 到 39/95。第一个实际 generation.start 服务端耗时 15.274 秒。旧可执行文件保留为 before-fast-transport-20260930 备份。

完整首轮启动耗时为 15.274、13.401、15.498 秒，均为 semantic_attempt=1、lease_epoch=1。修复后连续三轮共 9 个关系任务成功提交，进度已到 45/95，保持并发上限 3。


### 接收输入期间重复派发的根因

执行器 415 没有创建 task session、没有生成新候选，直接返回 committed。它的派发仍指向关系任务 000060，与执行器 412 相同，但 run 不同。415 的 handoff 在 16:28:45.850Z 发布，412 的首次 generation.start 到 16:29:32.341Z 才开始。新旧 run 的重叠发生在原执行器接收输入期间，早于任务租约建立。因此此前仅过滤 already_leased 的修复不覆盖这个实际间隙。原始任务完成后，新连接返回已有提交结果，未发生重复语义生成；损失是一个并发槽位和一次无效领取。计数只以实际已接受的任务为准。完整修复应在 refill 选择时保留 live_by_slot 中已发布 handoff 的任务占用；不能仅依赖 generation.start 后建立的任务租约。该间隙由下节记录的接收输入占用补丁修复。


### 接收输入占用补丁与 10 月 1 日恢复

接收输入期间的占用修复已部署为 held-input 版本：refill 消费终止事件后，将仍在 live_by_slot 的 opaque handoff refs 作为内部参数传给 Driver；Driver 读取本 invocation 的已有 dispatch projection 和 manifest，保留整个批次的 work-unit 占用，再交给派发选择器过滤。没有添加对外请求字段或新的持久状态。派发与 refill 测试 26 项通过，TypeScript 检查通过。后续实际补位验证见本节末尾。

上次 Root 在启动 424 后被用户中断，424 的 ownership 尚未写盘。恢复时按已记录的真实派发补齐该 ownership；三个执行器都不再活跃，Engine 进度为 58/95。422 的控制读取显示曾调用 open，423/424 没有已完成 open 的证据，均无语义提交。使用已安装 Engine 的 dispatch.finish 为三个旧 run 记录 harness_cancelled/root_supervisor；Engine 三个回执全部确认 phase=before_first_claim、task_receipts=[]。旧 refs 保留在 completed_refs，原 invocation 返回三个新的 code-issued handoff refs，已启动 425/426/427。无新 invocation、无计划重建、无语义重试消耗。

10 月 1 日恢复后，425/426/427 三个新执行器均成功提交，关系进度 58/95 → 61/95。held-input 版本的 refill 已接受两个仍被 Root 持有的 handoff refs，通过本 invocation 的真实 dispatch projection 解析占用，并返回后续的新任务；控制链路无失败边界。

### 95/95 完成后的发布质量检查边界（2026-10-01）

执行器 449–461 全部成功提交，Engine 确认 relation=95/95、local=5/5、selection=1/1，BookStructure 已接纳 702/702，pending=0。最后七项存在真实前序依赖，88/95 后每轮只有一个就绪任务；并发上限仍为 3。最终 refill 返回 NEEDS_USER(reason=quality_gate_failed, gate_status=integrity_failed)，尚未返回 DONE。

使用实际安装隔离源码只读重建质量报告：missing_artifacts=0、stale_artifacts=0、legacy_artifacts=0；702 个模型预算证明全部有效，内容质量 passed，eligible coverage=1、low-information rate=0。没有丢失产物或语义生成任务需要重新完成的证据。

发布检查有三处与 BookStructure 语义投影路由不一致：

- source_slice_coverage_invalid：该路由的 410 个章节片段使用 semantic_projection 和叶节点序号 core_range，而不是 source_slices/UTF-16 范围；quality routing 的 coverage=[]，通用检查却要求 13 个 reduction parent 都必须有字符切片覆盖。
- public_contributor_cardinality_invalid：13 个章节归约各有且仅有一个对应 final contributor。通用检查按 parent_lids 查找时，把局部拼接和后续关系贡献一起计入同一个 parent，每个章节得到 10–44 个匹配，因而全部误报。
- incomplete_eligible_closure：实际从全部 contributor、selection、relation 沿依赖遍历，可达 702/702，unreachable=0。触发点是 13 个 structure_reduce contributor 声明完整来源 leaf_lids，而归约描述符 evidence_lids 记录最终输入实际交付的引用范围；当前检查把完整来源范围与最终输入引用范围作直接包含判断。这两种范围需要分别核实，不能直接判为依赖闭包缺失。

后续修复应让 BookStructure 检查使用该路由已有的语义片段 core_range、精确 final work-unit 身份，以及依赖来源闭包；保留覆盖、唯一最终贡献与依赖新鲜度检查，不能直接跳过质量门禁。需要针对章节归约和多关系贡献的真实结构建立回归。恢复使用原 invocation 的 Engine 唯一选择 retry_current；请求 ID abreq1_bfe11737f134e3a4dca5f54855f44a943c96cdc25a24275c13e1b72d7f206e44，已随控制 ownership 空映射持久化，等待用户选择。未自行构造恢复 decision。
### 发布检查修复与恢复授权

用户明确选择“修复后重试”。修复将路由原有 BookStructureLeafCoverageManifestV1 传入质量检查，并按叶节点序号、实际工作单元 core_range 和交付 evidence 验证连续且唯一的完整覆盖；字符切片覆盖继续用于原有 source_slices 路径。章节 reduction parent 按 final work-unit 身份匹配贡献者，依赖链的新鲜度与片段可达性检查保留。structure_reduce 的完整来源范围改由该 final 的依赖证据并集验证，其他贡献者继续验证其直接交付范围。

回归先复现了与真实书籍相同的三条完整性误报，再通过修复。反例覆盖缺叶节点、重叠、缺覆盖记录、缺子产物、子产物内容变化、重复 final，以及来源超出依赖证据闭包。相关质量与发布集成共 7 项测试通过；完整章节拼接集成返回 DONE，并发布 82,514 字节结果。Core 类型检查通过。实际书籍只读质量报告也转为 passed，missing/stale/legacy=0、702/702 预算证明有效、13 个归约 parent 的误计数归零。

隔离打包仅加入本次质量检查与路由字段修改，未加入主工作区未完成教学阶段改动。2026-10-01 12:00 安装 leaf-quality 版本，旧入口保留为 before-leaf-quality-20261001 备份。已向原 invocation 发送用户确认的 request_id + retry_current，正在执行 Engine 的最终发布。

最终验收：原 invocation 的恢复 refill 返回 DONE(summary={completed_stages:3,status:complete})，ownership 空、无就绪或待执行子任务。Engine 确认 Pass1 198/198、profile_sidecar 1323/1323、BookStructure 702/702 全部 complete；local 5/5、selection 1/1、relation 95/95，publication=published。原计划 Pass2 disabled、并发上限 3、无额外 token 上限保持不变。用户边界已消费，控制检查点清除待恢复 decision。
