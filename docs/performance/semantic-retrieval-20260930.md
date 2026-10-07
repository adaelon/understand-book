# SR6 三组身份质量与长材料验收

状态：**验收工具已实现，两轮真实 Gold 已结束，SR6 验收未通过；长材料计划待确认。** 2026-10-01。合同：[ADR-0149](../adr/0149-构建期语义候选召回与Core-owned-Embedding-Provider.md)、[SR6](../切片方案-构建期语义候选召回.md)。完整汇总、冻结实验合同及代表性拒绝响应见 [机器记录](semantic-retrieval-sr6-20261001.json)。

## 实验入口与判定

`evals/semantic-retrieval/sr6.ts` 使用固定 28 records / 8 queries / K=6、12：A 为原 catalog JSON substring，B 为 Core exact/alias + lexical，C 为相同词法实现加真实 MiniLM。固定 query 的排序读取 SR1 保存的真实向量，直接核对当前完整投影文本和 query，复用不新增推理。

三组分别从相同完整 Gold ledger 开始，调用同一个 `formal-objects-extractor.md`，由真实模型选择 search/inspect/read/resolve/identity/finish。每次 action 都进入 `advanceObjectAlignment`，原文读取、inspect、引用、覆盖和稳定身份接纳保持。Gold 标签、目标对象对及判定理由不进入模型输入。

固定 query 排名与整份自主对齐分别报告。身份指标只取已完成、经 Core finish 接纳的状态，以 redirects 和 existing_ref 检查预定分离/同一对象对；未完成时为 null。逐例保留结果，汇总按无向对象对去重。该指标覆盖标注对象对；全部语义质量还需独立来源审阅。

`sr6-run.ts` 提供 prepare/confirm/run/report；计划保存实际 prompt、Gold、模型/endpoint、温度、输出上限和决策预算，续跑时直接比较。A/B/C 并行但各自持有独立 ledger 和模型请求，无共享对话。每组最多 384 次生成，每次决策最多 32 个非进展动作。调用前保存预留，收到响应后保存原文与用量，再执行 Core；已收到而未接纳的响应可续接，进程中断留下的未知调用不可当成零消耗。

C 的动态目录/query 经已确认 BuildPlan 和 `prepareBuildRetrieval` 请求本地模型。Core 复用有效准备时不必新增 slot；实验入口按真实依赖核对复用页，缺页/过期页明确停止。B 保持零 embedding，A 通过原搜索 action 路径。

## 本轮授权与执行

用户批准 `tmp/sr6-gold/plan.json`：`deepseek-v4-flash` / `https://api.deepseek.com`，temperature=0，单次最多 4096 output tokens；生成调用上限 1152、累计执行墙钟 120 分钟。只发送固定自编速率 Gold。响应 API 提供的 prompt/completion token 原样记录，未知用量单列，未计算货币费用。

本地 MiniLM 限额 1024 documents / 256 queries / 512 calls，含失败重试，禁用远端权重下载；模型及运行库同 SR5。Embedding BuildPlan ID `sr6-gold-local`，draft digest `b4b470521fd82d600193a03a0bdca73137f72b86c656906d28e91f0bb29d2223`。

A 组启动后为修正实验驱动的准备页复用接点停止过一次；第 23 次调用已发送而响应未知，保留 failed、未知 usage 和额度，不重置 ledger。前 22 次的有效动作、拒绝与实际用量保留。随后以同一已确认计划续跑，并行运行三组。部分响应耗尽 4096 output tokens 而没有 JSON，计入拒绝和实际消耗，不剔除。

```powershell
node --import tsx evals/semantic-retrieval/sr6-run.ts prepare tmp/sr6-gold tmp/sr5/embedding.json
# 仅在用户批准代码投影后执行一次 confirm
node --import tsx evals/semantic-retrieval/sr6-run.ts confirm tmp/sr6-gold
node --import tsx evals/semantic-retrieval/sr6-run.ts run tmp/sr6-gold tmp/sr5/embedding.json
node --import tsx evals/semantic-retrieval/sr6-run.ts report tmp/sr6-gold
```

运行记录：`tmp/sr6-gold/{plan,ranking,A,B,C,report,elapsed,failures}.json`；本地调用记录位于其 `.build/teaching/retrieval.json`，向量位于 `.build/semantic-retrieval/cache.json`。`plan.json` 已存在时 prepare 拒绝覆盖，恢复使用 run，不另建目录重置额度。

用户随后批准三组统一 `thinking=disabled` 重做，保留首轮并累计原额度。第二轮目录 `tmp/sr6-gold-nonthinking`，使用 `prepare-revision NEW_DIR CONFIG PRIOR_DIR → confirm → run`；三组从同一原始 Gold 重启，每组相同上限 216 次（384 减去首轮最大组消耗 168），prompt、读取 gate 和标签不变。本地模型继续使用首轮工作区的预算账本和缓存。实际响应报告模型名为 `deepseek-flash`，请求模型名仍为 `deepseek-v4-flash`。

## 真实结果与发布决定

