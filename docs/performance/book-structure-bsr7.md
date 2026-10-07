# BSR7：全书 BookStructure 对照与交付

状态：2026-10-03，已完成。全书发现、14单元章节来源验收及C/B两组11主题的真实生成完成，均官方next complete。两组最终来源与结构投影通过，open=0；两组真实fresh全书重放、Engine发布、新书安装位置的完整质量及已安装Reader/Book MCP双入口验收均通过。C按用户embedding优先选择作为主交付，原书保留。

## 执行范围与授权

用户要求读取checkpoint、实现BSR7并完成后刷新，选择独立Codex子代理，明确要求先使用embedding、允许3个生成代理并发，token总量无预算，并授权验收后安装新版本。提交上限从800经明确授权增至1200，失败与重试照计；原开始2026-10-02T02:26:22.597Z保留。最新明确生成截止为2026-10-03T02:56:00Z（香港10月3日10:56）；额度和截止不因中断、并发或B派生重置。[额度修订](book-structure-bsr7-20261002/quota-amendment.json)和[时间修订](book-structure-bsr7-20261002/deadline-amendment.json)保存授权。

实验[plan](book-structure-bsr7-20261002/plan.json)与[session](book-structure-bsr7-20261002/session.json)记录每次交付和接纳。新工作区仅复制六项source/公共前置成果，不复制旧BookStructure中间结果或发布历史；原书保留，旧结构仅作为历史对照。五处公式语义误标按实际原文定点修正于新副本，[formula-corrections](book-structure-bsr7-20261002/formula-corrections.json)记录前后值。

C为semantic_required，词法加已安装MiniLM L12-v2/q8/CPU/384维；B为lexical_only。主题计划247接受后，在任何主题next之前派生B，两组共享来源、2636候选、章节选择、11主题/47定位种子、prompt与生成预算，仅检索方式不同。新的独立opaque生成者只读各自next的prompt/input，不读实现、task、session/report、另一组或审阅答案；每个主题实际执行有意义非空search，再inspect/read取得判断证据。

## 全书发现与章节来源验收

发现覆盖14单元、572/572正文片段、9111/9111 core叶节点，保留2636候选。全书框架加发现共576次提交，573接受、3拒绝。序列化输入/输出估算2541915/455481 tokens；各执行区间相加26054528ms，实际发现墙钟11366548ms（3小时9分26.548秒）。[发现快照](book-structure-bsr7-20261002/discovery-report.json)独立保存该阶段成本。

最终章节选择946重点、345宏观路线点，各项meaning/conditions/reason、章节摘要和完整原文的重要主线已经来源核对。宏观路线是完整重点的有序子集，不通过缩短宏观路线删除重点。

| 单元 | canonical章节 | 重点 | 宏观 |
|---|---|---:|---:|
| 1 | 封面 | 0 | 0 |
| 2 | 前言 | 24 | 7 |
| 3 | 第1章 初识AI Infra | 36 | 11 |
| 4 | 第2章 模型架构 | 70 | 17 |
| 5 | 第3章 推理与训练负载 | 60 | 17 |
| 6 | 第4章 加速器架构 | 60 | 20 |
| 7 | 第5章 算子与运行时 | 59 | 21 |
| 8 | 第6章 超节点 | 68 | 25 |
| 9 | 第7章 数据中心网络 | 67 | 30 |
| 10 | 第8章 推理优化 | 66 | 31 |
| 11 | 第9章 分布式推理 | 87 | 36 |
| 12 | 第10章 训练系统 | 114 | 44 |
| 13 | 第11章 资源调度与运行环境 | 106 | 37 |
| 14 | 第12章 端边云协同 | 129 | 49 |
| 合计 | 全书 | 946 | 345 |

来源凭据：[基础1–7](book-structure-bsr7-20261002/chapters-1-7-review.json)、[推理优化](book-structure-bsr7-20261002/inference-discovery-review.json)、[系统8/9/11/12](book-structure-bsr7-20261002/systems-chapters-review.json)、[资源调度13](book-structure-bsr7-20261002/chapter-13-source-review.json)、[端边云14](book-structure-bsr7-20261002/chapter-14-source-review.json)。审阅者只核对正式选择与实际源，实际问题驱动有限修订，已经通过的范围沿用原审阅。

原文审阅发现并解决四个具体问题，原初选、问题及修订历史都保留：

