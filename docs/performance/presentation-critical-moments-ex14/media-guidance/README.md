# EX14.2 / EX14.3 Manim 指导增补

2026-10-08，**媒体分支工程完成**。B 为 `ex14.v3`（检查），C 为 `ex14.v4`（检查＋视觉方法），当前工作树使用 C。固定 runner 兼容样例、实际请求加载、压缩恢复及输入计量通过。提供方调用 0，未提交、未部署。

## 指导与对照版本

- [B-final/presentation](B-final/presentation/)：沿历史 B 的通用检查指导，向 Manim 参考增加有限初筛、其他关键位置的页面补查、图像/标签/读数核对。原四动作与每批现场重建规则保持。
- [C/presentation](C/presentation/)：在媒体 B 上恢复历史 C 的 local 方法，并增加经固定 runner 验证的对象/公式成分变换、参数与标签联动、中文及公式布局方法。
- 原 [A](../baseline/source/skills/presentation/)、[B](../checks/B/presentation/)、[C](../visual/C/presentation/) 全文及旧请求保持。新 B/C 相对各自历史版本仅改变 `references/manim.md`、共同版本标识及 README。
- [B manifest](B-final/manifest.json)、[C manifest](C/manifest.json)保存全部 14 份指导和相关源码；[总核对结果](verification.json)逐字节核对共同实现、Runtime revision、全文与实际请求。生产 renderer、preview、author 与 EX14 基线一致，原 Manim 合同完整保留为前缀。

完整 B/C 比较中，C−B 包含既有 local 方法和新增媒体方法；单看媒体增补时，分别与各自历史 B/C 比较。`B/` 是测试修订前的首次冻结，正式工程版本为 **B-final**。正式自然对照需在执行前另冻可运行的共同底座。

## 固定 runner 兼容性

[scene.py](scene.py)定义三个小场景，[compat.py](compat.py)逐个使用未修改的生产 `presentation_animation_runner.py`。最终输入、scene、runner、完整渲染日志、MP4、四帧 metadata、逐帧布局记录和解码图均在 [compat-4](compat-4/)。环境为 Windows、Python 3.11.4、Manim 0.21.0/Cairo、PyAV 18.1.0、Microsoft YaHei 与当前配置的 TeX。

| 场景 | 实际验证 | 结果 |
|---|---|---|
| 同一对象变换 | `Transform` 将方块连续变为圆；A 标签保持并随对象移动；解码位置对照独立线性轨迹 | 90 帧通过，7.31 秒 |
| 公式成分对应 | `TransformMatchingTex` 将 `a+b=c` 变为 `a+b+d=c+d`；显式 MathTex 成分、中文条件说明、无旧公式残留 | 90 帧解码，前/中/后图像观察通过，14.38 秒 |
| 参数、读数与中文公式 | 同一 `ValueTracker` 更新长度、端点和 `DecimalNumber`；实际显示组拥有 updater；首帧初始化；`Text` 中文与 `MathTex` 分别渲染后排列 | 90 帧位置核对、逐更新读数及包围框检查通过，33.44 秒 |

三个场景均为 3 秒、640×360、30fps、无音频，返回四帧；合计独立解码 270 帧。样例验证真实运行兼容及有限表示，不把动画平滑程度当作数学正确性的判据。

[layout.mjs](layout.mjs)在 Edge 154.0.4258.62 中显示真实解码帧：320×420、640×240、960×720，共 [9 组检查](layout-2/verification.json)，无水平溢出或缩放比例失真。实际查看了 [320px 公式](layout-2/320-formula.png)、[320px 对象](layout-2/320-object.png)及参数画面；中文、公式和单位可分辨，没有裁切。最终参数画面见 [320px 参数联动](layout-2/320-parameter.png)，具体图像核对记录在 [visual-review.json](visual-review.json)。展示图是定向布局证据，不产生 Author 回执。

## 实际加载与回归

| 执行 | 结果 | 证据 |
|---|---|---|
| B 实际选择、普通文字、Manim review/重复/压缩 | 3 项通过，14 次请求计划、28 份 Native/ReAct 载荷 | [B-run-3](B-run-3/)；本目录 `B-ex14_*.log` |
| C 演示定向回归 | **89 passed，0 failed，0 ignored**；新增测试实际挂入原测试树 | [C-runtime-1.log](C-runtime-1.log) |
| C 实际请求录制 | 28 次请求计划、56 份 Native/ReAct 载荷；另存压缩前后消息 | [C-run-1](C-run-1/) |
| 全文、共同实现、schema、载荷及 token 核对 | passed | [archive-1.log](archive-1.log)、[verification.json](verification.json) |

