# Q1 工作区缓存实验：输出质量复核

本报告保留 Q1 的固定输入回放和后续真实 Server 交付两种证据，分别判断候选讲稿、工具动作及实际交付。审阅只读取已经落盘的请求和响应，没有追加供应商请求，也没有编译或修改生产代码。修复前六份实际交付及不利样本仍保留在 [quality-review.md](quality-review.md)。

Q1 真实运行完成两问交付，核心目标、梯度和数字正确，追问的实质修订以新动作标识进入了最终保存和公开正文。它证明本次没有再发生修订通过审核却因沿用旧标识而无法落地的情况。仍有一句缺少适用条件的参数分散结论；审核虽已识别“小 λ 过拟合照旧”的正确性问题，依然把它列为可选建议。因此最终内容较修前解决了若干具体问题，但审核分类与全部讲解质量不能标为全通过。

## 固定输入回放

[live-active-q1/results.json](live-active-q1/results.json) 完成计划中的 16 次请求，baseline/candidate 各 8 次，按 AB/BA 顺序各重复两轮。两版都采用 Q1 公共指导，分别由生产函数生成 `requests/baseline-q1`、`requests/candidate-q1` 输入。四个状态依次为文字解释、已接受的展示设计、文字公式追问、已接受的展示设计；每次用户追问使用新的 Run 状态、保留固定原始历史。新模型输出不会进入后一条输入，工具调用也未执行。因此本组可以评估动作选择和新讲稿内容，不能证明用户已收到讲解、审核已接纳、页面已经生成或学习者已掌握。

全部 16 条响应完整结束，`response_complete=true`、`output_truncated=false`。其中 15 条返回原生工具调用，1 条返回普通中文 content，没有观察到上一轮禁用工具实验中的 DSML 泄漏。

| 当前状态 | baseline | candidate | 判断 |
|---|---|---|---|
| index 4，直接讲解训练/验证/测试集 | 两轮均 `tutor.step select`，两份完整候选讲稿 | 两轮均 `select`，两份完整候选讲稿 | 两版均直接回应，不改教学设计，不新增题目 |
| index 6，先用文字解释 L2 公式和算例 | 第一轮 `select`；第二轮普通中文正文，无 `select` | 两轮均 `select`，两份完整候选讲稿 | 候选四次文字请求的动作选择稳定；baseline 第二轮的正文不能自动算成已审核交付 |
| index 5、7，展示设计状态 | 四条均为 prepare 或进度首跳 | 四条均为 prepare 或进度首跳 | 未执行后续流程，不计完整解释或页面质量通过 |

### 直接文字解释与公式追问

candidate 的 index 4 见[第一轮](live-active-q1/05-r1-candidate-04-response.json)、[第二轮](live-active-q1/09-r2-candidate-04-response.json)，index 6 见[第一轮](live-active-q1/07-r1-candidate-06-response.json)、[第二轮](live-active-q1/11-r2-candidate-06-response.json)。四条均用 `operation=select`、`kind=explain`、`interaction_intent=direct_explanation`，直接提供完整 `delivery_text`，没有为普通文字请求重开 design 或调用 author，没有测验、要求先答题或宣称学习者已经掌握。

两版 index 4 的核心内容均正确区分训练集拟合参数、验证集选择模型及超参数、测试集最终评估。A/B 数值保持 `0.01/0.30` 与 `0.08/0.12`，按相同划分和指标比较，在当前验证集上选择乙；正文也保留验证结果不是对所有未来数据保证的边界。

两版 index 6 的四份新讲解均正确写出 `J=L_train+λΣw_j²`；λ=0 时目标为 `0.05` 和 `0.09`，λ=0.01 时惩罚为 `0.10` 和 `0.01`、总目标为 `0.15` 和 `0.10`。candidate 第二轮补充的十倍惩罚和训练损失优势 `0.04` 也正确。四份讲解都区分目标函数的排序与验证误差，不声称乙的目标更小就必然泛化更好，并说明用验证集或交叉验证选 λ、测试集不反复参与调参。

