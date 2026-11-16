# 后台分析修复续接点

- 用户要求修好后台学习分析失败并降低费用；此前已授权真实模型验证和上线，本次沿用，继续到部署完成。
- 主要实现：起步零模型、确定性去重输入、正确 180 秒适配器、低推理首次分析＋最多一次关闭推理纠正、内联／无新读取来源恢复、音乐来源预校验、遗漏细节不判困难。
- 本次七文件生产覆盖清单 `tmp/coach-cost-20261105/source-files.json`；基线六个已有文件与线上上一版相同，原工作树大量其他改动保持。
- Runtime 1060 通过18忽略；最终教练34通过1忽略；来源恢复6通过；Server后台30通过；证据目录 `tmp/coach-cost-20261105/`。
- 原六后台任务9调用3失败¥0.78163272；`live-2` 六输入6调用全部校验通过¥0.30823308，节省60.57%。
- `flow-live` 两轮真实新对话，三个后台任务全ready，第二轮观察recorded；后台2调用¥0.04160128，总8调用¥0.14972152。服务已停。
- `quality-live` 使用该第二轮原输入验证最后提示修改，1调用¥0.02936216，困难为空。无需再次全量付费复测。
- 本次所有付费实验21调用¥0.71112792；报告 `docs/performance/coach-cost-20261105/README.md`，统计 `measurements.json`。别把本次后台改善当全会话严格A/B。
- 已于2026-11-05 23:21:59 HKT发布 `coach-cost-20261105`，服务PID166920，维护21.552秒。数据 `/opt/understand-book/data/production-20261031`，schema22，无迁移。
- 正式目录 `/opt/understand-book/releases/coach-cost-20261105`，远端日志与脚本 `/opt/understand-book/acceptance/coach-cost-20261105`，备份 `/opt/understand-book/backups/coach-cost-20261105`。上一版context-cache-20261105保留。
- Linux release server编译13m10s成功，辅助五程序和网页沿用上一版，约1.04GiB磁盘可用。
- 状态：`python tmp/context-cache-deploy-20261105/remote.py python tmp/coach-cost-20261105/build_status.py`。这个脚本在Linux执行，不能直接本机运行。
- 切换前全部用户前台、模型计费和后台任务均为空；普通/管理员目录、历史、额度、工作区、画板与制作能力均通过。build.json和code-deployment.json已取得，保存在报告deployment子目录。不要重跑deploy.py。
- 线上七份源码与本地一致（仅归一化换行），source-match.json已保存。公网Windows curl实际GET：HTML200、ReaderJS200、匿名API401；初次urllib403也保留在public-health.json。
- 实现、真实模型验证、上线与报告均完成。没有运行中的实验或待发布步骤，历史失败未批量重试。
- SSH用现有remote.py帮助器（FileZilla凭据＋已固定主机密钥），不要输出凭据。网络偶有EOF10054，确认实际进程状态后仅重试未完成操作。
- 历史失败只读检查，没有批量重跑。报告不公开原用户私密输入或原理解记录。