Resident 使用脚本化 Adapter，执行真实请求装配与投影。普通文字未暴露 author 时无演示指导；global、静态 local 和未选 Manim 的 review 不含媒体增补。local/review 选择 Manim 后，完整新参考进入实际 Native/ReAct 消息；重复 prepare 不重复追加，压缩后完整参考保留一次。返回 global/静态 review 时当前块不含媒体内容，已经采样的历史指导仍在原锚点。

原四动作、图像绑定、候选交付资格、阶段前缀等回归包含在 89 项中。实际媒体 seek、解码滞后、末帧、小数恢复与离屏暂停证据沿用 [EX14.1-M](../media/README.md)；本次没有改动其实现或删除指导合同，没有重复运行既有完整媒体矩阵。

## 输入增量

沿用已冻结的官方 DeepSeek-V4-Flash-0731 tokenizer，`tokenizers 0.23.2`，`add_special_tokens=False`；版本与来源在 [既有 source.json](../visual/tokenizer/source.json)及本次 verification 中。下表计量实际追加的当前指导事件，包含包装与依赖。

| 当前选择 | 历史 B | 媒体 B | 历史 C | 媒体 C | 本次媒体增量 B / C |
|---|---:|---:|---:|---:|---:|
| global | 349 | 349 | 349 | 349 | 0 / 0 |
| static local | 407 | 407 | 635 | 635 | 0 / 0 |
| local + continuous_scene | 1355 | 1355 | 1583 | 1583 | 0 / 0 |
| review + continuous_scene | 1240 | 1240 | 1240 | 1240 | 0 / 0 |
| local + Manim | 2267 | 2492 | 2495 | 3117 | **225 / 622** |
| 返回 global | 349 | 349 | 349 | 349 | 0 / 0 |

Manim 正文由 892 → 1117 → 1514 token：检查增加 **225**，媒体方法再增加 **397**。完整媒体 C−B 的 Manim local 事件增加 625，其中 228 来自既有 local 方法，397 来自新媒体方法。累计历史输入另在 verification 的各请求记录中计量。以上是离线文本 token，不是供应商 usage、图片 token、缓存费用或实际支出。

## 原始失败与修正

- `compat-1`：将 updater 挂在无画面辅助对象，日志值正确但 Cairo 编码图形仍停在旧位置；独立像素轨迹核对失败。修为实际显示 `VGroup` 拥有 updater，指导同步说明。
- `compat-2`：机器轨迹检查通过，查看图像发现初始静止帧的标签未布局，以及公式换边路径中间重叠。补上首帧初始化，并新增首帧读数区域检查。
- `compat-3`：弧线换边仍可能在中途形成类似分式的形状；最终 `compat-4` 改用“两侧加同一 d”展示有效成分对应。旧视频与脚本保留，未将其机器通过误写为完整画面验收。
- `B-media-1.log`：既有压缩夹具的余量适用于较短参考，Manim 使压缩提前，未落在预设观察点；仅调整该测试的余量与压力文本。
- `B-media-2.log`：测试误把依赖 `continuous_scene` 当作显式 needs；修为核对显式 `manim` 和实际完整依赖正文。产品选择器没有修改。最终三项 B 与全套 C 通过。

## 重跑

运行前目标：兼容测试发现 API/首帧/编码几何/读数不符时修场景及指导；请求测试发现条件泄漏、重复注入或压缩丢失时修装配；归档发现版本错配时修冻结入口。输出使用新目录，保留每次原始失败。

```powershell
& 'tmp/ex9a-manim-venv/Scripts/python.exe' docs/performance/presentation-critical-moments-ex14/media-guidance/compat.py '<new compatibility directory>'
node docs/performance/presentation-critical-moments-ex14/media-guidance/layout.mjs '<compatibility directory>' '<new layout directory>'
$env:EX14_GUIDANCE_OUTPUT = '<new request directory>'
cargo test -p runtime --lib presentation -- --nocapture --test-threads=1
python docs/performance/presentation-critical-moments-ex14/media-guidance/verify.py
```

`freeze.py <new-arm>`保存当时指导及源码，拒绝覆盖已有目录；`verify.py`核对本轮 B-final/C、B-run-3/C-run-1、compat-4/layout-2。新批次需明确调整其核对路径。旧 A/B/C 的核对脚本含“等于当时工作树”断言，应配其冻结版本使用。

## 已知限制与接续

本轮通过 EX14.2/3 的 Manim 工程增补，未运行付费自然对照；EX14.4 的总额/单任务预算、模型与停止配置及成本接受界限仍待批准。未取得表示收益、采用或学习效果结论。中文/公式兼容结论限于本次 Windows 字体和 TeX 环境。

未新增 Linux 隔离 worker、实体手机或完整 Reader 连续使用验收。EX12.5 历史未完项继续保留；EX14.5 按实际采用路径验收，EX14.C 仍需既定条件准入。接续入口：[checkpoint_ex.md](../../../../checkpoint_ex.md)。
