# ADR-0157 以理解障碍选择视觉方法，并检查关键解释时刻

状态：设计已确认，EX14.0、EX14.1-B、EX14.2 与 EX14.3 通用路径完成；EX14.1-M 已完成；Manim 指导与兼容增补已完成；EX14.4–EX14.5 待实施。  
日期：2026-10-08。  
实施合同：[切片方案：EX14 视觉方法指导与关键解释时刻检查](../切片方案-EX14-视觉方法指导与关键解释时刻检查.md)。  
承接：ADR-0133、ADR-0139、ADR-0154、ADR-0155。

## 背景

现有 `presentation-method` 已要求围绕理解障碍选择表达、保持对象身份，并先验证关键局部。ADR-0139 提供可组合的解释方法；ADR-0154/0155 确定了阶段指导、Goal 与运行内解释设计的分工。本次增补落在现有局部制作、评审与技术参考中。[R01–R05]

“关键解释时刻”指需要观察的解释现场，包括动画位置、参数边界、分阶段揭示、长页后段或恢复后的状态。当前 Manim runner 仅取首、中、尾和输入列表中的首个 cue，最多返回四帧；页面 preview 每批最多四个动作且每次重新加载。补查能力同时受抽帧位置和现场可达性约束。[R06–R10]

外部 `3b1b-explainer-video` 提供视觉表示、对象变换、注意力与布局经验。这里采用适合读时解释的设计原则；具体实现遵守项目的媒体与 Reader 合同。[E01–E04]

## §1 指导与状态归属

**决策**：沿用现有阶段指导及状态所有权。
**否决**：
- 独立风格 skill：解释方法需要按当前障碍组合。
- 新建持久分镜或检查资源：现有设计与工作进度已能承载这些判断。
**命门**：Goal 保存用户要求与工作进度，`framework`/`focus` 保存解释设计，页面 state contract 保存读者现场；来源、版本与交付资格沿用 Runtime/Server/Reader 合同。[R01][R03–R05]
**展开**：[EX14 §2–3：代码落点与记录边界](../切片方案-EX14-视觉方法指导与关键解释时刻检查.md)。

## §2 关键现场补查

**决策**：按解释风险选择可达现场进行补查。
**否决**：
- 只凭四帧或 cue 覆盖率判定解释完整：关键关系可能出现在其他位置。
- 跨 preview 累积操作：新页面会丢弃上一批参数与演示选择。
- 穷举全部视口和参数：检查成本随组合增加。
**命门**：每批满足“重建前提 + 目标操作 ≤ 4”才可拆批；无法到达时记录未覆盖原因，工程验收通过 Reader 操作或定向浏览器测试补证，并与 Agent 自行观察分别归档。补证沿用当前候选/版本，正式三环境回执仍须由当前候选取得。[R08–R11][R16]
**展开**：[EX14 §3.2 与 §5：可达性边界及补查验证](../切片方案-EX14-视觉方法指导与关键解释时刻检查.md)。

## §3 观察证据

**决策**：分别验证资产、页面关系与连续使用。
**否决**：
- 用状态回执代替画面：报告目标位置不能证明几何与标签已经更新。
- 用截图代替连续使用：播放、暂停与恢复需要实际操作证据。
**命门**：资产帧用于初筛，页面截图与实际位置、数值和读数核对；后续模型请求须包含绑定候选、环境与动作的图片。连续使用沿用既有媒体解码、逻辑位置保存与暂停恢复合同。[R08–R13]
**展开**：[EX14 §3、§5、§6、§9：观察与 Reader 验收](../切片方案-EX14-视觉方法指导与关键解释时刻检查.md)。

## §4 条件性视觉方法

