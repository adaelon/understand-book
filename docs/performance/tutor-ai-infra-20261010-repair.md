# Tutor 流水线修复与实测（2026-10-10）

## 交付结果

新版已安装到 `E:/allwork/Understand Book/understand-book-build.exe`，文件大小 106,926,592 字节。对 `ai-infra-book-complete` 的原 invocation 执行完整 `build.refill` 成功，耗时 **27.5836141 秒**；同一进度在深层瓶颈修复前的热回读为 **261.1818831 秒**，本次对比约快 **9.5 倍**。这是两次实际命令的对比，不是长期吞吐量估计。

引擎确认 500 个工作单元已接纳，当前发现总数 503，待执行 3。对象对齐为已完成 10、剩余 1,253、当前总数 1,263。BookStructure 保持完成，Pass2 保持关闭。认知材料和发布仍在等待对象对齐完成。

## 采样补充的根因

最初完成共享存储和路由缓存后，局部路由已降到约 7.3 秒，但完整回读仍为 220.6 / 261.2 秒。随后对完整命令采样，241.35 秒的样本中：

- `validateWorkUnitDescriptorV3` 的编号包含校验占约 **184.76 秒（76.5%）**。代码使用 `basisParentLids.some(lid => !evidenceLids.includes(lid))`。本书每个全书对齐任务有 9,111 个来源编号；预算和派发规划又反复验证历史描述，形成大量平方级比较。
- `spawnSync` 占约 **29.27 秒（12.1%）**。安装契约检查通过启动自身 CLI 读取已内置的提示词、代理模板和 MCP 配置。
- 已关闭的 Pass2 仍在快照阶段构造候选和任务包，再由后面的计划筛掉。

这补全了此前仅凭日志和代码所作的判断：全历史重放和写放大确实存在，但不能把完整回读的耗时全部归于它们。完整采样保存在 `D:/understand-book-repair-20261010/refill-cpu`，对应的临时可读 bundle 为同目录 `sidecar-profile.js`。

## 实现

1. **线性证据校验**：V3/V4 描述校验建立一次编号集合，保留逐项包含检查及错误语义。没有删减来源编号，也没有跳过原有证据约束。
2. **内置资源直接读取**：sidecar 注册自己的内置提示词、协议包装、代理模板与边界配置。进程内读取沿用原有内容校验；源码调用外部 sidecar 的既有方式仍可使用。
3. **按已确认计划投影 Pass2**：将 `public_stage_closure` 传入快照与准备路径。关闭 Pass2 的计划跳过它的候选/任务包构造；显式检查和启用 Pass2 的计划仍可投影该阶段。
4. **共享任务材料**：全书来源、结构、证据编号和对象数据存入共享文件。对象按稳定键顺序分块保存，并另外保留逻辑顺序，避免 resolve 将对象移到末尾时重写所有数据块。相同的冻结任务与 writer context 不再各存一份。
5. **精简动作回执**：普通对齐动作只保存接纳动作、必要的检索规则与少量状态；最终身份物化仍保留其必要结果和依赖。旧格式回执继续读取，既有接纳文件保持原样。
6. **增量路由**：缓存已接纳 fragments 和各并行分支的动作位置。来源、结构、策略、检索依赖或原回执变化时重建；缓存丢失可由原回执重放恢复。缓存使用可保留对象共享关系的序列化和压缩，写满磁盘不会破坏上一份缓存或接纳成果。
7. **避免历史状态深拷贝**：检索动作复用对象表，resolve/identity 只复制改变的记录和容器。已冻结状态及累计原文阅读范围保持不可变。
8. **减少普通关键词翻页**：新查询每页最多 24 条，优先排列意思/别名命中的候选，保留全部召回和后续分页。已开始的旧查询仍按原来的 6 条与原顺序继续，prepared retrieval 的规则保持原样。

