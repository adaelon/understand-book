# ADR-0138：多 Harness 预构建与 DeepSeek Harness 原生适配

状态：**架构方向已确认；DH0 安装态探针与 DH1 公共合同拆分在途，新增协议细节待冻结，尚未通过完整宿主兼容与构建验收。** 2026-09-26 实施证据见 [兼容报告](../performance/dsh-prebuild-compatibility-20260925.md)。
日期：2026-09-25。变更类型：边界重构与新增宿主适配；不是新增模型 Provider，也不是重写预构建管线。

实施与验收：[切片方案-多Harness预构建与DeepSeekHarness适配](../切片方案-多Harness预构建与DeepSeekHarness适配.md)。

继承 ADR-0084、ADR-0101 的 harness 推理所有权，ADR-0114 至 ADR-0117 的执行器传输、连接与恢复边界，以及 ADR-0121、ADR-0125、ADR-0126、ADR-0128 的生命周期和有界纠错规则。将 ADR-0115 的共享注册拓扑明确限定为 **Codex 适配事实**，不强制其他宿主复制它；Codex 已接受的安全边界不被本次追溯改写。

## 阅读说明与依据范围

本文的“当前事实”以仓库源码为依据；“决策”“新增”“拟议”表示待实施设计。安装与运行证据见“已知限制”。

Understand Book 依据为 2026-09-25 的工作树，HEAD 为 `c2ff3e1d5cfa749496fe8a114b3165eb1c7d67c0`，包含未提交变更。DSH 依据为本地 `E:/allwork/download/agent/deepseek-harness`，HEAD 为 `477b4f420553e8a52c2fbccc464d7561b239c443`；核查时该仓库无已跟踪文件修改。下文 DSH 源码链接固定到此提交，DH0 另记录实际测试的安装版本。

## 背景：需要替换的是宿主，不是构建权威

现有根 Agent 消费 `build.step` 的 `SPAWN_EXECUTORS / WAIT / NEEDS_USER / DONE`；专用执行器通过四个 MCP 工具消费语义输入并提交候选。根 Agent 不能把子代理最终回答当作持久完成事实。[R1–R3]

这套分层可复用，但当前协议检查依赖 Codex TOML 和插件启动资产；transport 校验只接受 `codex_executor_mcp`；transport profile 还出现在公开输入 manifest 和私有 delivery record 中。因此“再复制一份 SKILL.md”或“只改一个 carrier 常量”不足以完成兼容适配。[R4–R8]

DSH 提供原生子代理与插件工具扩展。官方类型将能力检查、实际子代理对象、结果和资源释放分别表达；这些是接入依据，不是当前安装已兼容的证明。[W1–W3]

## §1 统一 Build Engine，模型与 Agent loop 仍归外部 Harness

**决策**：新增 DSH 原生插件，调用同一个已安装 Build Engine；Codex 与 DSH 是并列宿主适配器。

Build Engine 继续拥有 source/LID、BuildPlan、work unit、预算、租约、attempt、候选校验、质量门、产物发布和恢复。Harness 继续拥有模型配置、模型调用、对话循环、子代理运行和取消。插件只在二者之间投影控制动作与执行器工具。

**否决**：不让 Reader 或 Sidecar 直接连接 DeepSeek API；不把 DSH 接入写进 Rust Resident Provider；不复制 Pass1/Pass2/sidecar/BookStructure 的构建实现；不增加一个脱离 DSH 的独立模型调度服务。

**命门**：DSH 插件不依赖模型 API SDK、不读取或保存模型密钥、不自行处理模型推理协议。DSH 可使用其合法配置的模型路由；“使用 DSH”与“使用哪一个 DeepSeek 模型”必须分别记录。关闭或卸载 DSH 插件后，原 Codex 路径仍可使用。[R1]

**何时回头**：只有另行确认“无外部 harness 的预构建产品”时才讨论自带模型运行时，不作为此次失败回退。

## §2 适配器下沉生命周期，但不下沉语义与用户授权

**决策**：DSH 插件代码负责启动、等待、取消专用执行器，以及实时槽位和子代理归属；构建下一动作仍由 `build.step` 决定。

