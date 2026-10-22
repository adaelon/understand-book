# CS336 视频首帧延迟：根因与分片播放验证

2026-10-21，针对线上 **CS336 第 2 讲：PyTorch 与资源核算** 的加载等待进行诊断。现网版本为 `/opt/understand-book/releases/workspace-library-title-20261021`，服务 PID 541359。此次仅生成诊断记录和隔离播放副本，没有修改产品代码、替换原视频或部署服务。

## 结论

当前播放器直接读取 226 MB 原片，启动需要下载全片索引和浏览器探测所需的数据。原片 `moov` 已在开头，但有 **5,861,907 字节**；首个视频包在第 **5,861,955 字节**，包本身仅 18,779 字节。原生播放器在受控网络下读取约 **8.59 MB** 才显示首帧。公网媒体传输慢、无缓存复用，使这个启动成本变成几十秒；阅读器启动还串行等待工作区、字幕与阅读状态。

使用同一音视频流制作分片副本，通过浏览器 Media Source Extensions 按需装入初始化段和首个媒体段，在相同限速条件下将首帧从 **67.490 秒降至 6.452 秒**。首帧前接收量约 **8.59 MB → 0.55 MB**。这是隔离实验的实际结果，表明应首先减少播放启动数据量。

## 实测

| 场景 | 结果 | 说明 |
| --- | --- | --- |
| 线上视频，仅一个原生 `<video>` | 首帧 48.293 秒；播放时间超过 1 秒时为 61.187 秒 | 同一台 Windows 的 Edge；省去阅读器组件；冷浏览器缓存 |
| 从线上资料卡打开完整阅读器 | 11.726 秒出现播放器；随后 90 秒仍无首帧 | 首次为测试账号创建阅读现场，创建请求约 4.8 秒；不等同于所有恢复阅读场景 |
| 源站本地请求 1 MiB Range，三次 | 7.39–11.28 毫秒 | 正确 206 和 Content-Range；首尾片段；常驻 Book 已热 |
| 源站经公网域名请求同样大小片段，三次 | 6.93–7.82 秒 | 返回 Cloudflare `DYNAMIC`、`Cache-Control: no-store` |
| Windows 直连源站，请求 1 MiB，独立样本 | 响应头 0.287 秒，总计 5.262 秒 | 保留域名/SNI、TLS 校验和登录授权；直连仍存在传输成本 |
| Windows 公网 1 MiB 独立复测 | 读取响应体超时 | socket 读取等待上限 45 秒；不能把它当作成功吞吐样本 |
| 受控网络：原片 | 首帧 67.490 秒，播放超过 1 秒为 69.125 秒 | Edge，128 KiB/s，1.5 秒网络延迟 |
| 同样受控网络：初始化段 + 首段，经 MSE 播放 | 首帧 6.452 秒，播放超过 1 秒为 7.535 秒 | 同一机器、浏览器和网络参数；未转码 |

`playing_ms` 的判据是 `currentTime > 1`，不是只收到 `playing` 事件。公网裸播放器出现过 playing 事件后时钟仍停留在零的情况。

完整阅读器的请求顺序：创建阅读现场完成约 4.824 秒 → `video_context` 完成约 9.398 秒（解压后 459,212 字节）→ manifest、asset manifest、source fingerprint、草稿和正文窗口 → 11.711 秒才发出媒体请求。这个串行路径是可单独消除的额外等待。

## 代码依据

- `packages/web/src/App.vue:2377`：先等待 `api.videoContext()`；随后等待其他阅读资料、草稿及正文窗口，最后进入 reader surface。
- `packages/web/src/components/VideoReaderPane.vue:231`：`<video preload="metadata">` 的 `src` 指向完整 MP4。
- `crates/server/src/multi_user_host.rs:791`：媒体接口经过资料授权后，从 source manifest 取 `imported_path`，实际为 `media/original.mp4`。
- `crates/server/src/video_media.rs:179`：已有正确的 Range、seek 和有界文件流；响应为 `no-store`。此次没有发现先把整片读入内存的行为。
- `crates/server/src/published_library.rs:832`：已有常驻 Book 复用。热路径源站测量不足以支持“每次完整重载资料造成几十秒等待”的结论。
- `crates/server/src/multi_user_host.rs:316`、`:390`：四个通用 HTTP 工作线程同步发送媒体流。多个慢速视频下载能够占用这些线程，挤占普通 API；本次未对现网进行四路满载试验，该并发瓶颈与单视频首帧测量分开记录。

## 已验证的播放副本

原片为 H.264/AAC，1920×1080，约 77 分 21 秒，平均总码率约 389 kbps。使用以下参数生成隔离副本，耗时 **2.543 秒**：

