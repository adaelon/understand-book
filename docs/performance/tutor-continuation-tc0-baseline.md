# TC0 — Tutor 下一步协商实施基线

日期：2026-10-11（Asia/Hong_Kong）。对应 [ADR-0163](../adr/0163-tutor-continuation-and-contextual-reading-entry.md) 与[切片方案 TC0](../切片方案-Tutor下一步协商与低门槛阅读入口.md#tc0冻结基线归属和失败用例)。

**TC0 已完成**：建立现有行为基线、共用材料和可执行的预期失败用例；产品行为未改。TC1–TC8 仍待实施。测试使用本机隔离数据、SQLite/JSONL 和替身模型，未调用真实 Provider、未部署。

## 1. 基线与归属

- HEAD：`12b765bb33fe42fcd30fcda743187241ed56d1f3`，`merge: integrate PDF and conversation improvements with reading notes and sharing`，与读入 checkpoint 一致。
- 已读取只读插件 `project_overview`、`working_changes`；其 live working tree 不是 HEAD 快照。Git 核对 ADR-0163 只有本方案这一份，ADR 与切片方案开始时均为未跟踪文件。
- 编辑前逐文件状态、UTC 修改时间、字节数及相对 HEAD 的增删行数保存在 [working-tree-before.json](tutor-continuation-tc0/working-tree-before.json)。时间只表达观察时文件状态，不认定修改作者。
- 已有改动保留；没有提交、清理或回滚工作树。

| 文件／组 | TC0 前状态与已有内容 | 本次归属 |
|---|---|---|
| `crates/server/src/lib.rs` | 已修改，+7/−1；书籍标题读取、列表字段和相应测试 | 仅在既有 `#[cfg(test)] mod tests` 中注册 `tutor_continuation_tests` 一行 |
| `crates/runtime/src/tutor.rs` | 已修改，+13/−0；运行中控制状态指导 | 保留 |
| `App.vue` / `RightRail.vue` | 已修改，+104/−133、+19/−112；学习控件、阅读与回答界面调整 | 保留，不整文件替换 |
| `api.ts` / `network-client.ts` / `useTutorControl.ts` | 已有输入／路由／状态修改 | 保留 |
| `App.startup.test.ts` / `RightRail.test.ts` | 已有新增与修订断言 | 原样运行，未改断言 |
| `teaching.rs` / `run_admission.rs` / Memory 教学与证据模块 | 相对 HEAD 干净 | 只读核对与测试 |
| `tutor_continuation_tests.rs`、`tests/fixtures/tutor-continuation/` | 不存在 | TC0 新增 Server 用例与共用材料 |
| `playwright/tutor-continuation.spec.ts`、`playwright/fixtures/tutor-continuation.ts` | 不存在 | TC0 新增浏览器用例与夹具 |
| `.gitattributes` | 相对 HEAD 干净 | 仅固定共用 `source.md` 的 LF 换行，保持 Windows 检出后的源范围一致 |
| 本报告、日志目录、切片状态、ADR 状态、代码链路、checkpoint | 文档组含其他在途改动 | 新增证据、局部更新状态；checkpoint 按接手协议整页刷新 |

## 2. 复核后的实现接点

符号位置以本次工作树为准；后续编辑按符号定位。

| 接点 | 当前行为与下一片需要保留的边界 |
|---|---|
| `App.vue:askSelection/askPdfSelection/sendAgent` | 两种选区都保存 AskDraft，清空输入后打开助手；发送要求用户补文字。直接帮助应另设动作，并保留已有自由输入草稿。 |
| `RightRail.vue:onAnswerMouseUp/saveAnswerSelection/shareAnswerSelection` | 以已交付 turn 和连续回答摘录定位，现有操作为笔记／分享；没有回答提问或纠偏提交。 |
| `api.ts:AgentChatMeta`、`App.vue:submitAgentMessage` | 尚无 `entry_action`、`answer_ref`、`teaching_choice`；新输入需一路接到原 Run。 |
| `network-client.ts:createRun/recoverSubmissions/retrySubmission` | 先保存原 key 和完整请求，未知结果按 key 查询，显式重试复用旧载荷；不会自动补 `expected_chat_head`。原逻辑可复用。 |
| `lib.rs:AskQuote/parse_question_quote/agent_question_with_provenance` | 核验原文范围与摘录，保留选区来源；回答摘录不能冒充原文。 |
| `lib.rs:validate_agent_input/validate_agent_turn/prepare_agent_chat` | 首层检查 message 类型，后续回合校验拒绝空白 user；本地普通空白输入实际被拒绝。precommit 后 prepare，当前不按 `client_request_id` 查重。 |
| `run_admission.rs:normalize/admit/complete_preparation/recover` | 非空 message 和固定白名单归一化，两个不同 choice 会成为同一请求；FrozenTurn 保存后补事件、prepared，再 queued；恢复不调用原模型。 |
| `teaching.rs:freeze_prepare/learner_context` | `teaching_ref` 产生 `LearnerAction(chat_response)`；普通引用不能借此接续。首问创建会话，材料就绪决定 active/observing。 |
| `teaching.rs:current/check_source/check_activity_source` | 活动操作核验 control/session revision 与原 source version；新 choice 尚未进入这些核验。 |
| `tutor.rs:TeachingMove/spec/instructions`、`teaching.rs:record_user_delivery` | Move 是本轮实际动作，尚无 continuation；只在成功保存回答后建立公开交付，不能把未来选项记为 MoveSelected。 |
| `teaching.rs:behavior/route`、`TutorActivities.vue` | 原提示经 request_hint → HelpDelivered → 实际展示回执 → HelpDisplayed；submit 冻结 assistance_refs/attempt。定位原活动不能代替实际展示。 |
| `memory::teaching:TeachingFact/teaching_recent` | 现有事件枚举与近期种类过滤尚无 displayed/chosen 两种 continuation 事实。 |
| `learning_evidence:derive_assessed_evidence/rebuild_learner_projection` | 现有独立／帮助后／改答表现分流有效；来源式题目保留帮助条件，旧对象式题目还能核对 independent_support/assisted_support。 |
| `useTutorControl`、`App.vue:tutorAction` | 显式 set_enabled 本身不调用模型；start/resume 另有原教学入口。当前会话、暂停、其他来源和 readiness 分开表达。 |

## 3. 共用夹具

材料位于 `crates/server/src/tests/fixtures/tutor-continuation/`：

- `source.md`：平均速度的定义、相同距离与耗时关系、正时间条件；另有不相关段落。Server 与 Web 读取同一文件，Git 按 LF 检出。
- `scenario.json`：同一 book_id、固定原文摘录、已交付回答与回答摘录、未发送草稿，以及两项计划中的 continuation 文案。
- Server `fixture(ready)` 用真实 Book/LID 与 `source.txt`、`base.json`、discourse/formula 文件；ready 状态含 `book_structure.json`，缺少状态实际省略该文件。无构建回执、无可选 TeachingMap 也能建立来源式 Tutor 基线。
- `activity(object_only)` 分别建立 source target 与旧对象式冻结评分活动，沿真实 select、交付、展示、提示和评分路径保存。
- 本地夹具使用当前 JSONL SessionStore，验证已交付回答和选区重开可读；重复 key 用例在两次请求之间重新打开会话存储。
- 多人用例复用 MU5 的测试身份与授权宿主，发布同一小材料，经实际接纳、SQLite/JSONL、故障注入、恢复及单次领取执行。
- Web 在既有 Playwright/Vite 入口挂载真实 App。Markdown 渲染源文；PDF 由同一材料生成可选文字的一页 PDF，走真实 PDF.js 文本层及选区操作。API 返回隔离夹具；ready 与 missing_foundation 两种情况均保留普通解释。

| 覆盖面 | 本地 Server | 多人 Server | Web |
|---|---|---|---|
| 同一材料、原文范围 | 实际 Book/AskQuote | 同一材料发布与 AskQuote 接纳 | Markdown/PDF 实际选区发出的范围 |
| ready／缺少基础成品 | 实际文件与 active/observing | 本片新接纳夹具用 ready | 两种状态各覆盖两种阅读表面 |
| 原题帮助 | 来源式、旧对象式活动均实跑 | 复用已跑的原 Tutor/MU5 基线；E21 多人完整新链路待 TC7 | 原控件代码已读；本片没有生成新帮助 UI |
| 接纳与恢复 | 实际 JSONL 重开与重复调用红灯 | queued 响应丢失后查回同一回合，Provider 一次 | 既有 network-client 丢响应、查回、原 key 重试测试通过 |
| 回答纠偏 | 真正的原回答 + 待发送模型请求 | 新回答接续准入待 TC4 | 真实回答选区缺少纠偏入口的红灯 |

## 4. 命令与实际结果

运行目录为仓库根；PowerShell 下 Cargo 用本仓库 `tmp/tutor-continuation-tc0` 作为 TEMP/TMP。每组测试只运行以回答其对应失败问题；夹具调试错误修正后，记录最终运行结果。

| 检查／命令 | 要检测的失败；发现后动作 | 实际结果 | 原始输出 |
|---|---|---|---|
| `cargo test -p server --lib tutor`（新增前） | 原 Tutor 控制、源版本、提示／评分或观察已退化；归为既有基线 | 36 通过、5 忽略、650 过滤 | [server-tutor.log](tutor-continuation-tc0/server-tutor.log) |
| `cargo test -p server --lib mu10`（新增前） | 持久接纳／崩溃恢复的既有断点；独立列出，TC4/TC7 必须回看 | 2 通过、2 失败、2 忽略、685 过滤 | [server-mu10.log](tutor-continuation-tc0/server-mu10.log) |
| `cargo test -p runtime tutor` | 旧 Move/schema、Tutor 动态控制和上下文注入回归；保留原合同 | 7 通过、487 过滤；另两个 target 为 0 项，不计覆盖 | [runtime-tutor.log](tutor-continuation-tc0/runtime-tutor.log) |
| `cargo test -p memory` | 帮助条件、解释修订与投影存储回归；阻止误归咎新按钮 | 143 通过；doc tests 0 项 | [memory.log](tutor-continuation-tc0/memory.log) |
| `pnpm --filter @understand-book/web typecheck` | 既有 Web 类型不闭合；先标记编译基线 | 通过 | [web-typecheck.log](tutor-continuation-tc0/web-typecheck.log) |
| Web 定向单测（下方完整参数） | 原输入、回答选区、控制状态与网络恢复退化；保留原断言 | 5 文件、142 项通过 | [web-baseline.log](tutor-continuation-tc0/web-baseline.log) |
| `cargo test -p server --lib tutor_continuation_tests::continuation_` | 新夹具、真实存储与正常路径不成立；先修夹具再保留红灯 | 5 通过、8 预期失败用例默认忽略、691 过滤 | [server-new-baseline.log](tutor-continuation-tc0/server-new-baseline.log) |
| `cargo test -p server --lib tutor_continuation_tests::continuation_red -- --ignored` | 8 条行为缺口；后续按责任片转绿 | 8 条预期行为失败、696 过滤，未用编译错误代替 | [server-expected-red.log](tutor-continuation-tc0/server-expected-red.log) |
| `pnpm --filter @understand-book/web test:e2e tutor-continuation.spec.ts --grep continuation_baseline --workers=1` | 夹具打不开、选区丢失、原发送断链；先修夹具 | 4 项通过 | [browser-baseline.log](tutor-continuation-tc0/browser-baseline.log) |
| `$env:TC0_RUN_RED='1'; pnpm --filter @understand-book/web test:e2e tutor-continuation.spec.ts --grep continuation_red --workers=1` | 新直达帮助／回答纠偏入口缺失；交 TC2/TC5 | 5 条预期行为失败，均到达真实阅读／回答现场 | [browser-expected-red.log](tutor-continuation-tc0/browser-expected-red.log) |

Web 单测完整命令：

```powershell
pnpm --filter @understand-book/web test src/network-client.test.ts src/App.startup.test.ts src/components/RightRail.test.ts src/components/ConversationLearningGoal.test.ts src/useTutorControl.test.ts
```

Rust 预期失败用例使用带责任片说明的 `#[ignore]`，浏览器预期失败用例只在 `TC0_RUN_RED=1` 时运行；正常测试不会因为计划尚未实现而全部变红。显式运行红灯时保留真正的失败退出码与断言。实现相应合同后移除其 ignore/skip，并更新用例夹具。环境开关仅控制测试，不存在产品 feature flag。

定向 Server 基线使用模块前缀，避免 `continuation_` 同时选中既有 DeepSeek／演示续写用例。[纯空白输入排查](tutor-continuation-tc0/local-blank-probe.log) 单独运行通过，最终该用例已并入正常基线。交付前对照编辑前记录，已有接点源码／测试的修改时间与字节数保持一致；`lib.rs` 的新增差异仅为测试模块注册。

## 5. 七类失败的证据和接手点

| TC0 场景 | 实际观察 | 对应用例／接手 |
|---|---|---|
| 选区点帮助仍要补 prompt | 原“问 AI”点击后 Run 数为 0，补文字才为 1；四种组合均无直达帮助按钮 | Browser `baseline_*_manual_prompt`、`red_*_direct_help` → TC2 |
| 两项选择归一化成同一输入 | 相同 key、不同 option_id 返回 202 与同一 turn，未返回冲突 | `red_different_choices_conflict_for_same_key` → TC1/TC4 |
| choice／回答纠偏被记作活动回应 | 与 teaching_ref 混用时，各写入 1 条 chat_response；应在写入前拒绝歧义 | `red_choice_cannot_record_chat_response`、`red_clarification_cannot_record_chat_response` → TC1/TC5 |
| 丢失接纳结果后重复执行 | 多人 queued 断点恢复后同 key 仍为一次执行；本地同 key 重开后再次调用 Provider | `baseline_network_lost_acceptance_recovers_one_execution`、`red_local_lost_result_does_not_execute_twice` → TC4 |
| 暂停后旧 choice 继续生效 | 携 choice 的非空请求被当作普通请求接纳，没有经过新动作授权校验 | `red_paused_choice_is_not_an_ordinary_request` → TC4 |
| 题中原文求助后独立计分 | 两类题目均另起解释回合，原题 assistance_refs 为空；旧对象式题目 independent_support=1 | `red_source_question_help_is_assisted`、`red_object_question_help_is_assisted` → TC2/TC6 |
| 回答纠偏引用未进入请求 | 服务端当前请求只有 `This step is unclear`，原回答摘录丢失；Web 无对应回答操作 | `red_answer_excerpt_reaches_model_input` 与 Browser 回答选区用例 → TC1/TC5 |

E21 测试保留“定位原题”和“看到帮助”的区别：未来不接纳新解释回合时，测试再请求并展示原题提示，然后提交答案；不能仅凭返回原活动入口写 assistance。原帮助路径的绿色对照已证明正确记录可以得到 assisted_support=1、independent_support=0。

## 6. 既有问题与验证限制

**既有问题**：MU10 两项失败在新增 TC0 用例前已出现。`mu10_real_host_kill_commit_windows` 在 `mu10_tests.rs:345` 对恢复回合缺失的 admission_input 解包；`mu10_recovery_100_unclaimed_turns_keeps_original_inputs` 在 `:918` 期望 100 个带冻结输入的 pending turn，实际为 0。后一项此前对数据库中 queued 行数 100 的断言已通过。两项均未修复；TC4/TC7 不能据本片普通去重通过就宣称宿主恢复完整。

**已澄清**：本地纯空白普通聊天没有缺口。`validate_agent_turn` 的下游校验会拒绝，新增绿色基线保留这一事实。不要仅从首层 `validate_agent_input` 未 trim 就推断会产生空回合。

**验证限制**：

- 新 continuation 尚不存在。choice 用例使用方案约定的 JSON 载荷探测现有白名单、混用和暂停边界；不同 option 的用例使用占位 delivery_ref，不代表已完成建议生产／公开读取／合法选择验收。TC1 严格解析后先改为明确拒绝未知引用；TC3/TC4 以真实已交付建议补齐合法选择及异载荷冲突验收。
- 浏览器 API 是替身，PDF 来源解析响应是固定已核验范围；Server 另以真实 Book/AskQuote 验证范围。本片没有跑 PDF 导入、生产源映射或真实网络宿主的完整浏览器链路。
- 当前题中“原文求助后计分”红灯覆盖本地两类活动；多人新动作、两标签、所有接纳崩溃点、历史回答、账号切换、费用与新输入完整矩阵留给 TC4/TC7。
- 真实模型内容质量、真实读者、移动端新按钮、原生平台与演示宿主的新操作尚未验证。已有 Tutor 的 5 个忽略项及 MU10 的长负载／子进程辅助忽略项不计通过。
- 仓库既有 MU4、RN4 平台验收、Tutor T19 等待办沿原 checkpoint 指针；本片没有扩大修复范围。

## 7. TC1 接手

1. 从方案 §4 冻结 TeachingContinuation、互斥动作／回答引用、ResolvedInteraction 与 current_action 的序列化和 schema；长度上限放单一常量。
2. 共用解析先拒绝新输入的歧义组合及无效引用；保留普通空白拒绝、原文来源与真实用户补充，不借 teaching_ref 生成 chat_response。
3. 扩展 normalize 的字段保留，明确旧 Move/旧事件读取和旧二进制不兼容边界；按对应行为运行红灯，变绿后移除 ignore。
4. 合法建议选择的夹具随 TC3 的真实 MessageDelivered 建立；TC4 复用多人冻结输入，并补上本地 JSONL 接纳身份及恢复。持续保留本报告的已知 MU10 失败。
