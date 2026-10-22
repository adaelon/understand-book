# SESSION_CHECKPOINT_SELF_EVOLUTION — 2026-10-22，C 方案教练前移与原题对照已完成

用户已授权：沿用 C 完整材料供给，让教练先理解需求、选方法，再由主 agent 首次生成；原两题各新增一次，与旧 C1/C2 比较。禁止改线上。

- 新证据根 `tmp/coach-first-20261022/`；报告目标 `docs/performance/explanation-evolution/coach-first-20261022/`。新 common-source 从旧冻结 C 原样复制，初始 1040 文件逐字节一致；没有叠加主工作树其他修改。
- 显式当前请求信号 `presentation_planning_requested`（默认 false）、前置调用和同请求缓存已实现。26项独立定向测试通过，D binary `binaries/author-D.exe` 已归档；`build-logs/build-D-record.json` 保存新source实际重编证据。恰8文件变动，144个prompt/方法固定文件逐字节一致；前置 packet 的 `author_proposal` 为 null。这是时机与随之必要改变的输入条件，不声称严格单变量因果。
- 新流程在首次 build_sample_request 之前准备完整选中方法，实际首个作者请求已有完整正文及 runtime receipt 证据；普通问答不新增前置调用，未收到信号的原 prepare fallback 保留。一般自然语言意图路由尚非本轮目标，不能宣称所有线上入口已前置。实现仅在本地冻结候选副本，未合并主工作树 runtime、未上线。
- 两固定新位置均已完成首交付，后台PID96340退出；禁止再次启动或补样。29调用全部served deepseek-flash且usage齐全，新费峰价估算1.91682816元，新未知预留0；旧21.36272960另列，合计23.27955776。详见 experiment/final-evidence-note.json、activity-budget.json。
- 两页真实顺序均request-000教练→request-001首作者→prepare→write；作者方案在教练输入中显式null，admission真，各只1次教练。反传process-reveal@2完整5文件29748字符、岭回归再加compare-one-factor@3完整10文件48231字符，均在首作者请求且回执齐全。不是自主read_method；chart-reveal@3仍未选。
- 所有实现、构建、实验、独立评审与打包代理已完成；无活跃付费进程。不要覆盖主工作树已存在的改动。原 C 到前移版 8 文件补丁已随报告保留，基准是旧冻结 C，不能宣称可直接套到当前主树。
- 两题共6页匿名独立评审完成，含完整规定数值状态、步进/预设/额外交互及三视口。反向传播：前移版在桌面整体概念讲解上优先，明确直接边与完整路径、汇合后的贡献；改输入后仍有默认贡献文案未更新，小屏图偏小。旧C2细粒度步进和小屏更好。岭回归：旧C2优先、旧C1其次、前移版最后；前移版交互数值和可调退化最优解族正确，但中心化残差符号、均方与整体缩放、退化0/0套公式、数据净拉力解释有实质错误。不能声称稳定质量增益，也不能抹掉局部改善。
- `delivery/conclusion.json` status final；报告 `docs/performance/explanation-evolution/coach-first-20261022/comparison.html` 和 README 已正式完成。1440/320布局、两题页签、6页位置、49个直接本地入口通过；8份报告加95附件共103文件、90个Markdown引用可读。QA截图与记录在报告 evidence/delivery-check/。原模型页与评审原文未被打包过程修改。
- 本轮是两题各1个新首交付与旧C各2个历史首交付比较，配置相同、规划各自生成，非同时重跑或严格单变量；未运行 active 教学审核修订循环，不能据此评价线上审核拦截。真实学习者增益与模糊需求澄清未测。反传交付 method_refs=[]，岭回归声明两法；正文供给证据与作者声明区分记录。
- 当前建议：保留前置候选机制继续迭代，不按本轮结果直接上线；反传新页作桌面基础、借鉴旧C2小屏，岭回归保留旧C2。后续应检查中间推导、状态变化后的文案/图一致性；尚未启动额外生成或修订。冷启动先读本段及本轮 README；不得把“继续”理解为重跑旧12项或新2项。

---

# 2026-10-22 历史：三组首次交付实验完成

用户要求三个对比：A 无教练；B 有教练但主 Agent 自主决定读取；C 有教练且完整提供教练选中的全部方法材料。所有工作在本地冻结副本进行，没有部署线上。

