# 理解判断问题的区分力：发布记录

2026-10-14 17:55:48 HKT 已发布至 [understandbook.top](https://understandbook.top/)。当前 release 为 `tutor-question-discrimination-20261014`，Tutor 指导版本 `tb4.v3`，服务 PID 53917、active。

## 变更

用于判断理解的正式测验与普通提问，须让用户完成一个尚未被原文或已交付解释代答的关系判断；题目、期望表现与评分标准共同要求具体运用。定位与回忆可以作铺垫，复制或改写即可完成的正确回答不足以确认关系理解，也不据此判定不理解。只有下一步教学依赖消除不确定性且当前请求允许时，才继续追问。

内置平均速度示例改为同路程、含停留时间的行程比较。规则与范围见 [ADR-0164 §8](../../adr/0164-tutor-mission-and-cognitive-building-blocks.md#8-理解问题的区分力)。

## 构建与验证

现网 `last-hit-default-grants-20261013` 的 Tutor 源码与本次测试前基线一致。候选复制该 release，只替换 `crates/runtime/src/tutor.rs`；前端与其余生产源码沿用现网。服务器在最终 release 目录构建 `server`、`presentation_worker`，耗时 5 分 30 秒，退出码 0；构建限额为 1600 MiB 内存、无 swap、150% CPU、单并行任务。[构建回执](build-receipt.txt)。

本地已通过 29 项相关 Runtime 测试、5 项 Server 示例流程测试与两项契约导出检查；最后补充普通对话要求后，实际请求装配用例再次通过。工具参考文档已重新生成。

切换前进行中或未保存问答为 0。短暂维护期间切换服务和同版演示 worker；确认运行进程指向候选程序，且程序包含 `tb4.v3` 与新出题、按需追问指导。公网首页、登录、11 本授权材料的书库读取通过，authoring / plot / animation / preview 四项能力均为 ready。验证发起模型请求 0 次。[部署回执](deployment.json)。

## 运行与恢复

- 当前 release：`/opt/understand-book/releases/tutor-question-discrimination-20261014`。
- 前一 release：`/opt/understand-book/releases/last-hit-default-grants-20261013`，保留。
- 服务：`understand-book-multi.service`；数据根：`/opt/understand-book/data/multi-reader-to-tb-20261012`；schema 13。
- 原服务、Nginx 与演示配置：`/opt/understand-book/acceptance/tutor-question-discrimination-20261014/previous-configuration`。
- 构建及切换日志：同一 acceptance 目录中的 `build.log`、`deploy.log`；本地副本位于 `tmp/tutor-understanding-questions-20261013`。

新回合使用新版指导，已有活动沿用交付时冻结的评分合同。本次无数据迁移。回退时先确认没有进行中或未保存问答，在短暂维护期间恢复上述三份配置、重载服务并验证前一程序；保留当前数据及上线后的新写入。

## 已知限制

本次验证覆盖指导装配、示例合同、存储流程及生产运行状态，尚未进行真实模型出题效果验收，不能据此声称所有生成问题均已具备区分力或学习效果得到提升。