插件可在一个有界的前台控制调用中连续消费确定性动作。达到返回边界时，它返回安全的控制投影或用户选择，不依赖模型记忆保存槽位。不得以“一次工具调用未跑完整本书”为由建立第二套构建阶段状态机。

**否决**：不让 DSH 根 Agent 手工维护 `live_by_slot`；不让插件重新计算阶段完成度、自己 close 阶段或跳过 `NEEDS_USER`；不把模型生成的 `confirmed=true` 当成人类授权；不跨已结束的工具调用悄悄保留无所有者后台任务。

**命门**：用户确认绑定 Engine 返回的精确计划身份和授权范围，使用既有确认入口与回执语义；槽位数来自插件拥有的实时资源和已验证的宿主容量。插件只持有生命周期状态，不保存“任务已完成”的第二份权威账本。默认前台执行，取消沿真实宿主 signal 传播；后台 jobs 不在首版范围。[R1–R3]

准备/确认与执行分成两个控制调用。准备调用从 Engine 取得准确计划，以真实根 `exec.agent` 和 `exec.signal` 调用 `ctx.userQuestions.ask()`；`detail` 展示计划，`intent: { kind: 'plan-review', approve: <批准选项> }` 明确批准项。收到该问题的明确批准后重读计划身份；未漂移才创建 invocation，并先将 `invocation_ref` 返回为宿主可持久保留的控制结果。后续执行调用只继续该引用。`legacy-plan` 当前直接写入 `confirmed/explicit_legacy_command`，本身不是用户作答证据。拒绝、取消、无 answerer 或 delegated caller 均不启动执行器。[R12；W7]

正常让出控制以完成的 handoff 数为界：达到配置上限后停止补位，等待 owned child 终态并 dispose，再返回恢复引用；不会因软时限杀掉正在生成的 child。无 owned child 的外部 WAIT 可立即返回 Engine 的等待原因与建议间隔。用户取消或显式硬 deadline 才中断 child，清理后用原 invocation 重读 Engine；不自动循环重启。DSH 的 timeout policy 只对声明了 `timeoutMs` 的工具设限，未声明时没有该策略的默认超时；控制工具不设置短时固定超时。[W8]

**何时回头**：前台调用边界确实妨碍长构建时，再单独引入 DSH 的正式任务所有权机制；不能把一个悬空 Promise 当成后台执行能力。

## §3 专用子代理、可信调用身份与独占连接

**决策**：首版使用 DSH 原生 one-shot `spawn`；一个 Engine 签发的 handoff 对应一个新执行器和一条独占的 executor MCP stdio 连接。

一个 handoff 可以按 Engine 决定顺序处理其有界 dispatch 内的多个工作单元；不是每个 chunk 都创建子代理。新 handoff、恢复代次或终态后的下一次执行需要新 child 和新连接。dispatch slot 是调度位置，不是可反复复用的 child 池。

**否决**：不让多个执行器共享一条有状态连接；不使用 `fork` 继承根对话；不通过 followup 把第二个 handoff 交给终态 child；不根据 prompt、label、模型填写的 `agent_id` 或字符串角色名授予身份。

**命门**：DSH 工具执行的可信 Agent 对象必须与插件实际创建的 child 绑定，并校验 invocation、启动代次和原始 handoff。未绑定、终态、非专用 child、根 Agent、兄弟 child 和过期实例调用一律不转发到 Engine。四个工具的模型可见参数仍仅包含协议字段，不携带授权 token、私有路径或可伪造角色。[R6–R7；W2–W3]

绑定与工具安装在 `agent/created` 的串行监听器完成。当前 factory 等待这些监听器返回，随后原生 driver 才投递 child prompt；外部 `SubagentStartRequest` 没有可传入的 `setup` 参数。适配器以每次 launch 独立的进程内异步上下文关联创建事件（拟用 Node `AsyncLocalStorage`），核对 `agents.get(id) === agent` 与 `agents.isOwnedBy(id, parent)`，绑定一次确切对象；`start()` 返回后再核对 `run.localAgent`。监听器不得等待 `start()` 返回，否则形成互等。DH0 验证该关联跨 preset 初始化的异步调用仍成立，并覆盖两个重叠 launch 与失败清理。[W3、W9]

