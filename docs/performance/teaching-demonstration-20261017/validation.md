# 验证记录

## 执行路径

- 初次 runtime：630 passed、3 failed、8 ignored。两个失败是公开设计示例遗漏新 representation，已补；另一个是新增页面回归夹具模拟已保存设计时缺 source_id/source_revision，真实入口在第一步拒绝了它，已按真实保存形状补齐。最终完整串行运行：633 passed、0 failed、8 ignored，见 runtime-tests-final.log。
- server teaching_design：14 passed、2 opt-in ignored。包括无邀请支持页的帮助冻结、原场景与新版场景分离、焦点切换与未来帮助不回填。
- server presentation_follow_up：2 passed，验证精确旧版本与状态冻结、拒绝错配回执。
- server tutor：初次 113 passed、3 failed、19 opt-in ignored。两个公开示例失败随上述示例修复；最终实际 dispatch 的 5 个 tutor_to3 测试全部通过。余下 tutor_tb2_sol_outputs_replay_through_real_delivery 使用教学设计合同之前的固定模型录制，上轮已有同一不适用结果；不修改旧录制，不把它算作本轮通过。见 server-tutor-tests.log、server-examples-final.log。
- Web真实消息入口：修前4项失败，修后20 passed。验证 teaching-help/region-located 能到达原有处理分支；真实现场保存和帮助登记完成之后才 ack，操作不伪造用户表达。
- memory两项类型导出通过；初次发现 ts-rs 忽略合并 serde 属性造成错误判别联合，显式 TS tag 修源后重新生成。最终 vue-tsc 通过，Web生产构建通过。见 generated-types-final.log、web-build.log、web-typecheck-final.log。

## 内容与真实操作

live-01完成3轮、24次请求；独立内容审阅认为当前位置关系处理成立，未使用页，不能作为主动演示证据。live-02虽主动制作页面并按实际窄屏问题修订，但响应提取脚本最终从压缩历史拿到null，真实HTTP502，没有实际交付；失败证据保留。live-03新鲜状态/教师/接纳重跑同一原始困惑与尚未看到材料的学习者。

live-02还暴露了实际图片预算省略后的恢复信息不足。既有inspection准入正确拒绝未观察环境；新增信息直接列当前候选还缺的环境。追加行为回归覆盖预算裁图、补narrow后仅剩short、补short后真实交付以及其他候选隔离；最终117项presentation回归通过（runtime-presentation-final.log），server也用最终源码重新编译。未再重复无关测试。

新实验发布脚本对实际失败的response0029复制件验证：空普通响应在原子发布前拒绝，原始证据不改。不会把已保存版本或接纳决定当作用户已看到。

live-03 已完成3轮、28次请求，全部HTTP200。选择文字，用户主动分清接受/主干后推导最终输出，教师conclude；它证明文字路径可保持灵活，不能作为主动页面证据。live-04-recovery使用既有JSONL真实恢复，两轮HTTP200、19次请求。教师主动交付页面，独立学习者真实点击主题按钮，页面现场保存到server并随其原话进入下一轮。教师承接“不是搬过去”，补合并规则并留出一小步判断；用户自行答出章节仍接受时最终仍保留。阶段内容审阅见 content-review-interim.md，当前尚待第3轮反馈。

## 真实页面恢复缺陷

实际Reader bridge重开页面时发现作者restorer把saved.values.page误读为saved.values，丢失两个布尔状态。正式API文档与Reader相同，漏点是制作preview原先把restorer置为空函数。系统修复在真实preview末尾用新页面环境重开、恢复、回读values.page/visible_step；错误进入已有preview_failed，保留实际动作截图和结果。

同一target/profile的runtime+server编译通过（restore-build-tests.log）。新增2项真实浏览器回归通过：错误层级被拒、异步正确恢复通过、漏恢复步骤被拒；真实live-04坏页被新preview拒绝，只有测试副本修正两处路径后通过，原件未改（restore-regression-01）。既有来源标签、操作后结果读取、已保存学习页预览交付3项也通过（restore-existing-preview-tests.log）。没有为已通过的无关路径重复扩测。live-05-continued使用新exe恢复同一会话第3轮，非新鲜首次对话。

