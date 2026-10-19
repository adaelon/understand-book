# VI0a 视频样本与媒体基线

样本合同是 [samples.json](samples.json)，验收结论见 [视频验收](../../docs/验收-视频理解.md)。原片采用 VQ27 的 CS336 **2026 P2《PyTorch》1080P**。本目录是独立实验入口，输出供 VI0b/VI0c 使用。

VI0b 的本地 Whisper 配置、转写／短音频复核、任务状态与恢复入口见 [WHISPER.md](WHISPER.md)。

VI0c 的真实图片 MCP、Codex 换图实验、Runtime 请求记录、容量测量与待验证通路见 [IMAGES.md](IMAGES.md)。

VI2 产品导入、准备态播放与服务端登记见[导入与播放说明](../../docs/视频导入与播放-VI2.md)。`vi2-import.ts` 用完整原片的任务副本验证外部路径失效和工作区移动；`vi2-browser.mjs` 验证真实浏览器播放、跳转、恢复与 HTTP Range/HEAD。

VI5 读时工具、实际 PTS、裁剪和观察回执见[读时取帧说明](../../docs/视频读时取帧与观察-VI5.md)。`vi5-runtime.mjs record` 验证确切 VI3 首发至 Native/ReAct 的真实图片请求，`live` 供配置后的读时真模型验收。

VI6 输入现场与窗口草稿见[提问现场说明](../../docs/视频提问现场-VI6.md)。先构建 Server，再运行 `node evals/video/vi6-browser.mjs`，用 Edge 和原片验证 08:12 输入、08:27 编辑/发送、刷新恢复、历史绑定及真实帧请求；脚本独占临时 8794/5176 并在结束后停止服务。摘要与界面见 [results/vi6-20261010](results/vi6-20261010/)。

## 重跑

VI8b 起止时间范围见[范围提问说明](../../docs/视频起止时间范围提问-VI8b.md)。先构建最新 Server，再运行 `node evals/video/vi8b-browser.mjs`：原片 95.877 秒范围分两批读取六帧，仅交付两帧来源并展示间隙，核对双选择、取消、刷新、接单及独立预览。脚本使用临时 8794/5176 和 HTTP 记录器，结束后停止进程；凭据见 [results/vi8b-20261010](results/vi8b-20261010/)。

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

用户于 2026-10-08 确认“保留待听核项，先完成可验证部分”。2026-10-10 的 VI0b 已补入 V05 两个术语与 V04 关键短语的用户听核确认；V07 仍为不确定语义。见 [listening-review.json](results/vi0b-20261010/listening-review.json)。`audio_truth.correct_words` 保持 `null`、`pending_listening` 表示完整逐字真值待核；局部确认在 `listening_review` 单列。烧录字幕只写入 `candidates_from_burned_subtitles`。

打开 VI0b 生成的 `generated/vi0b/index.html` 可对照原声与机器时间戳；继续逐字听核后再填完整原话及人工句段边界。V05 两词均出现的问题已确认；V07 不能再作为整段静音样本。不能把字幕缺失标为静音，也不能直接把视觉无答案样例扩大为全片音频无答案。

## 已知限制

VI0a 可验证部分与 VI0b 本地音频服务实验完成，音频完整逐字真值仍待听核。原片覆盖是选定短片段，未宣称全片已标注；“同位置现场编辑”和“离开页面再返回”使用明确标注的补充夹具。Codex 原生图片实验与读时请求投影见 VI0c；读时真模型及 DSH 按用户确认留待验证。Linux、正式安装包、生产音频与图片接入由后续切片验收。

## VI3 全片转写与首发

[正式入口与恢复](../../docs/视频全片转写与首发-VI3.md)。完整原片在 `generated/vi2/moved-workspace`，16 块转写、720 段正文、映射及首发已完成；证据见 [results/vi3-20261010](results/vi3-20261010/)。

`vi3-verify.ts` 核对完整处理覆盖与正文映射；`vi3-recovery.ts <新输出目录>` 运行实际 Whisper 缺音频失败/恢复；`vi3-browser.mjs` 在本地服务上核对首发保持播放器、句段回看和刷新恢复。原始音频/视频与全量响应保留在生成工作区和发布包。

## VI7 媒体来源与原片预览

先加载工作区环境并构建 `cargo build -p server --bin server`，再运行 `node evals/video/vi7-browser.mjs`。脚本使用 VI3 首发与本地 HTTP 记录器，验证页面发送、实际帧来源、独立播放、Esc 关闭、主动进入和刷新回看；不执行模型语义推理。完整输出在 `generated/vi7/<时间>/`，本轮摘要和截图见 `results/vi7-20261010/`。合同与限制见 [VI7 说明](../../docs/视频来源交付与预览-VI7.md)。
## VI4 正式执行器联合输入

