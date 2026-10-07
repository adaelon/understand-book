# SR0 构建期语义候选召回基线

实施日期：2026-10-01。状态：**SR0 完成，继续 SR0a**。合同：[ADR-0149](../adr/0149-构建期语义候选召回与Core-owned-Embedding-Provider.md)、[SR 方案](../切片方案-构建期语义候选召回.md)。文件日期沿用方案预定路径。

后续状态：SR0a 已完成，见 [SR0a 记录](semantic-retrieval-sr0a-20261001.md)。下文保留 SR0 实验当时的 v1 行为、漏洞与测试结果，作为历史基线。

当前 substring 对受支持的跨块改写存在漏召回；真实本地 embedding 在固定 Gold 上找回这些应比较对象，满足 SR0 继续条件。生产 alignment 行为保持 v1，后续先补原文读取接纳。

## 来源与预先固定的判断

[Gold](../../packages/core/testdata/semantic-retrieval/gold.ts) 在首次 provider 排名前固定。复用仓库自编速率来源，增加自编车辆牵引方向及物理量定义段。两个 fragment 共 27 个当前对象，加 1 个由真实正式对象接纳器产生的 previous 对象。完整来源、对象、query 和旧算法返回序列保存在 [输入快照](semantic-retrieval-sr0-input.json)。不存在用户书籍摘录。

8 个 query 覆盖低词面同义、同词异义、高相关非同一、关系条件、角色方向、整体与部件、previous 改写。共 9 条“应比较”目标、9 条明确分离对象对标签；Gold 同时保存来源 LID、对象对和判定理由。“应比较”包括需要看过后分离的对象，不等于同义或允许合并。13 个其他物理概念作为有来源的干扰对象。来源与对象通过现有正式对象、coverage、identity 接纳路径验证。

预先固定：page size=6，K=6/12，主指标为逐 query 宏平均 recall@6；SR0 继续条件是旧 substring 有遗漏且真实 provider 在前六名找回。SR6 发布门槛为 C 高于 B、exact/alias 保留、明确分离对 false merge=0、整体 false merge 与 missed reuse 不劣于 B，并使用相同模型、prompt、读取门禁、决策预算。未按观察结果重写 Gold、query 或门槛。

## 当前行为与已存在缺口

`teaching-object-alignment.test.ts` 锁定以下现状：

- 空白分词、lowercase、任一词命中 catalog JSON；内部 key/来源 revision 也能命中，非空 search 保留 focus 自身。
- 空 query 按当前对象顺序再接 previous 浏览，每页 6 项；恰好到末尾返回空页，超过末尾拒绝。
- 非 focus merge 对象须 inspect；identity 的当前目标和额外 previous 须 inspect，未覆盖的旧身份阻止 finish，多对多映射拒绝。
- inspect/read 保留显式搜索；最近三条 inspected 完整记录进入输入，inspect 历史 key 继续保留；resolve 清空本次 inspect/search/reading。
- resolve 校验来源归属、revision 与允许 LID；identity 证据必须绑定当前目标。原正式对象接纳器继续校验完整 coverage 与 split/merge cardinality。
- **SR0a 缺口**：inspect 后无 read 可 merge、identity；读取某 LID 的一个字符即可允许整段引用。当前允许来源集合并不等于实际读取覆盖。
- **SR4b 缺口**：未展示对象修改后 bounded rendered input 可以完全相同。真实 teaching task/writer 测试证明 artifact 保存整份后状态，accepted lookup 按原输入返回旧后状态，而重新 freeze 完整 task 会报 changed；路由的 `alignment = result` 恢复仍使用整份旧账本。该测试刻画同输入/不同 ledger 的接点，不冒充已经实现 action replay。

两项缺口的测试名称明确带 `SR0 known gap` 或现状说明；它们是当前行为的刻画，SR0a/SR4b 实施时应改为新合同断言。

## 实验实现

入口 [prepare.ts](../../evals/semantic-retrieval/prepare.ts) 调用 [baseline.ts](../../evals/semantic-retrieval/baseline.ts) 构造现有 `ObjectAlignmentWork` 并通过真实 `advanceObjectAlignment(search)` 取得 A 的完整分页序列。

