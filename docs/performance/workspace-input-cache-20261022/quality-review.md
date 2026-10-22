# 工作区输入缓存实验：真实输出质量审查

审查日期：2026-10-22。审阅录制请求、原始 provider 回包、fixture 原文，以及后续真实 Server 运行的交付、持久化和 `answer.patch` 记录。审阅者没有追加供应商请求。以下解释、讲稿与问题均来自真实新输出，固定历史中的人工答案不计作模型输出。

自动工具模式 v2 的固定回放恢复了用户明确要求的文字讲解：两次正则化追问均生成 `tutor.step select` 的完整候选讲稿，算术、来源和优化目标的边界保持正确。后续真实 Server 补测的六份答案也均完成纯文字交付，核心数字和公式正确；但三组正文中仍有不同的概念边界问题，暖机还暴露了已接纳讲稿的修订没有落地。最终候选两问没有发布可点击来源，这属于现有可选来源呈现政策允许的行为差异。因此不能将“讲解效果不受影响”标记为全通过，也不能将无来源标记本身判为合同失败。强制禁用工具的附加实验出现大量协议文本，不能作为完整文字讲解的成功证据。

## 样本和判断范围

材料是两篇已读取的教学 fixture：《过拟合与泛化》《正则化与模型选择》。原文、publication 身份和有效来源引用见 [manifest](requests/candidate-v2/manifest.json)。两篇来源引用分别为 `source_ref_c52685f9e6cb9140` 和 `source_ref_679b2138443382f8`。

| 实验 | baseline 新输出 | candidate 新输出 | 可以支持的结论 |
|---|---|---|---|
| `live-ordinary`，普通四问，各两轮 | 8 条普通正文 | 8 条普通正文 | 两版有相同的核心答案能力，也有下述现存内容错误；普通请求完全不变 |
| `live-active`，v1 自动工具 | 4 份文字候选讲稿；其余为工具流程 | 2 份文字候选讲稿；index 6 两次改设计 | v1 权限说明造成了本轮必须纠正的行为变化 |
| `live-active-v2`，最终自动工具 | 4 份文字候选讲稿；4 次展示流程首跳 | 4 份文字候选讲稿；4 次展示流程首跳 | 在当前 text 请求上，两版都选文字讲解；候选讲稿内容可核对 |
| `live-active-text-v2`，`tool_choice=none` | 4 条正常正文、4 条 DSML 协议文本 | 1 条正常正文、7 条 DSML 协议文本 | 禁用工具改变了行为，不能把流正常结束当成讲解完成 |

`select` 的 `move.delivery_text` 是待 Runtime 审核和提交的候选讲稿。`agent.progress`、`presentation.author prepare/read_method` 是过程动作或计划。上述固定回放实验没有执行这些动作，因此它们不表示用户已经看到了讲解、页面已经交付、学习者已经掌握。后续实际执行审核和交付的补测单列如下。

## v1 发现及 v2 修正

index 6 的当前用户要求是“先用文字讲清楚”正则化公式与算例。v1 candidate 在两次请求中均选择 `tutor.step operation=design`，而同批 baseline 两次均选择 `select` 并提供文字讲稿：见 [v1 candidate 第一轮](live-active/07-r1-candidate-06-response.json)、[第二轮](live-active/11-r2-candidate-06-response.json)。这会让明确的文字讲解多走一次不必要的设计流程，不能以缓存收益抵消。

v2 仅将 text 状态下的 presentation 权限说明限定到该工具，不再暗示普通文字讲解也必须修改设计。核对 `requests/candidate-v2` 与 v1：index 4、6 只改变 `presentation_guidance_state.v1`；index 5、7 没有消息变化；所有工具定义、原文、引用、用户问题和非 system 消息均不变。普通 00–03 的 Native/ReAct 共 8 个请求文件与 baseline 字节完全相同，因此无需重新付费运行普通控制。

v2 的 index 6 两次 candidate 输出均恢复 `select`，没有修改设计或调用 presentation author：见 [第一轮完整讲稿](live-active-v2/07-r1-candidate-06-response.json)、[第二轮完整讲稿](live-active-v2/11-r2-candidate-06-response.json)。

## 最终自动模式的内容核对

index 4、6 在两版各产生四份完整候选讲稿，共八份。四份 candidate 讲稿尊重当前直接讲解的要求，没有新增测验、让用户先作答、判断用户已掌握或声称页面已交付。

