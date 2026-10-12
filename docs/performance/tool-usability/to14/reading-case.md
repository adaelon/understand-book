# 阅读案例：从雨水实验中核对数值与范围

任务：在 `rainwater-notes` 中查清北池从实验开始到结束减少多少升水，说明南池是否参与，并提供可核对的来源与短引文。

本例来自 [candidate-rainwater-1 的实际轨迹](runs/candidate-rainwater-1/agent/events.jsonl)。Agent 自行选择了四次调用；实际结果是 **37 − 19 = 18 升**，**南池未参与**。模型为 `gpt-6.1-sol`，工具执行经过原 MCP HTTP、发布材料及 SQLite 消费账本。

## 1. 选定材料及发布版本

调用 `book_list({})`，按材料名称选择返回条目，并复制完整 `published_book_ref`。后续调用都使用这一个引用。

目录提供可用能力和账户余额。它让 Agent 确定读哪份材料；原文结论需要随后取得正文。

## 2. 用字面词定位

调用 `book_search_text`，query 为 `北池`，published_book_ref 使用第一步的实际结果。此次命中指向 LID `1.1`，包括起始和结束处的出现位置。

定位结果用于选择读取范围。改后结果还提供候选 `next_calls`；调用者根据问题选用相应参数，不能把检索片段直接扩展为未读原文的结论。

## 3. 读取并续取

Agent 读取 LID `1.1`。第一页包含“北池起始有 37 升水”，但结束事实位于第二页，所以继续使用实际返回的 `next_calls`。

| 页 | 实际 UTF-16 范围 | 核对事实 |
|---|---|---|
| 第一页 | `[0, 8001)` | 北池起始有 37 升水 |
| 第二页 | `[8001, 8960)` | 北池有 19 升水；南池没有参与本次实验 |

第一页是 8,000 个 Unicode 字符，因材料含一个表情字符，UTF-16 范围长度为 8,001。续取 offset 从返回参数复制；直接把“字符数”当作 UTF-16 位置会算错边界。

## 4. 根据已读正文回答

两页提供完整前后数值及参与范围。Agent 计算 37 − 19，分别引用起始记录和结束／南池记录，从各自返回的 `sources[].source_url` 复制链接。两个来源保持同一个发布版本。

[实际终答与 usage](runs/candidate-rainwater-1/agent/metrics.json)和[原文、发布引用及消费账本](runs/candidate-rainwater-1/evidence.json)可以独立核对。此次免费目录不消费次数；检索和两页正文共消费 **3 次**，赠送额度从 12 变为 9。

## 复核与复跑

只核对现有样本，在仓库根目录执行：

```powershell
pnpm eval:tools:test
pnpm eval:tools:report docs/performance/tool-usability/to14/runs
```

要在新隔离材料和账号上重新执行完整阅读，使用本次固定的 Server 测试程序和 Codex 客户端。`remote-trial.mjs` 会启动原 MCP9 fixture、调用真实 Agent、保存轨迹并关闭临时宿主；目标目录须不存在。

```powershell
$env:TO14_CODEX='C:/Users/Lenovo/AppData/Local/OpenAI/Codex/bin/9691020b546a15b2/codex.exe'
node evals/tool-usability/remote-trial.mjs tmp/to14-new-reading D:/codex-build/to14-bin/candidate.exe
```

构建路径和客户端路径可以替换为同一冻结版本的实际位置；源码与编译依据见 [source-manifest.json](source-manifest.json)。真实任务的首次失败也应保存，不能覆盖成最后一次成功。

## 已知限制

- 本案例的 `reader.example` 链接属于临时夹具，宿主关闭后不能作为在线材料链接使用。实际服务应从当前获准的目录取得发布引用与来源链接。
- 这是一份可复跑阅读案例；该任务上的成功不证明所有工具族、教学场景或材料都具有同样表现。
- 各次实际 usage、失败及测量范围以本次汇总为准；上述两个复核命令不会调用模型，真实复跑命令会消耗宿主额度。
