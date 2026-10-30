# 《失败的逻辑》空窗口故障与恢复

2026-10-29。原构建为 standard_deep、Pass2 disabled、并发 8、无 token/wall 上限。保持原 `full` 质量门：Pass1 有节点窗口覆盖率至少 80%，低信息量规则命中率至多 10%。

## 已确认的原因

原始 EPUB 没有 h1–h6，277 个 spine HTML 为分页碎片，NCX 只有首页一条导航，不能据此恢复真实章节。导入得到 1,717 个顶层叶，旧窗口算法每个顶层立即封窗，形成 1,717 个单叶窗口。全部提交后 1,285 个窗口有节点，432 个为空，覆盖率 74.8398%；产出 6,050 个节点，完整性和输入证明全部通过。

432 个空窗口逐一分类：图表数值/刻度 226，页眉页码 105，图片 23，标题/目录 18，出版元数据 15，图示/OCR 碎片 10，注释小标题 8，参考文献碎片 2，公式碎片 1，需要相邻上下文的正文残句/引导句 24。空窗口长度中位数 4 字符，428 个不超过 40 字符。

连续执行批次第 1–4 位的空结果分别是 103/430、107/429、111/429、111/429，未发现后续位置异常。静态输入链路为各工作单元重新绑定输入、generation 和 lease；窗口/导入缺陷自 2026-06-22 已存在，不是 2026-10-28 插件新增。新版发布测试只覆盖连续执行协议，没有真实模型质量对照，本次不将其误称为语义质量证明。

另有两个同源完整性问题：原 EPUB 110 张图片中，仅 23 张被导入，行内容器及含图表格使其余 87 张丢失；无结构容器时 BookStructure 只选择首个叶子，单独合窗不足以保证后续全文覆盖。

## 修复范围

- `epub-adapter.ts`：递归保留行内容器中的图片与文本顺序；含图片的布局表格按行/单元格顺序保留图片及相邻文字，普通文本表格保留原路径。
- `segment.ts`：无标题的连续来源叶归属匿名 section，不添加来源文字或伪造章节，后续按真实 LID 祖先关系覆盖和导航。
- `window.ts`：连续根叶使用现有预算合窗，真实容器保留边界；超预算根叶完整独占一窗。
- `pass1-reduction.ts`：改变输入的路由使用前向 generation，旧已接纳成果保留，修正输入重新抽取。

## 验证与恢复状态

来源与隔离修复已完成。窗口 5 个新增回归先失败、修复后 14 项通过；嵌套图与含图表格 4 个回归先失败，adapter/segment/cover 联跑 20 项通过；无标题结构与下游发现覆盖 32 项、下游来源/Pass1/profile 检查 67 项通过（这些组存在重叠，不累计为独立总数）。Core 类型检查通过。

真实 EPUB 只读验证：110/110 张图片、缺失图片资源 0、无效 span 0、空白块 0；原始 122,135 个非空白正文字符及顺序全部保留。修复后 1,822 叶、23 窗，窗口预算仍为 12,000 token / 80 叶；窗口估算 token 最小 2,191、中位 4,817、最大 8,063，分区遗漏/重叠均为 0。

前向 generation 的集成回归先复现旧 namespace 的 preparation blocked；三个 Pass1 generation 改为 v2 后 preparation ready，新输入全待抽取，旧 task/receipt/artifact 逐字节保留。prompt/schema/full 语义合同不变。Pass1 reduction 15 项通过；handoff 16 项均取得通过证据，其中一个子进程测试批跑超时，单跑 4.8 秒通过。

候选 Windows 引擎已通过既有打包入口构建，实际 `protocol-doctor` 的八项检查均 compatible。独立审阅在冻结补丁快照再次验证真实图片/正文顺序、全叶分区、BookStructure 覆盖及 32 项测试，结论 NO_FINDINGS。

修复版已激活到原指定引擎位置，旧程序重命名保留，不终止已有进程。原 invocation 的普通 step 成功进入新 Pass1，首轮发现 29 个模型工作单元并派发 8 个专用执行器；此计数包含预算拆分片段，后续汇总任务可增加。真实语义质量门和整书最终结果仍待正常构建完成。

### 正式构建进展

修复后的 Pass1 已通过原 `full` 质量门并完成，35/35 个模型工作单元（含片段与汇总）提交。内容侧车随后完成并通过阶段验收，187 个任务执行完毕；原 invocation 已进入 BookStructure 的提纲阶段。整书尚未完成，后续以 Engine 的 `DONE` 为最终依据。

后续 BookStructure 已完成 80/80。2026-10-29 用户要求停止正式知识对象及下游构建、直接发书，当前已完成成果已在线发布并验证，见[发布记录](../logic-of-failure-publication-20261029/README.md)。原完整计划未声明 `DONE`，不得自动恢复用户已取消的阶段。

恢复沿用原 EPUB、原已确认 BuildPlan 和原 invocation；当前尚未发布 source/base/manifest，正常引擎会从原 EPUB 重新生成来源与任务。禁止手工改成果、清空旧状态或降低质量阈值。

### 章节写入失败的诊断修复

BookStructure 提交至 73/74 时，章节 inspect 动作用尽三次尝试，表现为 `artifact_writer/internal/writer_failed`。禁止发布的保存候选重放定位到 `applyStructureChapterAction` 的 `inspect requires a browsed chapter candidate`：当前目录 521 项均已浏览，本次请求的 6 个引用中有 1 个不属于目录。拒绝该候选是正确行为；缺陷是直接抛出普通 Error，将可纠正的候选引用错误归为内部写入故障，丢失字段及纠正反馈。

`book-structure-planning.ts:applyStructureChapterAction` 保持原 seen_refs 成员检查，以现有 `ExtractorContractError` 返回 `schema_invalid`、`/refs` 和逐字复制当前 index/seen_refs、先浏览再 inspect 的纠正信息。仅改变失败反馈，不改 prompt、schema、引用范围或质量门槛。新回归先失败，修复后该文件 12 项通过；Core 类型检查通过。测试同时覆盖目录中未浏览引用、未知引用、当前页合法引用，以及拒绝时工作状态与证据账本不变。

真实保存候选在修复后仍被拒绝，准确返回可纠正的 `/refs` 失败。维护入口 `.codex-recovery/logic-of-failure-writer-replay.ts` 只调用现有接纳验证，不写 semantic artifact；通过现有 scope/terminal 绑定的 `recordAutomaticBuildWriterDiagnosticCorrection` 记录前向诊断修复，保留原失败、候选和全部已接纳成果。修复引擎已激活，并沿原 request 的 `retry_current` 恢复，结果以当前控制 checkpoint 为准。
