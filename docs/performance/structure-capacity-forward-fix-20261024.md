# Structure organization capacity recovery — 2026-10-24

The optimization build reached `structure_execution_budget_exceeded`: rendered body
20,988 tokens, limit 20,000. Existing accepted artifacts remain authoritative.

`book-structure-organization.ts:renderStructureOrganizationInput` previously compacted
only oversized outlines. It now also removes JSON formatting overhead from oversized
chapter, theme and reconciliation bodies. Parsed content, prompts and capacity gates
are unchanged; fitting inputs and the established theme-plan rendering stay byte-stable.

Three synthetic regressions failed before the fix and passed afterward. Sixteen tests
across outline capacity and themes passed; core typecheck passed. The normal packaged
engine was rebuilt and installed with the previous executable retained. Windows file
mapping prevented overwrite, so the running executable was renamed before replacement.
The original `retry_current` request became stale when descriptor discovery advanced;
a fresh ordinary Engine read retained plan authorization and returned three executors.
Live continuation confirmed 1372/1375 tasks and 11/14 chapters on the same invocation.

A later compact chapter body reached 20,460 tokens: its 960-reference browsing ledger
used 11,221 tokens and its 64-candidate preview used 6,347. Bounded observation retained
only field token estimates and array counts (`tmp/structure-capacity-fix-20261024/capacity-counts.json`).
`fitStructureOrganizationExecution` now halves only an oversized chapter preview until
the real execution budget passes or the page reaches one item. Its fitted context is
stored with the generation input, so acceptance grants browsing state only for the
actual delivered page. All prior references, notes, excerpts and evidence remain intact;
unshown candidates remain reachable through `next_offset`. Already-fitting inputs retain
their identities. A genuinely oversized excerpt still fails the existing budget gate.
Twenty-five capacity, chapter planning and theme tests passed; core typecheck passed.
The normal engine resumed the same invocation at 1387/1389 tasks and 12/14 chapters,
issuing two fresh handoffs after its ordinary read revalidated the original plan.

## 2026-11-16 大纲输入分批与主题分页

《财富自由革命之路》的 147 个结构单元、2,394 个源文叶子节点生成了 42,118 估算 token 的紧凑大纲。
旧路由在尝试复用整本大纲时先检查单任务容量，超限后提前退出。每批四个单元的输入仍包含完整大纲，37 个批次均超过 20,000。

`book-structure-organization.ts` 在整本大纲无法执行时进入分批。原输入可容纳时沿用原任务身份，超限批次交付全书章节标题和当前单元完整节目录、摘录及证据。批次按实际执行预算缩小。

`book-structure-outline-pages.ts` 与 `structure_outline_theme_page` 使用已接纳的章节说明组织主题，分页浏览章节和主题目录，可按单元身份重新查看远处章节。新增主题及成员只引用本次实际交付的章节证据。Core 保留先前主题成员及证据，并在全部章节交付后接纳结束动作。输入页随执行预算缩小，接纳与覆盖记录绑定实际交付页。

分页主题使用独立的既有格式策略代号 `structure-outline-theme-page.full.v1`，经正常任务生成、writer、当前状态重放和执行器提示词分发路径接入。

专项验证共 58 项通过，覆盖大输入分批、184 单元已有分批恢复、跨页主题、未交付证据拒绝、完整覆盖、正式 writer 重放、旧成果复用、连续 worker、调度与提示词分发。Core TypeScript 检查通过。恢复测试的一处旧断言已按当前标准深读阶段集合修正为仅包含结构投影。

本书真实路由产生 37 个可执行批次，覆盖全部 147 个单元，输入为 8,932–10,563 估算 token。新编译引擎已安装到当前 operator 目录，旧二进制保留为 `understand-book-build.before-outline-input-paging-20261116-100858.exe`。

同一 invocation 的已安装引擎恢复派发，Pass1 178/178、内容侧车 283/283。37 个章节大纲批次全部接纳，主题分页完成后累计接纳 79 个组织任务，最终大纲包含全部 147 个章节单元和 18 个主题。

