# 切片方案：多 Harness 预构建与 DeepSeek Harness 适配

状态：**DH1–DH5 源码实现与确定性验收已完成；DH0 剩人工“要求修改”补验。DH6 尚未实施，不作发布支持声明。** 更新：2026-09-26。实际证据见 [兼容报告](performance/dsh-prebuild-compatibility-20260925.md)，续做入口见 [Harness checkpoint](../SESSION_CHECKPOINT_DSH.md)。
决策依据：[ADR-0138](adr/0138-multi-harness-prebuild-and-deepseek-harness-adapter.md)。

首版按 DH0–DH6 实施，其中 DH2 分成 DH2a、DH2b 两次独立验收；DH7 的自然语言目标/私有产物规划是后续扩展。本文新增文件、适配器接口和脚本均为拟议名称；DSH 已有 API 以文末固定源码依据为准。

## 0. 目标、范围与实现纪律

用户最终能在 DSH 中对已有有效工作区发起标准预构建、确认准确的 Pass2/预算/阶段闭包，然后由 DSH 提供模型与 child loop、Understand Book 提供构建权威，最终完成计划或真实停在用户/外部故障边界。

首版不直连 DeepSeek API，不改 Rust Resident，不另建任务数据库，不复制构建管线，不改变 LID/源正文/抽取 prompt/candidate schema，不用 PTC、不用 continuable child，不支持无所有者的后台执行，也不承诺活动 Codex invocation 原地热切 DSH。

### 0.1 仓库与宿主基线

Understand Book 实时工作树观察 HEAD：`c2ff3e1d5cfa749496fe8a114b3165eb1c7d67c0`。该值不覆盖未提交/未跟踪的批准文件。实施开始时重新记录 HEAD、工作树变更范围与测试基线；不撤销其他未提交工作。

DSH 源码基线为本地 `E:/allwork/download/agent/deepseek-harness` 的提交 `477b4f420553e8a52c2fbccc464d7561b239c443`，核查时无已跟踪文件修改。该提交的版本合并说明为 `dsh-0.1.7-rc.2`，不等于本机已安装版本。DH0 另固定实际安装来源、平台、架构、插件依赖、Provider 路由和模型。

### 0.2 不可降低的不变量

| ID | 不变量 | 主要验收 |
|---|---|---|
| I1 | Engine 独占计划、任务、预算、租约、候选接纳、发布和完成权威。 | DH4/DH5 的脚本化故障测试。 |
| I2 | Harness 独占模型调用与 Agent loop；插件不拥有模型凭据。 | 依赖/代码扫描、真实 Provider 路由记录。 |
| I3 | 根上下文不出现语义输入、candidate、私有产物正文。 | 父历史、工具结果、自动通知和诊断的哨兵扫描。 |
| I4 | 一个新 handoff 对应一个新 child 和独占 stdio 连接。 | 并发、终态、新代次和交叉 ref 测试。 |
| I5 | DSH 四工具的调用者必须为插件创建且当前有效的真实 child。 | 根/兄弟/伪造身份/过期对象负例。 |
| I6 | 未生成、生成失败、候选被拒、提交不确定、持久完成不能混为一谈。 | 分阶段故障注入与 durable state 对照。 |
| I7 | 已打开 session 不切模型、传输 profile 或语义策略。 | 配置热更、重启和串接旧记录测试。 |
| I8 | 历史记录不原地改写；未知版本失败关闭。 | Codex golden、旧记录恢复和新旧交叉运行。 |
| I9 | Codex 的共享注册和非角色鉴权事实保持如实表达。 | 原有 Codex 安全证据回归。 |
| I10 | 实际模型输入和提交内容完整，不靠截断、摘要或文件路径 fallback。 | Native 全链路容量探针和长 dispatch。 |

### 0.3 总体实施顺序

```text
DH0 固定兼容基线与探针
  → DH1 合同/doctor/版本边界
  → DH2a 显式配置贯穿，保持 Codex 行为
  → DH2b DSH 配置、新记录与容量路由
  → DH3 DSH 身份隔离与 executor bridge
  → DH4 单执行器标准构建闭环
  → DH5 并发、取消、故障与恢复
  → DH6 安装态发布、质量验收与回滚

DH7 自然语言目标和私有产物扩展（独立，首版后）
```

每片先建立可失败测试，再改实现；切片验收必须说明使用了 fake、脚本化 DSH、真实 DSH 还是安装包。不能用 mocked probe 或只匹配 skill 文本的测试替代安装态证明。

## 1. 当前落点与拟议新增结构

### 1.1 当前需要修改的真实代码落点

| 当前文件/模块 | 这次的修改责任 |
|---|---|
| `skills/build/automatic-build.ts` | 分离公共 doctor 与宿主检查；选择执行配置；不重写阶段算法。 |
| `skills/build/automatic-build-driver.ts` | 版本化 invocation 绑定、宿主故障消费与现有四动作接入；不新增第二份完成状态。 |
| `skills/build/sidecar-entry.ts` | 暴露版本化控制/启动入口，保持旧 CLI 兼容。 |
| `skills/build/build-executor-mcp.ts` | 按已解析的启动与 handoff 绑定选择合同和限额；保留连接状态机。 |
| `packages/core/src/build-executor-tool-contract.ts` / `build-executor-tool-adapter.ts` | 四操作 schema 单源与请求校验；Codex 注册验证迁至适配分支。 |
| `packages/core/src/build-executor-connection-capability.ts` | 分离宿主 bootstrap 验证与通用 ref/phase/ordinal 状态机。 |
| `packages/core/src/executor-transport.ts` | 版本化多宿主 profile，保留旧 Codex 常量与校验分支。 |
| `packages/core/src/automatic-build-executor-session.ts` | 公共和私有 session 的配置加载、交付、重放、提交容量与持久记录兼容。 |
| `automatic-build-budget.ts`、`automatic-build-dispatch.ts`、`automatic-build-dispatch-runtime.ts`、`automatic-build-lease.ts`、`automatic-build-policy-generation.ts`、`automatic-build-quality.ts` | 消费统一执行配置，清除当前调用链里隐式回落 Codex 的行为。 |
| `book-structure.ts`、`book-structure-generation.ts`、`book-structure-relation-routing.ts`、`build-orchestrator.ts` | 覆盖 BookStructure 各分支的路由与 transport 传递；不能只验证 Pass1。 |
| `plugins/understand-book/skills/build/SKILL.md` 与 Codex executor 资产 | 保留原宿主行为；抽取可共用说明时保持语义 prompt 不变。 |
| `apps/desktop/scripts/build-sidecar.mjs` | 新合同随现有 Windows Engine 发布；不把 DSH npm 包当作第二份 Engine。 |

