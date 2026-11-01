# 矩阵范数追问：纠错提前停止与内部诊断外露

2026-11-01，先只读排查生产服务、会话事件和对应源码，再按用户授权实施修复并发布。以下调查保留当时证据；实施范围、验证和发布结果见文末。未修改生产历史，未新增真实模型调用。严格模式依用户决定暂不实施。

## 已核实的会话

- 正式进程使用 `/opt/understand-book/releases/note-thinking-20261101`，PID `70237`。
- 资料：`optimization-modeling-algorithms-theory`。
- 会话：`chat-01a19538-9b3b-7cd3-8bd6-a0eca4ea1d67`。
- 回合：`turn_01a19619-6100-7692-bd9c-e96ae6a915c0`。
- 用户问题：“我知道向量范数了，那矩阵范数是什么，又怎么理解呢”。
- 香港时间 15:53:27 接纳问题，15:55:36 记录最后一次模型返回与工具拒绝，随后保存本轮的失败终态。截图的 16:44 为查看时刻。
- 事件源：正式数据根下 `users/adaelon/memory/agent-sessions/s_Y2hhdC0wMWExOTUzOC05YjNiLTdjZDMtOGJkNi1hMGVjYTRlYTFkNjc.jsonl`。事件序号 168–185 固定本次最后三次尝试及终态。

## 实际失败顺序

前三次模型调用完成查找、原文读取、来源登记与教学设计。此后有三次 `tutor.step` 尝试：

| 模型调用 | 时间（香港） | 工具结果 | 后续 |
| --- | --- | --- | --- |
| 第 4 次 | 15:55:12 | `TOOL_ARGUMENTS_INVALID`：JSON 尾部有多余字符 | 继续调用模型 |
| 第 5 次 | 15:55:27 | `TEACHING_INVALID`：直接讲解中夹带新的作答任务 | 继续调用模型 |
| 第 6 次 | 15:55:36 | `TOOL_ARGUMENTS_INVALID`：`move.delivery_text` 与替代的 `move.board_candidate_revision` 均缺失 | 保存失败终态；没有第 7 次模型调用 |

最后一次参数已移除 `response_expectation`。它没有待交付画板；本次应提供讲解正文。缺字段由真实工具校验结果证实，不根据公开调用历史中字段缺失推断：`tutor::public_arguments` 本来就会移除候选正文、参与分配等私有字段，不能把该历史当作原始模型参数。

## 原因链

1. **纠错过程被共享停滞额度提前终止。** `tool_progress_signature` 记录证据、定位、成功操作、教学接纳等状态，没有记录不同校验问题的修正过程。连续两批这些状态未变后，`ProgressPhaseGuard` 允许一个共享恢复批次。`argument_validation_recovery_granted` 仅按本阶段“是否曾发生参数错误”设置一次，不比较具体错误。第 4 次的 JSON 错误先占用该机会；第 6 次消耗恢复批次后，新的缺字段错误不能再开放纠错。工具结果已经追加到消息，但循环随即退出，模型没有消费这条新反馈。
2. **终止原因被最后的工具诊断替代。** 活跃教学尚未交付时，停止分支返回 `last_teaching_error`，从而把缺字段的详细诊断变成本轮终态；外部无法仅从该错误看出宿主先因停滞规则停止了执行。
3. **已有失败交付未覆盖该错误码。** `delivery_fallback::recoverable` 接受教学、部分演示及 provider/protocol/capacity 错误，但不接受本次 `TOOL_ARGUMENTS_INVALID` / `validation`，因此没有进入现有可用答案交付。
4. **前端未知错误继续原文输出。** 服务端保存原诊断；`agentFailureMessage` 未处理 `TOOL_ARGUMENTS_INVALID`，落到 `errorMessage(error.message, ...)`。后者仅负责本地化，不隔离内部诊断；`RightRail.vue` 直接渲染结果。即时失败及历史重开均可显示该内容。

对应本地位置（核对时的工作区）：

- `crates/runtime/src/orchestrator.rs:5485`、`:5529`、`:5546`、`:7934`、`:9215`、`:9321`、`:9359`。
- `crates/runtime/src/tool_registry.rs:673`：完整参数纠错提示；`crates/runtime/src/tutor.rs:474`：本次缺字段合同。
- `crates/runtime/src/delivery_fallback.rs:4`；`crates/server/src/agent_run.rs:568`、`:1123`。
- `packages/web/src/agent-run-state.ts:126`、`packages/i18n/src/index.ts:54`、`packages/web/src/components/RightRail.vue:1453`。

线上对应函数已直接读取确认；线上 `orchestrator.rs` 行号与当前工作区略有不同。默认请求使用 `OuterConfig::default()`，未设置轮数上限；本次最后六个模型活动全部成功返回，终态不是额度或用户取消。

