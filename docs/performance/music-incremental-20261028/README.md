# 音乐完整重建与失败恢复的增量处理

2026-10-28，S4 已实现。本轮把完整音乐 rebuild 中已经完成的音频检查、谱面解析与渲染、谱面合成示范按阶段复用，保持原有校准重放、事件接纳与来源修订规则。模型调用数为 0。

两份现有音乐 QA 工作区各执行一次建立记录的完整处理，再重复三次。重复构建中位数：舒伯特从 **1,560 ms 降至 387 ms，减少 75.2%**；贝多芬从 **3,894 ms 降至 288 ms，减少 92.6%**。这些数值描述本次完整 rebuild 的复用路径。

## 输入与方法

| 样本 | 编选输入 | 谱面对象 | SVG 页数 | 合成示范 | 已有校准记录 |
| --- | --- | ---: | ---: | ---: | ---: |
| 舒伯特 | [schubert-curation.json](../../../evals/music/schubert-curation.json) | 1,066 | 3 | 1 | 2 |
| 贝多芬 | [beethoven-curation.json](../../../evals/music/beethoven-curation.json) | 1,249 | 17 | 0 | 2 |

运行环境为 Windows、本地 Node.js v24.9.0、Verovio 6.3.0-425dd7b。原实现源码在改动前保存于 `tmp/music-incremental-20261028/baseline-source/`，并由独立基线进程加载。基线和候选分别从同一 `.understand-book/music-calibration-qa/` 种子复制工作区；每个版本、每份素材执行一次首轮和三次重复。候选的首轮没有阶段完成记录，仍执行完整处理并保存记录。

使用 [benchmark.mjs](./benchmark.mjs) 调用真实 `buildCuratedMusicWorkspace`，`performance.now()` 计量完整函数耗时。音频检查实际执行 ffprobe 和完整 ffmpeg 解码，谱面实际调用 Verovio，示范实际进行本地合成。原始结果保存在 [baseline.json](./baseline.json) 和 [candidate.json](./candidate.json)，其中每次运行均保留构建报告及复用标记。

## 测量结果

| 样本 | 基线首轮 ms | 候选首轮 ms | 基线重复三轮 ms，按运行顺序 | 候选重复三轮 ms，按运行顺序 | 重复中位数，基线 → 候选 |
| --- | ---: | ---: | --- | --- | --- |
| 舒伯特 | 5,769 | 6,990 | 4,198 / 1,560 / 1,484 | 355 / 387 / 536 | 1,560 → 387 ms |
| 贝多芬 | 3,655 | 5,886 | 4,110 / 3,504 / 3,894 | 318 / 288 / 273 | 3,894 → 288 ms |

重复耗时区间分别为：舒伯特基线 1,484–4,198 ms、候选 355–536 ms；贝多芬基线 3,504–4,110 ms、候选 273–318 ms。重复中位数对应约 4.03 倍和 13.53 倍的速度提升。

**首轮结果单独保留：两份候选首轮在本次测量中都更慢。** 每份首轮只有一个样本，本次证据不能确认稳定的首轮回归，也不能证明冷启动改善。首轮完整处理及建立阶段记录的成本没有计入重复命中的收益。

### 最终 Windows 引擎与实际运营入口

最终事务修正后，用旧 S1a 引擎和本次最终引擎，通过真实 `musicOperatorCommand → music-operator` 子进程各执行两轮。每轮都从同一无阶段记录的种子克隆，先完整重建，再在同一工作区重复构建；两个阶段均重新启动引擎进程。计时包含程序启动、进度确认、本地费用服务连接及完整处理。费用服务是本机夹具，只允许读取任务，所有运行均为 0 模型调用、0 提交费用。

| 样本 / 阶段 | 旧引擎两轮（ms） | 新引擎两轮（ms） | 均值，旧 → 新 |
| --- | --- | --- | --- |
| 舒伯特 / 无记录首次重建 | 7905.87 / 5826.55 | 6950.80 / 3961.07 | 6866.21 → 5455.93 |
| 舒伯特 / 重复构建 | 6748.61 / 3916.31 | 2013.14 / 957.95 | 5332.46 → 1485.54 |
| 贝多芬 / 无记录首次重建 | 8140.52 / 8491.61 | 7690.35 / 7781.54 | 8316.06 → 7735.95 |
| 贝多芬 / 重复构建 | 7431.39 / 8660.40 | 2096.82 / 2061.86 | 8045.89 → 2079.34 |

完整进程的重复构建均值分别降低 **72.1% / 74.2%**，所有候选命中预期阶段，manifest 与校准历史保持一致。这次打包对照未重现源码首轮较慢的现象，但首次重建样本仍只有两轮，舒伯特区间重叠，不将首次处理变快作为本轮结论。源码函数与打包进程是不同测量边界，结果分别保留。[最终打包原始结果](packaged-results.json)、[实际运营入口复跑脚本](packaged-benchmark.ts)。

## 正确性证据

