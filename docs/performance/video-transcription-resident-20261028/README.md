# S3：同次视频构建复用本地 Whisper 模型

2026-10-28 完成。一次 `buildVideoTranscript` 调用中，首个获准且缺少原始响应的音频块启动 Python、加载 `small.en`；后续缺失块通过同一进程顺序转写。每块原始响应仍先原子落盘，再返回给 Engine 校验、保存进度和结算该块用量。默认 300 秒分块、2 秒上下文、CPU int8 / 8 线程及全部识别参数保持原值。

普通识别错误只使该块失败，同一模型可继续后续块；进程退出则停止剩余块的准入，剩余块保留待处理状态。完成或取消时关闭进程。恢复继续复用已保存的原始响应，只对缺失响应启动新的进程。现有 `evals/video/vi3-recovery.ts` 使用的一次性文件请求入口保留。

## 实测

已有 77.4 分钟、16 块长视频记录的模型加载合计 30.503 秒，占转写至状态保存耗时的 2.09%。该记录用于确定优化机制，不是本轮候选性能结果。[历史原始记录](historical-vi3.json)

本轮使用已有本地 `faster-whisper 1.2.1 / small.en` 和 36 秒 `fp16.mp4` 短片，固定 18 秒分块、2 秒上下文，共 2 块；每种方式运行两轮。计时从转写构建开始到发布完成，包含抽音、Python 启动、模型加载、识别、状态保存和发布，不含视频导入。未调用外部模型 API。

| 指标 | 每块单独启动的基线 | 同次构建常驻进程 |
| --- | --- | --- |
| 两轮完整构建耗时 | 20.465 / 18.549 秒 | 16.858 / 15.547 秒 |
| 均值 | 19.507 秒 | 16.202 秒 |
| 每轮模型加载次数 | 2 | 1 |
| 两轮模型加载耗时 | 4.065 / 3.759 秒 | 2.349 / 1.941 秒 |
| 原始 segments（含字词时间戳及概率） | 两轮相同 | 与基线逐字段相同 |
| 正文及覆盖 | 两轮相同 | 与基线相同 |

本样本均值减少 3.304 秒（16.94%），两轮候选均快于两轮基线。原始结果见 [baseline.json](baseline.json)、[resident.json](resident.json)，比较见 [comparison.json](comparison.json)。后续块 `load_ms` 为 0，加载时间只归属首块；`inference_ms` 仍逐块计量。

## 验证

- 修改前冻结了两个生产文件及现有测试：[baseline/](baseline/)。原有转写与费用测试 10/10 通过：[baseline-tests.json](baseline-tests.json)。
- 候选的转写、费用及常驻进程测试合计 16/16 通过：[candidate-tests.json](candidate-tests.json)。
- 最终补上 Windows 管道 UTF-8 编码后，常驻进程 6/6 再次通过：[resident-final-tests.json](resident-final-tests.json)。夹具目录包含中文与空格，父进程故意设置 ASCII 管道编码，验证适配器明确使用 UTF-8。
- 新测试调用实际 Python 适配器，仅将 `faster_whisper` 模块替换为受控模块；断言同一进程加载一次、三个块逐次保存并结算、失败块恢复、全部准入拒绝时不启动模型、取消时保留已完成块和未知费用、回执丢失不重做识别、进程退出不继续预占后续块。
- `node node_modules/typescript/bin/tsc -p packages/core/tsconfig.json --noEmit` 通过。
- 最终 Windows 引擎实际执行 `video-transcribe`，使用随程序内嵌的 Python 适配器完成两个真实音频块。加载耗时为 2203 / 0 ms，完整 segments 与正文均同源码对照一致，模型 API 调用 0。见[打包验证回执](packaged-verification.json)及[复跑脚本](verify-packaged.mjs)。本次单轮耗时只用于安装路径验收，不并入前面的性能样本。

定向测试命令（仓库根）：

```powershell
$env:TEMP='E:/allwork/download/agent/understand-book/tmp/video-transcription-resident-20261028/temp'
$env:TMP=$env:TEMP
node node_modules/vitest/vitest.mjs run --dir packages/core/test packages/core/test/video-transcript-resident.test.ts packages/core/test/video-transcript.test.ts packages/core/test/service-usage.test.ts --maxWorkers 1 --no-file-parallelism
```

本地实测复跑入口是 [measure.ts](measure.ts)。使用新的输出目录，避免复用已经完成的构建。`baseline` 模式调用冻结的一次性 Python 适配器，复现每块独立启动；`resident` 模式走当前实际构建路径。本次最初的基线在生产代码修改前直接运行，完整原源码也已保留。

```powershell
$sample='E:/allwork/download/agent/understand-book-video-understanding/evals/video/generated/clips/fp16.mp4'
$python='E:/allwork/download/agent/understand-book-video-understanding/.worktree-local/whisper-venv/Scripts/python.exe'
$model='E:/allwork/download/agent/understand-book-video-understanding/.worktree-local/whisper-models/small.en'
$output='E:/allwork/download/agent/understand-book/tmp/video-transcription-resident-20261028/rerun'
node --import tsx docs/performance/video-transcription-resident-20261028/measure.ts baseline $sample $python $model $output 2
node --import tsx docs/performance/video-transcription-resident-20261028/measure.ts resident $sample $python $model $output 2
```

## 已知限制

两轮短片比较只支持该输入和本机配置下的结论。短分块使加载开销占比较高，不能将 16.94% 推广到默认 5 分钟分块的长视频；已有长视频的加载占比仅 2.09%。尚未重跑整部长视频，也未测量峰值内存；模型现在驻留到本次转写结束。全文逐字真值仍沿用既有样本限制，本次确认候选没有改变该样本的转写内容和时间戳。

外部额度服务通过实际 `TranscriptServiceLedger` 配合本地响应夹具验证；没有发出真实费用请求。最终引擎已重新打包，`video-asset whisper` 及实际编译入口均已确认使用常驻脚本。
