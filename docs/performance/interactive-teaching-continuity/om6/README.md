# OM6：完整场景与真实模型行为对照

日期：2026-10-20。基线 HEAD 为 `33c19c7fa8aec9a4f1031122bc177743b234b9fd`。沿当前工作树的 OM1–OM5 实现固定三组场景，使用实际存储、Tutor 工具、交付校验和 Reader 验证接续；同一场景对比 HEAD 与当前 Tutor 指导下的真实模型回答。

## 实现与实际修正

- [本地场景与回放](../../../../crates/server/src/tests/interactive_teaching_om6_tests.rs) 复用已有演示、教学设计、源码片段、JSONL 聊天和 LearningStore 夹具。销毁并重建 AppState 后导出实际 prepare，真实模型生成的 design 先经过原 `teaching::step` 接受，再将真实回执交给模型生成 select。
- [网络宿主](../../../../crates/server/src/tests/interactive_teaching_om6_network_tests.rs) 使用原多用户路由创建教学工作区、实际聊天、选入的历史前缀和新工作区，重建 Authorization／UserRegistry，再沿原异步运行交付。原聊天 JSONL 在当前运行前后按字节直接比较。
- [浏览器场景](../../../../packages/web/playwright/interactive-teaching-om6.spec.ts) 使用本地真实 RightRail／AgentPresentation 和网络 NetworkApp。本地演示验收入口补接普通聊天发送；教具保存、追问、网络运行订阅和重开沿原 API。
- [模型采样入口](run-model-comparison.mjs) 复用已有 `evals/semantic/provider-recorder.mjs`，保存实际请求、回答、usage、耗时与失败；没有增加通用评分平台。
- 真实模型在初次开放设计回答中把“相对固定基准的总节省时间”误写成随速度增加而递减。`tutor_writing_rules.md` 升为 **`plain-explanation.v3`**：比较必须明确基准、固定条件和步长，区分总量与继续改变一步的额外变化，正文纠正关系时同步修订设计。主模型与审阅继续共用这一份规则；`teaching-design.v13` 不变。

错误原文为“这个节省量随 v₁ 提高而递减”，对应公式是 `S/v₀ − S/v₁`。固定正数 S、v₀ 时，这个总节省量随 v₁ 增大而增加；固定增速步长的额外节省才会递减。保留[初次回答](after-open-design-initial/answer.md)、原请求和 provider 记录，仅重采样该案例的 design／select。修订后的[设计](after/open-design/design-output.json)和[正文](after/open-design/answer.md)采用 `T=S/v+停留时间` 分项比较，不再使用错误的递减判断。

## 三组场景

| 场景与实际入口 | 固定条件和当前请求 | 实际结果 |
| --- | --- | --- |
| 速度／本地 Reader | 同版教具输入 B=2 m/s，实际应用仍为 1 m/s，10 秒显示 10 米；要求先解释现场再给操作步骤 | 改后回答区分输入值与应用值，给出“应用速度”操作以及应用后 20 米；浏览器实际点击、保存并重开验证 |
| 借用／本地 Reader | 先展示 `&foo` 示例，学习者随后说“bar 是 &Foo，foo 仍拥有值”；重开后明确要求逐行解释两次 println 与移动差别，不先出题 | 改后设计采用原话引用，直接解释共享借用与移动；原表达保留已见帮助和 unassessed，不推断独立掌握，不新增作答邀请 |
| 开放设计／网络 Reader | 原话倾向少停站并保留必要休息，历史偏好简单讲解；选入原聊天前缀后，当前要求深入分析公式、多方案和取舍 | 改后回答分析减少非必要停留、安全条件下提速及组合，保留休息约束，没有唯一答案选择题；网络验证选入前缀、当前请求及原记录保持 |

本地原始证据见 [速度](local-speed-evidence.json)、[借用](local-borrowing-evidence.json)。网络原始证据见 [network-evidence.json](network-evidence.json)。教师连续动作、用户独立接手、演示取消及已见帮助冻结的既有工程证据继续采用 [OM5](../om5/README.md)，本片不重复累计它的通过数。

## 真实模型对照

使用已配置的 `deepseek-v4-flash`，有效对照均为 `temperature=0`、`max_tokens=8192`、`thinking=disabled`。三组输入分别来自 [speed](contexts/speed.json)、[borrowing](contexts/borrowing.json)、[open-design](contexts/open-design.json) 的实际持久化重开。两臂的材料、请求、当前上下文和工具合同相同，替换 Tutor 指导与共享写作规则。完整定义见 [comparison.json](comparison.json)。

速度与借用采用 OM3／OM5 的 v2 规则完成采样；开放设计修正后采用 v3。前两组没有为新增的公式规则重采样，其原回答在 v3 Runtime 中通过交付回放。

