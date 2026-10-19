# SESSION_CHECKPOINT — 2026-10-17 连续卡住教学

## 本轮工作
用户要求先上线已有主动邀请版本，再用隔离 gpt-6.1-sol 多轮验证连续卡住/零碎理解，处理首轮信息量，发现问题继续改系统实现。部署、修复后更新均已在当前任务授权范围内，无需再次确认。

## 当前状态
- 已有实现与验证：docs/performance/teaching-initiative-20261017/implementation.md。
- 初次上线成功：2026-10-17 08:36 HKT，teaching-initiative-20261017，主服务、worker、Nginx同步切换；deployment-initial/README.md和deployment.json有完整证据。随后已更新为本轮最终修复版本。
- 初次发布合入教学闭环依赖（87归档文件/64实际差异），Linux五程序和Web构建通过；新版静态资源、本地/公网账号书库与正文及演示能力已验收。
- 候选源码已固定，可独立后续实现；不要改 tmp/teaching-stuck-release-20261017。
- live-01已完整结束：6轮/28模型请求，全部support，首轮992字符，之后用户逐个卡在相邻关系。conversation.md/原始请求/交付/日志保留，未改写。两位均隔离gpt-6.1-sol。
- 已修改memory teaching_design.validate_focus、server局部current_target保持及return_context、runtime教学局部目标指导及演示focus约束。省略target路径也已修复并回归：同设计同关系继承实际局部目标，换关系/新版设计回到相应设计目标，显式目标优先。
- live-02已正常结束：6轮36请求/全HTTP200。教师stuck_fixed_teacher_sol；学习者stuck_fixed_learner_sol均fresh6.1sol。字符523/412/461/380/465/180，选择3support+3conclude。学习者最后自行把同一乙从候选→主题→最终的路线接回原困惑，并承认仍需盯一个点；教师限本例收束。没有邀请或页面、没有独立迁移证据。conversation/原始请求响应/交付/run.log和complete.json已保存。
- 新版本代码回归完成：runtime622pass8ignored，server design12pass1ignored，tutor115pass19ignored（明确跳过历史TB2缺design旧录制），target合同说明微调后的tutor_examples1pass。日志已存regression/。
- 最终修复teaching-scaffold-20261017已于09:16:43 HKT上线成功。server PID110819、WorkingDirectory、worker配置、Nginx均指向新release，schema14不变，本机/公网reader11/admin12授权书库、正文、演示能力及两入口9项目资源通过。停服至本机验收14.46秒，Linux5程序构建10m26s。首次preflight发现claimed运行后等待其09:15:37自然结束再切换，未取消或改其状态。deployment-final保存完整回执，validation代码passed/live completed/semantic reviewed_with_limitations。
- stuck_content_review_sol两组独立审阅均完成。live02降阶/针对反馈/最后局部收束成立，无实质原文错误；仍有首轮accepted反事实跨度偏大，随后及时修补。无新情境迁移证据是验证边界，不是已观察教学错误。各自content-review.md保存。不得将首轮信息量写为彻底解决。
- 最终远端可用921,817,088 bytes，完整数据备份351,421,712 bytes。只清了确认非运行旧编译缓存，程序、源码、数据和证据保留。两次冻结目录tmp/teaching-stuck-release-20261017和tmp/teaching-stuck-final-release-20261017不可改写。

## 交付与边界
实施与对话结果见docs/performance/teaching-stuck-20261017/implementation.md，最终部署见deployment-final/README.md。本轮已完成代码、回归、两组实际对话、独立语义审阅和最终上线；没有改写基线，也未提交git。
首轮已减量但仍提前引入accepted反事实；这组未出现主动邀请或交互演示，不能宣称验证了独立迁移或页面教学的语义效果。既有四轮主动邀请正例仍属于上一阶段证据。后续按实际任务继续，不加入每轮必问、固定字数或教学阶段。

## 可复用入口
- 当前服务器测试exe：D:/codex-build/understand-book-tb-195/debug/deps/server-aa6b9ef8be31fed3.exe；71e34eb355b24286为改前基线。
- 设置TEACHING_LIVE_OUTPUT为新绝对目录，清空TEACHING_LIVE_STATE/TEACHING_LIVE_START_TURN后运行ignored精确测试。
- input-NN.json是 {message:原话}；request-NNNN.json由实际服务器产生，响应格式{text,tool_calls}；structured_completion需返回相应结构原值。
- 教师通过Python JSON写response-NNNN.pending，再调用 docs/performance/teaching-design-loop-20261016/publish-response.py 发布，勿改已消费响应。
- 下一轮input={stop:true}结束。单文件等候上限20分钟。Windows不要在该exe运行时链接server测试，否则文件锁。

## 开始时查明的问题（本轮已按上述链路修复）
- 旧live-01将M⊆A⊆C、T从C选、macro种子与最终A∪T塞入唯一selection-branches积木；动作要求完整展开，造成首轮密度。
- server teaching.rs:1762会在无显式teaching_ref时用design.target覆盖current_target；教师局部缩小的move.target无法持续成为下轮工作目标。
- design.validate_move只验block存在；动作/普通邀请与演示write/patch尚未限定在focus内。runtime/orchestrator.rs约6854的演示绑定同样只验存在。
- 已有return_to/open_question_refs保存真实引用，但未组成直接可用的返回问题视图。
- 可沿现有字段修：区分总方向与局部目标、实际交付保持局部目标、focus实际约束选择/呈现、明确返回点。自由文本block仍可能过大，不能把字段合法当教学效果，勿加固定字数/概念数/轮数阈值。
