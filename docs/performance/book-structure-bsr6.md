# BSR6 新书前段候选发现

状态：2026-10-01 完成。源材料框架先行、完整小节发现、程序累计候选已接入正式 Core；完整第 8 章独立生成、原文内容验收与确定性重放通过。合同见 [ADR-0151](../adr/0151-book-structure-global-outline-and-semantic-retrieval.md)。

## 实现

技术书先从 canonical 标题、小节目录、章节开头及小结／第二段原文建立暂定框架，然后开始完整正文覆盖。框架只提供方向；正文和已有 discourse/formula 的实际来源承担证据。paper 保留既有路线。

发现输入读取完整 canonical span，替代旧 1200 字符预览；自然小节拥有连续 core 范围，过长小节使用原叶节点预算分块和辅助材料分片。标题仍计入 core 覆盖身份，但不进入候选证据范围，纯标题任务允许空概述和候选。

发现候选必须带独立 local ID、含义、适用条件、锚定教学理由与定位，单个贡献中 ID 唯一。同一 LID 的不同机制分别保留。每个片段沿现有 generation 独立落盘，目录直接读取原候选；程序只累计概述，不调度候选归并树。后段复用 BSR5 的章节引用选择、主题规划／动作、一次收敛及三清单物化。主题规划读取已经整理的简洁章级摘要。

当前输入接纳检查包含发现任务；针对本章的读取保留完整源材料框架，仅路由本章发现，渲染交付与全量读取一致。CLI、正式 opaque executor 和 sidecar bundle 注册同一发现 prompt。发现进度单列已接纳片段、core 叶节点和候选；关闭要求所有章节覆盖片段已经贡献。

## 确定性验证

2026-10-01 定向 15 文件 76 项全部通过；截止时刻的新接纳检查由实验适配器单项再测通过，仍计为同一测试。覆盖列表与命令如下。

```powershell
pnpm --filter @understand-book/core exec vitest run test/book-structure-discovery.test.ts test/book-structure-discovery-eval.test.ts test/book-structure-append-production.test.ts test/book-structure-production-recovery.test.ts test/book-structure-quality.test.ts test/book-structure-routability.test.ts test/book-structure-evidence.test.ts test/book-structure-reuse.test.ts test/book-structure-optional-pass2.test.ts test/automatic-build-retrieval.test.ts test/automatic-build-policy-generation.test.ts test/book-structure-candidates.test.ts test/book-structure-planning.test.ts test/model-input-routability.test.ts test/automatic-build-retrieval-driver.test.ts --maxWorkers=1
pnpm --filter @understand-book/core typecheck
```

- 完整长段和自然小节：25 个长段、超过旧预览的结尾均交付；长节拆分后 core 无缺口、无重叠且不跨节。
- 接纳与保留：框架先行、纯标题证据拒绝、同 LID 的两种含义、适用条件保留、重复累计和当前候选目录保持完整。
- 正式入口：发现及五类整理任务经 opaque executor 递交、重复提交、质量 gate、发布一次、搬移后恢复；旧成品在关闭前保持可读。
- 恢复与准备：两个 host／transport 的取消、Provider 失败、确认范围、检索模式切换、来源变化时拒绝过时写入、当前目录动作重放和旧回执保留；普通读写不调用 Provider。
- 实验适配器：pending 重用、无效 JSON 计入拒绝和额度、保存响应按原输入重放、候选与所有锚定载荷完全一致，实际 usage 保持未知。

## 真实实验合同

用户于 2026-10-01 明确选择独立 Codex subagent，最多 80 次提交（含失败／重试）、首个 next 起 60 分钟；不调用外部生成 API、embedding，不发布原书。记录：[计划](book-structure-bsr6-20261001/plan.json)、[材料与计量](book-structure-bsr6-20261001/review.json)、[逐步记录](book-structure-bsr6-20261001/session.json)。

隔离目录只复制 source、manifest、profile、base、discourse、formula 公共前置成果；没有旧 BookStructure 任务、章卡、候选或成品。框架涵盖 14 个 canonical 单元，正文只执行第 8 章的完整 653 个叶节点，路由为 40 个发现任务。生成者只读 next 交付的 prompt/input，不继承父聊天，不读验收材料、源码、旧发布或最终报告。每次冻结输入、响应、接纳／拒绝、估算和耗时单独保存。

第一份框架已接纳；发现 prompt 注册／信封修正发生在首个正文输入冻结之后，第二次提交被当前输入 gate 拒绝，随后 next 遇到同 generation 的冻结文件冲突。最终 discovery generation 使用 v2，保留 v1 冻结与失败响应后续建；该拒绝计入原额度，计时未重置。

## 真实结果与成本记录

[完整报告](book-structure-bsr6-20261001/report.json)确认 40 个正文发现任务接纳，653 个 core 叶节点完整且唯一，178 个独立候选直接从原片段重建。框架的 14 个单元来自原文；此处 178 是可引用候选数量，章节选择仍由后段整理完成。

