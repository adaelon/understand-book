# EX14.0：现状与实验范围

2026-10-08，**EX14.0 离线交付完成**。决策依据 [ADR-0157](../../adr/0157-obstacle-driven-visual-methods-and-critical-moment-review.md)，实施合同见 [EX14 方案 §4](../../切片方案-EX14-视觉方法指导与关键解释时刻检查.md)。

本目录冻结 EX14 涉及的源码表面、现有 A 组指导、真实请求装配和实验材料。真实模型调用为 0；实验状态为 **`blocked: paid experiment not authorized`**。后续状态（2026-10-08）：[EX14.1-B 浏览器验证](browser/README.md) 的 A06–A09/A11 通过；[EX14.2 通用检查指导](checks/README.md) 的 A13–A18 通过，B 组 ex14.v1 全文及实际载荷已冻结。[EX14.3 视觉方法指导](visual/README.md) 通用路径工程完成，C 组 ex14.v2 全文、实际载荷、表示样例及离线 token 计量已冻结；自然 N3、收益与采用仍待 EX14.4。[EX14.1-M 媒体验证](media/README.md) 的 A04–A10/A12 已通过，[EX14.2/3 Manim 增补](media-guidance/README.md)已完成：媒体 B/C ex14.v3/ex14.v4、兼容场景、真实请求与输入计量已冻结；下一入口为 EX14.4 前置配置。本文下述基线事实仍指 EX14.0。

## 基线与恢复

- 基准 commit：`4e0b68ee7d730709c2917319a2cfe9024e33cee6`，`feat: integrate book structure, tutoring and multi-user reader`。
- [manifest.json](baseline/manifest.json) 列出冻结时间、63 个相关文件及其跟踪状态；[git-status.txt](baseline/git-status.txt) 保存当时范围内的状态。
- [HEAD-to-working-tree 差异](baseline/head-to-working.patch) 保存 57 个已跟踪文件范围中的实际差异；[source/](baseline/source/) 保存全部 63 个文件的实际字节，包括 6 个未跟踪文件。
- A 组完整正文在 [source/skills/presentation/](baseline/source/skills/presentation/)，含共同方法、工程合同、阶段、目录、全部参考和例子。实际 revision 为 **ex13.v2**。原说明页的 ex13.v1 标识滞后，原样归档，后续指导变更时同步修正。
- EX14.0 时 B/C 尚未实现；拟变更范围在 [experiment.json](experiment.json)。后续 EX14.2 已保存 [B 组 ex14.v1](checks/B/manifest.json)，EX14.3 已保存 [C 组 ex14.v2](visual/C/manifest.json) 全文与实际载荷；正式实验仍需冻结当时可运行的共同底座。

恢复时在独立 checkout 使用该 commit，对照 manifest 仅恢复列出的路径：已跟踪文件应用保存的 patch，未跟踪文件从 source 原样复制；需要保留确切行尾时以 source 中实际字节为准。`verify.py` 在临时目录从 commit 逐文件读取、应用 patch 并逐文件核对文本，随后核对归档文件大小。源快照覆盖演示合同与 Runtime 请求装配，不是全部在途 ADM/视频/UI 工作的备份。后续正式实验必须再冻结当时可运行的共同底座，不能仅凭 HEAD 声称与本批相同。

`freeze.py` 是本次一次性采集入口，已存在的 source 目录会拒绝覆盖；后续批次使用新目录。基线包含原有 `presentation_authoring_tests.rs` 与 `presentation_ex13_live_tests.rs` 修改；本轮保留这些内容。

## 实际指导载荷

通过已有 `ex13_guidance_changes_append_after_results_and_keep_provider_prefix`，在脚本化 Adapter 的真实 Resident 循环中录制 11 次采样，未连接提供方。每次保存 `request-NN.json`、`native-NN.json`、`react-NN.json`；[payload-index.json](baseline/payload-index.json) 记录模块、字节量及空的 usage。字节量不作 token 数使用。

| 入口 | 原始载荷 | 实际指导 |
|---|---|---|
| 普通问题、author 尚未暴露 | [request-00](baseline/requests/request-00.json) | 无 presentation 模块；另有普通回答负向测试 |
| author 首次暴露/global | [request-01](baseline/requests/request-01.json) | common + engineering + capabilities + global；无技术参考 |
| local/state | [request-02](baseline/requests/request-02.json) | local + state；共同指令前缀不变 |
| local/editing | [request-05](baseline/requests/request-05.json) | local + editing；旧指导保留原锚点，当前选择更新 |
| review | [request-08](baseline/requests/request-08.json) | review，无所选参考；通过后续采样追加 |
| deliver 后及下一工作 | request-09 / request-10 | inactive → global；不从历史恢复旧设计 |

11 项模块单测另覆盖 static local、editing/static_plot、continuous_scene → state、manim → state/continuous_scene、konva 不隐含时间线、稳定顺序及 Native/ReAct 一致。它们证明选择器合同；本轮 Resident 录制没有声称覆盖全部 needs 组合的实际任务。

