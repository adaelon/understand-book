# 视频读时取帧与观察（VI5）

## 入口与范围

`video.read_frames` 已接入 Resident 工具发现、单用户与多用户运行端口。运行读取自身 `RunScope` 的确切发布目录；切换当前窗口不会改写这次读取的发布。视频来源上下文提供媒体身份、时长和原尺寸，模型通过 `tool.search` 找到工具。

```json
{
  "location": {
    "video": {"media_id": "361e204a-50c7-40cd-9054-072b01a27659", "media_revision": "1"},
    "start_ms": 555123,
    "end_ms": 556123
  },
  "sampling": [555123],
  "region": [0.015625, 0.4444444444444444, 0.5833333333333334, 0.1388888888888889]
}
```

省略 `region` 读取整帧。一次读取最多 8 个递增时刻、60 秒时间范围、16 MiB PNG；一次模型请求最多积累 8 张视频图片。FFmpeg 整批解码限时 60 秒，并响应运行取消。需要更长过程时分批读取；返回的离散帧不表示整个时间范围已经观察。

## 数据流与归属

```text
RunScope 的 publication/book_dir
  → 校验确切 publication.json、视频清单与转写映射
  → FFmpeg -copyts / showinfo → 原始 PTS、宽高、PNG
  → FrameBatch（帧/裁剪位置、请求范围、相交英文句段）
  → RunContext.pending_video
  → 活动上下文预算筛选 → AgentRequestPlan.video_images
  → Native/ReAct image_url（PNG data URL，detail=high）
  → 主模型请求成功且未取消
  → RunContext.media_observations + video.observe 活动
```

`requested_ms` 保留采样请求；`actual_pts_ms` 按解码器整数 PTS 与 time_base 计算。裁剪采用原图像素，不缩放；左上取 floor、右下取 ceil，返回实际像素范围归一化后的 `FrameRegion`。原尺寸始终保存在 `FrameLocation`，图片尺寸单列。相邻请求落在同一实际帧时只保留一次。

发布目录保持只读。多用户缓存放在既有 `shared-cache/<publication_id>/video-frames/`；单用户使用临时目录下的同发布缓存。本工作区临时目录由 `.worktree-local/environment.ps1` 限定。缓存键使用请求时刻与整数裁剪参数，避免重复解码；其中只保存纯媒体派生数据。读取依然先经过运行归属及多用户发布授权。

视频图片与 `preview_images` 分开携带用途和来源，不授予演示检查资格。每次图片请求生成关联 ID，写入实际图像旁的元数据；成功后的 `MediaObservation` 记录同一个 ID、Run、发布及真实帧/区域。裁剪观察仅登记区域。工具输出文本、缓存文件路径、失败请求、取消请求和预算省略都不授予观察资格。循环达到显式轮数上限时，最后一次无工具回答请求也携带待处理图片。

本轮账本由 Runtime 持有；`video.observe` 活动保存观察 ID 和数量，启用完整观测内容时保存具体回执。PNG 不写入聊天历史。VI7 将消费本轮账本完成 `source.present`、来源按钮及历史引用持久化。

## 验证与重跑

```powershell
. .\.worktree-local\environment.ps1
cargo test -p runtime --lib video_
cargo test -p runtime --lib tool_registry
cargo test -p runtime --lib tool_exposure
cargo test -p server --lib video_frames
cargo build -p server --example video_frames_probe
node evals/video/vi5-runtime.mjs record
```

服务器测试使用真实 FFmpeg 教学夹具，比较实际 PTS、整帧和裁剪 RGB 像素；验证缓存、非法范围/区域、错误发布、取消及解码失败。Runtime 测试通过正式发现与工具循环验证换帧、局部观察、连续读取进展、无图片返回、模型失败/取消、预算省略和最终请求。

原片重放从 VI3 首发读取 VI0c 的代码、渐进解释及矩阵样例；Native/ReAct 分别保存图片、实际 HTTP 请求、Provider 响应和测试回执。`record` 的响应明确标为本地记录器，不执行模型推理。结果归档在 `evals/video/results/vi5-20261010/verification.json`，完整请求和图片在 `evals/video/generated/vi5/record/`。

## 已知限制

- 读时真模型按既有确认保留待验；本机本轮未配置 `OPENCODE_API_KEY`、`OPENCODE_BASE_URL` 和 `FLUID_LLM_MODEL`。配置后运行 `node evals/video/vi5-runtime.mjs live`，再对照代码/公式与换图回答验收。
- 视频页问答仍由 VI6/VI7 接入输入现场与来源交付后开放。DSH、Linux、安装态及完整逐字听核沿原待办推进。
- 缓存依赖不可变发布与 FFmpeg；更换原视频必须建立新材料/发布身份。测试回执不作为用户阅读证据。
