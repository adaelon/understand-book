# TC6 — 交互事实与理解证据分流

2026-10-11；**TC6 已完成，未提交、未部署。** 沿 ADR-0163 §6 与切片方案 TC6；基线 HEAD `12b765b`，保留其他在途修改。

## 实施步骤

1. [x] 接入近期交互与来源摘要，认知待办仅使用真实用户表达；验证纯操作与助手内容不能独立形成假设。
2. [x] 画像审核、历史回填与待审核上下文只使用真实用户补充，保留连续回合身份；验证选择不会成为长期偏好依据。
3. [x] 固定轨迹验证真实纠正替代旧假设并改变真实模型输出，保存输入、冻结上下文、证据与投影。
4. [x] 回归原题帮助、普通观察及来源修订，更新架构、代码链路与 checkpoint。

## 验证目的

定向测试检测选择事实遗漏、纯点击进入认知待办、助手内容独立支持诊断及画像误引按钮文案；失败时修正对应查询、证据门禁或原话提取。既有回归检测原题帮助条件、普通观察、假设修订及来源边界被破坏；失败时修复受影响路径。真实模型轨迹检查纠正后是否采用新假设并改变讲法，保留失败原输出，不以数据库新增记录替代内容验收。

## 实施结果

- `teaching_recent` 纳入 ContinuationDisplayed／ContinuationChosen；`trace_summary` 保留原选项、回答引用与有界摘录，标明助手来源；精确内容沿原 trace 回读。
- active 与 observing 共用认知待办筛选，仅收当前来源版本的真实用户表达；纯 choice／continue／clarify 不成为待解释事实。冻结 `current_action` 继续保留原回答、来源、公开请求和真实补充。
- `TeachingEvent::learner_expression` 统一处理认知依据与假设引文。助手交付、展示或阅读接触不能单独产生知识诊断；类型化操作有真实补充时仍可形成假设。既有 `supersedes`、来源版本、帮助条件和表现聚合继续生效。
- 画像审核和历史回填的 `user` 只含真实原话，`input_action` 单列动作；无文字操作保持连续序号，不能通过非空引文校验生成候选。待审核上下文省略纯动作、保留真实补充。公开聊天历史继续显示原操作。
- Tutor 指导 revision=`tc6.v1`，明确选择、继续和纠偏各自语义；纯点击不诊断具体知识缺陷，重复选择不定型长期偏好。

## 确定性验证

| 验证 | 结果 | 证据 |
|---|---|---|
| 实现前复现：选择未进入近期事实、助手交付单独支持假设 | 2 项按预期失败 | [red.log](tutor-continuation-tc6/red.log) |
| 首组 TC6 来源与防污染 | 4 通过 | [targeted.log](tutor-continuation-tc6/targeted.log) |
| TC6／画像历史回填／原题帮助／普通观察／T17 修订／来源版本／按需指导 | 20 通过、2 默认忽略 | [server.log](tutor-continuation-tc6/server.log) |
| Memory 全量 | 149 通过 | [memory.log](tutor-continuation-tc6/memory.log) |
| Runtime Tutor 与画像审核 | 34 通过 | [runtime.log](tutor-continuation-tc6/runtime.log) |
| TC6 真实模型纠正轨迹，显式执行忽略入口 | 1 通过 | [live.log](tutor-continuation-tc6/live.log) |

分组有重叠。Server 默认忽略的是 TC2A 实模对照及本次 TC6 实模入口，后者单独执行。新增确定性测试共 8 项（Server 7、Runtime 1）。连续三次相同形式的选择、重复读取与未选择不增加证据、能力投影或画像；本地／多人冻结快照均保留选择事实。整次回答继续／纠偏没有真实文字时拒绝诊断，有真实补充则保存精确原话。旧假设被纠正后不可采用，换来源版本不沿用旧假设。

原题帮助回归同时覆盖 source-only 与旧对象式活动：定位帮助入口不算已获得帮助，实际显示后答对保留 assistance_refs、assisted_support，independent_support 不增加。关闭 Tutor 的普通理解观察继续可用，不隐式开启会话或 select。

环境：Rust `+1.95.0`，`CARGO_TARGET_DIR=D:/codex-build/understand-book-no-progress-195`；dev/test debug=0、incremental=0，TEMP/TMP=`D:/codex-build/tmp`，`-j 2`。

```text
cargo +1.95.0 test -p server --lib -j 2 -- continuation_tc6 tc6_historical tutor_ordinary tutor_t17 tutor_target_reports continuation_red_source_question_help continuation_red_object_question_help continuation_tc2_current_question continuation_tc2a --test-threads=1
cargo +1.95.0 test -p memory --lib -j 2 -- --test-threads=1
cargo +1.95.0 test -p runtime --lib -j 2 -- tutor memory_review --test-threads=1
```

实模使用上述最新 Server 测试二进制，从 `crates/server` 目录以完整测试名 `tests::tutor_continuation_tests::continuation_tc6_live_correction_trajectory --ignored --exact --nocapture` 执行；`TC6_LIVE_OUTPUT` 指向新的绝对输出目录，`FLUID_LLM_MODEL=deepseek-v4-flash`，沿原 `.env` 的 Provider 配置，观测上传关闭。

## 固定轨迹与实际输出

轨迹起点采用确定性的原话／假设夹具，并重放 TC5 已保存的真实回答；本次新的纠正响应由真实 `deepseek-v4-flash` 生成。旧假设是“需要拆开等式变形”；读者通过 `clarify_answer` 引用原回答片段，并补充“乘除互逆我已经明白了……停顿还要算进分母？”。

1. [输入](tutor-continuation-tc6/live/input.json)与[冻结 LearnerContext](tutor-continuation-tc6/live/learner-context.json)同时保留真实补充、助手摘录、原回答身份和旧假设。
2. 模型生成新 hypothesis，引用当前用户事实，并用 `supersedes=interpretation:tc6-prior` 替换旧解释。新 teaching_implication 要求停止代数回代，转向“总时长和移动时长”。
3. 最终 Move 的 `interpretation_refs` 采用新假设；[公开回答](tutor-continuation-tc6/live/answer.md)实际对比含停顿的全程平均速度和只算移动时间的量，用 120 公里／2 小时与 1.5 小时说明差异。回答未继续展开乘除互逆。
4. [完整结果](tutor-continuation-tc6/live/result.json)保存事实、两条 hypothesis、引用关系及前后对象投影。旧假设不再是 current；本轨迹无正式对象，前后 rows 均为空，没有新增 Performance 或能力支持。

结果：completed、incomplete=false；6 次采样，Provider usage 累计 **135,049 token**。这表示累计使用量；[轨迹摘要](tutor-continuation-tc6/trajectory.json)保留计算结果与一次校验错误。原请求和响应均在 live 目录，未重跑挑选更好的答案。

## 失败记录与已知限制

- 实模首次 select 把有真实补充的请求写成 current_action；原意图校验拒绝后，模型改用 current_request 并完成 explain。该次失败与修正保留在 response-03／04 和 result.json。
- 本次为一条脚本化纠正轨迹，真实读者人数仍为 0；输出仍偏长。它证明本场景中假设修订与讲法改变实际发生，不关闭 TC2 的既有公开内容质量问题或 TC7 的读者验收。
- 本片没有改动前端；浏览器交互证据沿 TC5。旧二进制读取与降级演练、发布准备继续归 TC8；未上线。
