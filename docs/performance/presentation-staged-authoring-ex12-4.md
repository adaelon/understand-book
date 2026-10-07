# EX12.4 关键局部、全篇生成与提示对照

2026-10-02。状态：实现、真实对照和局部纠错已落盘；**EX12.4 最终原生交付与内容验收已在 EX13.6 完成。** 同对象 revision 2 已交付并通过三环境七路径；原 402 失败及离线修正记录保留。详见 [EX13.6](presentation-context-ex13/ex13-6/README.md)。合同见[切片方案](../切片方案-演示页全局框架与分阶段制作.md)与 [ADR-0154](../adr/0154-presentation-global-framework-and-staged-authoring.md)。

本报告原对照使用方法 `ex12.v5`，最终接续使用 `ex13.v2`。正确归档配置的阶段组实际完成了全局→关键局部→预览反馈→展开→串读→交付；范围遗漏仍需外部反馈。共同底座修复了触摸来源按钮、来源列表错误反馈和 Flash 压缩输出预算。累计保留 25 次真实运行、326 条 chat 响应记录，已知总用量 28,109,986 token（包含缓存输入；不是费用估算），明细见 [run-summary.json](presentation-staged-authoring-ex12-4/run-summary.json)。

## 输入与比较方法

原问题沿用 EX12.0 的两轮逐字请求、第一轮 53 条消息、上一轮回答与确切交付 `presentation-1790851114402727998-8` revision 1。读取原 admission 的 reader snapshot 和 native / `deepseek-v4-flash` runtime profile（初始装配的 resolution 偏差及修复见下）；使用原发布 `01a0f58a-3382-7e03-890a-e3e36adfe89e` 的 base、source 和 manifests，以及归档的七张原图。冻结输入见 [inputs.json](presentation-staged-authoring-ex12-4/inputs.json)、[history-before.json](presentation-staged-authoring-ex12-4/history-before.json)、[材料回执](presentation-staged-authoring-ex12-4/material-receipt.json)。原五张参考图仍由 [EX12.0](presentation-staged-authoring-baseline.md) 索引；不把原模型未收到的参考图加入某一组请求。

另外两项自然请求在运行前选定，均使用同一发布 1.3 材料：

- 预测后观察：先判断从单请求变成一批八个请求后，单请求等待和整批输出速度会怎样，再观察结果并解释。
- 静态概念关系：显存已放 70 GB 权重，为何下一 token 还要读取 70 GB；区分驻留量、每步读取量和运算量。

测试入口 `presentation_ex12_tests.rs:ex12_live_comparison` 调用原 `prepare_agent_chat → execute_prepared`。模型、工具 schema、候选存储、来源门槛、预览和图片资格均使用同一实现；只在测试 adapter 发送请求前替换演示指导。旧组使用归档 `ex11.v11` 正文及原 author policy；新组使用当前阶段资产。两组都收到相同 scroll 使用说明，并保留相同 prepare 工具。普通文字回答可以不激活演示能力。

每组每题首跑一次；失败不覆盖。首批指导保存在 [guidance-initial](presentation-staged-authoring-ex12-4/guidance-initial/presentation/README.md)，所有实际 instruction assets、当前框架、消息、工具、图片说明、响应、候选、usage 和耗时保存于各运行目录。`summarize.py` 直接比较两组 admitted messages/message/reader/profile，产出 [run-summary.json](presentation-staged-authoring-ex12-4/run-summary.json) 与各组 `audit.json`。

## 首批结果（ex12.v3，共同样式修复前）

| 请求 / 指导 | 模型 chat / structured | 工具调用 | 已知总 token | 耗时 | 结果 |
| --- | ---: | ---: | ---: | ---: | --- |
| 原问题 / 旧 | 13 / 2 | 41 | 686,931 | 424.05 s | 未交付；未知来源引用后，补交 write 缺 title，最终 AGENT_NO_PROGRESS。 |
| 原问题 / 分阶段 | 22 / 1 | 52 | 2,251,399 | 568.59 s | 交付完整长页，但没有先试做关键局部，也没有切换 local/review。 |
| 预测 / 旧 | 6 / 0 | 18 | 104,299 | 40.04 s | 完成先判断、再看结果的文字说明。 |
| 预测 / 分阶段 | 20 / 0 | 33 | 1,470,230 | 418.45 s | 交付选择预测与结果解释页；始终停在 global。 |
| 静态关系 / 旧 | 23 / 0 | 53 | 1,759,756 | 452.46 s | 制作了交互候选，最终缺 short-content 预览，重复 deliver 后 AGENT_NO_PROGRESS。 |
| 静态关系 / 分阶段 | 19 / 0 | 41 | 1,410,684 | 401.24 s | 交付长篇参数实验，始终停在 global。 |

