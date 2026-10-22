# 本例诊断与 RSI 接手结论

最终案例入口：[collected-final/case.json](collected-final/case.json)，公开结构索引：[collected-final/index.json](collected-final/index.json)。`collected/` 与 `collected-specific-diagnosis/` 是范围说明收窄之前的本地收集快照，接手以 collected-final 为准。

本例由既有 `evals/semantic/explanation-case.mjs` 生成，没有增加案例整理框架。身份是 `actual_reader_use`，状态 `pending_diagnosis`。所有转换均留在当前 trace 目录；没有改线上或发起模型请求。

## 事实

1. 原页首次生成看到九类方法目录，方法阅读工具可用；完整 trace 中 read_method 为0，保存结果 method_refs 明确为空。
2. 原页有机制表达：s4 已有 pairSvg query-key 配对格子、S/P/B 控件、长方形与三角形计数解释。s5 表下也解释 m 从8192变1、N_pair 变8193、m_out 保持1。不能概括为“整页没有图”或“没有计数解释”。
3. 截图关注的 s5 汇总区包含三条计数轴和分模块 FLOPs。该汇总没有与前文 s4 现有图形直接联动，需要读者跨段对应，这是本轮教师诊断的具体落点。
4. 真实首次追问要求把矩阵讲清楚、画出来，引用的是“第三笔·单节点”。它能证明存在理解障碍，但不能单独证明用户就是在批评 s5 的数字汇总。
5. 原始页面已真实交付，配套回合仍 incomplete / PROVIDER_ERROR。案例整理器的 original_delivery=confirmed 只确认 answer_view 确实交付该版本，不表示教学审核或学习成功。

## 候选改进与待验证问题

先增强目录的适用线索，使作者在遇到“已有机制图，但后续参数汇总需要跨段心算对应”的问题时，能选中并读取方法；再用具体反例和修订示范，展示怎样把汇总数值与机制图联动，或在汇总旁复现一个紧凑的小规模关系图，保留精确数表供查阅。

这是候选教学改进，不是已证实的因果结论。现有 trace 无法证明未读取方法就是效果差的唯一原因，也无法证明某方法已用却无效。是否能改善同模型对未示范新题的首次生成，需要另行 A/B；本案例不记方法收益或学习收益。

## 材料来源与缺口

`case-materials/original.json` 和 `history.json` 是获准 LangSmith 导出的字段投影，逐字段记录 `_provenance`，不是线上 store 的原始文件。owner、turn 来自实际首次 Provider Tutor context，且和后续真实保存状态回执一致；history 的 completed 来自原始 runtime execution_state，answer_view 原样保存。HTML 来自完整 write 加真实四条 CSS patch；未导出的 source_bindings 等 store 字段明确缺失。

source 保存7段实际成功读到的文本范围及原run定位，没有补全全书。Provider 记录按既有格式保留24个真实 LLM span，移除 reasoning_content；其中2项缺usage，合计token保持null，不以估计值补齐。本次新增模型请求为0。

真实 reader-use 材料还包含当时的确切 saved receipt 与页面现场，但它属于追问，不能冒充产品投票。产品 feedback_current / feedback_previous / feedback_scene 均 missing。repaired 仍 missing；当前离线教师范例不写作历史修复。

最终自动输出 gaps：

- repair_delivery_unconfirmed
- revision_difference_unavailable
- feedback_missing
- feedback_scene_missing

这些缺口不妨碍把它作为真实使用的待诊断材料；不把它包装成已证实修复收益的完整案例。
