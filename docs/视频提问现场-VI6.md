# 视频提问现场（VI6）

## 完成状态与操作

2026-10-10：VI6 实现与确定性验收完成。已有英文首发的视频可以进入阅读器，在问答栏保存绑定当前画面的草稿；播放器显示当前位置，问答栏独立显示问题绑定位置。设计依据：[切片方案 §5 / VI6](切片方案-视频理解与渐进伴读.md)。

首次实际非空输入暂停一次并建立绑定。聚焦不暂停；输入法、粘贴和后续编辑沿用同一现场，用户可继续播放。清空文字且没有区域/范围，或点击“取消草稿”，释放绑定；下一次输入建立新草稿。发送失败保留原绑定，服务器接受后移交给回合。

## 数据流

`浏览器实际呈现帧 → VideoQuestionContext → 窗口草稿 → turn.accepted / FrozenTurn.reader → RunScope.reader_input → video.question → 按需读取真实帧`。

`VideoQuestionContext` 保存 draft_id、PublishedBookRef、视频/转写材料、实际 PTS 和原尺寸，以及可选区域/范围。草稿保存时校验原发布与材料，同一草稿不能更换绑定帧；VI8a 允许编辑该帧上的区域。提交只引用窗口与草稿 ID，服务器从已保存草稿取现场。

服务器按窗口保存草稿。网络沿现有 Workspace CAS 与 checkpoint，本地沿 session 旁的 `*.video-drafts.json` 保存。接单先提交原有回合日志，再清除移交的草稿；清理失败时，恢复读取仍以已接单回合为准。网络移交只更新草稿 checkpoint，不增加 Reader revision，避免本次接单使自己的后续 Reader 操作过期。

Pending Turn、冻结输入和历史均保存同一个现场。排队与服务恢复从原冻结输入启动，取消排队仍保留历史现场。`video.question` 进入首个模型请求；真正看见画面的依据继续由 VI5 的真实帧请求和成功观察产生。

## 实际帧精度

播放器通过 `requestVideoFrameCallback` 读取实际呈现帧，跳转期间禁止建立新绑定。`book/video_context` 从原发布返回材料、英文映射和原视频 stream time_base；浏览器将回调的 mediaTime 还原成该流整数 PTS，按原精度保存。

原片跳转至 492 秒时，浏览器回报 `491.966688 s`，原流实际为 `491966.6875 ms`（time_base `1/16000`）。直接使用舍入值会使原帧早于请求起点，导致 VI5 解码拒绝；恢复整数 PTS 后，输入绑定和真实解码精确相同。界面标签就近显示 `08:12`，存储和读取保持原精度。

## 验证与复现

前端用例覆盖首次输入、输入法/粘贴/编辑、不重复暂停、清空/取消、失败保留、异步恢复和窗口隔离；服务端用例覆盖本地持久保存、移交、原发布历史，以及网络排队、幂等接单、冷恢复、取消、双窗口与恢复后的首个模型请求。普通聊天和接单恢复回归一并验证，结果见[视频验收 VI6](验收-视频理解.md)。

在仓库根目录运行：

```powershell
. .\.worktree-local\environment.ps1
cargo build -p server --bin server
node evals/video/vi6-browser.mjs
```

脚本使用已有 `evals/video/generated/vi2/moved-workspace` 的 VI3 首发，启动独立后端 8794、前端 5176 和本地 HTTP 记录器，结束时停止子进程。Edge 真实播放验证“08:12 输入、08:27 编辑/发送”、刷新恢复、原发布接单和真实帧读取。请求、回合和截图写入 `evals/video/generated/vi6/<时间>/`，摘要见 [verification.json](../evals/video/results/vi6-20261010/verification.json)。

## 已知限制

- 视频页发送与来源预览已由 [VI7](视频来源交付与预览-VI7.md) 开放，框选已由 [VI8a](视频暂停画面框选-VI8a.md) 接入；本片的历史验收使用正式接单 API，现有脚本兼容已开放入口。时间段选择界面由 VI8b 接入。
- 实际 HTTP 接收端为本地记录器，不作语义推理；读时真模型与 DSH 沿既有确认待验。
- 本次验证环境为 Windows / Edge，依赖 FFmpeg、ffprobe 与浏览器实际帧回调。Linux、安装态和其他浏览器未验。
