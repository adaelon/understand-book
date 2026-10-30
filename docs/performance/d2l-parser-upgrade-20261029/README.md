# EPUB 解析器升级后的续建恢复

2026-10-29。目标：恢复《动手学深度学习 PyTorch 版 2020.3.13》的原已确认构建，standard_deep、Pass2 disabled、并发 8。

## 根因

06:28 激活的引擎包含 EPUB 嵌套图片、含图表格及无标题分段修复。06:34 原构建恢复时，`technicalLearningTargetFromWorkspace` 用新解析器重新解释工作区 `source.epub`，再与旧版已经发布的 `source.txt` 比较，触发 `build_step_failed`。源 EPUB 未变：旧解析器的输出与保存正文完全一致。

旧输出为 797,309 字符、7,236 块、2,040 图片；新解析输出为 798,194 字符、7,436 块、2,052 图片。解析升级改变了同一输入的派生正文，续建不应把它作为隐式重新导入。依据 ADR-0146，续建消费已导入的规范正文和引用标识，更新使用独立入口。

恢复时另发现 Pass1 生成代次从 v1 改为 v2，会把已确认复用的已发布 Pass1 重新列为待抽取。该阶段的正式关闭记录和公开成果仍然有效。

主流程恢复后，执行器输入渲染及候选写入的独立路径仍直接调用 `epubToSource`，触发 `profile sidecar shadow task source fingerprint is stale`；MCP 对外投影为 `executor_internal/open`。用解析器升级的合成 EPUB 在这两条路径上分别复现，再统一正文加载入口。

内容侧车全部执行结束时，阶段验收没有传递已确认计划的 Pass1 复用项，又按当前 producer 路由 Pass1，因而报 `quality stage is not reachable in the current snapshot: profile_sidecar`。独立回归复现了该阶段切换问题。

## 修复

- `book-source.ts:loadBookSource`：工作区清单指定的 EPUB 快照与 `source.txt` 共用保存正文和 `base.json` 中的 LID；新 EPUB 输入继续使用当前解析器。
- `build-orchestrator.ts:technicalLearningTargetFromWorkspace`：保留 EPUB 包的原输入身份，不再以新解析结果重新解释已发布正文。
- `automatic-build-close.ts:readAcceptedPass1Close` 与构建路由：仅在已确认计划声明复用、来源身份与质量档一致、正式发布回执和公开文件字节一致时复用 Pass1。复用保留原 freshness 绑定。
- `automatic-build-incremental.ts:observe`：把规范正文纳入增量观察，正文变化会使缓存失效。
- `skills/build/automatic-build.ts` 的输入与 writer，以及 `automatic-build-executor-session.ts` 的 reducer 重放：使用同一规范正文加载入口，避免执行阶段再次解析包文件。
- `runAutomaticBuildCloseStage` 与 `closeAutomaticBuildStage`：验收前后保留计划的 reuse 绑定；`profile-sidecar-batch.ts` 只重读本阶段的生产路由，校验相同的 policy contracts 和已接受贡献，避免重新路由上游 producer。

## 验证

来源升级回归 4 项先失败，修复后 5 项全部通过；Pass1 生产代次变更回归先失败，修复后通过。来源加载、完整 orchestrator、增量读取、终态发布和 BookStructure 收口联跑 35 项通过；EPUB 图片与无标题结构另 10 项通过。Core TypeScript 类型检查通过。

执行器输入与 writer 的新增回归 2 项先以来源指纹过期失败，修复后通过；更新后的来源加载、内容输入准备、writer 和执行器完整会话联跑 68 项通过（包含真实 stdio 生命周期和连续批次），Core 类型检查再次通过。

真实书籍只读恢复：保存正文及全部 LID 均保持一致；原 Pass1 正式发布可复用；内容侧车 2,704 / 2,999 已提交、295 待完成。未改写已有正文、基座或已接受语义成果。

通过项目既有 `build-sidecar.mjs` 打包 Windows 引擎。候选程序对原目标、原已确认计划执行 `protocol-doctor`，八项兼容检查全部通过。07:07 激活执行器读取修复，07:56 激活包含阶段验收复用修复的最终版本；原引擎保存在同目录 `understand-book-build.before-d2l-resume-20261029.exe`，中间版本另有独立备份。

真实续建保持原计划、原 invocation 与并发 8。book_exec_680–753 连续成功提交。阶段验收复用修复后，Engine 返回内容侧车 **2,999 / 2,999、complete**，并正式进入 `book_structure`；首个总纲执行器 book_exec_754 已启动。输入、提交、验收、发布与下一阶段调度链路均已通过真实任务验证。

阶段验收新增回归先以同样的不可达错误失败，修复后通过。Core 类型检查、关闭事务 7 项、构建调度及相关发布 26 项均通过。同步集成测试曾使 Vitest 的 `onTaskUpdate` RPC 饿死；在各测试之间让出事件循环后，完整重跑退出码 0、无未处理错误。

## 范围

新解析器多识别的 12 张图片属于重新导入的变化，本次续建保留当前书籍版本已发布的规范正文。没有原地改版或重编号已有引用。

## 总纲提交超限与分批修复