- 证据根目录 `tmp/coach-three-arm-20261022/`；最终报告 `docs/performance/explanation-evolution/coach-three-arm-20261022/comparison.html`。默认展示三组都有交付的反向传播第2次，全部四个页签/12位置保留。
- 原定两题×三组×两次，共12固定位置；11份首次成功交付、backprop-B1宿主中断保留、0未启动、无补样。主批和恢复批均已结束，PID56920退出，禁止把“继续”理解成重跑这12项。
- 原 `runs/` 保留A1成功和B1中断；`runs-recovery/` 汇总全部12位置。B1缺最终usage/完成标记，3.145728元最坏预留保留，不评页面质量。
- 所有172个完成调用返回deepseek-flash（请求deepseek-v4-flash），模型/共同作者输入与配置相同；B/C各自独立规划，不是固定同一计划只切换供给。
- A4页无教练/正文自读0；B3成功页收到教练规划但正文自读0；C4页在首写前均获得选中方法全部文件，反向传播每页5文件、岭回归每页10文件。实际正文/运行回执/交付声明分开留证，不能把完整提供当作内部理解或质量提升。
- C实际选择process-reveal@2及岭回归compare-one-factor@3；B另有compare-one-factor@2/part-whole@2。chart-reveal@3虽然共同可用，本轮未被选、读、提供或声明，不能声称直接验证CS336教师图表反例迁移。
- 教练实际时机是用户需求→作者公开framework/focus→教练规划选型→材料交接→首写。七个成功教练均在作者初步方案之后介入；未测试教练在作者构思前先行。
- 两题独立匿名评审已完成，实际操作11页、完整阅读和三种视口。反向传播：A2/Oak核心路径与相消解释更稳，B2/Maple精确扰动更具体，C2/Birch按步及窄屏更顺；无全面胜者。岭回归：C2/Amber按深入推导与图数一致用途默认优先，A2/Jade同属优先组且手机数字表更好。组内波动与实质错误保留，未观察到稳定组别优势，也不等于教练或示范普遍无效。
- 公开教练规划本身有反例：backprop-B2将漏贡献概括为低估/变小；x=2,y=-1时gx从正确-3变成漏算0，非总变小。作者公开输入没有同一概括；仅记录输入输出差别，不据此归因最终错误。证据 `experiment/coach-plan-observation.json`。
- 6项定向测试、8项runner/evidence离线验证及三binary同源验证已完成；只有ARM一行变化。生成后已重新运行修正System大小写的summarize.py，最终全文统计正确，不再使用进程缓存的旧解析。
- `experiment/final-evidence-note.json` 与 `activity-budget.json` 已结清：usage峰时估算11.66597880元 + 未知预留3.14572800元 = 本轮保守14.81170680元；原内部上限20元未变。旧活动6.55102280单列，跨这两轮21.36272960，无重复相加；不是供应商实付账单。
- `delivery/conclusion.json` 已由root写为final。最终打包与实际验收完成：1440/320布局、四页签、11页/12位置、554个本地入口通过；484个复制目标、243条引用无问题。评审文件/截图与两题实际引用依据完整收录，验收截图在报告 `evidence/delivery-check/`。原始页面与质量判断未被打包过程改动，已向应用提交最终comparison.html预览。
- 下一步建议尚未实施：不将强制全文作为已验证有效的线上默认；优先单独测试教练在作者初步方案前根据用户需求定阅读目标和方法，并核对规划自身的概括。输入已详细指定学习目标，本轮不衡量教练澄清模糊需求的全部价值。
---

# SESSION_CHECKPOINT_SELF_EVOLUTION — 2026-10-22，教练选型交接诊断完成

## 当前工作：按本次需求选择表达方法

用户已授权本地实现并继续同模型对照，仍禁止改线上。当前证据目录 `tmp/coach-method-handoff-20261022/`；前一批 CS336 的结果保留在下方。

