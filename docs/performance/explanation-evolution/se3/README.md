# SE3：演示版本的能力使用记录

2026-10-16。状态：完成。基于 HEAD `402d9be` 的现有工作树、SE2 编译读取路径及 SE1 `discrete-update@1` 候选。下一入口为 SE4 版本级反馈存储。最后构建为默认正式空目录。

## 实施步骤

- [x] 接入 write/patch 的 method_refs 与候选/版本记录；patch 省略继承、提供即完整替换，已有引用的整页 write 必须声明。
- [x] 从实际 Provider 请求记录目录与已返回正文范围，保存私人运行事实并校验新引用的读取依据。
- [x] 验证未读拒绝、范围截断、局部替换、整页重写、旧页读取和不可变版本；D1 读取→适配→真实预览→交付。
- [x] 更新方案、代码链路、架构与主/专线 checkpoint，下一入口为 SE4。

## 使用合同

~~~json
{"operation":"write","title":"一次更新","html":"…","readable_content":"…","method_refs":[{"method_id":"discrete-update","revision":1,"scope":"更新图与公式同步"}]}
{"operation":"patch","candidate_id":"当前候选","edits":[{"old_text":"旧局部","new_text":"新局部"}],"method_refs":[]}
~~~

`method_refs` 是完整列表。patch 省略时继承；提供时替换，[] 清空。局部替换保留仍使用的项并移除不再使用的项。write 不隐式继承：基版有非空使用记录时，必须重新声明；新页省略时保存空列表。旧内容缺少字段时为未记录，read 返回 null，小修不推断历史。

新引用必须存在于当前编译目录，且确切 id/revision 有本回合成功采样时实际提供的非空正文。只看到目录、read_method 执行后尚未进入下一次采样、缺失版本都不能提供此依据。读取卡片也会如实记录为卡片；是否读了代码可由 file 和字符范围独立判断。引用从确切旧候选/版本继承时，无需重新读取，也不因该方法退出当前目录而失效。

候选的 `content.method_refs` 保存声明及服务端写入的 basis：`read {turn_id, tool_call_ids}`、`candidate {candidate_id}` 或 `version {reference}`。`content.method_observations` 保存写入候选时的本回合输入事实快照；交付时原样进入不可变内容版本。继承指向旧对象，旧对象中的原始读取依据继续可回读。

## 数据流与落点

- [Runtime 类型](../../../../crates/runtime/src/presentation.rs)、[Author 合同](../../../../crates/runtime/src/presentation_author.rs)与 [schema](../../../../crates/runtime/src/tool_schema.rs)：区分未记录、空声明、确切引用与其依据；引用及读取定位保留在精简历史中。
- [presentation_methods](../../../../crates/runtime/src/presentation_methods.rs)：从实际请求中的当前回合工具结果提取已投影正文的字符范围，排除历史回放、预算错误及重复观察；目录指导修订为 `se3.v1.catalog.1`，共同指导保持 ex14.v4。
- [Resident 循环](../../../../crates/runtime/src/orchestrator.rs)：在成功采样后、执行下一批工具前记录输入事实，包含最终收敛采样。Run 活动分别记录 `presentation.methods.catalog/read/declared`；活动中的数量表达方法数、提供字符数或声明项数。
- [Server Author](../../../../crates/server/src/presentation_author.rs)：校验新引用或继承来源，保存候选快照，read 返回使用关系；原 preview/交付资格不变。
- [私有存储](../../../../crates/server/src/presentation_store.rs)：按当前用户、会话、回合保存不可变输入批次，即使该次只读资料、没有制作页面也保留。会话和回合目录名复用 SessionStore 的 URL-safe Base64 编码，记录沿已有私有存储配额与写入路径。
- [本地端口](../../../../crates/server/src/agent_run.rs)与 [网络端口](../../../../crates/server/src/workspace_registry.rs)：沿各自当前用户/书籍授权将运行事实送入同一私有存储。

## 验证结果

| 检查 | 结果与证据 |
|---|---|
| SE3 定向合同 | Runtime 2 项、Server 3 项通过；[最终候选日志](candidate-tests-verified.log) |
| 默认正式组合 | SE3 的 5 项及 SE2 的 6 项通过，编译方法数为 0；[SE3 日志](formal-tests.log)、[SE2 Runtime](formal-se2-runtime.log)、[SE2 Server](formal-se2-server.log)、[正式输入事实](formal/observations.json)、[正式请求](formal/provider-inputs.json) |
| 引用与版本语义 | 未读/仅目录/尚未采样拒绝；错误 revision 拒绝；重复声明拒绝；小修继承、完整列表局部替换、整页显式重声明/清空、退役方法继承、旧页未记录、历史版本/候选不变及重复交付不增版本均通过 |
| 投影记录 | 使用实际正文字符数，保留请求终点，预算错误不计为读取；排除旧回合记录，重复采样不重复计数 |
| Provider 输入与候选 | 9 次实际请求、6 段读取；四文件逐字与登记源码一致；[请求](candidate-final/provider-inputs.json)、[输入事实](candidate-final/observations.json)、[候选](candidate-final/candidate.json)、[活动](candidate-final/activities.json) |
| D1 完整链路 | 13 次实际请求、6 段四文件读取、适配、三环境真实预览及交付通过；[日志](d1-verified.log)、[请求](d1-verified/provider-inputs.json)、[输入事实](d1-verified/observations.json)、[候选](d1-verified/candidate.json)、[确切交付](d1-verified/version.json)、[活动](d1-verified/activities.json) |
| 存储重开 | 重建 AppState 并加载落盘历史后，输入事实和交付版本逐项一致 |
| Runtime 演示回归 | 102 项最终通过；[原运行](runtime-regression.log)的 101 项和 [目录配置修正后单项重跑](runtime-catalog-retry.log) |
| Provider 工具合同 | 17 项通过，覆盖 Native/ReAct schema 与示例；[日志](contract-regression.log) |
| Server 演示回归 | 59 项最终通过，45 项原有显式 ignored；[原运行](server-regression.log)的 56 项及 [EX14](server-ex14-retry.log)、[EX13](server-ex13-retry.log)、[现场冻结](server-followup-retry.log)三项重跑 |