上表来自已定位的调用点，并非穷尽清单。DH1/DH2 必须重新搜索 `CODEX_EXECUTOR_TRANSPORT_PROFILE_V2`、`CODEX_EXECUTOR_DELIVERY_BATCH_LIMIT_V1`、`codex_executor_mcp`、`root_shared`、`codex_conversation` 与 session/profile 版本字符串，检查 `skills/` 和所有当前批准源码；逐项说明保留或迁移理由。不能只搜索 `packages/core/src` 后宣称完整覆盖。[ADR 证据 R3–R10]

### 1.2 新增结构（拟议）

```text
packages/dsh-plugin/
  package.json
  src/
    index.ts                 # 官方插件注册/卸载
    capability-check.ts      # 实际宿主能力与安装配置检查
    build-control.ts         # 有界前台控制调用与用户边界
    engine-client.ts         # 已安装 Engine 的 argv/stdin 控制桥
    executor-host.ts          # spawn/slot/cancel/dispose 的宿主投影
    executor-bridge.ts        # child → 独占 MCP 连接
    executor-role.ts          # DSH 专属 bootstrap 投影
    diagnostics.ts            # 允许列表诊断，非构建权威
  assets/
    executor-preset.yml        # 专用 child composition，随正式包发布
  test/
    capability.test.ts
    binding.test.ts
    bridge.test.ts
    control.test.ts
    lifecycle.test.ts
    privacy.test.ts
    installed.test.ts

packages/core/src/
  build-harness-contract.ts   # 宿主/执行配置的中立合同
  build-execution-profile.ts  # 已发布 profile 注册与解析
  # 模块可按 DH1 最小依赖合并，不为每个数据对象单开包。

skills/build/
  build-engine-doctor.ts      # 公共 Engine 检查
  codex-build-doctor.ts       # 现有 Codex 资产检查投影

scripts/
  verify-dsh-prebuild-release.mjs  # DH6 新增发布门

docs/
  performance/dsh-prebuild-compatibility-20260925.md
  performance/dsh-prebuild-installed-<实际验收日期>.md
```

包名拟为 `@understand-book/dsh-plugin`，实际发布名须在 DH6 核查，不在本设计中声称已注册。插件依赖的官方 DSH 包及 peerDependencies 以固定版本的公开导出为准，不能凭路径猜 npm 包名；也不打包一套私有 DSH 核心依赖导致运行时身份分裂。

## 2. 关键接口与版本合同

### 2.1 三层入口

**人类/根 Agent 控制面。** DSH 插件封装准备标准计划、确认后的执行与恢复。初始必要输入为工作区/源目标、Pass2 选择和预算约束。计划投影完全使用 Engine 结果；用户批准的是准确投影，不是笼统“全量构建”。展示的错误、选项和完成摘要不由插件补写语义结论。

**宿主生命周期面。** 插件内部接收 Engine 的 launch 引用，转换为 DSH child；向 Engine 提供可验证范围内的当前容量和有界故障观察。内部接口不成为模型可调用的“任意启动 child/切换 profile”入口。

**专用 executor 面。** 子代理使用现有四操作。MCP 侧参数白名单继续由 Engine 执行；工具正文是书籍数据，不能因为其中存在指令而改变角色、工具或提交通道。

控制面分为两个调用（以下为拟议接口）：

```text
prepare_and_confirm(target, pass2, budget)
  → Engine 准确计划投影
  → userQuestions.ask({ agent: exec.agent, signal: exec.signal,
      questions: [{ id, question: 确认本次构建计划, detail: 准确投影,
                    options: [{ label: 批准 }, { label: 拒绝 }],
                    intent: { kind: 'plan-review', approve: 批准 } }] })
  → 核对回答 id 与明确批准 → 重读计划身份 → 创建 invocation
  → 返回 invocation_ref（宿主先持久记录这个控制结果）

run(invocation_ref)
  → 消费 build.step → 返回同一 invocation_ref + Engine 控制投影
```

只有精确问题的单一批准选项才批准计划；自由文本、空答、拒绝或混合答案不自动转成批准。`ask()` 必须携带真实且当前存活的根 Agent；无服务/answerer、取消、`DELEGATED_CALLER` 或计划漂移均不创建 invocation。`legacy-plan` 的 `confirmed/explicit_legacy_command` 由代码直接生成，不能拿它替代上述交互。DSH 的新 invocation 记录由插件内部创建调用关联计划身份、根 session 和实际批准选择，作为恢复时可读的确认记录；这些字段不开放为模型自报参数，也不另建授权数据库。准备调用只返回可恢复的控制身份，不启动 child，避免长执行被取消后连运行引用都未留存。[ADR R12；W7]

运行遇 `NEEDS_USER` 时先停止补位、收尾已有 child、上报待观察，再重读 Engine 的当前 request/choice；向根 Agent 对应的人类问答服务展示该请求，明确答案才映射为原 `request_id/choice_id`。清理期间 request 已变化时废弃旧问题。取消后不记回答；中断恢复沿用原 invocation，不能重新运行 `legacy-plan` 来代替恢复。

### 2.2 内部接口示意（不是已经存在的 API）

```ts
interface HarnessLaunch {
  // 只能由当前 build.step 输出进入该接口。
  opaque_handoff_ref: string;
  dispatch_slot_ref?: string;
}

interface OwnedExecutor {
  launch_id: string;               // 插件本次启动实例，不作为业务完成身份
  child_id: string;                // 来自真实 DSH run
  dispose(): Promise<void>;        // 幂等；清理完成后才释放拥有关系
  terminal: Promise<HostTerminalObservation>;
}

interface PrebuildHarnessAdapter {
  observeCapacity(): Promise<number>; // 经上限裁剪后为 0..3
  launch(input: HarnessLaunch, signal: AbortSignal): Promise<OwnedExecutor>;
  disposeOwned(): Promise<void>;
}
```

具体 DSH 调用使用其真实 `parent: Agent`、`ContentBlock[]` prompt 和 `signal` 合同，不照抄简化网页示例。`persona/toolFilter/agentOptions/maxDepth` 只有在所选 provider 宣告并实测支持时使用。`maxDepth` 的含义是绝对深度上限，不能误设为 0 后阻止合法 child；执行器禁止继续派生也由工具能力限制兑现。[W1、W3、W5]

