# SR0a Alignment 原文读取接纳 — 2026-10-01

状态：**已完成，下一步 SR1。** 合同：[ADR-0149](../adr/0149-构建期语义候选召回与Core-owned-Embedding-Provider.md)、[SR0a 方案](../切片方案-构建期语义候选召回.md#sr0a--alignment-原文读取接纳)。

## 实现

`ObjectAlignmentWork` 升为 `formal_object_alignment.v2`，以 `source_id + source_revision + lid + [start,end)` 保存本次决策实际 `read` 返回的 UTF-16 范围。同一来源段落的重叠、相邻范围合并；存在间隙时分别保留。模型输入展示累计 `read_ranges`，正文仍只保留最近一次有界 `reading`。

`resolve.object.source_bindings` 和 `identity.source_bindings` 在原有来源、inspect、引用与身份校验之后检查读取覆盖。带 `range_utf16` 时验证该范围，否则要求整段读完。inspect、来源预览和候选指针不计读取；失败不改变原 work。成功 resolve/identity 清空读取范围、近期正文、inspect/search 与本轮步数。

接纳产物保存范围，冻结任务与续建路由恢复同一范围；重复读取不扩张范围，但仍消耗一次动作。32 步无进展预算沿用现有路由，耗尽保持 incomplete，已接纳范围留在 artifact 中。

正式对象策略及 schema 合同升为 `formal_objects.v2`，generation 升为 `formal_objects.full.v2`；正式对象成品仍为 `formal_objects.v1`。正式 prompt 声明整段/范围读取、决策清空及预算合同，并更新既有 prompt 绑定。新 generation 避免相同输入撞上旧不可变任务/产物路径，旧产物保留；旧 alignment 后状态不用于新接纳，旧冻结 alignment 提交也拒绝推进。

## 验证

先将 SR0 的三个漏洞刻画改为拒绝断言：inspect 后无 read 合并、无 read identity、读一个字符后整段引用。旧实现三项均因“未抛错”失败，再实施门禁。

最终执行：

```powershell
pnpm --filter @understand-book/core exec vitest run test/teaching-object-alignment.test.ts test/teaching-map.test.ts test/teaching-build.test.ts test/teaching-object-fragments.test.ts test/semantic-retrieval-baseline.test.ts --testTimeout=30000
pnpm --filter @understand-book/core typecheck
```

**5 文件、41 项通过，Core 类型检查通过。** 检测对象与失败处置在运行前明确：门禁漏接纳则修覆盖逻辑；身份/来源/覆盖回归则保留原约束并修接点；续建丢范围或旧合同复用则修冻结/接纳绑定；类型错误修调用与联合类型收窄。

- Alignment 15 项：无 read 拒绝；整段与片段引用；乱序、重叠、重复、相邻读取；间隙拒绝；来源 ID/revision 隔离；近期正文替换后累计范围保留；失败不修改状态；resolve/identity 清空及下一决策重新读取；旧版本拒绝。既有 inspect、身份延续与拆并仍通过。
- Teaching build 11 项：真实 input/writer/route 保存并合并两次读取；resolve 后续建为空范围；下一候选复用上轮范围被拒绝；32 步耗尽保持未完成、无 readiness；新旧 generation 隔离且旧文件保留；旧 alignment 后状态拒绝接纳。Pass2 enabled/disabled、小材料发布、定向修复仍通过。
- Fragments 7 项：长段按每次不超过 2,000 UTF-16 字符读齐后合并、组装、来源审阅及发布；恢复后的正式身份稳定。
- Teaching map 6 项、固定 Gold 2 项：原身份、拆并、来源与 coverage gate 保持；Gold 仅补读取动作，来源、query、对象对和 K 未变。

首轮回归 39 项通过；类型检查发现 Payload 联合类型并非都有 version，已增加明确收窄。补齐旧 generation 与预算测试后得到以上最终结果。

## 已知限制

SR4b 的完整后状态恢复缺口仍保留：未展示 ledger 变化可保持 bounded input 不变，现有恢复仍使用完整后状态；对应 SR0 刻画测试保留。SR0 历史探针与原始记录不改写。

本轮为确定性接纳与续建验证，未调用真实语义模型或 embedding provider，未重做召回排名；真实身份质量及长材料模型闭环仍由 SR6/T5A 验收。本轮未打包、安装或提交 Git，现有安装包不代表当前源码。
