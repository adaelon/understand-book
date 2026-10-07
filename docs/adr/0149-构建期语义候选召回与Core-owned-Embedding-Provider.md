# ADR-0149 构建期语义候选召回与 Core-owned Embedding Provider

状态：**设计已接受，2026-09-30；SR0、SR0a、SR1–SR5 于 2026-10-01 完成；SR6 验收工具已实现，两轮真实 Gold 未满足发布门槛。** [SR0 证据](../performance/semantic-retrieval-baseline-20260930.md)、[SR0a 证据](../performance/semantic-retrieval-sr0a-20261001.md)、[SR1–SR3 证据](../performance/semantic-retrieval-sr1-sr3-20261001.md)、[SR4a 证据](../performance/semantic-retrieval-sr4a-20261001.md)、[SR4b 证据](../performance/semantic-retrieval-sr4b-20261001.md)、[SR5 证据](../performance/semantic-retrieval-sr5-20261001.md)、[SR6 记录](../performance/semantic-retrieval-20260930.md)。默认保持词法路径，长材料验收待执行，SR6 未完成。

变更类型：构建期候选召回增强、Core 依赖边界新增与 alignment 来源读取接纳补全；沿用既有 BuildStage 和 Codex / DeepSeek Harness executor 传输协议。

本 ADR 限定性修订 ADR-0002：仅在构建期语义候选召回范围内重新引入 embedding。承接 ADR-0097 的候选发现职责、ADR-0138 的多 Harness 边界、ADR-0142 的正式对象身份与来源接纳，以及 ADR-0146 的先重算实际输入再判断模型成果复用。

实施合同、切片与测试见[构建期语义候选召回方案](../切片方案-构建期语义候选召回.md)。

## §1 落点：T5A Formal Object 跨块对齐

当前大材料路径为 `routeFormalObjectFragments → fragment tasks → newObjectAlignment → alignment tasks → acceptFormalObjects`。`search` 对 catalog JSON 做空白分词后的 lowercase substring 匹配，每页 6 条，空 query 浏览完整目录。

Core 要求参与决策的非 focus 对象已经 inspect，并校验来源归属、引用、覆盖和拆并；SR0a 已要求 resolve/identity 的引用被本次决策实际读取的原文范围覆盖，成功决策后清空，未结束决策的读取范围随续建恢复。

**决策**：增强 alignment 召回，补齐原文读取接纳。

**否决**：

- 新增知识层或顶层检索 BuildStage：候选发现已有 alignment 接点。
- 按 similarity 自动 merge 或迁移身份：相似度不能证明对象相同。
- 把 inspect 当作原文读取：完整对象记录仍只是候选描述。

**命门**：Embedding 负责召回，Agent 负责语义比较，Core 校验本次引用的读取覆盖并接纳结果；正式身份继续由既有规则分配。

## §2 Core-owned Provider 与执行边界

**决策**：Provider 只在获授权的异步准备中调用。

Core 定义异步 `embed_documents / embed_query` 合同，构建入口注入一个 adapter；合同包含 provider/model/config/dimensions 身份、输入及批量限制、取消信号与可获得的用量。SR0–SR1 先用一个真实 provider 验证合同。

**否决**：

- 将 provider 放入 `BuildExecutionProfileV1` 或 executor：Harness 继续只承担既有语义任务。
- 在普通 snapshot 路由或 candidate writer 中请求 embedding：这些路径还承担状态读取和确定性接纳。
- 新建独立后台循环：复用现有 authorized preparation 与构建控制调用。

**命门**：计划预览、剩余工作查询和普通恢复读取只消费准备结果，缺失时返回待准备；网络调用、重建与用量记录必须位于已确认计划及预算范围内。document/query vectors 使用同一 provider identity。

## §3 当前目录的确定性语义投影

**决策**：按当前目录投影，缓存未变文本的向量。

投影包含 `kind/meaning/aliases/conditions`，以及 relation 的角色和 composite 的组件摘要。摘要只展开一跳；当前对象按 qualified key、previous 对象按正式引用解析，字段顺序与文本上限由 projection version 固定。

**否决**：

- 直接 embedding 整个对象 JSON：临时 key、身份、路径、revision 和构建状态会污染召回。
- 只在 fragments 接纳后建一次索引：resolve 会改变含义、参与者和组件引用。
- 按相同投影合并对象：相同文本只允许共享向量缓存。

**命门**：resolve 后重投影当前目录，参与者或组件摘要变化也更新对应投影；按 `provider identity + projection version + content digest` 跳过未变 embedding。`source_bindings` 继续服务 read/accept。

