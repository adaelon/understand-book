# TO5：Goal、Reader、Memory 用途与动作示例

日期：2026-10-12。基线：`93b476d` 加主工作区 TO3–TO4、TO7–TO12。状态：已完成，未提交、未部署。

## 实施步骤

- [x] 阅读 checkpoint 冷启动入口、TO5、R1–R3 固定源码与 TO/TB 所有权。
- [x] 保存改前工具定义、实际 Provider 请求及固定任务输入。
- [x] 在现有定义旁补 Goal 三分支、Reader 笔记／高亮及四类 Memory 示例。
- [x] 验证最终 schema、真实回合依据、持久化回执、纯解释无写入及相关回归。
- [x] 更新方案、代码链路和 checkpoint。

## 范围与验证

沿用 ResidentGoal、Goal 工作计划、阅读笔记、Note 正文放置和私人记忆的现有含义，属于纯技术扩展。输入示例和说明由 `declared_tool_specs` 所有，分支约束仍由 `tool_schema::complete_spec` 所有。

契约测试检测 operation 字段混入、发布时示例丢失及真实解析拒绝；失败修相应定义或示例。真实运行检测虚构 revise 依据、笔记被错存成普通记忆、回执与重新打开后的记录不符，以及纯解释任务产生写入；失败修具体原因，不以 schema 合法代替执行成功。模型效果对照由 TO14 承接。

## 实现

`orchestrator::declared_tool_specs` 发布 10 个完整输入。`non_book_routing_guidance` 同步用途与避免场景，供 TO7 的发现结果复用。

| 工具 | 示例与执行语义 | 成功回执 |
|---|---|---|
| goal.update | working 保存 focus/next_move/计划；refine 仅用于目标起始回合；revise 引用当前用户回合及原话 | goal_id、revision、changed；没有变化返回 changed=false |
| reader.note | 保存用户要求的文字，并以真实 lid 作为正文放置 | ok、note_id、CREATED/EXISTING；重复保存复用笔记 |
| reader.highlight | 唯一原文 quote 或整段，两种输入 | ok、highlight_id；保存 session 高亮 |
| memory.save | highlight 保存提供的内容；position 保存位置记忆；qa 保存实质问题原话；context 保存有来源的读者背景或偏好 | 已保存记录的 mem_id、type、layer、anchor、content、citations |

示例中的位置与回合 ID 必须替换为真实对象。Goal requirements 的依据只能来自原始或当前用户回合；工作计划不改变要求，也不证明交付。原 `complete_spec` 分支与字段约束保持。

笔记与 Reader 高亮先保存私人记录，再尝试更新 Reader 选中状态；`reader_effect.status=not_applied` 表示界面动作未应用，不能据此否认已保存的记录。`memory.save(type=highlight)` 保存调用者提供的内容，不定位原文范围，也不选中 Reader；源文高亮使用 `reader.highlight`。用户偏好的 context 示例没有书籍引用，`citations=[]` 合法，不能凭空编造书源。原存储、授权与事实来源规则保持。

固定任务暴露两处意图识别缺口：`记为……笔记` 未激活笔记工具，`不保存笔记、高亮或记忆` 却激活了写工具。`tool_exposure::classify_turn_intent/explicit_action` 在现有词表补 `记为` 与 `不保存`；分句后的“只高亮这段”仍单独授予高亮权限。失败保存在 [tests.log](tests.log)。同轮另外两项失败是新测试夹具错误：查询材料缺少概念节点、ReAct 使用了结构体而非实际发布对象的字段顺序；修正夹具后验证原协议。

## 复现

```powershell
$env:CARGO_TARGET_DIR='D:/codex-build/understand-book-no-progress-195'
$env:CARGO_PROFILE_DEV_DEBUG='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
$env:CARGO_INCREMENTAL='0'
$env:CARGO_BUILD_JOBS='1'
$env:TEMP='D:/codex-build/to3-temp'
$env:TMP=$env:TEMP
$env:TO5_DIR=(Resolve-Path 'docs/performance/tool-usability/to5').Path
$env:TO5_SPEC_SNAPSHOT=Join-Path $env:TO5_DIR 'after.json'
cargo +1.95.0 --config 'profile.test.package.runtime.codegen-units=16' test -p runtime -p server --lib -- to5 --test-threads=1
python docs/performance/tool-usability/to5/validate.py
```

相关回归（沿用上述环境）：

