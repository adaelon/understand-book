# 固定执行器与媒体预构建：可行性核验

日期：2026-10-28。结论：书籍的固定执行槽与有界连续执行在技术上可行；优先打通当前普通书籍逐单元结束的会话边界。视频翻译、discourse 并行具备明确独立输入；Whisper 常驻可行但已有长视频样本的模型加载占比较小。音乐直接校准已增量执行，优化仅针对完整重建与失败恢复。

本轮更新[实施方案](../../实施方案-预构建吞吐与执行器复用.md)，新增探针与证据，未修改生产逻辑。工作树 HEAD 为 `d791eea51009a4c36150528de50feb5d081c1281`；其他既有未提交修改保留。使用本机 Node v24.9.0、已安装 DSH 0.1.7-rc.2；真实 Provider 调用为 0。

## 1. 书籍：可行，需改变任务结束与 worker 结束的绑定

当前 `automatic-build-executor-session.ts:5209` 对 handoff record V4/V5 在单个工作单元接纳后立即返回 DONE。`currentPublicDispatchResponse` 与 `inspectCurrentPublicRecovery`（同文件 4105、4126 行附近）也把当前引用固定到确切 work unit/recovery identity。规划器将多个单元放入同一个 dispatch，不等于当前执行器会连续做完该组。

本次定向回归确认：三单元 dispatch 的第一项提交仍返回 DONE；底层 dispatch runtime 又能在同一 dispatch 中依次处理八个任务、每次只有一份活动租约并保存八份回执。两个测试均通过。这说明任务层可以复用，限制在当前公开会话与宿主接续方式。

最小落地切片采用一个有界 dispatch 内连续工作：每项仍有独立输入、session、sink、lease、attempt 和接纳回执；完成后由 Engine 发下一份输入，批次完成、失败或需要换上下文时才结束 worker。需明确区分 worker 会话与单任务恢复身份，不能仅删除 DONE 分支或允许任意换 ref。随后再接跨 dispatch 的同阶段队列。

现有视频翻译的两个任务已通过同一真实 MCP phase gate 连续接纳测试，尝试次数均为 1；这是连续交付的可复用基础，不是新的书籍协议已经完成的证明。

## 2. DSH：本机公开 API 已实测可复用

探针通过 `ctx.agents.create()` 持有一个 child，连续两次 followup/whenIdle，使用 ScriptedModel。观察结果：

- 同一个 child 对象完成两轮，受控模型请求数为 2。
- Agent 作用域的工具注册在两轮间保持存在。
- 第二轮请求包含第一轮任务历史，上下文确实累积。
- 主动 dispose 后 child 从 registry 移除。

[实际结果](dsh-reuse-result.json)、[复跑脚本](dsh-reuse-probe.mjs)。第一次探针缺少生产已有的 native 工具模式和显式 maxTokens，未进入模型；配置修正后成功。完整首次失败和诊断范围保留在结果中。

官方 `startContinuable/sendMessage` 可继续子会话，但空闲后会释放 Agent activation，后续消息可能冷恢复（本机 `dsh-subagent/lib/index.js:1155–1205`）。直接替换 API 不保证消除启动开销，还涉及 session persistence。

推荐首个切片沿原生 one-shot 生命周期，在同一受上下文预算约束的运行内由工具接续任务。若采用多回合常驻，再使用自持 AgentHandle，明确保留父子归属、preset、权限继承和取消处理。

项目需拆开的绑定包括 `executor-host.ts:69–111` 的 launch/active/dispose、`executor-bridge.ts:15–39` 的初始 handoff/连接闭包、`executor-tools.ts:22–36` 的工具注册及 `build-control.ts:210–251` 的任务回执。工具及计量 middleware 只注册一次，每次使用当前任务 binding，旧请求结算结束后才切换。

公共 Agent API 没有清空已处理历史的 reset；inbox.clear 仅取消待处理输入。当前专用 preset 不做历史裁剪，模型边界又会计算全量历史。达到上下文预算前轮换 child，后续请求增加的历史输入成本照实际调用计入。

## Codex 当前聊天的补充验证

2026-10-28，用户单独询问 Codex 可行性后，Root 使用当前宿主的 `collaboration.followup_task`，向已经完成并返回 final 的 `/root/host_reuse_feasibility` 发送一个新任务。该工具成功返回，原子代理随后回复：“已在同一个子代理中收到新的后续任务。”未创建新的子代理，也未修改生产逻辑。

该实测证明当前 Codex 宿主支持继续使用已完成的原生子代理。它没有运行专用 `understand_book_executor` 的第二个 handoff，因此不证明现有构建连接已支持换任务，也不证明提速或上下文清空。

Codex 推荐路线是在一次有界执行中由 Engine 连续交付任务，阶段结束或上下文需轮换时才返回终态。当前 Root 仍可用原生工具创建及替换少量执行器；阶段内接续不依赖外部程序创建 Codex 子代理的接口。逐任务调用 followup 虽然宿主支持，仍会保留主 Agent 补位等待。

## 3. 视频：并行的内容依赖已核实

| 工作 | 判断 | 最小前提 |
| --- | --- | --- |
| 翻译 | 可并行；输入只有原文及原文上下文 | 控制端统一领取、更新 plan/progress，worker 返回确切任务 candidate |
| discourse | 可并行；实际模型输入不带先前 graph 概念，关系端点限当前候选 | 冻结同一发布快照，集中接纳累计清单 |
| 视觉 | 可并行 | 保留领取前已接纳相同画面的复用，错误绑定确切任务，防止增加无用模型调用 |
| graph | 首轮保留顺序 | `existing_nodes` / `same_as` 真实依赖前序接纳概念；并行需另做对象归并 |
| Whisper | 常驻可行 | 同一实例逐块收请求，保留逐块计量、结果落盘、取消和缺失块恢复 |

