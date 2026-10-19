# 自主教学多轮验证与理解视图修复

2026-10-16 至 2026-10-17。本次承接“用户只表达困惑和不完整理解，不安排教学步骤”的验证要求，教师与模拟学习者使用隔离的 gpt-6.1-sol 子代理，另由同模型独立审阅实际对话。生产修改基于当前工作树，改动前的六个相关文件保存于 `autonomous-change-baseline/`；仓库中其他未提交工作保留。

## 结论

教师能够主动建立关系框架，并针对用户暴露的具体误解更换表示方式；肯定用户时，也能区分眼前例子中的正确表达与独立应用能力。但这组七轮对话没有出现教师主动邀请用户完成一个新判断的行为，不能把它写成“已经自主跑通学习者尝试与反馈”。用户表达由模拟学习者或诊断脚本提供。教师已经表现出组织解释和响应式纠正的能力，主动组织学习者尝试仍是这次暴露出的缺口。

本轮修复了一个明确的实现问题：同一回合内，设计创建时的理解快照会落后于后来保存或修订的理解证据；已解释过的事实也可能继续被列为未解释。修复后，下一次模型请求消费最新理解，原始表达及当时的帮助条件保持不变。真实对话中的模型请求已确认这一刷新实际发生。这项修复保障反馈依据的时效性；本次实验没有证明它会使教师主动提问或必然减少首轮信息量。

逐字记录见[完整对话](autonomous-conversation.md)，内容判断见[独立语义审阅](autonomous-semantic-review.md)。这两个文件以实际交付为依据，原始请求和工具记录另保留供追溯。

## 对话中的具体表现

用户最初把 candidate、accepted、macro、theme 想成一道接一道的筛选，因而不理解“没进章节主干的点为什么又进主题”。教师自己提出“保存下来”和“承担什么角色”两个观察方向，并画出章节、主题从完整候选目录分出的两条路径。它不是逐个复述字段定义，而是用一个关系框架解释现象。首轮同时讲到最终并集、throughlines 和三种去向，内容准确但偏密；第二、三轮逐渐收窄。

第四轮用户说，先前只是对着解释把句子说顺，仍感觉一边撤选会让另一边也失去同一个点。教师改用“一张候选卡、两张引用名单”，展示只撤销章节选取前后的四行对照，定位到“划掉名单中的引用”和“拿走候选本身”的混淆。固定候选目录、主题证据与选择、章节主干等条件后，解释了为何最终结果仍包含该点。这是自行选择的静态示范，没有用户指定表示形式。

第六轮在真实新建聊天后，用户由最终包含 Yi 反推本章一定 accepted 了 Yi。教师用三行相同结果、不同来源的表，说明“至少有一路选择”不能推出“就是本章选择”，并交代多章情况下还可能来自别的章节。第七轮用户对着表修正后，教师只确认有帮助的局部解释，保留其脱离表时尚未稳定应用的自述。

这七轮都没有主动留下一个由学习者来完成的新推断；没有普通表达邀请，也没有主动组织迁移尝试。用户的第二、三、五、七轮表达来自隔离学习者，第四六轮来自主线程的诊断输入。因此，“能根据表达纠正”有直接证据，“会主动引出表达并检验修正后的使用”尚无。

## 实现修复

实际链路是 `prepare_agent_chat` 冻结输入事实 → `tutor.step(design/evidence/assess)` 接受变化 → Resident 下一次采样 → `select` 与交付。问题出在中间的状态投递：冻结输入与可修订判断没有彻底分开，新的证据保存成功后，设计时的旧理解仍可作为当前视图存在。

- [`crates/server/src/teaching.rs`](../../../crates/server/src/teaching.rs)：假设证据、假设修订和来源辨别证据接受后，返回现有有界查询生成的最新理解视图。基于本回合最初冻结的候选事实重新计算 `uninterpreted_fact_refs`，不会因本轮新增设计或帮助事件改变原始输入窗口。
- [`crates/runtime/src/tutor.rs`](../../../crates/runtime/src/tutor.rs)：原话、请求和输入时的帮助条件继续冻结；当前判断、待回应事项、证据版本及未解释事实列表移入动态视图。当前设计不再混在冻结输入中。
- [`crates/runtime/src/orchestrator.rs`](../../../crates/runtime/src/orchestrator.rs)：分别替换当前教学设计与当前理解片段；设计、证据、评估成功后，下一次采样使用更新后的状态。
- [`crates/runtime/src/tool_result.rs`](../../../crates/runtime/src/tool_result.rs)：历史工具回执保留完整引用和必要结果，不重复携带会过时的完整理解快照。完整原始结果仍在追踪记录中，当前状态由可替换片段提供。

没有新增学习阶段、掌握分数、每轮必问规则或强制演示门槛，也没有改变生产默认模式。教学提示模块没有为这轮实验增加“多问问题”指令；控制文字仅随实际数据分层调整。

真实执行证据见 [`autonomous-03/request-0035.json`](autonomous-03/request-0035.json)：最新 `runtime_state_snapshot.v1` 中只有一份当前设计和一份当前理解视图，包含刚接受的 `hypothesis-inverse-membership-06`，`evidence_watermark=6`，`uninterpreted_fact_refs=[]`。这验证了生产循环向模型投递更新后的判断，而非只验证工具返回字段。

