# TO3：Tutor 当前状态的有效示例

日期：2026-10-12。代码基线：`93b476d`。状态：TO3 已实现，改动未提交、未部署。

## 工作步骤

- [x] 读取 checkpoint 全部冷启动入口、TO/TB 所有权、TC2A 轨迹、当前契约及 REA 固定版本 R1/R3。
- [x] 保存改前定义和固定三次模型基线。
- [x] 实现完整输入示例、状态投影、评分 submit 条件及建议选择的动作约束。
- [x] 验证最终 schema、Native/ReAct 请求、真实分派、事实／帮助／暂停边界及评估后的示例撤下。
- [x] 完成固定三次改后观察，记录输入成本和内容问题，更新方案、代码链路及 checkpoint。

## 实现

沿用 TeachingMove、TeachingTarget、LearnerContext 和现有证据所有权，属于纯技术契约补齐。教学指导保持 TB 所有权。

- `tutor::spec/input_examples` 提供 10 个完整输入：material、trace、三种 select（解释／带提示及揭示的比较／开放评分）、outside、assess、understanding、两种 evidence（来源辨析表现／hypothesis）。来源身份省略并由 Runtime 绑定，select 参数始终嵌套在 `move`。
- `spec_for_context` 按 active／observing／reference 与当前 UI 动作发布适用示例，移除 Runtime 接管的 capability／意图字段；暂停或不可用状态没有 Tutor 工具。assess 使用真实 pending_assessment_refs，hypothesis 使用真实 uninterpreted_fact_refs，无相应事实时不发布这两类示例。
- 用户选择 explain／compare／observe 建议后，schema 的 kind 限定为已选动作，与原服务端检查一致。真实补充原话仍沿原语义交给模型解释。
- `tool_schema::complete_spec` 将服务端已有的“评分活动必须允许 submit”投影到 schema；原 hint／reveal 所需内容约束保持。
- `run_outer` 在采样边界刷新 Tutor 定义，已完成评估的引用及示例随原评估指导一起撤下。
- 示例说明明确使用平均速度夹具，未把示例当作用户事实。测试通过真实原文、交付、提交、解释及持久记录证明对应输入可执行；其他材料的文字、位置、对象和表现引用按实际观察填写。

## 验证结果

验证针对状态投影遗留字段、错误嵌套、评分缺少 submit、未读来源、无用户原话形成解释、帮助内容缺失及暂停后越权；失败修相应示例／投影，保留业务拒绝。初次验证发现 ReAct 测试序列化顺序差异、读取类测试未结束活动教学、夹具评分引用超出已读段落及观察模式误用旧会话事实引用，均已修正。

| 入口 | 结果 | 证据 |
|---|---|---|
| `cargo +1.95.0 test -p runtime --lib -- tool_registry::contract_tests tutor --test-threads=1` | 40 通过，含类型导出；14 个状态／动作上下文、86 个投影示例、Native/ReAct 请求及四类 TC2A 反例 | [runtime-final.log](runtime-final.log)、[states.json](states.json) |
| `cargo +1.95.0 test -p server --lib tutor_to3 -- --test-threads=1` | 5 通过；10 个 canonical 输入经真实分派，观察模式与 UI 入口采用实际发布输入；来源、正式评估、帮助和暂停边界保持 | [server-examples.log](server-examples.log) |
| `cargo +1.95.0 test -p server --lib tutor -- --test-threads=1` | 103 通过、12 默认忽略；包含上述 5 项和评估后下一采样撤下示例 | [server-final.log](server-final.log) |
| `cargo +1.95.0 test -p server --lib mcp_contract_export_tool_definitions -- --ignored --test-threads=1` | 1 通过，导出 61 个实际定义 | [export.log](export.log)、[after.json](after.json) |
| `python docs/performance/tool-usability/to3/compare.py` | Draft 7 验证 61 个最终 schema、33 个发布示例及 14 个状态 schema；其他工具定义完整不变；实际模型配置一致；反例计算及原输出引文定位通过 | [comparison.log](comparison.log)、[sizes-and-results.json](sizes-and-results.json) |

正确性回归合计 **143 项通过**，不重复计数定向 5 项，也不把默认忽略项计为通过。两个模型采样入口另行显式执行，HTTP 成功与内容正确分别报告。运行中的 ts-rs serde 属性警告为既有警告。

## 固定模型观察

