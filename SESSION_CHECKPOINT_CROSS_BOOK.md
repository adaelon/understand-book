# SESSION_CHECKPOINT_CROSS_BOOK — 2026-10-16

## 新鲜度自检
- 写入时最新 commit：402d9be feat: preserve current reader, focus mode and runtime work。
- 本功能在当前工作树实现；发布、自进化、缓存及其他 UI 工作线也有未提交改动，禁止整文件覆盖或整体回滚。

## 当前在做什么
跨书补讲 CB0–CB5 全部完成。内部授权多书读取、私人理解读写、多书证据/引用、原任务追问、卡片与笔记接续均通过自动验收。未提交；2026-10-16 16:02 HKT 已随 current-20261016 上线，见 docs/performance/current-release-20261016/README.md。

## 下一步（后续可选）
1. 阅读 `docs/切片方案-跨书补讲与理解接续.md` §7–8，查看实际验收与边界。
2. 如需真实模型体验，用已获两书授权的账号在原书询问 Rust 知识困难，验证自由选书和讲解质量；该评测本次未执行。
3. 如需提交，按本线代码链路选择改动并保留其他工作线；部署已完成，无需重新实施 CB0–CB5。

## 未提交 / 未完成
- 代码、设计、ADR、术语、架构、代码链路及本 checkpoint 均为工作树改动；本次要求的实现无剩余事项。
- 首版参考材料限账号授权的已发布书库，未连接该书库的单机运行不暴露 library 工具。
- 真实模型补讲质量评测未执行；部署已完成；既有需要专用环境的模型/浏览器用例保持默认忽略。

## 冷启动读序
1. `docs/切片方案-跨书补讲与理解接续.md`、`docs/adr/0169-cross-book-reading-and-private-understanding.md`。
2. `docs/架构.md` 的跨书补讲节、`docs/代码链路.md` 的 CB1–CB5 条目。
3. `crates/runtime/src/library.rs`；`orchestrator.rs` 的多书证据、library 分派与进展；`tool_result.rs` 的 library 投影。
4. `crates/server/src/cross_book.rs`、`workspace_registry.rs` 的 NetworkRunPort/来源交互、`interaction_input.rs`、`presentation_store.rs` 的参考来源保存。
5. `crates/server/src/tests/cross_book_tests.rs`、`mcp_read_tests.rs` 的跨书测试，以及 `presentation_store_tests.rs` 的跨书卡片测试。

## 验证结果
- Runtime：cross_book 4、tool_registry 9、tool_exposure 28、agent_prompt 5、source_ 59、progress 9 通过（过滤集合有交叉）。
- Server：cross_book 4、mcp_read 10、tutor_continuation 62、presentation_store 29、session_event 3 通过；追问与卡片合计 13 个既有模型/浏览器专项用例默认忽略。
- 最终 NetworkRunPort 用例在连续三页目录后完成补讲，覆盖普通追问、继续/澄清、私人理解、重开、授权撤销和原 Reader/聊天保持。
- Web：RightRail 60 通过；类型检查通过；NoteSourceBinding 已重新导出。

## 决策摘要
- 内部复用读取服务，身份来自 Run；无远程 MCP 自调用、额外凭证或调用包扣次。
- SourceBinding 携带 publication_id 和既有 source_revision；本轮只引用实际读到的正文。
- 理解归当前用户 LearningMemory 和参考来源，真实反馈形成未评估假设；交付后记录帮助，修正沿 supersedes。
- 根 checkpoint 属于其他工作线，本线状态独立保存在本文件。
