# 视频导入与原片播放（VI2）

## 本地入口

开发环境先加载 `.worktree-local/environment.ps1`。导入入口属于现有 Build CLI：

```powershell
node --import tsx skills/build/automatic-build.ts video-import '本地原片.mp4' --workspace '.understand-book/lecture-video'
node --import tsx skills/build/automatic-build.ts video-status --workspace '.understand-book/lecture-video'
node --import tsx skills/build/automatic-build.ts video-resume --workspace '.understand-book/lecture-video'
cargo run -p server --bin server -- '.understand-book/lecture-video'
pnpm --filter @understand-book/web dev --port 5175 --strictPort
```

打开 `http://localhost:5175` 后直接进入指定视频。书库根目录下已导入的视频也显示在本地入口，可选择观看。前端使用浏览器原生播放、暂停、进度拖动和全屏控制；播放位置与倍速按用户和原片身份保存，刷新后恢复为暂停状态。

首个支持组合为 **H.264 视频 / AAC 音频 / MP4 容器**，本次真实样本为 1920×1080 CS336 2026 P2。导入依赖 `ffprobe` 与 `ffmpeg`；可用 `FFPROBE`、`FFMPEG` 环境变量指定可执行文件。导入会排除附属封面流，探测导入副本并实际解码首帧。播放器解码失败时显示浏览器错误提示。

## 工作区与恢复

```text
video-import → copying → probing → awaiting_transcript
                   ↘ failed ← ↗
video-resume → 已有完整副本则重试探测；未完成复制则重新复制
```

| 文件 | 责任 |
| --- | --- |
| `video_import.json` | `VideoImport` 准备状态、原片身份、复制字节进度、失败原因与 `VideoSourceAsset` |
| `media/original.mp4` | 导入时逐字节复制的原件；播放及后续处理读取此文件 |
| `media/original.mp4.part` | 复制中的临时文件；完成后改名，中断后恢复会重新复制 |
| `media/probe.json` | 实际 ffprobe 媒体信息 |

外部绝对路径只保留为来源信息。完整复制后即使外部原路径失效，`video-resume` 仍读取副本；整体移动工作区保持 `media_id/media_revision` 和相对路径。重试不会创建新媒体身份。另一份输入必须使用新工作区。

复制中的字节进度与探测阶段持续可读；失败原因保存在状态中。全片转写尚未执行，准备页显示等待状态并禁用提问。此阶段不会写 `base.json`、正文或伪造发布身份。VI3 将在全片英文正文与时间映射成立后关联首个发布。

## 多人服务登记与授权

材料维护方先在服务所在机器完成同样的本地导入，再停止服务，沿现有 ServiceWriter 离线登记：

```text
reader_maintenance register-video <绝对服务根> <导入工作区> <用户ID> [用户ID...]
reader_maintenance video-readers <绝对服务根> <media_id> <media_revision> [用户ID...]
```

`video-readers` 替换允许读取的用户集合；省略用户表示撤销全部访问。用户必须为已有启用账号。登记在 `library/videos/<media_id>/<media_revision>/` 保存媒体快照、准备记录及 `readers.json`，原工作区随后可以移动。重复登记同一身份不会覆盖已登记原片。

网络书架列出该账号获授权的准备态视频。`/api/videos` 返回目录，`/api/videos/<id>/<revision>/status` 返回进度，`media` 返回媒体，`ask` 在此阶段返回 `409 VIDEO_TRANSCRIPT_NOT_READY`。所有网络入口先执行已有会话、账号、Origin/CSRF 规则，再检查该媒体的读者授权；状态、HEAD 和 Range 不绕过授权。公开投影不包含来源文件路径。

本地与多人服务共用 `video_media::stream_response`，支持 GET、HEAD 和单个字节范围：正常范围 `206 + Content-Range`，越界 `416 + bytes */长度`，无 Range 返回 `200`。读取基于文件句柄及限定长度，媒体字节不加载进 Book；暂停与跳转由浏览器的范围请求驱动。

## 复跑验收

```powershell
pnpm -C packages/core exec vitest run test/video-import.test.ts test/source-manifest.test.ts
pnpm -C packages/web exec vitest run src/components/VideoReaderPane.test.ts
cargo test -p server --lib video_
node --import tsx evals/video/vi2-import.ts 'evals/video/generated/vi2-new'
# 将后端指向上一步的 moved-workspace，并启动前端后运行：
node evals/video/vi2-browser.mjs 'evals/video/generated/vi2-new'
```

真实原片验收只移动任务生成的外部副本和导入工作区，用户提供的原片保持原位。浏览器验收在 555.123、1200、4620 秒跳转，实际播放后暂停、刷新恢复，检查响应字节、HEAD 长度及准备态问答拒绝。结果见[视频验收](验收-视频理解.md)。

## 已知限制

- 支持结论限于本次 Windows 开发环境与 Edge 实播的样本组合；Linux 机器、桌面安装包和其他编解码组合留 VI18 验收。多人服务分支的真实 HTTP、授权、Range 与 HEAD 在本机测试。
- 当前入口为本地 Build CLI；普通网络读者消费维护方登记的材料。整讲转写、首个发布及进度接续由 VI3 实现，读时取帧与问答由 VI5–VI7 接入。
- 浏览器存储的观看进度在同一站点与用户下恢复；跨浏览器同步尚未实现。