**何时回头**：固定版本缺少可靠创建归属或注册时序时，停止该版本的 DSH 支持并报告缺失能力；不自动退回根 Agent 执行或共享连接。

## §4 执行器仅使用四工具，候选不进入父代理

**决策**：DSH 专用执行器只开放 `executor.open`、`executor.input.next`、`executor.generation.start`、`executor.submit_candidate` 的原生工具映射；复用现有语义抽取 prompt、输出 schema 和代码提交门。

DSH 注册名称可以使用宿主合法名称；名称映射由确定性代码完成，不让模型猜测 Codex 的 `mcp__...` 名称。MCP 侧保留规范操作名和封闭参数校验，不能因 DSH 参数 DSL 较宽而接受额外字段。动态 spawn 内容只携带 Engine 签发的原始 handoff ref 及既有允许的有界控制纠错，不附带工作区路径、语义输入或候选。[R5–R6]

**否决**：不把候选作为子代理最终回答、结构化子代理结果、文件、shell 参数、日志或父代理工具结果；不启用 shell、文件写入、额外 MCP、网页工具或继续派生 child 的能力；不默认启用 PTC。

**命门**：输入只经专用 child 的模型可见工具结果；候选只经其 `submit_candidate` 请求进入代码所有的 sink。父代理仅得到允许列表内的控制/生命周期投影。子代理原始 final、流式输出及宿主自动摘要不能绕过这一投影进入父代理上下文。

首版不设置 DSH `outputSchema` 来约束子代理 final：官方当前 in-process 路径可能用额外 capture 工具实现它，不能同时未经证明宣称“精确只有四工具”。由插件依据真实 Engine 响应与宿主终态构造有界结果；模型 final 不构成提交证据。[W1]

`spawn` 不继承父聊天，但 `applyChildComposition` 会接入父 preset。创建监听器在首个 turn 前通过 `agentPresets.select` 选择随包提供的 executor preset，保留原生 spawn 已安装的 persona 与 delegation context；专用 preset 不加载 build skill、自动 compaction 或 tool-result-pruner。四工具注册在精确 child scope，`tools.restrict({ allow: [] })` 屏蔽继承工具，`tools.presentAs('native')` 排除 PTC `run_code`；限制列表不能填写 child 自己注册的四工具名。宿主全局 spill 仍须按完整封套容量处理，DH0 同时检查实际首请求及后续请求，不以注册表或 canonical value 完整代替模型可见完整。[W10–W12]

**何时回头**：将来需要结构化 final 或 PTC 时，作为新的能力矩阵验收，不借此改变候选提交路径。

## §5 公共检查与宿主检查分离

**决策**：拆分 Engine contract 检查与 Harness contract 检查，结果由代码组合。

公共检查包含协议与产物支持、抽取资产、输入/输出门、预算路由和恢复能力。Codex 检查保留自身角色投影、共享 MCP 注册与启动器合同；DSH 检查自身插件依赖、spawn 能力、可信身份、工具约束、连接、取消和容量。

**否决**：不删除旧检查来使 DSH 变绿；不让 DSH 携带一份实际不用的 Codex TOML；不把 Agent 自报“已安装”“兼容”当作探针结果。

**命门**：旧 Codex doctor 输出分支保持兼容；新增结构化宿主检查必须显式版本化。声明支持只在受控探针与对应安装态测试通过后成立；缺证据记未验证或不支持，而非成功。探针使用合成/授权夹具，不领取用户真实构建 handoff。[R4]

**何时回头**：出现第二种非 Codex 宿主后再评估更通用的插件 SDK；此次只抽已经被两种宿主实际复用的合同。

## §6 执行配置显式绑定，协议扩展不伪装成字节不变

**决策**：将宿主执行配置从 Codex 单例常量中解耦，在 invocation 创建前解析，在当前执行期间冻结；公共与私有 session 均使用同一条配置传递链。

配置区分三类信息：实际 harness/适配器版本；模型路由与模型预算；工具传输和 candidate 请求容量。前两者不得从模型叙述猜测，容量不得直接抄产品宣传值。小型配置用显式 id/revision/version 与字段比较，不新增 `profile_digest`、`bootstrap_digest` 等控制摘要。[R2、R7–R9]

