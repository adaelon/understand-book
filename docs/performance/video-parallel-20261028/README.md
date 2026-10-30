# 视频独立语义批次并行（S2a）

日期：2026-10-28。翻译与 discourse 使用两个执行槽；每个槽连续处理原有有界批次，完成后领取下一个待处理组。graph 依赖先前接纳的概念，视觉依赖已接纳画面复用，二者继续顺序执行。

## 实现与状态所有权

`runVideoTranslation` 和 `runVideoSemanticSeries` 的 discourse 分支创建一个 `video-executor-coordinator.ts` 控制端。原四工具 MCP session、候选接纳和工作区写入都运行在父进程；Codex 的 MCP 子进程通过仅绑定 `127.0.0.1` 的临时 HTTP 入口转发 JSON-RPC，不再独立读改写共享 plan/catalog。所有会话写入经过同一个队列。

翻译的外部快照发布也进入该队列，直到异步复制结束才允许下一次候选提交，避免发布读到跨时刻的文件集合。discourse 在启动前冻结所有待处理输入，再并行独立组；graph 仍在前一组接纳后准备下一组。

执行槽上限为 2，可通过内部 `VideoExecutorControl.max_parallel=1` 做同路径对照。任务数量、输入与候选上限、模型配置、预算和独立回执保持原约束。首个失败或取消停止补位，终止其他活动进程，并等待所有已启动执行的回执处理完成后返回。已接纳成果保留，恢复只执行缺失任务。

## 受控耗时对照

夹具为 360 段转写、3 个发布组、12 个候选；每个候选模拟 120 ms 模型等待。使用真实父进程 MCP 会话、HTTP 请求、候选校验、原子写入和发布，替换 Codex 模型调用。按 1、2 个槽交替运行各 3 轮。

| 执行槽 | 三轮耗时（ms） | 中位数（ms） | 范围（ms） |
| --- | --- | --- | --- |
| 1 | 3180.59、3366.48、3074.25 | 3180.59 | 3074.25–3366.48 |
| 2 | 2457.53、2605.64、2571.66 | 2571.66 | 2457.53–2605.64 |

该场景中位耗时下降 19.1%，两组观测范围未重叠。两种配置生成相同译文、发布覆盖和任务尝试次数，每项均接纳一次。原始结果：[translation-timing.json](translation-timing.json)。

这是新控制端的一槽/两槽机制比较；不包含真实 Codex 启动、真实模型质量或整片媒体处理，不是旧版到新版的整片加速比例。修改前源码保存在 `tmp/video-parallel-20261028/baseline/`；测试输入、调用和断言位于 `packages/core/test/video-translation-parallel.test.ts`。

## 正确性证据

- 翻译 9 项：并行无丢失、异步快照一致、失败停止补位、取消、按缺失项恢复、完成后不再调用模型，以及原接纳合同。[原始结果](translation-tests.json)
- discourse 与 graph 14 项：两个独立 discourse 组同时执行、单槽对照、失败及取消后停止补位、输入身份保持、graph 顺序依赖与既有候选校验。命令：`pnpm --filter @understand-book/core exec vitest run test/video-semantic-parallel.test.ts src/video-semantic-series.test.ts test/video-discourse.test.ts`；14/14 通过。
- 视频运营/费用 11 项：使用真实 Node 子进程，失败后另一槽取消并完成终态回执，第三组未启动；并行预算拒绝不启动子进程；既有费用与运营恢复路径继续通过。[原始结果](runtime-tests.json)
- 源码 stdio 转发 2 项：经 `automatic-build.ts` 的两个真实命令入口，完成初始化、通知、发现工具和候选接纳；子进程没有可读取的工作区或 jobs 文件，父进程连续接纳两项共 40 段，关闭 stdin 后 code 0、stderr 空。测试：`test/video-executor-coordinator.test.ts`。
- 最终编译引擎共 3 项通过：上述两个视频入口和 S1a 的书籍三单元连续 stdio 提交。通过 `UNDERSTAND_BOOK_EXECUTOR_TEST_EXE` 直接执行最终安装目录内的 Windows 程序。[打包协议回执](packaged-protocol-tests.json)、[内嵌常驻 Whisper 验证](packaged-whisper.json)。
- 已完成翻译的恢复只需要发布，不启动控制端；该分支显式顺序发布。补充断言先发现两个快照复制重叠，修复后复制峰值为 1，模型调用仍为 0。[修复验证](resume-publication-fixed.json)

所有测试使用 E 盘临时目录，未调用付费模型。真实视频模型耗时、服务限流和并发峰值内存仍需在有预算的代表性视频任务中测量。
