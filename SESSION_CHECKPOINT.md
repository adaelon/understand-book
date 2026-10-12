# SESSION_CHECKPOINT — 2026-10-12 17:49（TB 工作区）

## 新鲜度自检
- 工作区：E:/allwork/download/agent/understand-book-reading-share；分支 codex/reading-share。
- 写入时最新 commit：9f1ea18 — docs: prepare reading-share to start TB1；读入时对比 git log -3 与工作树。
- TB1–TB5 及集成 TO3 均未提交、未部署。共同提交 93b476d，TO0–TO2 为 b45c2b6；TO3 来自主工作区未提交实现，冻结副本在 TB2 记录。

## 当前在做什么
TB5 已完成：教法说明覆盖尝试、示范与按需帮助；理解详情直接展示适用目标、帮助条件与未评分性质。TB4 实际记录经真实接口读取、纠正并重开，原话和条件保留。下一片为 TB6。

## 下一步（可直接接手）
1. 读 TB5 README 和 browser/initial-evidence.json、corrected-evidence.json，再读 TB4 接续正文与现场；区分脚本纠正、实际观察、暂定理解及未回答活动。
2. 按 TB 方案 TB6 在既有可读材料中选定一段论证／定义辨析内容，固定来源范围、读者起点、可观察活动和帮助条件，写入 TB6 记录。
3. 准备真实读者使用入口与过程记录；真实读者缺席时保留未完成项。教学模型实验先固定新增次数，TB3 的 16 次与 TB4 的 2 次授权已用完。
4. 按 TB6 真实产出运行对应内容／工程验证，记录实际采用的帮助与接续；完成后更新方案、代码链路和本页。
5. 跨支线集成保留目标支线 checkpoint，按符号合并 TB／TO；TO4 本工作区未集成。DeepSeek 专项仅用户明确要求后安排，先核对 TB2/to3-before/starting-assignment/transport-01.json 及 1,032,768 token 未结算预占。

## 未提交 / 未完成
- TB5：TutorPanel.vue 的教法说明、UnderstandingSpace.vue 的目标／帮助条件与未评分表达；两个组件测试；新 tutor_understanding_view_tests.rs、lib.rs 注册、Playwright tb5 场景／配置、验收目录及文档。
- TB4：runtime/tutor.rs 三阶段／UNDERSTANDING 指导、最终请求测试、tutor_presentation_tests.rs、Web 夹具与浏览器场景、实验和文档；原始输出及局部修正分别保留。
- TB3：SELECT/UNDERSTANDING、memory/current_interpretations 分页、server/teaching.rs 理解回读和帮助继承；Web 分页／纠正重载、tutor_adaptation_tests.rs、实验与记录。
- TB1/TB2：固定夹具／来源／recorder 透传、.gitattributes、tb2.v4 演进、orchestrator 来源后缀修复、tutor_building_blocks_tests.rs、九组交付及快照保留。
- TO3：tutor.rs spec/input_examples/spec_for_context、tool_schema.rs submit 条件、orchestrator.rs 采样刷新及 Runtime/Server 契约／示例测试；未带入主目录其他工具／首页修改。
- TB6 待实施。预读来源与固定回放不证明生产 Provider 自主循环；真人 0，无学习收益结论。TB3 原参数失败和授权修复偏差、TB4 原状态恢复／配色／引用修正见各片记录。
- 原 tmp/、packages/web/test-results/、generated 文件改动保留；TB2 历史回执未被本轮修改。TC7–TC8、TC2 内容问题、Core BookStructure 旧用例、TO1 逆序正文范围 panic 和已知旧断链沿原记录。

## 冷启动读序
1. [TB5 记录](docs/performance/tutor-building-blocks-tb5/README.md)、[原理解](docs/performance/tutor-building-blocks-tb5/browser/initial-evidence.json)、[纠正记录](docs/performance/tutor-building-blocks-tb5/browser/corrected-evidence.json)、[浏览器检查](docs/performance/tutor-building-blocks-tb5/browser/checks.json) — 视图、实际接口与隔离夹具范围。
2. [TB 方案](docs/切片方案-Tutor使命与认知积木.md)的行为合同／TB5–TB6／并行边界／验证；[ADR-0164](docs/adr/0164-tutor-mission-and-cognitive-building-blocks.md)、[CONTEXT](CONTEXT.md) 的认知积木／TeachingMove／学习证据。
3. [TB4 记录](docs/performance/tutor-building-blocks-tb4/README.md)、summary.json、sol/author/answer.md、sol/followup/answer.md；browser/followup-context.json 的 scene／understanding 与 followup-result.json 的 evidence — 已交付教具、现场和未答活动。
4. [TB3 记录](docs/performance/tutor-building-blocks-tb3/README.md)及其中最终八份正文；[TB2 Sol](docs/performance/tutor-building-blocks-tb2/sol/README.md)、[TB1](docs/performance/tutor-building-blocks-tb1/README.md)原文与内容核对 — 帮助适配与材料来处。
5. [代码链路](docs/代码链路.md)顶部 TB5–TB3；TutorPanel.vue、UnderstandingSpace.vue 及测试；runtime/tutor.rs、server/teaching.rs 的 evidence_view、tutor_understanding_view_tests.rs；[TO 方案](docs/切片方案-工具契约与Agent可用性优化.md) TO4／TO11／TO14 — 原接点和后续所有权。

## 本会话决策摘要
- TB5 复用原 prompt（expected_performance）、assistance_count、原话与解释；活动帮助次数为 0 不推断独立完成，TB4 的此前讲解和教具支持随原文呈现。
- 默认教法与本轮直接请求分别说明；普通交流未评分，正式活动沿原合同。没有新增领域模型、工具字段或生产后端变化。
- 浏览器使用 TB4 已保存理解与原事实 payload 的隔离存储夹具，纠正文案为脚本；不重跑模型或改写 TB4 历史回执。

## 验证与环境
- TB5：组件 20、Server 投影／纠正 1、browser host 1、浏览器完整路径 1 条、Web 类型检查通过；1280／320／390 像素检查通过，已查看截图。日志与复现见 TB5 README。
- TB4 历史：Runtime 29、Server 相关 4、host 1、浏览器 1、四组代码和恢复检查、类型检查通过；TB3 历史结果见原记录。本轮未重复未改动的指导和模型实验。
- TB5 新增模型 0；TB4 Sol 2 次、TB3 16 次，usage／费用未知。TB2 DeepSeek 历史 74 次 HTTP、已知 1,527,944 token，另有 1,032,768 token 预占；TB1 历史 54 次／1,187,623 token 另列。
- Rust +1.95.0；CARGO_TARGET_DIR=D:/codex-build/understand-book-tb-195；dev/test debug=0、incremental=0、-j 2；TEMP/TMP=D:/codex-build/tb1-temp。
- TB5 本地入口：tutor_tb5_captured；ignored tutor_tb5_understanding_browser_host（4187）＋playwright.tb5.config.ts（4186），结束 POST /stop。复现只重写 TB5 记录；本轮 host 已停止。
- 宽 Server tutor_ 会重写 TB2 回执，重跑前保留；TB4 完整回放也会刷新自身记录。旧 TB1 run.ps1／TB2 run-paired.ps1 会调用 DeepSeek，不作默认入口，不清零预占重试。