进入 BookStructure 后，原全书总纲连续三次以 `transport/candidate_request_too_large/generation` 失败。根因是总纲要求一次返回全部目录单元的 question/progression，候选请求上限为 2,048 estimated tokens / 32,768 bytes；输入预算检查仅将声明的输出 reserve 截到传输上限，没有让整份输出成为可提交的任务。本书规范正文包含 **184 个目录单元**，一次提交无法满足该要求。

`book-structure-organization.ts` 对尚未接纳的较大总纲创建每批 4 单元的章节任务，各批保留完整的全书输入并按原顺序合并；已接纳的原格式总纲继续复用，小总纲保持原路径。新增独立的章节与主题提示合同，原提示字节保持不变。主题通过 append / extend / finish 累积，成员证据在每次提交时校验，完成后才开放全书总纲。调度、输入渲染和工作单元类型同步支持两个新任务种类，单次传输上限不变。

184 单元回归先复现单个超限任务，修复后通过真实 writer 验证所有章节分批提交、已成功批次复用、主题成员与证据延续，以及完整总纲合并。来源目录不完整或主题成员缺少证据仍被拒绝。容量、规划、恢复、准备、阶段发布和调度共 **33 项通过，1 项原有跳过**，Core TypeScript 类型检查通过。

真实书籍只读路由耗时 1,667 ms，返回 **46 个待执行章节批次**，没有输入预算阻塞；184 个单元均有可用引用依据。修复已打包并激活到原 operator 引擎路径，激活前的版本保留为 `understand-book-build.before-d2l-outline-fix-20261029.exe`。语义生成的真实恢复结果另由 Engine 进度确认。

首次实跑暴露了新任务接入的提示映射遗漏：章节批次仍使用旧的 `book-structure-outline` 提示提供者。已新增 `book-structure-outline-chapters.md` 与 `book-structure-outline-themes.md`，分别注册到 extractor、CLI 白名单和编译后的提示资产表。新增生产提示路由测试直接比对任务合同与实际交付提示，提示与调度 15 项测试、Core 类型检查均通过；打包可执行程序对两种新提示的实际返回字节也均与合同一致。08:52:50 打包的版本已激活，前版保留为 `understand-book-build.before-d2l-outline-prompt-fix-20261029.exe`。Engine 随后确认 **45/46 个章节批次已提交**。

最后一个批次的 `schema_invalid/artifact_writer` 诊断显示缺少 question 引用，但路径为 `/0/question/evidence_lids`。新增章节分批 validator 在外层对象之外校验数组，造成错误路径漏掉 `/chapters`。现使用完整批次 schema 校验后再验证来源顺序与引用；既存失败的修正反馈补齐该已知路径，保留原始失败记录。3 项新增回归先失败，修复后规划和 extractor 合同共 **35/35 通过**，Core 类型检查通过。v6 打包后激活，前版备份为 `understand-book-build.before-d2l-outline-diagnostic-fix-20261029.exe`。按用户已授权的修复与继续范围恢复同一 invocation，45 个已接受批次保持复用。book_exec_851 随后成功提交，Engine 确认章节总纲 **46/46** 全部通过，并进入整书主题归纳。

随后四个主题动作均已接纳，整书总纲合并完成，Engine 发现 682 个后续结构任务，结构阶段已发现总量为 732。继续同一 invocation 后，最新 Engine 投影为 **165/732 已验收、567 待完成**。这只是当前结构阶段的已发现数量，尚不代表后续阶段总量或全书完成。

聊天恢复通过根任务自己的控制检查点继续，保留已完成 ref、原 invocation 和已确认计划。每次返回投影、登记新执行器及提交续接请求时均保存控制状态；检查点不包含语义输入或候选内容。中断批次已按拥有的执行器身份提交给 Engine 续接，已接受成果未回退。

## Windows 调度锁读取竞争

持续 worker 版本沿原书续建后，Engine 已推进到 **210/732**。2026-10-29 16:05:59 的 request `abreq1_b77e3ba9c91273e201f2279329ea6125fb857de6edbfd9594020765dd16802b5` 记录 `EPERM / open / readFileSync / withAutomaticBuildWorkerSchedulerLock`。refill 请求字段正确，失败来自争用者在独占创建返回 EEXIST 后读取现有锁的 PID：锁持有者可在两者之间释放文件，Windows 将删除中的文件投影为 EPERM/EACCES，而旧路径仅容忍 ENOENT。

`automatic-build-worker.ts:withAutomaticBuildWorkerSchedulerLock` 在上述 PID 读取位置把 EPERM/EACCES 继续作为竞争等待处理。没有绕过独占创建、删除活跃持有者的锁或吞掉其他存储错误。新增真实锁持有者与争用者的回归，两个 Windows 错误先复现失败，修复后验证等待、顺序和锁清理；EIO 仍原样失败。新增锁回归、持续 worker 集成和 driver 失败边界共 **23/23 通过**，Core TypeScript 类型检查通过。

v7 通过既有打包入口生成 614 模块的可执行程序，原目标与原已确认计划的 protocol-doctor **8 项 compatible、invalid_artifacts=0**。已激活到原 operator 路径，前版保留为 `understand-book-build.before-d2l-worker-lock-fix-20261029.exe`。保持原 invocation、质量门和并发上限，继续消费已终止 child 的控制观察并恢复构建。整书完成仍须由 Engine DONE 确认。