- 已在首次有效 `presentation.author prepare` 同步调用现有教练模块的表达规划入口，覆盖 observing/active。输入区分用户原话、按钮动作、作者设想和已读材料；输出目的、理解目标、阅读路径、方法确切版本及适配要求。
- 主 Agent 在下一次采样通过既有 `sampling_guidance` 接收一次完整交接，按需读取全文示范；重复 prepare 复用，普通失败不重试，取消/费用停止向上传播，失败响应用量也计入。
- 修复了中途新增 TurnFrozen 导致的请求前缀回退。相关 123 项回归通过，见 `presentation-verified-tests.log`。广检另发现旧音乐 source.present schema 测试期待过时，原快照已相同，本轮未改它。
- A/B 共同入口、作者提示、方法库 rev6 与模型；唯一差别是 A 原教练规划 prompt/schema 映射空 methods，B 明确选型。A 不是未改生产。正式 catalog 保持 rev5。
- 正确二进制 `binaries/author-A.exe` 与 `author-B-fixed.exe`，源码与实际重编译证据 `logs/pair-verification.json`。最初 B 因 Cargo 缓存复用 A，小试两调用后停止，全部保存于 `aborted-build-pilot/`，不计质量样本。
- 中断批保守费用上界 2.23608896 元，其中 2.162688 是缺 usage 的最坏预留，不能称已知实付。初次恢复合同 `experiment/contract-recovery.json` 保留为历史。随后分页小试又支出 1.14066784；正式新题尚未启动时，root为保留原定八次完整对照，将非用户指定的内部活动上限从10修订为15元，累计历史3.37675680、当前可用11.62324320，见 activity-budget.json。
- 恢复 runner session36109 已结束，第二批归档 `paging-pilot/`：22调用、首次交付，但仅读到部分示范，因此gate拒绝，正式新题仍未启动。真正问题是read_method显式length被当成精确范围，next_offset提前为null，作者随后跳过6000..12000；不是宿主clamp。
- 已把read_method length改为页大小，服务和runtime输出预算投影都给实际末端游标及现有NextCall；普通read精确范围保持。5项分页runtime+普通read1项+实际观察1项通过。v2两臂真实重编完成，server所有方法333字符分页重建测试通过，证据logs-v2/pair-verification.json。
- v2试跑第三批已归档network-pilot/，2调用、无交付。真正失败是教练8192总输出中7990为推理，native返回unfinished:length，wire记录finish_reason:length；连接reset为后续现象，不能称瞬时网络故障。该批保守2.24962944元，三批prior合计5.62638624，15元上限内剩9.37361376，不加预算。
- v3最小修正完成：仅新表达规划调用16_384总输出，并按现有DeepSeek结构化压缩路径明确low推理；其他模型None，主作者及后台教练不变。模块9项测试通过。实验A/B同配置，common-source-v3及author-A-plan/B-plan真实重编、逐文件验证完成；证据logs-v3/pair-verification.json。v2恢复合同撤回不再执行。
- contract-v3.json最后pilot已结束，runner96364退出，无活跃付费进程。日志run-v3.log，目录runs/。22calls、首交付presentation-1792658991339127100-14 rev1；规划1次完整返回(2509output/1622reasoning)，实际read_method 5次进入主模型。discrete-update@3卡与README完整，但HTML只7000/25860；另两法仅读卡；首交付method_refs=[]。实际request004 messages20有offset7000续读调用，作者未继续。按冻结门槛gate拒绝，8个heldout全部未启动，没有新题质量对照结论。
- 四批共48calls，活动保守费用上限6.55102280元，其中成功usage峰时估算2.22564680、失败/中断预留4.32537600，不能称全额已知实付。activity-budget.json已settled；未耗尽预算，按读取条件停止，最后pilot不再恢复。
- Reader已实际渲染首交付三种视口，原样页面rendered/pilot-B0。delivery/conclusion.json statusfinal，终版报告已打包至 docs/performance/explanation-evolution/coach-method-handoff-20261022/，以诊断页面/方法覆盖/未启动列表为主体，不展示空白A/B卡。delivery/page-check.json记录1280/320布局及本地链接通过；没有新题独立评审。
- 后续建议尚未实施：由执行层完成选中主方法的连续载入并记录实际消费，保留有理由不采用的路径；不要仅增加提示词。完整示范消费是本次实验条件，不是所有好页面的普遍必要条件。线上/正式catalog仍保持不变。

---

# SESSION_CHECKPOINT_SELF_EVOLUTION — 2026-10-22，CS336 反例迁移实验

## 当前接手：CS336 参数核算反例

用户授权本地沿用 RSI 调查实际方法读取、加入反例与教师示范、用同模型未示范新题做新旧首次交付对照；禁止改线上。当前记录见 [本轮报告](docs/performance/explanation-evolution/cs336-transfer-20261022/README.md)，全部运行证据在 `tmp/rsi-author-transfer-20261022/`。