baseline [第二轮 index 6](live-active-q1/15-r2-baseline-06-response.json) 是正常、完整的中文解释，不是协议文本；它也包含正确算例和泛化边界。但这条响应没有 `tutor.step select`，并在当前“先用文字讲清楚”问题后附加了“怎样做成直观比较”的页面方案。它未执行页面动作，不能判为页面已经交付；固定回放也没有执行 Runtime 后续处理，不能将这条直接计为已审核的用户回答。

八份文字输出中使用的 `source_ref` 均来自当前请求中已绑定的两份材料：`source_ref_c52685f9e6cb9140` 对应《过拟合与泛化》，`source_ref_679b2138443382f8` 对应《正则化与模型选择》，没有未知来源 ID。可对照[实际 candidate 请求](live-active-q1/05-r1-candidate-04-request.json)及[公式追问请求](live-active-q1/07-r1-candidate-06-request.json)。引用是否呈现仍按现有可选政策判断，不把未附链接本身作为失败。

### 未审核输出中仍需保留的问题

1. **由误差表现反推确定成因。** candidate [第二轮 index 4](live-active-q1/09-r2-candidate-04-response.json) 最后说“训练误差持续下降而验证误差上升，说明模型在贴合训练样本特有的细节”，又把“训练与验证误差都高”直接判断为“属于欠拟合”。原文将前者描述为一种典型过拟合表现、后者描述为欠拟合的常见表现，均不是仅凭误差数字确定成因的充分条件。baseline [第一轮 index 4](live-active-q1/01-r1-baseline-04-response.json) 也有同类确定归因。讲解前部的条件解释和末尾的未来数据保留，没有消除这些反向推断的缺口。
2. **基线仍有过强正则化必然欠拟合的表述。** baseline [第二轮 index 4](live-active-q1/13-r2-baseline-04-response.json) 和[第二轮 index 6](live-active-q1/15-r2-baseline-06-response.json) 使用“惩罚过强也会……变成欠拟合”，而材料保留“可能”。其中 index 4 最后还将过拟合、欠拟合概括为两种误差“走向相反与同向”，混淆了误差水平、变化方向与机制判断。candidate 的两份公式追问保留了“可能欠拟合”，没有在这处复现无条件断言。
3. **两版展示计划仍预设未经材料保证的 U 形曲线。** candidate [第一轮 index 7](live-active-q1/08-r1-candidate-07-response.json) 的 prepare 框架称验证误差“先降后升、最低点不在 lambda 最大处”；[第二轮](live-active-q1/12-r2-candidate-07-response.json) 重复这一形状。baseline [第二轮 index 7](live-active-q1/16-r2-baseline-07-response.json) 也将“验证误差先降后升”写进关系。原文只支持验证误差不保证单调下降及过强惩罚可能欠拟合，不能推出所有数据都呈 U 形或最优值必在内部。candidate 第二轮的 focus 虽称“示意曲线”，框架仍将预期观察写成确定关系，未明确曲线是一组假设教学数据及其适用范围。该问题目前存在于待执行 author 输入，尚不能称为已生成或已交付页面错误。
4. **基线展示计划的一处材料身份不匹配。** baseline [第二轮 index 7](live-active-q1/16-r2-baseline-07-response.json) 新增 `lambda-tradeoff` block，写 `source_id=regularization` 却使用 `source_revision=c7835a65…`。当前输入中该版本属于 `overfitting`，`regularization` 的版本是 `cb95bd25…`，见[该次实际请求](live-active-q1/16-r2-baseline-07-request.json)。这是工具参数中已出现的来源身份不匹配；因为 prepare 没有执行，不能断言其通过验证或进入交付。它与八份文字输出中有效的 `source_ref` 是两项不同的事实。

本组支持“Q1 candidate 在四次明确文字请求中直接生成完整候选讲稿，核心算术正确，已有来源 ID 有效”。它同时保留未经审核的科学边界和展示计划问题。固定回放没有调用交付审核，不能借此判定 Q1 审核强化已经纠正这些草稿，也不能把原生工具响应的正常结束算成解释完成。该轮可能与本地编译重叠，延迟不作为独立因果证据。