本地原生 spawn 的 `SubagentStartRequest` 不暴露 factory `setup`。首版拟用每次 launch 独立的 `AsyncLocalStorage` 上下文关联串行 `agent/created`，监听器核对真实对象与 `agents.isOwnedBy(id, parent)` 后，在首个 turn 前选择 executor preset、注册 child 自有四工具、安装 `restrict({ allow: [] })` 和 `presentAs('native')`，再返回。监听器不等待外层 `start()`；外层返回后核对 `run.localAgent`。此处异步关联与 preset 切换须通过 DH0 真实宿主探针，不靠 label 或 parent 相同认领 child。[W9–W11]

`HostTerminalObservation` 是插件内部观察，不等于 `automatic_build_executor_lifecycle`。只有能从真实 MCP DONE/错误构造的字段才能转成对应协议对象；其他宿主故障独立表达，不用类型强转伪装成 Engine 错误。

### 2.3 执行配置与可信来源

拟议的执行配置包含：`profile_id/profile_revision`、`harness_kind`、`adapter_release`、已选择的传输/manifest 能力、完整模型预算和观察来源。可另附实际 DSH release、Provider/model、reasoning 配置及是否已验证。

字段在创建 invocation 前由 Engine 已发布配置和插件实际宿主检查组合，写入 **Engine 所有的执行记录**。根 Agent 只能选择已公开支持的配置，不可提交任意更大的容量数字。模型密钥永不进入记录。

不创建以 hash 包装的小控制身份；使用显式字段和版本。具体内容哈希仅沿用现有语义输入/产物与幂等用途。传输变化本身不应成为语义 freshness 的新否决条件；若改变了实际语义输入/策略，仍按现有规则重新评估。

### 2.4 版本兼容矩阵：DH1 必须先冻结

| 对象 | 旧 Codex 分支 | 新宿主扩展要求 |
|---|---|---|
| `build.step` 四动作 | 不变。 | 不增设插件私有的第五种构建成功状态。 |
| invocation create/record | V1 继续按旧 Codex 规则解析，拒绝新增字段；V2 Codex 只接受配置身份。 | V2 DSH 要求 execution_profile、model_runtime 与 confirmation，保存真实根会话和批准计划身份；创建前重读计划与源指纹。 |
| executor 四工具请求 | 现有版本与封闭 schema 保留。 | profile 不来自模型工具参数；如参数必须变化，新增请求版本。 |
| Session V3 | Codex 旧响应形状和重放行为保留。 | DSH 实际输出 Session V4；V3/V4 连接状态共享实现但互拒另一代响应。 |
| transport profile | 当前 `executor_transport_profile.v2` 和 Codex 数值保持原义。 | V3 decoder 已实现，carrier=`dsh_native_mcp`、session=`automatic_build_executor_session.v4`。旧 V2 decoder 明确拒绝 V3。 |
| input manifest | 当前 V3 含 Codex profile 的分支保留。 | 实际输出 V4，包含显式 execution_profile 和匹配的 transport V3；正文 segment 顺序/字段/计数均封闭校验。 |
| delivery session record | 历史公共/私有 V3 均按旧配置解释。 | 公共 V4 实际发出并恢复；私有 V4 decoder 保留但 DSH 私有运行拒绝。旧入口拒绝新记录。 |
| opaque handoff | 公共当前 V4、历史 V3 与私有 V1 保持原义。 | DSH V5 已绑定执行配置与 Session V4，实际发行与恢复；不复用旧 handoff V4 的版本含义。 |
| generation-start record | V2，缓存 V3 响应。 | V3，缓存 V4 响应，按所属 delivery 校验。 |
| bootstrap / doctor | 原 Codex 资产与旧版本输出保持可用。 | DSH bootstrap V1、公共 Engine doctor；MCP initialize 显式协商 `dsh_build_executor_contract.v1`，未协商不能调用工具。 |
| 已发布语义产物 | schema、source、LID 与 freshness 门不变。 | 已接纳产物不因只有 harness 字符串变化而被一概作废。 |

registry 的固定身份为 `codex_mcp_v3@1` / `dsh_native_v4@1`；容量数字归 registry 所有。DSH 每批 1 chunk、MCP 序列化上限 16384 字节，单结果/候选请求仍保留现有保守上限。模型快照、真实确认关联和初始化协商已接入控制/执行路径。当前确定性构建使用官方 DSH 安装包依赖与源码 Engine；独立安装与真实模型标准构建验收属于 DH6。

### 2.5 传输能力的完整链路

```text
已发布执行配置
  → 计划估算 / preflight
  → work unit 和模型预算路由
  → dispatch 与 lease 校验
  → handoff 的执行绑定
  → MCP 响应序列化 / DSH Native 呈现
  → 模型实际输入
  → candidate 工具参数 / MCP 请求
  → 现有 candidate gate、writer、quality 与 publication
```

DH2 必须消除中间路径隐式回落到 Codex 常量的情况，包含公共/私有 session、BookStructure local/selection/relation/stitch 分支和恢复读取。profile 只在入口选择一次；消费者校验它与当前运行匹配，而不是每处再次凭环境变量选择。

### 2.6 前台调用与恢复

正常让出控制采用配置的完成 handoff 数上限。达到上限后停止补位，等待所有 owned child 终态并 dispose，消费已收到的宿主观察，返回同一个 `invocation_ref` 和 Engine 控制投影。无 owned child 的外部 WAIT 可以携带原等待原因与建议间隔返回；这些是控制调用的返回条件，不新增 Engine 完成状态。

DSH timeout policy 仅对工具声明的 `timeoutMs` 启用 deadline；控制工具默认不设短时硬超时。用户取消、宿主退出或部署显式配置的硬 deadline 才中断当前 child，清理后由原 invocation 恢复；不能将硬超时当正常分页而自动无限续跑。安装环境另有调用时限时，DH0 必须证明单次 handoff 能在该约束下完成，否则该组合暂不支持。[W8]

输入完整性同时覆盖当前结果和历史：child 从当前标准 preset 继承自动 compaction 与 tool-result-pruner，宿主 spill 可以保留 canonical value 却替换 Native 文本。专用 executor preset 排除前两者；按后者的实际模型可见封套设定完整分片上限。CLI/自定义 profile 有全局压缩策略时单独确认其兼容性，不能只卸载 child 的同名插件后宣称不受影响。[W10–W12]

## DH0 — 固定 DSH 兼容基线与最小探针

**目标**：在不修改语义构建算法、不使用用户真实书籍的前提下，证明宿主能承载所需身份、角色、工具和生命周期。

**输入**：ADR §1–§5；DSH 固定源码 W1–W12；当前安装包与 Engine 版本。实际安装不可用的环境可做源码环境探针，但安装态证据未取得前不能标记本片通过。

