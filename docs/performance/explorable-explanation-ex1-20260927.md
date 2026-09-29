# EX1 手工金样板与 EX5a 播放测量

日期：2026-09-27。冻结输入、独立答案及门槛见 [EX0](explorable-explanation-ex0-20260927.md)。样板源码：[ex1-learning-rate.html](ex1-learning-rate.html)。真实 Reader 截图：[桌面中途](ex1-windows/desktop-middle.png)、[窄触屏](ex1-windows/touch-narrow.png)、[短触屏](ex1-windows/touch-short.png)。

## 样板与表示对照

页面以同一数轴上的四条对象身份稳定的轨迹显示 `η=0.2/0.8/1.1` 和可调 η；空心残影仅标真实迭代端点，实心点沿端点之间连续移动。数值区始终显示已完成的真实步骤。滑动到第 2→3 步的 75% 时，0.2 实心点仍在 2 左侧，0.8 已在右侧，直接显现同损失序列中的方向差别。先预测再揭示的选择与当前步骤绑定。

对照 [ED3 已交付版本](ed3-windows/C-position/view.json)：ED3 有参数位置、带符号误差、蛛网图及离散步进，已能区分三种现象。EX1 的新增表示是同一坐标并排跟踪三条身份固定的轨迹、可停在跨越过程中的位置，以及预测后揭示；离散端点已能回答“是否跨越”，连续过渡的独立学习增益尚未由读者任务证明。样板没有据此抽取通用动画 helper。

核心计算只由 `result(eta,k)` 生成，SVG 位置、残影、数字表和读数取该结果；过渡坐标在两个真实结果间插值。Reader 交付文字说明模型、三种现象与插值边界，没有另写错误的数值序列。自动检查覆盖独立答案中三种 η 的多个步骤、预测揭示、回退后重复定位、改参数清除旧轨迹、三视口、触控目标，以及中途现场保存、同版重开和追问。

## 验证

- `pnpm exec playwright test playwright/agent-presentation-ex1.spec.ts --reporter=line`：3/3，通过真实 Rust 验收宿主、Reader iframe、私有状态保存和追问回执。数值表与 EX0 独立答案一致；第 2→3 步 75% 的跨越判断由 SVG 坐标断言；重开后保留步骤 2、进度 0.5、预测揭示，追问携带同一现场。
- `cargo test -p server presentation_ex1_gold_uses_three_real_preview_environments -- --ignored --nocapture`：1/1。同一 HTML 经 `presentation.author` 写入，三种必要视口均执行下一步、预测和揭示，取得完整 preview 回执后允许交付。
- 三视口为 960×720 mouse、320×420 touch、640×240 touch。页面脚本无错误，正文没有横向溢出，按钮至少 44×44 CSS px；窄触屏的同轴图在图内横向滚动，截图只显示当时窗口，完整内容仍可通过滚动查看。
- EX5a 最终版五秒真实播放：304 个帧间隔，95 百分位 16.7 ms；255 次 MutationObserver 回调，回调耗时 95 百分位 1.2 ms；6 次宿主保存请求，发生在语义步骤/播放结束而非逐帧。均在 EX0 预设的 50 ms、8 ms 和非逐帧保存界限内。该仪表测浏览器观察回调，不单独归因于 bridge 内部函数。

## 已知问题

- 没有真实读者的学习前后任务，因此连续过渡对理解的独立增益尚未评估；工程与可见表示通过不等于学习效果通过。
- 当前 preview 工具没有中间时间定位动作，EX1 通过真实 Reader/Playwright 验证定位；EX6 仍需正式接入 preview 合同。
- 窄屏图使用图内横向滚动；若后续读者任务表明这种比较负担过高，再以实际反馈修改移动布局。