[tool-schema.json](baseline/tool-schema.json) 从 request-01 的实际工具列表提取；11 次采样中暴露后的 schema 相同。工具仍为 prepare/render_plot/render_animation/read/search/patch/write/preview/deliver。本轮未改产品指导、revision 或工具 schema。

## 环境、资源与分支状态

机器诊断见 [environment.json](baseline/environment.json)。本机 Windows AMD64；Rust/Cargo 1.96.0，Node 24.9.0，默认 Python 3.14.4。BrowserPreview 自动发现 Edge 154.0.4258.62；未显式设置 `UNDERSTAND_BOOK_PREVIEW_BROWSER`。

Manim 候选解释器为 `tmp/ex9a-manim-venv/Scripts/python.exe`：Python 3.11.4、Manim 0.21.0、PyAV 18.1.0、Pillow 12.3.0，libx264 可发现；latex/dvisvgm 位于 `E:/texlive/2025/bin/windows/`。当前进程未设置 `UNDERSTAND_BOOK_ANIMATION_PYTHON`，按现有宿主逻辑 render_animation 会 unavailable。EX14.1-M 需显式配置该解释器后验证；导入成功不代表渲染、中文/公式、解码或恢复通过。

| 分支 | 本轮状态 | 后续入口 |
|---|---|---|
| 指导选择与请求装配 | passed，1 项真实循环录制 + 11 项模块测试 | EX14.2/3 变更后定向回归 |
| 浏览器基础前提 | N3 三环境真实 preview + 原生 deliver passed，6 张截图，无脚本错误/水平溢出 | EX14.1-B 关键位置、受控故障与可达性尚未运行 |
| Manim | 依赖导入 passed；当前宿主配置 blocked；F1/F2-M not_run | EX14.1-M，独立于浏览器路径 |
| 真实模型 | blocked: paid experiment not authorized | 批准、冻结执行配置后才进入 EX14.4 |
| Linux/实体手机/真实读者 | 本轮 not_run | 历史发布与 EX12.5 未完项见下节 |

有效约束来自冻结源码，而非新设限制：

- preview 每次新建页面，最多 4 个动作，超时 30 秒。`scroll`、中心 `click`、限定 `key`、语义 `seek`；末动作后才读取 `read_selector`。每批都须独立重建前提。
- 正式三环境：320×420 touch、640×240 touch、960×720 mouse。允许自定义宽 240–1920、高 160–2160；legacy width 可满足历史宿主合同，但不计为本批三环境证据。
- Manim 固定 Cairo、30fps、无音频；默认 1280×720，可选偶数尺寸 320–1600 × 240–1200；代码 ≤32 KiB，data ≤128 KiB，cues ≤16；超时及实际时长 ≤180 秒。
- runner 取首、中、尾、输入顺序首个 cue，目标去重；Rust 接受 1–4 帧，单 PNG ≤4 MiB，MP4 ≤8 MiB，每版本动画和海报解码合计 ≤24 MiB，文本文件合计 ≤1 MiB。
- Linux 隔离 worker 的资源约束另有输入 100 MiB、输出 40 MiB、内存 768M、CPUQuota 100%，隔离通道临时媒体数量/字节限制也保留；本机直接 BrowserPreview 不冒充该通道验证。

## 冻结材料与实验边界

正式请求、材料路径、18 次顺序、失败规则及待批准的成本约束均在 [experiment.json](experiment.json)。每个用例每组两次独立重复；各组新会话、同一材料、模型、工具、渲染环境及底座，只改变指导。拟使用原 native/deepseek-v4-flash 配置，确切 profile、响应模型和输出限制在获批执行批次冻结。EX13 的历史 9 元额度不沿用。

- N1：[简化二次函数材料](materials/N1.txt)，沿用 ADR-0139 的模型与三个学习率。核对值留在评审目录，不放进制作请求。
- N2：[完整 Rust 代码](materials/N2.rs)和 [Option::take 原始说明](materials/N2-option-take.md)。官方章节冻结为本机 Rust 1.96.0 [原始 HTML](materials/N2-option-take.html)，与运行样例的编译器一致，不依赖随后变化的在线 stable。
- N3：用户确认建立固定静态基准页；[输入来源](materials/N3-source.txt)、[页面源码](../../../crates/server/tests/fixtures/presentation-ex14-n3.html)、[确切交付版本](materials/N3/version.json)。同一 revision 1 为各组输入，原生准备回执在 [preparation.json](materials/N3/preparation.json)。这是隔离测试会话中的确定性材料准备，提供方调用为 0，模型观察为 not_run；不是历史用户交付或自然任务成功样本。

N3 的确切对象为 `presentation-1791457237623633400-0` revision **1**。保留整数整除与平面欧氏几何两个例子；浏览器截图可直接查看 [桌面](materials/N3/desktop-content-0.png)、[窄屏后段](materials/N3/narrow-content-1.png)。

[独立评审依据](review-only/rubric.md) 不进入制作输入。记录 B−A 的具体漏检/修复变化和 C−B 的具体表示收益，逐对报告改善、相当与退步。实际未加载受检指导的运行保留，但记“未检验该项增量”；N3 对 B/C 的负向控制分别按实际加载判断。人工/定向浏览器补证与 Agent 自行观察分列。