- 超节点章补入注意力DP/专家EP综合算例：扩大卡数释放每卡权重容量、吞吐提高而单用户速度不变；继续扩大受KV读取、计算和All-to-All限制。
- 网络章补入固定全局token时重新核算TP/DP组合的局部负载形状、有效算力与通信；保留kernel/KV布局及余量条件。
- 分布式推理章补入EP扩大使每卡专家减少、波动抵消变弱、等待域扩大，从而最忙卡相对平均偏差放大的因果链；区分算力/接收受限与小batch权重受限，固定种子模拟平均不冒充实测或每批结论。
- 端边云章初选f33-1把每步5.47GB称为权重读取；实际是约4.26GB权重+1.21GB KV。正式修订剔除该非宏观误标点，其他129重点/49宏观及顺序保持；摘要准确保留同Qwen3-8B q4_0/8K/每轮45token公平比较、decode读取下界、一次prefill和端侧准备取0等乐观条件，delta来源验收通过。

## 实现与确定性验证

`full-book.ts`调用正式Core task reader、冻结、当前输入writer、检索准备、quality和Engine阶段关闭。分章发现session隔离响应，共享计时/额度；失败原响应保留，完成观察先于额度门禁。`request-chapter-revision`将具体源审问题持久化，在原章选择和完整seen/evidence账本上重新接纳，不重新发现。`verify-full-book.ts`在fresh工作区首次修订前恢复原请求。

实际大输入失败由有限交付修正：章节候选索引和章节/主题原文目录各64项分页，read保留稳定全局索引；inspect附原文位置。章节inspect/read省略上一动作已交付的重复预览，browse仍正常交付，完整seen/evidence账本保持。纯标题core携带graph正文上下文时，overview要求按core实际正文判断，与发现prompt一致。

主题规划保留全部14章节摘要/问题和345宏观种子引用，候选meaning使用32字导航，完整条件仍由search/inspect交付。实际12单元259种子21123>20000，种子plan仅移除JSON排版空白后18067；实际13单元296种子20195>20000，导航48→32字后17059。真实fixture先红后绿，旧无种子输入字节保持。最终14章及末章修订后，正式Core task reader实测19304/20000，通过[完整容量凭据](book-structure-bsr7-20261002/theme-plan-full-capacity.json)，没有模型/Provider调用。

训练系统章114重点/44宏观的完整引用输出2643，实际candidate容量1981（3500为预留），即使单字符摘要引用下界2228仍超限。三次拒绝后停止并修正：从已接受动作22启用有限`structure_chapter_selection`合同，每次select_stops至多48引用，最终Core注入全部草稿并执行原选择/来源门禁；边界前任务/prompt/proof保留。真实193–196分段48/48/18+final已全部接纳，完整114/44来源通过。`resume-after-fix`保留三次失败、原时间和计数，记录具体修复停止点；新next直接交付实际candidate容量。

阶段关闭采用BookStructure范围，继续验证当前来源、输入、质量、完整覆盖、回执和新鲜度；有效公共前置成果无需重新抽取上游即可发布当前结构。普通全书路由仍按原阶段依赖执行。CLI导入不执行入口，源码与compiled sidecar显式调用同一runAutomaticBuildCli；CommonJS入口回归已修正。

主题原文审阅在生成过程中分批核对已接受结果，只审新增或变化部分。实际发现三处局部问题：B将kernel路径解释改为kernel耗时表征、B混同有效吞吐分子与观察时间分母、C将附近工作站/云端比较称为端云；均已登记正式请求并真实接纳修订。Core复用既有`reopenStructureTheme`，原主题与一次初始收敛重放后顺序定点重开，保留其他主题和证据账本，以原theme prompt/retrieval/writer接纳，修订后不再全目录收敛。`full-book.ts request-theme-revision`保存事件，fresh重放首次修订前恢复原请求；生产fixture+主题12项、两模式fullbook/fresh重放/比较/发布2项及typecheck通过。B最终来源关闭两项问题，C最终称谓三字段修订也已resolved；两组最终目录、结构投影、全部阶段、依赖及额外重点均来源通过，open=0，初审和修订历史完整保留。[B最终来源](book-structure-bsr7-20261002/themes-B-source-review.json)、[C最终来源](book-structure-bsr7-20261002/themes-C-source-review.json)保存实际验收。