总 token 包含实际 chat 与 structured usage；reasoning 是输出 token 的子集，不重复相加。并发运行下耗时是该次墙钟时间，不是提示性能的稳定估计。

**阶段交接问题。** `prediction-staged/response-05.json` 与 `static-staged/response-03.json` 提交 global 后直接 write；原问题 `mechanism-staged/response-07.json` 也只提交 global。实际后续 request 的 instruction assets 仍为 global，没有所需局部能力参考。框架有记录，制作职责没有发生切换。

**原问题的画面与完整性。** 首批阶段组将 token、逐步依赖、单位和三个角色放在公式之前，见 [单位段](presentation-staged-authoring-ex12-4/mechanism-staged/independent/960-01.png)。窄屏分块图与按钮可辨认，见 [320px 局部](presentation-staged-authoring-ex12-4/mechanism-staged/independent/320-08.png)。参数区与后方比较图仍分离多屏；页底虽有术语与算例速查，但缺少原材料练习 1-3/1-4 的完整任务。它不满足本切片的完整验收。

同一页的[重叠旁注](presentation-staged-authoring-ex12-4/mechanism-staged/independent/960-07.png)把“相加与重叠”的差说成约 0.0177 ms。按页面的八块模型，相加为 21.0370 ms，八块重叠为 20.9132 ms，两者相差约 0.1238 ms；0.0177 ms 是八块重叠与理想 max 下界的差。数值行与解释文字不一致，不能把工程预览通过当作计算解释通过。

**两类扩展题。** 预测页确实提供判断选项、揭晓反馈及同尺度条形图，见[判断](presentation-staged-authoring-ex12-4/prediction-staged/independent/960-01.png)与[对照](presentation-staged-authoring-ex12-4/prediction-staged/independent/960-02.png)。静态关系页的[概念卡](presentation-staged-authoring-ex12-4/static-staged/independent/320-03.png)能分清三种量，但继续扩展为多个参数、时间下界、硬件变化、batch、练习，超出了这道窄问题所需的表达范围。初始算力滑块 DOM 值为 989000 GFLOP/s，读数及计算使用 989400 GFLOP/s，控件与当前设定也有不一致。

## 反馈后的修订

**方法资产 ex12.v4。** 全局职责明确用独立 prepare(local) 交接框架、关键局部 focus 和 needs；下一次采样再制作。局部职责明确取得关键局部反馈后展开完整范围，再 prepare(review) 串读。共同正文明确按当前问题确定范围，静态概念可以用文字或静态对照完整说明；参数和动画须解决具体未澄清的疑问。工程合同同步这些调用顺序。Runtime 不新增阶段强制拒绝门槛。

**共同触摸样式。** 当前宿主 CSS 对来源按钮施加 `min-height:36px !important`，短标签还可能小于 44px 宽，与三环境预览要求的 44×44 冲突。原问题阶段组多次失败后自行增加更高优先级样式才继续。真实浏览器最小用例在 320×420 报 `29.0625×36`，见[失败日志](presentation-staged-authoring-ex12-4/source-touch-red.log)。将 coarse pointer 来源按钮改为最小 44×44 后，三环境通过，见[修复日志](presentation-staged-authoring-ex12-4/source-touch-green.log)。该修正属于共同底座，不能算成提示收益。

## 复验结果与第二次修订

v4 原问题两组均交付。旧组 18 次 chat、2 次 structured、52 次工具调用、2,327,670 token、709.86 s；阶段组 26 次 chat、1 次 structured、47 次工具调用、2,759,909 token、540.43 s。阶段组在 response-06 切换 local，首个 write 仍一次写出 44,113 字符的完整页面，最终没有 review 交接。它把“完整 HTML 页面”的技术要求混同于“先写完整作品”，没有完成关键局部试做。

