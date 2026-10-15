# TC3 — 随实际回答交付下一步建议

2026-10-11；状态：**TC3 工程、定向回归和本片小规模内容验证完成，未提交、未部署。** 沿已确认的 ADR-0163 §3／§6 与 TC3 切片执行，用户本轮明确授权继续。TeachingMove、TeachingContinuation、MessageDelivered 与公开投影均沿 CONTEXT 既有定义。

## 实施与验证计划

1. 接通当前 active 工具合同与可选建议指导，保持直接回答优先。验证普通解释无建议仍成功、有建议能被同一 select 接受；失败则修正模型工具面与服务端合同。
2. 验证每回合最多一组、建议继承完整已读目标、公开文案来源、正式活动抑制。失败则收紧原 select 校验，不新增推荐存储或评估 Agent。
3. 复用 MessageDelivered 与 activities 读取。验证未保存／失败无公开候选、回执补写幂等且零 Provider 调用、公共视图无私有评分字段；失败则修正交付或投影边界。
4. 固定材料真实模型各运行一次：普通解释可自行建议、明确只要当前答案。保存完整请求、回答、事实、投影和调用数，人工核对建议具体、不重复、可在已读范围兑现。结果不理想时保留原记录并修正通用策略。

## 验收口径

公开回答与建议的实际内容优先；内部目标错误而公开回答正确且没有后续影响证据时，记为内部质量观察，不单独判整轮失败。既有 TC2A 第三次复验的公开内容错误继续保留，详见[复验记录](tutor-continuation-tc2a/recovery-20261011/README.md)。

## 第一轮真实模型观察与修正

[原始记录](tutor-continuation-tc3/live/)固定两个场景，各执行一次。解释场景 8 次采样、135,042 token，保存一条实际 Move／交付和一组两项建议；仅读取场景结果不新增模型调用。然而推荐项要求再做一遍正文已完成的等距离数值对比，备选项也重复了正文已说明的正时间条件，未通过本片“不重复”要求。公开正文基本定义与 80／90 算例正确；“慢的那段自然占用更多时间”一句需保留等距离条件，另作 TC2 内容观察。

只要当前答案的场景 4 次采样、40,630 token，incomplete=true、AGENT_NO_PROGRESS，没有教学交付或建议。原夹具把标题放在父节点而没有任何叶节点覆盖，导致 `book.search_text` 返回 SEARCH_SOURCE_INVALID；这不是 TC3 建议抑制造成的失败，不能当成“无建议成功样本”。已让首叶覆盖标题，选区偏移相应调整，并加入真实检索回归。

依据这轮证据，`tc3.v2` 指导要求先确定本轮解释范围，再逐项排除该范围内已完成的建议；select 回执提醒最终回答完成本轮范围，把可选后续工作留待选择。没有改目标先提交时序、增加模型调用或审查 Agent。首轮还出现 move 字段错放到顶层；错误轨迹会保留顶层候选，现已一并剔除顶层 continuation 与非法字符串 move。完整首轮失败原样保留，不用后续成功替换。

交付校验同时覆盖：已选择但没有任何回答内容，不生成交付事实；不完整回答保留实际文本事实但不公开建议；暂停后的真实回答保留交付，建议状态为 stale。

## 第二轮内容观察

[v2 原始记录](tutor-continuation-tc3/live-v2/)沿相同文字问题和材料，各运行一次；为聚焦建议生成，两场景均附带真实选区，并采用修正后的源叶覆盖。该轮不是单变量因果对照。两个回答均完整交付；只要当前答案的场景实际返回“平均速度是总路程除以总时间。”，没有建议，读取追加调用为 0。

解释场景仍重复：推荐项要求用数字对比分段平均，但 `move.prompt` 和公开正文已经包含 60／20、30／40 的对照；备选正时间条件也已讲完。因此失败发生在建议选择时，不能仅归因于最终回答扩大范围。v3 把“不重复”落实到建议 schema 的 label／purpose 描述，要求用途说明相对本轮解释的新收获，优先一个改变条件、辨析或观察的新步骤，禁止仅换数字重复运算；只追加一次 explain 场景复验，已通过的 answer-only 不重跑。

所有请求记录保留公共任务输入、指令、工具表和工具结果，省略 Provider 私有续接字段；回答、参数、交付事件、公开投影及调用计数可独立核对。

## 最终内容证据

[v3 实际回答](tutor-continuation-tc3/live-v3/explain/answer.md)说明定义、总时间的作用和正时间条件，并给出单一行程 `120÷2=60` 的回代示例。[原始公开建议投影](tutor-continuation-tc3/live-v3/explain/public-continuations.json)包含：