加载工作区环境后运行 `node --import tsx evals/video/vi4-executor.ts live <Codex 可执行文件> [模型名]`。脚本将原英文摘录和 650 秒整帧/代码裁剪交给正式四工具会话，保存真实 MCP 图片、候选提交及接纳。隔离工作区与原始请求在 `generated/vi4/<时间>/`，摘要见 [results/vi4-20261010](results/vi4-20261010/)。合同与范围见 [VI4 说明](../../docs/视频外部构建图片输入-VI4.md)。

## VI9 累积发布与双窗口接入

加载工作区环境后运行 `node --import tsx evals/video/vi9-browser.mjs`。脚本复制 VI3 原片工作区，在独立 8794 / 5176 / 5177 端口启动真实多人宿主、HTTPS 代理及 Web，调用正式发布器接纳四段中文夹具、两张真实帧与索引。需要本机 Edge、OpenSSL、Rust 和现有原片；结束时关闭本次进程。

双窗口分别验证忙时固定、空闲接入、媒体元素与观看位置保持、聊天保存/恢复、缺依赖拒绝和后到通知不回退。记录器只提供可控制终态的回答。完整输出在 `generated/vi9/<时间>/`，成功记录在 [results/vi9-20261010](results/vi9-20261010/)，发布成本及合同见 [VI9 说明](../../docs/视频同源累积发布与接入-VI9.md)。

## VI10 整讲中文与缺批恢复

加载工作区环境后运行 `node --import tsx evals/video/vi10-run.ts <Codex可执行文件>`，使用真实外部 Harness 翻译 720 段，按 120 段一批累积发布。输出在 `generated/vi10/`；同命令恢复缺失任务，已接纳成果保留。默认 CLI 模型需已可用，本次使用 low reasoning。

`node evals/video/vi10-browser.mjs` 从英文首发验证中文自动接入、英文锚点、播放现场、整讲覆盖与刷新；使用独立 8794/5176。结束后运行 `node evals/video/vi7-browser.mjs evals/video/generated/vi10/workspace` 验证双语发布的原片来源与历史回看，回答采用本地 HTTP 记录器。`node --import tsx evals/video/vi10-verify.ts` 汇总整讲、已保存的中断恢复样本、10 段语义抽查、浏览器结果和成本；该汇总依赖本轮 `recovery-before.json`，不是自动制造中断的脚本。

成功记录在 [results/vi10-20261010](results/vi10-20261010/)，操作与限制见 [VI10 说明](../../docs/视频整讲中文分批补齐-VI10.md)。

## VI11 候选扫描与视觉事件

加载工作区环境后运行 `node --import tsx evals/video/vi11-run.ts <Codex可执行文件>`。脚本从 VI10 产物建立独立工作区与书库，全片扫描后对 550–655 秒、1180–1225 秒执行真实模型任务，并运行独立 A→B→A 补充夹具；重跑恢复缺失任务。原片 22 个 VU 分两批发布，保留 720 段中文。

`vi11-calibrate.ts` 比较同一标注样本的采样间隔和阈值，`vi11-camera.ts` 验证运动面板不会挤掉教学变化。`node --import tsx evals/video/vi11-verify.ts` 从旧、新正式发布回读代表及前后帧，逐像素比较、核对覆盖缺口、转写与译文保持，并验证完成态不再启动模型或重复发布；失败后修复对应取帧、接纳或恢复路径。

产物位于 `generated/vi11/`，摘要见 [results/vi11-20261010](results/vi11-20261010/)。本轮扫描覆盖全片，语义验收覆盖 150 秒及补充夹具。通用入口、参数、模型与存储成本见 [VI11 说明](../../docs/视频算法候选与视觉事件-VI11.md)。

## VI16 校正草稿与采纳

`node --import tsx evals/video/vi16-run.ts <Codex可执行文件>` 在独立维护副本生成真实术语/疑点草稿，复用 VI15 结果并请求新的 590–598 秒原声复核。支持 `VI16_OUTPUT` / `VI16_AUDIO_EXE`，后者默认 `CARGO_TARGET_DIR/debug/examples/video_audio_probe.exe`。同命令恢复已冻结请求和已保存草稿。

`node --import tsx evals/video/vi16-verify.ts` 在新的副本中验证部分/整批采纳、拒绝与人工编辑冲突，按字节核对规范英文、时间映射及发布保持。原始模型草稿不采纳。结果位于 `generated/vi16/`，归档见 [results/vi16-20261011](results/vi16-20261011/)，维护命令及 VI17 输入见 [VI16 说明](../../docs/视频人工编辑与校正草稿-VI16.md)。

