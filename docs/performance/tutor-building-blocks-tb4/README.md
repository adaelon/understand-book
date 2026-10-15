# TB4：可操作的认知积木

2026-10-12，工程与子代理内容验收完成，指导版本 `tb4.v1`。沿 TB3 independent-step 尚未回答的 `b.0 = 8` 判断，交付可选择两组 Copy／引用条件的教具；保存与重开现场后，在原聊天继续，让学习者自行选择对照并解释理由。代码、最终请求、实际交付、浏览器操作与理解记录见 [summary.json](summary.json)。

## 实施与边界

- `runtime/tutor.rs` 的 Global 把能力目标、已有支撑、学习者活动放入既有 framework/focus；Local 要求操作、代码与结果一致，按当前帮助请求保留未揭示结果；Review 用实际操作和内容依据验收。UNDERSTANDING 消费实际版本与发送时现场，区分教具结果、用户表达和独立解释。
- 最终请求按状态、阶段装配；共享呈现阶段文档已有所需工程指导，本片未改共享 schema 或工具示例。开工核对主工作区 checkpoint 时 TO4 尚未实施，本片使用既有 `presentation.author`。
- 新增 Server 冻结内容回放及浏览器夹具，复用 TB3 的实际交付和上下文读取；可视化测试入口按夹具显示本轮问题。领域模型、存储和正式评估合同沿用现有实现。

## 固定实验与实际内容

本轮用户批准新增 2 次 GPT-6.1-Sol 实验：生成教具、从实际保存现场继续教学，各一次。独立上下文，`fork_turns=none`，省略推理覆盖，使用模型默认设置。两次均已用完，之后的局部修正与回放没有新增模型调用。

| 实验 | 固定输入 | 原始输出 | 最终使用内容 |
| --- | --- | --- | --- |
| author | [input.json](sol/author/input.json)，188,166 UTF-8 bytes；TB3 理解、已展示帮助、未答活动、原文与当前工具说明 | [output.json](sol/author/output.json)、[answer.md](sol/author/answer.md) | [artifact.json](artifact.json)，两个 HTML 修正见下 |
| followup | [input.json](sol/followup/input.json)，153,051 UTF-8 bytes；浏览器真实现场及脚本用户表达 | [output.json](sol/followup/output.json)、[answer.md](sol/followup/answer.md) | [followup-delivery.json](followup-delivery.json)，补全已接受的解释引用 |

教具中的 A、B 可独立选择 `Debug, Clone, Copy`／`Debug` 和 `&a`／`&mut a`。每次改条件隐藏旧结果，点击“查看结果”后显示对应编译结果。固定 `let mut a = Foo(7)`、`u32`、`b.0 = 8`、先打印 b 再打印 a；页面明确说明比前题增加 mut、哪些是原文改写／补充实验，以及结果为预设而非浏览器即时编译。

页面自己的代码生成与事件逻辑实际执行后，四种代码交给 Rust 1.95.0 编译：两种 Copy 条件下，共享引用均为 E0594；可变引用均编译通过并打印 `b = Foo(8)`、`a = Foo(8)`。见 [content-final.json](content-final.json)。

最终保存现场是 A：Copy + `&a`，B：非 Copy + `&a`，两组均已显示 E0594。脚本回应还报告先前试过 B 的 `&mut a`，并明确区分教具给的结果和自己的局部推断。接续解释只把当前现场当作直接观察，把先前操作保留为用户报告；保留已有讲解、代码和错误原因提供的支撑。下一步让学习者自主选两组、指出相同／变化条件并把结果与理由连起来。理解记录为 `hypothesis / unassessed`，没有 `response_assessed`，新解释被实际交付的 move 引用。

## 检查目的与结果

| 检查 | 检测的具体失败；失败后的处理 | 结果与证据 |
| --- | --- | --- |
| Runtime 最终请求 | 教学指导在错误状态／阶段出现，framework/focus 丢失或重复；修相应装配 | 29 项通过，[runtime.log](runtime.log)；[Global](requests/Global.json)、[Local](requests/Local.json)、[Review](requests/Review.json) |
| Server 相关路径 | TB3 帮助或未答活动丢失、分页理解遗漏、交付现场归属错；修原记录／读取／夹具 | 4 项通过，[server-related.log](server-related.log)，包含 TB4 seed，单独 seed 日志不重复计数 |
| Rust 与状态恢复 | 展示结果不符、恢复字段无效；修生成页面 | 4 组编译／运行及恢复通过，[content-green.log](content-green.log)；原始恢复失败见 [content-original.json](content-original.json) |
| author / preview / deliver | 引用、候选、显示或操作失败；修具体输入或页面后重新交付 | 最终候选 5 次真实预览，含窄、短、桌面三环境；检查截图后用该 candidate_id 放行，[receipts.json](replay/receipts.json)、[accepted-preview.json](replay/accepted-preview.json)、[delivered-version.json](replay/delivered-version.json) |
| 浏览器与原聊天 | 切换留旧结果、重开丢状态、来源不可读／不可打开、追问错版本；修页面或实际回传接点 | 1 条完整路径通过，[browser.log](browser.log)、[checks.json](browser/checks.json)、[移动端恢复](browser/mobile-restored.png)、[接续截图](browser/followup-desktop.png) |
| 实际理解与交付 | 把操作记为能力、帮助未采用、新解释未绑定、文本快照编号过期；修指导消费或回放的运行时引用 | [followup-context.json](browser/followup-context.json)、[followup-result.json](browser/followup-result.json)、[author-host.log](author-host.log)；host 测试通过 |
| 类型与收口 | 测试入口类型不符、交接文件缺失、记录矛盾或差异格式错误；修相应文件 | [web-types.log](web-types.log)；`summarize.py` 核对最终链路，本次文档链接与差异检查 |