v4 原问题的[读取/计算对照](presentation-staged-authoring-ex12-4/mechanism-staged-v4/independent/320-28.png)在线性轴下保持真实比例，提供对数轴查看短条；全篇仍缺练习 1-3/1-4 的完整任务。[边界段](presentation-staged-authoring-ex12-4/mechanism-staged-v4/independent/960-34.png)又把加入状态读取项后的转折说成“比 147.7 更早”。按它自己列出的公式，若其余量固定，B* = R_W / (βF₁/Π − R_state,1)：增加非负状态项使正分母变小，交点应后移；分母不为正时不存在计算追平读取的正交点。该页解释不成立，未选为验收版本。

v4 预测组切换 local 后，write 引用了刚观察到的来源，却没有填写 source_ref_ids。错误只显示 UNKNOWN_SOURCE_REF，模型反复 source.present 并重复同样的 write，最终 AGENT_NO_PROGRESS。9 次 chat、27 次工具调用、364,003 token、225.67 s，无交付。首批原问题旧组也出现相同漏项，因此这是共同工具反馈问题。

v4 静态关系组 20 次 chat、35 次工具调用、1,512,932 token、425.35 s，交付。global→local 已发生，但首个 write 仍为完整长页，增加的 batch 转折实验继续超出这道概念区别题所需范围。对稳定关系的形式选择不能宣布已经可靠解决。

**方法资产 ex12.v5。** 区分“可独立运行的 HTML”与“完整作品范围”：关键局部 focus 的首个候选只包含目标关系及必要前提，其他段落留在框架；先看实际结果再展开全文。工程说明明确每次 write 的来源附着列表。

**共同来源恢复反馈。** schema 说明 source_ref_ids 必须显式列出所有引用；UNKNOWN_SOURCE_REF 保留拒绝并提示补充 write/patch 的来源列表，已有来源不必重新登记。未观察来源仍拒绝，来源权限与交付门槛不变。用例覆盖漏项提示、补齐后保存及没有来源权限时拒绝。

v5 只重跑原问题两组和受来源问题影响的预测阶段组。v4 指导快照保存在 [guidance-v4](presentation-staged-authoring-ex12-4/guidance-v4/README.md)，所有失败及中间产物保留；不同底座结果不混算提示收益。

**v5 同底座结果。** 原问题旧组 9 次 chat／1 次 structured／27 次工具调用／746,384 token／365.08 s，交付；阶段组 22 次 chat／1 次 structured／51 次工具调用／2,400,160 token／579.52 s，交付。阶段组 response-05 记录 global，07 进入 local，08 保存 18,837 字符的预算计算器原型，09 发起真实预览，10 开始补全余下部分；13 全写完整候选后沿长页观察，16/18 等继续修图，19 主动 prepare(review)，request-20 确实加载 review 指导，随后 response-20 直接交付。首次实现了关键局部先行及串读职责交接；框架和最终内容仍漏掉两项练习与 1.3.4，②第5条把补回忽略项概括成“更早转向受计算限制”，也不成立。review 交接沿用了自己的八站框架与“已完成验证”摘要，没有在交付前找出这些遗漏。独立观察中 320px 五个参数的文档位置从 y≈5206 到 5878，相关结果还在下方，多屏操作问题仍在。

2026-10-02 复核说明：此前本段将 v5 误记为“没有主动 prepare(review)”；现已按 response-19 和 request-20 更正。此证据表明，阶段交接完成仍不足以保证范围完整或解释正确。

v5 预测阶段组采用普通文字完成，5 次 chat、14 次工具调用、88,973 token、43.86 s；没有进入 author，因此不能把这次当作来源修复的真实 write 复验。正文在“先自己判断”标题下直接给出答案，没有保留先判断的机会；保留为形式选择未满足需求的样本。最初 prediction-staged 的交互页面通过独立错误／正确选择和 B=8／256 操作，作为这一关系的有效产物证据。