- index 4 正确区分训练集拟合参数、验证集选择模型/超参数、测试集最终评估；能解释拟合噪声时训练误差下降而未见样本误差上升；保留“验证结果不保证所有未来数据”的边界。见 [candidate 第一轮](live-active-v2/05-r1-candidate-04-response.json)、[第二轮](live-active-v2/09-r2-candidate-04-response.json)。
- index 6 正确计算 `lambda=0` 时两个目标值为 `0.05`、`0.09`，`lambda=0.01` 时为 `0.15`、`0.10`；第一轮额外算出的惩罚差 `0.09`、训练损失差 `0.04`、净目标差 `0.05` 也正确。
- 正则化讲稿区分训练目标的偏好变化与验证误差改善，没有把“乙的惩罚后目标更小”说成“乙一定泛化更好”。保留验证/交叉验证选择 lambda、测试集不反复调参、过强惩罚可能欠拟合的条件。
- 所有新正文及候选讲稿里的来源 ID 均属于本轮实际绑定的两份材料，没有未知来源 ID。candidate index 4 第一轮的两处 `[[source_ref_...]]` 属于现有来源编译器支持的已绑定后缀格式，不能误报为伪造引用；对应覆盖见 Runtime 的 `source_presentation_bound_suffix_formats_compile_without_repair` 测试。

index 5、7 的两版各四个回包均只走到 `agent.progress` 或 `presentation.author prepare`。它们没有本轮完整的数字讲解或欠拟合说明，因此不计作完成讲解，也不计作页面质量通过。比如 [candidate index 5 第一轮](live-active-v2/06-r1-candidate-05-response.json) 只有“先讲清再做页面”的进度消息；[index 7 第二轮](live-active-v2/12-r2-candidate-07-response.json) 只有进度和 prepare。是否最终生成正确的图、是否保留交互状态，需实际执行后续流程才能判断。

## 普通组已观察到的内容问题

普通组两版请求完全相同，v2 也没有改变这些请求。以下问题属于这组模型输出已经存在的错误或过度推断，不能归因于本轮工具缓存改动，也不能用“普通组不受影响”将其写成质量全通过。

1. **散点位置颠倒。** [candidate 第一轮 index 1](live-ordinary/06-r1-candidate-01-response.json) 明确给定横轴训练误差、纵轴验证误差，却说甲在“右下”、乙在“左上”。正确的相对位置是甲 `(0.01,0.30)` 比乙 `(0.08,0.12)` 更靠左、更靠上。
2. **把可能的曲线形状当成确定规律。** [baseline 第一轮 index 3](live-ordinary/04-r1-baseline-03-response.json)、[candidate 第一轮 index 3](live-ordinary/08-r1-candidate-03-response.json) 说验证误差呈 U 形、最优点在中间而不在两端。原文只说明验证误差不保证随 lambda 单调下降，以及惩罚过强可能欠拟合，不能据此推出任何数据上的最优点都在内部。标成推论也不能消除这个推断缺口。
3. **混淆横轴方向。** 同一份 [candidate 第一轮 index 3](live-ordinary/08-r1-candidate-03-response.json) 先给横轴 lambda，随后把“训练误差单调下降而验证误差回升”的区段用于解释该图，混入了训练推进或复杂度增加时的典型方向。比较页面必须明确是在改变 lambda、训练轮数还是模型复杂度。
4. **对数轴包含零。** [baseline 第二轮 index 3](live-ordinary/16-r2-baseline-03-response.json) 要求 lambda 对数轴“从 0 扫到明显过强的区间”。普通对数轴无法包含 0；lambda=0 需独立展示，或选择明确支持零的其他轴定义。
5. **从给定误差反推出训练过程。** [baseline 第一轮 index 1](live-ordinary/02-r1-baseline-01-response.json) 和 [candidate 第一轮 index 1](live-ordinary/06-r1-candidate-01-response.json) 将训练误差 0.01 直接解释为几乎记住全部样本和噪声。该数字及验证落差支持讨论过拟合风险，不能单独证明模型实际记忆了哪些噪声。

四次正则化数字追问（index 2）均给出正确的 `0.15`、`0.10`，并说明目标函数排序本身不证明验证效果。全部普通正文引用的 ID 有效，也没有把学习者追问当成已掌握；这些成立之处不消除上面的图形与推断问题。

