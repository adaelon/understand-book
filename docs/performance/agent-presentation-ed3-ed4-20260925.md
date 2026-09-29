# ED3–ED4：自然请求说明与真实预览反馈

日期：2026-09-25。底座：HEAD `c2ff3e1` 加工作树 ED1/ED2；三条请求由真实 Resident 入口执行，固定测试书只含无关的 `X…尾巴` 示例，任务所需材料均在用户请求中。模型为本机 `.env` 配置的 `deepseek-v4-flash`，native 工具协议；原始输入、各次 Provider 请求/响应、结果和耗时保存在 [`ed3-windows/`](ed3-windows/)。B/C 的唯一区别是测试适配器在 B 请求中去掉 `resident-agent.skill.presentation-method`；生产路径没有开关。两组使用同一测试代码、工具 schema 和预算，独立会话，每题各一次，原始失败照录。

## 运行前冻结的判断条件

- 普通短答：直接解释原文，不无故制作；C 不增加固定模型回合，单次 token 不超过 B 的 1.3 倍。
- 表示盲区：实际产物应让 `η=0.2/0.8/1.1` 的位置变化或带符号误差可区分，明确临界值只属于给定一维模型；不能只展示损失。运行至多 12 次普通模型采样和既有一次交付宽限，≤100,000 tokens、≤6 分钟；未交付按失败记录。
- 非数值材料：区分观察事实与因果推测，不捏造因果实验或模拟器；C 模型采样不超过 B+1，token 不超过 B 的 1.5 倍。
- 说明收益以可见产物和上述独立算例判断；页面无脚本错误、Agent 自评或执行完毕都不单独算表达改善。单次对照只能定位失败，不能证明稳定效果；用户学习效果本轮不评估。

## B 原始基线

| 任务 | HTTP | 模型采样 | tokens | 墙钟 | 观察 |
| --- | ---: | ---: | ---: | ---: | --- |
| 普通短答 | 200 | 1 | 5,023 | 6.9 秒 | 直接文字解释，没有制作。 |
| 表示盲区 | 500 | 13（含收尾） | 结果未返回 | 265.8 秒 | 多轮制作后在禁用工具的收尾采样仍调用工具，`FINALIZATION_TOOL_PROTOCOL_VIOLATION`；没有交付。 |
| 非数值材料 | 200 | 3 | 12,908 | 13.6 秒 | 待与 C 同底座结果核对。 |

表示盲区 B 是 ED4 改动前的初始基线；如 C 进入预览链路，需在 ED4 完成后重跑 B，不能把反馈修复计作说明收益。普通短答与非数值材料没有走预览，仍可作说明加载和短答退化参照。

同底座 B 重跑保存在 [`B-position-final/`](ed3-windows/B-position-final/)：HTTP 200、8 次模型请求、7 个常规回合、283,661 tokens、242.4 秒。模型发现制作能力并调用 `render_plot`，但预览后没有交付；最终改以文字解释并明确承认页面未完成。文字中的误差乘数及例子可核对，然而用户要求的可见表示没有交付，且超过运行前冻结的 100,000 token 成本界限。最初 B 的协议错误与此次未交付都保留为失败样本，不取较好的一次替代原始结果。

## C 原始结果

| 任务 | HTTP | 模型采样 | tokens | 墙钟 | 说明实际加载 | 观察 |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| 普通短答 | 200 | 1 | 5,116 | 7.7 秒 | 否 | 直接文字解释，无固定回合；用量为 B 的 1.02 倍。 |
| 非数值材料 | 200 | 5 | 22,691 | 14.4 秒 | 否 | 没有发现制作能力；用量超过预设 1.5 倍界限。此差异不能归因于未加载的说明。 |
| 表示盲区 | 200 | 12 | 473,068 | 241.0 秒 | 是，从制作工具可见后的请求开始 | 交付了有位置/误差轨迹、滑块及预览截图的页面；超过预设 100,000 token 界限，最终文字中 η=1.25 的第 4 步数值错误。 |

这两题验证了普通回答不载入完整制作说明；它们没有测量新说明的表达收益。模型采样差异保留为原始对照的随机波动与能力路由观察，后续决策只用真正加载说明的题目评估说明本身。

表示盲区 C 的交付版本保存在 [`C-position/view.json`](ed3-windows/C-position/view.json)，实际窄屏和桌面预览分别见 [`image-09-00.png`](ed3-windows/C-position/image-09-00.png) 与 [`image-10-03.png`](ed3-windows/C-position/image-10-03.png)。页面的 `traj(eta,n)` 以 `e←(1−2η)e` 计算，能显示同侧接近、两侧交替收敛与发散。独立计算 η=1.25：`w_0=0, w_1=5, w_2=-2.5, w_3=8.75, w_4=-8.125`；最终文字把最后一项写成 `−21.9`，与页面公式不符。C 完成了交互交付，但未达到本批成本与准确性条件；一次结果不足以证明短说明稳定优于 B。

## ED4 工程判据

定向测试须证明失败、部分完成和同批多预览图片进入下一次 native/react 请求；每张图携带候选、环境、步骤和状态。同批 `deliver` 仍被后续采样门槛拒绝；候选只在完整预览且实际可用图片进入后续采样时取得资格。预算缺口须在请求中说明未送达范围，不把遗漏截图当已检查。真实模型修正另外用不含脚本异常的预置差表示观察；数值按给定一维模型独立核对。

