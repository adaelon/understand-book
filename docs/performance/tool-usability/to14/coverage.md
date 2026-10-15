# TO14 场景与证据

这份映射将真实模型观察与已有确定性证据分开。TO1–TO13 的原通过结果在相关输入不变时沿用；本轮新跑的编译、接线、判定测试和真实任务见本目录。

| TO14 场景 | 原确定性证据 | 本轮真实模型范围 |
|---|---|---|
| 已有正文选区的局部解释 | [TO5 explain.json](../to5/explain.json)：禁止写操作；[TO3](../to3/README.md)：选区／来源接纳 | explain-only；每版 1 次 |
| 字面首次／之前出现 | [TO2](../to2/README.md)、[TO8](../to8/README.md)、[TO10](../to10/README.md)：actual occurrence 次序、scope、cursor | rainwater 自选检索；未单独抽取“之前出现”的真模样本 |
| 语义歧义与两处比较 | [TO8](../to8/README.md)：选第二概念、两侧实际读取；[TO2](../to2/README.md)：query obligations | 沿用确定性证据 |
| 查询／综合证据不足 | [TO11](../to11/README.md)：support 判断和限制保留；省略引用不能成为正文证据 | 沿用确定性证据 |
| Context 裁剪／结构不可用 | [TO9](../to9/README.md)：12→10、k=0、空集与真实缺结构；[TO12](../to12/README.md)：真实 MCP schema | 沿用确定性证据 |
| 超预算正文／结构化输出 | [TO10](../to10/README.md)：跨叶和单叶拼接、原查询续页；[TO11](../to11/README.md)：300 个位置和实际补读；[TO12](../to12/README.md)：132 个交付样本 | rainwater 每版 3 次，跨两页的 UTF-16 来源与实际账本 |
| Tutor 接续／提示／理解更新 | [TO3](../to3/README.md)：状态合同、真实 TeachingMove、帮助／表现／评估／暂停；[TO13](../to13/README.md)：14 可用与3不可用状态 | tutor-continue 每版 3 次；数学条件另核对；不外推提示和理解更新收益 |
| 修改已有演示 | [TO4 chain.json](../to4/chain.json)：真实浏览器三环境、候选换代、修改对象、来源资产和交付回执 | 沿用确定性证据 |
| 保存笔记／纯解释 | [TO5](../to5/README.md)：真实重开持久化与回执；Goal／Memory 字段和副作用 | save-note / explain-only 各每版1次；[本次选区包装缺陷](preflight-findings.md)照常占失败分母 |
| Executor 正常／恢复 | [TO6](../to6/README.md)：V3/V4 版本、真实 session、租约恢复、旧 sink 拒绝与 stdio 提交 | 沿用确定性证据；没有把脚本调用写成真实模型构建 |

每项契约改动已有对应正确性证据。本轮真实实验覆盖两个材料题族、四种任务，未覆盖所有工具族的真实模型选用；其余工具族的性能收益保持未测。TO5 的 EXISTING 笔记完成判断、TC2 的数学条件、真人学习效果及生产发布分别沿原记录。