相关验证已经通过：章节planning8项、有限分段/来源修订writer1项、词法/语义两模式fullbook与fresh重放2项、主题11项、Core类型检查；阶段关闭/生产恢复/路由相关回归通过。这些确定性测试不替代本轮实际全书内容验收。

## 真实全书主题对照

[comparison.json](book-structure-bsr7-20261002/comparison.json)确认同候选目录、同共享章节选择和共同候选正文；共享前缀806提交，序列化输入/输出估算5104359/597064 tokens。C/B匹配主题阶段分开计量，累计实验891/1200提交，历史7拒绝保留；两组本轮主题无新增拒绝。

| 实际指标 | C：词法加embedding | B：纯词法 |
|---|---:|---:|
| 主题阶段提交/接受/拒绝 | 39/39/0 | 46/46/0 |
| 输入/输出序列化估算tokens | 324899/10962 | 371792/11568 |
| 首次交付至最后接纳ms | 4945835 | 4446630 |
| 最终章节单元/重点/主线 | 14/956/11 | 14/948/11 |
| 主题阶段/前置依赖/新增重点 | 49/9/10 | 47/7/2 |
| 实际embedding文档/查询/调用 | 2650/12/344 | 0/0/0 |
| 实际embedding输入tokens/调用累计ms | 125159/15690.6121 | 0/0 |

C补入执行时间合成、共享KV取舍、AllReduce计量、部署选择、专家容量规则、样本筛选与梯度区别、RL分布和执行闭环、写入重叠与异步返回尚未提交等10处额外重点。B补入注意力两条计算路径与部分和两处额外重点。各项均按真实来源验收；数量不替代质量判断。C按用户embedding优先选择作为主交付，B保留同源对照结果。

两名独立生成者使用不同实际查询和动作，且墙钟包含执行与审阅期间的停留；本轮C提交和序列化估算较少、主题墙钟较长，不能归因为embedding稳定提速或严格费用下降。实际Codex模型calls/input/output tokens仍未知为null。

## 成品与交付状态

纳入正式主题来源修订的最新源码和成品25提示资产Node/Bun canonical parity、T7 executor传输、plugin-release、release-config通过；完整NSIS构建/export成功。[package-final-build](book-structure-bsr7-20261002/package-final-build.json)记录实际文件、时间和大小。候选安装包为`dist/UnderstandBookSetup-BSR7-20261002.exe`，63177800bytes；插件新版本`0.1.0+codex.20261003012800`。

安装位置为E:/allwork/Understand Book，旧Reader/Book MCP/build三exe和web资源已备份。新Reader、Book MCP、build和web资源已实际安装，[二进制安装凭据](book-structure-bsr7-20261002/binary-installation.json)记录文件大小和时间。运行中的旧MCP/build文件分别改名保留。新书已完整复制到E:/allwork/download/agent/lifebook/versions/bsr7-20261002/.understand-book/ai-infra-book-complete，[持久目录凭据](book-structure-bsr7-20261002/durable-book.json)记录实际结果，原发布保留。

插件沿现有本地understand-book市场安装为`0.1.0+codex.20261003012800`，其他市场保留。CLI add两次遇Windows缓存访问拒绝，随后将已验收published插件完整复制到独立新版本cache；实际Codex plugin list确认新版本installed=true、enabled=true，缓存manifest一致。[插件安装凭据](book-structure-bsr7-20261002/plugin-installation.json)保存实际安装方式与缓存刷新问题。

真实C/B主题生成、实际非空检索、对照及两组最终来源验收已完成。[C真实重放回执](book-structure-bsr7-20261002/verification.json)恢复838个接纳步骤；[B真实重放回执](book-structure-bsr7-20261002/lexical/verification.json)恢复共享799及自身46个接纳步骤，共845步。两组独立fresh工作区均以当前Core重新交付，输入与prompt、完整候选目录和最终全书结构一致，full quality通过，9111叶节点无覆盖缺口，模型/Provider调用均为0。保存的Core检索准备仅在当前请求及完整候选依赖一致时复用。

[C发布回执](book-structure-bsr7-20261002/publication.json)与[B发布回执](book-structure-bsr7-20261002/lexical/publication.json)均为closed，实际公开结构分别为14单元/956重点/11主线与14/948/11，宏观路线均345点。已安装compiled CLI直接对新位置执行book_structure/full quality，[完整质量凭据](book-structure-bsr7-20261002/installed-quality.json)为gate/integrity passed，9111叶节点无缺口、无重叠。