**明确反馈后的接续。** 在 v5 原问题与 v4 静态题的确切版本上，分别追加 [原问题反馈](presentation-staged-authoring-ex12-4/mechanism-feedback.json) 与 [静态题反馈](presentation-staged-authoring-ex12-4/static-feedback.json)。原问题反馈指出缺项、错误推论与多屏操作，并明确要求进入 review；静态题反馈只要求收回原概念范围。使用同一模型与 v5 指导，复制已交付版本和会话到独立目录继续修订；这些是有外部反馈的修复，不是重新抽样，也不计入提示独立收益。

**接续时的共同压缩失败。** 两个 feedback 运行都在进入修订制作前报 COMPACTION_FAILED / Provider unfinished response: length。原问题压缩耗时 58.51 s、16,384 输出 token，其中 reasoning 14,345；静态题为 59.30 s、16,384 输出，其中 reasoning 13,624。原始 completion request/response 与失败 outcome 保留。针对实际使用的 catalog DeepSeek Flash 档案，将压缩输出预算从 16,384 调至 32,768，并让压缩输入空间扣除同一个预算；其他档案不变，不加入重试框架或放宽摘要校验。feedback2 从同一已交付版本与相同反馈重新接续，单独记录这一共同底座修复。

**配置装配偏差及修复。** feedback2 的实际 completion request 仍是 16,384，定位发现 `adapter_from_config_with_runtime_profile` 会将归档 CatalogMatch 改写为 ExplicitOverride。初始与 v4/v5 的两组使用了同样的模型、材料、工具及该显式配置，但没有启用 CatalogMatch 专用的 131,072 演示输出预算，因此不能把它们说成原配置的精确复现。记录 adapter 改为向原循环返回未经改写的归档 profile；live admission 增加完整 profile 等值断言，定向测试同时确认演示 131,072 和压缩 32,768。此前所有请求保留，不回写其元数据。另以 `mechanism-*-catalog` 补跑原问题同底座对照；feedback3 从此前选定版本和同一反馈继续，仍单独记录。

正确 profile 下，feedback3 的原问题压缩已完成：25,916 输出 token（其中 reasoning 11,803），确实超过原 16,384 上限；静态题压缩也已完成。这里的证据是实际完成的模型响应，不只是假 adapter 预算断言。

**补充定性请求。** v4 运行前另行冻结“模型漏项与实现开销如何区分”，来源为同一节实测材料（3.6.5.1）。旧／新两组均以文字完成，没有激活演示工具；分别为 6 次 chat／16 与 14 次工具调用／109,096 与 77,955 token／35.34 与 34.20 s。该样本说明普通文字路径仍可使用；两组都没有加载演示指导，不能算成新指导改变形式选择的证据，也不能替代原静态题的失败记录。

## 正确归档配置的对照与静态题收口

| 原问题 / 指导 | chat / structured | 工具调用 | 总 token | 耗时 | 结果 |
| --- | ---: | ---: | ---: | ---: | --- |
| 旧指导 / catalog | 15 / 1 | 37 | 1,404,690 | 433.10 s | 触摸控件修正后，最终未满足截图后续采样检查要求；重复 deliver，AGENT_NO_PROGRESS。 |
| 阶段指导 / catalog | 31 / 1 | 75 | 2,947,326 | 527.88 s | 局部原型→预览修正→展开→prepare(review)→交付；仍漏两项练习。 |

两组 admitted profile 与归档逐字段相等，已激活作者的实际 131,072 输出上限。阶段组 response-12 将全局框架交给 local，13 写参数实验台，后续实际观察并修正触摸和图形问题，28 进入 review，29 交付。它证明原循环能跑通阶段链路；范围完整性仍需外部反馈，不以交付成功替代内容验收。原问题的最终选定版本继续来自事先指定的 v5 页面反馈修订，未在这次新样本中挑更好的一页。