## 运行与恢复记录

| 运行 | 实际结果 | 对本次结论的用途 |
|---|---|---|
| `autonomous-01` | 2 轮，12 次模型请求 | 发现验证夹具强制了 guided inquiry，不能代表正常默认选择；保留诊断，不作为默认模式效果证据 |
| `autonomous-02` | 3 轮交付；第 4 轮在第 24 次响应处因文件 JSON 格式错误返回 502 | 保留失败响应和真实失败回合；修正文件发布方式，不将失败包装为成功 |
| `autonomous-03` | 正常默认模式，前 6 轮交付，共 37 次模型请求；原第 7 轮发出第 38、39 次请求，后者未获得响应 | 主要内容证据；第 26 次请求真实调用结构化压缩辅助通道并继续成功执行 |
| `autonomous-03-retry` | 保留前 6 轮状态，第 7 轮新回合重试交付，4 次模型请求，HTTP 200 | 补齐第 7 轮；不宣称七轮连续无中断 |

文件发布现在先用 Python 生成并解析 pending JSON，随后原子改名。该调整属于验证传输，不改变生产教师行为。

恢复采用服务器现有 JSONL 启动恢复流程。原状态目录复制到重试目录后再加载，原第七轮 `turn_ae7b2d219a987bad` 保留输入和一次搜索并标记中断；新回合 `turn_ae7b2c219a9879fa` 重新提交同一用户表达。前六轮没有重放，原 `request-0039.json` 没有补造响应。恢复细节见[重试记录](autonomous-03-retry/experiment.md)，最终执行日志见[恢复运行日志](autonomous-regression/teaching-autonomous-03-retry.log)。

## 回归依据

定向回归检测的是：设计与证据在同轮先后更新时，旧判断是否继续冒充当前判断；修订后是否仍保留旧版本；已解释事实是否仍被标记待解释；原始用户表达和帮助条件是否被错误改写；大回执是否丢失完整引用。出现上述失败就修正状态分层或投递链路，而不通过放宽教学判断来令测试通过。修复前的有效失败保留于[理解刷新失败记录](autonomous-regression/teaching-interpretation-refresh-red-corrected.log)及[未解释事实失败记录](autonomous-regression/teaching-uninterpreted-refresh-red.log)。

最终相关回归：

| 范围 | 结果 | 原始日志 |
|---|---|---|
| Runtime 完整串行回归 | 619 通过，0 失败，8 ignored | [runtime](autonomous-regression/teaching-interpretation-refresh-runtime-final.log) |
| Server 教学设计 | 6 通过，0 失败，1 ignored | [teaching design](autonomous-regression/teaching-interpretation-refresh-server-teaching_design_.log) |
| Server 理解视图 | 1 通过，0 失败，1 ignored | [understanding](autonomous-regression/teaching-interpretation-refresh-server-tutor_understanding_view_tests.log) |
| Server 适应性反馈 | 2 通过，0 失败，2 ignored | [adaptation](autonomous-regression/teaching-interpretation-refresh-server-tutor_adaptation_tests.log) |
| Server 教学循环 | 33 通过，0 失败，5 ignored | [loop](autonomous-regression/teaching-interpretation-refresh-server-tutor_loop_tests.log) |
| Server 接续 | 62 通过，0 失败，8 ignored | [continuation](autonomous-regression/teaching-interpretation-refresh-server-tutor_continuation_tests.log) |

Server 本轮相关回归共 104 通过，17 ignored。另单独执行了真实模型交互验证，不能把 ignored 数量当作已执行模型验证数。没有运行会覆盖旧 TB2 证据的回放。恢复支持修改了测试 harness，重新编译并实际执行恢复验证成功。Memory 与 Web 未在本轮修改，其旧验证结果仍留在[上一阶段实现记录](implementation.md)，不计为本轮重跑。

## 已知问题与验证边界

- 主动引出用户尝试仍未出现。这一组能确认响应式反馈，不能宣称自主教学闭环全部达标。已有工具合同允许正常默认模式下使用表达邀请、guided inquiry 与演示创作，当前证据没有指向一个禁止这些行为的运行时门槛。进一步改变选择机制需要保留用户意图与任务判断，不能把本次缺口直接改成每轮必问。
- 首轮信息偏密；后续能够收窄。没有真人负担或学习保持数据，不能推出稳定的信息量控制效果。
- 本组只生成静态图与表，没有生成、打开或操作交互页面。静态表示适合这组关系问题，但它不能验证“会主动选择交互演示页”。此前 `sol-live-03` 的页面由用户明确请求，证明的是页面接入与反馈链路；与本组自主选择证据分开。
- 第四、六轮是诊断脚本，第二、三、五、七轮为模型模拟学习者。独立内容审阅也由模型完成。这些材料不能替代真人学习效果研究或线上默认模型表现。
- 第七轮经历进程中断和恢复重试；新教师子代理从真实恢复请求继续，原中断材料完整保留。本次为本地实现与验证，未部署到线上。
