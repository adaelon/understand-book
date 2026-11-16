# 回答质量与主流模型对照

把产品实际回答与同模型直接回答、指定主流模型的回答放在同一问题下比较。对照答案也是待核验的答案，不是标准答案。已有正式评分协议仍见 [QUALITY_EVAL](../semantic/QUALITY_EVAL.md)；这里负责采样与准备对照材料，结果保持探索性，不冒充通过人审校准的正式分数。

首批 [音乐题集](../music/answer-quality-cases.json) 包含六道开发题和两道保留题。M01 原样保留用户的问题。原来 Gemini 网页回答的模型版本、上下文和调用配置不全，只作历史观察，不与新实验合并统计。

## 线上路径与 Codex 对照

用户明确要求 DeepSeek 沿用线上产品路径、Astra 经 Codex 时，使用 `online-browser.mjs` 和 `online-comparison.mjs`。2026-11-11 的实际条件和结果见 线上路径对照。这一路不使用下述本地单用户服务和 Provider 代理。

先在隔离目录准备当前实际部署程序、`dist-multi`、准确的出版物和每题独立账号，保留线上模型配置与原生接口。由产品内置 full 观测保存公开模型请求及工具结果，并把账号凭据保存在实验目录的 `credentials.private.json`；该文件不进入报告或版本库。

```text
node online-browser.mjs --out <隔离实验目录> --cases <题集> --only M01,M06 --url <隔离HTTPS地址>
node evals/answer-quality/online-comparison.mjs --out <下载的实验目录> --only M01,M06 --codex <codex.exe>
```

浏览器必须真实发送原题，并保持播放器命令派发工作。运行前的页面检查可加 `--probe`，不会向模型提问。仅在采集错误后恢复同一已提交会话时使用 `--recover`，保留原错误并读取原回合，不能把它当成新样本。服务端记录的起止时间优先于采集恢复的等待时间。

导入要求每题保留 `sample.json`、`provider.json` 与完整公开 `observations.json`。同材料对照取自进入模型上下文的工具返回；原工具执行成功后仍可能因返回大小限制而在上下文中成为错误，因此不能改用更完整的后台观测来补充给 Astra。`comparison.json` 汇集两路样本，`answers.md` 保存答案全文。此采样不代表对多模态、交互和长期教学效果的全面验证。

若用户另行要求 Codex 自主读取完整预构建材料，使用独立样本目录，保留原输入和原答案；`runCodex` 的 `allowShell: true` 在已有只读模式下开放文件读取，`onEvent` 可保存实时读取记录。该条件不同于 `same_material` 的汇总工具返回。第一题的实际补充结果见 Astra 读取预构建材料。

以下运行说明对应首轮本地实验及通用导入入口。

## 输入条件

`knowledge`：主流模型直接收到原题和显式 `context`；产品可使用当前材料和工具。回答“产品整体比直接问模型增加了什么”，不将差异单独归因于某个工具。

`same_material`：先保存产品实际读取的工具结果，再把同一问题、显式上下文及这些材料交给对照模型。材料按工具调用去重，保留调用参数；不发送产品最终答案、评测标准或参考事实。若没有取得材料且题目也没有显式上下文，记录 unavailable，不偷换成空上下文测试。这一路依然有系统提示、交互和推理配置差异。

产品每题使用新进程、新对话、独立记忆，无谱面选区或播放光标。实际产品可自然调用工具；不在用户题目中追加“必须查谱”“必须引用”等答案模板。调用记录保留材料获取过程、实际用量和错误。调用预算沿用小规模实验的有界方式，达到限制即保留失败，不反复抽样直到出现好答案。

## 运行

先构建实际被测 Reader，再选择已发布的确切资料目录：

```powershell
cargo build -p server --bin server -j 1
node --test evals/answer-quality/core.test.mjs evals/answer-quality/run.test.mjs evals/semantic/provider-recorder.test.mjs
node evals/answer-quality/run.mjs run --book <已发布材料目录> --browser --codex <本机codex.exe> --out .understand-book/answer-quality/run-01
```

默认六道开发题、每题三路各一次：`product`、`same_model`、`astra`。产品和同模型对照复用 `.env` 的现有配置，`--product-model` 可显式选择当前服务返回的规范模型 ID，不改写 `.env`。Astra 使用已登录 Codex 的独立会话，默认 `gpt-6-astra / medium`，配置上关闭 shell、插件、应用、搜索、记忆和项目文档注入。Codex 仍有自己的系统上下文，也可能暴露资源查询工具；不能称为完全无工具或单次裸 API。首轮M02实际查询了两次资源目录，结果为空，详见该样本 `events.jsonl`。实际 CLI 版本、模型、推理配置和 Token 保存在每个样本中；订阅实际账单不可从 Token 推算。

首轮六题中，两路直答的用户输入文本经逐题比对均相同，但完整系统提示、执行流程和预算未对齐。产品一路运行本地Reader的真实代码和工具循环，两路直答没有复刻这套流程。首轮也没有将本地构建、资料、用户状态与线上实际部署逐项核对，因此是回答样本对照，不能据此宣称纯模型能力差距或线上产品质量。