静态题反馈修订交付同一对象 `presentation-1790918303334663100-4` revision **2**：范围收回驻留量、每步读取量、运算量，删去批量转折与算力实验。保留一次执行计数的交互，直接显示驻留量不变而累计读取增加。15 次 chat、1 次 structured、32 次工具调用、1,023,380 token、306.74 s。三步后实际读数为 70 GB／210 GB／140 GFLOPs/步，重置后累计为 0 GB，三种视口均通过，见 [static-verification.json](presentation-staged-authoring-ex12-4/static-verification.json) 与 [320px 图文](presentation-staged-authoring-ex12-4/static-staged-feedback3/independent/320-05.png)。这个修订满足原窄问题；不能据此宣称首跑即可自动收好范围。

## 原问题的完整补齐与最终局部纠错

feedback3 已补入两项练习和 1.3.4，进入 review 并交付新对象 `presentation-1790920818511542500-6` revision 1。模型没有为这次 write 填 based_on，因此它不是旧对象 revision 2；记录保持原样。实际读数中，练习 1-3 的原始三档与读取减半后的时间／吞吐均符合独立代入值，但减半后的摘要仍写 B*=105.5，旁注甚至说“105.5 已经落在 64 之前”。

独立操作还发现新加的 sticky 结果区在 320×420 下遮住了正在操作的滑块和标签；pointer-events:none 只让点击穿透，未解决可见性，见 [聚焦 N 时的截图](presentation-staged-authoring-ex12-4/mechanism-staged-feedback3/independent/320-24.png)。练习 1-4 仍要求读者自己从图取另外两档数据，见 [缺项位置](presentation-staged-authoring-ex12-4/mechanism-staged-feedback3/independent/320-16.png)。数值失败见 [mechanism-feedback3-failures.json](presentation-staged-authoring-ex12-4/mechanism-feedback3-failures.json)。

据此追加 [最终局部反馈](presentation-staged-authoring-ex12-4/mechanism-final-feedback.json)：在该确切对象上 patch，修正减半后的转折与旁注、消除遮挡、补齐实际图中四档数据与汇算比值，并同步可读内容的因果说明。四档值取自冻结 SVG 中的实际标注；独立算值见 [expected-calculations.json](presentation-staged-authoring-ex12-4/expected-calculations.json)。这次修订属于成品纠错，不改变提示对照样本。

最终反馈的 feedback4 在制作前再次触发压缩输出截断：54,780 输入 token、32,768 输出 token（其中 reasoning 18,041），待覆盖项 237 个，其中必需 84 个。当前 CatalogMatch Flash 压缩预算因此改为 **65,536**，输入空间同步扣除；feedback5 仍从 feedback3 的同一版本使用完全相同的局部反馈继续。32k 的成功与这次失败均保留，不能把一次压缩通过当作连续修订已经稳定。

feedback5 首次压缩输出 37,269 token，因引用输入中不存在的 source ID 被现有校验拒绝；原有一次修复路径输出 47,810 token 后通过，未放宽覆盖规则或新增重试。EX13.0 直接比对录制后更正此前“重复 source ID”的误记：`item.fact.quote_receipts` 将 source.186 的 `...089cc882f` 写成了 `...088cc882f`，详见[离线基线](presentation-context-ex13/README.md)。模型随后读取确切版本并进入 local，在写入修正前收到 `402 Insufficient Balance`；见 [outcome.json](presentation-staged-authoring-ex12-4/mechanism-staged-feedback5/outcome.json)。本次没有新交付，之后停止真实模型调用。

已在 [local-correction/content.json](presentation-staged-authoring-ex12-4/local-correction/content.json) 完成剩余纠错，并保存针对 `presentation-1790920818511542500-6` revision 1 的 [patch.json](presentation-staged-authoring-ex12-4/local-correction/patch.json)：11 个精确替换及同步可读正文。每个原片段唯一匹配，顺序应用可还原修正内容。

- 权重减半时读取量和交点同步变化，练习 1-3 的 B* 从 105.5 改为 52.7，计算量保持不变。
- 窄屏结果区改为正常文档流，保留控件就近提示；[320px 聚焦截图](presentation-staged-authoring-ex12-4/local-correction/independent/320-24.png)可见标签、控件和提示。
- 练习 1-4 补齐原图 B=1/4/16/64 的吞吐、TPOT、相对倍数与增幅；[实测表截图](presentation-staged-authoring-ex12-4/local-correction/independent/960-25.png)和独立代入值一致。
- 正文同步区分估算与实测、权重读取与计算量，移除修订过程叙述。

