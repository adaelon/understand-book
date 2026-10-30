# Codex 有界批次连续执行（S1a）

日期：2026-10-28。完成首个生产实现切片：Codex 同一有界 dispatch 内连续处理工作单元。保持原模型、质量合同、并发与批次规划上限。跨 dispatch 的长期 worker 队列、DSH 生命周期优化及视频/音乐优化仍在后续切片。

## 结果

旧的当前公共 handoff 每个工作单元提交后返回 DONE；新建 Codex 会话提交成功后直接返回下一项 DELIVER_INPUT，直到有界批次完成。固定三单元夹具在一个有效 MCP 连接和一个工作进程内完成三个独立生成与提交，启动边界由三次降为一次。中间两次无需 Root 等待终态、重新派发和重新建立连接。

这是启动次数与接续机制的验证，不是整书耗时或真实模型质量的性能对照；不能据此宣称三倍加速。真实模型推理、累计上下文带来的输入成本及宿主端延迟未在该确定性夹具中测量。

## 实现

- `automatic-build-executor-session.ts`：新 Codex V4 handoff 的首次 open 写入内部 `automatic_build_executor_open_record.v2`，一次冻结当前 dispatch 剩余可领取前缀的逐项 recovery identity。每项仍独立交付、领取、验证、提交；接续不会重算身份以接管后来的恢复代次。
- `automatic-build-dispatch-runtime.ts`：只读提取连续可领取前缀；当前项不可领取时保持原异常，后续不可领取项截断批次授权，不预先领取任何任务。
- `automatic-build-driver.ts`：主控识别仍覆盖当前单元的原 handoff，优先保留 live ref；部分槽位恢复时仅重发缺失项。槽位已满且子代理处于输入交付阶段时继续 WAIT。
- 终态释放指向批次当前项，失败不继续下一项；旧 epoch 不能从 A 的旧 ref 接管 B 的新 epoch。旧的已持久化 open.v1 与 DSH V5 handoff 保持原逐项终态。
- 单元计时使用该单元 delivery 的首次创建时间，避免 B 的启动耗时包含 A 的生成时间。
- 角色、技能、项目 agent 和插件发布副本同步：收到下一项输入直接继续，每项仅使用其 generation_input_ref 对应的输入与证据；终态后新 ref 仍使用新 child。

原批次预算继续生效：pass1 最多 4 单元、合计估算输入 50,000 token；公式侧车最多 8 单元、合计 4,000 token；discourse 最多 4 单元、合计 6,000 token。其他多数阶段仍为单单元批次。本切片不会让 worker 无限制积累整本书的上下文。

## 验证

执行器完整回归 52 项通过，随后新增的多单元超大候选终止用例单独验证。原有终态测试改为等待异步响应或 flush/finish 后断言，6 项通过。DSH session 与执行 profile 11 项通过。角色、发布及注册合同 52 项均已有通过证据（首轮 50 项，修正现有 predecessor 陈旧断言并将默认 registry 改放 E 盘后复验另外 2 项）。主控完整回归 85/85 通过；dispatch runtime 32/32 通过；最终打包 stdio 与新增失败边界 2/2 通过。Core TypeScript 类型检查通过。

关键覆盖：

1. 一个连接连续完成 A、B、C，每项语义尝试 1 次；重复提交的正式回执与成果字节不变。
2. A 完成、B 输入阶段 child 退出：只提升 B 的 bootstrap epoch，A 的已提交成果保留；旧 open/start/submit 重放不能接管新 B。
3. live child 逐项前进时保持原 ref；另一个槽位终止时，不替换仍在工作的同伴。
4. 超大候选记录失败并结束本批次，不交付 B；旧持久化 open.v1 保留原终态。
5. Windows 打包程序通过真实 stdio 执行三个单元。测试先在独立连接故意发送错误请求版本并确认关闭；随后一个正确连接连续完成三项。原始 transcript 因而包含两次 open，其中一次是该故障注入，并非业务任务额外启动。

测试及构建临时目录：`tmp/continuous-executor-20261028/`。使用已有 `apps/desktop/scripts/build-sidecar.mjs` 构建 Windows 程序与配套音乐资源，产物位于该目录的 `release/`。源码与打包共用同一协议测试，设置测试专用 `UNDERSTAND_BOOK_EXECUTOR_TEST_EXE` 可选择打包程序；未设置时测试源码 stdio。

### 保存的证据

- [机制指标](mechanism-result.json)：三个单元、一个有效工作连接、三次生成、一个批次提交终态。
- [执行器完整回归](executor-session.json)：52 项通过；新增第 53 项失败边界与最终打包复验见[最终两项报告](final-boundaries.json)。
- [主控完整回归](driver-result.json)：85 项通过，包含多槽恢复与预算/重试/阶段收口。
- [DSH 与执行 profile](dsh-profiles.json)：11 项通过。
- [最终 Windows 程序 stdio 原始往返](packaged-stdio.json)：独立错误版本连接后，一个正确连接连续完成三项。

## 限制与后续

- 本轮没有真实慢书的模型端到端对照；未测书籍生成质量、总输入 token 或整体提速比例。模型输出合同和写入校验保持原规则。
- S1a 实施时 C 盘空间为 0，后续已恢复至约 3.7 GB；新测试 registry、临时文件和构建产物均在 E 盘。现已通过独立插件完成本项目新会话接入，当前已加载的 MCP 连接继续使用原进程，见[实际安装与 launcher 验证](../prebuild-engine-activation-20261028/README.md)。
- 后续同日完成视频独立翻译/discourse 并行、常驻转写与音乐增量 rebuild，已包含在最终引擎，见[统一方案状态](../../实施方案-预构建吞吐与执行器复用.md)。跨 dispatch 继续复用仍需要累计上下文与逐任务成本绑定。