## 禁用工具附加实验的异常

`live-active-text-v2` 仅将请求的 `tool_choice` 改为 `none`，输出上限和材料保持一致。但供应商的计量和行为均发生变化：许多响应将内部 `<｜｜DSML｜｜ calls>` 工具协议直接写进 `content`，而不是返回面向用户的完整解释。

baseline 的 index 4、6 共四条是正常正文；index 5、7 共四条是协议包裹的进度消息。candidate 仅 [第一轮 index 7](live-active-text-v2/08-r1-candidate-07-response.json) 是普通正文，其余七条均含 DSML：

- [第一轮 index 4](live-active-text-v2/05-r1-candidate-04-response.json)、[第一轮 index 6](live-active-text-v2/07-r1-candidate-06-response.json) 只有内部选择结构和解释计划，没有完整讲稿。
- [第二轮 index 4](live-active-text-v2/09-r2-candidate-04-response.json)、[第二轮 index 6](live-active-text-v2/11-r2-candidate-06-response.json) 在协议结构中嵌有内容正确的讲稿，仍不能当成已经正常生成的用户回答或有效工具执行。
- 其他 candidate 协议文本为 author 准备、方法读取或进度消息。

candidate 唯一的普通正文正确说明正则化和欠拟合的基本关系，但又把验证误差“先降后升”、存在“中间强度”说成给定事实，仍有上述曲线边界问题。

该组 16 条响应都没有被输出上限截断，全部 `complete_text_sample=true`。这个字段只说明流正常结束、有 content 且没有原生 tool_calls，**不表示讲解在语义上完成，也不表示正文不是工具协议**。统计不能把这 16 条全算成成功讲解。

该组记录的首 content 延迟包含协议开头的首字符。baseline 全八条中位数为 1141.598 ms，candidate 为 1376.104 ms；text 设计 index 4、6 的四条中位数分别为 1008.580 ms、1390.796 ms，但 candidate 这四条全是协议文本。这些值保留为原始观测，不能称为可读讲解的首字速度，更不能作为自动工具模式的缓存收益或用户看到答案的延迟结论。自动模式本身在这组请求里只返回工具调用，没有可比较的正文首字样本。

## 真实 Server 交付补测：修复前记录

补测使用单篇材料《过拟合与正则化》，`book_id=workspace-cache-live-overfitting`、LID `1.1`。三组实际 `book.text` 返回的正文逐字相同，共 468 个字符、468 个 UTF-16 code units、1182 个 UTF-8 字节；见 [暖机读取](live-delivery-candidate/request-005.json)、[基线读取](live-delivery-baseline/request-004.json)、[正式候选读取](live-delivery-candidate-idle/request-004.json)。正文没有因为缓存优化被裁短。第一问要求纯文字解释过拟合与 A/B 选择；第二问在真实上一问历史上继续解释 λ=0.01 的 L2 目标、惩罚梯度和验证误差的边界，两问都明确不要页面、不要出题。

| 运行 | 实际交付 | 首问首个公开正文 | 追问首个公开正文 | HTTP 总数 | 交付来源 |
|---|---|---:|---:|---:|---|
| `live-delivery-candidate`，暖机候选 | 2/2，均非 incomplete | 104.566 s | 222.132 s | 19 | 两问均保留有效来源 |
| `live-delivery-baseline`，正式基线 | 2/2，均非 incomplete | 153.875 s | 194.887 s | 21 | 两问均保留有效来源 |
| `live-delivery-candidate-idle`，正式候选 | 2/2，均非 incomplete | 71.993 s | 247.534 s | 17 | 两问均没有已发布来源 |

暖机候选与编译重叠，只用于完整链路和质量验证。正式基线、候选顺序运行；候选首问更快、追问更慢，不能将其写成首字延迟全面改善。费用、未缓存 tokens 和命中率见主报告，此处保留与交付质量有关的运行身份。

