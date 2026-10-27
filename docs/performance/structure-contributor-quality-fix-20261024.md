# BookStructure 辅助分片贡献范围修复

2026-10-24，原优化教材 invocation 在 1489/1489 项完成、14 章和 8 个主题及汇总完成后，发布前检查返回 `integrity_failed / incomplete_eligible_closure`。

`build-orchestrator.ts` 的 discovery 路由把所有贡献者的 `parent_lids` 都登记为 `source_leaf_range` 内的正文叶。辅助分片的位置可落在标题叶，但实际交付的是关联正文证据；位置范围并不等于其贡献范围。真实工作区有 6 个辅助分片触发这一差异。1489 个模型记录均有效且可达，没有缺失、过期产物或无效证明；23997 个核心叶覆盖完整，内容质量通过。

修复将 discovery 贡献者范围绑定到其原有 descriptor 的实际交付证据。核心叶的完整覆盖仍由独立 coverage manifest 验证；质量门、候选证据规则、生成输入、接受结果及任务身份均保持原有约束。未修改既有产物或重建计划。

`book-structure-discovery.test.ts` 新增真实生产路由回归：标题与正文共享概念且材料量触发辅助分片。修复前贡献者登记断言失败，修复后通过。相关 discovery、BookStructure quality、quality v2 共 13 项测试通过，core 类型检查通过。覆盖缺口、过期依赖和范围外证据等原有拒绝测试仍通过。

原工作区只读重放显示贡献范围差异从 6 降至 0，质量门变为 `passed`；无模型调用、无产物发布。限定数量诊断见 `.codex-recovery/optimization-structure-quality-diagnostic.json`，实现为 `.codex-recovery/diagnose-optimization-structure-quality.ts`。正常打包不含诊断插桩。

正常执行器安装后，同一 recovery request 的 `retry_current` 被接受。引擎确认 BookStructure 完成、publication 为 `published`，并进入 `formal_objects`（初始展开 758 项）。原 invocation 与计划沿用。安装前版本保存在安装目录 `understand-book-build.before-structure-contributor-20261024.exe`。
