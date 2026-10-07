# SR1–SR3 构建期语义召回 — 2026-10-01

状态：**SR1、SR2、SR3 已完成；下一步 SR4a。** 合同：[ADR-0149](../adr/0149-构建期语义候选召回与Core-owned-Embedding-Provider.md)、[切片方案](../切片方案-构建期语义候选召回.md)。本次用户确认实施 checkpoint 的 SR1–SR3。

## 实施状态与接点

- [x] SR1：异步 Provider 合同、确定性一跳投影、计数 fake 与同接口真实本地样本。
- [x] SR2：可再生缓存、按当前投影重算、brute-force cosine、失败续补、真实删除重建及 3k/10k 测量。
- [x] SR3：exact/alias + lexical + semantic、完整空查询浏览、稳定分页、显式准备数据的 alignment 接点。

`embedding-provider.ts:callEmbedding` 显式比较 provider/model/revision/config/dimensions，校验批量、真实 token 计数、返回条数、有限非零向量及维度。document/query 使用同一身份，调用前后均检查身份；取消传到 adapter，并拒绝取消后的结果。每次实际调用记录角色、记录数、耗时、成功/失败/取消及可获得 usage，未知 token/费用保持缺省。重试与授权属于调用方。

`semantic-retrieval.ts:retrievalCatalog/projectRetrievalCatalog` 按当前 qualified key 或 previous 完整 `{source_id, object_id}` 引用解析，previous 取最高有效 object_revision。投影顺序为 kind、meaning、aliases、conditions、角色配对、组件摘要；仅展开引用对象的基础语义字段。`formal_object_retrieval_projection.v1` 固定每条引用摘要前 256 个 Unicode code points、整体前 1024 个；完整 meaning/aliases/词法字段保持。模型实际 tokenizer 再检查 MiniLM 的 128-token 上限，超限明确拒绝。content_digest 仅绑定 embedding_text，用于省去未变文本的 embedding。

`semantic-retrieval-cache.ts:prepareRetrievalVectors` 在 `.build/semantic-retrieval/cache.json` 保存格式版本、provider 身份（含维度）、projection version、key→digest 映射及 digest→vector。一次文件替换使这些字段一起提交；一轮准备结束或失败时保存已完成批次，不逐批重写整个增长中的缓存。维度不符的项重算，模型/config/projection 变化失效；相同文本共享向量、不同 key 保持独立。缓存只管理自己的文件。

`semantic-retrieval-preparation.ts:prepareSemanticRetrieval` 生成可 JSON 保存的 `PreparedRetrieval`，记录实际 records、request、policy、provider/projection 与融合序列，直接比较依赖决定复用。offset 不属于排序依赖；翻页、inspect/read 复用序列，policy 变化复用 document/query vectors。词法字段变化即使未改变 embedding_text 也会刷新排序。空查询及 lexical_only 不创建缓存、不调用 provider；已有有效准备可在缓存删除、adapter 离线时继续使用。

`retrieveHybrid` 先按当前 key 过滤 self/退出目录项并去重，再保留全部 exact/alias 前缀，以 lexical、semantic 交替补位。遇到重复项合并 match_reasons 并继续扫描，semantic 最多新增 12 个对象；同分按 key。空查询按 key 浏览完整目录，含 focus/current/previous；每页 6 项，返回 candidate_count、next_offset 与 semantic_truncated。diagnostics 保存 cosine，模型页只包含当前对象摘要及命中理由。

`teaching-object-alignment.ts:alignmentRetrievalRequest/objectAlignmentInput/advanceObjectAlignment` 接受显式传入的准备记录；search 接纳只保存 query/offset，未准备或过期页面拒绝渲染。inspect/read 保持请求，resolve/identity 清空请求并转到下一 focus。**现有 teaching router 尚未注入准备结果，仍沿用 SR0 substring 路径。** SR4a 接入授权/预算/调度后才切换生产路径；SR4b 仍负责 action 重放。

Agent prompt 已说明可选准备页合同。为避免新 prompt 与旧不可变任务同路径冲突，正式对象 policy/generation 升至 `formal_objects.v3` / `formal_objects.full.v3`；action schema 保持 `formal_objects.v2`，alignment 状态保持 `formal_object_alignment.v2`，成品保持 `formal_objects.v1`。v1/v2 generation 保留，既有正式身份继续走 previous 基线。

## 验证结果

运行目的：Provider 测试发现越限/身份/响应/取消合同违规；投影测试发现标识污染、引用解析或一跳传播错误；缓存测试发现不必要调用、漏重算、失败批次丢失或错误向量复用；融合测试发现 exact 丢失、重复占配额、分页漂移及显式请求被覆盖。失败时修对应实现；教学回归检测 read/identity/coverage/冻结恢复/发布接点变化，保留原门禁修正接点。类型检查检测新接口与现有调用不兼容。