| 轮次 | 组 | 生成调用 | 接纳动作 | inspect / read | 拒绝 | 完成 | 错误合并 / 漏复用 |
| --- | --- | ---: | ---: | --- | ---: | --- | --- |
| Provider 默认思考配置 | A | 168 | 143 | 12 / 28 | 23 | 否 | 未知 |
| Provider 默认思考配置 | B | 128 | 112 | 8 / 30 | 16 | 是 | 0 / 0（仅标注对象对） |
| Provider 默认思考配置 | C | 6 | 2 | 1 / 1 | 4 | 否 | 未知 |
| thinking=disabled | A | 5 | 1 | 0 / 1 | 4 | 否 | 未知 |
| thinking=disabled | B | 24 | 10 | 3 / 4 | 14 | 否 | 未知 |
| thinking=disabled | C | 14 | 8 | 1 / 4 | 6 | 否 | 未知 |

首轮 C 的空响应消耗了单次 4096 output tokens。非思考轮三组均由连续三次拒绝停止：A/C 重复返回 `{action:{kind:...}}` 或 `{type:"json_object",action:...}`，不符合顶层动作合同；B 还尝试合并整体与其组成部分，Core 以 `alignment merge creates self containment` 拒绝。未解包模型输出、代写语义动作或降低 gate。首轮 B 的完成结果不能替代非思考轮 B，未完成组的质量保持 null。

固定 query 排名使用 SR1 留存真实向量，结果如下；C exact/alias 命中保留。此处排名增益不能证明完整身份质量。

| 组 | recall@6 | precision@6 | recall@12 | precision@12 |
| --- | ---: | ---: | ---: | ---: |
| A | 12.5% | 2.083% | 12.5% | 1.042% |
| B | 12.5% | 2.083% | 12.5% | 1.042% |
| C | 100% | 18.75% | 100% | 9.375% |

两轮累计生成 **345 / 1152 次**；已知 input tokens **657,113**、output tokens **509,253**，另有 **2 次**被中断调用的用量未知，均占调用额度。记录的保守累计时间 **23.86 / 120 分钟**。本地 embedding 累计 **29 / 1024 documents、4 / 256 queries、9 / 512 calls**，已知 tokens=547、unknown=0，推理耗时合计约 184ms；该耗时不含所有准备和加载时间。本地 API 费用 0 USD，生成货币费用未核算。剩余额度不自动批准改变 prompt/model 或新实验轮。

**发布决定：保持 `lexical_only`。** 机器判定 `eligible=false`，原因是缺少三组完整结果和长材料验收。SR6 不能标为完成。下一步先修正模型遵守动作合同的问题，按同一修订合同重新设计三组对照；真实长材料仍使用下述独立草稿确认。

## 长材料计划

用户同意来源为 `E:/allwork/download/agent/lifebook/.understand-book/ai-infra-book-complete`。`sr6-long-preview.ts` 调用当前源码的正式计划生成器和 retrieval configure 入口，保留此前 Pass2=disabled；基础三阶段全部可复用，新增 formal_objects / cognitive_materials / teaching_publish。当前正式对象分块任务 338 个，后续任务与 tokens/耗时估计未知。

草稿投影：`tmp/sr6-long-preview.json`；BuildPlan ID `plan-d6c0c579ae21a001`、digest `ac925088541c928514bb437bdd7ea9a777fe0e0c5efd18a8afc32174efb57b46`。草稿额度为 5,000,000 total tokens / 240 分钟、本地 20,000 documents / 5,000 queries / 10,000 calls。当前 status=draft，未调用真实长材料生成或 embedding；这些额度尚未授权。

## 验证

定向测试检测指标漏算、未读引用被接受、决策预算失效、未知值冒充零、Gold 标签进入模型输入、旧 substring 偏离、旧向量被错误采用，以及缺少 Agent/长材料证据却判定可发布。另覆盖 Core 不新增 slot 时复用当前页和过期页拒绝。发现问题修对应入口，不改 Gold 或发布门槛。

`semantic-retrieval-sr6.test.ts` 6 项、`teaching-object-alignment.test.ts` 16 项、`teaching-source-review.test.ts` 3 项，共 25 项通过。新增 CLI 测试实际启动 preview/confirm/revision，检测未确认调用、重置原额度和改写首轮计划；不调用真实 provider。读取越界用例先红后绿：Core 返回实际段长及 2000 UTF-16 上限，错误读取仍不计入覆盖。Core 类型检查通过；实验 runner 与长材料 preview 单独纳入 TypeScript 检查。

## 已知限制

- SR6 尚未完成；两轮真实 Agent 对照已停止，长材料、独立 source review、实际 failure/cache rebuild/config change 恢复、稳定身份与 Reader 就绪均未验收。
- 默认保持词法路径。发布判定要求三组完整结果、C 对 B 召回提高、exact/alias 保留、标注分离对零误合并、漏复用不恶化及长材料验收。
- 现有安装包未包含全部教学/SR5 源码；本轮入口属于源码验收，不代表安装包已更新。
- 当前源码已编译独立验收 Engine：`tmp/sr6-engine/understand-book-build-x86_64-pc-windows-msvc.exe`；未安装或部署，未提交 Git。
