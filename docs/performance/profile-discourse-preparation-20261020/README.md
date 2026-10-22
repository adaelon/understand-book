# Profile discourse 提前准备与交接耗时

2026-10-20，本地编译版测量：普通提取交接中位数 **4.432 → 3.988 秒（缩短 10.0%）**；fragment **7.317 → 4.978 秒（缩短 32.0%）**。

## 实现

首批 profile_sidecar 执行器启动后，Root 在它们生成内容期间调用一次 `build.prepare`，仅传已保存的 invocation。Engine 构造普通 discourse 输入、fragment 切片及共享来源分析，保存到工作区 `.build/automatic-build/v3/profile-discourse-inputs.json`。Root 只接收状态和数量。

补位仍通过现有 `build.refill`：消费终止 child，保留其他槽位所有权，读取当前回执和预算，再发布下一份交接。原文身份、目标、提取策略或输入预算不匹配时重新路由。准备不创建 dispatch、租约或语义尝试；reducer 继续依赖实时接纳结果。

新执行器读取普通 discourse 或 fragment 时，在 Engine 内调用原有 replay 函数，保留来源、策略和预算校验，省去逐任务启动 input 子进程。formula 和 reducer 的输入调用路径保持原状。

代码入口：

- `packages/core/src/build-orchestrator.ts:prepareProfileDiscourseInputs/readPreparedProfileDiscourseInputs/routeProfileSidecarProductionStage`
- `skills/build/automatic-build-driver.ts:runAutomaticBuildDriverCommand` 的 `automatic_build_prepare_request.v1`
- `skills/build/automatic-build.ts:renderAutomaticBuildTaskInput`
- `skills/build/sidecar-entry.ts` 的 `build.prepare`
- `skills/build/SKILL.md:Four-action loop` 及 plugin 发布副本

## 场景与口径

使用实际编译的 Windows Engine 和 stdio MCP 连接，计时范围为：

`executor.submit_candidate 开始 → DONE committed → build.refill → 新连接 initialize/open/input.next/generation.start → GENERATE`

三个执行器同时持有任务，先结束第三个，保留前两个，再启动第四个新连接。提交、写入、回执、租约、Driver 和输入传输均使用生产实现。候选内容由固定测试数据生成。

| 场景 | 输入规模 | 初始任务数 |
| --- | --- | ---: |
| 普通提取 | 96 个标题及独立短段，每段重复固定句子 12 次 | 96 |
| fragment | 12 个标题及独立长段，每段重复固定句子 540 次 | 36 |

每个版本、每个场景测首轮及三次重复；每轮恢复同一绝对路径下的初始工作区。准备在前三个执行器已进入 GENERATE 后、提交前通过独立 Engine 进程完成，耗时单独记录。准备记录大小分别为 895,346 和 1,132,804 字节。

环境：Windows 10 x64，AMD Ryzen 7 5800H，16 个逻辑处理器，约 14.9 GB 内存；测试驱动 Node v24.9.0。测试按单 worker 串行执行；性能计时期间未并行运行本轮其他测试或构建。

基线来自本轮修改前的工作区，HEAD 为 `33c19c7fa8aec9a4f1031122bc177743b234b9fd`，保留已有未提交修改。独立编译目录复制当前运行所需源码，再用开始时保存的四个实现文件覆盖本轮修改。候选由当前源码另行编译，两份程序分别运行相同测试。

## 结果

下表为三次重复的中位数和总交接耗时范围，单位秒。

| 场景 | 优化前总交接 | 优化后总交接 | 缩短 | 优化前范围 | 优化后范围 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 普通提取 | 4.432 | 3.988 | 10.0% | 4.418–4.463 | 3.843–4.032 |
| fragment | 7.317 | 4.978 | 32.0% | 7.042–7.531 | 4.939–5.058 |