Reader启动会恢复之前保存的书籍；本次通过已安装Reader的实际`POST /book/open`加载新书并保存current_book_dir，再重启本任务启动的Reader使用该新书。[选书凭据](book-structure-bsr7-20261002/reader-session.json)与[运行入口](book-structure-bsr7-20261002/reader-active-host.json)记录实际状态。已安装Book MCP分别以显式目录及无目录参数、实际Reader保存的当前书执行book_guide_path和全部14单元book_structure；[显式入口](book-structure-bsr7-20261002/reader-explicit/reader-verification.json)及[Reader默认入口](book-structure-bsr7-20261002/reader-active/reader-verification.json)均passed。章节身份、摘要、重点和锚定原因、宏观顺序与完整主线一致，956重点/345宏观/11主线全量可读；训练系统章117重点（114章节选择加3主题补充）/44宏观完整，模型调用0。

真实读取验收曾在unit7发现“阶段耗时”变为“阶段���时”，60个重点均存在。差异来自验收工具对每个stdout Buffer独立解码，中文UTF-8跨块被替换。`verify-reader-full-book.ts`改为stdout/stderr连续UTF-8流解码，并在比较前保存投影；同一真实全书双入口随后通过。Rust读取/序列化保持原文，生产成品无需因此重建。[原失败差异](book-structure-bsr7-20261002/reader-projection-diff.json)保留诊断证据。

## Linux书籍同步

2026-10-03 16:51按用户后续请求发布到`https://115.190.121.150`。正文与base同原线上书逐字节一致；沿用线上来源清单、asset_manifest及全部图片，加入完整C组book_structure、discourse_index和已修正的formula_semantics。公共阅读产物已同步，约1.6GB构建/重放档案保留在Windows交付目录。正式程序沿用实际release `/opt/understand-book/releases/ex13-jl-20261002`，本次无源码或程序更新。

新默认发布为`ai-infra-book-complete / 01a100f5-e3c3-72f3-8bfb-eb8a08774ab5`，继承reader、puff、adaelon的既有授权。停服前活动/未保存运行0，先用原生维护工具保存一致性备份，再用原生publish_book导入和授权，服务恢复active。原发布、笔记及聊天保留；备份位于`/opt/understand-book/backups/bsr7-20261003/service`。[发布回执](book-structure-bsr7-20261002/linux/deployment.json)与[原生发布清单](book-structure-bsr7-20261002/linux/publication.json)记录实际引用和目录。

[公网Reader验收](book-structure-bsr7-20261002/linux/reader-verification.json)通过HTTPS认证入口读取全部14单元，956重点的身份与锚定原因、345宏观顺序及11完整主线与交付结构一致；默认版本可见，原图片读取成功，模型请求0。验证脚本按Reader已有可选标题语义接受未填写标题的null表示，兼容服务器Python 3.6，并使用原生发布清单选择图片。已打开的旧阅读现场继续绑定旧版本；从材料选择中打开新版（默认）即可使用新结构。

## 已知限制

- Codex实际模型调用数和input/output tokens不可得，保持null；序列化估算不等于实际模型用量。本地embedding按真实账本单列，不把无可见usage报成零费用。
- 完整墙钟含中断和人工审阅，不以执行耗时相加或历史702项估算声称严格费用下降或全书提速比例。发现成本、共享整理前缀与匹配主题成本分别比较。
- 2636原发现候选及历史响应保留，仍含`unit:14:fragment:0032#f33-1`把5.47GB总读取误称为权重读取的误标；该点已从正式章节选择移除，两组最终主题均未选择。其他单元的同后缀引用不是这一误标。
- 旧结构的36重点/53主线与本轮数量差异不直接证明质量；质量依据是实际机制、成立条件、各章发展和有证据的依赖。
- 本轮交付范围为BookStructure；SR其他消费者的历史验收和产品默认模式不因本轮实验自动改写。
- 本聊天的既有Book工具连接仍返回`book_structure.json not attached`，尚未附着新书结构。新启动的已安装MCP显式及Reader默认入口已全量通过；本轮没有重建旧聊天连接。