费用总额和单任务金额均为 null、approved=false；单次时间、调用数及相对费用/耗时界限仅为待批准提案。当前无可执行付费预算，也不作采用裁决。自动重试为 0；供应商失败或缺 usage 停批，任务边界停止保留原始结果，修订重跑另批。媒体分支未通过时三组 Manim 参考均保留 A 原文；媒体指导后续增补使用新批次。

## 验证入口与原始记录

运行目的：恢复核对能发现漏存文件或 patch 与实际源码不符，失败即补齐归档；载荷核对能发现指导/工具基线串位，失败即重新定位录制；N1/N2 核对能发现材料预期错误，失败即在处理组运行前更正；N3 原生测试能发现静态页预览或交付失败，失败保留原目录并在新目录修复重跑。

```powershell
# 现有离线实际请求录制；重跑需使用新记录目录。
$env:EX13_REQUEST_RECORDING_DIR = '<new absolute directory>'
cargo test -p runtime --lib ex13_guidance_changes_append_after_results_and_keep_provider_prefix -- --nocapture
cargo test -p runtime --lib agent_prompt::presentation::tests -- --nocapture

# 本轮新增的离线基准材料生成器，先确认测试确实注册。
cargo test -p server --lib ex14_baseline_deliver_static_n3 -- --list
$env:EX14_N3_OUTPUT = '<new absolute directory>'
cargo test -p server --lib ex14_baseline_deliver_static_n3 -- --ignored --nocapture --test-threads=1

# 从已冻结证据核对恢复、载荷、材料和实验状态。
python docs/performance/presentation-critical-moments-ex14/verify.py
```

原始日志集中在 baseline：Runtime 录制首条命令误用精确短名，命中 0 测试，未计为通过；去掉错误过滤后 1 项通过并产生 33 份请求。Server 首次编译被同时进行的 ADM 工作暂缺 `adm9_tests.rs` 阻断，保留失败日志，没有改动 ADM 文件。

随后 Server 编译及注册成功；为避开另一项工作的共享构建锁，停止了本轮排队的 cargo 命令，直接运行刚注册的 `target/debug/deps/server-b7394b1f5c08710a.exe tests::presentation_author_tests::ex14_baseline::ex14_baseline_deliver_static_n3 --exact --ignored --nocapture --test-threads=1`，1 项通过（15.65 秒），见 [n3-delivery-direct.log](baseline/n3-delivery-direct.log)。第一次归档核对因 Python 通用换行读取与 Rust include_str 保留 CRLF 不同而误判阶段文本；更正为按原字节解码后，6 组核对全部通过，见 [verification.json](verification.json)，原始失败日志保留。产品内容未为该核对修改。

## A01–A03 离线验收

| 项 | 本片结论 | 证据与保留前提 |
|---|---|---|
| A01 | A 组及相关底座可恢复，通过 | manifest、原文、patch 与 57 文件重建通过；B/C 尚未形成，在 EX14.2/3 保存确切全文后才检查三个版本恢复 |
| A02 | 离线材料与对照边界已冻结 | N1/N2/N3、18 次交错次序、三组差异、失败/质量规则已固定；模型执行配置、费用与成本采用界限在处理组运行前批准并冻结 |
| A03 | 分支前提独立记录，通过 | 浏览器基准 passed；Manim 宿主配置 blocked、关键场景 not_run；真实实验 blocked，提供方调用 0 |

本轮源文件变更只增加基准材料生成测试、两行测试挂载和静态 HTML；snapshot 中其余源文件在收口前仍与工作树逐字节相同。后续正式试验的启动条件与 EX14.0 离线完成分开记录。

后续 EX14.1 按方案新增真实浏览器/媒体测试。已有环境入口为 `ex12_scroll_author_binds_images_reading_and_candidate_receipts`、`ex11_animation_render_version_read_reuse`、`ex11_animation_errors_and_process_tree_cancellation`、`ex11_media_formal_preview_delivery`；本轮未因它们存在而记为通过。付费 `ex12_live_comparison` / `ex13_live_continuation` 不属于上述命令。

## 历史状态与已知限制

EX13.0–EX13.6 与 EX12.4 原生内容验收完成。[EX13.6](../presentation-context-ex13/ex13-6/README.md) 的“未部署”被较后的 [Linux 发布记录](../../Linux上线-EX12-EX13-JL.md) 更新：release `ex13-jl-20261002` 已上线，选定演示 `presentation-1790920818511542500-6` revision 2、27 个来源、十站范围保留。Linux 长页、保存重开、回顾、隔离、备份恢复与一次真实现场追问已有历史证据。

EX12.5 线上同对象再次局部修订、长期连续使用、实体手机及学习效果仍未完成。N3 基准页准备不改变该状态。本轮没有运行 EX14.1 的关键位置工程矩阵、B/C 自然对照或学习测量。实际模型 tokenizer 新增输入量待确定模型及 B/C 文本后计量。B/C 指导正文、付费边界批准与正式共同底座冻结是后续运行前提。