- 推荐：比较两段等长路程的速度算术平均与总路程／总时间。正文只做过单一行程回代，尚未完成两段数值比较；这一项把定性理由落实为新的定量对照。
- 备选：在行程中加入一段停顿，观察平均速度如何变化。正文提到停顿消耗时间，但没有进行保持路程、改变总时间的观察；可直接由已读定义兑现。

两项保持当前“总时间作为分母”的焦点，未承诺跨材料、评分或额外工具权限。实际只保存一个 MoveSelected 与一个 MessageDelivered，未为两个待选项生成教学活动；重复读取追加 Provider 调用 **0**。该样本经一次 target 类型错误纠正后完整交付，不能据此宣称所有输入稳定。

无建议样本沿 [v2 单句回答](tutor-continuation-tc3/live-v2/answer-only/answer.md)：实际公开文本只有定义，保存一条 Move／交付，公开 continuation=null。该项已通过，v3 未重复采样。

| 版本／场景 | Provider 调用 | 累计 token | Move／交付 | 建议 | 内容结论 |
|---|---:|---:|---:|---|---|
| v1 解释 | 8 | 135,042 | 1／1 | 两项 | 重复已完成内容 |
| v1 单句 | 4 | 40,630 | 0／0 | 无 | 夹具检索失败，未完整交付 |
| v2 解释 | 5 | 82,933 | 1／1 | 两项 | 选择时已与正文重复 |
| v2 单句 | 4 | 55,986 | 1／1 | 无 | 单句请求正常交付 |
| v3 解释 | 6 | 120,996 | 1／1 | 两项 | 有具体新增用途，当前范围可兑现 |

五个原始结果全部保留，累计 provider usage **435,587 token**；不是单次上下文、货币费用或同任务成本基准。机器汇总见 [summary.json](tutor-continuation-tc3/summary.json)，每个 result.json 同时含公开回答、事实和投影。真实模型验证证明这里有通过样本，不是通用准确率或用户学习效果结论。

## 确定性验证

- [初始红灯](tutor-continuation-tc3/red.log)：4 个新增行为用例均因旧实现拒绝 continuation 失败。
- [Server 最终回归](tutor-continuation-tc3/server-final.log)：78 通过、10 默认忽略，包含 8 个 TC3 测试及原来源、正式评分、帮助归属、暂停、交付与无进展恢复。
- [Runtime 最终回归](tutor-continuation-tc3/runtime-final.log)：59 通过、1 默认忽略，覆盖实际工具面、非法候选轨迹隐藏、按需指导、导航／演示与暂停。
- 实模录制补充选区输入、去除 Provider 私有续接字段后，重新编译并运行 [TC3 夹具验证](tutor-continuation-tc3/fixture-final.log)：8 通过、1 忽略；生产代码与上一组最终回归相同。
- v3 只调整指导文字和 schema 描述，最后 [Server TC3](tutor-continuation-tc3/schema-final.log) 8 通过／1 忽略、[Runtime 合同](tutor-continuation-tc3/runtime-schema-final.log) 6 通过，随后执行上述单场景实模；没有扩展数据类型或选择消费合同。
- 中间两次测试修正完整保留：首次编译纠正测试中的控制 revision 字段；首次 Server 75 通过／1 失败，唯一失败是新测试误用了不存在的 `/tutor/behavior`，已改为原 `/tutor/action`。这两项不是生产逻辑失败。

复现环境：Rust `+1.95.0`，`CARGO_TARGET_DIR=D:/codex-build/understand-book-no-progress-195`，dev/test debug=0、incremental=0，TEMP/TMP=`D:/codex-build/tmp`，`-j 2`。Server 筛选 `--lib tutor`，Runtime 筛选 `--lib -- tutor agent_prompt guided_read context_fragment`，均串行测试。付费实模入口默认忽略，单独运行 `continuation_tc3_live_answer_and_optional_suggestions --ignored --nocapture --test-threads=1`，需全新 `TC3_OUTPUT` 目录。

## 已知限制与下一步

本片交付和读取建议，当前公开状态为 offered；选择消费归 TC4，界面归 TC5，展示／选择事实归 TC6。建议不是未来教学活动；结构校验不宣称能识别用自然语言泄露答案。内容验证尚未实际执行用户选择，完整兑现与纠偏沿 TC5／TC7；真实读者人数仍为 0，TC2 既有公开内容问题继续保留。

下一片 TC4 接通单次选择的持久接纳、消费状态与两种宿主恢复，保留 head／来源／控制 revision 边界及原题帮助归属。原 MU10 两项失败继续从该片跟进。本次没有部署或改造前端按钮。