每次运行均核对原录音字节和校准历史不变，校准后的 `occurrences`、`note_alignments`、`coverage` 与种子一致；同版本多次运行所得 manifest 一致。候选与基线还逐字节比较了 manifest、`source.txt`、`base.json`、可移植 curation、解析结果、渲染输入、全部 SVG、MIDI 和示范音频，全部通过。

Verovio 在每次实际渲染时，向 MEI 的 `<application>` 元素写入当前生成时间。新旧版本的 MEI 比较只去掉 **`<name>Verovio</name>` 所在 application 的 `isodate` 属性**，其余完整 XML 严格一致；原谱的 encoding-date 和所有音乐对象均保留。阶段复用命中时，不重新渲染，因此 MEI 与其他产物仍要求逐字节不变。

相关验证累计 **12 项通过**：

- [music-incremental.test.ts](../../../packages/core/src/music-incremental.test.ts)：6 项。通过记录真实处理器调用次数，验证旧工作区先完整处理再复用、缺失产物只补对应阶段、后段失败保留前段成果、路径与扩展名和处理参数影响复用、示范请求变化触发重算、发布失败回滚并恢复。
- [music-import.test.ts](../../../packages/core/src/music-import.test.ts)：1 项。保留原始来源与导读，校准历史继续重放，更新后的音乐事件正常接纳。
- [music-score.test.ts](../../../packages/core/src/music-score.test.ts)：5 项。谱面对象、调号/拍号/速度编码、布局映射和示范生成行为保持正确。

计数测试调用真实 ffprobe、ffmpeg 和 Verovio，只在测试边界记录调用次数。最初一次执行发现 MEI 自动生成时间影响跨渲染字节比较，修正测试比较口径后复测相关两项通过。最终还定向验证了发布事务：在多项文件已经推广后注入失败，先前音频、manifest 和完成记录全部回滚；恢复只重做未接纳示范，下一次全部命中。

核心 TypeScript 检查通过。最终的示范完成记录与音频原子提交修正在源码函数测量之后加入；它不改变命中复用路径，已用事务失败/恢复用例验证，随后也纳入上述最终打包运营入口对照。

## 实现边界

[music-import.ts](../../../packages/core/src/music-import.ts) 使用一个 `.build/music/completed.json` 记录以下完成阶段：

| 阶段 | 比较依据 | 缺失时补做的产物 |
| --- | --- | --- |
| 音频检查 | 录音来源身份、实际输入路径、媒体路径/扩展名、探测和解码程序及固定参数、处理版本 | probe 与完整解码验证 |
| 乐谱解析 | 谱本来源身份、实际输入与快照路径、解析版本 | 原 MusicXML、渲染输入与解析对象 |
| 乐谱渲染 | 解析输入绑定、Verovio/处理版本和固定渲染参数 | SVG 页、MEI、MIDI 和对象映射 |
| 合成示范 | 示范 ID、谱本来源、tempo、所选音符 ID 及实际音符数据、合成版本和参数 | 对应 WAV 和时长 |

原件沿用 `copyOriginal` 的逐字节比较；同一来源修订不能悄悄替换已存原件。复用判断不新增哈希或额外缓存服务。音频、解析和渲染每段成功后保存记录，即使后续校准或事件校验失败，恢复仍可复用前段。

处理版本和实际渲染参数集中在 [music-score.ts](../../../packages/core/src/music-score.ts)，合成版本和参数集中在 [music-demonstrations.ts](../../../packages/core/src/music-demonstrations.ts)。变更解析、渲染、打包 Verovio 或合成算法时应同步更新对应处理版本。

同一示范 ID 的 tempo 或选音变化仍允许产生新结果。新 WAV、校验通过的 manifest 和新完成记录通过现有 `publishAutomaticBuildArtifactSet` 一起提交，失败不会把旧音频与旧恢复记录拆开。修改示范标题仅更新展示数据，复用音频。直接校准入口保持原行为；完整重建每次仍执行校准历史重放、事件绑定、manifest/successor 校验和发布。

## 复现

在仓库根目录运行以下命令，临时目录指定到 E 盘：

```powershell
$env:TEMP='E:/allwork/download/agent/understand-book/tmp/continuous-executor-20261028/temp'
$env:TMP=$env:TEMP
pnpm --filter @understand-book/core exec vitest run src/music-incremental.test.ts src/music-import.test.ts src/music-score.test.ts
node --import tsx docs/performance/music-incremental-20261028/benchmark.mjs baseline
node --import tsx docs/performance/music-incremental-20261028/benchmark.mjs candidate
```

基线命令需使用改动前保存的实现，候选命令使用本次实现。脚本拒绝覆盖已有 benchmark 输出目录；再次测量时先为脚本选择新的独立根目录，并保留旧结果。两种模式均使用现有音乐 QA 种子，不调用模型。

## 已知限制

这里只验证两份现成选段的完整 rebuild/恢复。每份只有三次重复样本，首轮各一次，没有测量更长的整部作品、跨作品并行、编选与人工听核耗时或音乐读时交互延迟。重复构建仍读取和核对原件、重放校准、验证事件并发布清单；复用没有跳过这些工作。
