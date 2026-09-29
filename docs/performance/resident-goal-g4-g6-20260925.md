# Resident Goal G4–G6 验收记录（2026-09-25）

## G4 完成与有限收尾

明确要求演示页时，Goal 从用户原文建立 `presentation_delivery` 要求；明确“只用文字”仍建 `content` 要求。正常终答前，Runtime 对尚缺页面或 Reader 操作返回具体缺口，在原十二次采样额度内继续。额度耗尽后以诚实的未完成文字收尾；禁用工具收尾若仍收到工具调用，也保留 `TURN_LIMIT_EXCEEDED` 和开放 Goal。Provider 失败保留失败原因与开放 Goal。

Server 复用现有答案呈现引用校验，只在回答非 incomplete、无 warning、页面确已挂入最终视图、确切版本属于当前 Goal 时，将回合与 Goal completed 一起提交。文字声称完成、候选、预览、未挂载页面、其他 Goal 的交付均不能满足页面要求。提交失败仍是未保存结果，重启后可沿原 Goal 继续。

脚本测试覆盖上述六种页面路径、预算内重试、禁用工具收尾、取消、Provider 失败和历史提交失败。未增加独立评审模型或预算。

## G5 Reader 状态与继续

`/agent/history` 的会话包含 `goals`，回合包含 `goal_ref`。Reader 问答栏显示开放任务、最近停止原因、页面缺口、已有部分结果，并发出带确切 `goal_id` 的“继续任务”或 `goal_action=cancel` 的“取消任务”。停止当前生成沿原 Run 取消路径，不取消 Goal。重启后继续同一 ID、完成任务后的修改建立关联 Goal、替换开放 Goal、多个开放任务的裸“继续”歧义均有 Server 测试。

Web 的 RightRail 交互测试 18/18 通过；类型检查与生产构建通过。开发宿主的实际浏览器页面版本重开及交付测试通过。

## G6 自然请求：章节交付通过，整体验收未完成

使用 `.env` 当前配置的 `deepseek-v4-flash` 原生 Provider、十二次正常采样上限。原章节请求原话不加工具指令。测试书由本机同一本 Markdown 源构建，章节锚点 `1.11`；无法证明它与原私有会话当时的已构建版本逐字一致。原 G0 私有轨迹未保存可移植模型标识或用量，故不能计算可信的同配置前后 token 差额。原 G0 失败轨迹为 39 次工具调用、无制作/预览/交付、`TURN_LIMIT_EXCEEDED`。

| 场景与证据 | 实际结果 | 模型请求 / 回报 tokens |
| --- | --- | ---: |
| 章节原话，`tmp/g6-resident-goal/run-1` | 12 次正常采样后制作候选并三次预览，预览因页面控件不可操作失败；无交付。末次禁用工具调用造成协议错误，Goal open。该轮在 G4 收尾修正前运行 | 13 / 340,620 |
| 章节原话，`run-2` | 第 2 次请求连接重置，`PROVIDER_ERROR`；Goal open | 2 请求，1 回执 / 5,118 |
| 章节原话，`run-3` | 第 9 次请求返回 Provider 402 `Insufficient Balance`；前 8 次回执已进入制作/预览，无交付，Goal open | 9 请求，8 回执 / 151,186 |
| 章节原话，`run-4` | Provider 完整运行；第 11 轮写候选，第 12 轮触屏预览因按钮目标小于 44 CSS 像素失败，`TURN_LIMIT_EXCEEDED`，Goal open。修正制作说明的触屏控件尺寸要求 | 13 / 399,803 |
| 章节原话，`run-5` | 第 6 轮写候选，第 7 轮预览后修订，第 9 轮三种环境预览均无错误，第 10 轮交付。HTTP 200、Goal completed、确切版本挂入回答 | 11 / 425,544 |
| 普通短问，`short` | 内容回答完成，Goal completed；未满足冻结的“一次正常采样”判据 | 7 / 100,528 |
| 引用原文的短问，`short-quoted` | 内容回答完成，Goal completed；未满足一次采样判据 | 9 / 136,900 |
| 明确只要文字，`text-only` | 未要求页面；十二轮后文字概述以 `TURN_LIMIT_EXCEEDED` 结束，Goal open | 13 / 264,638 |

上述 token 为本轮已收到的 Provider `usage` 合计；Provider 失败请求没有回执用量，不推断为零成本。`run-5` 交付版本为 `presentation-1790325941164209800-2` revision 1，页面和当前 Goal 的结果引用一致。独立浏览器复看截图保存在 `tmp/g6-resident-goal/run-5/delivered-desktop.png`、`delivered-narrow.png` 和 `delivered-narrow-viewport.png`。页面按“为什么需要 → 两个设计维度 → 信息增量判据 → 共享/不共享上下文 → Agent 社会”组织，包含比较表和图；窄屏首屏可读。页面也明确标注后段拓扑、并行协调和失败模式未逐段核对，因此内容覆盖只能判为整体关系已呈现、细节覆盖有限。

交付版 `sources` 为空，HTML 内也没有 `data-source-ref`；读者无法从页面直接回到具体正文。这是内容质量验收还需修的实际缺口。

制作中调整要求、超限后自然继续、多个开放对象的自然模型请求和 Windows Debug 桌面包、Linux Reader 发布路径尚未验收；普通短问与只要文字场景尚未满足冻结判据。G6 整体保持未通过；不以脚本化回归替代自然模型结果。

## 限制与下一步

`run-4` 的失败暴露了触屏控件尺寸这一具体制作缺口；说明修正后的 `run-5` 在原预算内交付。后续应针对尚未达标的短问/纯文字场景和未运行的跨轮自然场景继续定位，再做两个发布宿主的保存、继续和渲染验收。只修可观察失败，不扩大十二轮预算来宣布通过。