每次 spawn 的 `agentOptions` 显式传入 invocation 已冻结的 provider、model、reasoning 配置与输出 `maxTokens`，不能在下一次前台调用时重新继承父代理的最新模型。`maxTokens` 是输出上限，不代表完整上下文容量；实际请求路由须与记录一致。[W10]

**兼容裁决**：

- 保留 `automatic_build_executor_session.v3` 的四工具职责、动作和重放语义；旧 Codex 继续使用现有 V3 输入 manifest、V2 transport profile 及原请求形状。
- DSH 的多宿主 transport 是显式扩展。拟新增独立版本的 transport profile、输入 manifest 和持久 delivery record；由启动检查明确选择该能力分支，不向旧客户端发送新枚举/新记录。
- V3 外层状态机复用不等于完整线协议字节不变。DH1 必须冻结序列化版本矩阵：若现有 V3 公开合同不允许嵌套 manifest 的新版本分支，则升级 DSH 的外层响应封套，而不是修改旧 V3 的含义或放松校验。该选择须在 DSH 实现前写入本 ADR 的协议附表并过 golden 测试。
- 任一旧封闭请求不追加新字段。新 invocation/启动配置需要新增字段时，使用显式新版本；宿主配置不由模型塞进 `executor.open`。

2026-09-26 已实施协议附表：

| 对象 | Codex | DSH |
|---|---|---|
| 执行配置 | `codex_mcp_v3@1` | `dsh_native_v4@1` |
| invocation create/record | V1 legacy 或 V2 显式配置 | V2，另要求 `build_executor_model_runtime.v1` 与 `dsh_build_confirmation.v1`，1–3 并发 |
| bootstrap | 既有 Codex bootstrap | `understand_book_dsh_executor_bootstrap.v1` |
| 安装启动控制协商 | 保持原入口 | 只读 `dsh_build_capabilities.v1`：prepare/read V2、controller V1；工具注册前协商 |
| initialize | 既有合同 | MCP `2025-06-18`；`capabilities.experimental.understand_book_executor` 精确声明并回传 `dsh_build_executor_contract.v1` |
| Session 响应 / transport | V3 / V2 | V4 / V3 |
| 公开 input manifest | V3 | V4，绑定 profile |
| 公共 delivery record | V3 | V4，绑定 profile |
| 公共 opaque handoff | 既有 V4，历史 V3 可读 | V5，绑定 profile 与 Session V4 |
| generation-start record | V2，缓存 Session V3 响应 | V3，缓存 Session V4 响应 |
| 四工具请求 | open V3、input.next V4、generation.start V3、submit V3 | 复用同一封闭请求，不增加模型可选 profile 字段 |
| 私有 delivery | 原 V3 路径保留 | V4 decoder 已有，DSH 私有创建/运行入口拒绝，留给 DH7 |

新连接在初始化协商成功前不能 list/call；旧入口拒绝 DSH 记录。确认记录绑定计划 id/revision/digest、真实根会话、问题与批准答案；Engine 在创建前重读计划与源指纹。准备调用生成的标准计划确认状态不替代宿主人类回答。运行期间每次启动均读取 invocation 保存的模型快照。

**否决**：不原地把旧 profile 的 `carrier` 改成 DSH；不把只有类型放宽的改动当作全链路适配；不让同一打开的 session 中途切 profile 或模型；不静默迁移历史控制记录。

**命门**：plan/preflight → descriptor/router → dispatch/lease → handoff → delivery → generation → candidate gate → quality/publication 对同一执行配置达成一致。源、LID、语义 prompt、候选 schema 与产物身份的既有规则不因换宿主被放松。预算变化触发重新评估和必要确认，而不是自动扩大授权。

## §7 模型预算与传输分片分别验收

**决策**：输入路由考虑完整模型上下文，传输配置考虑实际序列化通道，二者不能相互替代。

必须计入角色说明、工具 schema、语义 prompt、输入、全局目录、输出预留，以及同一个 child 顺序执行多个工作单元时已经积累的工具历史。单条工具结果小，不证明整个上下文装得下。不能靠 DSH 自动摘要、结果截断或附件替换维持“成功交付”的表象。