## 长对话的跨轮调用编号

live-05第3轮HTTP200、conclude，确认用户正确理由后收束。第4轮实际重开引起的困惑让教师自主检查并修订页面，但在预览后历史压缩报HTTP500，未交付。不是以字段或代理口头报告替代成功。

源代码明确支持跨用户轮复用call ID，生产ReAct parser省略ID时也生成react_1。prepare_compaction却跨多个完成轮去重，ActiveToolResultLedger又用本轮同ID投影旧历史。最小修复移除错误的跨轮去重（保留轮内配对/重复校验），并把当前ledger投影限于当前用户轮。来源仍按既有带消息位置的source_item_id分别保留，没有新ID迁移。

compaction相关41 passed / 5 ignored；tool_result相关34 passed；runtime/server组合构建通过（39.12秒）。新增回归覆盖两轮同ID不同内容的来源与checkpoint重读、旧正文/调用不被当前结果覆盖，以及同轮重复仍拒绝。日志 runtime-compaction-reused-id.log、runtime-tool-result-reused-id.log、runtime-server-compaction-fix-build.log。live-06-final用原JSONL和原场景回执重试失败第4轮，不改历史ID。

## 本轮派生运行状态的容量

live-06 request0005成功压缩46个来源，旧编号错误已消失；但最后预览后以ACTIVE_CONTEXT_EXHAUSTED退出，仍无交付。原request0010累积9份各18000字符的运行状态及过期增量指导。修复在已有tool-body retirement后仍有实际压力时，退役被替代的状态，完整重组当前active指导；低压正常追加路径不变，不提高模型预算。

真实request0010先按原结构重建并断言输入逐消息相同、原token估算相同，再经实际ledger退役：含原图片的输入从114487降至50870 tokens，9快照降至1，20条原始/冻结/checkpoint消息与18个active完整模块保留。见recorded-sampling-pressure-result.json。context_fragment 8 passed / 1默认ignored，压力集成1 passed，录制回放显式运行1 passed；组合runtime/server构建成功（45.34秒）。live-07-sampling使用最新exe、原JSONL及新鲜隔离6.1教师续跑，不依赖旧教师的隐藏记忆。

## 最终实际交互结果

live-07 的新教师准确解释恢复缺失，但没有修旧页；该分支完成一轮 HTTP 200 后结束，未提供给原学习者，不能算作页面修复成功。最终代码的全新 live-08 采用新教师与新学习者，两轮 15 请求全部 HTTP 200：文字已能澄清当前关系，学习者自行推导后教师收束。它证明系统仍允许文字路径，不是主动演示的证据。

live-09 对原作者曾自主启动、但被运行时故障中断的页面修订任务重试；不是全新首次成功。最终三轮 25 请求全部 HTTP 200，真实进程 exit 0、1 passed。原页面 revision 2 已交付；学习者自主点击章节，真实状态经服务器保存、读回，再在新页面环境恢复。恢复后仍为 chapter=true、theme=false，左栏乙保留、右栏乙列入。实际截图、状态与服务器回执均在 live-09-authoring-retry/。

学习者自己说出“两方只要还有一方选它，最终就有乙；两方都不选才是右边没有乙，也不表示左边的乙没了”。教师针对这个理由反馈，没有把操作或显示当作理解。学习者看到真实重开后确认选择仍在，并明确当前困惑消除；最后教师将恢复结论限定为这一次，按已解决的问题结束。最终正文分别为 233、210、144 字符。最终独立 6.1 sol 内容审阅未发现实质教学内容问题，见 content-review-final.md。

主教学路径为 live04 的主动页面与一小步推断、live05 第三轮的具体反馈；live09 补足页面实际修订与保存重开闭环。中断、旧页缺陷和替代分支原始记录全部保留，没有拼成一次无失败的新鲜会话。便于阅读的原文串联见 learning-conversation.md。发布的独立构建与运行验收记录以 deployment/README.md 为准。