七条操作路径在三环境通过，含默认值、B=8、权重减半、练习原值／减半、控件聚焦、四档实测汇算；没有脚本错误或水平溢出。证据见 [verification.json](presentation-staged-authoring-ex12-4/local-correction/verification.json) 与独立观察记录。该文件状态为 `local_verified_not_native_delivered`，观察的 reference 为 null；原已交付版本保持原样。

最终接续已在 [EX13.6](presentation-context-ex13/ex13-6/README.md) 完成：恢复 feedback5 的未完成 Goal 与检查点，以 feedback3 的 revision 1 应用原 11 项补丁及一项补充公式修正，原生三环境预览、模型图片检查、review 和 deliver 后生成同对象 revision 2。独立重放三环境七条操作路径、整份内容与来源比对通过。累计 22 次提供方调用、空闲价估算 0.93951468 元，属于这次最终接续，不并入此前 25 次对照统计。下一片为 EX12.5。

## 验证

检查针对的具体失败是：旧／新组输入不同；旧提示中残留新职责；真实请求未携带当前阶段；原始版本无法读取；来源按钮被共同 CSS 压小；完整内容或计算旁注与实际结果不符。对应动作分别是修实验装配、职责交接、原版本材料、共同样式或生成内容。

- 指导隔离测试通过，覆盖 global/local/review 三种请求，断言工具、输入、profile 与输出上限相同，scroll 正文各出现一次。
- 原首轮历史及版本读取、真实 admission 测试通过。首次 admission 因测试夹具父目录共用旧 learning.db 而失败，尚未调用模型；改为各组独立存储后通过，失败目录和日志保留。
- Runtime EX12 定向 12 项通过，覆盖阶段替换、压缩、同批操作、取消、来源／候选权限、历史和 scroll 投影，见 [runtime-tests.log](presentation-staged-authoring-ex12-4/runtime-tests.log)。
- 触摸来源按钮用例先失败后通过；实际三环境浏览器执行。
- 最终 65,536 压缩预算下 20 项 compaction 定向测试通过，含同一输出预算传递、原子失败、前／中回合压缩、阶段框架恢复与源覆盖；见 [compaction-final-budget-tests.log](presentation-staged-authoring-ex12-4/compaction-final-budget-tests.log)。
- 预测交互的未作答、错误选择、正确选择、B=8、B=256 五条路径在三环境通过，读取实际结果区域核对；见 [prediction-verification.json](presentation-staged-authoring-ex12-4/prediction-verification.json)。
- 最终 profile、指导隔离与来源错误恢复 3 项通过，见 [final-profile-tests.log](presentation-staged-authoring-ex12-4/final-profile-tests.log)；当前指导组装 15 项通过，见 [guidance-v5-tests.log](presentation-staged-authoring-ex12-4/guidance-v5-tests.log)。
- 已交付首批页面按标题顺序完成三种视口独立截图；首批截图未发现水平溢出或浏览器脚本错误。详细记录在各组 `independent/observations.json`，独立截图不授予生产交付资格。独立浏览器统一使用当前修复后的共同 CSS；各模型运行当时的原始截图另存于 request 对应 image 文件，首次 36px 宿主缺陷的证据以原始观察为准。

## 已知限制

EX12.4 最终原生预览、图片检查、交付与内容验收已完成；余额阻塞已解除。首跑范围完整性和表达形式选择仍不稳定，完整内容依赖外部反馈纠错。EX13.6 收尾回答存在练习编号误指，已在其记录中注明；实际页面独立验收通过。

初始、v4/v5 两组的配置彼此相同，但测试 adapter 曾将 CatalogMatch 改成 ExplicitOverride；只有修复后的 catalog 两组才与原档案逐字段一致。各批次分别保留，不能混算为同一配置的提示收益。单次样本只能支持对应产物和缺陷结论，不能据此宣布一般性提示收益或学习效果。

Windows 桌面浏览器缩窄视口验证不等于实体手机测试。Linux 部署、Reader 连续操作、保存重开、后续局部修改属于 EX12.5。本轮不改变正式线上版本。原运行内框架仍不跨轮持久化。
