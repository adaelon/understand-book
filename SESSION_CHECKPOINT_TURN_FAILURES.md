# 网站两次 Tutor 回合失败修复（2026-10-11）

用户已批准具体方案，并要求继续修复。变更类型：纯技术；沿用现有 TurnFrozen、Dynamic、CompactionCheckpoint、ActiveContextBudget 定义。

- [x] Tutor 完整学习上下文每个请求仅出现一次；动态暂停仍生效。
- [x] 压缩候选使用实际请求同口径比较；无收益且原请求可容纳则继续；先验排除必然无收益；同一历史边界不重复生成；仅接受后持久化。
- [x] DeepSeek Flash 普通 Agent 输出上限和预留一致为 32768；原输入压缩阈值不变；演示页和压缩专用额度不变。
- [x] 定向回归与原两次请求隔离验证，记录交付状态。

证据位于 tmp/turn-failure-diagnosis-20261011。第一次 length（8000 输出全部为推理）；第二次有效摘要被不同投影口径判为 14593 > 3783。原始线上历史不得改写。

工作树已有大量他人未提交修改；orchestrator.rs 原有修改仅诊断注释，保留。编译 TEMP/TMP 放 E 盘，C 盘空间不足。

本地验证：Runtime 491 通过、3 忽略；Server service_limits 6 通过；Tutor 36 通过、5 忽略；Server compaction 4 通过。日志 tmp/turn-failure-repair-20261011。
候选 /opt/understand-book/releases/tutor-turn-fix-20261011 已完成 Linux release 编译；只覆盖本次 Runtime 文件，其余来自当前线上版本。隔离根 /opt/understand-book/data/tutor-turn-fix-replay-20261011，使用 SQLite 在线备份复制学习状态，新聊天复验，LangSmith project understand-book-turn-fix-20261011。复验脚本 /tmp/replay-tutor-fix.py，进度在隔离根 replay-progress.log。已于 09:34 切换生产服务，切换 4.4 秒；两个真实回合均 completed 且有完整交付。公网浏览器首页/登录页 200，未登录 API 401。

交付记录：docs/修复记录-Tutor回合失败-20261011.md。已知问题：原有 REVIEW_INPUT_MISSING；第二条真实复验耗时 535.2 秒，LangSmith 根 span 尾部缺失但 API/持久回合为完成。
