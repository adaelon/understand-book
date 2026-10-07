# SR5 真实本地 Adapter 与配置确认 — 2026-10-01

状态：**已完成，下一步 SR6。** 合同：[ADR-0149](../adr/0149-构建期语义候选召回与Core-owned-Embedding-Provider.md)、[SR5](../切片方案-构建期语义候选召回.md)。

## 实施

`local-embedding-provider.ts` 将已验证的本地 MiniLM 接入 Core。读取实际模型 metadata 和 Transformers.js / ONNX Runtime 版本，固定 CPU q8、384 维、mean pooling、L2 normalize、无前缀、不截断、4/1 线程。批量上限 8，实际 tokenizer 上限 128。只加载已有本地文件，关闭远端模型加载；返回实际 input tokens 与本地推理 API 费用 0 USD。

`callEmbedding` 对 adapter 明确选择的本地 `EBUSY/EAGAIN` 错误最多重试一次。每次实际尝试都重新经过配置、取消与预算 gate，分别预留和记录用量。错误响应、超长输入、配置漂移和取消不自动重试。

`BuildRetrievalRuntime.open_provider` 只在已确认计划下、确实缺少语义准备结果时打开模型；本次 preparation 在成功、失败或取消后释放会话。普通 plan/snapshot/writer、lexical_only、空查询和有效准备页均不加载模型。墙钟取消覆盖加载和推理，本次配置与确认身份不同则保持 `build_plan_retrieval_drift`。

`build-retrieval-config.ts` 从宿主的 `UNDERSTAND_BOOK_EMBEDDING_CONFIG` 读取本地安装位置；配置文件相对路径以文件目录解析。BuildPlan 持久化检索模式、local、实际 provider/model/config identity、数据范围及额度，不保存安装路径。此 adapter 无 secret。配置不会从 Codex/DSH 的语义模型选择推断。

驱动增加 `build_retrieval_configure.v1` 与 `build_retrieval_confirm.v1`：配置生成新 revision 的草稿文件，旧计划保持；确认要求对应 plan digest。代码投影展示模式、执行位置、模型/revision/config、文本范围、文档/query 估计、失败/重试也计入的额度、总计划预算和费用类别。无明确配置的现有计划继续原词法路径。

`automaticBuildStepTransitions` 在原计划、invocation、租约和安装门禁之后交出检索准备请求。生产 CLI 的异步 step、refill 和 DSH controller 等待 Core 准备，再继续原驱动流程；同步 API 保留明确的待准备结果。CLI 信号和直接 API 的 AbortSignal 传入 preparation。Provider 失败/取消分别投影为 `retrieval_unavailable/retrieval_cancelled`，不归为语义候选错误。`automaticBuildNextWithPreparation` 也默认采用同一宿主配置。

DSH `ub_build_prepare_and_confirm` 可接收 retrieval mode 与 budget。Engine 先生成包含检索配置的草稿，既有用户确认框展示唯一的完整计划，批准后确认该计划并创建 invocation；拒绝不创建 invocation。DSH 进程启动仅增加传递本地 embedding 配置路径。Codex 使用相同代码生成的配置投影与确认命令。

## 配置与使用

本地安装配置示例（运行库和模型必须已经存在）：

```json
{
  "version": "local_embedding_config.v1",
  "runtime_dir": "E:/local-models/minilm-runtime",
  "model_dir": "E:/local-models/minilm"
}
```

在启动宿主前设置 `UNDERSTAND_BOOK_EMBEDDING_CONFIG` 为该文件的绝对路径。运行库目录包含 `node_modules/@huggingface/transformers` 与 `onnxruntime-node`；模型目录沿 SR0 格式包含 `model-metadata.json`。本次真实配置仍为 Transformers.js 3.8.1 / ONNX 1.21.0，模型 revision `2c4055b12046f11709e9df2c122e59ffbdc2f900`。

Codex 在已有代码生成的标准 BuildPlan 上，经 `<build-exe> build.step` 发送：

```json
{
  "version": "build_retrieval_configure.v1",
  "build_plan_path": "<现有代码生成的计划路径>",
  "retrieval_mode": "semantic_required",
  "budget": { "max_documents": 12, "max_queries": 4, "max_calls": 12 }
}
```

展示返回的 `review_markdown` 及对应计划身份，用户批准后发送 `build_retrieval_confirm.v1`，字段为返回的 `build_plan_path`、`plan_digest` 与 `confirmation_source: "codex_conversation"`；随后按原 invocation 创建/恢复协议执行。改变配置或提高额度重新生成并确认草稿；同 plan ID 的历史调用消耗继续累计。`config_file` 可用于指定预览的安装文件，执行宿主仍须设置上述环境变量指向同一实际配置。

DSH 的 `ub_build_prepare_and_confirm` 增加可选参数：