## 为什么之前修过仍出现

[ADR-0150](../../adr/0150-resident-unbounded-tool-loop-and-progress-stops.md) 保留“连续两批无进展后一个共享恢复批次”。[2026-10-28 参数恢复修复](../board-argument-recovery-20261028/README.md) 增加当前合同和字段层级提示，限制画板终态诊断长度，但普通教学路径及共享停止规则仍在。

`tool_argument_recovery_tests.rs` 覆盖“先语义拒绝，再首次参数错误，随后修正成功”以及重复缺同一字段的停止；没有覆盖本次“JSON 错误 → 内容拒绝 → 新的缺正文错误”。前端已有测试覆盖教学和演示错误，未覆盖本次错误码。

## 验证及处理方向

将截图诊断输入实际 `agentFailureMessage`：`TOOL_ARGUMENTS_INVALID` 在 `zh-CN` 与 `en` 均返回原诊断；对照 `TEACHING_INVALID` 则返回已有友好提示。生产会话事件与线上源码已经足以确认停止链路，未重新发起付费问题或修改历史。

修复应首先保证新的、可修正的工具反馈能够进入后续模型采样，按真实重复停滞判断停止，保留已有取消、明确额度与不可恢复失败边界。随后补齐最终失败的交付和显示边界；仅替换报错文案无法解决本次回答被提前终止。此为调查阶段结论，实施见下文。

## 追加调查：为什么会产生坏 JSON 和缺字段

读取上述回合的完整可观测记录，核对模型实际请求、返回的工具参数及下一次请求中的修订草稿。未重新调用模型。根 trace 为 `01a19619-63a3-7f80-856e-4400ec5f0d98`；最后三次模型 span 分别为 `01a1961a-90f5-7771-8be9-112b7daad9d5`、`01a1961a-f96c-78f3-9f73-82e182847428`、`01a1961b-33ec-7d82-a314-89dc696513a7`。

### 模型实际收到的定义

- 请求模型为 `deepseek-v4-flash`，使用 Messages 格式；`tutor_step` 的定义包含 `name`、`description`、`input_schema`，没有 `strict`。
- 该工具的 schema 按不转义 Unicode、无多余空格的 JSON 序列化后为 **54,445 字符**；去掉 17 个 examples 后为 39,253 字符；另有 3,519 字符工具描述。工具覆盖 9 种操作，根对象有 24 个候选字段，`move` 有 27 个候选字段。这些数字不表示所有字段均须每次填写。
- `move` 的必填列表为 `move_id`、`object_ids`、`kind`、`prompt`、`design_ref`、`block_refs`、`participation`，并要求 `delivery_text` 或 `board_candidate_revision` 至少一个。本次没有待交付画板，应提供正文。
- 最后三次调用的这些定义相同，正文要求确实传到了模型。`prompt` 的 schema 只有字符串类型；`delivery_text` 的描述则明确要求完整最终正文。
- `model_spec_for_context` 保持同一权限模式下的声明稳定，另用 `execution_guidance` 传当前条件要求。因此模型还须同时遵守通用 schema、字段描述、当前执行状态和语义规则。该设计见 `tutor.rs:608`、`:644`，不能将其误称为适配器随机丢失 required。

### 两次参数错误的实际形状

第 4 次调用的参数长 1,654 字符。首个完整 JSON 对象只包含 `move` 和误放在根层的 `coach_adjustment`；对象之后还有以下文本：

```text
, "operation": "select", "finish": true}
```

因此这是对象层级/闭合位置出错，控制字段落到了已闭合对象外；单纯截掉多余尾部还会丢失操作和结束标记。Python JSON 解码器在字符位置 1,614 报 Extra data，与线上 Rust 按字节计数的 trailing characters 诊断相符。观测记录中 normalized response 与 parsed tool call 的参数字符串完全相同。流适配器 `deepseek_messages_stream.rs:178` 直接拼接各参数增量，没有插入括号；本记录不包含原始 SSE 数据包，故不把 normalized response 称为原始网络抓包。

第 5、6 次调用对比如下：

| 字段/状态 | 第 5 次 | 第 6 次 |
| --- | --- | --- |
| JSON 解析 | 成功 | 成功 |
| `participation.choice` | `invite` | `conclude` |
| `interaction_intent` | `direct_explanation` | `direct_explanation` |
| `response_expectation` | 有 | 移除 |
| `delivery_text` | 1,718 字符完整正文 | 移除 |

第 6 次只改动 `coach_adjustment`、`participation`，移除了 `response_expectation` **以及不应移除的 `delivery_text`**，没有新增字段。它确实修正了第 5 次的教学方式问题，又引入了新的缺正文问题。