**决策**：在所选指导中增补少量条件性方法。
**否决**：
- 全量方法注入共同正文：增加无关输入，干扰当前表达选择。
- 固定开场、运动频率或色盘：无法对应具体理解障碍。
**命门**：`local.md` 补对象对应、变化、对照、注意重点与边界提示；`review.md` 补关键位置检查；技术细节进入所选参考。继续按采样追加指导，实施时更新 `ex13.v2` 修订号并保存各组确切文本。[R01][R05][R14]
**何时回头**：真实任务证明短指导不足时，再考虑拆分方法资源及其实际加载路径。
**展开**：[EX14 §6–7：检查指导与视觉方法](../切片方案-EX14-视觉方法指导与关键解释时刻检查.md)。

## §5 验证依赖与采用

**决策**：按路径验证，并以实际加载支持归因。
**否决**：
- 用 Manim 环境阻断全部工作：浏览器和静态指导有独立验证路径。
- 用未加载新指导的短答证明其收益或克制：受检增量没有进入该次运行。[R17]
**命门**：EX14.1-B 验证浏览器补查与可达性，EX14.1-M 验证真实媒体；未通过媒体分支时，各组 Manim 参考保留共同基线。冻结同底座的 A（现状）、B（A + 检查指导）、C（B + 视觉方法），分别比较 B−A 与 C−B；新增媒体指导使用新批次。
**采用条件**：全部运行留档，实际加载受检指导的收益与负向控制单列；未加载记“未检验该项增量”。N3 使用现有静态页措辞修订，纯文字路径作为路由回归。工程、表示与学习证据分开，按预先冻结的质量及成本边界决定采用范围。
**展开**：[EX14 §4–9：基线、验证分支与自然请求对照](../切片方案-EX14-视觉方法指导与关键解释时刻检查.md)。

## §6 回滚组合

**决策**：回退到已验证的指导组合。
**否决**：
- 从 C 仅撤掉检查指导后直接采用：A + 视觉方法未被三组对照验证。
**命门**：支持 C → B、C → A、B → A；其他组合另批验证。回滚同步指导正文、revision 与载荷记录，保留已交付内容、读者现场及有效工程回归。
**展开**：[EX14 §13：完成记录与回滚](../切片方案-EX14-视觉方法指导与关键解释时刻检查.md)。

## §7 自动抽帧增强

**决策**：重复补查困难成立后再评估抽帧增强。
**否决**：
- 首批直接扩帧：增加图片成本仍不能保证采到关键关系。
- 只修改 Python 采样：返回数量同时受 Rust、传输与模型输入预算约束。
**命门**：至少两个独立任务出现可复现的媒体补查困难或不可接受成本，排除指导未执行、页面定位/解码错误、演示选择错误及图片未进入请求后，EX14.C 比较四帧内改选、有界增帧、读取已有媒体指定位置。单纯参数控件不可达按 §2 处理。选型后补齐合同再实施。[R06–R10]
**展开**：[EX14 §10：条件准入与完整实现链路](../切片方案-EX14-视觉方法指导与关键解释时刻检查.md)。

## 已知限制

EX14.0 已冻结离线基线、实验材料与实际指导载荷，N3 静态基准通过三环境原生交付，见[基线记录](../performance/presentation-critical-moments-ex14/README.md)。EX14.1-B 的 A06–A09/A11 通过，见[浏览器记录](../performance/presentation-critical-moments-ex14/browser/README.md)。EX14.2 通用路径 A13–A18 通过，B 组 ex14.v1 全文与实际载荷已冻结，见[检查指导记录](../performance/presentation-critical-moments-ex14/checks/README.md)。EX14.3 通用路径工程完成，C 组 ex14.v2 全文、实际载荷、表示样例与离线 token 计量见[视觉方法记录](../performance/presentation-critical-moments-ex14/visual/README.md)。EX14.1-M 的 A04–A10/A12 已通过，见[媒体验证](../performance/presentation-critical-moments-ex14/media/README.md)。Manim 指导增补及兼容样例已完成，媒体 B/C 为 ex14.v3/ex14.v4，见[媒体指导验收](../performance/presentation-critical-moments-ex14/media-guidance/README.md)。真实模型实验和真实读者测量待执行。付费实验的范围与额度在启动前确定；采用结论限定到实际验证的指导组合和路径。

