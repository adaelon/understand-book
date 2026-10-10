# 附录 E 运行、验证与实验入口

想确认一项机制时，先选择能够区分两种解释的最小运行。若问题是“记录器能否读取SSE用量”，固定响应足以回答；若问题是“换一种检索能否改善完整任务”，则需要保留真实任务、模型、程序、预算和评分条件。两者不应使用同一套成功表述。

本附录把项目入口、此前已经执行的书稿验证和历史实验放在可回查的位置。本轮仅核对文档、脚本入口与引用，没有执行这里列出的产品命令。

## 本地工作入口

下面命令以仓库根目录为当前目录。依赖与运行环境应采用项目已有配置；产物、服务或模型调用是否发生，由对应脚本决定。

| 目的 | 命令或入口 | 实际作用 |
| --- | --- | --- |
| 准备最小演示材料 | pnpm demo:prepare | [create.ts](../../../examples/quickstart/create.ts)从示例Markdown生成quickstart-demo材料，语义图为脚本构造 |
| 启动Web开发界面 | pnpm --filter @understand-book/web dev | [Web脚本](../../../packages/web/package.json)启动Vite；阅读服务仍需另行提供 |
| 构建Web | pnpm build | [根脚本](../../../package.json)转到Web构建，包含类型检查与Vite产物 |
| 检查Core类型 | pnpm --filter @understand-book/core typecheck | [Core脚本](../../../packages/core/package.json)执行tsc --noEmit |
| 运行记录器既有单元用例 | node --test evals/semantic/provider-recorder.test.mjs | [用例](../../../evals/semantic/provider-recorder.test.mjs)针对本地评测记录器；通过不自动覆盖当前生产签名或所有流格式 |
| 运行演示历史字节的定向用例 | cargo test -p runtime ex13_redaction_preserves_pre_new_object_history_bytes | [Runtime用例](../../../crates/runtime/src/presentation_author.rs)检查裁剪保留旧调用字节；第23章曾实际运行，非本轮执行 |

这些命令有不同产物：材料准备会写示例目录，Web构建会写发行文件，Rust测试会产生编译产物并可能触发类型导出。当前书稿工作只把它们列为入口。

桌面安装、插件配置、Linux release、迁入和恢复涉及具体运行组合，应沿[第20章](../chapters/20-桌面插件与Linux发布.md)读取真实脚本及前置条件。将一段启动命令复制到另一台机器，不能代替版本、材料、私人数据和环境的匹配。

## 效果评测入口与运行条件

| 入口 | 负责的问题 | 运行前必须辨认的条件 |
| --- | --- | --- |
| [evals/semantic/run.mjs](../../../evals/semantic/run.mjs)，根脚本eval:semantic | 语义相关评测入口 | 本次数据、模式、输出目录与Provider配置 |
| [agent-run.mjs](../../../evals/semantic/agent-run.mjs)，根脚本eval:agent | 真实产品回合、共同任务或显式选择的评测协议 | 题集与协议参数、Book目录、程序版本、模型、预算、独立私人状态 |
| [quality-run.mjs](../../../evals/semantic/quality-run.mjs)，根脚本eval:quality | 从已有运行形成质量评估 | historical-v2、task-v1、task-v2是不同协议；默认值不能代替本次选择 |
| [task-quality-v2.mjs](../../../evals/semantic/task-quality-v2.mjs) | 以实际交付片段建立当前评分依据 | 引文在何种视图中定位，支持与位置分别怎样判断 |
| [diagnostic-core.mjs](../../../evals/semantic/diagnostic-core.mjs) | 单题诊断与干预的结构依据 | 干预改变哪一个变量，比较哪些固定结果 |
| [provider-recorder.mjs](../../../evals/semantic/provider-recorder.mjs) | 保存请求、响应与可测用量 | 用途分类与usage完整性是否适合本次Provider响应 |

agent-run会读取本地配置并执行真实产品与模型请求，quality路径的不同子命令也有不同外部调用范围。因此，研究旧报告时应先阅读原始文件；不要为了查看一个分数直接启动整批评测。

要形成新的对照，先固定待判断的问题。保留同一材料与任务要求，写明程序和工作区、模型与采样设置、可用工具、停止条件、评分协议、执行顺序和不完整结果的处理方式。首次失败、超时和未决都属于结果；追加试跑应拥有自己的记录，不能覆盖原样本。

## 历史实验从哪里查