**否决**：不通过新建 LID、修改源正文、删证据、缩短抽取 prompt 或降低 schema 来适配宿主；不任意扩大 candidate 输出上限；不让插件另写语义切分器。

**命门**：分片可不同，但完整语义输入必须一致。超限在进入生成前由现有路由/预算门处理；需要变更 work unit 或 dispatch 形状时，经既有 Engine 规划与恢复规则完成。candidate 必须完整到达现有 sink；不以文件路径 fallback 绕过工具参数上限。[R7–R9]

**何时回头**：如确有必要优化模型专用 prompt 或工作单元策略，另立语义策略变更并进行质量对照；不与首次宿主接入混做。

## §8 重试、取消、失联与跨宿主接手

**决策**：语义重试、bootstrap 恢复、连接恢复与宿主故障分开；Engine 是唯一恢复裁决者。

插件精确记录它拥有的 launch 和真实 MCP 操作观察，保持原错误类别。没有发生语义生成不能记成语义失败；提交后丢失响应不能直接再生成。先重读持久状态，再决定等待、重试或用户边界。

**否决**：不将所有 DSH 异常改写为 `bootstrap_unavailable`；不把 context overflow 当 schema 错误；不凭 child final 置任务完成；不在插件退出时强删 lease、篡改 attempt 或重写 committed artifact。

**命门**：先完成先补位；旧回调不能释放替代 child 的槽位；`NEEDS_USER` 不丢其他 slot 和待上报观察。cancel/卸载必须停止新启动，传播 signal，等待所有已拥有 child 的资源释放；提交结果不确定时交由 Engine 恢复。故障映射不足时显式返回有界未映射宿主诊断，不为套用旧恢复分支伪造错误码。[R3、R6]

DH5 实现使用 `dsh_build_prepare.v2` / `dsh_build_invocation_read.v2` 传递确认的并发上限；旧 V1 字段集合保持。`dsh_build_controller.v1` 在 Engine registry 内保存 controller 的 PID/启动身份和每次 launch 的 `dsh_executor_observation.v1`；只在确证旧 PID 退出后接手，未知活性保持阻塞。该记录不替代工作区 lease/attempt。原生 spawn 无实时空闲配额接口，因此取实测三并发上限，扣除实际 Agent 注册表及插件在途创建；容量来源明确为保守估算。

跨宿主接手是“新运行复用有效构建成果”，不是把 Codex 的活动连接搬到 DSH。首版不承诺活动 invocation 原地切宿主。先停止/核实旧 owner，再以新的已验证执行配置评估计划；必要时重新确认，创建新 invocation，复用由 Engine 判定仍有效的产物。未处理的活动 lease 不得抢占。[R2–R3]

## §9 数据边界与能力证明按宿主分别表述

**决策**：区分父代理上下文隔离、插件调用身份限制、宿主日志保留和模型服务外传四个边界。

Codex 保持既有 `caller_role_authenticated=false` 与共享注册事实，靠正常路径非调用合同和 trace 验收；DSH 在插件工具入口验证真实 child 身份。不得将 DSH 的能力性门禁追溯描述为 Codex 已有保证，也不把同进程插件边界宣传为恶意插件/操作系统级沙箱。[R2、R6；W2]

**否决**：不默认导出全文 trace 到 LangSmith；不记录候选、语义 chunk、凭据、私有路径或隐藏推理到本插件诊断；不声称“未进入父代理”等于“不在 DSH 会话日志中”。

**命门**：文档说明正文和候选会经 DSH 的选定模型路由处理，并可能保留在 DSH child 会话中。插件诊断只保存必要的控制身份、阶段、错误码、时长和有来源的用量；未知 usage 保留未知，不能把估算写成实耗。全局 trace 的日志保留、删除与外传由宿主策略分别管理。[W4]

**何时回头**：要求正文绝不落宿主日志的部署不能使用未经专门验证的普通 DSH 会话；不靠隐藏 UI 冒充满足要求。

## §10 首版范围、分发与回滚

**决策**：首版交付已有有效工作区上的 `standard_deep` 构建与恢复，明确选择 Pass2，单执行器通过后再开放当前 `1..3` 并发范围。

