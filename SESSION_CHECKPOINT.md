# SESSION_CHECKPOINT — 2026-10-12（TB 工作区）

## 新鲜度自检
- 工作区：E:/allwork/download/agent/understand-book-reading-share；分支 codex/reading-share。
- 写入时最新提交及两线共同基线：93b476da49b005def3f52b7b95bdaff4867e6e7b — docs: align TB design and prepare TO-TB parallel handoffs。
- TO0–TO2 的实现提交：b45c2b6602c72d3cd9604fa45c2df7ce72a97f3a。本工作区已同步这些代码、夹具、验证证据、TB 设计及既有 Tutor 改进。
- 接手时对照 git log -3 与文件状态；本支线 checkpoint 取代旧 MCP 接手入口。MCP0–MCP10 已完成。

## 当前在做什么
TB0 设计完成，下一片是 TB1：真实材料与教学行为基线。主工作区并行推进 TO3 的工具契约和状态示例；TB 在本目录工作。

## 下一步（可直接接手）
1. 阅读 [TB1](docs/切片方案-Tutor使命与认知积木.md#tb1-真实材料与教学行为基线)、[ADR-0164](docs/adr/0164-tutor-mission-and-cognitive-building-blocks.md) 和 [并行边界](docs/切片方案-Tutor使命与认知积木.md#to-与-tb-的并行边界)，确认本片只固定材料、输入和真实现状。
2. 阅读 TC0/TC6 记录与 crates/server/src/tests/tutor_loop_tests.rs、tutor_continuation_tests.rs 的既有入口，选取真实可读材料；固定原文范围、两个明确起点、误解表达及直接讲解请求。
3. 在 docs/performance/tutor-building-blocks-tb1/ 保存运行约定：共同代码基线 93b476d、材料版本、会话状态、模型设置、重复次数、费用/token 上限与停止条件，沿项目现有模型和费用约定执行。
4. 保存实际模型请求、工具返回及最终交付，区分脚本化输入与真实读者表现；列出已有可保留行为和具体教学缺口，失败按原样保留。
5. 更新 TB1 状态、实施记录及本 checkpoint；TB1 完成后进入 TB2 的文字教学指导，涉及工具契约缺口时先交回原所有者修订。

## 未提交 / 未完成
- TB1–TB6 待实施；工作区同步和设计完成不能代替教学行为验收。
- 本轮支线提交仅更新此 checkpoint；原有 tmp/ 与 packages/web/test-results/ 保留。
- TO3–TO14 由主工作区推进。TB 负责 tutor.rs 的 COMMON/SELECT/UNDERSTANDING/instruction_modules 教学语义及 Tutor 呈现阶段指导；spec/spec_for_context、工具参数/示例、共同结果投影由 TO 负责。
- 合并 TO 更新时保留本支线的 TB 接手入口；合并回 main 时保留 main 的 TO 接手入口，进度以各自方案和实施记录为准。
- 既有 TC7–TC8、真实读者验收、TC2 内容质量、旧教学事件兼容和原有 Core BookStructure 用例，沿各自记录保留。
- TO1 已记录本地/Resident 逆序正文范围可能 panic；遇到实际阻断时按原读取所有权处理。
- 正式题内动态帮助及新的类型化“独立尝试”建议仍受原合同约束，具体见 TB 方案第 8 节。

## 冷启动读序
1. [TB 方案](docs/切片方案-Tutor使命与认知积木.md) — 使命、行为合同、按需指导、并行边界、TB1 和验证原则。
2. [ADR-0164](docs/adr/0164-tutor-mission-and-cognitive-building-blocks.md)、[CONTEXT](CONTEXT.md#认知积木) — 术语与决策。
3. [TO2 实施记录](docs/performance/tool-usability/to2/README.md) 与 [TO 方案](docs/切片方案-工具契约与Agent可用性优化.md) TO3/TO4/TO14 — 当前工具基础、分工与固定对照。
4. [TC0](docs/performance/tutor-continuation-tc0-baseline.md)、[TC6](docs/performance/tutor-continuation-tc6.md)及原测试入口 — 可复用材料、实际轨迹及其限制。
5. tutor.rs、server/teaching.rs、memory/learning_evidence.rs 和 [代码链路](docs/代码链路.md) — 当前指导、真实事实与学习记忆。

## 本会话决策摘要
- 使命是提供适合当前理解的认知积木，让学习者实际运用，并依据回应调整帮助；见 ADR-0164。
- TB 对照固定同一 TO 版本；涉及请求/结果的 TO 更新需对受影响场景重新配对，保留旧记录。
- 工具合法、教学内容成立、学习者有所进展分别给出证据；当前方案优先打通文字教学。

## 验证与环境
- 基线沿用 TO2 的 42 项正确性测试、61 个最终 schema 和 23 个示例验证及 TO1 的 Context 结果；准备阶段只同步已保存成果和文档。
- Rust +1.95.0；TB 建议使用 CARGO_TARGET_DIR=D:/codex-build/understand-book-tb-195，dev/test debug=0、incremental=0、构建并行度 2；TEMP/TMP 在 D:/codex-build 下按实际片建立。
- 主工作区使用原 TO 构建目录，两线分开保存新的运行证据；本工作区尚未运行 TB1 实模或真人验收。
