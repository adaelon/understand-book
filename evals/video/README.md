# VI0a 视频样本与媒体基线

样本合同是 [samples.json](samples.json)，验收结论见 [视频验收](../../docs/验收-视频理解.md)。原片采用 VQ27 的 CS336 **2026 P2《PyTorch》1080P**。本目录是独立实验入口，输出供 VI0b/VI0c 使用。

## 重跑

在仓库根目录运行，要求 Node.js、FFmpeg、ffprobe 可执行。已验证 Windows x64 / Node 24.9.0 / FFmpeg `N-116037-g539d2e989d-20240628`；需要 H.264/AAC 解码、libx264/PNG/PCM 编码、drawtext 及 Consolas 字体。

```powershell
node --test evals/video/video-media.test.mjs
node evals/video/media.mjs replay
node evals/video/media.mjs verify
```

原片移动后可用 `--source '完整路径'`，输出可用 `--out '目录'`；`replay` 和 `verify` 应传同样的参数。也可设置 `VI0A_SOURCE`、`FFMPEG`、`FFPROBE`。`replay` 会覆盖指定目录中同名生成物；原片保持只读。

结果默认写入被 Git 忽略的 `generated/`：

- `index.html`：按问题查看原分辨率图片、短视频、原声音频和待听核项；可在浏览器本地打开。
- `frames/*.png`：14 张 1920×1080 图片；11 张原片帧，3 张补充夹具帧。
- `clips/*.mp4`、`*.wav`、`*.location.json`：5 段视频、4 段 PCM 原声及各自原时间映射。
- `replay.json`：实际工具版本、原片 ffprobe 响应、请求时刻/实际 PTS 和产物相对路径。
- `verification.json`：19 项原片重读与解码结果。
- `supplement.mp4`：6 秒独立教学夹具，依次显示 A→B→A，原生无音轨。

2026-10-08 的机器结果保存在 [results/vi0a-20261008](results/vi0a-20261008/)。其中相对媒体路径以该次 `generated/` 为根；清单、脚本和 JSON 入库，媒体可由原片重建。

## 定位与数据流

```text
samples.json + VQ27 原片
    → ffprobe 选择非封面视频流及音频流
    → 原时间轴取帧 / 原分辨率短视频 / PCM 音频
    → replay.json + 每段 location.json + 本地查看页
    → verify 从原片重读 → 像素、音频采样、时间映射结果
```

帧使用解码器的 PTS ticks 与 time_base 计算 `actual_pts_ms`，请求时刻另存。取请求时刻之后的第一帧；本样本容许差值小于 34 ms。例：请求 `555123 ms`，实际帧 `555133.3125 ms`，不是将请求值冒充实际值。

片段时间为左闭右开区间：`source_ms = source_start_ms + clip_local_ms`。`*.location.json` 可随片段移动。播放 MP4 是有损副本；PNG 保留原分辨率，WAV 使用原采样率与声道的 PCM。音频快取先预解码一秒恢复 AAC 状态，再裁到请求区间。

补充夹具有独立 `source_id` 和 0–6000 ms 时间轴。教学代码逐字依据原片 555.123s/650s 的代码实例；固定位置、微改和重复次序由 `makeSupplement` 构造。它只用于检查变化与重复定位，不表示讲者在原片做了这些操作。

## 验证责任

单测针对封面流误选、PTS 精度、非零时间偏移、实际 FFmpeg 微改/重现、AAC 截取以及待听核标注。失败后修改媒体提取或样例引用，不调整语义答案来迁就错误输出。

`verify` 从原片重取所有帧，逐像素比较 PNG；完整解码短视频，并在每段开头和中点对照原片，容许有损编码平均通道误差小于 3/255；逐段用从头解码的方式重取原声，与 WAV 逐采样严格比较。PCM 时长与容器指定范围允许不足 1 ms 的采样边界差异，并保存实际时长。

语义标注依据实际 1080P 画面读取，机器比对证明媒体复现一致。两种凭据分别记录。没有模型回答或模型自评分参与本轮验收。

## 音频听核交接

用户于 2026-10-08 确认“保留待听核项，先完成可验证部分”。`audio_truth.correct_words` 当前保持 `null`，状态为 `pending_listening`。烧录字幕仅写入 `candidates_from_burned_subtitles`。

打开查看页的 V04、V05、V07，回听对应 WAV；听核后填正确原话、实际原片句段范围、核对者及无法确认的词。V05 要核实讲者是否切换到 overflow 话题；V07 要核实有无语音/停顿。不能把字幕缺失标为静音，也不能直接把视觉无答案样例扩大为全片音频无答案。

## 已知限制

当前可验证部分完成，音频真值仍待听核。原片覆盖是选定短片段，未宣称全片已标注；“同位置现场编辑”和“离开页面再返回”使用明确标注的补充夹具。仅当前 Windows 开发环境已验证；Linux、正式安装包、音频服务与主模型图片通道由后续切片验收。