六份 `answer-01/02.md` 忽略排版空白后均与 `message_delivered.payload.answer` 相同；交付正文又与 `reply.answer`、`saved_turn.outcome.answer` 及最后一条 `answer.patch` 的 markdown 相同。六份保存状态均为 `completed`，交付动作均为 `explain`，`presentation` 和 `assessment_contract_ref` 均为空。实际正文没有出题、要求先作答、宣称学习者已经掌握或声称交付了页面。由此确认这些是用户实际收到并保存的讲解，不是仅停留在工具参数中的计划。各组交付证据见 [暖机首问](live-delivery-candidate/turn-01.json)、[追问](live-delivery-candidate/turn-02.json)，[基线首问](live-delivery-baseline/turn-01.json)、[追问](live-delivery-baseline/turn-02.json)，[正式候选首问](live-delivery-candidate-idle/turn-01.json)、[追问](live-delivery-candidate-idle/turn-02.json)。

### 正确内容及审核发挥作用的部分

三份首问均正确给出 A 的训练/验证误差 `0.01/0.30`、B 的 `0.08/0.12`，误差差距分别为 `0.29/0.04`，并在同一划分和度量下依据较低的验证误差选择 B。三份追问均正确写出 `J(w)=(1/n)Σ[f_w(x_i)-y_i]²+0.01Σw_j²`，明确 0.01 是惩罚权重而非从训练误差里减去的数，惩罚梯度为 `2λw_j=0.02w_j`，并保留偏置是否受惩罚取决于模型约定。

暖机追问的梯度示例 `w_j=2→0.04`、`0→0`、`−5→−0.10` 正确。基线追问的 `1→0.02`、`10→0.2` 正确。正式候选追问明确将新增目标值示例标为自设数字：`0.05+0.01×2=0.07`、`0.06+0.01×0.5=0.065`，计算和排序均正确；梯度下降更新式 `(1−0.02η)w_j−η∂L_train/∂w_j` 也正确，并补上小学习率条件。其线性最小二乘补充推导在所声明的 `1/n` 损失、所有参数参与惩罚的约定下，`(XᵀX+0.01nI)w=Xᵀy` 的系数正确。

审核实际阻止了一处错误交付。正式候选最初写“验证误差 = 偏差 + 方差 + 数据本身的噪声”，[第 13 次调用](live-delivery-candidate-idle/response-013.json) 判为 `revise`，要求平方损失下对训练集和新样本取期望的 `偏差²+方差+噪声` 分解。随后[第 14 次调用](live-delivery-candidate-idle/response-014.json) 修改讲稿，[第 15 次调用](live-delivery-candidate-idle/response-015.json) 接纳。实际 [answer-02](live-delivery-candidate-idle/answer-02.md) 对应第 14 次候选稿，已包含偏差平方、期望误差与有限验证集估计的区别；第 16 次最终模型 content 不作为另一份已交付答案计数。

### 来源呈现差异与实际交付中保留的问题

1. **正式候选两问没有呈现可点击来源，构成实测体验差异。** [首问](live-delivery-candidate-idle/turn-01.json) 与[追问](live-delivery-candidate-idle/turn-02.json) 的 trace 均有 `book.text`，但没有 `source.present`。两问 `message_delivered`、最终 `answer.patch` 和持久化回答的 `answer_view.sources`、`source_bindings` 均为空。这不是 Markdown 导出省略了引用。暖机和基线分别绑定 `source_ref_070022b13c140f81`（LID 1.1、UTF-16 范围 8–212）及 `source_ref_a78478a608240e0c`（213–468）。正式候选正文使用了原文，却没有可点击的出处入口。现有政策明确将 `source.present` 设为可选：教学目标须有本轮已读证据，已使用的引用必须合法，但无标记纯文字回答本身合法。因此记录为可追溯体验差异，不据此认定来源交付合同失败或增加强制引用要求；单次自主运行也不足以将差异归因于缓存改动。
2. **暖机与正式候选首问把过拟合迹象写成已证实的成因。** [暖机首问](live-delivery-candidate/answer-01.md) 说训练优势未转化为验证优势“正对应过度适应偶然细节”，还由 B 的两个误差推断其“正常拟合”；[正式候选首问](live-delivery-candidate-idle/answer-01.md) 由误差差距直接说“训练样本上的偶然细节被拟合进去了”。这些数字支持讨论过拟合风险及在当前验证集上选择 B，不能单独证明模型拟合了哪些噪声或其训练机制。[基线首问](live-delivery-baseline/answer-01.md) 使用“迹象”的限定更合适。
3. **暖机追问含无条件的训练误差变化断言。** 实际 [answer-02](live-delivery-candidate/answer-02.md) 说给 A 加上 λ=0.01 后“训练误差会从 0.01 上升”，并称这“由惩罚机制直接给出”；还把惩罚过强后的欠拟合、验证误差上升写成必然。仅给定 A 的现有误差，不能断定该解是未惩罚训练目标的全局最优，训练误差严格上升也不由加入软惩罚无条件推出。原文明确保留“可能欠拟合”。后来生成的软化稿没有替换这份交付，不能据其抵消错误。
4. **基线追问的梯度文字与公式相矛盾。** [answer-02](live-delivery-baseline/answer-02.md) 第三节写“它与数据项相减”，紧接着正确写出 `∂J/∂w_j=∂L_data/∂w_j+2λw_j`。总梯度是两项相加，二者方向可能相抵；不能用无条件的“相减”解释总梯度。
5. **正式候选追问仍将条件规律绝对化。** 实际 [answer-02](live-delivery-candidate-idle/answer-02.md) 第四节称“两端都是坏值”“λ=0……过拟合原样保留”。λ=0 不必然过拟合，也不能排除它在给定验证集上最优；因此不能从“验证误差不保证单调下降”推出两端必坏。第三节先将权重收缩写成必然减弱偶然细节依赖、导致方差下降，再引用原文的“可能降低方差”，前后条件不一致。第二节又在一般单参数驻点公式后说“λ 越大，平衡点越靠近 0”，未限定模型和参数间耦合；L2 总体偏好较小范数不等于每个坐标都随 λ 单调靠近零。这些超出原文边界的推断仍出现在已接纳稿中。