```text
ffmpeg -nostdin -hide_banner -loglevel warning -n -copyts -i original.mp4 \
  -map 0:v:0 -map 0:a:0 -c copy -avoid_negative_ts disabled \
  -movflags +frag_keyframe+delay_moov+default_base_moof+global_sidx \
  playback-fragmented.mp4
```

副本 223,103,803 字节，929 个片段。`moov` 为 1,608 字节；包含全局索引时首个 `mdat` 在 26,840 字节处。MSE 实验实际仅请求 1,636 字节初始化段及首段 `bytes=24012-569652`。

全片 **139,240 个视频包**和 **199,887 个音频包**的 PTS、DTS、大小和关键帧标记逐项相同。视频包时长也相同；音频包的 ffprobe `duration` 字段在分片格式中部分返回 N/A，因此音频验证使用实际 PTS/DTS，没有把 duration 字段声称为完全一致。

分片把各时间段的媒体和相应元数据放在一起；浏览器通过初始化段和媒体段解码。格式依据为 [FFmpeg 分片文档](https://www.ffmpeg.org/ffmpeg-formats.html#Fragmentation) 与 [W3C ISO BMFF 字节流规范](https://www.w3.org/TR/mse-byte-stream-format-isobmff/)。原片已是前置索引，再做一次 `faststart` 不会消除这块全片索引。

## 建议的修复顺序

1. **加入用于在线播放的分片副本。** 保留原件作为导入、抽帧和来源引用依据；在导入/发布时生成保持时间轴的 fMP4 分片，播放器通过 MSE 或成熟的 HLS 播放实现按当前时间取段。此次样本已有约五秒的关键帧间隔，可先直接复制音视频流，无需重新压制画面或重跑转写。已有发布补建一次播放副本。
2. **提前启动播放器。** 获得已授权的资料引用和轻量播放信息后挂载播放器；字幕、草稿、正文窗口等继续异步载入。显示已有封面与明确的加载状态，字幕和提问能力在各自资料就绪后启用。
3. **改善媒体发送路径。** 先完成现有资料鉴权，再让 nginx 的内部媒体位置承担文件传输，释放通用 API 线程。另行验证适合实际访问网络的媒体分发路径，当前 Cloudflare 公网链路的迟延和抖动应纳入验收。若增加缓存，须保持授权检查，不把整个 `/api` 改为公共缓存。

正式验收应覆盖当前这份 77 分钟视频的首帧、连续播放、中段/末段拖动、关闭重开续播、字幕定位和截图时间点；同时在真实 Windows WebView2 与网页浏览器执行。增加并行观看场景，确认普通 API 不再被长视频发送占满。验收使用实际公网路径，不能用本报告的受控网络数字代替线上效果。

## 原始记录与复验入口

- [源站 Range 数据](origin.json)、[MP4 结构和首包](mp4.json)。
- [线上浏览器请求与媒体事件](browser.json)、[独立直连样本](network-direct-isolated.json)、[独立公网超时](network-isolated.log)。
- [重新封装结果](remux.json)、[音频时间戳比对](audio-timing.json)。
- [原片受控实验](controlled-browser.json)、[MSE 分片受控实验](controlled-mse.json)。
- 诊断脚本和本机媒体前缀：`tmp/video-startup-20261021/`。`node tmp/video-startup-20261021/controlled-browser.mjs --mse` 复验分片首段。脚本使用已下载的真实前缀、保留完整文件长度，并记录所有字节范围。
- 线上隔离副本：`/opt/understand-book/acceptance/video-startup-20261021/playback-fragmented.mp4`；原视频未覆盖。浏览器使用测试账号进行普通阅读，没有调用模型或付费搜索。

## 已知限制

- 受控实验验证的是首帧与第一秒播放，采用完整文件的真实前缀。尚未验证完整片段调度、整片拖动、真实 WebView2 或生产部署后的效果。6.452 秒不是线上承诺。
- 同次记录的 `bare-repeat` 与第一组 Windows 公网/直连吞吐测试重叠，存在带宽竞争；这些样本保留在暂存目录，但不作为干净重复样本或速度提升比例使用。独立复测在这些下载结束后执行。
- 直接把分片 MP4 作为普通 `video.src` 的前缀实验请求了未提供的文件尾部，之后超时；该受限样本不能判定完整分片文件不兼容。已通过的实验是按需向 MSE 提交初始化段和首个媒体段，不能直接宣称替换文件就已解决所有播放行为。
- 源站快速数据主要为常驻资料热路径；没有测量服务重启后的第一次 Book 加载，也没有做生产压力测试。
