# 视频来源合同（VI1）

2026-10-10，已实现。范围与语义沿用[视频方案 §3–4](切片方案-视频理解与渐进伴读.md)、[ADR-0145](adr/0145-video-media-locations-and-revisable-visual-units.md)及既有视频术语。

## 权威与入口

`crates/base-schema/src/video.rs` 是 Rust 权威定义，包含 serde / schemars 合同和校验；`cargo test -p base-schema export_bindings` 生成 Core 的 22 个新 TS 类型。`PublishedBookRef` 从 Server 移到该权威层，Server 保留原路径再导出，字段和 URL 行为不变。

```text
原片导入记录 → VideoSourceAsset
正文 + base.json + TranscriptMap → 校验 LID / UTF-16 / 媒体身份和时间
VideoSourceManifest → 分能力覆盖和材料版本关系
实际读取位置 → MediaObservation 合同 → EvidenceTarget 合同
```

Core 的 `source-manifest.ts:buildVideoSourceManifest` 校验媒体清单；`video-contract.ts:validateTranscriptMap` 将映射与真实正文、LID 基座绑定。Rust 对应 `VideoSourceManifest::validate`、`TranscriptMap::validate`；读时入口为 `Book::validate_video_source` 和 `Book::validate_video_evidence_target`，错误码 `INVALID_VIDEO_SOURCE`。

媒体清单使用独立 `video_source_manifest.v1` 合同，预定文件名 `video_source_manifest.json`；正文仍为 `source.txt`。原有 Markdown/EPUB/PDF 清单及文本 `EvidenceRange` / `ResolvedSource` 沿用既有合同。

## 字段与校验口径

| 合同 | 口径 |
| --- | --- |
| `VideoSourceRef` / `TranscriptRef` | 显式 ID 与修订；转写绑定确定原片。路径、文本相同均不代替身份 |
| `VideoSourceAsset` | 保存导入后相对路径、外部来源路径、实际时长及原始宽高；后续读导入路径 |
| `VideoMaterial` / 清单前版 | 原片与转写共同固定材料；`validate_successor` / `validateVideoMaterialSuccessor` 校验同源补齐保持 book_id，来源修订使用新 book_id 并指明前版 |
| `MediaLocation` | 原片时间轴的毫秒浮点数，非空半开区间，`0 ≤ start < end ≤ duration` |
| `FrameLocation` / `FrameRead` | 实际 PTS 与请求时刻分开保存；实际帧须在片内，宽高取解码后原帧 |
| `FrameRegion` | `[x,y,width,height]` 相对原帧归一化；拒绝空区域和越界，裁剪宽高不替代原帧尺寸 |
| `TranscriptMap` | 每段独立 ID、正文及原片区间；`text_range` 是叶子 LID 内 UTF-16 半开范围，必须逐字匹配规范正文。多个识别段可映射到同一段落，正文和时间顺序一致 |
| `TranscriptWord` | 字符范围相对所属段；毫秒为原片时刻，身份继承所属段。允许机器返回起止重合，保持句段内有序；不以空估计生成可播放区间 |
| 转写来源与精度 | `machine_transcription` 保存引擎、模型、语言；`raw_result_path` 保留原始响应。`words=[]` 明确只有句段定位，不从字符比例推导逐词时刻 |
| `TranscriptTranslation` | 必须指向相同转写版本中存在的 segment_id |
| `VideoCoverage` | 每种能力分别保存 accepted / pending / failed，不重叠；`gaps` / `videoCoverageGaps` 返回已接纳区间的补集。静音可有处理覆盖而无文字 |
| `VideoQuestionContext` | 固定草稿、发布、材料、实际帧、可选区域与范围；区域必须来自现场中的同一帧 |
| `EvidenceTarget` | text / visual / audio / joint；纯视觉无需 LID。联合成员各写支持角色，至少两个非联合成员，固定同一发布；引用不依赖 VU ID |
| `MediaObservation` | 视觉带 Run 和成功模型请求的关联 ID；音频带 Run、工具调用和复核结果关联 ID；保持实际读取帧或音频位置 |

## 夹具与验证

共同夹具：`crates/base-schema/fixtures/video-contract.json`。

- lecture：VI0b `review-overflow.json` 三个真实识别段映射到一个 LID；VI0c 请求 555123 ms / 实际 555133.3125 ms、1920×1080 原帧和代码裁剪。
- supplement：VI0a 独立 6 秒教学夹具，1 秒和 5 秒重复画面保持两个位置；纯视觉来源无需正文。静音转写与覆盖为明确标注的合同构造数据。
- einsum：实际 `einsum.json:segments[0]`，首词 “you” 起止均为 1180000 ms。

夹具顶层 `provenance` 区分真实实验值和构造的书、发布、运行、处理回执、译文及覆盖状态。TS 验证后将完整 JSON 送入 Rust 示例校验、序列化，再由 TS 读回逐字段比较；Rust 单测也独立读取同一夹具。

```powershell
. .\.worktree-local\environment.ps1
cargo test -p base-schema
pnpm -C packages/core exec vitest run test/video-contract.test.ts test/source-manifest.test.ts
pnpm -C packages/core typecheck
cargo test -p read-tools --lib video_contract
cargo test -p read-tools --lib source_presentation
cargo test -p read-tools --lib paper_minimap_dual_reads_v1_and_v2_source_maps_but_rejects_unknown_versions
cargo check -p server --lib
```

本轮结果：VI1 Rust 8/8；绑定生成 40/40（包含 22 个新类型）；Core 36/36（VI1 29，原清单 7）；读时新入口 1/1、文本来源 13/13、PDF 映射 1/1；Core 类型检查和 Server 编译通过。

## 限制与后续接点

- VI1 提供合同和显式校验入口。媒体文件登记、可读性和播放由 VI2 接入；整讲转写就绪、映射写入和发布消费由 VI3 接入，Book 当前不会自动加载视频清单。
- 观察合同校验只判断结构、归属及范围；真实成功处理回执由 VI5 的 Runtime 创建，VI7 的本轮账本负责来源交付。夹具回执不登记成生产观察。
- 来源修订比较不承担发布先后和覆盖累积判定；同源发布接入由 VI9 完成，材料实际更新由 VI17 完成。
- 词时间为机器估计，完整逐字真值和 V07 原话仍待听核。读时真模型、DSH、Linux 与安装态沿原待办继续验收。
