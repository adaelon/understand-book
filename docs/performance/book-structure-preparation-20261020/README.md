# BookStructure 提前准备与交接耗时

日期：2026-10-20。

结论：提前准备使本地整阶段路由计算减少约 47% 和 35%；本轮尚未证明完整交接稳定变快。最终候选与旧版复测的交接中位数分别为短章节 4.695 → 4.519 秒、长章节 5.985 → 5.991 秒，样本范围重叠。实现作为独立候选保留，日常安装版未替换。

## 完整交接结果

下表采用后一次旧版复测作为对照，两边均为首轮之后三个样本的中位数。旧版首轮波动更大，不能用它放大收益；首轮、复测和所有候选的原始数据均保留。

| 场景 | 旧版复测中位数（范围） | 最终候选中位数（范围） | 中位数变化 | 判断 |
| --- | ---: | ---: | ---: | --- |
| compact | 4.695 s（4.319–4.850） | 4.519 s（4.451–4.529） | 缩短 3.8% | 范围重叠，未证明稳定改善 |
| fragmented | 5.985 s（5.244–7.620） | 5.991 s（5.537–6.273） | 增加 0.1% | 没有可确认的改善 |

分段中位数：

| 场景 | 提交，旧 → 新 | 补位，旧 → 新 | 下一执行器开始生成，旧 → 新 |
| --- | ---: | ---: | ---: |
| compact | 494 → 533 ms | 2,377 → 2,224 ms | 1,805 → 1,710 ms |
| fragmented | 1,061 → 992 ms | 2,480 → 2,458 ms | 2,444 → 2,366 ms |

提前准备的重复样本中位数分别为 **1.040 秒**（0.989–1.077）和 **1.273 秒**（1.258–1.285），在生成窗口内执行。它把批量路由计算移出了补位，但进程启动、工作区读取、当前依赖检查与其他规划仍在交接路径中。

旧版第一轮的重复样本中位数为 6.616 / 7.435 秒；合并两轮旧版后为 4.773 / 7.296 秒。若只比较合并中位数，会得到长章节约 17.9% 的表观下降，与较保守的复测对比并不一致，因此不将其作为已证明收益。

原始记录：[旧版首轮](compiled-baseline.json)、[旧版复测](compiled-baseline-repeat.json)、[最终候选](compiled-release.json)、[所有版本汇总](comparison.json)、[环境](environment.json)。每轮各场景的第一个样本单列在 JSON 中，未混入上述重复样本统计。

## 改动

`build.prepare` 接受 `stage=book_structure`，在执行器生成期间准备章节 unit/fragment 的路由、切片、输入和预算证明。准备结果保存在工作区 `.build/automatic-build/v4/book-structure-inputs.json`，整阶段规划与补位复用；按 `parent_lid` 校验单章时沿原路重算当前章，避免解析全书缓存。接口只返回计数和状态，不创建 dispatch、租约或语义尝试。

复用条件包括既有原文身份、目标、执行策略、传输配置、预算、当前已接纳提纲，以及每个章节已有的 `bookStructureUnitHash`。该章节标识已经绑定 Pass1/discourse/formula/可选 Pass2 投影，本次直接复用它跳过切片和输入预算计算。准备不缓存已接纳子结果、pending 状态、reducer 或章节/主题组织结果。

技术书 discovery 依赖提纲。提纲未接纳时返回 `skipped/outline_pending`，Root 在下一次章节执行器启动后再准备。执行器的 open、generation.start 和 submit 仍读取当前任务与实际依赖。BookStructure 冻结输入在 Engine 内调用原读取和渲染函数，省去额外 input 子进程。

## 场景与计时

| 场景 | 源内容 | BookStructure 任务 |
| --- | --- | --- |
| compact | 32 章，每章 2 段，每段约 975 字符 | 32 个 discovery fragment，每章一片 |
| fragmented | 8 章，每章 32 段，每段约 975 字符 | 16 个 discovery fragment，每章两片 |

技术书当前的章节提取统一走 discovery fragment，compact 表示每章一片的短章节。论文 profile 的普通 `structure_unit` 另有输入等价性与缓存命中回归，未计入这两个性能场景。

基准使用真实编译版 Engine、真实 stdio MCP、任务存储、writer、Driver 和三槽位补位。在每个样本开始前恢复同一份 fixture，绝对路径不变，准备缓存为空；Pass1、profile sidecar 和提纲是固定的合法前置成果。先启动三个执行器，再让其中一个提交固定候选，只释放它的槽位并启动一个新执行器。

主要指标为 `submit_candidate` 开始到下一执行器 `GENERATE` 返回的墙钟时间，拆成：

- `submit_ms`：提交、校验和持久接纳。
- `refill_ms`：回收已终止 child 的槽位，读取当前状态并产生一份交接。
- `next_start_ms`：新进程/stdio MCP 初始化、open、完整 input.next、generation.start。
- `handoff_ms`：上述整段实测时间；各分段中位数之和不必等于总时间中位数。

`prepare_ms` 单独计时，发生在模拟的生成窗口内，不计入交接指标。每轮首个样本单列，之后三个样本用于重复测量。这里的首轮不是清空操作系统缓存后的冷启动；每次命令和新执行器都使用新进程。