## 原始失败与修正

1. **状态恢复**：原输出读取 `saved.values[k]`，实际合同是 `saved.values.page[k]`。原候选五次预览后，确定性恢复检查失败，停止交付；[replay-original](replay-original/) 与原输出保留。只修改字段读取，恢复检查由失败转为通过。
2. **来源显示**：原 HTML 给所有按钮设置强调色背景，宿主来源文字也是强调色，最终截图中来源入口看起来空白。保留 [修前截图](browser-before-source-color-fix/followup-desktop.png)，将该颜色规则限定为非来源按钮。最终两处来源标签可读，实际经来源预览打开正文。
3. **解释引用**：原接续创建了新的理解假设，但 select 只引用 TB3 旧假设。[原接续交付](browser-before-ref-completion/followup-result.json)与原模型输出保留；从实际 evidence 回执取得 `interpretation:tb4-followup-copy-shared-access-observation`，追加到 select 的引用，教学正文未变。见 [followup-repair.json](followup-repair.json)。

上述两处页面修正由 [repair-page.py](repair-page.py) 从原模型输出重建，记录于 [page-repair.json](page-repair.json)。最终内容核对只允许这些字段差异；原输出没有被覆盖，未将人工修正算成模型首次成功。

回放夹具另有三处修正：交付 payload 保存的是完整 presentation 版本，初版断言误当成裸 ref；点击来源先打开预览，初版浏览器断言跳过了“在正文中查看”；相同操作的自动保存批次可能令重跑的快照编号由 9 变为 8，初版回放未重绑模型解释中写出的编号。现先核对冻结现场和本次现场的条件／可见结果相同，再只重绑这段运行时标识，保留 [scene-rebinding.json](browser/scene-rebinding.json)。相关失败日志、修前回放目录保留；这些不算模型采样失败。

## 本地复现

`prepare-sol.py` 只冻结本地输入，已有输入拒绝覆盖；它不调用模型。无需重新生成模型输出。复现会改写本目录的 `replay/`、`browser/` 和汇总，先将现有记录保留到单独目录。

环境：Rust `+1.95.0`，`CARGO_TARGET_DIR=D:/codex-build/understand-book-tb-195`，dev/test debug=0、incremental=0，`TEMP/TMP=D:/codex-build/tb1-temp`，编译并发 2。Web 使用仓库现有 pnpm 依赖。

1. 内容核对：`node docs/performance/tutor-building-blocks-tb4/verify-content.mjs artifact.json`。页面样式修复未改变已经核对的代码生成与状态逻辑。
2. 请求核对：显式设置 `TB4_REQUEST_SNAPSHOT` 为输出目录，运行 Runtime 的 `tutor_tc2a_presentation_guidance_follows_discovery_phase_and_delivery`，可导出三阶段最终请求。
3. 启动 `cargo +1.95.0 test -p server --lib -j2 tutor_tb4_author_and_browser_host -- --ignored --nocapture --test-threads=1`。它使用冻结输出，真实执行 author/preview，等待查看 `replay/` 图片与回执。确认后写 `accepted-preview.json`，其中 candidate_id 必须来自本次 `awaiting-inspection.json`。随后交付、服务于 4185 端口。
4. 在 `packages/web` 运行 `pnpm exec playwright test --config playwright.tb4.config.ts`，执行操作、存储重开、原聊天追问与来源入口；配置启动／复用 4184 的本地 Web。它沿冻结输入的相同事实重绑本次引用，不新建模型调用。
5. 向 `http://127.0.0.1:4185/stop` POST，等待 Server 测试退出；运行 `python docs/performance/tutor-building-blocks-tb4/summarize.py`。宽泛的 Server `tutor_` 测试会重写旧 TB2 回执，继续沿原 checkpoint 的保留要求处理。

## 已知限制

- 两次模型输入有预读来源和固定工具次序；验证的是实际内容经真实工具、存储和浏览器交付，不包含生产 Provider 的自主调用循环。没有对照组或重复采样，不据此宣称普遍改善或首次调用稳定性。
- 真人读者 0，用户表达为明确标识的脚本夹具。已验证可操作和接续，真实学习收益及迁移留待 TB6。
- 两次模型 token、费用和模型耗时未返回，均为未知。指导相对 TB3 的 UTF-8 增量分别为 Global 488、Local 543、Review 465、UNDERSTANDING 486 bytes；前三段按阶段择一加载。这些是文字体积，不是 token 或费用。
- DeepSeek 新增调用 0；TB2 历史 1,032,768 token 未结算预占保持，原专项未结项沿原记录。TB5–TB6 待实施；TO4 尚未集成，本片不代表 TO4 完成。
- 本片与既有 TB1–TB3、TO3 修改均未提交、未部署。