主要代码：`packages/core/src/stage-work-unit.ts`、`teaching-build.ts`、`teaching-object-alignment.ts`、`teaching-task-storage.ts`、`teaching-route-cache.ts`、`build-orchestrator.ts`，以及 `skills/build/automatic-build.ts`、`executor-prompt.ts`、`sidecar-entry.ts`。

## 写入大小与磁盘

| 项目 | 修复前样本 | 修复后实际文件 |
|---|---:|---:|
| 单个冻结对齐任务 | 4,807,087 字节 | 8,419 / 10,957 / 9,502 字节 |
| 内容相同的 current 副本 | 再写一份 4,807,087 字节 | 三个任务均未创建重复副本 |
| 共享材料首次建立 | 每动作重复写入 | 28 个文件，共 3,501,186 字节，后续复用 |

普通动作的精简回执通过 writer 回归验证；这次真实恢复只回读并派生待执行任务，没有新增语义候选。上述 8–11 KB 是冻结文件大小，不是把首次共享材料和缓存的开销算作零。

30 份旧引擎备份已从安装目录转存到 `D:/understand-book-repair-20261010/previous-engine-backups/`，共 3,200,834,560 字节，全部保留。清单为该目录 `relocation-manifest.json`。当前使用中的旧版本以安装目录中的 `running-before-tutor-*.exe` 名称保留，以免中断已打开的进程。最终检查 E 盘约有 2.40 GB 可用。

## 验证

- Teaching build / object alignment：52 项通过，覆盖旧回执复用、缓存丢失与依赖变化、连续原文阅读、磁盘写满恢复、对象引用重写、稳定身份复用、检索分页连续性及历史状态不可变。
- Model input budget / executor prompt：19 项通过，包含 10,000 个编号的 V3/V4 正常/越界校验，以及全部已注册提示词在进程内读取后的字节一致性。
- 新增 Pass2 计划范围回归：1 项通过。Pass2 启用/关闭的教学发布回归另复验通过。
- Core 类型检查通过；最终 sidecar 构建成功；修改文件的空白差异检查通过。
- 安装后的完整 `build.refill` 返回 `SPAWN_EXECUTORS`，ready 数为 3，耗时 27.5836141 秒，退出码 0。

命令回执、计时与非语义验证数据在 `tmp/tutor-analysis-20261010/repair-final-output.json`、`repair-final-timing.json`、`repair-verification.json`。

## 恢复位置

仍使用原 invocation：`abinv1_5a49b34acdd7ea4ceb869a8968ae35de935e54719969e21f74b120040dda33f6`。

`tmp/tutor-published-reuse-20261008/control.json` 已消费 tutor_816 / 817 / 818 的终止通知：live slots 为 0，completed handoff refs 为 818，`next_child` 仍为 819；最终的三个待派发引用保存在 `ready_after_repair`。本轮没有启动新的语义子执行器。下一次继续仍应由原 invocation 回读并按正式 handoff 派发，不能把 500 个工作单元当作 500 个已对齐对象。

## 已知问题与测量限制

- 三个对象的整批合并屏障和每次 invocation 一个动作的执行协议仍保留。此次通过加大普通检索页减少启动次数，没有改成持续对象会话或任意完成顺序下的滚动调度。
- 先前记录的 73 分钟主控延迟收取结果，现有证据仍不能指定其外部成因；本补丁没有修复宿主调度。
- 未重新运行数小时构建来测最终对象吞吐量，整本 Tutor 尚未完成。
- C 盘在最终快照仍只有约 109 MB 可用；本次转存的是相关引擎备份，没有迁移其他用户数据或 Codex 状态。
- 单独运行既有 `exposes BookStructure without Pass2...` 测试时，其旧期望列表缺少现有的 `structure_chapter_selection`，在本次新增断言之前失败。未为该无关旧期望改生产代码；使用独立的新回归验证本次 Pass2 变更。本报告不声称全库测试全部通过。