### 已证实的修订身份与复审顺序问题

暖机追问[第 14 次调用](live-delivery-candidate/response-014.json) 提交 `move_id=explain-l2-regularization`，包含上面无条件的训练误差与欠拟合断言；[第 15 次审核](live-delivery-candidate/response-015.json) 返回 `accept`，同时将两处正确性问题写入可选 `improvements`。模型于是[第 16 次](live-delivery-candidate/response-016.json) 用相同 `move_id` 修改正文，[第 17 次审核](live-delivery-candidate/response-017.json) 又返回 `accept`，之后 Runtime 才拒绝：“同一教学操作不能改写已保存事实”。这次多余修订及复审增加约 79.22 s，且修订没有落地。

去掉有效来源标记后，实际交付严格对应第 14 次旧候选稿，与第 16 次软化稿不同。[第 18 次](live-delivery-candidate/response-018.json) 最终模型 content 也不是实际保存/推送的答案；交付、保存与最终 patch 都保留旧稿。证据在[该轮 turn trace 与交付](live-delivery-candidate/turn-02.json)中一致。该轮没有暴露 `presentation.author`，不能归因于本轮 author 权限说明；它是补测确认的既有修订身份检查及复审顺序限制，单列为额外性能和交付一致性发现。

基线追问也保留了额外尝试：第一次已接纳讲稿仍是实际交付，第 18 次又提交修订后触发 `TEACHING_REVIEW_FAILED`（审核输出未抽取到合法 JSON）。[基线 turn-02](live-delivery-baseline/turn-02.json) 中该失败没有被删去；它不影响旧稿已交付的事实，也不能当成修订已经成功。

## Limitations

固定回放部分使用固定历史、固定输入状态，每臂每条件两次调用；新模型输出不会成为下一条请求的历史，工具调用没有执行，候选讲稿没有经过正式交付审核。真实 Server 补测另外覆盖了单篇材料、两问连续对话的自主读取、真实审核、交付、持久化和公开正文 patch，但每个正式版本只有一轮，且实际工具轨迹不同。暖机与编译重叠不用于独立延迟比较。这些样本不覆盖长期跨材料工作区、课堂学习效果或完整页面交付质量，也不足以把所有自主行为差异归因于缓存改动。

v1 的文字讲解绕路已被真实请求发现并在 v2 固定回放的两次重试中修正。补测进一步确认了六次完整文字交付和正确的核心数字、目标及梯度，也观察到正式候选在可选出处呈现上的不同、不同组正文的过度推断和暖机修订未落地。普通组既有事实问题、展示分支未完成流程及禁用工具实验的协议泄漏同样保留。当前证据支持这些具体判断，不能据此声称全部讲解效果无损或质量问题已经解决。