| 材料 | 主要观察对象 | 本书采用的结论范围 |
| --- | --- | --- |
| [LA1—LA6](../../LA1-LA6实施与验收.md) | 证据、来源、交付和恢复的阶段问题 | 用于解释失败与早期修复，当前机制另回读实现 |
| [LA7—LA10](../../LA7-LA10实施与验收.md)，2026-09-08批次 | 产品共同任务、Text/Tree/Graph同环对照及动作 | LA7与LA9-v2分开；接入探针、配置无效首批和独立修复不拼接成绩 |
| [EV3—EV4](../../performance/agent-eval-ev3-ev4-20260925.md) | 人工校准与分层题集 | 区分评分依据与真实产品回答 |
| [EV5—EV6](../../performance/agent-eval-ev5-ev6-20260925.md) | 首个单题诊断与干预 | 只解释固定题目及其对照条件 |
| [CQ8](../../performance/source-delivery-cq8-20260926.md) | 来源交付及范围表达 | 开发结果与人审校准分开；完整来源不自动消除过强推论 |
| [EV7](../../performance/agent-eval-ev7-20260926.md) | 范围提示的单变量对照 | 六对十二份首次回答，候选未达到门槛而撤回 |
| [2026-09-30构建恢复](../../performance/build-control-recovery-20260930.md) | 输入、交接、恢复与准备 | 历史耗时对应记录中的程序和阶段；当前流程按源码判断 |
| [BSR7](../../performance/book-structure-bsr7.md) | 特定BookStructure成果及后续选择 | 各消费者、所选成果和计量口径分别保留 |
| [SR6](../../performance/semantic-retrieval-20260930.md) | 构建期语义召回及Gold验收 | 排名改善与完整验收未通过同时记录 |
| [EX12.4](../../performance/presentation-staged-authoring-ex12-4.md) | 分阶段制作、反馈与完整展开 | 局部制作结果与尚未闭合的原生验收分开 |
| [EX13.6](../../performance/presentation-context-ex13/ex13-6/README.md) | 原聊天继续修订、确切版本及浏览器操作 | 支持该次同对象revision 2交付，不外推所有模型或长期学习效果 |

时间接近不等于实验条件相同。原始记录中的模型、程序、预算、题集和评分版本决定比较边界，表中日期只是查找入口。

## 怎样复用本书已经完成的验证

各章SOURCES记录了源码阅读、现有用例实际执行、固定响应重放和教学计算。需要再次判断同一分支时，先看输入、断言和输出能否回答当前问题；只有代码变化、先前失败或新条件改变结论时，才需要补跑。

| 已有记录 | 可以直接查什么 |
| --- | --- |
| [第16章验证](../SOURCES.md#第-16-章续写验证记录) | 会话事件、日志追加、原事件重试与恢复 |
| [第17章验证](../SOURCES.md#第-17-章续写验证记录) | 演示候选、预览、确切版本与源码投影 |
| [第18章验证](../SOURCES.md#第-18-章续写验证记录) | 等待期间操作、归属、取消和观察恢复 |
| [第19章验证](../SOURCES.md#第-19-章续写验证记录) | 多人准入、许可、保存恢复；含三项旧MU4夹具失败 |
| [第20章验证](../SOURCES.md#第-20-章续写验证记录) | 发行脚本、迁入恢复及替身路径失败后的单项补验 |
| [第21章验证](../SOURCES.md#第-21-章续写验证记录) | 活动、观测、评分和SSE记录器问题 |
| [第22章验证](../SOURCES.md#第-22-章续写验证记录) | 进展停止、活动容量、工作计划及检索准备 |
| [第23章验证](../SOURCES.md#第-23-章续写验证记录) | 交接、冻结输入、候选反馈、分段选择和两项记录器问题复现 |
| [第24章验证](../SOURCES.md#第-24-章续写验证记录) | 口述依据、源码摘录、历史数字与教学容量计算；没有新增产品测试 |

核对程序可能依赖当时保存的书稿副本、临时驱动或运行条件，它们用于解释那次结果，不应直接当作永远适用的全库测试入口。过去运行过某项测试，也不代表本次运行过。

## 已知计量边界

当前记录器仍只识别旧来源修复签名，生产v4可能被计为unclassified；整包JSON解析不能提取SSE中的usage，缺失时总量保持未知。第23章的固定响应同时证明可选Token上限在这种缺失条件下未阻止后续请求。这里保留问题，没有补零、估造费用或重新运行模型。

模型活动耗时、Provider服务时间、资源等待、整轮墙钟时间和页面首片段是不同量；工具预览和浏览器操作也不等于学习效果。本书的教学公式说明数量关系，不能替代同条件实测。

返回[全书入口](../README.md)，或进入[附录F：章节与面试问题对照](F-章节与面试问题对照.md)。
