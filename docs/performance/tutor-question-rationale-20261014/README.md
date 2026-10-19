# Tutor 出题依据与条件解析

2026-10-14。本地实现、工程验证与固定内容样本验收完成。 2026-10-15 已随 `current-20261015` 发布，见 [上线记录](../current-release-20261015/README.md)。

## 行为与实现

指导版本 `tb4.v4`。`crates/runtime/src/tutor.rs:COMMON` 要求用于判断理解的问题先明确具体判断、决定性条件、受支持的答案及可核对的简短理由。公开学习目标描述要判断什么，预先准备的解答保存到既有私有解析或评分合同。选择题使用同一判断维度的可信推理路径；选中某项不足以证明选择它的原因。

按请求或揭示规则提供解析时，连接条件、关系与结论，必要时说明易混方法在哪个范围内有效。当前原文、帮助条件和用户真实表达继续决定解释范围。沿用现有 TutorLoop、TeachingTarget、AssessmentContract 和帮助记录。

`target_input_schema` 明确 `expected_performance` 对于问题不得写出答案，因为该目标会公开。`input_examples` 的平均速度题保存完整私有解析，区分全程平均速度与移动期间平均速度；普通时间比较示例也连接同路程、时间公式和耗时结论。

设计依据见 [ADR-0164 §9](../../adr/0164-tutor-mission-and-cognitive-building-blocks.md#9-出题依据与解析) 与 [行为合同](../../切片方案-Tutor使命与认知积木.md#2-目标与行为合同)。参考了 StudyHub 的 [答案准备](https://github.com/EricWang1358/dsh-web-studyhub/blob/main/lib/generation.js) 和 [条件解析](https://github.com/EricWang1358/dsh-web-studyhub/blob/main/lib/assessment-quality.js) 思路。

## 工程验证

环境沿用 Rust 1.95.0、`D:/codex-build/understand-book-tb-195`、test/dev debug=0、incremental=0、单构建任务及 runtime codegen-units=16。

| 验证 | 检测的具体失败 | 结果 |
|---|---|---|
| `tutor_tb2_final_request_keeps_mission_scoped_to_active_teaching` | 指导未进入实际请求、重复出现或在暂停状态仍生效 | 最终版本 1 项通过，见 [日志](runtime-final.log) |
| Server `tutor_to3_` | 发布示例被拒绝、私有解析提前暴露、揭示文本不一致、后来的帮助改写原始作答条件 | 5 项通过，见 [日志](server-tests.log) |
| `generate-tool-reference.mjs` 内置导出验证 | 最终状态 schema／Native／ReAct 投影不接受示例，文档未消费当前契约 | 最终版本两项导出检查通过，生成 17 种 Tutor 状态，见 [日志](tool-reference-final.log) |

Server 测试通过真实存储和交付入口验证：公开活动与 display 回执没有私有解析；显式 reveal 返回原说明；HelpDelivered 与 HelpDisplayed 分开保存；之后显示解析不会改变先前回答的 `assistance_refs`。`reveal` 沿现有合同允许用户主动请求，`after_submit` 控制评分反馈，不表示提交前禁止揭示。

## 固定内容样本

三组均复用 [TB1 Rust 借用原文](../tutor-building-blocks-tb1/source.txt)，整段绑定为 `1.1`，背景与用户请求是编写的实验输入。各组通过无父会话历史的独立 Codex 子代理生成一次，出题组局部修正后补跑一次，共 4 次生成，继承当前模型与推理设置。工具未提供具体模型版本、token 用量或费用。输入保存当时的指导、当前导出工具契约及完整原文，输出保存实际 `select` 和中文教学回答。

| 样本 | 观察结果 |
|---|---|
| [首轮出题](question/output.json)，[输入](question/input.json) | 生成“先移动非 Copy 值，再借用新所有者”的组合判断，并要求选项及理由；题面保留推断，但 `expected_performance` 写出了答案和完整理由。该字段随活动公开，因此此样本未通过答案边界。 |
| [错误反馈](feedback/output.json)，[输入](feedback/input.json) | 区分 Copy 的按值复制与共享引用的访问权限，指出实际错误连接；用独立可变副本说明学习者的思路在哪种操作下成立，额外规则明确标为补充。 |
| [直接讲解](direct/output.json)，[输入](direct/input.json) | 完整回答用户要求的五点，复用已陈述的移动知识，补充规则保持来源差异，没有要求先作答或把交付解释记作掌握。 |
| [出题补跑](question-repaired/output.json)，[输入](question-repaired/input.json) | 生成借用移动后新所有者／旧绑定的对照题。公开目标描述比较关系，题面提供必要条件并要求具体理由；答案与判断依据进入私有 `explanation`。四个选项沿是否能编译这一维度展开，解析保留“只选正确项尚未确认理由”。 |

首轮失败后只补充公共目标与私有解答的字段边界，没有改动原样本。随后使用同一来源、用户请求和独立生成方式，仅补跑出题组；[补跑输入](question-repaired/input.json) 保存修订后的指导及契约。原始失败与首轮日志保留。

反馈中的两段实际生成代码通过编译器核对，见 [代码验证](feedback-code-check.json)：实现 Copy 后通过 `&Foo` 写字段报 E0594；经 `let mut own = *bar` 复制并修改独立值后，输出 `foo = Foo(0), own = Foo(1)`。只添加了外层 `main` 与用户给出的类型声明，生成语句保持原样。

补跑题目的两段实际代码也通过编译器核对，见 [题目验证](question-code-check.json)：借用新所有者可编译，输出两行 `Foo(0)`；借用已移动的旧绑定报 E0382，支持该题的 B 选项。仅将题面共用类型定义拼到各段 `main` 前，代码保持原样。四份 [Rust 样例](code/) 可用 `rustc +1.95.0 --edition=2021` 独立复核。

## 已知限制

- 这些是固定输入的内容样本，真实学习者为 0，不能推断理解改善、迁移能力、成功率或稳定成本变化。
- 本轮没有生产 Provider 调用或模型自主工具循环验证。工程测试中的模型回执是脚本输入；内容样本与真实工具交付验证分别报告。
- 当前采用指引与字段说明约束语义，没有程序化证明每次生成的题目都具备区分力或没有答案泄露。
- 本次已随 2026-10-15 版本发布；此前 `tb4.v3` 的发布记录见 [理解问题区分力](../tutor-question-discrimination-20261014/README.md)。