```json
"retrieval": {
  "retrieval_mode": "semantic_required",
  "budget": { "max_documents": 12, "max_queries": 4, "max_calls": 12 }
}
```

选择 `lexical_only` 时不需要模型配置，可以将三项 embedding 额度设为 0。这里的数字只是配置形状示例，每次真实执行仍以用户确认的计划及额度为准。

## 验证

运行目的：定向用例检测未确认/缺 provider/配置漂移仍发请求、重试漏计、会话不释放，以及异步驱动提前派发；现有驱动/补位/租约/教学/预算回归检测 SR5 接点改变原恢复或接纳行为。失败修对应实现，不放宽现有门禁。Core/DSH 类型检查检测跨同步、异步和前台接口的不兼容。

- 定向 4 文件 22 项通过。新增 9 项覆盖有限重试、逐次额度、延迟加载/释放、配置草稿与精确确认、Codex/DSH 异步准备，以及 refill/controller 的失败与取消。
- 首轮驱动测试替身误拦原基础准备，导致等待；改为只替代授权检索准备后通过。首轮类型检查发现闭包中的 provider narrowing 和测试中的重载替身签名，均已修正。
- 最终 Core 回归 **13 文件、153 项全部通过**，退出码 0；包含原驱动 52 项、教学 18 项、orchestrator 19 项，以及预算、计划门禁、补位、DSH session/plan 和本轮新路径。耗时约 311 秒，无未处理错误。
- DSH 前台 **2 文件、10 项通过**：实际进程加载配置、确认投影、批准创建、拒绝/取消/来源漂移不创建、原工具入口与预算边界。样本 runtime 只有 metadata，验证预览与批准不加载模型、不执行推理。
- Core 与 DSH plugin 类型检查均通过。未新增真实语义模型调用，除下述授权探针外所有用例均为确定性 fixture/fake。

```powershell
pnpm --filter @understand-book/core exec vitest run test/build-retrieval-config.test.ts test/automatic-build-retrieval-driver.test.ts test/automatic-build-retrieval.test.ts test/embedding-provider.test.ts test/automatic-build-driver.test.ts test/automatic-build-driver-failure.test.ts test/automatic-build-refill.test.ts test/dsh-build-session.test.ts test/dsh-build-plan.test.ts test/automatic-build-plan-gate.test.ts test/automatic-build-budget.test.ts test/teaching-build.test.ts test/build-orchestrator.test.ts --testTimeout=60000 --maxWorkers=1 --no-file-parallelism
$env:UNDERSTAND_BOOK_TEST_NODE = (Get-Command node).Source
pnpm --filter @understand-book/dsh-plugin exec node --import tsx --test test/control-confirmation.test.ts test/plugin-entry.test.ts
pnpm --filter @understand-book/core typecheck
pnpm --filter @understand-book/dsh-plugin typecheck
```

### 真实本地额度与结果

用户明确授权本轮最多 **12 documents / 4 queries / 12 calls**，含失败与重试；仅已有合成样本、已下载本地模型，无远端推理或模型下载。`evals/semantic-retrieval/sr5-authorized.ts` 经 confirmed BuildPlan 和正式 `prepareBuildRetrieval` 执行，不调用语义生成模型。

| 项目 | 实测 |
| --- | ---: |
| 文档 | 9，分批 8+1 |
| 查询 | 3，含一次取消与一次恢复 |
| 总调用 | 5 |
| 已知实际 input tokens | 143 |
| 预算预留 input tokens | 150 |
| 未知 usage 调用 | 1，取消的查询 |
| Provider 调用耗时合计 | 95.4 ms |
| 包含加载/释放的脚本总耗时 | 3717 ms |
| 本地推理 API 费用 | 0 USD |

有效页在未注入 provider 时原样复用；真实查询中取消，不接纳取消结果，原候选页保留；恢复只补 query，文档累计仍为 9；配置 revision 漂移在调用前阻断。详细身份、各次预留/实际用量与断言见 [真实验收 JSON](semantic-retrieval-sr5-real-20261001.json)。剩余额度为 3 documents / 1 query / 7 calls，仅属于本次验收，不作为 SR6 或新任务授权。

## 已知限制

- 默认仍为词法路径。SR6 的固定 Gold 三组身份质量对照、false merge/missed reuse、长材料及发布门槛尚未验收。
- 仅支持上述本地 MiniLM adapter，未提供远端 adapter。模型超过 128 tokens 明确拒绝；投影上限并不保证每种语言都落在 tokenizer 上限内。
- 正在进行的单次 CPU 推理不能抢占；取消后拒绝其结果并释放会话。宿主强制终止进程时可能来不及写取消状态，预先持久化的调用额度仍保留，后续可恢复缺失准备。
- 未提交 Git、未打包、安装或部署；既有安装包未包含此次源码。其他工作线改动保持。
