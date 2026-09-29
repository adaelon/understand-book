# EX0：可探索解释实验冻结记录

冻结时间：2026-09-27（Asia/Hong_Kong）。本节在 EX1 样板与 EX2 处理组运行前写入；后续实测结果只追加到各自记录，不改本节门槛。

## 输入与独立答案

- 自然请求及完整材料：[ex0-inputs.json](explorable-explanation-ex0-inputs.json)。四题为学习率困惑、普通短答、观察/因果边界、静态换视角；前三题逐字沿用 ED3 输入。每题在独立新会话运行，不把答案或控件要求放入请求。
- 核对关系、具体数值和非数值边界：[ex0-answers.md](explorable-explanation-ex0-answers.md)。一维模型不推广到其他优化器或损失面。
- 表示基线：[ED3 实际交付 view.json](ed3-windows/C-position/view.json)及[原始判定](agent-presentation-ed3-ed4-20260925.md)。它已有位置、误差轨迹、滑块、分步及静态图；C 用 12 次采样、473,068 tokens、241.0 秒交付，超过当批 100,000 token 上限，最终文字中 `η=1.25, w₄` 错误。B 同底座重跑未交付，花 283,661 tokens、242.4 秒。这是历史表示与成本参照，不充作本批 B/C 结果。

## 本批共同底座与差异

- Git HEAD：`c2ff3e1 feat: ship reader, agent, viewport, and observability updates`。本批使用 2026-09-27 工作树而非 HEAD 纯净版：冻结时 `crates/runtime/src/agent_prompt.rs`、`lib.rs`、`crates/server/src/agent_run.rs`、`agent_run_tests.rs`、`presentation_author.rs`、`packages/web/src/presentation-bridge.js`、`components/AgentPresentation.vue` 已修改，`skills/presentation/SKILL.md`、ADR-0139 与实施方案未跟踪。其他既存工作树修改保留；运行记录须注明后续是否发生共同底座变化，变化后不混入同一批。
- Windows 本机真实 Resident/Server/BrowserPreview 路径；`.env` 已配置 `deepseek-v4-flash`，Provider 使用 Runtime native 工具协议，`temperature=0`，Runtime 默认模型输出上限 8,000 tokens/次。除实验 skill 文本与版本外，两组共用工具 schema、能力发现入口、来源及交付合同、三视口与同一模型配置。原始 Provider 用量以实报为准，错误和未返回用量原样记账。
- B：冻结的 `ed3.v1` 主 skill 正文（EX2 修改前的 `skills/presentation/SKILL.md`）。C：EX2 新版主 skill。实验适配只替换 `resident-agent.skill.presentation-method` 的 revision/text；不改变生产能力路由或工具。每次记录模型请求是否实际带该模块，未加载只用于路由与短答退化观察。顺序按 B1/C1/C2/B2/B3/C3 交错学习率题；其他题 B/C 各一次。独立会话与私有状态。

## 运行与判定条件

| 项 | 冻结条件 |
| --- | --- |
| EX1 金样板 | 与 ED3 相同算例和信息量，另做静态/离散步进对照；比较同一对象能否在过渡中被追踪、跨越最优点是否更易辨认。三种 η、多个 k、最终文字均与独立答案一致。真实 Reader 保存、重开、追问须捕获同一暂停现场。 |
| EX1 浏览器 | 960×720 mouse、320×420 touch、640×240 touch 均无脚本错误、关键内容不裁切、触控目标至少 44 CSS px；播放、暂停、中间定位、回退重放、改参、预测揭示逐项可见。 |
| EX2 学习率质量 | 每组三次独立运行，保留全部失败。交付产物必须在一维范围内区分 `η=0.2/0.8/1.1` 的方向和距离变化；数值、readable content、最终文字正确。C 的新增收益须指出 ED3/B 中缺失的具体关系，而不把单纯页面存在算收益。 |
| EX2 其他三题 | 每组各一次定位退化。普通短答直接答，无固定额外模型回合；C tokens ≤ B×1.3。非数值题与换视角题不得虚构因果或不必要动画/页面；C 采样 ≤ B+1，tokens ≤ B×1.5。一次观察不声称稳定性。 |
| 每次学习率预算 | ≤12 次普通模型采样，允许既有一次交付宽限；≤300,000 Provider 实报 tokens，≤300 秒墙钟；所有必要的 write、三视口 preview、修订重试及交付均计入。超额、Provider 错误、未交付、触屏不可用、数值错误各记失败，不挑成功样本替换。 |
| 稳定判定 | 三次学习率 C 均满足质量与预算才称本批通过；若只有局部收益，记录具体收益但 EX2 Gate 未过。B/C 均失败或 C 未明显优于同底座 B 时不宣称 skill 增益。 |

Provider tokens 采用实际返回值；没有返回就记录“未知”，不能把未知当 0。模型生成成本不含安装时间。学习效果另列，当前尚未评估，不从页面操作或 Agent 自评推出。

## EX5a 预先冻结的动画测量条件

在 EX1 可播放样板上测 Windows Chromium/Reader 真实播放；目标 30 fps，典型场景为三条不超过 8 步的轨迹、一个移动对象及文字读数。连续 5 秒内相邻可见帧间隔的 95 百分位 ≤50 ms，单帧 bridge 本地观察耗时的 95 百分位 ≤8 ms；逐帧几何变化不应产生逐帧宿主保存请求。若超限，先区分页面渲染与 bridge 观察开销再决定 EX5b。测量不代替 EX1 的语义和保存验收。

## EX0 Gate

请求与材料、独立答案、ED3 已交付表示、工作树底座、组差异、运行次数、失败记账和质量/成本门槛均已落盘。此 Gate 只允许启动 EX1/EX2，不预判两者是否通过。
