# BSR5 正式调度、恢复与发布

日期：2026-10-01。BSR5 已完成；合同见 [ADR-0151](../adr/0151-book-structure-global-outline-and-semantic-retrieval.md)。

## 实现

技术书的既有 unit/fragment/reduce 成果进入当前候选目录，随后执行全书框架、章节引用选择、主题规划、各主题动作和一次目录收敛。五类新工作单元使用现有 V4 descriptor、policy/generation、冻结输入、executor、writer 与发布机制。paper 继续原结构路线。局部来源覆盖与新整理进度分别记录。

`book-structure-organization.ts` 负责路由及动作重放，`book-structure-generation.ts` 负责接纳。冻结任务包含模型交付正文和 Core 当前上下文；实际模型输入只有渲染正文和引用范围。回执保存已接受动作。恢复先取得实际页面和交付输入，再将动作应用于当前候选目录；失败的当前接纳检查产生替代任务，历史回执保留。写入及 executor 检查沿用任务的检索选择，避免保存的旧语义配置影响新词法任务。

BuildPlan 可明确选择结构投影、正式对象投影或组合范围。旧 formal-only 确认不能授权结构语义请求。两个 host 都把实际待准备阶段交给同一获授权异步入口；普通观察、writer 和剩余工作估计不调用 Provider。准备和缺项计算使用消费者自身 projection identity/cache namespace，取消和 Provider 失败保留既有动作、文档向量和计量。

完整章节重点由引用物化，宏观路线只引用其中子集；主题可补选内容并保存有双端依据的依赖。新成果全部接纳后发布三清单，重复提交不增加贡献，重复关闭及搬移恢复保持成品字节一致。尚未完成或 unresolved 的目录不覆盖旧发布。

质量报告分别给出叶节点覆盖、候选数、框架、章节、主题、收敛与发布状态。发布状态属于观察值，独立于跨发布前后保持稳定的质量证据 digest。

## 预算与执行合同

组织任务输入上限估算 20,000 tokens，context floor 32,768；输出预留按框架／章节／主题分别为 5,000／3,500／6,500。实际候选提交额度由现有 transport contract 计算，不提升两个 carrier 的请求上限。章节和单主题各最多 128 个动作，超限或未解决收敛问题保留成果并停止自动发布。三个 prompt 已注册、打包并补齐 executor 交付说明。

## 验证

以下验证使用临时合成材料、确定性响应及 fake embedding；没有执行新的真实模型实验。

- `book-structure-append-production.test.ts`：五类任务经真实 opaque executor 输入、生成、提交与重复回执路径；三章六个详细重点、每章一个宏观路线点、一条主题；旧成品保留、质量通过、原子发布、重复关闭与搬移重建。
- `book-structure-production-recovery.test.ts`：Codex/DSH 选择下的缺准备、取消、Provider 失败与续建；模式切换；旧确认范围拒绝；26 候选中的未展示项变更仍保留，独立章节成果不丢；未完整浏览不能选择、来源变更不能由旧 writer 接纳；未解决目录保持 incomplete。
- `automatic-build-retrieval-driver.test.ts`、`automatic-build-retrieval.test.ts`：两个 host 准备实际阶段，两类消费者独立缓存、重复准备零新增调用，确认和预算沿用现有入口。
- 既有回归涵盖章节／主题／检索／物化合同、BSR0 真实反例、opaque 输入路由、质量、关闭、paper 投影、可选 Pass2 和后到 Pass2 的失效。
- Rust `read-tools::tests::bsr5_chapter_projection_keeps_detailed_stops_outside_macro_route`：真实 `Book::structure` 返回同章详细点，`guide_path` 只返回宏观路线点；指定 LID 仍能取得详细点。

Core 定向回归共 19 个文件、114 项通过，Core typecheck 通过；上述 Rust 测试 1 项通过。测试中发现并修复了 prompt 交付说明、候选传输额度、模式切换上下文和发布观察值影响关闭凭据的接入问题。

## 已知限制

前段发现仍沿用既有 unit/fragment/reduce，框架目前由这些已接纳概述形成；BSR6 才替换为框架先行的完整新书发现。超大输入与候选响应仍受正式执行合同约束；此片没有改变传输协议。

本片证明正式接入与恢复行为，真实内容质量沿用 BSR2/4 的局部证据；全书新发现、内容对照和端到端成本留给 BSR6/7。默认仍为 `lexical_only`。没有安装插件、发布原书或复用旧实验授权启动新实验。
