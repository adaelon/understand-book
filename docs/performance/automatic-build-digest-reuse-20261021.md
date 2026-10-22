# 2026-10-21 计划摘要复用与重放字节比较

未变化的画像任务目录复用既有计划摘要，预检查继续重新计算待办、预算和可用槽位。已提交候选的重放直接比较已读取字节，去掉两次临时 SHA-256 运算。

## 实现

- `automatic-build-executor-session.ts:assertCandidateReplayMatches` 使用 `Buffer.equals`；保留原 UTF-8/JSON 读取、BOM 归一化和冲突行为。
- `automatic-build-incremental.ts:saveCatalog/readCatalog` 在派生目录头保存、恢复摘要，与任务描述一同原子写入。摘要缺失时从目录中的既有任务描述计算一次。
- `build-orchestrator.ts:refreshAutomaticBuildProfileSidecar` 直接比较重路由前后的任务描述，实际变化才清除摘要。普通任务完成、结果删除、尚未改变任务描述的分片完成都复用原值。
- `automatic-build.ts:preflightForAction` 将路由提供的摘要传给 `buildAutomaticBuildPreflight`；未提供摘要的调用保持原计算路径。来源、策略、发布变化沿用现有目录失效判断。

原摘要算法、规范化字节、计划及任务身份保持一致。缓存提示只存在于派生目录头；旧引擎重写目录时会丢弃它，避免把过期提示带入后续恢复。

## 验证

旧实现上两项回归失败：相同候选重放仍做两次哈希，未变化的 Driver 调用仍计算两次计划摘要。

修复后相关 **28 项测试**分批通过：增量路由 4 项、候选重放及结构化提交 2 项、预算 12 项、派发 6 项、保留失败观察队列的恢复 4 项。覆盖内容包括：

- 同一 invocation 的后续 Driver 调用不再计算计划摘要；预算和可用槽位仍按当前状态计算。
- 普通完成、删除结果、分片尚未形成归约任务时复用摘要；归约任务出现后摘要更新，且与完整重新计算一致。
- 旧目录缺少摘要时只补齐派生数据；目录丢失可重建；持久策略变化仍返回原恢复边界。
- 相同候选和 BOM 归一化后的候选继续幂等接收；内容或字节空白变化仍拒绝，不新增尝试。

原始记录：[红测](../../tmp/digest-reuse-20261021/red-regression.json)、[最终增量回归](../../tmp/digest-reuse-20261021/incremental-catalog-final.json)、[候选重放](../../tmp/digest-reuse-20261021/regression.json)、[预算、派发与队列恢复](../../tmp/digest-reuse-20261021/budget-dispatch-recovery.json)。Core TypeScript 检查通过。

编译版在独立合成工作区完成跨进程验证：同一 invocation 的后续补位复用目录，旧目录补齐摘要后字节与原目录一致，新增结果被消费且未变化的目录不重写。摘要与完整计算一致，派发遵守 8 路上限，模型调用为 0。见[编译版验证记录](../../tmp/digest-reuse-20261021/compiled-smoke-result.json)。

## 性能对比

运行环境为 Windows x64、Bun 1.3.14。预检查使用真实路由生成的 V3 画像描述扩展出的 6265 个合成任务，4769 个待办，并发配置 8；单个描述约 2247 字节。对照使用未提供缓存摘要的原分支，候选使用同一个预检查函数和已建立的摘要，完整返回值相等。

正确性比较后另做一次预热，再交替顺序测量 7 对样本。重放比较调用保存的修改前函数及当前函数，包含文件读取、JSON 解析和比较；每个样本执行 20 次，报告单次平均耗时的中位数。

| 场景 | 修改前中位数 | 修改后中位数 | 减少 |
| --- | ---: | ---: | ---: |
| 6265 个任务的预检查 | 1554.591 ms | 1193.047 ms | 23.26% |
| 512 KiB 候选重放比较 | 7.434 ms | 6.453 ms | 13.20% |
| 4 MiB 候选重放比较 | 20.637 ms | 15.272 ms | 26.00% |

预检查 7 次样本范围为修改前 1533.156–1639.508 ms、修改后 1109.836–1261.815 ms。单独重算该计划摘要的中位数为 349.014 ms；首次建立摘要或计划变化时仍需支付这项成本。

基线保存了本轮修改前的相关源码，包括当时已有的未提交修改，位于 `tmp/digest-reuse-20261021/baseline/`。[可复跑入口](../../tmp/digest-reuse-20261021/benchmark.ts)和[原始样本](../../tmp/digest-reuse-20261021/performance.json)保留在同目录。仓库根目录运行：

```powershell
& 'D:\fate\node\node_modules\bun\bin\bun.exe' run 'tmp/digest-reuse-20261021/benchmark.ts'
```

首次夹具设置错误地预期默认路由为 V4，实际返回受支持的 V3；该次在测量开始前退出，日志保存在 `benchmark-setup-failure.log`。修正版本预期后完成上述全部样本。

## 测量范围

预检查计时不包含派生目录读写、宿主调度和模型生成；重放计时只覆盖读取和比较函数。这些数字说明本地路径的收益，不能换算为整本书完成时间。合成验证未调用模型，未读取或改写正式书籍的计划、成果或 invocation。

## 本机安装

2026-10-21 13:08（香港时间）已将通过验证的编译版安装到 `E:\allwork\Understand Book\understand-book-build.exe`，上一版保存在同目录 `understand-book-build.before-digest-reuse-20261021.exe`。

安装文件与候选逐字节一致，原插件启动入口的初始化握手通过。正式构建控制记录与本轮开始时逐字节一致；未新建原书的 invocation、未改确认计划、未启动正式语义构建。见[安装记录](../../tmp/digest-reuse-20261021/installation.json)和[安装验证](../../tmp/digest-reuse-20261021/installed-handshake.json)。