## 路由计算与方案取舍

另在同一 Node 进程中对同一输入交替启用/停用准备文件，先预热一对，再计时七对。准备文件的改名、还原与准备本身均在计时外，源文件和已接纳成果保持相同。这项实验隔离 BookStructure 路由，不包含进程启动、MCP 或主模型。

最终候选的整阶段路由中位数：

| 场景与读取范围 | 未准备墙钟 / CPU | 已准备墙钟 / CPU | 判断 |
| --- | ---: | ---: | --- |
| compact，整阶段 | 345 / 359 ms | 182 / 187 ms | 路由墙钟减少 47.3% |
| fragmented，整阶段 | 623 / 640 ms | 405 / 406 ms | 路由墙钟减少 34.9% |

按 `parent_lid` 读取单章时，两组都执行相同的原路由，没有新增缓存读取；对应样本保留，用于观察测量波动。最终准备数据分别为 544,714 和 769,387 字节。

存储方案经过两次对照：首个单文件候选在 Node 单章实验中出现约 183 → 204 ms 的小幅回退；按章文件候选消除了这项额外读取，但 compact 交接中位数为 4.143 秒，没有超过首个候选的 3.842 秒，准备数据还增加到 3.49 MB。最终只把单文件准备结果用于批量规划，单章保留原路由。不同候选的整段交接波动较大，不能按这些中位数单独给实现排序；最终取舍依据是隔离实验中可重复的批量计算节省和读取范围。

原始交替样本见 [最终路由实验](routing-pairs-release.json)；[按章文件实验](routing-pairs-final.json)、[首个单文件实验](routing-pairs.json)也保留。CPU 时间为进程 user + system，可包含运行时线程工作，因此可能高于同次墙钟时间。

使用同一初版二进制关闭准备的[对照结果](compiled-candidate-without-preparation.json)也有较大波动，仅作为诊断记录；不据此声称输入进程消除与提前准备各自贡献了固定比例。

## 正确性验证

相关回归共 10 个文件、113 项通过；可选路由计时测试从普通回归中跳过，另行执行。Core TypeScript 检查通过，源技能与插件发布副本的一致性检查通过。

覆盖了准备前后输入与任务描述符一致、等待提纲、提纲/上游投影/原文/质量策略/传输配置变更后的失效、拒绝旧冻结候选、实时 fragment 接纳和章节就绪，以及普通论文 unit 的准备。已有预算批准、三槽位补位、BookStructure 整理恢复与关闭，以及 profile discourse 两条准备路径均通过。

编译版交接场景要求另两名 child 的槽位归属保持不变，新交接只有一份；每次固定候选仅产生一个语义尝试、一次持久提交，失败计数为零。比较的是输入、路由和接纳行为，没有进行真实模型的内容质量评测。

最终编译版的两个交接场景和隔离路由实验共三项通过。日志见 [regressions.log](regressions.log) 与 [compiled-release.log](compiled-release.log)。

## 复验

测试入口：`packages/core/test/book-structure-preparation.test.ts`。在 `packages/core` 运行：

```powershell
$env:STRUCTURE_HANDOFF_MODE = 'baseline' # 候选完整优化用 prepared
$env:STRUCTURE_HANDOFF_EXE = '<绝对路径>/understand-book-build-x86_64-pc-windows-msvc.exe'
$env:STRUCTURE_HANDOFF_REPORT = '<绝对路径>/report.json'
node ../../node_modules/vitest/vitest.mjs run test/book-structure-preparation.test.ts --maxWorkers=1 --minWorkers=1 --reporter=dot
```

隔离路由实验在性能 fixture 已存在后执行：

```powershell
$env:STRUCTURE_ROUTING_REPORT = '<绝对路径>/routing-pairs.json'
node ../../node_modules/vitest/vitest.mjs run test/book-structure-preparation.test.ts -t 'isolates cached routing' --maxWorkers=1 --minWorkers=1 --reporter=dot
```

性能模式使用 `tmp/book-structure-preparation-20261020/{compact,fragmented}-initial` 保存同输入的起点，逐样本恢复对应 `*-work`。不设置报告路径时，测试使用自动清理的临时目录和较小输入。基线可执行文件与候选文件分开保存；基线包含此前已完成的 profile discourse 优化。

基线 HEAD、改动前相关源码及其未提交 diff、完整的构建源码快照 `baseline-compile/`、编译日志、可执行文件和原始 fixture 保存在 `tmp/book-structure-preparation-20261020/`。最终候选为该目录 `compiled-release/` 下的可执行文件；`compiled-final/` 是已测量但未采用的按章文件候选。固定候选只用于验证路由、交接和接纳，不调用模型。

## 已知边界

- 本实验不包含主模型排队、思考、原生 Codex 子代理创建或真实内容生成时间，不能据此推算全书加速比例。
- 准备窗口若短于实际准备耗时，剩余部分会落到等待时间内；连续任务可复用同一次准备。
- 本地文件读取、解析和实时依赖检查仍存在；后续章节选择、主题与最终组织没有提前缓存。
- 本轮交付独立的编译候选，未替换日常安装版。