**修改/新增**：`packages/dsh-plugin` 最小插件骨架、`capability-check.ts`、合成四工具测试服务器；`docs/performance/dsh-prebuild-compatibility-20260925.md`。探针只在专用测试配置里挂载，生产不新增模型可见健康检查工具。

**实施**：

1. 固定 OS/架构、DSH Desktop 或 CLI、安装来源、runtime/plugin 包版本与源码提交；记录实际 provider/model 路由，但不复制凭据。
2. 检查原生 `spawn` 的能力描述与请求签名，使用真实 `parent` 和 cancellation signal；确认不是 `fork` 或远程 provider。
3. 在真实 standard preset 和 PTC/both 父配置下验证 child 首请求恰好四工具、Native 呈现且不继承根聊天；根、普通 child 和兄弟 child 无法有效调用。验证 executor preset 切换不改变父配置，后续请求未被 compaction/pruner 改写。
4. 根据 §2.2 验证“串行 agent/created 完成 → child prompt → start 返回”的创建链，强制第一工具早于调用方取得 handle。验证异步上下文跨 await 的精确关联、未关联创建零绑定、两个重叠 launch 各自只绑定一个 Agent；监听器不等待 start。
5. 验证 `result`、发布前拒绝、发布后错误、`dispose()` 和取消，确认没有遗留 child/MCP 进程。探针至少覆盖两次同时启动与一次中途失败。
6. 探测 Native 工具结果和参数的完整链路；记录 UTF-8 字节、封套成本、实际模型可见内容、工具名限制、JSON schema 子集和自动截断/附件行为。
7. 验证根 `userQuestions.ask()` 的完整计划展示、明确批准、拒绝、取消、无 answerer 与 delegated caller；确认宿主真实调用时限及未声明 `timeoutMs` 的行为，不能用模拟批准替代安装态交互。

**测试**：真实 DSH 原语探针 + 可复放的固定结果；不需要昂贵书籍构建。协议往返可使用 fake LLM，最终仍需至少一次真实模型的工具往返证明 model-visible 路径。

**Do not**：不启用 `outputSchema` 的额外 capture 工具来绕过四工具约束；不写“0 根调用”替代根主动调用负例；不在生产环境拿真实 handoff 做 doctor。

**完成条件**：固定版本上每项能力都有 pass/fail/unsupported 和实际证据；未解决创建绑定、精确工具或传输问题时阻断 DH3，不降低隔离要求。

**回退**：移除测试插件/测试 profile；不触碰用户真实工作区、Codex 配置或 DSH Desktop 保留 profile。

## DH1 — 公共合同、宿主检查与版本边界

**目标**：没有 Codex TOML 的 DSH 环境可以通过自己的真实检查，但原 Codex 检查与安全事实不被删除。

**依赖**：DH0 的能力结论；允许先做纯函数重构，不允许跳过 probe 后宣称宿主可用。

**修改**：`automatic-build.ts`、`build-executor-tool-adapter.ts`、`build-executor-connection-capability.ts`、`sidecar-entry.ts`；新增中立合同和公共/Codex doctor 模块。

**实施**：

1. 将公共 Engine 检查抽出，保持旧 Codex doctor 外部输出；宿主验证从同一公共结果组合，不能复制公共检查实现。
2. 分离四工具操作/schema、纯连接状态机、Codex 注册/TOML 检查。DSH 包不导入 Codex 指令资产。
3. 冻结 §2.4 的具体版本/协商矩阵，新增严格 decoder 和 golden；包括公开 manifest、私有记录，而非只改 TypeScript 类型。
4. 为新的 invocation 创建路径增加已发布 profile 选择。旧 V1 缺配置时只能落在明确的 Codex legacy 分支，不能“按当前安装的宿主”猜历史含义。
5. 旧安全报告继续写 shared registration 与 non-authenticated role；DSH 报告自己的实际边界，不复用同一个布尔值掩盖差异。

**复用测试**：`automatic-build-release-v3.test.ts`、`automatic-build-driver.test.ts`、`codex-executor-agent.test.ts`、`codex-build-skill-v2.test.ts`、`build-executor-mcp.test.ts`、`build-executor-tool-adapter.test.ts`。

**新增测试**：Codex 资产缺失只影响 Codex 分支；公共 prompt 缺失影响所有分支；新 profile 发给旧 decoder 被拒；未知版本被拒；新旧记录不能混用；四工具多字段输入在桥和 Engine 端被拒。

**Do not**：不全仓删除 `codex` 字样；不将现有 stage/plan schema 顺带升级；不放宽旧测试来接受新响应。

**完成条件**：旧 Codex golden 无非预期变化；DSH 的兼容结论独立成立；协议矩阵无未决字段，并回填 ADR §6。

**回退**：保留原 Codex 入口，禁用新宿主选择；新记录从未被旧 decoder 原地修改。

## DH2 — 执行配置贯穿计划、路由、交付与提交

DH2a、DH2b 分别提交验收结果；完成前者后再引入后者的新行为。

**DH2a：配置解耦。** 将现有 Codex 配置显式传至公共/私有 session、预算、dispatch/lease 和 BookStructure 各分支。仍只使用旧 Codex profile 与原记录，旧 golden 和相关回归保持不变；这一步不启用 DSH carrier 或新序列化分支。

**DH2b：DSH 能力接入。** 在 DH2a 基础上接入 DH1 冻结的新配置/记录及 DSH 容量路由，验收下列跨 profile、容量和恢复用例。首版验证 DSH 公共标准闭包；私有路径本片保留 Codex 行为及共享 decoder 兼容，DSH 私有运行入口仍在启动前拒绝，功能验收留给 DH7。

**目标**：消除“doctor 选了 DSH，真正生成/提交仍按 Codex 常量”的半适配。

**依赖**：DH1 的冻结合同、DH0 容量观测。

**修改**：§1.1 中 transport/session/预算/dispatch/lease/quality/BookStructure 模块；相关 batch limit、输入读取硬上限和 serializer 也须清点。

**实施**：

1. 增加只读已发布 profile registry：保留原 Codex 精确配置；DSH profile 使用 DH0 实测上限与保守预留，不填未经证明的容量。
2. 在计划确认前解析模型预算与传输能力，创建 invocation 时冻结实际 profile；每次 spawn 显式传冻结的 `agentOptions`（provider/model/reasoning/maxTokens），续跑不继承父代理后来切换的模型。`maxTokens` 为输出上限；完整上下文容量另算。语义期间变更模型配置只影响后续已重新评估的运行。
3. 逐层传递显式执行配置，不在底层读取全局环境/默认值做二次选路；新 record 绑定 profile 并在 replay 时校验。
4. 完整输入预算覆盖 role/tool schema/全局目录/已积累历史/候选输出及必要预留；不能只计算本次正文。首版 dispatch 形状保守，不通过截断旧输入或新建 LID 解决容量。
5. 分别验收 candidate value 与序列化请求上限；响应按实际 MCP+DSH 封套测量。禁止把 response-cap 与 model-context-cap 当成同一指标。
6. 所有 accepted artifact 复用仍走现有 semantic input/prompt/schema/freshness 门；transport 调整不新增一层控制 digest。

