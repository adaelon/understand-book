# U1–U2 工作区搬迁验证记录

日期：2026-10-01。合同：[ADR-0146](../adr/0146-portable-build-workspaces-and-incremental-book-updates.md)、[U 方案](../切片方案-工作区搬迁与书籍增量更新.md)。U1、U2 已实现，工作树未提交。

## U1 来源快照

技术书续建优先解析已导入工作区。Markdown 读取内部 `source.txt`；EPUB 导入保存 `source.epub`，`canonical_source.snapshot_path` 为相对路径，原 `path` 保留导入来源。EPUB 包身份继续使用包字节，规范正文单独参与任务输入；读取包时核对其正文与内部 `source.txt` 一致。

`book-source.ts` 统一 Core 与构建 CLI 的来源加载。旧 EPUB 工作区缺少包快照时，使用已有 `source.txt + base.json.lid_nodes`，避免将无标题标记的规范正文按 Markdown 重新切分。导入复制显式 PDF / 映射附件；已有工作区收口保留来源及图片清单。Paper 继续使用现有 Workbench 相对路径和可信来源门禁。

失败复现：真实 `pass1-batch --allow-partial` 导入 Markdown / EPUB，复制工作区并修改原文件；修改前分别报 canonical source 漂移和 invalid zip data。修复后原文件编辑、删除及图片原件删除均不改变内部输入；再次收口仍可读取原图片。测试同时比较搬迁前后的工作单元、完整输入摘要和引用范围，并验证 EPUB 包身份不同于规范正文身份。

Paper 用真实 PDF 几何提取和 source reconciliation 生成中间结果；复制后修改原目录 Markdown / PDF，在新目录运行 hybrid foundation 并经正式目标入口读取，规范正文及内部 PDF 正确。

## U2 成果与执行归属

语义产物、策略锁、迁移回执和已通过质量报告按书籍、来源、profile、任务完整输入与抽取合同判断有效性。搬迁不修改原成果信封；读取冻结任务时只在内存中投影当前寻址信息，当前 writer 仍要求当前目标。

复制来的执行记录保留物理序号，但不占新位置的租约或重试额度。租约读取、提交入口继续核对原执行目标；旧 token 和复制后的旧租约路径不能作为新运行提交。新关闭回执按既有发布事务和既有 freshness identity 分开存储：`.build/automatic-build/v2/close/<stage>/<transaction_id>/<freshness_digest>.json`，旧回执保留。

失败复现：导入两章材料、完成一项 Pass1 并领取另一项，复制工作区后恢复；修改前报 policy_generation_conflict。修复后正式 `automaticBuildPlan/Next` 只调度未完成项，领取新位置租约并交付正确输入，旧提交入口拒绝请求且不写结果。Pass1 和 profile sidecar 关闭后再次搬迁，完成状态保持。

BookStructure 使用现有 29 章完整生产夹具，经过局部贡献、selection、relation delta、质量门禁及发布。搬迁后没有待抽取任务；移除新目录的公开结构文件后，经正常 close 入口从已完成成果重发 82,514 字节输出，与原输出完全一致。该用例还复现并修复了跨位置关闭回执冲突。策略采用测试覆盖复制后中断的旧 generation 采用，恢复后当前成果绑定新位置、旧来源文件不变。反例保持跨书、不同来源、不同 profile、输入或语义合同变化不可采用。

## 验证结果

- 来源、入口、Workbench、策略 generation、语义产物及显式旧计划：6 文件，52 项通过。
- BookStructure 完整生产链与阶段 close：2 文件，8 项通过。
- 执行记录、租约、Pass1 / profile 归约、教学构建及 CLI：6 文件，63 项通过。
- `pnpm --filter @understand-book/core typecheck`：通过。

入口集成含多个真实子进程，定向回归使用 `--testTimeout=30000`；BookStructure 完整夹具保留自身 120 秒限时。一次并发运行触发既有 5 秒限时，扩大该次运行限时后通过，未修改既有测试断言来绕过失败。

## 限制

- 旧 EPUB 若缺少原包，只能从已有正文与 LID 树恢复来源；若旧任务绑定的是已经丢失的包字节身份，不能将正文摘要冒充该身份采用成果，需要恢复原包或重建。
- 搬迁仍保留 `.understand-book/<book_id>` 目录结构和 book_id；从新位置开始新执行，已有 invocation / executor 不迁移归属。
- 模型边界使用固定候选验证生产 reader、writer、调度、引用与发布，未调用付费 Provider。真实材料和模型成本验收保留给 U11。
- 内容更新、独立新版、跨版引用重绑与私人记录延续由 U3–U10 交付。