每臂分两次采样：design → 原工具实际接受 → 带真实 design_ref／block_refs 的 select。初始任务希望一个 JSON 带齐设计和交付，模型按指导先给设计，因此后续沿正式的两阶段流程继续。原任务与原响应均保留；复现脚本已将首阶段请求明确为 design。初次开放设计的额外字段没有冒充实际工具执行，最终正文始终取实际 select 的 `delivery_text`，未经手工润色。

| 场景 | HEAD 指导：实际回答及交付 | 当前指导：实际回答及交付 | 两阶段 token：前／后 | 两阶段 provider 秒：前／后 |
| --- | --- | --- | --- | --- |
| 速度 | [回答](before/speed/answer.md)正确解释输入／显示滞后，交付通过 | [回答](after/speed/answer.md)正确解释并给出操作，交付通过 | 42,513／44,388 | 8.214／8.082 |
| 借用 | [回答](before/borrowing/answer.md)直接解释借用和移动，交付通过 | [回答](after/borrowing/answer.md)逐行解释并采用原话依据，交付通过 | 50,042／50,683 | 10.408／9.074 |
| 开放设计 | [回答](before/open-design/answer.md)给出多个方案；正文引用内部 LID，被原 `RAW_LID_LEAK` 门禁拒绝，本次有界回放没有交付 | [修订后回答](after/open-design/answer.md)保留多方案与当前明确请求，交付通过 | 48,023／48,710 | 14.238／9.102 |

每个 `before|after/<case>/` 保存：

- `input.json`、`design-request.json`、`design-provider.json`、`design-output.json`：第一阶段的实际输入和原响应。
- `design-receipt.json`、`stage2-input.json`：原工具接受的回执及后续上下文。
- `request.json`、`provider.json`、`select-output.json`：第二阶段的实际请求、原响应和选择。
- `output.json`、`answer.md`：由两阶段响应整理的原设计、选择与公开正文。
- `replay.json`：原 Runtime 工具循环、prepare、主模型采样输入、审阅输入、选择及交付记录。

共 **15 次 provider 调用，360,143 total tokens，缺失 usage 0，provider 耗时合计 108.834 秒**，见 [model-usage.json](model-usage.json)。其中最终六臂共 12 次；另外保留初次默认 thinking 用满输出额度却未返回正文的 1 次失败，以及开放设计错误版本的 2 次。后者重跑只替换共享规则；速度和借用的已通过回答没有再次采样。各次耗时是 provider 请求时长，不包含编译、浏览器或人类阅读时间。

初次失败见 [failed-default-thinking-speed](failed-default-thinking-speed/provider.json)。修复后的有效对照统一关闭 thinking，两个臂使用相同设置。[after/tutor_writing_rules.md](after/tutor_writing_rules.md) 保留初始 v2，[v3 快照](after/tutor_writing_rules.v3.md)和修订后开放设计的 input 保存实际新增规则。

## 工程验证

| 检查 | 检测的具体失败与处置 | 结果 |
| --- | --- | --- |
| 三组持久化输入导出 | 真实现场、原表达或帮助在重开后丢失则修对应 prepare／接续路径 | 1 项通过，包含 3 场景；[导出日志](export.log)、[最终普通测试日志](export-final.log) |
| 模型设计真实接受 | 模型引用、设计字段或目标不符合原合同则定位该实际设计；不手改公开正文 | 1 项通过，包含 6 臂；修订后的开放设计单独接受通过；[初次](design-accept.log)、[修订后](design-repair-accept.log) |
| 冻结回答走原工具循环 | 改后正文未通过交付门禁或落盘正文改变则失败 | 1 项通过，包含 6 臂；改后 3 臂原文交付，改前开放设计拒绝如实保留；[日志](replay-final.log) |
| 主模型与审阅共享规则 | v3 漏装配或重复装配则修共享模块入口 | 2 项通过；[日志](runtime-rules.log) |
| Web 类型 | 真实验收入口新增发送事件与消费者不一致则修入口 | 通过；[日志](web-types.log) |
| Reader 浏览器 | 实际应用值、重开、帮助条件或选入历史断链则定位实际 API／页面路径 | 本地 1 项、网络 1 项通过；[本地日志](browser-local-final.log)、[网络最终日志](browser-final.log) |
| 工具参考生成 | 现行指导与导出的工具／状态示例不同步则修原生成输入 | 通过；Resident 44、Local MCP 16、Remote MCP 11、Executor 4，Tutor 状态 17；原导出和状态示例验证均通过；[日志](tool-reference.log) |

测试默认 ignored 的两个模型回放与两个 HTTP 宿主均须显式运行；它们不是默认单元测试的通过数。采样脚本调用真实 provider，冻结回放、浏览器和规则装配测试不调用 provider。

交付检查：涉及的已跟踪文件通过 `git diff --check`，新增源码／脚本／记录无行末空白；131 个本地证据和文档链接均存在，checkpoint 为 46 行，见 `delivery-check.json` 与 `diff-check.log`。两个验收宿主已通过 `/stop` 正常结束。