**复用测试**：`executor-transport.test.ts`、`executor-carrier-capacity.test.ts`、`automatic-build-budget.test.ts`、`automatic-build-routing-v3.test.ts`、`automatic-build-executor-session.test.ts`、`book-structure-routability.test.ts`、`book-structure-relation-routing.test.ts`、`book-structure-reuse.test.ts`。

**新增测试**：同样输入在不同 chunk 边界还原一致；UTF-8 多字节、CRLF、公式转义、嵌套 JSON 与接近上限输入；限额前一值/等值/后一值；长 dispatch 累积上下文；提交超限无 partial commit；新公共记录不得回落 Codex 配置，旧公共/私有记录仍按原分支解码，共享新记录 decoder 不丢失配置。

**Do not**：不顺便调 Pass1/Pass2 prompt；不因为 DSH 模型窗口更大就扩大所有批次；不缓存完整正文到插件诊断中。

**完成条件**：profile 从计划到 publication 全链路一致；Codex 默认行为保持；每个超限场景在语义生成或写入前以真实类别拒绝。

**回退**：停止新 DSH invocation；保留新增记录只读；不改动已发布语义产物。

## DH3 — DSH 专用 child 身份、四工具与独占 MCP 桥

**目标**：真实 DSH child 能执行一个合法 handoff，且桥不提供任意路径/命令或跨 child 调用能力。

**依赖**：DH0–DH2。

**新增**：`executor-host.ts`、`executor-bridge.ts`、`executor-role.ts`、`engine-client.ts`、`binding.test.ts`、`bridge.test.ts`、`privacy.test.ts`。

**实施**：

1. 从已配置路径/既有安装发现机制解析 Engine，使用 argv 数组和受限 stdin 启动；不要拼 shell 字符串。spawn 环境仅继承必要变量，凭据不注入 Engine。控制面与 executor 端必须使用相同 `UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT`/对应已解析 registry root、用户身份和 Engine 版本，防止 ref 因读取另一套默认目录而失联；工作区路径须在该执行环境实际可达。目标是 build executor MCP，不是只读 Book MCP 或代码只读 MCP。
2. 每个 launch 创建独立 child、AbortController/宿主 signal 关联和独立 stdio client；启动映射存真实 Agent 对象、启动代次、handoff 与连接。
3. 按 §2.2 和 DH0 验证的创建时序绑定 child，安装 executor preset、精确四工具与 Native 呈现；绑定失败时无 executor.open 副作用。沿用现有 `agentPresets.select` 和 child scope，不为此修改 DSH Agent loop。`toolFilter` 只能遮蔽继承工具，child 自有工具另按真实 schemas 验证；不能把 `run_code` 填进 deny 列表。
4. 工具 schema 来自同一规范定义；DSH 名称映射是纯投影。由于宿主 schema/Parameter DSL 未必表达全部约束，桥和 Engine 仍执行封闭对象校验，不允许额外参数穿透。
5. 每条连接串行处理操作；重复请求仅按既有 replay 门处理。不同连接可并发，同一连接不并发预取 input/start/submit。禁用未按该协议验证的 MCP 透明重连、跨 child 连接池和外层通用工具重试；断连后是否可重放由 Engine 的幂等/恢复事实决定，不让宿主自动再执行副作用。
6. 四工具只转发该 child 的请求；一个新 ref 必须走新 child/新连接；终态后拒绝调用。MCP stderr/error 映射为允许列表诊断，原始文本不直接进根结果。
7. 真实 Engine 响应形成最终生命周期事实。丢弃或本地严格解析子代理 final，不把它当作 candidate 或完成权威；关闭任何向父代理自动转发完整 child 输出的通道。

**测试**：根主动调用、非专用 child、兄弟 child、伪造 label、相同 session 字符串的错误 Agent 对象、stale instance、新 ref 复用连接、重复 submit、阶段乱序、并发工具请求、异常工具结果；PTC/both 父配置下 child 仍为 Native 四工具，父 preset 与工具模式不变。

**Do not**：不设置 `outputSchema`；不依赖父代理 prompt 宣称身份限制；不把 stdio 的进程私有 symbol 称为 caller-role 认证。

**完成条件**：真实 DSH 单 handoff 成功；所有身份负例在到达 Engine 前拒绝；两个 child 的语义/连接/回调独立；父上下文哨兵泄漏为零，且日志范围说明准确。

**回退**：注销插件工具、拒绝新 launch，dispose 所有已拥有 child 和连接；不删除 task/lease/产物来制造“干净退出”。

## DH4 — 单执行器标准计划完整闭环

**目标**：DSH 中的一次普通标准预构建能跑到真实 DONE 或真实用户边界，而非只跑一个抽取样例。

**依赖**：DH3；首版 `max_parallel=1`。

**新增/修改**：`build-control.ts`、`control.test.ts`；必要的版本化 Sidecar 控制入口；DSH 用户指引。已有 `legacy-plan` 和 `build.step` 的功能应通过共享代码包装，不复制计划生成算法。

**实施**：

1. 明确标准路线、目标工作区、Pass2 和授权预算，读取 Engine 投影。计划身份漂移或授权内容变化时重新展示与确认。
2. 实现 §2.1 的准备/确认与运行两个调用：插件直接调用根的 `userQuestions.ask()`，核对答案与计划身份后创建 invocation，先返回并持久保留引用；运行工具拒绝未经过该确认流程的初始运行。中断后继续原引用，明确区分 Engine 已有确认状态和真实人类作答。自然语言目标按首版能力拒绝。
3. 控制驱动只消费四动作：spawn 用 DH3；wait 遵守 Engine 建议并可取消；needs_user 原样投影 request/choice；done 只输出 Engine completion summary。
4. 按 §2.6 区分正常让出与硬取消；正常达到 handoff 数上限时停止补位并等已有 child 收尾，硬取消才中断。`NEEDS_USER` 在收尾、上报观察并重读当前请求后再问用户；无 owned child 的外部 WAIT 返回等待投影。所有返回均保留原 invocation 身份，首版不做后台移交。
5. child final 不触发 DONE；每次 child 终态后重读 `build.step`。待上报失败不因为一次成功或用户边界被丢弃。
6. 日志只保存允许列表内控制字段。输入/候选/重试字段反馈不经过根 Agent，DSH 角色不激活 build skill 形成递归构建。