- 已确认原页看到 revision 4 目录，具体方法读取 0 次，三次 write 与成功 patch 明确 `method_refs: []`。完整页已有前文配对图；本轮处理汇总处与前文图形的对应，不宣称全页没有图。
- 既有案例整理流程已产出 `trace/collected-final/case.json`，真实追问与来源保留；历史修复和产品投票缺失如实标记。
- `skills/presentation/methods/chart-reveal/v3/` 为已验证候选六文件。计数、生产 iframe 保存重开/导出和实际首屏检查通过；正式 catalog 仍为本轮前 revision 5。
- 共同源码和 A/B 可执行文件已冻结，仅 B 注册新候选。合同在 `experiment/contract.json`：两道新题 KV/质检，各 A/B 两次、共八次；自然选择方法、不强制读取、失败不补抽；内部费用上限 10 元。
- 本轮只将 SE8 的固定输出覆盖改为可选，省略时保留正式逐请求上限；A/B 都用同一版本。未激活 active Tutor 审核，不把作者资产实验称作完整教学循环。
- 2026-10-22 13:14 HKT 八次生成全部完成，134 次模型调用，峰时估算 6.67164616 元；全部首次成功交付，具体方法读取 0 次，交付 method_refs 均为空。没有补跑或选择性替换。实际返回模型均为 deepseek-flash。
- 页面已有局部改善，但两组仍存在讲解、图尺度和按钮语义问题，未证明稳定增益；候选正文根本未进入输入，因此不能声称教师示范迁移已生效。正式目录维持 revision 5；本地保留 chart-reveal@3 候选，不发布。
- 八页实际渲染与匿名操作证据、完整结论及对照入口见本轮报告。下一步应按理解目标选择并读取方法，验证真实适配后再做独立新题对照；本批冻结条件内没有修改该选择步骤。当前两题此后作为回归材料。

下方保留旧阶段原记录，其“暂缓”和未调用状态只适用于 2026-10-16 旧批次。

---

## 2026-10-16 历史记录：三维联动模板已登记

## 新鲜度自检

- 写入时最新 commit：402d9be feat: preserve current reader, focus mode and runtime work。
- 本线未提交，其他工作线改动保留；线上发布另看专线 checkpoint。

## 当前状态

用户先要求建立首批模板，随后认为表现力不足，指定参考 Ciechanowski 制作完整齿轮演示。用户已认可作品并明确要求提取三维交互与图解联动。spatial-linked-views@1 已完成并接入正式目录；SE8 继续暂缓。

## 已完成

- catalog revision 4，共九单元：discrete-update@3、其余原七类@2、spatial-linked-views@1；原版目录保留。八新版已完成104组数学/SVG、24次重开/导出、24预览/8交付。
- 同一运行核心生成齿轮和弹簧两个自包含作品；附主例/改写例、卡片、适配说明、Author 元数据、检查与按需可读核心。
- 宿主固定 Three.js 0.180.0，prepare 可选 three，write 可选 libraries:["three"]；版本冻结、读取元数据、修订淘汰依赖遵循既有机制。
- 三维与图解使用单一计算结果；相机/拆解/时间通过原有桥保存，重开暂停，WebGL 像素可静态输出。
- 42 组独立数学及几何、6 次桥重开、6 次实际导出、6 组真实 Author 预览和 2 次交付通过。
- Runtime presentation 110 项、库版本 2 项、SE2 2 项、前端装配 5 项通过。新方法 23 次源文件分片读取进入 25 次受控 Provider 请求。
- 总览九单元在三宽度可用；原八单元首批证据保留在 docs/performance/presentation-method-library-v1。

## 下一步

1. 用户体验 http://127.0.0.1:4197/spatial-linked-views/v1/preview.html?topic=spring ，根据新主题需求改写模型与视图。
2. 后续 Agent 制作遵循 read_method → prepare → write(method_refs/libraries 明确) → 当前候选 preview → deliver。
3. 2026-10-16 16:02 HKT 已随 current-20261016 上线；Linux真实三视口WebGL/Author交付通过，见 docs/performance/current-release-20261016/README.md。

## 边界

- 当前证明共用核心能迁移两个主题；没有 Agent 自然选型、独立迁移收益或读者学习效果的结论。
- SE8 收益实验继续暂缓；本轮无付费模型调用。
- SE1–SE7 原证据、discrete-update@1 和候选清单保持历史身份。
- 原工作树其他专线改动不属于本次；FP7 暂停。

## 冷启动读序

1. skills/presentation/methods/spatial-linked-views/v1/card.md、example/README.md。
2. docs/performance/presentation-spatial-template-v1/README.md、acceptance.json。
3. skills/presentation/methods/README.md、catalog.json、SOURCES.md。
4. crates/runtime/src/presentation_methods.rs、agent_prompt/presentation.rs；crates/server/src/presentation_libraries.rs。
5. docs/adr/0168-executable-explanation-methods-and-evidence-based-evolution.md §8；原首批模板记录。