## 复现入口

在仓库根目录选择独立输出目录，将其绝对路径设为 `OM6_OUTPUT`。普通测试未设置该变量时只验证三组持久化条件，不覆盖已保存的模型输入。重新采样会产生外部调用；已有冻结证据可直接用于回放。

```powershell
# 生成真实的三组输入；OM6_OUTPUT 指向本次输出目录。
cargo test -p server --lib om6_three_scenarios_export_persisted_input_conditions -- --nocapture
node docs/performance/interactive-teaching-continuity/om6/run-model-comparison.mjs prepare
node docs/performance/interactive-teaching-continuity/om6/run-model-comparison.mjs run
cargo test -p server --lib om6_accept_model_designs -- --ignored --nocapture
node docs/performance/interactive-teaching-continuity/om6/run-model-comparison.mjs select
cargo test -p server --lib om6_replay_real_outputs_through_delivery -- --ignored --nocapture
```

采样复现读取 `.env` 中已有 provider 配置；原始记录没有保存凭证。`prepare` 冻结执行时的 HEAD 与工作树指导，已交付比较的基线和精确输入以本目录记录为准。定向设计接受可用 `OM6_CASE`／`OM6_VARIANT` 选择一个已有失败案例。

浏览器用两个独立终端启动宿主，再启动原 Vite 代理：

```powershell
cargo test -p server --lib om6_local_browser_host -- --ignored --nocapture
cargo test -p server --lib om6_network_browser_host -- --ignored --nocapture

# 在第三个终端运行，OM6_OUTPUT 沿用宿主的证据目录。
$env:UNDERSTAND_BOOK_ADDR = '127.0.0.1:4194'
$env:OM6_LOCAL_HOST = 'http://127.0.0.1:4194'
$env:OM6_NETWORK_BACKEND = 'http://127.0.0.1:4196'
$env:OM6_NETWORK_CONTROL = 'http://127.0.0.1:4197'
pnpm --filter @understand-book/web exec playwright test --config playwright.om6.config.ts
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:4194/stop
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:4197/stop
```

本地宿主除夹具控制和组装真实准备／执行的聊天入口外，调用原路由；网络宿主运行原多用户服务，浏览器代理为该本地测试提供实际 Host、Origin 和登录身份。Reader 不依赖公开部署。重跑本地用例前通过 `/om6/case/speed` 重建起始状态；网络用例重新启动宿主生成独立记录。

重开截图：[速度实际读数](local-speed-reading.png)、[速度教具](local-speed-reopened.png)、[借用回答](local-borrowing-reopened.png)、[网络开放设计](network-open-design-reopened.png)。页面文字与原存储记录同时用于核对。

共享规则与文档验证：

```text
cargo test -p runtime --lib -- teaching_writing_rules_reach teaching_review_receives
pnpm --filter @understand-book/web typecheck
pnpm docs:tools
```

## 失败记录与限制

- 采样和回放分开：真实模型负责设计、支撑选择与原文；冻结回放只重绑重建夹具的确切引用，沿实际工具、校验、存储和浏览器执行。语义审阅使用已有确定性接受适配器，实际审阅输入有记录，本片没有测量真实审阅模型发现错误的能力。
- 两臂都收到当前真实上下文。因此本对照测的是这些固定材料下的指导行为，不是旧二进制与新二进制的全栈因果实验，也不能单独证明 continuity 投影带来的提升。速度和借用两臂都满足场景要求；没有据此宣称改后普遍优于改前。
- 改前开放设计的内部 LID 拒绝被保留；初次回放将所有臂都要求通过而提前停止，随后改为只要求改后通过、完整记录基线失败。见[初次日志](replay-initial.log)。这是有界冻结回放，未测真实模型收到该错误后能否自行修正。
- 首轮浏览器夹具遗漏网络 Tutor 开启，且把 `{text,truncated}` 原话摘要当作字符串；已按正式控制路由启用，采用实际字段，并要求历史种子确有教学交付。夹具读取原 `ordered_messages` 中的冻结输入和最新动态学习视图，并使用控制状态返回的当前 revision。对不属于本场景的后台档案请求返回错误，不再误当交付审阅。见[首轮浏览器日志](browser-initial.log)、[上下文解析失败](network-host-final.log)、[控制 revision 失败](network-host-context-fixed.log)。
- 两个浏览器用例的断言通过后，退出时未完成的代理请求导致测试进程报错；末尾离开 Reader 页面以停止后台请求，再等待在途请求完成，随后只重跑网络用例。见[收尾失败日志](browser-teardown.log)及上表最终日志。
- 一次构建遗漏 `--lib`，其他二进制目标出现依赖元数据错误，见[构建日志](network-build.log)；随后定向 `--lib` 构建通过，见[日志](network-lib-build.log)。本片通过结论不包含完整二进制集合。
- 三组有限样本与工程链路不能证明真人学习收益；这需要后续实际使用证据。
