# 暂停画面框选（VI8a）

## 操作与草稿

视频阅读页点击“框选画面”暂停当前画面，拖动选择代码、公式或图示。有效选区保存为草稿内容，问题栏显示绑定时间与“已选区域”。退出框选、Escape、指针取消和没有面积的点击不建立草稿。

草稿固定发布、材料、实际 PTS 和原尺寸。已有文字时，框选和重新框选只编辑该绑定帧上的区域；继续播放后框线隐藏，“回到问题画面”恢复原帧后可重新框选。布局改变只重算显示位置。

取消选区保留已有文字与原帧；没有文字或时间范围时释放草稿。“取消草稿”同时释放文字和选择。发送失败保留现场，接单后沿 VI6 移交给 Run 与历史。区域草稿支持刷新恢复。

## 坐标与链路

`浏览器实际帧 → object-fit: contain 的图像显示区 → 指针位置 → 最近原图像素边界 → FrameRegion.rect_normalized → VideoQuestionContext → 服务器草稿 → Run → video.read_frames(region) → 本轮观察 → source.present → 原区域预览`。

`videoContentRect` 扣除上下或左右黑边，选区层覆盖实际图像。指针捕获支持反向拖动及拖出画面，末端限制在原图范围内；选区至少跨越一个原图像素。布局或播放在拖动中改变时取消未完成手势。浏览器布局与指针的亚像素精度不同，边界贴齐最近原图像素，避免不同布局向外取整多出一列。

服务器允许修改同一草稿的 `region`，继续固定 `draft_id`、发布、材料、原帧和既有范围。VI1 校验区域属于绑定帧；VI5 读取原分辨率局部或整帧；VI7 保存实际观察区域并在来源预览中重读原像素。

## 验证与复现

2026-10-10：90 个不重复 Web 用例、3 个 Server 草稿用例通过，Web 类型检查和 Server 构建通过。覆盖宽窄/黑边映射、反向/越界/无面积手势、指针取消与 Escape、播放隐藏、草稿编辑/取消/失败/恢复、禁止改绑与接单移交；相邻播放器、问答栏、本地/网络启动及来源预览回归通过。

原片浏览器使用 VI3 首发，在 `650000 ms` 的 `1920×1080` 帧选中 `[120, 480, 1120, 150]` 像素区域。756 像素宽播放器、340 像素宽播放器、上下黑边、左右黑边四种布局得到同一区域。继续播放、窗口缩放、回到绑定帧、刷新和发送保持同一现场。1120×150 PNG 进入读时请求，回答来源预览返回相同图片字节；预览保持主播放位置与倍速，刷新后历史来源仍能读取。

```powershell
. .\.worktree-local\environment.ps1
pnpm --filter @understand-book/web test src/video-region.test.ts src/video-question.test.ts src/components/VideoReaderPane.test.ts src/components/RightRail.test.ts src/App.startup.test.ts src/NetworkApp.chat.test.ts src/components/MediaSourcePreview.test.ts src/components/SourceExcerpt.test.ts
cargo test -p server --lib video_draft
cargo build -p server --bin server
node evals/video/vi8a-browser.mjs
```

脚本使用独立 8794/5176 进程和本地 HTTP 记录器，完成后停止子进程。凭据：[verification.json](../evals/video/results/vi8a-20261010/verification.json)、[tests.json](../evals/video/results/vi8a-20261010/tests.json)、[宽屏框选](../evals/video/results/vi8a-20261010/selection-wide.png)、[区域来源预览](../evals/video/results/vi8a-20261010/answer-source-preview.png)。

## 已知限制

- 原片 HTTP 记录器验证真实裁剪、图片请求、观察与来源回看，不执行语义推理；读时真模型与 DSH 沿既有确认待配置。
- 验收环境为 Windows / Edge。Linux、安装态及其他浏览器沿 VI18 验收；起止时间范围界面由 VI8b 接续。
