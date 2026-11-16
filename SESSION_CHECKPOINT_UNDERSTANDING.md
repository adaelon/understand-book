# SESSION_CHECKPOINT — 理解变化与学习地图（2026-11-12 已完成）

## 授权与范围
用户授权完成四项：理解变化与教学接续、理解地图及一级入口、短补学返回、提问训练。不含上线和后续自行设计验证/隔时应用。共享工作树基于6c18a75f，原有大量修改不可回退；本轮起点文件备份在 .tmp/understanding-20261112/baseline/。

## 已实现
- Memory UnderstandingObservation.learner_summary 区分学习者小结/teaching_implication；UnderstandingSummary.previous 保留之前原话和帮助条件，旧记录无教师指令泄露。
- course enqueue 冻结同来源版本当前理解；server with_material 按实际来源和相关知识边选12项，补入 known_focuses。knowledge_tree candidates 包含真实关系边。
- UnderstandingMap 为独立一级页面，保留 ReaderWorkspace；主题→问题→证据，首页 UnderstandingEntry、课程小结，手机只展开当前主题。旧 TutorPanel 入口转到地图。
- CourseFeedback.support_branch、question_guide；OpenSupport/ReturnFromSupport 保存原反馈和位置，只有打开的补学计入 HelpDisplayed；原活动 v-show 保留草稿/实验。
- AskQuestion/ResolveQuestion + CourseQuestions：原话持久化、三句提问支撑、接续/自行标记已解决。建议问法采用后仍以 question 提交，不当作回答或掌握。
- 隐藏阅读页时 readerActive=false，ReaderPane 不更新阅读位置。新增返回草稿/导航/提问归类测试。
- 提问反馈以原话直接接帮助，前后端统一省去acknowledgement二次转述；ReadFeedback仅记录实际反馈和作者支撑，排除未打开支路、未展示转述与内部next_step。后续reply_to同样使用实际展示部分。

## 验证
- memory understanding 6、course 47（含bindings）通过；server teaching_coach::observation_tests 16通过（含关系候选、冻结相关状态和地图归属）。
- 8个Web文件共62个相关用例通过（按文件最后运行去重）；最终typecheck+production build通过。建议问法按question提交、ReaderPane隐藏保护、导航均已验证。
- Playwright地图与课程流程4场景通过（1366/390）；手机独立提问页隐藏原活动后，最后2个课程场景又通过。10张截图已复制到docs/evidence/understanding-map-20261112并直接查看。
- Runtime tutor_coach最终42通过、1默认忽略；类型导出137通过。课程示例合并为完整record对象，既有一次修正提供确切字段路径；未新增重试。
- 真实模型最终model-bounds两输入通过合同与内容核对：修正和帮助条件准确，提问只记具体疑问，短补学用T≥F/C且T≥R/W解释max，返回原问题，建议问句可直接提交。耗时27.637s/12.204s。原始模型ack不用于提问页面和已交付帮助。
- 较高推理强度未解决原先错误时序，已经恢复首次low/一次纠正none。全部中间失败与对照保留在验收目录，不能只用合同通过代替内容核对。
- 最后course47、反馈/播放器30、生产构建、桌面手机课程2场景复验通过；实际帮助归属补充回归1通过。最终日志为memory-course-release、delivered-help、web-release、web-build-release、browser-course-release；最终提问截图两宽已直接查看。

## 交付
本次四项范围已完成，未部署，无遗留运行中的本轮测试。实施记录、CONTEXT.md与代码链路已更新；完整证据与10张页面截图在docs/evidence/understanding-map-20261112/README.md。浏览器使用代表性接口响应，存储/状态和真实模型分别验证，未将回放称为真人学习效果研究。旧记录继续可读，不补造历史变化。

## 环境与命令
- Rust: CARGO_TARGET_DIR=D:/codex-course-build-validation; CARGO_INCREMENTAL=0; CARGO_PROFILE_DEV_DEBUG=0。Server链接约2–3分钟，日志重定向防警告刷屏。
- .env 已配置ProviderConfig，禁止打印凭证。真实模型 deepseek-v4-flash。
- COURSE_BOOK_DIR=E:/allwork/download/agent/understand-book/.tmp/reader-header-qa/book-service/library/published/ai-infra-book-complete/01a19f34-f30f-7ae0-b3ae-05a958965e20
- UNDERSTANDING_TRIAL_OUTPUT=E:/allwork/download/agent/understand-book/docs/evidence/understanding-map-20261112/model-bounds
- cargo test -p server understanding_change_and_support_model_trial --lib -- --ignored --nocapture
- Web cwd packages/web: node ../../node_modules/vitest/vitest.mjs run <具体文件>；node ../../node_modules/@playwright/test/cli.js test playwright/understanding-map.spec.ts playwright/understanding-course-flow.spec.ts --workers=1 --reporter=line
- Playwright每次清test-results，先结束最后一轮再复制截图。
