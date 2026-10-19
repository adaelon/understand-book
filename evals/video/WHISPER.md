# VI0b 本地 Whisper 实验

入口为 `whisper_experiment.py`，固定请求在 `whisper-experiment.json`。四段 VI0a 原声用于转写，三个短裁剪用于问答／校正复核，数字静音用于输入变化对照，fp16 的两块用于检查上下文和恢复。每次识别只传音频与相同识别参数，不传字幕、问题答案或候选术语。

## 配置与运行

在仓库根目录执行。Python 环境、模型和下载缓存只写入 `.worktree-local/`；实验媒体与结果默认写入 `evals/video/generated/vi0b/`。

```powershell
. .\.worktree-local\environment.ps1
py -3.11 -m venv .worktree-local/whisper-venv
& .worktree-local/whisper-venv/Scripts/python.exe -m pip install -r evals/video/requirements-whisper.txt
$env:HF_HOME = Join-Path $PWD '.worktree-local/huggingface'
$env:HF_HUB_DISABLE_XET = '1'
& .worktree-local/whisper-venv/Scripts/python.exe -c "from faster_whisper.utils import download_model; download_model('small.en', output_dir='.worktree-local/whisper-models/small.en', revision='d1d751a5f8271d482d14ca55d9e2deeebbae577f')"
node evals/video/media.mjs replay
& .worktree-local/whisper-venv/Scripts/python.exe evals/video/whisper_experiment.py prepare
& .worktree-local/whisper-venv/Scripts/python.exe evals/video/whisper_experiment.py run
& .worktree-local/whisper-venv/Scripts/python.exe evals/video/whisper_experiment.py report
```

当前选定 `faster-whisper 1.2.1 / small.en / CPU int8 / 8 threads`，英文、beam size 5、temperature 0、词级时间戳、VAD 静音门限 500 ms、关闭前窗文本提示。原始响应另存完整实际参数及依赖版本。下载与模型加载分别计时；结果中的 `elapsed_ms` 覆盖实际消耗完识别生成器的时间。

2026-10-10 实测记录见 [results/vi0b-20261010](results/vi0b-20261010/)。固定十项成功，累计输入 218.9996 秒、识别 59.773 秒、模型加载 2.110 秒；包括重复复核及分块。模型文件 483,545,366 字节。首次识别的结果保存失败后重跑，那次未保留耗时，未计入成功任务总耗时。

保持讲者顺序的机器转写节选及复核结论见[忠实转写样例](results/vi0b-20261010/transcript-sample.md)。

首次下载关闭 Xet 后在 88.927 秒完成；直连重下载同一模型耗时 96.329 秒，最终文件经实际模型加载和十项推理使用。下载缓存与临时文件仅在本工作区保留。

[faster-whisper 官方说明](https://github.com/SYSTRAN/faster-whisper)提供 CPU int8、词时间戳及 VAD 接口；[Whisper 原始实现](https://github.com/openai/whisper)采用约 30 秒内部窗口处理长输入。本实验使用同一模型配置承担转写和短音频复核。

## 输入、结果与恢复

```text
VI0a PCM + 原片时间映射 → prepare → jobs.json + audio/*.wav
    → run → results/<job>.json（running → completed / failed）
    → report → verification.json + index.html
```

每项结果保存输入来源、原片偏移、请求参数、完整原始 segments/info、词／句段原片位置、实际耗时和失败原因。模型区间越界时保留原始响应并将任务标记失败。WAV 尾端不足 1 ms 的容器／采样边界差异按实际采样记录。

NumPy 标量转换为 JSON 原生数值／布尔；VAD 默认的无穷大上限以字符串 `"Infinity"` 保存，其余非有限数同样按名称保存，避免生成标准 JSON 不能读取的常量。

`status` 查询本地结果状态。`run --only review-overflow` 只处理指定任务；已完成且输入／配置相同的结果复用，失败或遗留 running 的任务重新识别。没有远端任务 ID 或远端查询接口。改变模型／参数／材料后使用新的 `--out`，先 `prepare` 再 `run`；替换原片须按 VI0a 更新材料修订。

实测重跑复用了十项结果；另一次真实恢复试验先传缺失 WAV，记录 `FileNotFoundError`，补回原声后同任务成功并产生相同文字，重试耗时 3.486 秒。凭据见 [recovery/verification.json](results/vi0b-20261010/recovery/verification.json)。

重现失败恢复时使用全新 `--out`，`prepare` 后临时移动该目录的 `audio/review-overflow.wav`，执行 `run --only review-overflow` 应记录失败；放回同一 WAV 后重跑该命令应成功。此过程只操作新实验输出，不移动 VI0a 原片或已有结果。

## 长音频接入依据

`chunk_plan(duration_ms, chunk_ms, context_ms)` 生成连续、不重叠的核心区间，每块输入可带两侧上下文。试验以 36 秒 fp16 分成两个 18 秒核心，各保留最多 2 秒上下文；完整响应保留上下文词，`owned_by_core` 按词中点标注归属，边界中点只归右块。该标记不承诺模型在独立两块的边界绝无漏词或重复。

VI3 可按此入口采用 5 分钟核心、两侧 2 秒上下文，逐块保存原始响应与完成状态；失败块重新识别，完整结果从已保存块继续。5 分钟参数及整讲拼接需在 VI3 实测，不能用短片段结果宣称整讲效果。转写就绪由处理区间覆盖判断，不能把模型没有返回词直接当作已确认静音。

## 验证

```powershell
& .worktree-local/whisper-venv/Scripts/python.exe -m unittest discover -s evals/video -p test_whisper_experiment.py -v
```

单测针对实际 PCM 尾部边界、非零原片偏移、末尾不足整块、上下文归属、失败／中断重跑及原始结果保留。真实模型实验独立运行；恢复单测中的故障注入不算模型集成验收。

`report` 检查全部任务成功、两种不同语音得到不同非空文本、数字静音得到空文本。语义准确率与人工听核真值不由这些检查推导。原声和候选文字可在生成的 `index.html` 对照。

## 已知问题

- 用户确认 V05 两词均出现和 V04 关键短语；V07 只给出不确定语义。完整逐字真值、逐词时刻和精确停顿区间仍未人工标注，`correct_words` 保持空值；详见[听核记录](results/vi0b-20261010/listening-review.json)。
- `small.en` 将画面的 `seq1/seq2` 输出为 `sec1/sec2`，同段 `NANDs/NANs` 拼写随切片变化；`denser`、`sarcastic` 等输出也须复核。保留原始返回，后续通过有依据的校正草稿处理。该配置适合作为首个可接入的机器转写基线，不承诺术语全对。
- 完整 77 分钟转写与生产接入已由 [VI3](../../docs/视频全片转写与首发-VI3.md) 完成；Linux 和安装态仍待验证。
- 本地模型不产生 API 费用；运行占用的算力、电耗和模型下载流量未折算金额。
