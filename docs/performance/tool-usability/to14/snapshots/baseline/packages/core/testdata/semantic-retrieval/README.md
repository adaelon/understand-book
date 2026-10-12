# SR0 fixed Gold

2026-10-01 在首次 provider 排名之前固定 `gold.ts`。来源是仓库 `test/fixtures/teaching-source.ts` 的完整自编短材料，增加自编牵引方向段和物理量定义段；没有用户书籍正文。两个 fragment 使用现有 T5A proposal 形状，previous 由真实 `acceptFormalObjects` 生成。固定候选顺序、query、对象对、理由和 K=6/12；每例的 `compare` 表示应交给 Agent 比较，`separate` 表示比较后必须分离，二者可以重叠。`same` 是应复用的身份/同义标签。

主指标为宏平均 recall@6，同时报告 @12、precision 和逐例排名。SR0 继续条件为当前 substring 有应比较对象漏召回且真实 provider 在 @6 找回。发布要求 C 高于 B、exact/alias 保留、明确分离对 false merge=0、总体身份错误不高于 B，并保持同一模型、prompt、读取门禁和决策预算。检索命中不等于身份接纳。

探针限定本地 CPU、至多 40 documents / 10 queries、batch=8、每输入含特殊 token 不超过 128；超长报错，不静默截断。一次 document 批处理与一次 query 批处理，无自动重试；联网只下载公开模型，推理文本不出本机。模型选择为 `Xenova/paraphrase-multilingual-MiniLM-L12-v2` q8，mean pooling、L2 normalization，无 document/query 前缀。模型 revision 在结果中记录。

执行脚本与结果见 `docs/performance/semantic-retrieval-baseline-20260930.md`。SR0 只刻画现状与离线探索；SR1–SR5 才建立生产合同与集成。真实 Agent 步数、read/inspect、false merge、missed reuse 未运行时必须记为未测。