## VI17 来源改版

`node --import tsx evals/video/vi17-run.ts CODEX_EXE PUBLISHER` 创建独立书库和新版，消费 VI16 部分采纳输入及 VI10 的真实翻译任务。`node --import tsx evals/video/vi17-verify.ts` 验证 tensor 中文/语义更新、未变输入复用、前版保持和完成态恢复。`node --import tsx evals/video/vi17-replacement.ts` 裁取实际原片，调用真实 Whisper 验证替换媒体身份和新时间码。运行前加载工作区环境；`VI17_OUTPUT` 可指定独立输出目录。

归档见 [results/vi17-20261011](results/vi17-20261011/)，正式维护命令见 [VI17 说明](../../docs/视频来源改版与派生更新-VI17.md)。原正式发布和 VI16 原始草稿保持。

## VI12 视觉检索与原片精读

构建 `cargo build -p read-tools --example video_search` 和 `cargo build -p server --example video_frames_probe` 后，运行 `node evals/video/vi12-verify.mjs`。支持 `VI12_SEARCH_EXE` / `VI12_FRAME_EXE` 覆盖可执行文件位置。脚本消费 VI11 固定发布，以默认八项分页检索十四个查询场景，再沿 VI5 回读十三帧并用本地 HTTP 记录器核对真实图片请求与观察回执。

范围包括原片代码/渐进高亮/重复主体/自然语言、GPU 序列号负例、未索引当前帧和独立 A→B→A 夹具。原片发布只读，补充夹具的合成说明在 `generated/vi12/supplement` 整理为可读测试副本。完整查询、请求与图片在 `generated/vi12/`，验收见 [results/vi12-20261010](results/vi12-20261010/)，合同与限制见 [VI12 说明](../../docs/视频视觉检索与精读-VI12.md)。


## VI14 讲解结构与原片对照

`node --import tsx evals/video/vi14-run.ts <Codex可执行文件>` 冻结 einsum 的两组 VU 与四段英文，经正式四工具会话生产有来源的讲解角色和关系。随后用 `skills/build/video-discourse.ts run <工作区> discourse-vi14-einsum <Codex可执行文件> <publish_book可执行文件> <服务根>` 累积发布；已完成任务直接复用。

`node evals/video/vi14-verify.mjs` 使用 V04 原问题、求和子问题和无依据负例，对比原输入发布与新增 discourse 的发布，检查视觉候选保持、关系独占候选、新旧成果及实际帧/文本回读。支持 `VI14_SEARCH_EXE`、`VI14_FRAME_EXE`；默认取 `CARGO_TARGET_DIR/debug/examples/`。本机沿 D 盘构建环境，详细入口见 [VI14](../../docs/视频讲解结构与检索-VI14.md)，产物在 `generated/vi14/`。


## VI18 整讲与安装

`node --import tsx evals/video/vi18-run.ts DRIVER CODEX_EXE PUBLISHER visual` 先完成四个独立分区的全讲视觉；`semantic` 接续图谱和讲解，每会话最多三个独立任务。DRIVER 可为编译 Build Engine 或源码 `skills/build/automatic-build.ts`。设置 `VI18_OUTPUT` 指定生产根；同一分区只运行一个写入者，重跑只补未接纳成果。

`node --import tsx evals/video/vi18-verify.ts` 验证全讲覆盖、720 段、原正文、已完成视觉任务恢复次数，并记录真实调用用量、耗时和体积。金额未从 CLI 返回，保留未知。`vi18-reader.mjs SERVER WEB_DIST PUBLISHED_BOOK INSTALL_DIR [VIDEO_RESOURCES]` 验证独立布局的播放、跳转、刷新、原媒体 Range/HEAD。

`apps/desktop/scripts/smoke-video-release.mjs SIDECAR PUBLISHER ORIGINAL_MP4 OUTPUT_DIR [AUDIO_REVIEW]` 验证编译入口、随包工具、实际 Whisper、缺项恢复和原片发布。实际凭据在 [VI18 结果](results/vi18-20261011/)，安装说明见 [VI18](../../docs/视频安装与整讲验收-VI18.md)，当前进度见 [实施状态](../../docs/VI18-实施状态.md)。

VI18 最终结果（2026-10-11）：543/383/383 个视觉/图谱/讲解任务全部接纳，720 段双语和四项能力全片覆盖。最终 publication `01a129cf-c1b7-72c2-bac7-40f9740fda6f`；`verification.json` 和 `reader.json` 均 passed，Windows NSIS 发布门通过。全部贡献已完整时 semantic 恢复只发布一次，原运行日志由报告路径读取。原生安装升级/WebView、Linux、DSH、读时真模型及完整逐字真值的范围见报告已知问题。