SE2 的只读 Provider 测试同时验证：普通无 Author 回答不保存目录观察；只有发现/读取而没有候选时，仍保存对应事实。测试入口为 [SE3](../../../../crates/server/src/tests/presentation_se3_tests.rs)与 [SE2](../../../../crates/server/src/tests/presentation_se2_tests.rs)。

## D1 的实际适配与观察

受控 Provider 从 read_method 实际返回的源码和运行说明组装 write，把标题、控件默认值、独立打开默认值与 initial_state 改为 η=0.5。公式仍为 `xₖ₊₁=xₖ−2η(xₖ−2)`、初值 4；所以 x₁=2，后续梯度与更新量均为 0。三个环境实际读取的 scene 均为 η=.5、k=1、p=0；公式为 `2 + (0) = 2`，预览无错误，截图进入下一次 Provider 请求后才执行 deliver。

最终交付为 `presentation-1792115999243856900-7@1`，候选身份保存在 version.json。使用关系为 `discrete-update@1`、作用局部“离散更新图与公式同步”，basis 指向本回合六次读取；四个文件的范围可逐段回查。

观察 [320×420](d1-verified/narrow-content.png)、[640×240](d1-verified/short-content.png)、[960×720](d1-verified/desktop-content.png)：图形到达 x=2，桌面可同时看到当前值 2、梯度/更新量 0、下一值 2；短屏沿原滚动布局观察。此前同源码运行的截图留在 [d1](d1/desktop-content.png)。本次验证着重于 SE3 的使用追溯，SE1 的完整几何/交互/保存恢复证据继续保留原身份。

## 复现

在仓库根目录编译候选组合并运行定向测试：

~~~powershell
$env:UNDERSTAND_BOOK_METHOD_CATALOG = 'docs/performance/explanation-evolution/se1/candidate-catalog.json'
$env:SE3_OUTPUT = Join-Path (Get-Location) 'tmp/se3-candidate-new'
cargo test -p runtime -p server --lib se3_ -- --nocapture
~~~

D1 需要本机 Chromium/Edge 和新的输出目录。在相同组合下运行：

~~~powershell
$env:SE3_OUTPUT = Join-Path (Get-Location) 'tmp/se3-d1-new'
cargo test -p runtime -p server --lib se3_d1_reads_adapts_previews_and_delivers -- --ignored --nocapture
~~~

结束前清除候选构建选择，重新验证正式空目录：

~~~powershell
Remove-Item Env:UNDERSTAND_BOOK_METHOD_CATALOG -ErrorAction SilentlyContinue
$env:SE3_OUTPUT = Join-Path (Get-Location) 'tmp/se3-formal-new'
cargo test -p runtime -p server --lib se3_ -- --nocapture
cargo test -p runtime -p server --lib se2_ -- --nocapture
~~~

相关回归过滤器为 Runtime 的 `presentation`、`tool_registry::contract_tests` 和 Server 的 `presentation`。本次复用 Cargo 刚输出的测试 exe，避免切换单包/双包 feature 组合而重复编译；直接运行候选 exe 时，SE2 的清单一致性测试仍需设置相同的构建选择环境变量。旧 Server 测试共享固定临时目录，受影响用例使用独立 TEMP/TMP 运行。

## 已知限制与失败记录

- Provider 返回受控工具调用；资产读取、请求装配、预算投影、存储、浏览器预览和交付均使用实际实现。没有付费模型调用，未验证自然选卡、跨题收益、读者学习效果或正式采用；反馈存储待 SE4，采用待 SE9。正式目录仍为空，未发布或部署。
- [首次编译](candidate-tests.log)遇到 RunContext 整体借用与消息借用冲突，改为仅借用观察字段。[第二轮](candidate-tests-2.log)发现真实会话 ID 不满足演示对象 ID 文件名约束，复用既有可逆文件名编码后通过；[修复运行](candidate-tests-3.log)和 [活动/重开验证](candidate-tests-final.log)保留。
- 首次 Runtime 回归直接运行候选 exe 时遗漏选择变量，清单一致性一项失败；补同一变量后单项通过。首次 Server 并发回归出现共享 book-dir 被删除和两个既有学习事实冲突；EX14 单独运行、两个 follow-up 用例隔离 TEMP/TMP 后通过。SE3 自身测试采用独立命名的 book-dir 和独立学习存储。原日志保留，既有共享测试目录问题未改动。
- 编译中的既有 ts-rs serde 属性警告保留在日志。