**测试**：技术书/论文两个 profile 的合成合法工作区；Pass2 开/关；小预算拒绝；批准后计划漂移；拒绝/无 answerer/取消时零启动；确认结果已记录后中断；多次前台调用复用同一 invocation 完成计划；长 handoff 不被正常让出打断；硬取消后恢复；候选一次失败后成功；公共阶段闭包全部完成；最终不足门禁时不可 DONE；计划包含未支持私有产物时必须拒绝。

**完成条件**：至少一条标准计划完整通过已有 writer/quality/publication；有中断后恢复样例；不依赖源码目录的安装态最终证明留在 DH6。

**回退**：禁用 DSH 根控制入口，保留构建工作区与诊断；Codex 仍能按原合同使用 Engine。

## DH5 — 并发、生命周期竞态、取消与恢复

**实现记录**：见 [2026-09-26 DH5 报告](performance/dsh-dh5-20260926.md)。准备与读取 V2 保存确认的 1–3 并发；Engine controller V1 保存前台所有权及逐 launch 观察。

**目标**：将同一闭环扩展到既有 `1..3` 容量范围，故障不能造成重复构建、孤儿执行器或虚假成功。

**依赖**：DH4。

**修改**：`executor-host.ts`、`build-control.ts`、`diagnostics.ts`，必要的 Engine 有界宿主观察 decoder；不得在插件复制 lease/attempt 状态机。

**实施**：

1. 维护本进程 `live_by_slot`、已处理 terminal 与待上报观察；key 同时核对 slot、启动实例与原始 handoff。相同 slot 的历史回调不能释放 replacement。
2. 可用槽位由请求并发上限、插件已拥有资源和宿主已验证容量共同限制。拿不到实时宿主容量时保守限制，并如实报告，不能把历史配置当空闲容量。
3. 首个 owned child 终态即推进 Engine，消化已经到达的其他 terminal 后再补位；不得使用等待全部 child 的批次屏障。
4. 同一 invocation 的前台 controller 进入需要串行化；不同根会话/不同进程争用同一工作区仍交给 Engine 的 lease/owner 门，不额外发明覆盖磁盘事实的全局锁数据库。
5. 保留 bootstrap、open correction、connection、semantic attempt、candidate retry 与宿主 runtime 故障的分类。缺失观察不能推断成零调用；未知异常不套用旧重试授权。
6. 用户取消/宿主退出/插件卸载停止新启动；取消所有 owned child，等待全部清理，即便其中一个清理失败也不能漏掉其他 child。连接/提交结果未知时保存有界观察，恢复由 Engine 裁定。
7. 崩溃重启不从 DSH final 或本地 Map 重建完成状态。先查 Engine 与实际宿主存活状态，活动 lease 未处置前不重复开 child；无法证明 owner 死亡时等待/提示外部阻塞。
8. 验收跨宿主接手：停止旧 owner，重新评估/必要确认，以新 invocation 启动 DSH；有效 accepted artifact 由 Engine 复用。首版只支持受控接手，不迁移活动连接。

**复用测试**：`automatic-build-concurrency.test.ts`、`automatic-build-dispatch-runtime.test.ts`、`automatic-build-lease.test.ts`、`automatic-build-recovery.test.ts`、`automatic-build-driver-failure.test.ts`、`build-executor-call-diagnostics.test.ts`。

**完成条件**：§3 全部确定性故障矩阵通过；两与三并发均覆盖；未产生重复 accepted write；取消后无插件拥有的活跃 child/MCP 进程；跨宿主受控接手保留原产物与历史。

**回退**：将 DSH 声明容量收回到已验收的 1；如涉及所有权或重复提交问题则禁用整个 DSH 入口，而非只隐藏异常。

## DH6 — 真实安装、发布资产、质量与回滚门

**分步完成记录（2026-09-27）**：正式包与 Windows Desktop 安装生命周期已完成，见 [DH6 安装报告](performance/dsh-dh6-install-20260926.md)。插件 `0.1.0-rc.2` 与独立 Engine 已在源码目录外验收；官方插件页安装/升级/卸载、完整进程重启、包内取消/卸载/重新启用通过。真实模型质量、带业务进度的 Desktop 重启恢复和受控回滚继续待做，DH6 总项保持未完成。

**目标**：证明交付的是可安装适配器，而不是只在仓库测试环境有效的脚本。

**依赖**：DH0–DH5。

**新增**：发布包构建与 `verify-dsh-prebuild-release.mjs`、安装文档、固定版本兼容报告、真实模型报告与回滚记录。

**实施**：

1. 用实际 tarball/正式插件安装入口与已安装 Build Engine 验收，运行目录不在源码仓库内；验证角色/合同资源随包存在。
2. 首个正式目标为实际 Windows x64 DSH Desktop 组合；通过 Desktop 官方插件管理方式安装。CLI 在独立合法 profile 验收，不能用公共 CLI 写 Desktop 保留 profile。
3. 检查依赖未打包另一份 DSH 核心身份对象；按所选版本官方插件依赖约定组织 peer/runtime dependencies。未知用户配置不覆盖，Windows 路径含空格和中文仍可启动。
4. 验证安装、重启、升级、旧 Engine 不兼容、卸载、缺失 Engine、取消后退出；不依赖全局 Node/Bun/Cargo 回退。
5. 用明确授权材料做真实模型构建；保存全部尝试及失败，记录 profile、版本、model、预算、用量来源、重试分布与产物质量。
6. 执行一次受控回滚：禁用 DSH、保留新记录和 accepted artifact，使用兼容的 Codex/Engine 组合重新进入；旧二进制遇新控制记录失败关闭，不伪称可原地降级。
7. 诊断与外传扫描：默认不添加 LangSmith 全量导出，不复制 child 原始 transcript。若使用现有观测能力，只导出已允许的控制事实。

**质量判定**：使用相同授权夹具和现有质量门。固定 candidate 的控制对照要求完全一致；真实模型不要求逐字一致，但结构合法性、证据合法性、阶段闭包和 publication 必须通过现有门。语义抽取质量用预先冻结的 rubric/人工抽查复核，不以“DONE”替代。

**样本最低要求（工程 smoke，不是统计稳定性证明）**：技术书与论文各一个固定小工作区，分别覆盖 Pass2 开/关；至少各三次独立真实运行并保留所有结果；另有多 chunk、长 dispatch、一次 schema retry、一次提交后断连和一次重启恢复。执行前固定成功率与质量比较口径，不能失败后删题或放宽门槛。

