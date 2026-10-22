# 视频提问提交延迟与回答等待 — 2026-10-21

> 发布更新：2026-10-21 已上线，Windows 0.3.3 已发布并完成本机升级。以下保留修复阶段记录；最终线上状态见 [发布记录](../reader-latency-release-20261021/README.md)。

## 结论

截图中的视频问题存在两段不同的等待：前端积压草稿保存，随后服务端多轮模型取材与生成。本轮修复提交前的积压、无反馈和保存失败后无法原文重试；另从线上既有记录确认生成阶段的实际耗时。未发布线上变更。

## 提交前的根因与改动

- `packages/web/src/video-question.ts:createVideoQuestion` 原来为每次输入排队保存一份完整草稿，`submission()` 等整个队列。现在保留进行中的保存，将相邻等待中的保存合并为最后一次编辑；取消草稿仍是顺序边界。画面、选区、时间范围和图片绑定沿用原草稿。
- `packages/web/src/network-client.ts:workspaceAction` 原来每次视频草稿保存先 GET 工作区再 POST。现在直接使用当前版本提交，只有服务端明确返回 `WORKSPACE_REVISION_CONFLICT` 才读取并重试一次。服务端 `workspace_registry.rs:request` 在执行草稿命令前检查版本。
- 视频草稿请求（包括冲突后的刷新）设置 15 秒超时。失败保留本地内容，用户再次发送会重存同一草稿；不会因此自动创建模型请求。
- `packages/web/src/App.vue:submitAgentMessage` 在第一个异步等待前显示问题和提交状态、锁定发送。状态区分“正在保存视频提问…”与“正在提交问题…”。视频保存阶段暂时禁用编辑，避免提交中的问题和画面被新输入改变。保存失败明确留在未提交状态。
- 书籍提问共用提交状态；它不经过视频草稿队列。

## 可重复的提交测量

使用截图中的问题，31 次输入事件、相邻事件间隔 80 ms、每个 HTTP 请求模拟 500 ms。运行真实 `createVideoQuestion → api.videoDraft → workspaceAction → api.agentRunCreate` 前端链路，HTTP 返回使用受控替身，无模型调用。计时由 fake timers 确定，因此单次前后对照即可判断排队机制，不代表线上延迟分位数。

| 指标 | 修改前 | 修改后 |
| --- | ---: | ---: |
| 草稿保存数 | 31 | 6 |
| 总 HTTP 请求数（含提交） | 64 | 8 |
| 点击到提交模型请求 | 29,020 ms | 1,020 ms |
| 点击到模拟受理返回 | 29,520 ms | 1,520 ms |
| 最终问题文本一致 | 是 | 是 |

原始数据：`before.json`、`after.json`。场景代码：`packages/web/src/video-question.performance.test.ts`。

```powershell
$env:QUESTION_SUBMIT_REPORT='E:/allwork/download/agent/understand-book/docs/performance/question-submit-loading-20261021/after.json'
pnpm --dir packages/web exec vitest run src/video-question.performance.test.ts
```

## 线上生成阶段的证据

只读分析现有 session 日志，没有发起新模型请求。已核对 2026-10-21 16:48:48 +08:00 的视频问题与截图文字完全相同。

- 运行最后一项活动结束于约 116.26 秒；8 次模型调用合计约 111.56 秒。
- 第一次记录到的模型正文出现在运行约 109.75 秒处。该指标来自模型活动记录，不等同于浏览器收到正文的时间。
- 第 5、6 轮引用视频时，模型把版本号中的 `8329` 写成 `8323`。两次视觉引用返回 `SOURCE_NOT_OBSERVED`，随后使用同一个错误版本号引用转写，返回 `WORKSPACE_MATERIAL_UNAVAILABLE`。
- 检索、读取书籍正文和注册正常引用大多仅数毫秒至数十毫秒；两次视频读帧分别约 2.04、2.05 秒。主要等待来自模型回合。
- 最近一条书籍运行约 12.95 秒，4 次模型调用。前一天的部分连续教学运行还包含教学检查与失败后的重新尝试；不能把它们归因于视频草稿保存。

原始阶段数据保存在 `production-timings.json`，派生汇总在 `production-summary.json`，截图匹配与错误引用在 `video-tools.json`。未保存用户问答正文、书籍原文或账号标识。日志的 `finished_at` 使用冻结时间，不能以其与 `accepted_at` 的差计算耗时；本报告使用活动的 `started_ms + duration_ms`。

## 验证与版本

- 修改前新增的 6 个回归用例失败，118 个既有用例通过：`tests-before.log`。
- 原文重新发送的回归用例在修复前失败：`retry-before.log`。
- 最终 6 个相关测试文件、194 项测试通过：`tests-final.log`。覆盖草稿合并、取消与重新绑定、图片和时间范围、保存失败、超时、原文重试、立即显示状态、重复点击、引用冲突和提交结果恢复。
- 类型检查结果：`typecheck-final.log`。
- `before/` 保存本轮开始时 6 个文件的完整工作区版本，包括原有未提交改动。`fix.patch` 只包含本轮相对该版本的修改和新增性能用例，不包含此前课程目录优化。

## 已知问题

- 本轮没有缩短或重新测量真实模型生成耗时；截图问题的多轮取材、模型抄错引用和教学检查延迟仍需单独优化并验证回答质量。
- 500 ms 模拟请求耗时仅证明前端积压已消除，不能据此承诺线上一秒回答。
- 改动保存在本地工作区，尚未构建发布线上前端或更新生产服务。Windows 云端学习页面与网站使用同一远端前端，发布后才会采用改动。
