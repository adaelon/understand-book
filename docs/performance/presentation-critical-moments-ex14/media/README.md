# EX14.1-M：真实媒体关键现场

2026-10-08，**EX14.1-M 完成，A04–A10、A12 通过**。使用既有固定 Manim runner、Author 和 BrowserPreview；提供方调用 0。指导保持 ex14.v2，Manim 参考增补属于后续独立批次。

最终证据为 [run-6 原生验收](run-6/verification.json)、[browser-8 独立浏览器](browser-8/verification.json)、[逐帧解码](run-6/decoded/verification.json)；[汇总及绑定核对](verification.json)确认三者对应候选 `candidate-1791463203478744400-1`。完整候选、真实 MP4、scene、data、cues、逐动作请求、回执与 PNG 均保存在 run-6。

## 验收结果

| 验收 | 实际结果 | 证据 |
|---|---|---|
| A04 四帧遗漏 | F1 时长 8 秒，640×360、30fps、无音频；返回 0、1.5、4、7.966667 秒四帧。240 个解码帧中，5.8–6.166667 秒的 12 帧含人为错误，四帧初筛全部遗漏 | [渲染回执](run-6/render-response.json)、[实际故障帧](run-6/decoded/6.000.png) |
| A05 后续 cue 补查 | 第二 cue 为 6 秒；三环境页面补查均显示移动方块和红色 WRONG A 标签，前后位置不含红色标签 | [短屏故障截图](run-6/short-content-fault-4.png)、三环境 `*-fault-response.json` |
| A06 前后读数 | 5.5、6、6.5 秒分别作为独立批次末动作；各自 reading 绑定逻辑位置、解码时间、活动 B、候选与环境 | 三环境 `*-before/fault/after-request/response.json` |
| A07 重建前提 | 每批滚动 → 选择 B → 4.25 → 目标，共四动作；每个新页面从 A / 0 开始 | 9 组独立批次及逐动作图 |
| A08 活动演示 | A/B 使用同一固定过程、分别拥有位置；选择 B 后截图与 B 状态一致；省略选择时实际仍为 A | [遗漏选择回执](run-6/omitted-selection-response.json) |
| A09 小数与回退 | 三环境执行 6 → 2.25 → 2.25；实际方块和轨迹复原，重复定位像素一致，未来轨迹消失 | 三环境 `*-rollback-response.json` |
| A10 解码与失败 | 暂缓真实媒体响应时 seek 保持 pending/逻辑 0，释放后显示真实 6 秒帧；永久停滞约 2.5 秒报超时。损坏媒体在原生 seek 阶段返回工具错误，无预览资格 | [解码错误](run-6/decode-failure-error.json)、browser-8 的媒体事件与实际 PNG |
| A10 末帧与状态谎报 | 逻辑终点保持 8，媒体帧为 239/30 秒，重复终点可完成；独立解码图能区分最后一帧和前一帧。冻结媒体变体报逻辑 6、画面仍为 0，被像素判据捕获 | 三环境 `*-endpoint-response.json`、[冻结媒体图](run-6/frozen-media-2.png)、browser-8 endpoint 两例 |
| A12 短面板与播放 | 320×160、640×160 控件与画面同屏，播放时解码帧和方块实际推进；暂停、重播、离屏暂停与返回后保持暂停通过 | [320px 短面板](browser-8/short-320.png)、[640px 短面板](browser-8/short-640.png)、browser-8 读数与逐帧 PNG |

原生测试显式使用 320×420 touch、640×240 touch、960×720 mouse。18 次 preview 尝试中 17 次完成、1 次拒绝，保存 78 张页面 PNG，另有 4 张资产初筛图。正常候选取得三种正式环境回执；夹具保留人为内容故障，未交付。

独立浏览器读取该完整候选，解析其媒体资源路径，加载项目现有 `presentation-media.js` 生命周期。通过真实 HTTP Range 响应暂缓媒体交付，观察解码等待；短面板使用真实点击与共享离屏暂停逻辑。额外验证播放后暂停到 6.125、等待回调、调用已注册恢复函数后仍保持逻辑小数及相同解码图像。

截图的方块位置与固定解析轨迹 `pixel_x = 95 + 56.25 × media_seconds` 比较；原生截图按实际 video 矩形、缩放与留黑区域换算。对象像素限定在视频区域内，避免页面文字的彩色子像素干扰。红色卡片、方块与回退轨迹分别核对，该判据只用于确定性夹具。

## 实现与环境

- [F1 scene](../../../../crates/server/tests/fixtures/presentation-ex14-f1.py)：ValueTracker 驱动方块、轨迹及 5.8–6.2 秒人为错误标签。
- [F2-M 页面](../../../../crates/server/tests/fixtures/presentation-ex14-f2-m.html)：长页、两个独立演示位置、可访问控件、暂停定位、异步解码和恢复。
- [原生测试](../../../../crates/server/src/tests/presentation_ex14_media_tests.rs)：挂入原测试树的 `ex14_media` 子模块。
- [observe.mjs](observe.mjs)、[decode.py](decode.py)、[verify.py](verify.py)：定向浏览器、独立逐帧解码、最终证据与候选绑定核对。
- [source/manifest.json](source/manifest.json)：保存本轮测试、夹具及实际相关媒体/预览实现源码。