改前、改后各固定 3 次 `continuation_tc2a_live_fixed_input_comparison`，未追加采样。DeepSeek V4 Flash、temperature=0、原平均速度材料、固定首答、`continue_answer`、空补充及先 select 后交付时序一致；完整配置由脚本直接比较。Provider 请求超时 120 秒，实际每次采样输出预留／上限 32,768 token，各轮记录实际活动输入和累计 usage。

| 组／次 | 采样数 | 首轮估算输入 token | 累计 provider token | 参数拒绝 | 保存目标／教学交付 | incomplete |
|---|---:|---:|---:|---:|---:|---|
| 改前 1 | 3 | 17,444 | 69,154 | 0 | 1／1 | false |
| 改前 2 | 3 | 17,444 | 69,195 | 0 | 1／1 | false |
| 改前 3 | 3 | 17,444 | 73,168 | 0 | 1／1 | false |
| 改后 1 | 3 | 18,185 | 63,992 | 0 | 1／1 | false |
| 改后 2 | 3 | 18,185 | 68,518 | 0 | 1／1 | false |
| 改后 3 | 3 | 18,185 | 66,322 | 0 | 1／1 | false |

两组均 3／3 交付、0 参数拒绝，**本组未证明参数错误率下降**。首轮输入增加 741 token（4.25%）；全组累计 usage 从 211,517 变为 198,832，入口耗时从 143.83 秒变为 109.80 秒。输出内容不同且仅三次，不能据此宣称稳定成本或时延收益。实际请求、响应、结果、正文和持久目标分别见 [baseline](baseline/) 与 [after-runs](after-runs/)。采样期间未使用正式评估，评估完成后的刷新由最终确定性回归覆盖。

两组正文均出现条件概括问题：把等时写成算术平均成立的必要条件，或概括成等路程不能直接平均。独立反例取速度 10／10、时间 1／2，两种平均均为 10；等速等距离同样取等号。两段总时间为正时，差值为 `(v₁−v₂)(t₁−t₂)/(2(t₁+t₂))`。逐次引文和四个计算用例见 [content-findings.json](content-findings.json)。改后第 1 次还在正文重复了已结构化保存的两项 continuation；此项属于教学交付内容观察，参数本身合法。

## 输入成本与复现

Resident canonical schema 总量从 44,279 增至 47,850 UTF-8 字节（+3,571），其中 tutor.step 从 11,152 增至 14,723，description 增加 211 字节。本地／远程 MCP 仍为 12,131／15,453 字节，定义完整不变。实际 continue_answer schema 从 10,141 增至 12,895；仅示例占 2,037 字节。直接解释状态仅示例占 836 字节，无表达的 observing 为 75，reference 为 45；逐状态数据见 sizes-and-results.json。

环境：Rust `+1.95.0`，`CARGO_TARGET_DIR=D:/codex-build/understand-book-no-progress-195`，dev/test debug=0、incremental=0、构建并行度 2，TEMP/TMP=`D:/codex-build/to3-temp`。导出定义设置 `TOOL_CONTRACT_SNAPSHOT`；导出状态设置 `TO3_STATE_SNAPSHOT` 后执行 Runtime 契约测试。显式实模入口使用全新 `TC2A_OUTPUT`；历史输出不可覆盖。基线定义沿用 [TO2 after.json](../to2/after.json)，保存为 [before.json](before.json)，Tutor 基线源码另存 [before-tutor.rs](before-tutor.rs)。

REA 阅读范围：固定提交 `2ff850de68dc9b05c6faf2bbfd26d56bc2d5d27c` 的 R1 工具设计／设计 skill、R3 示例定义／examplesFor／advertisedInputExamples 测试，出处见 [TO 方案第 6 节](../../../切片方案-工具契约与Agent可用性优化.md#6-rea-原仓库怎么找怎么读)。沿现有 Registry 验证实际发布输入，没有新建注册层。

## 已知限制

- 示例是调用形状与所需前提的演示，不能作为用户事实或替代 Agent 对材料、引用和教法的判断。非当前引用示例仍采用固定平均速度夹具，实际调用需换成本次材料和事件。
- TC2 的数学条件问题、改后单次重复建议及真人学习效果仍未解决；这次没有生产部署、浏览器或真实读者验收。
- 采样计划原记录把 `OuterConfig.token_budget=120000` 当成累计停止线；检查实现后确认它是保留字段，当前不负责停止。六次实际 usage 均低于该数值，固定三次／组与逐请求上限实际生效；本记录不宣称存在硬累计 token 或货币费用封顶。费用未由 Provider 返回，未用估算值冒充实付费用。
- TO1 记录的本地／Resident 逆序正文范围问题保持独立待办。TO4–TO14 与 TB 工作沿各自方案继续。