## 真实 Server 交付

### 第一问：过拟合与 A/B 选择

[实际 answer-01](live-delivery-candidate-q1/answer-01.md) 已完成交付。[turn-01](live-delivery-candidate-q1/turn-01.json) 中 `delivered=true`、`runtime_incomplete=false`，保存状态为 `completed`，累计 8 次 HTTP 请求，首个公开正文时间为 75.300 s。忽略排版空白后，文件正文与 `message_delivered.payload.answer`、`reply.answer`、`saved_turn.outcome.answer` 及 [events-01](live-delivery-candidate-q1/events-01.json) 最后一条公开 `answer.patch` 的 markdown 一致。交付动作是 `explain`，没有 presentation 或 assessment；正文为直接解释，没有出题或声称用户掌握。

核心内容正确：训练/验证数据的用途区分清楚，A/B 仍为 `0.01/0.30` 与 `0.08/0.12`，差距 `0.29/0.04` 和 `0.12` 为 `0.30` 四成的计算正确。选择依据落在较低的验证误差，并保留相同划分、相同度量和这份验证数据的范围。正文将 A 的差距称为“典型形态”“过拟合的迹象”，没有再次由四个数字直接断言 A 已经拟合了哪些噪声，也未提出“训练与验证误差都高就必然欠拟合”。“让出一点训练贴合度换取更低验证误差”的表达是在比较两个候选模型的表现，本次不将其解读为声称已验证某项训练操作的因果效果。

本问实际调用 `source.present`，交付、保存和最终 patch 均保留 `source_ref_070022b13c140f81`，绑定 `workspace-cache-live-overfitting` 的 LID 1.1、UTF-16 范围 8–212。按现有可选引用政策，这确认本次有可点击来源，不将其与之前没有呈现来源的合法答案混为合同通过/失败之分。

[request-006](live-delivery-candidate-q1/request-006.json) 的真实 system prompt 为 `teaching-delivery.v24`，包含“修订改变关键主张真假、因果方向或成立条件时列入 findings 并 revise，不能降为可选 improvements”的强化段落。[review response-006](live-delivery-candidate-q1/response-006.json) 返回 `accept`、空 findings 和 improvements。该问独立复核未发现必须阻止交付的数字或核心结论错误；单次合格输出不证明新的审核规则已覆盖所有过度断言。

### 第二问：L2 目标、梯度与泛化边界

[实际 answer-02](live-delivery-candidate-q1/answer-02.md) 已完成交付，[turn-02](live-delivery-candidate-q1/turn-02.json) 的 `delivered=true`、`runtime_incomplete=false`，保存状态为 `completed`。首个公开正文时间为 253.245 s；[complete.json](live-delivery-candidate-q1/complete.json) 记录整组 20 次 HTTP、2 次成功交付、无预算拒绝。文件正文、`message_delivered`、`reply.answer`、持久化 outcome 和 [events-02](live-delivery-candidate-q1/events-02.json) 最后一条公开 patch 一致。实际交付是 `explain-l2-objective-gradient-v3`，没有 presentation、assessment 或出题，来源为合法的 `source_ref_a78478a608240e0c`，对应 LID 1.1、UTF-16 范围 213–468。

目标式 `J(w)=(1/n)Σ[f_w(x_i)−y_i]²+0.01Σw_j²`、惩罚梯度 `0.02w_j`、含数据项的完整梯度和一步更新 `(1−0.02η)w_j−η∂L_train/∂w_j` 均正确。正文解释 0.01 是平方和惩罚的权重，不是从训练误差减去的值，并说明偏置是否纳入惩罚由模型约定决定。最终稿将收缩限制在通常的小正学习率情形，使用 `|w_j|` 说明回拉强度、以朝零说明方向；没有把总梯度误写成数据项与惩罚项相减。正文保留“训练目标不同于另一份数据上的验证误差”，以及用验证数据选择 λ、λ=0.01 不自带改善保证的边界。大 λ 一段的讨论紧接“惩罚压过数据项、模型已不能拟合稳定规律”的条件，本次不将这一条件下的欠拟合解释扩大读成所有模型必然欠拟合。