```text
cargo +1.95.0 --config 'profile.test.package.runtime.codegen-units=16' test -p runtime --lib -- tool_registry::contract_tests goal::tests explicit_reader_writes explicit_highlight_quote dispatch_memory_save multihop_query_then_save_then_finish tool_search_to7 g2_goal_update explicit_note_and_highlight explicit_navigation --test-threads=1
cargo +1.95.0 --config 'profile.test.package.runtime.codegen-units=16' test -p server --lib -- resident_goal_update_is_saved mu1c_old_run --test-threads=1
```

## 验证结果

Runtime **44 个不同用例通过**，Server **3 个不同用例通过**。Runtime 最终 21 项见 [verified-tests.log](verified-tests.log)，其余已通过项沿用 [final-tests.log](final-tests.log)；Server 新增 Goal 用例最终通过见 [goal-bound-tests.log](goal-bound-tests.log)，另外两项回归通过见 [server-tests.log](server-tests.log)。本次新增 5 项测试。

- [工具定义及请求](after.json)：10 个示例通过最终 schema、Registry 和 Native／ReAct 投影；Goal 另经 `GoalUpdate` 解析。原 operation 字段集不变，跨分支字段不能混入。
- [Reader 链路](reader.json)：实际 Runtime 保存笔记、再次保存同一笔记、保存唯一原文高亮和整段高亮；回执分别与重新打开后的记录 ID、类型、内容、LID、正文放置和范围对应。重复笔记返回 `EXISTING`，没有新增记录或创建 effect。
- [Memory 链路](memory.json)：四种公开类型均真实落盘；qa 前执行现有 Book query 的 PlanGate 和支撑判断，保存的是原始问题；context 保存用户原话且引用为空；position 属于 session，其余为 long_term，Reader 状态不变。
- [纯解释](explain.json)：带否定保存意图的原文解释请求没有暴露三种写工具，没有工具执行、记忆记录或 Reader 变更。
- [Goal 链路](goal.json)：实际 HTTP 聊天产生原始和后续回合，后续请求显式携带原 `goal_id`。working／refine／revise 回执与下一采样前重新加载的持久记录对应；虚构原话经 HTTP 拒绝，旧回合依据与后续回合 refine 经同一领域处理器拒绝，状态均保持。不变更新的领域结果为 changed=false。正常修订保留工作计划，Goal 在实际答案结束后完成。
- [独立 Draft 7](validation.json)：10 个发布示例、11 个成功执行输入通过，26 个字段／类型反例被拒绝；3 类语义依据错误的输入形状合法，执行时拒绝。去掉 examples 后，输入约束与改前逐值一致。

四份工具定义合计 **4,711 → 8,006 字节（+3,295）**。这是序列化定义大小，不是 billed token 或模型收益。改前请求和固定任务见 [before.json](before.json)，缺少示例的改前失败见 [red.log](red.log)。

所有中间失败保留在本目录日志中。最终夹具使用实际大写笔记状态、结构化 query 协议、唯一调用 ID 和显式 Goal 选择；即时重复调用仍受既有无进展规则限制。无效依据测试区分 HTTP 路径与同一领域处理器，未修改生产停止规则。

## 已知限制

- 已有 Goal 完成判定对笔记去重回执覆盖不足：当保存笔记被列为 `reader_action`，而本轮 `reader.note` 返回 `EXISTING` 且没有其他 Reader effect，记录已保存但 Goal 仍可能报告动作未完成。代码依据为 `orchestrator::dispatch_registered` 只对 `NoteSaveStatus::Created` 生成可撤销 Note effect，`run_effects` 不补计去重回执，`ResidentGoal::objective_gap` 只计 Reader effects。本片保留去重及撤销语义；后续应让完成判断消费实际保存回执，不能为已有笔记伪造可撤销创建动作。
- 本片使用脚本模型及本地 HTTP Provider 夹具，验证实际工具合同、Runtime 分派、持久化与下一采样。真实模型选择和任务完成率收益由 TO14 验收；实验沿 [模型约定](../to10/README.md#实验模型约定)。
- 本轮没有生产部署。

## 参考

已阅读 REA 固定提交 `2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c` 的 R1 tool-design／rea-tool-design，R2 toolContractTypes／toolContracts，R3 toolContractHelpers／toolContractExamples／advertisedInputExamples.test；借鉴原所有者维护用途、完整输入示例及实际发布后的解析验证。入口见 [TO 方案第 6 节](../../../切片方案-工具契约与Agent可用性优化.md#6-rea-原仓库怎么找怎么读)。