**完成条件**：正式包通过；兼容矩阵中的每个“支持”均有安装态证据；未测试的平台/模型/模式标记未验证。真实失败未解决时维持测试版，不宣称稳定发布。

**回退**：停止分发不兼容包，保留旧包与旧入口；不删除历史成果或清空用户宿主配置。

## DH7 — 自然语言目标与 Reader-private 规划（后续独立扩展）

**目标**：在统一引擎里扩展 DSH 能力范围，不另建私有产物管线。

**依赖**：DH6 通过；单独确认该产品范围后实施。

**当前约束**：现有自然语言路线使用 `--codex-build-intent`、`codex_build_intent_command.v2` 和 `codex_conversation` 确认来源，不能让 DSH 假冒该来源。[ADR R3]

**修改**：`build-intent` 相关 controller/schema、Desktop 规划入口、确认来源投影与指标；DSH 控制适配器。具体 Rust/TS 落点在本片开始时按当前源码重新定位，不为首版重构整个 Reader。

**实施**：提供宿主中性的版本化入口，保留旧 Codex 路径兼容；目标/context/candidate 经保护通道传递；Reader 继续拥有信任、私有存储和准确计划确认。私有执行复用 DH2/DH3 的执行配置与专用提交，不把私有目标/正文写 argv、环境变量或公开诊断。

**验收**：合法私有计划、上下文漂移、重确认、非信任工作区、跨会话恢复、取消、非授权访问；明确 DSH child 日志与 Provider 的数据处理范围。

**Do not**：不在 DH4 中静默把自然语言目标降格为普通标准计划；不把“DSH 首版不支持”写成 Engine 不再支持私有构建。

**完成条件**：完整私有产物闭环和授权/隐私门通过后，单独扩大 DSH 能力声明。

## 3. 故障分类与验收矩阵

### 3.1 观察映射：先保留事实，再选择恢复

| 观察 | 可声明的事实 | 允许动作 | 禁止动作 |
|---|---|---|---|
| provider/依赖不支持启动能力 | 启动前能力缺失，无语义工作证明。 | compatibility 阻断；有真实零调用 bootstrap 事实时才用相应观察。 | 将所有异常写成 candidate retry。 |
| open 返回 `connection_terminal` / `handoff_ref_mismatch` | 保留原码与实际 phase。 | 交给既有 open 恢复/纠错，新的恢复 ref 使用新 child。 | 根 Agent 亲自 open 测试、覆盖原码。 |
| child 把 issued ref 抄错 | 已发 ref 与实际 open 参数不一致。 | 依据真实诊断走既有有界 correction。 | 随意切换到另一个任务或用旧 ref 重建结果。 |
| 工具超时，无法确认是否提交 | 未知，不等于未执行。 | 先查持久提交/attempt/lease，再由 Engine 决定。 | 立即二次生成或盲目重发非可证幂等调用。 |
| DSH context/transport 超限 | 宿主输入/通道失败。 | 停止当前执行，报告真实类别；重新评估配置/路由。 | 删除输入、压缩证据、冒充 schema 错误。 |
| writer/schema gate 拒绝 | 当前 Engine 已给出候选失败事实。 | 使用既有 `retry_feedback` 和授权次数。 | 放宽 schema 或插件无限重试。 |
| child final 自称 committed | 仅是模型文字。 | 丢弃其完成主张，重读 build.step。 | 将 slot/产物标成全局完成。 |
| Engine committed 后 child 崩溃 | 持久事实优先，宿主退出仍需清理。 | 保留成果，处理残余观察与资源。 | 用崩溃覆盖 committed 或重复写成果。 |
| 无可用 child trace/诊断 | 缺证据。 | `evidence_missing` 类有界结果；保留未知。 | 宣称零调用或建议无依据的重装。 |
| 用户取消/插件卸载 | 明确取消信号，不证明没有 commit。 | 停新启动、dispose、持久状态恢复。 | 删除 lease/receipt 或把取消变成成功。 |

新宿主诊断名称只在版本化宿主合同中定义。若现有 `build.step` request 无法表达需要持久消费的新观察，DH1/DH5 增加明确版本；不要把新字段塞进 V1，也不要用字符串前缀伪装旧 Engine 错误。

### 3.2 必测用例

| 编号 | 场景 | 必须成立 |
|---|---|---|
| T01 | 无 Codex TOML 的 DSH 环境 | 只检查 DSH 资产；公共 Engine 缺失仍失败。 |
| T02 | 原 Codex 旧安装/旧 invocation | 原结果与安全说明不回归。 |
| T03 | 新 profile/manifest 给旧客户端 | 启动协商阻断，不能静默降级。 |
| T04 | 根 Agent 调 executor.open | 插件入口拒绝，零 Engine handoff 领取。 |
| T05 | 普通 child/伪造 label/模型 agent_id | 全部拒绝。 |
| T06 | A 使用 B 的 handoff/session/sink | 归属门拒绝，B 的任务不变。 |
| T07 | 相同 id 字符串但非当前 Agent 实例 | 拒绝过期/伪造运行态身份。 |
| T08 | child 首调用抢在 launch 返回前 | 正确等待可信绑定或失败关闭，无越权 open。 |
| T09 | 两 child 同时启动，一个初始化失败 | 各自资源/结果独立，失败不释放另一个槽。 |
| T10 | 非法字段/错版本/无效 JSON | 封闭校验拒绝，候选无写入。 |
| T11 | input.next 重复/乱序/start 提前 | 按现有 ack/phase/replay 合同接受或拒绝。 |
| T12 | 同连接并发工具调用 | 串行约束生效，不发生状态越迁。 |
| T13 | 新 handoff 复用已终态 child/连接 | 拒绝，必须新建。 |
| T14 | 中文/公式/代码/边界容量传输 | 逐段还原与实际模型可见内容一致。 |
| T15 | 完整上下文超限但单 chunk 未超限 | 不误判可路由，不隐式压缩或截断。 |
| T16 | candidate 请求超限/截断 | 无部分提交、无文件路径 fallback。 |
| T17 | 候选一次 schema 错误后成功 | 精确反馈，不改变语义输入/schema。 |
| T18 | 已提交但响应丢失 | 重读持久状态，复用成果，无重复生成。 |
| T19 | 先快后慢的 3 个 child | 快 child 释放后及时补位，不等待整批。 |
| T20 | 旧回调晚于 replacement 启动 | 不能释放 replacement 的槽位。 |
| T21 | NEEDS_USER 时仍有其他 slot/待观察 | 不丢观察、不孤儿化 child、不扩大授权。 |
| T22 | 取消/卸载/宿主退出 | 所有 owned child 和连接清理完毕；不伪造业务终态。 |
| T23 | 重启时旧 lease 仍活动 | 不抢占；等待或真实阻塞，非重复启动。 |
| T24 | Codex → DSH 受控接手 | 新 invocation，旧记录保留，有效成果按门禁复用。 |
| T25 | 模型 final/宿主通知含泄漏哨兵 | 不进入父上下文、控制结果或插件诊断。 |
| T26 | 含 Reader-private 请求的计划进入首版 | 开始语义前拒绝，不丢私有产物后继续。 |
| T27 | 插件重启、双 controller 同时继续 | 本地执行串行；跨进程由 Engine 所有权阻断冲突。 |
| T28 | DSH 运行中模型/配置发生热更 | 当前绑定不被静默替换；新运行重新评估。 |
| T29 | 旧 Engine 启动新记录 | 失败关闭，记录未改写。 |
| T30 | 无源码仓库的真实安装目录 | 从发布资产跑完整闭环。 |
| T31 | 父 preset 为 standard，工具模式为 PTC/both | child 选择专用 preset 且首请求/后续请求恰好四工具，父配置不变。 |
| T32 | 宿主会 spill 或压缩历史工具结果 | canonical value 与实际模型输入分别核验；语义正文未被替换、裁剪或摘要。 |
| T33 | 明确批准、拒绝、空答、取消、无 answerer、delegated caller | 仅精确批准可创建 invocation；计划身份漂移重新确认。 |
| T34 | 长 handoff、达到让出计数、连续续跑、一次硬取消 | 正常让出不打断 handoff；原 invocation 可完成，取消不形成自动重启循环。 |
| T35 | NEEDS_USER 清理期间请求改变 | 上报观察后重读 request/choice，旧答案不能授权新请求。 |