| 观测 | 结果 |
| --- | --- |
| 完整章节覆盖 | 653 / 653；缺口 0、重复 0 |
| 正文任务／候选 | 40 / 178 |
| 语义提交 | 42；接受 41（框架 1、正文 40），拒绝 1 |
| 首个 next → 最后接纳墙钟 | 58 分 52.291 秒（3,532,291 ms） |
| 序列化输入／输出估算 | 186,318 / 38,545 tokens，包含拒绝提交 |
| Codex 实际模型调用／tokens | 未知，分别保存为 null |
| 外部生成 API／embedding | 0 / 0 |

首个 next 为 2026-10-01 14:27:14.553 UTC，最后接纳为 15:26:06.844 UTC，早于原截止 15:27:14.553 UTC。计时包括生成与接纳等待，不包括结束后的验收重放。以上是新书前段发现成本；不与复用旧候选的 BSR2 整理次数或旧全书历史估算做同条件费用比较。

[重放记录](book-structure-bsr6-20261001/verification.json)在新的 tmp 书目录重新接纳全部 41 份保存响应：每步 prompt/input、完整候选目录和所有任务的锚定载荷完全一致，模型调用 0、Provider 调用 0。重放输入与接纳记录保存在同目录 `replay/`，源文件副本留在 tmp，不写入原书。

## 原文内容验收

[内容审阅](book-structure-bsr6-20261001/content-review.json)记录实际候选引用、含义、条件、原文与判断。九类已知机制都以可讲解的候选保留，未压缩为仅容量和有效吞吐两个点；这一判断来自内容核对，不以 LID 命中数量代替语义。

| 机制 | 已保留的关键内容 |
| --- | --- |
| 显存需求与准入 | 权重、去重物理 KV、激活与工作区；42 条短请求生成后 13,608 MiB 超过 12 GiB，接纳前预留生成状态 |
| 合批读取收益 | `D_w/b+Lk` 只摊薄权重；各请求 KV 仍相加，`s_w` 使用 bytes |
| 连续批处理与分块 | 870→766 ms 同时最大输出间隔 26→376 ms；decode 优先分块的 133 ms / 844 ms 取舍及预算 |
| 分页、共享与写时复制 | 地址映射、内容共享和私有尾块分开；释放要求引用归零与设备操作完成 |
| 前缀复用与恢复位置 | 模型／位置／adapter／格式及连续前缀条件；从 8192 快照恢复到匹配 10752 仍重算 2560 |
| 缓存保留收益 | `p_h(T_r−T_f)−T_m`；同容量比较保留命中率、传输与维护成本条件 |
| 压缩与卸载 | 省读取须扣串行转换，收益随长度改变；逐轮 PCIe 搬移约 10.8 秒违反 7 秒期限 |
| 推测解码 | 草稿与目标验证分开；`min(1,p/q)` 接受及拒绝后正差额重采样保持目标分布 |
| 有效产出 | 正确且按时才计产出；饱和时完成吞吐近不变而 goodput 降约 41%；错误／迟到资源仍计成本 |

同一 `10.13.2.11` 的准入判断与取消后回收分别保留为 `fragment:0032#admission-capacity-and-deadline`、`fragment:0032#cancel-after-device-completion`，拥有不同含义、条件与独立引用。最后小结和实验脚注候选还保留测量配置、统计窗口、单向带宽、理论算例与实测区别，避免把局部数字推广为通用规律。

## 已知限制

- 全书发现加整理、全书语义取舍、端到端成本对照和正式安装／Reader 交付留 BSR7；本片不发布原书。
- 暂定框架使用少量源段落，个别摘录在 500 字符边界截断；框架不能代替正文，生成者只使用完整给出的论据。
- 实际 Codex 模型调用量与 tokens 不可得。提交次数、序列化估算和真实墙钟分列，不能据此宣称严格费用下降。
- 本次证明完整章节的前段发现及九类已知机制保留；178 个候选尚未经过本次真实章节／主题取舍，不代表全书质量或候选穷尽性。
- 复用的公共 formula 成果存在五处语义误标，生成者独立发现并按实际交付正文生成正确候选；公共基础成果保持原样，后续定点修正含义、单位及上下文引用。详细原记录和对应候选见内容审阅中的 `upstream_issues`：

| 公式 LID | 公共成果误标 | 正文与新候选 |
| --- | --- | --- |
| `10.8.3.21` | 8192+255 误称短请求 | 长请求，短请求输入为 2048 |
| `10.9.2.13` | `s_w` 误释为每权重元素运算次数 | 每权重元素字节数，BF16 为 2 bytes |
| `10.12.4.7` | 9.9 ms/token 误称串行 | 并行草稿 9.9，所给同输出数串行 13.5 |
| `10.13.4.20` | `r_D` 标为新 token/s | 本节为请求 decode 调用/s |
| `10.13.4.51` | 2.37 秒误称方案 B | D 为 2.37 秒，B 约 2.45 秒 |