本机 Windows，Edge 154.0.4258.62；显式设置 `UNDERSTAND_BOOK_ANIMATION_PYTHON` 到仓库 `tmp/ex9a-manim-venv/Scripts/python.exe`。实际 Python、Manim、PyAV、Pillow 版本见逐帧解码记录。固定 runner 仍为 Manim 0.21.0 / Cairo / 30fps，四帧与四动作上限保持。

## 测试与重跑

| 运行 | 结果 | 记录 |
|---|---|---|
| 测试注册 | 1 项真实挂入 `presentation_author_tests::ex14_media` | [registration-1.log](registration-1.log) |
| 最终原生真实渲染/预览 | 1 passed，0 failed，0 ignored；85.26 秒 | [native-6.log](native-6.log) |
| 最终独立浏览器 | 6 组 passed：延迟、超时、两种短面板与各自末帧 | [browser-8/verification.json](browser-8/verification.json) |
| 最终独立解码 | 240 帧轨迹及 12 个故障帧核对通过，保存 5 个关键解码图 | [decode-6.log](decode-6.log) |
| 最终归档绑定 | 原生/浏览器同候选，场景与页面原文、18 份请求和图片绑定通过 | [verification.json](verification.json) |

运行目的：注册失败则修测试挂载；原生失败则从真实截图、状态和错误区分夹具与宿主；解码或播放失败则修相应定位/生命周期路径；归档绑定不一致则修正最终目录，避免将不同候选的成绩合并。

```powershell
cargo test -p server --lib ex14_manim_ -- --list
$env:UNDERSTAND_BOOK_ANIMATION_PYTHON = (Resolve-Path 'tmp/ex9a-manim-venv/Scripts/python.exe').Path
$env:EX14_MEDIA_OUTPUT = '<new absolute native directory>'
cargo test -p server --lib ex14_manim_ -- --ignored --nocapture --test-threads=1
node docs/performance/presentation-critical-moments-ex14/media/observe.mjs $env:EX14_MEDIA_OUTPUT '<new absolute browser directory>'
& $env:UNDERSTAND_BOOK_ANIMATION_PYTHON docs/performance/presentation-critical-moments-ex14/media/decode.py $env:EX14_MEDIA_OUTPUT
python docs/performance/presentation-critical-moments-ex14/media/verify.py
```

原生、浏览器与独立解码入口拒绝覆盖旧目录；归档核对器固定核对本轮 run-6/browser-8，新批次另定入口。编译出现既有 ts-rs serde 属性告警。

## 修订与原始失败

`native-1` 的颜色阈值漏掉浏览器色彩转换后的蓝色；修正为固定颜色内部范围。`native-2/3` 暴露夹具末帧边界与 EOF 读数问题：现在定位最后一帧内部，并把到达 duration 的媒体时钟映射到最后编码帧。`native-4` 的位置核对混入正文彩色子像素，现限定视频区域。`native-5` 误将被拒绝的异步操作按已完成预览状态断言；现核对真实 `PRESENTATION_AUTHORING_FAILED / Media decode failed` 及资格缺失。失败日志和当时候选均保留。

`browser-1/2/3` 的测试媒体响应未支持 Range，浏览器无法跳到目标；修为真实 206 字节范围响应后通过。`browser-4` 的重播等待误接受旧时间，现等待实际播放且媒体时间进入起始区间；后续成功记录保留。最终 browser-8 和 run-6 使用同一候选。未发现需要修改产品宿主或公共合同的缺陷。

## 后续媒体指导

2026-10-08，EX14.2/3 Manim 指导及固定 runner 兼容样例已在[独立媒体指导批次](../media-guidance/README.md)完成，新增媒体 B/C 为 ex14.v3/ex14.v4。本文 ex14.v2、run-6/browser-8 及以下本片限制保留原验收时点。

## 已知限制与接续

本片证明固定环境中的观察、解码与播放工程路径。原生工具由测试代码调用，独立浏览器另行标记；没有模型自行识别/修复故障的结果，没有自然请求收益、采用或学习效果结论。共享生命周期与页面恢复函数验证不代替完整 Reader 的保存、离开、重开验收。

Manim 参考仍保持 A/B/C 共同原文；EX14.2/3 媒体指导增补、中文/公式布局与 TransformMatchingTex 等兼容场景尚待独立实施，并新冻指导及对照批次。F1 使用 ValueTracker，不代表所有拟增 API 已验证。EX14.4 的付费预算和执行配置仍未授权；EX14.C 未触发。

本轮未验证 Linux 隔离 worker、实体手机或完整 Reader 连续使用；未提交、未部署。EX12.5 历史状态继续见 [Linux 发布记录](../../../Linux上线-EX12-EX13-JL.md)。接续入口为 [checkpoint_ex.md](../../../../checkpoint_ex.md)，主视频 checkpoint 保持。