首版要跑完整个已确认的标准阶段闭包，而不是仅演示 Pass1。自然语言目标规划、Reader-private 定制产物、跨平台 Build Engine 分发、PTC、continuable child、后台 jobs 和混合宿主同时执行均不属于首版承诺。

这只是 DSH 新适配器的发布能力范围，不删除统一引擎的私有产物能力，不改变 Codex 已有功能。遇到超出 DSH 首版范围的计划，在启动前拒绝并说明能力缺口，不能静默删掉私有产物。[R2–R3]

新增包位置拟为 `packages/dsh-plugin/`。只分发插件、角色与控制桥，调用已安装的 Build Engine；不复制 Engine 源码作为第二份 npm 运行时。控制进程和 executor MCP 必须解析到同一个 Engine 安装、driver registry root 与当前授权工作区；DSH、Engine 和工作区在首版位于同一受支持执行环境，不把远端 Reader 地址当作远端构建通道。现有发布脚本明确面向 Windows x64，首个正式验收优先覆盖 Windows DSH Desktop；CLI 在其独立合法 profile 中另验。未验证的平台不宣称支持。[R10；W4]

**否决**：不修改用户自定义 Codex TOML；不通过公共 DSH CLI 修改 Desktop 保留 profile；不为修复安装清空 `.dsh`、替换 pnpm 管理的包或要求无差别提升权限；不静默安装 Node/Bun/Cargo 作为缺失 Engine 的回退。

**命门**：DSH 插件可独立禁用或卸载；已发布产物不因卸载删除。旧引擎不能消费新控制记录时明确拒绝，不把“回滚入口”宣传为“旧二进制可继续新记录”。回滚应保留新记录，使用仍受支持的 Codex/Engine 组合创建新运行并按既有门禁复用产物。

## 目标依赖方向

```text
DSH 插件 → DSH 官方宿主能力
    │
    ├─ 控制适配 → 已安装 Build Engine 的计划 / build.step
    └─ child 专用桥 → 独占 executor MCP → 同一 Session 核心

Codex 插件 → Codex 宿主能力 → 同一个 Build Engine / Session 核心

Core 不反向依赖 DSH；Rust Resident 不承担这条预构建运行链。
```

## 发布必须证明的结果

DH0–DH6 全部通过才可声称首版支持；DH7 是独立扩展。必须同时具备确定性回归、实际 DSH 工具/子代理探针、真实模型标准构建，以及安装态中断恢复证据。模型结果允许语义差异；控制协议、引用门禁和产物有效性不允许因宿主不同而放松。

## 已知限制

DH6 的正式包与 Windows Desktop 安装生命周期已完成：`0.1.0-rc.2` tarball、独立编译 Engine、官方插件页安装/升级/卸载、完整 Desktop 重启，以及包内取消/卸载/重新启用。环境要求与证据见 [DH6 安装报告](../performance/dsh-dh6-install-20260926.md)。真实模型质量、带业务进度的 Desktop 重启恢复和回滚仍未验收。

DH1–DH4 的源码实现和安装态确定性标准闭环已落地，证据见[兼容记录](../performance/dsh-prebuild-compatibility-20260925.md)。DH0 人工“要求修改”仍待实际操作；全文展示通过的组合为官方 `0.1.7-rc.2` 加 SidebarRight 修复。论文已验证计划/预算边界，完整闭环证据来自技术书。DH5 并发、故障、取消及受控接手的确定性证据见 [DH5 报告](../performance/dsh-dh5-20260926.md)。真实模型仅完成小型四工具往返，完整构建尚未验收。CLI/自定义 profile 若存在全局 compaction 或裁剪策略，须另行验证兼容性。

## 仓库证据索引

以下行号为此次及紧邻本次的实时只读返回位置，后续修改后应按符号定位复核。

