# SESSION_CHECKPOINT — 2026-10-12 21:07（TO 主工作区）

## 新鲜度自检
- 工作区：E:/allwork/download/agent/understand-book；分支 main。
- 写入时最新 commit：93b476d docs: align TB design and prepare TO-TB parallel handoffs。
- 读入时对比git log -3；TO3–TO14及选区保存修复已完成但未提交，不能只按HEAD判断。

## 当前在做什么
TO0–TO14已完成。TO14发现的“选区＋保存”意图丢失已修复，38项定向测试通过。TO14原始16任务仍保留13通过、2笔记失败、1执行器未决的修复前结果；下一推荐独立小片是EXISTING笔记的Goal完成判断。TB仍由reading-share按原分工维护。

## 下一步（可直接接手）
1. 阅读selection-intent-fix/README.md及production.diff，确认run_context_inner已经使用current_user_message识别动作，不再重复修此问题。
2. 如接续EXISTING问题，先读to5 README已知限制，定位Runtime的Goal reader_action完成判据与reader.note成功回执路径。
3. 用已存在笔记和明确保存要求补真实Server失败回归：回执为EXISTING、重开仍为同一笔记、本轮无创建effect，核对Goal是否错误保持未完成。
4. 在现有完成判据接点依据真实保存回执处理已完成保存，保留去重、撤销及非笔记Reader操作语义；只运行受影响用例，再更新代码链路和checkpoint。
5. 若继续性能收缩，按TO14 README选择可归因的具体投影与固定任务；保留原始样本，基线和候选使用不同CARGO_TARGET_DIR。

## 未提交 / 未完成
- 最新修复：crates/runtime/src/orchestrator.rs一处动作识别输入；Server selection_intent_tests.rs及父模块入口；selection-intent-fix/红绿证据、说明与相关文档，均未提交。
- TO14：evals/tool-usability/执行器、判定／测试／材料核对；Server ignored试验适配器；package scripts；to14源码快照、原始轨迹、汇总和阅读案例，均未提交。
- TO3–TO13：契约示例、发现／读取交接、覆盖／续取／预算、输出schema、Executor生命周期、生成参考文档已验证未提交，详见各片记录。
- 独立首页：CONTEXT、架构／代码链路首页段落、Web首页及新增组件／测试；已有改动、其他输出及worktree保留。
- 已知独立问题：EXISTING笔记不产生创建Reader effect，Goal若要求reader_action仍可能判未完成；本次未修改这一完成判断。
- TC2历史数学条件、TO3重复正文建议、真人学习验收、TC7–TC8、旧教学事件兼容及Core BookStructure原用例沿原记录。
- 本轮未提交Git、未部署生产服务、未更新安装态插件；MCP0–MCP10沿生产运行单。

## 冷启动读序
1. docs/performance/tool-usability/selection-intent-fix/README.md、production.diff、results.json — 已完成修复、真实红绿证据和权限边界。
2. docs/performance/tool-usability/to5/README.md已知限制；crates/runtime/src/orchestrator.rs的Goal完成判断与reader.note分派；crates/server/src/goal_example_tests.rs — 下一独立缺陷。
3. docs/切片方案-工具契约与Agent可用性优化.md第5/9/11/12节；docs/代码链路.md最新修复与TO14段；docs/工具契约参考.md的Reader／Memory／Goal表面 — 所有权与实际合同。
4. docs/performance/tool-usability/to14/README.md、PLAN.md、coverage.md、content-review.md — 固定实验的失败分母、输入成本、数学核对和未测范围。
5. 如需模型实验，再读to14/reading-case.md、source-manifest.json、evals/tool-usability/入口、to10 README实验模型约定及Tutor使命方案TO/TB并行边界。

## 本会话决策摘要
- 选区包装中的user_question是JSON字符串，原动作识别连它也排除；改用已有current_user_message，缺省时沿用question。原引号过滤、模型包装和证据校验保持，见selection-intent-fix README。
- 回归实际经过/agent/chat、工具发现／分派和磁盘重开；明确保存为CREATED且1条，纯解释与正文保存命令均拒绝且0条。模型仅为固定决策驱动，本修复无真实模型采样。
- TO14基线67efb60，候选93b476d＋当时TO改动；同一gpt-6.1-sol／medium、CLI0.162.0-alpha.2。16预选任务保留原结果，无失败补抽；原预算停止及仅补未启动任务单列。
- TO14未测出误用下降；雨水累计输入+9.39%，Tutor有效配对+9.70%。整组差异不单独归因某条示例；5份交付和6份持久动作的数学条件核对未出现本批过度概括。
- 共享target曾混用基线read-tools产物；已清理read-tools／book-tool-contracts编译缓存重建，原失败日志保留。TO14复跑说明已改成每版本独立target；冻结程序及原始试验数据未改。

## 验证与环境
- 修复：新增Server红测1项预期失败，改后新增3项通过；既有Tutor UI 1、Runtime tool_exposure 31、write_examples 3通过，共38项。证据见selection-intent-fix/。
- TO14：两版Server编译、Node评测器11项、16份材料／CLI／模型配置核对通过；全16任务实际1,754,227 token，货币费用null，活动输入与累计usage分别记录。
- 无模型修复复核：cargo --config 'profile.test.package.runtime.codegen-units=16' test -p server --lib selection_intent:: -- --test-threads=1；其余3条命令见修复README。
- Rust：RUSTUP_TOOLCHAIN=1.95.0；主工作区CARGO_TARGET_DIR=D:/codex-build/understand-book-no-progress-195；dev/test debug=0、incremental=0、CARGO_BUILD_JOBS=1；TEMP/TMP=D:/codex-build/to3-temp；TB及不同源码版本独立target。
- TO14冻结程序仍在D:/codex-build/to14-bin/baseline.exe、candidate.exe；源码在to14/snapshots/。本修复未覆盖这些程序，当前测试程序由主工作区重新构建。
