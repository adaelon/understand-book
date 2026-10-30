# 零基础默认与教学对话接续上线记录

2026-10-28 17:43:29 HKT 已发布到 [Understand Book](https://understandbook.top/)。当前 release 为 `/opt/understand-book/releases/beginner-dialogue-20261028`，服务 PID 400133，schema 21。维护窗口 9.63 秒，无数据迁移。

## 发布内容

主 Agent、Tutor、教学教练、表达教练、解释审核与画像提取共同采用当前知识点的零基础默认；相关真实表达和用户明确要求决定当前帮助深度。新规则也覆盖旧聊天恢复和普通问答。

回答完整后，在有价值的停靠点邀请用户表达理解，或用具体变化、观察和预测打开下一知识点。已经说清当前关系时优先探索下一条未解关系；用户说不知道、继续、直接讲或改问新问题时，接续其实际请求。一次保留一个主要回应任务，明确只要答案时自然结束。

以线上 `rust-board-teaching-20261028` 为基线，三方合并 13 个源码及测试文件；保留线上空间笔记说明、画板规划、账号与注册行为。Tutor 指导为 `teaching-design.v21`，共同学习起点与讲解职责为 v2，解释审核为 `explanation-delivery.v6`，呈现指导为 `coach-handoff.v2`。范围与实际线上差量见 [release-source.json](release-source.json)、[release.patch](release.patch) 和 [merge-report.json](merge-report.json)。

## 验证

- 实现阶段最终 92 项不同相关自动回归通过，覆盖共同请求装配、Tutor 接续、教练及纠错、邀请交付、公开工具示例、解释审核和请求预算。一项需指定录制目录的回放保持忽略。见 [local-regression-summary.json](local-regression-summary.json)。
- 最终候选完成 Linux release 构建，六个服务相关程序成功；临时交换空间移除，系统参数恢复。上传的 13 个源文件与候选内容相同。见 [build-completion.json](build-completion.json)。
- 最终服务程序包含 7 个本轮核心教学内容与版本标记；全部 371 个前端文件与上一线上版本逐字节相同。见 [release-verification.json](release-verification.json)。
- 使用最终 Linux 程序和隔离数据副本验证 3 个已有账号、书库、正文、聊天、空间笔记及 8 张旧画板；副本中的聊天、画板、额度周期保持，模型调用为 0。见 [integration-qa.json](integration-qa.json)。
- 切换后通过公网验证首页、阅读器、管理入口、9 份引用资源及画板预览资源；读者和管理员登录、正文、历史、额度与制作能力可用。见 [public-verification.json](public-verification.json)。

## 数据与恢复点

切换时没有进行中或后台复核任务。50 份历史聊天、85 份画板文件逐字节保持，13 条额度周期记录保持，schema 仍为 21。切换回执见 [deployment.json](deployment.json)。

上一版 `/opt/understand-book/releases/rust-board-teaching-20261028` 保留。配置和可变数据备份位于 `/opt/understand-book/backups/beginner-dialogue-20261028`。开放公网前失败会恢复旧服务与配置；恢复不倒退生产数据，开放后的恢复需考虑期间产生的新记录。

## 已知问题

本次部署验证不调用模型，不将服务健康检查等同于新的教学质量评测。实现阶段的 23 次零基础样例与 10 次对话样例、已经观察到的事实措辞错误、解释偏长及未完整覆盖的 Reader 多轮行为，分别保留于[零基础实施记录](../../../实施记录-零基础教学默认.md)和[对话接续实施记录](../../../实施记录-教学对话接续.md)。这些限制不因上线而消失。
