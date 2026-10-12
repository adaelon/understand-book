# SESSION_CHECKPOINT — 2026-10-12（TO 主工作区）

## 新鲜度自检
- 工作区：E:/allwork/download/agent/understand-book；分支 main。
- 写入时最新提交：b45c2b6602c72d3cd9604fa45c2df7ce72a97f3a — feat: preserve TO1-TO2 tool contracts and executable examples。
- TO0–TO2 代码、夹具、日志和快照已提交；本 checkpoint 随 TB 设计与并行分工保存。接手时对照 git log -3 和目标文件实际状态。

## 当前在做什么
TO2 已完成，下一片为 TO3：Tutor 当前状态的有效示例。TB 已完成设计，reading-share 从 TB1 接手；两条线的符号所有权见 [并行边界](docs/切片方案-Tutor使命与认知积木.md#to-与-tb-的并行边界)。

## 下一步（可直接接手）
1. 阅读 [TO3](docs/切片方案-工具契约与Agent可用性优化.md#to3tutor-当前状态的有效示例)、[TO2 实施记录](docs/performance/tool-usability/to2/README.md) 和 TB 并行边界；TO 负责 tutor::spec/spec_for_context，教学指导由 TB 负责。
2. 对照 crates/runtime/src/tutor.rs、tool_schema.rs 的当前状态投影及 TC2A 轨迹，固定受支持 operation、嵌套 move 和原参数拒绝场景。
3. 用原 Tutor 测试上下文构造 source/action/fact refs；最终状态 schema 只发布可执行示例，保留原动作权限。
4. 复用 tool_registry::contract_tests 的最终 schema、真实解析与 Native/ReAct 请求验证，补状态分派的受影响用例；按固定预算分别记录参数错误和教学内容错误。
5. 导出定义并记录 TO3 的实际结果、输入体积和剩余问题，更新 TO 方案、代码链路及本 checkpoint；与 TB 集成时保持共同合同和指导一致。

## 未提交 / 未完成
- TO3–TO14、TB1–TB6 待实施；本轮准备不算实现这些切片。
- 主目录仍保留独立首页改版：Web 首页、CONTEXT 首页定义、架构及代码链路的首页段落，另有首页新增组件和测试。按 git 状态保留，不混入 TO 提交。
- reading-share 用本支线 checkpoint 接手 TB1；合并该支线时保留 main 的 TO 接手状态，TB 进度由其方案及实施记录提供。
- 既有本地/Resident 逆序正文范围可能在 Book::text 切片时 panic，远程 text_page 已校验；按 TO1 已知限制独立处理。
- Tutor TC7–TC8、真实读者验收、TC2 教学内容质量、旧教学事件兼容及原有 Core BookStructure 用例，沿各自方案保留未结项。
- MCP0–MCP10 已完成，生产状态沿 MCP 运行单；本轮 TO/TB 准备未部署。临时输出及其他 worktree 保留。

## 冷启动读序
1. [TO 方案](docs/切片方案-工具契约与Agent可用性优化.md) 第 7 节、TO3、第 9/11/12 节 — 所有权、下一片、检查与限制。
2. [TB 方案](docs/切片方案-Tutor使命与认知积木.md) 的指导装配与并行边界、[ADR-0164](docs/adr/0164-tutor-mission-and-cognitive-building-blocks.md) — 教学职责和契约职责。
3. [TO2 实施记录](docs/performance/tool-usability/to2/README.md)、同目录 after.json / sizes.json、[代码链路](docs/代码链路.md) — 已完成成果与实际证据。
4. [TC2A 轨迹](docs/performance/tutor-continuation-tc2a.md)、tutor.rs、tool_schema.rs、tool_contract_tests.rs — 使用错误、状态投影和验证入口。
5. TO 方案第 6 节 R1/R3 的固定源码链接 — 选择权、示例和最终发布验证原则。

## 本会话决策摘要
- TO 维护工具契约/示例/共同结果投影，TB 维护教学指导/帮助适配/相关界面；共享合同由原所有者修订。
- 同组 TB 对照固定同一 TO 实现、材料、模型和采样预算；实际参数、教学内容与学习者表现分别给依据。

## 验证与环境
- 沿用 TO2 已完成的 42 项正确性测试、61 个最终 schema 和 23 个发布示例验证；TO1 的 5 项 Context 验证也已归档。本次准备未修改执行代码。
- Rust +1.95.0；TO 沿用 CARGO_TARGET_DIR=D:/codex-build/understand-book-no-progress-195，dev/test debug=0、incremental=0、构建并行度 2，TEMP/TMP 在 D:/codex-build 下。
- TB 使用独立构建输出目录，避免两条支线反复覆盖构建缓存；环境细节以各片记录为准。
- 实模收益与真人学习表现尚未由 TO2 证明；默认忽略入口需按对应切片预算显式执行。