首次预置尝试保存在 [`ed4-windows/seeded-loss-only/`](ed4-windows/seeded-loss-only/)：首个 `write` 被替换成正确运行、只显示损失的初稿，但模型在预览它之前又写了新候选；随后交付。HTTP 200，9 次模型请求、459,361 tokens、331.0 秒。此轨迹只说明模型主动重写，**不能计为基于缺陷截图的修正**。第二次定向运行把预览之前的候选都置成同一有效初稿，确保首次真实预览覆盖该缺陷；其原始轨迹另存，不用第一次成功替代。

第二次尝试保存在 [`ed4-windows/seeded-loss-only-previewed/`](ed4-windows/seeded-loss-only-previewed/)：模型在首次预览之前收到替换的有效候选，但还没预览即返回 `PROVIDER_EMPTY_RESPONSE`；HTTP 502、4 次模型请求，不能判断表示修正。随后改用三次脚本化设置调用，固定发现能力、写入有效的仅损失页面、在 320×420 touch 环境真实预览，再将完整回执和截图交给模型。预置页面无脚本错误，所展示的损失不能区分同损失、不同方向的更新。

固定预览后的 native 原始尝试见 [`ed4-windows/seeded-preview-provider/`](ed4-windows/seeded-preview-provider/)：首个实际 Provider 请求含该截图及 `preview_environment_recorded`，模型调用 `render_plot` 后，下一次请求收到 HTTP 400，报 `The reasoning_content in the thinking mode must be passed back to the API`。定位为流式响应丢弃 `reasoning_content` 且后续 assistant 工具调用未回传它。相同预览的 ReAct 尝试见 [`ed4-windows/seeded-preview-react/`](ed4-windows/seeded-preview-react/)：首个请求同样包含截图，但回复无法解析为 final 或 tool_calls，HTTP 502。两次均没有交付，不能算视觉反馈修正。Runtime 已增加流式 reasoning 保留及同一运行内 assistant 工具调用回传；另以同一固定预览重新运行 native，结果见后续记录。

修复后同预览的原始运行见 [`seeded-preview-native-fixed/`](ed4-windows/seeded-preview-native-fixed/) 与诊断运行 [`seeded-preview-native-trace/`](ed4-windows/seeded-preview-native-trace/)：真实模型收到截图后，首次回复即调用 `presentation.author(write)`，写出参数位置、带符号距离、损失对照和 η 控件，不再只展示损失。后一次请求仍返回相同 400；诊断显示新真实模型回合的完整 reasoning（39,505 字符）已回传，缺的是三个脚本化设置回合的 reasoning。此预置夹具使用了工具调用消息，却没有 Provider 生成的推理字段；[DeepSeek 思考模式工具协议](https://api-docs.deepseek.com/guides/thinking_mode/)要求历史各轮完整回传。故这两次只证明模型**提出**了针对截图的表示修正，不能证明完成再次预览与交付；脚本化预置历史不能直接作为该 Provider 的完整端到端验收。后续不再以重复运行同一夹具凑成功率。

把诊断运行产生的真实新版候选另交给原 `BrowserPreview`，三个视口均无脚本执行错误。960×720 mouse 没有浏览器问题，实际图能显示 `η=0.8` 时位置在 `2` 两侧交替（`w=0,3.2,1.28,2.432,1.741,2.156`）；320×420 和 640×240 touch 各有 9 个 `touch_target_small`，滑杆高度约 16 CSS px，按钮约 37 CSS px，尚不能通过触屏预览合同。三视口结果与截图保存在 [`revised-browser/`](ed4-windows/revised-browser/)。这属于模型修订产物的具体质量问题，不能以公式和控件设计意图替代实际可交付页面。

## 判定与验证

- ED3 的按需加载和 B/C 原始对照已完成。C 的表示盲区交付可见轨迹，B 未交付；但 C 的 473,068 tokens 及最终数值错误均越过预设门槛。短说明的稳定表达收益尚未证明，当前接入留在工作树供下一切片决定，不作为已选发布方案。
- ED4 的工程缺口已补齐：失败、部分完成、同批不同候选的真实截图进入下一次采样；预算省略会列出未见的候选/环境；完整环境回执和后续采样门槛仍决定交付。真实模型在看到有效但表示不足的页面后提出换表示；该候选触屏预览未过，亦没有端到端交付。因此“修复了反馈传输”和“完成了表达修正交付”是两项不同结果。
- 定向回归：Runtime `cargo test -p runtime presentation_ --lib` 32/32；流式 reasoning 与 native 后续工具消息各 1/1；Server 浏览器 `presentation_author_browser_correction_and_private_delivery` 和 `presentation_author_requires_three_explicit_environment_receipts` 各 1/1。八个目标 Rust 文件 rustfmt 检查与相关已跟踪文件 `git diff --check` 通过。真实模型预置夹具与重放候选的失败均按原始结果保留，不计入通过项。

## 已知问题

- ED3 只有三条自然请求各一次；C 成本、最终数值准确性不满足本批界限。模型生成有随机性，不能由一次交付推出稳定收益。
- ED4 的预置历史由脚本化工具消息构成，未携带真实 Provider 的推理字段；即使 Runtime 已回传真实回合的推理，DeepSeek 后续请求仍拒绝它。此夹具已从常规代码移除，原始记录保留。真实候选的触屏目标尺寸未达浏览器门槛。
- Linux、正式桌面安装包与真实手机入口沿用 ED1/ED2 未完成的发布验收范围；本批 Windows 的 touch 预览不等于手机实机。
