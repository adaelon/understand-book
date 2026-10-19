# 教学交接修复发布 — 2026-10-17

**最终候选已于 2026-10-17 11:10:14 HKT 上线。** 运行版本为 `/opt/understand-book/releases/teaching-handoff-20261017`，主进程 PID **120702**，服务 active。实际可执行文件、WorkingDirectory、演示 worker 配置和 Nginx 根均指向该版本，见 [after-state.json](after-state.json)。

## 最终发布内容

本次只发布 `tmp/teaching-handoff-release-20261017/frozen-source-final/` 对应的六个生产源码：runtime 的 tutor.rs、orchestrator.rs、run_context.rs，server 的 teaching.rs、teaching/design.rs，以及 memory 的 teaching.rs。归档为 `source-final.tar.gz`（205,275 bytes），清单见 [release-source-final.json](release-source-final.json)。initial 和 review 两份未接纳快照均未上线。

发布前再次逐字节确认全部六源：当前工作区、final 快照和远端候选三者完全一致，没有做换行规范化，见 [source-final-consistency.json](source-final-consistency.json)。最终 Linux 五程序构建成功，**5 分 53 秒、exit 0**，见 [build-final.log](build-final.log)。Web 保留上一版实际分发内容，两个入口与上一版逐字节一致；schema14 与数据根 `/opt/understand-book/data/multi-reader-to-tb-20261012` 不变。

## 发布前验证

[validation-final.json](validation-final.json) 记录了最终发布依据：

- runtime 全量 629 pass / 8 ignored；server 教学设计 14 pass / 1 ignored；相关 tutor 115 pass / 19 ignored，排除历史 TB2 回放。
- 本地最终构建通过；真实新 helper 与隔离 6.1 sol 对原始失败稿的回归返回 revise，指出需要删除的额外分支，见 [review-result.json](../review-regression/review-result.json)。
- live-03 完成 3 轮、20 次实际请求，support → invite → conclude；首轮无额外 accepted 反事实，第二轮补足条件并留下新判断，用户给出理由，第三轮针对反馈后收束。
- live-04 完成 4 轮、29 次实际请求，support → support → invite → conclude。审阅 0008 退回多余 accepted 反事实，修改后的 0010 接纳，后续 0016、0022、0028 接纳。局部混淆后缩小为 accepted/macro 对照，补齐 theme 入口和证据，再留小丙判断，收到用户理由并针对反馈后自然收束。
- 两组 teacher/learner 均为相互隔离的 gpt-6.1-sol；学习者只读实际交付正文，发言逐字转发。两份独立内容审阅已完成，未发现实质教学内容问题，见 [live-03 审阅](../live-03/independent-content-review.md) 与 [live-04 审阅](../live-04/independent-content-review.md)。

## 线上切换与验收

最终脚本于 **11:09:11 HKT** 开始切换流程，**11:10:14 HKT** 完成。维护前后均确认没有活动或未保存运行，完整备份后切换主程序、演示 worker 与 Nginx；无需打断用户运行。停服至本机验收 **16.52 秒**。见 [deployment.json](deployment.json) 和 [deploy-success.log](deploy-success.log)。

本机及正式域名均验证 reader **11 项**、admin **12 项**授权版本清单与切换前一致，默认书 manifest 和正文可读，四项演示能力 ready；受限书对 reader 仍返回 404。源站首页、阅读器入口与候选精确字节一致，公网项目资源引用清单与资源字节一致，两入口共检查 **9 项引用资源**。当前实际 reader 资源为 `reader-Dn8YVSQY.js` / `reader-WvkzKM4B.css`，与上一版分发一致。发布检查外部模型调用 **0 次**。

## 空间、备份和回退

备份后实际可用空间为 **380,534,784 bytes**。确认旧 `teaching-scaffold-20261017` 没有运行进程后，只清其 `target/release/{deps,build,.fingerprint,incremental}` 编译中间产物，五个回滚程序仍存在且可执行。没有删除运行程序、源码、Web、数据、备份或验收证据。清理后实际可用空间为 **918,482,944 bytes**，见 [post-deployment-cache-cleanup.json](post-deployment-cache-cleanup.json)。本轮较早清理的 `teaching-initiative-20261017` 编译缓存记录见 [cache-cleanup.json](cache-cleanup.json)。

完整数据备份位于 `/opt/understand-book/backups/teaching-handoff-20261017`，大小 **351,425,806 bytes**。原配置位于 `/opt/understand-book/acceptance/teaching-handoff-20261017/previous-configuration-final`。回退版本 `teaching-scaffold-20261017` 的五个正式程序和运行资源保留；必要时沿用维护流程恢复 unit、sandbox 和 Nginx，保留 schema14 的最新数据。完整备份仅用于确有数据损坏时恢复。

所用控制和发布脚本为 `tmp/teaching-handoff-release-20261017/control-final.py`、`deploy-final.py`，后者读取 final 构建与最终验证回执。本目录保存脚本和关键日志；远端证据位于 `/opt/understand-book/acceptance/teaching-handoff-20261017/`。

## 未接纳候选与已知限制

首份候选完整构建成功（10 分 27 秒），但 live-01 首轮提前引入 accepted 反事实并绊住学习者，因此未发布。首次快照和归档保留，日志、清单和原因见 [candidate-initial/](candidate-initial/)。review 候选构建成功（5 分 50 秒），但 live-02 额外反事实仍被审阅放行，因此未发布；证据见 [candidate-review/](candidate-review/)。这些失败证据没有改写为成功记录。

最终语义证据来自模型模拟与近距离即时应用，不能据此声称长期或全面掌握。部署验收确认实际版本、授权数据、能力和资源生效，不额外声称线上真人学习效果已经验证。