实际修订过程如下，三次均经过真实审核：

| 候选与审核 | 动作标识 | 审核及后续变化 |
|---|---|---|
| [response-013](live-delivery-candidate-q1/response-013.json) → [review-014](live-delivery-candidate-q1/response-014.json) | `explain-l2-objective-gradient` | `accept` 并建议修复“经驮”错字及“不把权重推向某个目标值”的方向解释 |
| [response-015](live-delivery-candidate-q1/response-015.json) → [review-016](live-delivery-candidate-q1/response-016.json) | `explain-l2-objective-gradient-v2` | 已修为朝零回拉；`accept` 并建议用绝对值说明负权重强度、将“λ 太小过拟合照旧”改成风险表述 |
| [response-017](live-delivery-candidate-q1/response-017.json) → [review-018](live-delivery-candidate-q1/response-018.json) | `explain-l2-objective-gradient-v3` | 上述实质改动进入稿件；`accept`，仅剩导语“三步”与四节标题的可选表达建议 |

每次 select 返回的 `delivery_move_refs` 都更新为本次新标识。最终 reply 和唯一一次 `message_delivered` 只指向 v3；去掉合法来源标记后，交付正文与 response-017 完全一致，旧稿没有作为最终答案留存。这是新标识修订和待交付引用替换在真实轨迹中的证据；本次没有提交重复标识，因此不能把它写成重复标识提前拒绝分支也已由此实测覆盖。[response-019](live-delivery-candidate-q1/response-019.json) 最后的普通 content 又调整了导语，但没有再次 select；它不是另一份实际交付，不用于替换这里的质量判断。

最初“它不把权重推向某个目标值”的表述已改成“朝向零、强度与 |w_j| 成正比”，修复了与公式方向的冲突；“λ 太小……过拟合照旧”已改为“正则化作用很弱，过拟合的风险仍在”，没有继续将弱惩罚直接判成必然过拟合。这些是有内容意义、而且确实进入最终交付的改动，不是只发生在未公开回包中的润色。

仍有一个明确的条件缺口：第三节说“最优解偏向权重较小、把拟合任务分散到更多参数上的配置”。现有目标和梯度支持对现有权重施加平方和代价、偏好较小范数，不能一般性推出使用更多参数或将拟合分散出去；这种分散解释需要说明参数能替代、共同解释同一关系等模型条件。正文没有给出这些条件，原文也没有这项结论。将这半句删去，或限定到明确的参数关系示例，才与当前证据范围相符。这里不把“权重代价”“回拉”等已有公式支撑的教学比喻另判为问题。

### 审核规则与最终支持边界

真实运行使用 v24，但正确性与表达建议的分流仍不可靠。[review-016](live-delivery-candidate-q1/response-016.json) 已明说草稿把“小 λ 的过拟合风险写成了结果”，却继续返回空 findings、`accept`，只在 improvements 中建议改为风险。按此次已启用的 v24，改变关键结论成立条件的改动应进入 findings 并 revise。此次模型自愿采用了建议，所以最终稿消除了这项断言；这不等于审核已将同类错误设为可靠的交付阻塞。参数分散结论也通过了三次审核并保留在最终稿。

最终可以确认：候选固定回放的四次文字请求直接生成完整讲稿；真实两问完成纯文字交付，核心数值和公式正确，有效引用进入答案；追问的两个实质修订通过新标识进入保存和公开 patch，没有复现旧标识导致的修订丢失。尚不能确认所有科学边界都得到正确审核或讲解效果完全无损。固定回放中的展示计划没有执行，真实补测仍只有单篇材料两问；它不证明长期跨材料学习效果、页面交付质量或整个产品的延迟收益。