## 4. 验收与证据格式

### 4.1 分层验收

**L0：纯函数与协议。** 固定 candidate、时间和输入，验证 Engine 行为与已有 Codex 基线一致；这一层不能证明 DSH 的工具呈现或身份隔离。

**L1：真实 DSH + 脚本化模型。** 覆盖作用域、注册、发布时序、取消、自动通知、Native 序列化和安装组合；不以假实现代替真实宿主。

**L2：真实 DSH + 真实模型 + 已安装 Engine。** 覆盖标准阶段闭包、候选格式失败、输入还原、引用有效性、BookStructure、实际用量与质量。

**L3：安装/升级/重启/回滚。** 正式包与真实 profile，无源码目录依赖，验证全部资源释放与历史兼容。

### 4.2 报告字段

每次报告记录：测试材料授权与 fixture 身份、源码/包版本、平台与宿主、实际 Provider/model、profile/version、native/PTC 模式、配置是否可验证、并发数、输入/输出字节和估算 token、真实 usage 及其覆盖范围、attempt 与失败分类、完成阶段、accepted artifact 数量、质量门结果、父上下文扫描、资源清理结果、未测试项。

时间用真实测量；未知模型/用量/退出状态保留未知。不要为了与 Codex 对齐而伪造 DSH 的 `reasoning_effort` 值，也不要把一次任务总 token 复制成每个模型调用的 token。

控制报告不附原始书稿、语义 candidate、环境变量、私有路径或模型隐藏推理。需要复核的内容证据留在授权夹具/测试工件中，受独立访问和保留策略管理。

### 4.3 可复用测试入口与拟新增命令

以下现有入口已经在 package.json 核对；本次没有执行：

```bash
pnpm --filter @understand-book/core test
pnpm --filter @understand-book/core typecheck
pnpm test
```

DH6 拟新增并由切片真实实现后才能使用：

```bash
pnpm --filter @understand-book/dsh-plugin test
pnpm --filter @understand-book/dsh-plugin typecheck
node scripts/verify-dsh-prebuild-release.mjs
```

不要把示意命令当作当前可执行入口。遇到既有失败先保留基线与归因；不得删除测试或放宽断言使新适配“全绿”。

## 5. 合并与发布检查清单

- [ ] DH0：实际版本与创建/工具/连接/取消/容量探针通过。
- [x] DH1：公共 doctor 与宿主检查分离；协议矩阵冻结；Codex golden 保持。
- [x] DH2a：显式 Codex 配置贯穿 plan/preflight/dispatch/lease/session/BookStructure，旧记录保持；本轮 187 项相关回归与 Core typecheck 通过。DSH 分支已由 DH2b 接通。
- [x] DH2b：DSH 配置全链路一致；模型预算与传输预算独立验收。
- [x] DH3：可信 child 身份、独占连接、精确四工具及父上下文隔离通过。
- [x] DH4：单执行器标准阶段闭包完整，确认与用户边界准确。
- [x] DH5：并发、故障、取消、controller 重建和受控接手的确定性验收通过；进程清理及提交复用证据见 DH5 报告。
- [ ] DH6：正式安装包、真实模型质量与回滚通过，未验证组合明确标记。
- [ ] 文档更新：`CONTEXT.md` 只补术语；`docs/架构.md`、`docs/代码链路.md`、`docs/预购建流程.md` 与安装说明反映实际实现，不提前填“已支持”。
- [ ] 观测/隐私扫描没有新增默认外传，没有把 DSH child 日志宣称为不落盘。
- [ ] 所有新增记录版本、旧版本 decoder 与回滚范围均有明确消费方和测试。

DH7 不在上述首版通过条件内；完成后应另行更新能力矩阵。

## 6. 资料与定位说明

仓库依据见 [ADR-0138 的 R1–R12 与 W1–W12](adr/0138-multi-harness-prebuild-and-deepseek-harness-adapter.md)。新增文件以本文拟议结构为准。

外部能力依据：

- [W1：DSH Subagent 合同](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/docs/subsystems/subagent.md)
- [W2：DSH 工具扩展合同](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/docs/cookbook/adding-a-tool.md)
- [W3：DSH Subagent 类型](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/subagent/subagent/src/types.ts)
- [W4：DSH 架构](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/docs/architecture.md)
- [W5：DSH Subagent 服务](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/subagent/subagent/src/index.ts)
- [W6：DSH 原生 spawn](https://github.com/deepseek-ai/deepseek-harness/blob/477b4f420553e8a52c2fbccc464d7561b239c443/packages/subagent/subagent-spawn-in-process/src/index.ts)

W7–W12 的确认、超时、创建时序、作用域、preset 与输入保留源码定位见 ADR。上述源码均按 §0.1 固定提交核查；实际安装版本由 DH0 记录。