- 9 文件回归一次通过 **58 项**，包括原 41 项教学/Gold 测试和新 17 项。
- 随后补充一跳非递归、v2 generation 保留、identity 清空显式请求 3 项；对应文件定向运行通过。最终覆盖 **61 项**，没有重复跑已通过的整套回归。
- Core `tsc --noEmit` 通过。
- 首轮测试中的两处夹具预期已修正：active_refs 与对象共享引用，需要替换引用才能模拟错源；“去程”还被两条条件关系引用，实际应重算自身、整体及两条关系共 4 项。生产逻辑正确，未为旧预期减少依赖传播。

命令（仓库根目录）：

```powershell
pnpm --filter @understand-book/core exec vitest run test/embedding-provider.test.ts test/semantic-retrieval.test.ts test/semantic-retrieval-cache.test.ts test/semantic-retrieval-hybrid.test.ts test/teaching-object-alignment.test.ts test/teaching-build.test.ts test/teaching-map.test.ts test/teaching-object-fragments.test.ts test/semantic-retrieval-baseline.test.ts --testTimeout=30000
pnpm --filter @understand-book/core typecheck
```

## 真实本地样本与额度

沿用 SR0 已下载的 `Xenova/paraphrase-multilingual-MiniLM-L12-v2`，revision `2c4055b12046f11709e9df2c122e59ffbdc2f900`；Transformers.js 3.8.1 / ONNX Runtime 1.21.0，CPU q8、384 维、mean pooling、L2 normalization、无前缀、禁用 SDK truncation。最大批量 8，远端模型加载关闭。`local-provider.ts` 是实验 adapter。

| 运行 | 文档 / 查询 | 实际 tokenizer tokens | 结果 |
| --- | ---: | ---: | --- |
| SR1 同接口固定 Gold | 28 / 8 | 542 | 最大输入 44 tokens；12 次调用，含加载约 1616 ms |
| SR2 六条 Gold 记录删除缓存重建 | 12 / 2 | 212 | 两次融合序列一致，最大 cosine 差 0；准备约 97 ms，不含模型加载 |
| 本轮总量 | **40 / 10** | **754** | 无重试、无远端推理 |

[SR1 向量与用量](semantic-retrieval-sr1-vectors-20261001.json)、[真实重建](semantic-retrieval-sr2-rebuild-20261001.json)。推理 API 费用为 0（本地）；计费 token 不适用，电力费用未测。以上探针合计用完本轮 40 documents / 10 queries 样本额度。

SR3 排名只读取 SR1 已保存的真实向量，没有新增推理；用正式 Core 排序代码固定 Gold/query/K：

| 组 | recall@6 | recall@12 | precision@6 | precision@12 |
| --- | ---: | ---: | ---: | ---: |
| B exact/alias + lexical | 12.5% | 12.5% | 2.08% | 1.04% |
| C 同一词法 + embedding | 100% | 100% | 18.75% | 9.38% |

[逐例结果](semantic-retrieval-sr3-ranking-20261001.json)。9/9 应比较目标进入前六；排序是候选证据，身份质量仍待 SR6。

复现实验入口：`sr1-probe.ts RUNTIME MODEL OUTPUT`、`sr2-rebuild.ts RUNTIME MODEL OUTPUT`、`sr3-ranking.ts SAVED_VECTORS OUTPUT`，均位于 `evals/semantic-retrieval/`。前两个需要明确的本地样本额度；第三个仅离线计算。

## 单书规模

[scale.ts](../../evals/semantic-retrieval/scale.ts) 在 Ryzen 7 5800H / Node v24.9.0 上对每个规模测量一次，使用 384 维合成向量。目的是判断朴素投影/余弦是否可承担单书目录；超出可用延迟或内存时再调整实现。

| records | 投影 | cosine + 排序 | heap 增量 | RSS 增量 | cache JSON |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 3,000 | 17.9 ms | 16.9 ms | 23.3 MiB | 40.5 MiB | 21.4 MiB |
| 10,000 | 50.9 ms | 69.4 ms | 48.0 MiB | 138.4 MiB | 71.4 MiB |

[原始结果](semantic-retrieval-sr2-scale-20261001.json)。这些结果支持继续使用 brute-force cosine；缓存序列化成本须在真实长材料中记录。

## 已知限制

- SR4a 的确认计划、预算、ordinary snapshot 待准备状态和调用用量持久化，SR4b 的当前 ledger action 重放，以及 SR5 生产 adapter/配置面尚未实施。普通生产 alignment 仍是旧 substring，实验接口不能视为生产 semantic mode 已开启。
- 现有完整后状态恢复缺口及其 SR0 测试保留；准备记录具备序列化形状，尚未接入 teaching task/artifact 的恢复调度。
- 128-token 超长 document/query 明确拒绝；1024-code-point 投影并不保证自动适配所有语言的 token 上限。词法字段保留完整内容。实验本地 ONNX 调用可在前后响应取消，正在运行的单个 CPU 推理不可抢占。
- 规模结果不含真实 embedding、磁盘读写和长材料闭环，内存为单次进程测量。真实 Agent 的 false merge、missed reuse、read/inspect、总成本和发布门槛留待 SR6/T5A。
- 未提交 Git、未打包、未安装或部署。工作树中其他工作线改动保留；现有安装包不代表本次源码。
