# SESSION_CHECKPOINT — 调用成本与 DeepSeek 适配 — 2026-09-28

## 新鲜度自检
- 写入时最新 commit：`c2ff3e1 feat: ship reader, agent, viewport, and observability updates`。
- 本任务成果纳入 2026-09-29 集成提交；当前提交身份见主 SESSION_CHECKPOINT.md 与 git log。

## 当前在做什么
已完成用户确认的四项源码修正：DeepSeek 模型/用量、消息续接持久化、逐采样状态尾部追加、压缩独立预算。

## 下一步（可直接接手）
1. 阅读 `docs/计划-调用成本与DeepSeek适配.md` 的验证结果与已知问题。
2. 原 EX11 Goal 分类测试失败已解决；2026-09-29 集成验证 Runtime 402 项通过、3 项忽略。
3. 如准备实际费用对照，使用更新后的应用重跑同一阅读任务，记录 Provider 输入、缓存命中、输出与思考子项，并比较交付结果；本轮未发起付费调用。

## 未提交 / 未完成
- 本次源码及文档已纳入集成提交；本地原始运行产物按 docs/performance/ARTIFACTS.md 保留。
- Runtime 399 通过、1 已有失败、3 忽略；Server 定向 2 + 历史 6 通过；Observability 19 通过；Web/Observability 类型检查通过。
- 未重新打包或重启正在运行的桌面应用；实际缓存命中、费用与规划质量对照尚未测量。

## 冷启动读序
1. `docs/计划-调用成本与DeepSeek适配.md` — 范围、实现规则、验证与限制。
2. `docs/架构.md` 的“调用成本与模型续接” — 请求和私有状态数据流。
3. `docs/代码链路.md` 的两条“2026-09-28 调用成本” — 文件、符号与测试索引。
4. `CONTEXT.md` 的“Provider 续接状态与采样状态快照” — 术语边界。

## 本会话决策摘要
- 动态状态每次采样完整提供，旧快照不改写，最后快照权威；仅在当前运行内保留。
- 私有续接绑定原模型与 assistant 消息；旧字段缺失时保留全文为历史上下文，不伪造推理。
- DeepSeek Flash 档案识别 1M 容量，应用压缩阈值保持 96K；压缩上限 16,384，DeepSeek 使用 low 思考强度。
