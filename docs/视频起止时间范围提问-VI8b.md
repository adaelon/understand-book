# 起止时间范围提问（VI8b）

## 操作与草稿

视频阅读页点击“选择时间范围”，填写起点与终点秒数，或移动播放器后点击“当前位置设为起点/终点”，保存后输入问题。范围须在原片内且终点晚于起点，可超过单次读取的 60 秒。放弃修改或 Escape 关闭编辑器，保留已保存内容。

范围与区域分别保存在 `VideoQuestionContext.range` / `region`。新增范围时固定当前实际帧；已有问题时编辑范围保持草稿 ID、发布、视频/转写材料和原帧。范围可以包含原帧之外的过程，原帧选区仍是独立关注点。播放器继续播放、跳转与倍速变化不改写草稿。

取消范围保留文字和区域；取消区域保留文字和范围；三者全部清空时释放草稿。刷新恢复、失败重试、排队与接单沿 VI6 的窗口草稿与冻结回合链路。问题头和历史同时显示绑定帧、区域标记及起止范围。

## 读取与引用

`VideoReaderPane 起止编辑 → video-question.selectRange/clearRange → video_draft 同绑定范围编辑 → Run 冻结现场 → video.question → video.read_frames 多批有序帧与相交转写 → 成功观察 → source.present → 实际支持帧与间隙预览`。

Runtime 的提问提示按范围引导读取过程：每批最多 60 秒、8 个递增采样时刻；长范围分批取得材料，需要时补读变化位置和前后文。区域关注点不自动套用到全部过程帧。VI5 继续返回实际 PTS、相交转写与 `continuous_coverage: false`。

VI7 的来源门禁继续逐帧、逐区域校验成功观察。保存的来源只包含交付的支持材料；预览按实际时间展示帧，并列出引用帧之间“帧间过程未连续观察”的间隙。原片上下文供回看，不扩大引用的观察资格。

## 验证与复现

2026-10-10：Web 92 个用例、Server 草稿 4 个用例、Runtime 视频 4 个用例通过；Web 类型检查及 Server 构建通过。原片浏览器闭环通过，凭据见 [verification.json](../evals/video/results/vi8b-20261010/verification.json)、[tests.json](../evals/video/results/vi8b-20261010/tests.json) 与 [来源预览](../evals/video/results/vi8b-20261010/answer-source-preview.png)。

```powershell
. .\.worktree-local\environment.ps1
pnpm --filter @understand-book/web test src/video-question.test.ts src/components/VideoReaderPane.test.ts src/components/MediaSourcePreview.test.ts src/components/RightRail.test.ts src/App.startup.test.ts src/NetworkApp.chat.test.ts src/components/SourceExcerpt.test.ts
pnpm --filter @understand-book/web typecheck
cargo test -p server --lib video_draft
cargo test -p runtime --lib video_
cargo build -p server --bin server
node evals/video/vi8b-browser.mjs
```

草稿测试覆盖范围独立保存、双选择互相取消、最后内容释放、保存失败、长范围与原帧独立、错误时间/媒体拒绝、持久恢复、网络排队与接单。播放器测试覆盖起止输入、当前时刻设端点、放弃/取消、播放与编辑不改原帧。来源测试覆盖按时间排序与不可播放时仍保留间隙说明。

原片脚本沿 VI3 首发，选择 `555123–651000 ms`，分两批读取六帧及相交英文；来源仅交付首末两帧。使用独立 8794/5176 进程，结束后停止子进程。

实际 PTS 为 `555133.3125 / 585000 / 600000 / 615133.3125 / 630000 / 650000 ms`；首末帧展示 float16 与 bfloat16 的 `1e-8` 下溢示例。草稿仍绑定 `650000 ms` 的独立代码选区，播放到 700 秒后编辑、刷新、发送均保持同一现场。来源预览逐字节匹配首末输入图片，保留主播放器 507 秒 / 1.5 倍速，刷新后仍可回看。完整请求与六帧图片保存在 `evals/video/generated/vi8b/2026-10-10T07-45-39.920Z/`。

## 已知限制

- 原片浏览器使用 HTTP 记录器验证传递、真实图片、成功观察和来源回看，不执行语义推理；读时真模型与 DSH 沿已有确认待配置。
- Windows / Edge 为本轮验收环境；Linux、安装态、其他浏览器和完整逐字听核沿原待办处理。