共享写入依据：翻译 `video-translation.ts:71–76,99–113,149–154`、视觉 `video-visual-units.ts:165–179,230–256` 都会读改写整份 plan；视觉失败还查找全局第一个 running。`video-enrichment.ts:48–57` 更新共同累计清单。因此直接给外层循环加并行会产生状态覆盖或错误归属问题。

graph 的 `video-graph.ts:60–72,127–132,175–178` 会读取并使用已接纳对象。discourse 的 `video-discourse.ts:135–137` 实际交付不含这些对象；它可以从同一发布准备独立输入。

### 常驻 Whisper 的收益上限需要按已有样本校准

[77.4 分钟真实视频记录](../../../evals/video/results/vi3-20261010/verification.json)包含 16 个音频块：

| 指标 | 原始值 | 含义 |
| --- | ---: | --- |
| 转写至状态保存 | 1,460,517 ms | 约 24.3 分钟 |
| 模型推理合计 | 1,226,864 ms | 总时间约 84.0% |
| 模型加载合计 | 30,503 ms | 总时间约 2.09% |

仅消除重复模型加载不可能使该样本大幅加速；常驻仍要加载一次。Python 启动、模块 import 和抽音没有单独分解，不能把 2.09% 当成整个常驻方案的严格收益上限，也不能把剩余时间全部归给抽音。此证据将常驻转写排在书籍接续、视频语义并行之后。

## 4. 音乐：完整重建可复用，直接校准已经增量

`music-import.ts:44–60` 的直接校准只更新 manifest、curation 和历史；不会解码、解析或渲染。历史 7,268/8,907 ms 是两份选段完整 rebuild 的记录。

完整 rebuild 重复执行探测/解码（96–104）、谱面解析/渲染（107–117）和示范合成（127–135）。现有 probe、parsed、SVG/MEI/MIDI、来源快照和编选请求可复用；尚需补齐局部完成记录和实际渲染/合成参数及处理版本比较。无需新缓存服务或额外摘要体系。

## 5. 验证证据

| 验证 | 结果 | 证明范围 |
| --- | --- | --- |
| 当前普通书籍三单元 dispatch 中间提交 | 1 项通过 | 第一单元提交即 DONE 的当前行为 |
| 八任务公式 dispatch | 1 项通过 | 任务层连续推进、单活动租约、逐任务回执 |
| 视频翻译连续任务 | 1 项通过 | 同一个 MCP session 连续接纳两个任务，尝试次数各为 1 |
| 已安装 DSH Agent 复用探针 | 两轮通过 | 同 child、工具注册存活、历史累积及清理 |
| 当前 Codex 原生子代理后续任务 | 成功 | 已完成的同一子代理经 followup_task 再次响应，未验证构建 MCP 换任务 |
| 独立异步 MCP 生命周期探针 | 6 个场景通过 | committed/retryable/interrupted/error 结束、WAIT 保持、未绑定 ref 纠正 |
| 原 MCP 生命周期测试 | 1 项通过，5 项失败 | 原测试即时断言与异步 transport 不一致，保留失败记录 |

原始结果：[Core 定向测试](core-boundaries.json)、[翻译连续任务](video-continuation.json)、[异步 transport](mcp-lifecycle-result.json)、[DSH 两轮](dsh-reuse-result.json)。

原生命周期测试在 input.write 后立即判断 destroyed 或 output.read；实际 `serveBuildExecutorMcpSession`（`skills/build/build-executor-mcp.ts:403`）通过 Promise 队列处理。独立探针等待实际响应/finish 后验证六个对应场景通过。未修改原测试，也未将失败计为成功。

复跑独立探针：

```powershell
node --import tsx docs/performance/prebuild-feasibility-20261028/dsh-reuse-probe.mjs
node --import tsx docs/performance/prebuild-feasibility-20261028/mcp-lifecycle-probe.ts
```

定向测试在 `packages/core` 执行本地 Vitest，分别选择 `returns committed after an intermediate current V4 generation submit`、`runs an eight-task formula dispatch`、`executor MCP transport lifecycle`、`uses the formal MCP phase gate and advances accepted tasks in one bounded handoff`；临时目录重定向至 E 盘。保存的 JSON 含完整断言结果和失败信息。

## 已知限制与下一步

- 本轮没有实现新书籍协议，没有运行真实模型性能/质量对照，也没有证明自动补位的端到端时延。
- DSH 公开 Agent API 的本地复用成功不等于完整安装插件链路验收；Codex 原生子代理续用已实测，专用构建会话接续仍需新 Engine/角色协议匹配后验证。
- 原 MCP 生命周期测试存在上述五项失败，实施时应按异步契约修正测试并保留原覆盖。
- 本轮 C 盘约余 1.4 MB，分析子代理能创建，但长期真实运行需要足够状态存储空间。

下一切片优先验证三执行槽连续完成至少两轮书籍任务：每项独立接纳，完成立即补位；中断只恢复缺失项，预算拒绝不发新请求，上下文轮换保留已完成成果。通过该切片后再比较同输入、同并发、同模型的真实吞吐和成本。