| 组 | 实现 |
| --- | --- |
| A | 当前生产 catalog JSON substring，含 focus 自身 |
| B | 独立实验 exact/alias + 语义字段 substring，过滤 focus、按 key 稳定排序 |
| C | 与 B 相同词法实现，加真实向量 cosine 排名，按 SR3 交替补位规则融合，semantic 最多贡献 12 条 |
| Semantic | 相同向量的纯 cosine 排序，过滤 focus，用于单独观察 provider 贡献 |

实验投影依次使用 kind、meaning、aliases、conditions、角色对应对象摘要与组件摘要，一跳展开，不含对象 ID/路径/revision。当前与 previous 使用相同语义字段；本次 previous 为简单 concept。模型仍须 inspect/read 后判断关系，排序本身没有身份修改入口。[ranking.mjs](../../evals/semantic-retrieval/ranking.mjs) 为离线原型，生产投影、缓存及搜索合同由 SR1–SR3 落地。

## 真实 Provider 与用量

本地 CPU 推理：`Xenova/paraphrase-multilingual-MiniLM-L12-v2`，revision `2c4055b12046f11709e9df2c122e59ffbdc2f900`；Transformers.js **3.8.1**，ONNX Runtime **1.21.0**，q8，384 维，mean pooling、L2 normalization，document/query 均无前缀。模型依据：[ONNX 模型卡](https://huggingface.co/Xenova/paraphrase-multilingual-MiniLM-L12-v2)、[原模型卡](https://huggingface.co/sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2)。

固定实验额度为最多 40 documents、10 queries，batch=8；按原模型 128-token 序列设置输入上限，先用实际 tokenizer 检查，超限报错。实测最大输入 44 tokens，批量 8/8/8/4 documents 和 8 queries；未探测 provider 极限批大小。输出验证条数、384 维与有限数值，关闭远端模型加载，所有推理在本机完成。

主机 Windows x64 / Ryzen 7 5800H / Node v24.9.0；intra-op=4、inter-op=1。正式结果见 [结果 JSON](semantic-retrieval-sr0-result.json)：

| 项目 | 结果 |
| --- | ---: |
| 文档 / 查询记录 | 28 / 8 |
| document 批次 / query 批次 | 4 / 1 |
| tokenizer 实际输入 token（含特殊 token，不含 padding） | 458 / 84，共 542 |
| 模型加载 | 847.9 ms |
| 五批模型调用合计 | 141.5 ms |
| 加载、tokenize、推理和排序总耗时 | 1002.4 ms |
| embedding 重试 / 远端推理调用 | 0 / 0 |
| 推理 API 费用 | 0（本地模型） |
| provider 计费 token / 电力费用 | 不适用 / 未测 |

公开模型权重 118,308,126 字节，tokenizer 17,082,913 字节，另有配置文件；下载时间不计入推理耗时。首次 Node 直连与代理路径分别超时、连接重置，改用本机现有代理下的 Python 下载成功；这两次失败没有执行 embedding。第一次成功推理后补充自动读取 runtime 版本并重跑，排名与召回相同，JSON 保留最终运行的实际计时。

## 召回结果

recall 按每例 relevant 目标计算后宏平均；precision@K 的分母固定为 K，空余位置计入分母。JSON 同时保留按实际返回条数计算的 precision。标签只声明 Gold 的应比较目标，低 precision 不表示其它返回对象必然无关。

| 组 | recall@6 | recall@12 | precision@6 | precision@12 | Gold 命中对数@6 |
| --- | ---: | ---: | ---: | ---: | ---: |
| A 当前 substring | 12.5% | 12.5% | 2.08% | 1.04% | 1/9 |
| B 改进词法原型 | 12.5% | 12.5% | 2.08% | 1.04% | 1/9 |
| C 词法 + embedding 原型 | 100% | 100% | 18.75% | 9.38% | 9/9 |
| Semantic 纯 cosine | 100% | 100% | 18.75% | 9.38% | 9/9 |

| Gold case | 应比较对象 | A/B recall@6 | C 中排名 | 预定判断 |
| --- | --- | ---: | --- | --- |
| low-lexical-speed | base/speed | 0 / 0 | 5 | 同义候选 |
| low-lexical-method | base/measure | 0 / 0 | 1 | 相同测量方法 |
| homonym | base/velocity | 1 / 1 | 1 | 位移与路程定义分离 |
| related-not-identical | base/measure | 0 / 0 | 5 | 定义与测量方法分离 |
| relation-conditions | base/equal-time | 0 / 0 | 4 | 等距离与等时间条件分离 |
| relation-direction | base/b-pulls-a | 0 / 0 | 4 | 施力/受力角色互换，分离 |
| whole-parts | base/out、base/back | 0 / 0 | 3、4 | 整体与两个部件分别保留 |
| previous-paraphrase | previous/old-speed | 0 / 0 | 2 | 显式复用既有身份 |

例如 query“行驶全程的距离与历时之比”在 A 中只找到自身，无法召回“总路程除以总时间的平均速率”；C 将后者列为第 5。相同 query 找回旧对象“累计行程和耗时的比值”为第 2。这两个对象对的判断由来源定义支持，不由 cosine 值决定。

## 验证与复现

执行前判断：现状测试用于发现 search/来源/身份约束与已刻画行为不符；Gold 接纳测试发现不存在的引用或无法通过真实 gate 的对象；排序测试发现 exact 丢失、重复占用配额或指标分母错误；类型检查发现 TS 接点不兼容。失败时修正对应实验或断言，不据红测试修改生产合同。

- alignment、teaching-map、teaching-build：3 文件、28 项通过。
- 新增 Gold 基线：1 文件、2 项通过。
- Node 原型排序与指标：1 项通过。
- Core `tsc --noEmit`：通过。
- 真实 provider：28 documents / 8 queries 全部完成，continue=true。

在仓库根目录复现，模型及依赖只落临时目录：

```powershell
npm install --prefix tmp/sr0-runtime --no-audit --no-fund @huggingface/transformers@3.8.1
python evals/semantic-retrieval/download-model.py tmp/sr0-runtime/model 2c4055b12046f11709e9df2c122e59ffbdc2f900
node --import tsx evals/semantic-retrieval/prepare.ts docs/performance/semantic-retrieval-sr0-input.json
node evals/semantic-retrieval/spike.mjs tmp/sr0-runtime tmp/sr0-runtime/model docs/performance/semantic-retrieval-sr0-input.json docs/performance/semantic-retrieval-sr0-result.json
pnpm --filter @understand-book/core exec vitest run test/teaching-object-alignment.test.ts test/teaching-map.test.ts test/teaching-build.test.ts test/semantic-retrieval-baseline.test.ts --testTimeout=30000
node --test evals/semantic-retrieval/ranking.test.mjs
pnpm --filter @understand-book/core typecheck
```

## 已知限制与接续

这是固定自编短材料的真实 provider 探针。对象使用受支持的 T5A 路径，B/C 为离线原型；没有真实长书或语义生成模型闭环。**模型步骤、inspect/read 次数、false merge、false split/missed reuse、生成模型输入 token、整份构建耗时均未测**；决策指标在 JSON 记为 null，不能记为零或通过。脚本给定的身份映射测试只验证 Core 机制。

128 tokens 是当前探针采用的输入限制，不代表已验证长对象投影；批处理仅验证最大 8 条，未测取消、失败恢复、缓存、配置变化、3k/10k 目录与生产 BuildPlan 成本约束。重复 query 仍各自计一次，未实现 cache。

SR0 继续门槛已满足。接续 **SR0a**：累计 UTF-16 原文读取范围并要求 resolve/identity 引用被覆盖；其次 SR1 可据该真实本地模型验证异步接口与投影合同。SR4b 仍需修复完整后状态恢复。SR6 按固定 Gold 与相同模型/预算实施最终 A/B/C 决策闭环，明确分离对象对 false merge=0 仍是发布要求。