### 修订接口的具体负担

`teaching_revision.rs:5`、`:60` 的局部修订只支持 `delivery_text` 与已有 `response_expectation.public_prompt` 的段落替换，不能修改 `participation` 或删除整个 `response_expectation`。`server/src/teaching/design.rs:98`、`:108` 按参与方式和邀请对象校验，单纯把邀请语改空不能完成本次方式纠正。`tutor.rs:799` 明确要求非文本约定变化时重交完整 `select`。

因此本次将邀请改成收束，需要重新生成整份参数及原本已写好的正文。第 6 次请求中的 `agent.teaching_revision` 仍保有 `teaching-draft:1`：7 段正文、1 段邀请语和原完整选择的其余字段。不存在宿主没有给模型保留正文的证据；实际发生的是重新提交时漏带。历史 tool-use 输入会移除正文等私有字段，完整草稿通过独立上下文片段提供，见 `tutor.rs:6`、`orchestrator.rs:3536`。

### 结构约束能力及结论

`deepseek_messages.rs:437`、`:561` 只提交常规工具 schema，没有启用供应商保证结构一致性的严格模式。[DeepSeek Tool Calls 文档](https://api-docs.deepseek.com/guides/tool_calls/) 将 strict 作为单独 Beta 功能，并限制受支持的 schema；[Messages 兼容文档](https://api-docs.deepseek.com/guides/anthropic_api/) 列出当前使用的工具字段，未提供等同的 strict 保证。不能把常规 schema 当作生成阶段的强制约束，也不能未经兼容核实就给现有 Messages 请求添加一个开关。

已确认的问题是：多操作、深层字段的工具负担，以及非文本修正必须重新提交完整正文的修订缺口；本次正文规则未丢失，模型输出确实违反了已收到的定义。长 schema 对错误率的具体贡献尚无对照测量，不能仅凭字符数归因全部历史错误。严格结构输出能解决的范围是 JSON 语法和结构，并不能证明教学内容符合用户意图。

处理方向应包括缩小必要输入、让保留草稿上的本次参与方式纠正无需复制无关正文，以及前述新错误反馈继续执行。用户随后决定暂不实施严格模式，按以下范围修复。

## 已实施的变化

1. `ProgressPhaseGuard` 记录当前阶段已反馈的具体校验问题。新问题可以获得后续纠正采样；重复问题、JSON 位置变化、附加草稿提示变化以及错误循环不能反复续期。画板选择和修改共享相同问题记录。实际进展、取消、额度、显式轮数限制保留各自含义。
2. 普通文本 `revise` 新增可选 `move_changes`，只支持 `participation`、`response_expectation`、`coach_adjustment`；未指名字段保留，后两项允许用 `null` 移除。完整正文、来源、设计、标识与 `finish` 留在宿主草稿中，重建完整 `select` 后重新执行参数和业务校验。整项改邀请与段落改邀请冲突时原子拒绝。
3. 固定工具说明明确 `prompt` 与 `delivery_text`、根字段与 `move` 字段的职责；删除重复的完整选择示例，新增短修订示例。纠错提示不再枚举所有可选字段。同一执行模式下的工具声明不因草稿或错误而变化，动态状态继续通过历史快照追加。
4. 未完成教学的终态保留真实停止码，不再由最后工具诊断顶替。无进展进入已有可用答案交付；显式轮数限制不发起额外模型调用。服务端公开回复、终态流、重连快照、阅读现场历史视图、独立历史查询及前端最终提示均使用公开文案，内部持久化和诊断保留原始原因。

## 缓存与请求负担

比较修改前保存的当前工作树与本次候选，采用同一普通教学模式、相同 UTF-8 JSON 序列化：

| 测量 | 修改前 | 修改后 |
| --- | ---: | ---: |
| 模型工具 schema 字符 | 54,445 | 52,505 |
| 工具说明字符 | 3,519 | 3,389 |
| 模型工具示例数 | 17 | 14 |
| 长正文夹具的完整选择 / 局部修订字节 | 3,726 | 204 |

长正文夹具保留 2,880 字节原文，局部修改载荷减少约 94.5%。这项测量比较模型需要生成的工具参数，不将字节数当作 Token 数或缓存命中率。

两条实际编排回归捕获每次 `AgentRequestPlan`，经当前 DeepSeek Messages 投影后逐轮比较 `system`、`tools` 和此前已发送的完整 `messages`：新请求只追加，原前缀逐项相同。覆盖坏 JSON、语义拒绝、新缺字段，以及基于草稿修改参与方式后重新校验。定义更新会让变更部分重新建立缓存；同版本纠错无需重新声明工具或修改已发送前缀。

## 验证

修改前已用生产相同形状的序列复现：第三种校验反馈后停止；局部修订不能改变参与方式；前端原样输出内部错误。候选覆盖连续纠错成功、重复/循环错误停止、显式轮数限制、保留正文且继续校验、历史重开和流重连公开投影、中英文未知错误文案。

| 验证范围 | 最终结果 |
| --- | --- |
| runtime 完整库测试 | 1,005 通过，16 个既有导出/人工/真实模型夹具跳过；最终串行运行，避免并行类型导出占用同一文件 |
| server Agent 相关测试 | 72 项通过，6 个既有真实模型夹具跳过；包含公开存储失败事件的单项复验 |
| 前端错误映射与对话恢复 | 25 项通过 |
| 前端类型检查 | 通过 |
| 实际组件浏览器检查 | 2 项通过，均测试已知与未知错误码 |

主要复验入口：

```text
cargo test -p runtime --lib --target-dir .tmp-rust -- --test-threads=1
cargo test -p server --lib agent_ --target-dir .tmp-rust -- --test-threads=4
pnpm --filter @understand-book/web exec vitest run src/agent-run-state.test.ts src/NetworkApp.chat.test.ts
pnpm --filter @understand-book/web typecheck
pnpm --filter @understand-book/web exec playwright test --config playwright/agent-failure.config.ts
```

缓存与参数大小的机器可读对照见 [cache-comparison.json](cache-comparison.json)。最终运行时完整复验直接运行上述 Cargo 已编译的同一测试程序，结果为 1,005/0/16；服务端为已通过的 71 项及公开存储失败事件断言更新后的 1 项复验。 发布前补查独立历史查询的 `authorization::turn_json`：普通错误原来已有固定提示，但 `board_delivery_error` 仍直接返回持久化诊断，现统一使用公开错误投影。补充断言后，公开错误、两种历史入口及即时/恢复快照的 3 项针对性测试全部通过。

浏览器使用实际 `RightRail` 和公开错误映射函数，以截图中的问题、内部校验错误及未知错误码测试 390×844 与 1440×844 视口：提示可见、没有原始字段诊断、没有横向溢出、发送按钮可用。人工查看截图确认布局。

- [手机验证截图](public-failure-390.png)
- [桌面验证截图](public-failure-1440.png)

## 正式发布

2026-11-01 **19:14:22 HKT** 已发布到 [understandbook.top](https://understandbook.top/index.html)。版本目录 `/opt/understand-book/releases/tutor-recovery-20261101`，正式进程 PID `76092`，实际维护窗口 **21.96 秒**。构建和候选验证期间原服务持续运行；只有最后的程序切换与就绪检查处于维护窗口。

- 以当时正式 `note-thinking-20261101` 完整源目录为基线，合入本次修复。保留线上额外的画板恢复处理、笔记理解功能和手机笔记操作修复。源码合并记录见 [source-merge.json](source-merge.json)。
- Linux 六个正式程序完成 release 构建；多用户前端类型检查、25 项相关回归、生产打包全部通过。新发布目录保留旧后台和 16 个旧版本资源，1,235 个 CSS 资源引用可读。
- 使用独立生产数据副本启动候选，验证三个账户的登录、材料、历史、额度、工作区和演示能力。在副本中读取本报告对应的真实失败回合，公开错误为“这次回答未能完成，请重试。”；模型调用增量为 0。见 [候选验证](qa-report.json)。
- 切换前正在执行的回答、模型、服务和搜索请求均为 0。本次没有数据结构变更，不运行数据迁移、不在维护期间复制整库。备份原服务、代理、环境和演示配置；启动失败时只恢复旧程序配置，不回退用户数据。
- 发布后正式 HTTPS 首页、阅读页和后台入口均命中新版本资源。reader/admin 登录、历史、额度、工作区和已有笔记路径可读，演示能力正常，正式进程确认运行新程序。见 [发布回执](deployment.json) 与 [公网验证](public-verification.json)。
- 临时验证进程已退出，独立验证数据已删除；新旧发布目录、构建日志及配置回退副本保留。回退配置目录为 `/opt/understand-book/backups/tutor-recovery-20261101`，服务器发布与验证脚本保留于 `/opt/understand-book/acceptance/tutor-recovery-20261101`。见 [清理记录](cleanup.json)。

## 已知边界

已发布线上；未对真实供应商进行修复前后付费对照，因此不声明线上命中率或模型错误率已改善。已验证的是请求前缀保持不变与修订参数减少。普通工具 schema 仍不保证模型永远生成合法 JSON；本次修复让可修正失败得到反馈和继续机会，并保证终态内部诊断不进入用户提示。
