# CS336 / AI Infra 计算核算页方法使用调查

## 结论

原始生成回合看到了方法目录，但没有读取任何具体方法。不能把这个案例归因为“读了某个模板后仍写不好”，也不能把目录已进入上下文当作方法已被使用。

## 页面定位与任务

- 原始用户任务：先帮我建立一个2.1节的整体架构的心智模型吧，我希望给我演示页啊。
- 原页标题：2.1 计算图与前向执行：一张图，三笔账。
- 原始 trace：`01a15356-3941-7393-aa0c-c77c956cbac1`，2026-10-19 08:45:22 UTC。
- 原始保存引用：`presentation-1792399783087560643-73@1`。
- 截图中的词嵌入 / ×36 注意力 / ×36 前馈 / 词表头，8192 与已有8192再补1的切换，以及113.8 TFLOPs /19.8 TFLOPs /1.2 GFLOPs /133.6 TFLOPs，均在该版本对应核算区。
- 截图自身没有版本标识，不能仅凭像素断言截图拍摄时就是 rev1。能确认这块内容的原始生成来源；后续 QKV 审核实验用的是同一页面 rev5。

## 方法输入与调用证据

完整原始 trace 查询返回69条，无下一页，39个 tool span、24个 LLM span；根观测标记 complete / dropped_count=0。

首次 author Provider 请求 `01a15356-3955-7923-a817-961eabb18732` 的 `inputs.messages[5]` 是实际发送的 system message，包含 `Presentation method directory`。该目录为9类方法，含 chart-reveal@2、part-whole@2、process-reveal@2、compare-one-factor@2等。工具 schema 同时提供 `read_method`。

整轮 `read_method` 调用数量为0。三次 write（先局部试做，再全页，再带状态完整页）及成功 patch 的返回值都明确是 `method_refs: []`；不是字段未记录。具体调用清单在 trace-evidence-summary.json。

因此可确认：目录暴露已走通；本轮没有通过方法入口读取 card.md、example/index.html、example/README.md 等具体材料，没有声明实际使用任何方法。无法从一个这样的案例证明方法内容有用或无用。

## 可复核材料

- `original-trace-source.json`：授权从 LangSmith 只读导出的完整原始 trace；含私有会话与原始模型记录，仅供核查。
- `original-sampling-guidance.txt`：首个 Provider 请求的实际 system message。
- `original-method-catalog.json`：从该 system message 原样解析的目录。
- `original-presentation-tool-schema.json`：同一请求的实际工具 schema。
- `original-task.json`：原始任务与第一次读者追问。
- `trace-evidence-summary.json`：无长页面或隐藏推理的精简证据。
- `original-revision1-reconstructed-from-exact-write-and-patch.html`：完整第三次 write HTML，按随后真实4条CSS补丁逐条替换而成。每条旧值恰好出现一次。不是重新生成的页面。

## 交付与适合的 RSI 案例身份

原始回合 `execution_state=completed`、`delivery_state=delivered`，`answer_view.parts` 确实附上rev1。但总结果 `status=error`、`PROVIDER_ERROR`、`incomplete=true`；配套教学交付未完成，不能称为成功通过审核的首次生成。页面已保存并交到读者手里，这两件事不矛盾。

首次追问 trace `01a1549a-1ff1-7b93-b624-3f05a0a03050` 明确附带同一rev1页面上下文，用户说“这个矩阵那个矩阵我看不懂啊，可以把所有矩阵都画出来吗”。这是真实读者使用证据，但不是产品里的喜欢/不喜欢投票。

本页适合进入既有 `explanation-case.mjs`，以 `actual_reader_use` 的失败页/待诊断案例身份保留。原任务、源文、原交付、原样目录、明确空方法引用及用户追问均有依据；产品反馈票据、确切截图回执、修正版/修复交付没有就保留missing，不伪造。新教师修订若是离线制作，应标为本轮教师示范，不能写成历史真实修复。

既有整理器只会把存量store结构的完整版本/历史认作交付链：当前 LangSmith 原始输出已包含 answer_view 与完成状态，可以明确标注来源地转换为其要求的材料结构；若不做这种有来源的转换，保持原交付确认项缺失即可，不应伪称直接导出了线上版本存储。

## 本轮改进的含义

这个案例首先支持改进选用入口：让作者在“数值齐全但数量关系靠读者心算”的障碍下选中合适方法，并实际读取图例和改写依据。把反例加入方法文件仍有价值，但若作者不读取，不能期待其中的改法进入下一次生成。


## 案例整理后的补充定位

正式接手以 diagnosis.md 和 collected-final/case.json 为准。原页 s4 已有配对图及计数解释，s5 也有文字解释；本轮改进目标是 s5 汇总与已有机制图之间的跨段映射，不是补一个整页不存在的机制。首次用户追问引用第三笔单节点，并不单独指向 s5。