## §4 Hybrid retrieval 与搜索合同

**决策**：先过滤有效对象，再融合并稳定分页。

非空 query 及自动 focus 召回先移除 focus 自身和已退出目录的旧 key，以当前 canonical key 去重，再做 exact/alias、lexical 和 semantic Top-K 融合。仍在目录中的已 resolve 对象继续可检索。空 query 保留完整目录浏览能力，不调用 embedding。

**否决**：

- Top-K 截断后才过滤、去重：无效条目会挤掉实际候选。
- similarity 作为同一对象概率：返回 `match_reasons`，分数只作诊断。
- 自动 focus 页覆盖显式 search：Agent 的 query/offset 决定当前搜索页。

**命门**：同一目录快照、query 和 policy 的融合序列固定后再按 offset 分页；inspect/read 保持显式搜索，resolve/identity 结束本次决策后才切换下一 focus。配额和截断语义由 SR3 固定。

## §5 模型 action 复用与 Core 状态恢复

**决策**：复用模型 action，按当前目录重放效果。

起始实现保存并恢复整份 `ObjectAlignmentWork` 后状态。SR4b 已改为保留可重新应用的 action；先准备实际检索页并渲染输入，再判断模型 action 是否可复用。search action 接纳只记录请求，结果由 Core 准备并进入下一步输入。

**否决**：

- 从旧 search 后状态取得候选页再判断 freshness：旧结果会绕过重检索。
- 仅凭 bounded input 相同恢复完整目录：未展示对象可能已经变化。
- provider 或诊断字段变化就重做模型任务：模型复用依据仍是实际输入、prompt 与语义合同。

**命门**：action 在当前 ledger 上重新应用并通过 Core gate；只有 Core 依赖也相同时才恢复整份后状态或既有 finish 结果。正式对象分配保持幂等。向量缓存可删除，已接纳 action 和正式身份不随之删除。

## §6 BuildPlan、失败与成本

**决策**：计划绑定检索配置、数据边界与执行预算。

`lexical_only` 保留纯本地词法能力；`semantic_required` 在需要检索准备但 provider 不可用时保持 blocked/incomplete。配置包含 local/remote、provider 身份与投影文本的数据边界；展示记录数估计及可获得的费用信息，执行用量包括文档重算、query 和重试。

**否决**：

- 把配置和费用只做成展示字段：实际调用必须受本轮确认范围及预算约束。
- 将 provider 失败当作空召回或无效模型候选：失败原因与恢复责任不同。
- 将收益尚未证实的 embedding 升为默认必需依赖：先通过 SR0/SR6。

**命门**：provider 失败保留已接纳 action；恢复仅补所缺准备，不递增语义生成 attempt。配置漂移或预算超出沿现有计划恢复边界处理。SR4a 的执行约束与 SR5 的真实 adapter/确认面共同完成后才能启用生产调用。

## §7 首版范围与 Pass2 后续准入

**决策**：SR0–SR6 只增强 T5A 对象对齐。

**否决**：

- 同时接入 Pass2：候选召回与关系判断的变化无法分别归因。
- 扩展读时或跨书向量检索：超出本次候选对齐需求。
- 首版引入 vector database：先验证单书规模的暴力 cosine。

**命门**：Pass2 另开切片，前提是相对改进词法基线的真实召回增益、false merge 不恶化、provider/cache/replay/cost 边界验收通过，以及双端原文 evidence packet 满足自身合同。

**展开**：BookStructure 作为后续消费者的设计与独立验收见 [ADR-0151](0151-book-structure-global-outline-and-semantic-retrieval.md)，复用基础能力并采用自己的候选投影与结构语义。

## §8 验收与继续实施条件

**决策**：以真实召回增益和身份质量决定启用。

SR0 固定 gold、query、K 和错误容忍边界，并完成一个真实 provider 的小规模验证。SR6 对比当前 substring、改进 exact+lexical、同一词法实现加 embedding；比较条件包括相同来源读取约束、模型、prompt 和决策预算。

**否决**：

- 用 fake provider 证明语义收益：它只验证管线与排序合同。
- 只与旧 substring 比较：词法排序改进可能解释全部收益。
- 运行后再选择容忍范围：发布门槛必须在看结果之前确定。

**命门**：记录 recall/precision@K、false merge、false split/missed reuse、模型步骤、原文读取、输入 tokens、embedding 用量与总耗时；同时验收显式 query 刷新、未展示目录变化、来源读取覆盖和失败续建。收益不足时保持 `lexical_only`。