受控夹具只验证观察能力，自然用例覆盖有限的解释关系；学习效果需真实读者在新参数或新例子上的表现。EX12.5 Reader 连续使用的历史状态独立保留。

## 参考与证据

初次核查记录保留在 EX14.0；实施时依据相关文件内容重新冻结基线。

- **[R01]** [共同方法指导](../../skills/presentation/SKILL.md)：解释原则、Goal/framework 分工与真实观察。
- **[R02]** [ADR-0139](0139-explorable-explanation.md)：可组合方法、观察边界与分层验收。
- **[R03]** [ADR-0154](0154-presentation-global-framework-and-staged-authoring.md)：关键局部、阶段加载与串读。
- **[R04]** [ADR-0155](0155-goal-work-plan-and-version-centered-presentation-context.md)：Goal、稳定前缀、确切版本与实验边界。
- **[R05]** [指导装配](../../crates/runtime/src/agent_prompt/presentation.rs)：revision、阶段、参考及依赖选择。
- **[R06]** [Manim runner](../../crates/server/src/presentation_animation_runner.py)：固定版本、采样目标及解码帧时间。
- **[R07]** [动画资源验证](../../crates/server/src/presentation_animation.rs)：cues、metadata、四帧及字节上限。
- **[R08]** [预览类型](../../crates/runtime/src/presentation_preview.rs)：环境、动作、逻辑位置与观察结构。
- **[R09]** [Author 工具](../../crates/server/src/presentation_author.rs)：四动作限制、候选/run 绑定与交付。
- **[R10]** [浏览器预览](../../crates/server/src/presentation_preview.rs)：新页面、真实交互、逐动作观察与末态读取。
- **[R11]** [工程指导](../../skills/presentation/engineering.md)：阶段、来源、三环境及后续采样。
- **[R12]** [Manim 参考](../../skills/presentation/references/manim.md)：短面板、解码、暂停/恢复与逻辑进度。
- **[R13]** [连续场景参考](../../skills/presentation/references/continuous-scene.md)：定位、恢复与图形核对。
- **[R14]** [模块装配测试](../../crates/runtime/src/agent_prompt/presentation/tests.rs)及 [Resident 请求测试](../../crates/runtime/src/presentation_authoring_tests.rs)：按需加载与追加指导。
- **[R15]** [动画测试](../../crates/server/src/tests/presentation_animation_tests.rs)：资源合同与依赖真实环境的测试入口。
- **[R16]** [已有圆盘页面](../performance/ex11-local-demonstrations/ex11.7/batch12/runs/disk-1/delivered-raw.html)及 [数学检查](../../packages/web/playwright/agent-presentation-ex11-batch12-math.spec.ts)：半径参数与实际使用的核对值。
- **[R17]** [EX12.4 实验记录](../performance/presentation-staged-authoring-ex12-4.md)：未加载演示指导的纯文字运行只能支持文字路径结论。

外部参考于 2026-10-08 查阅，作为方法讨论材料。若后续复制实现代码，再核查所用版本、授权与依赖。

- **[E01]** [3b1b-explainer-video / SKILL.md](https://raw.githubusercontent.com/zhuweijun1003-source/zhuwj-skills/main/3b1b-explainer-video/SKILL.md)
- **[E02]** [visual-metaphors.md](https://raw.githubusercontent.com/zhuweijun1003-source/zhuwj-skills/main/3b1b-explainer-video/references/visual-metaphors.md)
- **[E03]** [advanced-animation.md](https://raw.githubusercontent.com/zhuweijun1003-source/zhuwj-skills/main/3b1b-explainer-video/references/advanced-animation.md)
- **[E04]** [layout-and-motion.md](https://raw.githubusercontent.com/zhuweijun1003-source/zhuwj-skills/main/3b1b-explainer-video/references/layout-and-motion.md)