随后 Engine 在正文发现路由返回 `NEEDS_USER`，reason 为 `routing_blocked`，code 为 `model_input_unsplittable`，phase 为 `routing`，stage 为 `book_structure`，retryable 为 false，recovery_actions 为 `["upgrade_executor"]`，choices 为空。请求 ID 为 `abreq1_2831f554bc084511d09764cb445bc3dc01e12f5f993d97440f991dffaa2a532e`。所有本次拥有的执行器均已终止并通过 refill 释放；整书尚未发布。

数值诊断使用生产路由与当前传输预算，只输出数量和 token 数。147 个源单元中，143 个可路由为 283 个片段，另外 4 个单元的首个不可拆分输入超过正文发现的 6,000 token 上限。一个完整叶子节点正文为 5,822 token，带封装共 6,348；三个关系证据包分别为 10,634、11,096、9,453。它们的主题导航仅占 1–85 token，章节规划占 79–97 token，阻塞并非主题分页固定开销。原大纲容量问题已通过真实运行验证；后续原子路由的修复见下一节。

## 2026-11-16 正文与关系证据切片

`book-structure-fragment-slices.ts:routeBookStructureFragmentSlices` 复用现有 `routeModelInputSlices`，只在完整正文叶或辅助记录超限时按规范化原文的绝对 UTF-16 范围切片。每页经当前 V3 执行预算与传输证明重新接纳。正文切片使用现有 `source_slices` descriptor，关系证据页保留 `semantic_projection`，引用权限来自本页实际交付的原文。

`book-structure.ts:coverageManifest` 将长叶关联到全部切片；`automatic-build-quality.ts` 同时核对整数叶覆盖、UTF-16 覆盖和切片任务集合。`build-orchestrator.ts` 在全部关联切片接纳后才将该叶计为完成，并将全部观察交给既有章节组织路径。普通输入保持既有任务与渲染路径。辅助记录本身过大、公式/图片等无安全切分边界的内容继续返回原有容量边界。

辅助记录分页先保留可容纳的记录组，共享原文分页，避免逐条重复阅读同一长段。实际书籍的首版逐条分页产生 969 个任务（正文 276、图节点 432、关系 241、话语记录 20）；合批后为 369 个（276、28、45、20），减少 600 个发现任务。147 个单元全部可路由，阻塞数为 0；既有 147 章节、18 主题、79 个组织成果继续复用。这里统计的是发现任务，不是整书后续总任务或完成耗时。

相关验证共 59 项通过，Core TypeScript 检查通过。新增回归覆盖规范化原文完整拼接、关系两端分页、未交付证据拒绝、辅助记录合批、真正不可分记录、缺片/重复/错误关联、部分提交后的完成计数，以及准备缓存与 focused writer 一致性。长段落合成样本从切片、逐片提交、恢复、章节组织到结构发布闭环通过。一次新增测试的节点顺序预期已按既有按 ID 排序语义修正，复测通过。

已重新编译并替换当前 operator 引擎，备份为 `understand-book-build.before-discovery-input-slices-20261116-114313.exe`。已安装二进制对原 invocation 的 `build.prepare` 返回 `prepared`、`fragment_tasks=369`、`ordinary_tasks=0`。本次修复验收止于后续输入准备完成；真实书籍的剩余语义生成及整书发布尚未完成。

## 2026-11-16 章节读取容量

原书已接纳全部 369 个正文发现片段，覆盖 2,394 个原文节点。章节读取在第 3 个动作的输入达到 23,444 token，超过 20,000 上限。其中全书章节规划占约 15,120 token，请求的两段原文占约 5,864 token。这次容量阻塞发生在章节组织，正文发现结果继续复用。

`fitStructureOrganizationExecution` 在章节包仍超限时，把其他章节的规划缩为依原序排列的标题和标识，保留本章规划、主题与请求原文。渲染后重新通过执行预算校验。真实工作区只读重放维持 36 个已接纳章节，余下 111 个可派发，原超限包为 14,140 token，包含两段原文。章规划、容量及生产恢复的 28 项测试和 Core 类型检查通过。operator 引擎已安装，上一版备份为 `understand-book-build.before-chapter-read-budget-20261116-161632.exe`。