本机2026-11-11的实际配置使用 `--product-model deepseek-flash`。配置文件中旧的 `deepseek-v4-flash` 别名经过本地记录代理后，无法直接与当前模型目录中的ID匹配；为保持原生接口能力选择，重跑本题集时继续显式指定服务返回的规范ID。

默认每路请求超时 300 秒；产品每题累计 Token 上限 800,000，同模型直答单次输出上限 32,768（含该服务计入输出的推理 Token）。可以分别用 `--timeout-ms`、`--product-token-limit`、`--direct-max-tokens` 调整，实际值写入运行记录。首轮历史运行使用 8,192 的直答上限；其截断记录保留，不能套用新版默认值解释。限额是在运行前固定的条件，不是能力评分；达到限制的样本不计为正常交付。

Provider 记录器同时支持 Chat Completions 和产品实际使用的原生 Messages 接口；模型发现请求单列，不混入生成用量。未报告的用量保留未知。产品 `MUSIC_RESULT_BUDGET` 是单次工具材料返回限制，与实验累计 Token 上限不同。

`--only M01,M06` 选择题目；`--baseline-model` 指定实际比较模型；`--arms product,astra` 省略同模型对照；`--server` 指定当前构建的 Reader。Runner 冻结可执行文件、相关未提交源码差异、资料引用和输入，保存到新的输出目录。保留题 M07/M08 默认不运行，后续修正完成后另开目录测试。

音乐等可能触发界面操作的题目使用 `--browser`。它复制已构建的 `packages/web/dist`（可用 `--web-dist` 指定），打开真实 Reader，由生产页面完成播放器命令，保存实际回执、页面错误、渲染正文与截图。原题仍由同一服务端请求提交，不增加光标或选区。播放回执不代表模型听过录音，也不是学习效果证据。省略此选项是 API-only 条件：如果产品尝试操作播放器，该条件下的超时必须标为环境影响，不能直接算作正常产品体验。

`--baseline-run <已有run.json>` 仅复用模型、推理配置和输入文本都完全相同的已交付 Astra 样本，保留其原运行路径、耗时和用量；材料变化会重新调用。复用不是新增独立重复采样。

原始运行和答案位于 Git 忽略的 `.understand-book/answer-quality/`。`run.json` 保留全部样本，`report.md` 汇集可读答案；每题目录保留 Provider 记录、产品实际材料、引用解析、原始交付和 Codex 调用。没有完整答案、超时、截断、调用失败不会从分母消失。

## 比较其他领域与已有输出

`--cases <题集.json>` 接受其他领域的问题，题集只需 `version` 与 `cases`。每题提供 `id`、`title`、`question`、`baseline_mode` 和用于评阅的 `assessment`；可附显式 `context`、`reference_facts` 和真实 Reader 的 `reader_request`。不要把答案标准放进 context。

也可以给每题设置 `saved_product`，路径相对题集文件，指向已获准使用的原始回答导出：

```json
{
  "answer": "实际交付的完整正文",
  "status": "delivered",
  "materials": [{"content": "当时实际提供给产品的原文或工具结果"}],
  "usage": null,
  "elapsed_ms": null
}
```

全部题目使用保存的回答时，无需 `--book`，不会重跑产品。使用 `--arms product,astra` 时也无需配置产品 Provider 密钥。原耗时、用量与本次导入耗时分列；报告展示原运行指标，未知值不以导入耗时代替。选定的导出应保留实际问题和上下文；不从其他用户历史、当前书库或后续修订中补造旧输入。

## 评阅

```powershell
node evals/answer-quality/run.mjs prepare-review --run .understand-book/answer-quality/run-01/run.json
```

每个成功配对生成 A/B、B/A 两份匿名输入，附原题显式上下文、实际工具材料和稀疏参考事实；模型映射另存 `jobs.private.json`。评阅时只给评委单份输入，不给映射或其他顺序的判断。原材料本身也可能出错，不能以复述材料作为正确性的充分条件。失败或截断样本保留在总报告，但不伪造配对偏好。若没有完整配对，明确报告零对；有一方没完成时不强行计算内容胜率。`delivered` 仅代表正常返回正文，内容是否正确、是否满足用户任务另行判断。

先核对具体重要事实，再判断是否完成用户任务、是否解释了所问关系。不同术语、不同合理分析路线允许并存；不按长度、标题数、引用数、工具数或与 Astra 的措辞相似度判优。记录具体答案片段与核验依据，允许两边都错、相当或证据不足。模型评阅未经相关领域的人审校准时只作探索性发现，沿用项目已有正式评分边界。

这一入口比较文字回答。来源是否真正支持论断、谱例是否正确播放、图表和交互是否有效，要结合保存的原始交付及产品专项验证；不能仅凭正文偏好宣称所有输出形态的质量已覆盖。