| 编号 | 证据 |
|---|---|
| R1 | `docs/adr/0101-deterministic-prebuild-protocol-ownership-and-codex-semantic-boundary.md:12–56`：确定性所有权、四动作、harness 推理与 mailbox 边界。 |
| R2 | `docs/adr/0115-root-shared-executor-mcp-and-subagent-inheritance.md:9–100,117–150`：Codex 共享注册、非角色鉴权、HERO 与恢复边界。 |
| R3 | `plugins/understand-book/skills/build/SKILL.md:31–45,63–125,127–180,182–229,234–326`：规划、外部动作、纠错、槽位、提交与完成权威。 |
| R4 | `skills/build/automatic-build.ts:3007–3072`：doctor 对 Codex 资产的组合依赖。 |
| R5 | `plugins/understand-book/assets/codex-agents/understand-book-executor.toml:13–20,23–91,104–140`：角色、Codex 工具映射、chunk 与 final 合同。 |
| R6 | `packages/core/src/build-executor-tool-adapter.ts:24–34,93–154,323–383`；`packages/core/src/build-executor-connection-capability.ts:76–88,432–520`：封闭四工具、真实安全边界、连接状态与 ref 约束。 |
| R7 | `packages/core/src/executor-transport.ts:4–14,141–146,174–233`：carrier 单值、传输与 batch 上限。 |
| R8 | `packages/core/src/automatic-build-executor-session.ts:541–584,748–825`：公共/私有持久记录和公开 manifest 直接包含 transport profile。 |
| R9 | `skills/build/automatic-build-driver.ts:223–261`；`packages/core/src/automatic-build-budget.ts:57–61,646`：invocation、provenance 和预算校验的当前合同。 |
| R10 | `apps/desktop/scripts/build-sidecar.mjs:9,22–34`：已安装 Engine 的 Windows x64 编译入口。 |
| R11 | `packages/core/package.json:1–8`；`package.json:7–17`；本次 `packages/core/test` 文件清单：测试与 typecheck 入口。 |
| R12 | `packages/core/src/build-intent-controller.ts:mapLegacyBuildInvocation`；`plugins/understand-book/skills/build/SKILL.md` 的标准计划确认步骤：legacy 函数直接标记 confirmed，人类确认由宿主控制流程承接。 |

## DSH 官方依据

核查日期为 2026-09-25。链接固定到本地源码提交；源码能力与安装态证据分列。

- [W1：Subagent 合同](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/docs/subsystems/subagent.md)：能力检查、spawn/fork、工具限制、结构化结果及作用域。
- [W2：工具扩展合同](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/docs/cookbook/adding-a-tool.md)：执行身份、参数校验、canonical value、signal、PTC 与输出边界。
- [W3：Subagent 类型](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/subagent/subagent/src/types.ts)：`SubagentStartRequest`、`SubagentRun.localAgent/result/dispose`。
- [W4：架构与生命周期](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/docs/architecture.md)：`agent/created`、日志与模型上下文、Desktop/CLI 边界。
- [W5：Subagent 服务实现](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/subagent/subagent/src/index.ts)：能力检查、启动发布、父子目录与错误清理。
- [W6：原生 spawn 实现](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/subagent/subagent-spawn-in-process/src/index.ts)：原生 spawn Provider 的当前入口，具体时序由 DH0 固定版本核实。
- [W7：人类问答服务](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/interaction/user-questions/src/index.ts)与[问题类型](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/interaction/user-questions/src/types.ts)：`ask`、真实根身份、answerer、取消及 `plan-review/detail`。
- [W8：工具超时策略](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/guard/timeout-policy/src/index.ts)：没有 `timeoutMs` 时不设 deadline，超时替换结果发生在协作清理之后。
- [W9：原生 one-shot driver](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/subagent/subagent-in-process-driver/src/index.ts)、[Agent factory](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/core/agent-loop/src/index.ts)与[Agent registry](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/core/agent/src/index.ts)：`startInProcessRun`、`setupAndPublish`、串行 `announce` 与 `isOwnedBy`。
- [W10：child composition](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/subagent/subagent/src/child-agent.ts)与[工具作用域](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/core/tools/src/index.ts)：父 preset/模型继承、`restrict` 的继承层与自有层差别、`presentAs('native')`。
- [W11：preset 选择](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/preset/agent-preset-registry/src/index.ts)与[标准 preset](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/bundle/web-app/presets/standard.patch.yml)：`select` 只允许首个 turn 前切换，标准组合包含 compaction 与 tool-result-pruner。
- [W12：spill 策略](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/spill/spill-policy/src/index.ts)与[自动 compaction](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/compaction/compaction-basic/src/index.ts)：Native 结果替换、逐步压力检查与工具历史裁剪。
