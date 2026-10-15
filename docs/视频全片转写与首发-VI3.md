# 全片转写与首个发布（VI3）

## 操作入口

在已完成 VI2 导入的工作区运行。模型配置沿用 VI0b 已验证的 `faster-whisper / small.en / CPU int8 / 8 threads`，模型和 Python 由操作者显式指定。

```powershell
. .\.worktree-local\environment.ps1
node --import tsx skills/build/automatic-build.ts video-transcribe --workspace 'evals/video/generated/vi2/moved-workspace' --python '.worktree-local/whisper-venv/Scripts/python.exe' --model-dir '.worktree-local/whisper-models/small.en'
node --import tsx skills/build/automatic-build.ts video-status --workspace 'evals/video/generated/vi2/moved-workspace'
cargo run -p server --bin publish_book -- "$PWD/evals/video/generated/vi2/moved-workspace/.build/reader-service" import "$PWD/evals/video/generated/vi2/moved-workspace"
```

重复 `video-transcribe` 恢复失败或中断的工作。识别失败时命令返回非零；所有成功响应保留。最后一条命令沿既有 `PublishedLibrary` 写入不可变发布，并把首个 `PublishedBookRef` 关联回原媒体。将本地发布服务目录放在工作区内，整体移动时相对链接继续有效。

多人部署先按 VI2 登记准备态视频，再把 `publish_book` 的服务根设为同一服务根。发布会将现有媒体读者授权给首个发布，并更新准备态关联；正文接口仍检查发布授权。普通读者只能消费材料，导入、识别和发布由维护方执行。

## 构建与恢复

```text
video_import.json + media/original.mp4
  → video_build.json 中的连续核心区间计划
  → FFmpeg 16 kHz mono PCM（5 分钟核心、两侧各 2 秒上下文）
  → 本地 Whisper → transcript/raw/chunk-NNN.json
  → 重叠词组连接 / 无语音连接 → 英文段落与 LID、TranscriptMap
  → video_transcript 阶段事务 → PublishedLibrary → video_publication.json
```

Build Engine 持有计划、状态、重试次数、媒体与转写身份。Python 适配器只接收 PCM 并保存完整原始响应；没有远端任务 ID，持久任务身份是媒体身份和 `chunk-NNN`。各块成功后即保存，后续块失败不清空成功结果；服务响应已落盘但 Engine 尚未记完成时，恢复直接接纳该响应。

`video_build.json` 的每块状态分别为 pending / running / completed / failed。成功空响应表示该范围已处理但没有识别文本；失败保留原范围及原因。整片计划成功后才能生成完整覆盖。全静音材料保存处理结果，并报告没有英文正文，不创建空 Book。

相邻识别块用重叠区域的共同词序列确定连接，保留一次该词序列及前后正文。两边在连接时刻附近均没有识别语音时可按无语音连接；仍有语音且无法匹配时停止发布，保存连接失败。连接记录在 `transcript/joins.json`，低于 0.5 的机器词概率仅作为待复核线索保存，不用于删除词语。

整讲实际出现四个零时长短句和一处相邻句时间重叠。它们与相邻句合为一个段落，以已有词时间的包围区间定位；文字和原始词估计全部保留，合段记录写入 `timing_groups`。不生成推测的逐词时长。

## 产物与读取

| 文件 | 用途 |
| --- | --- |
| `video_build.json` | 恢复入口、逐块处理覆盖、失败、身份与配置 |
| `transcript/raw/chunk-NNN.json` | 每次成功识别的完整响应、词概率、VAD 信息、实际版本和耗时 |
| `transcript/raw_results.json` | 发布内保留的全片原始响应和配置汇总 |
| `source.txt`、`base.json` | 保持讲者顺序的英文正文、现有段级 LID、初始空语义图谱 |
| `transcript_map.json` | 英文句段/LID/UTF-16 范围与原片时间的关系 |
| `video_source_manifest.json` | 原媒体身份、转写版本、全片语言覆盖及其他能力待处理区间 |
| `transcript/joins.json` | 块连接依据和低概率词线索 |
| `video_publication.json` | 确切发布引用及相对发布目录 |

Reader 的 `Book::load` 校验正文映射、完整语言覆盖和媒体/原始响应依赖；媒体本身不加载进 Book 内存。发布时复制这些依赖，并沿既有只读目录与登记边界提交。相同 book_id 的后续发布比较原片字节，替换媒体要求新 book_id。

发布 readiness 为 `transcript_ready`，`english_searchable=true`、`semantic=pending`。现有 `book.text` / `book.search_text` 可读取和检索完整英文；图谱、讲解结构及教学资产继续按各自状态提供。

本地/多人准备态播放器在首发关联后加载 `/api/videos/<id>/<revision>/transcript`，按句段按钮跳回原片，保持同一个播放器及原媒体进度键。正式发布的正文映射与媒体分别通过 `video_transcript`、`video/media` 提供；媒体沿已有授权后使用流式 Range/HEAD。

## 限制

- 公开定位精度为句段；机器词估计与概率保留在原始结果，逐词时间没有人工对齐。
- 技术词拼写、完整人工逐字真值与 V07 停顿继续沿 VI0b 的待核记录；原识别结果不自动改成画面文字。
- 读时真实帧、提问现场与引用交付由 VI5–VI7 接入，视频页暂不开放问答。英文发布就绪与视觉伴读就绪分别显示。
- 本机 Python、模型和 FFmpeg 是显式依赖；Linux 与安装包实测由 VI18 完成。

## 验证

定向测试覆盖连续计划、重叠词连接、无法连接时阻止发布、失败/静音区分、只恢复缺失识别、工作区移动、缺依赖/覆盖缺口拒绝发布、初始空图谱 Reader 加载、发布授权、句段回看和播放器保持。完整原片的实际结果记录在[视频验收](验收-视频理解.md)。