| 场景 | 环节 | 优化前 | 优化后 |
| --- | --- | ---: | ---: |
| 普通提取 | 提交接纳 | 0.748 | 0.751 |
| 普通提取 | 补位命令 | 2.374 | 2.360 |
| 普通提取 | 新连接至可生成 | 1.342 | 0.827 |
| fragment | 提交接纳 | 1.163 | 1.145 |
| fragment | 补位命令 | 4.244 | 3.016 |
| fragment | 新连接至可生成 | 1.897 | 0.832 |

各环节中位数分别计算，不相加作为总交接中位数。

普通提取的收益主要来自输入接收；其补位耗时差异落在本组波动内，尚未证明提前准备本身使补位变快。fragment 的补位和输入接收都有明显改善。提交接纳没有可归因的加速。

提前准备中位数：普通 **0.801 秒**，fragment **2.369 秒**。这是每阶段一次的额外工作，安排在已有执行器生成期间。它不计入上述交接间隔；如果生成窗口不足，未被覆盖的准备时间仍会增加等待。

首轮总交接也保留：普通 **5.325 → 3.949 秒**；fragment **11.678 → 4.790 秒**。系统文件缓存未强制清空，首轮不作为冷启动保证。

## 正确性与复验

`profile-discourse-inputs.test.ts` 验证来源及质量策略变化后重算、准备不占租约或语义尝试、输入字节一致、来源变化仍拒绝，以及新提交的 fragment 能解除 reducer 依赖。`profile-discourse-preparation.test.ts` 验证跨进程准备、三槽位补位、已提交任务不重发、其他槽位不释放，以及每次提交只有一个语义尝试和一次持久提交。

相关回归包括 Driver、budget、refill、production v3 routing 和 profile-sidecar reduction，**7 个文件、111 项测试通过**，见 [regressions.log](regressions.log)。Core TypeScript 类型检查通过，发布技能一致性检查通过；两份 Engine 均编译成功，分别通过两个真实 stdio MCP 性能场景。

测试入口位于 `packages/core/test/profile-discourse-preparation.test.ts`。在 `packages/core` 中设置三个环境变量后运行：

```powershell
$env:DISCOURSE_HANDOFF_MODE = 'baseline' # 候选改为 prepared
$env:DISCOURSE_HANDOFF_EXE = '<对应的编译版引擎绝对路径>'
$env:DISCOURSE_HANDOFF_REPORT = '<独立结果 JSON 绝对路径>'
node ../../node_modules/vitest/vitest.mjs run test/profile-discourse-preparation.test.ts --maxWorkers=1 --minWorkers=1 --reporter=dot
```

省略环境变量时，它作为较小规模的普通回归执行。性能模式的固定样例和初始副本保存在仓库 `tmp/profile-discourse-preparation-20261020/` 下，四个基线实现文件位于 `baseline-source/`，独立编译源码位于 `baseline-compile/`。优化前后程序分别位于 `compiled-baseline/` 和 `compiled/`。

原始结果：[baseline.json](baseline.json)、[candidate.json](candidate.json)、[comparison.json](comparison.json)、[environment.json](environment.json)。两份编译版测试均通过两个场景，计时值在各自 JSON 中逐样本保存。

## 已知边界

- 这些是合成书籍上的引擎交接实测，不包含主模型收到终止消息后的推理或排队、Codex 原生子代理创建、模型生成正文的耗时，也不表示整本构建提速相同比例。
- 输入及候选验证使用真实实现，模型候选采用确定性数据；没有重新评估真实模型生成内容的语义质量。
- 三次重复足以区分本组交接差异，不用于推断 p95 或其他规模的加速比例。
- 较早的开发环境记录保留在同一临时目录。首次 source 基线的业务断言通过，但测试框架回传超时；修正让出执行的间隔后重跑通过。上述结论全部采用干净通过的编译版结果。
- 候选程序已编译并实测，尚未替换日常安装版；自动提前准备还需要相应构建技能随发布更新。
