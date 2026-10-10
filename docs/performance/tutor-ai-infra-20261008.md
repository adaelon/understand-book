# AI Infra Tutor 补建 — 2026-10-08

## 范围与当前状态

工作区：`E:\allwork\download\agent\lifebook\.understand-book\ai-infra-book-complete`。

用户已确定 Pass2 关闭，并要求保留已完成的 BookStructure，只补 Tutor。2026-10-01 的 BookStructure 关闭回执为 closed、质量门 passed，发布回执 committed，来源身份与当前一致。

已修复计划把生产者算法升级当作缺失工作的复用问题。标准计划以既有发布回执绑定具体 BookStructure；来源或公开文件不匹配时停止复用。未选择复用的路由仍按当前算法执行。

## 验证与引擎

- 回归先红后绿：历史发布回执、缺少当前生产任务时继续复用、进入正式对象、定向读取教学任务、公开文件／来源变化时停止。
- 2 项定向路由测试、69 项计划门禁／计划创建／发布关闭／驱动测试通过，Core 类型检查通过。
- 已安装引擎：`E:\allwork\Understand Book\understand-book-build.exe`。
- 旧版备份：`E:\allwork\Understand Book\understand-book-build.before-tutor-published-reuse-20261008.exe`。
- 打包副本：`tmp/tutor-published-reuse-20261008/understand-book-build-x86_64-pc-windows-msvc.exe`。

## 已确认计划

- 代码返回的计划路径：`E:\allwork\download\agent\lifebook\.understand-book\ai-infra-book-complete\.build\automatic-build\v2\legacy-plans\6ce62b356d3d6995.json`。保持不透明，不手改计划。
- plan_id：`plan-3260d48421cf28cc`。
- plan_digest：`c6f5f1add9a56faf9e72b354d7cb50e31aa258be7eb2ee42b5f34289034060a1`。
- 复用：Pass1、profile_sidecar、BookStructure。
- 新建：formal_objects → cognitive_materials → teaching_publish。
- 排除：Pass2，以及私人 timeline/concept_map/comparison_table/argument_map。
- 预算：未另设总 token／总时长上限；on_exceed=needs_user。
- 正式对象当前已发现 338 个任务，当前估算 1,926,684–2,032,140 tokens。后续归并、认知素材与审阅尚未计入；三路并发时间参考 9.4 小时，低置信度、无匹配历史样本。

## 恢复入口

用户已确认上述具体范围与预算估算。已创建 max_parallel=3 的 invocation：`abinv1_5a49b34acdd7ea4ceb869a8968ae35de935e54719969e21f74b120040dda33f6`。沿同一计划和 invocation 按 understand-book-build 技能的 build.step／build.refill 协议执行，不重新创建计划。已进入正式对象阶段；首批 3 个子代理已启动。根代理所有权账本保存于 `tmp/tutor-published-reuse-20261008/control.json`，只包含不透明引用与生命周期信息。恢复后核对活跃子代理；每次终态用 build.refill 同时释放该子代理的槽位并续派。

2026-10-08 恢复时引擎确认 formal_objects 已接纳 8/338 项。中断的 tutor_013 后续恢复同一原 ref 并成功提交。下一次恢复时已接纳 105/338 项，tutor_158～160 的原连接返回 bootstrap_unavailable，已逐项按协议上报引擎。引擎为第三路发出替换引用，fresh tutor_161 成功提交。tutor_188 返回 committed 后的续派遇到 E 盘 ENOSPC。用户释放空间后完成恢复修复，引擎确认 125/338，剩余当前已发现的 213 项，并返回三个可启动引用。tutor_189、190、191 已分别启动，实际并发恢复为 3。未重新创建计划或 invocation。后续认知素材与教学发布仍等待依赖。仅引擎 DONE 代表整个计划完成。

## 三路并发恢复修复

前两路反复返回已结束的旧引用。已打开而尚未开始生成的会话没有任务租约，其恢复身份不会随租约过期更新；既有 bootstrap_failure 又会因已打开记录而退出，造成续派永久跳过旧引用。

续派将已有 completed_refs 交给 Driver。Driver 只对本次返回、已终态、已打开且未被 live 所有权保留的当前引用尝试恢复；派发层确认当前恢复身份一致、没有活动生成租约后，写入独立的 terminal_session 恢复记录并推进现有控制代次。保持派发槽位、语义尝试次数、已接纳成果与其他 live 子任务。

新增回归先复现 ready_executors 为 0，再验证两项在 open 后中断的会话获得两个新引用、第三项生成保持 live、重复续派保持同一替换引用、恢复后 semantic_attempt 仍为 1。驱动、续派、派发运行共 79 项通过，Core 类型检查通过。新引擎打包于 `tmp/tutor-terminal-recovery-20261008/understand-book-build-x86_64-pc-windows-msvc.exe`，已安装至原路径，旧版备份为 `E:\allwork\Understand Book\understand-book-build.before-terminal-recovery-20261008.exe`。继续同一 invocation 的 build.refill。

磁盘满时进度账本写入失败，已从根代理保存的生命周期状态恢复 `control.json`。旧的可再生成打包副本移至 `C:\Users\Lenovo\.codex\tutor-build-staged-20261008.exe` 保留；原安装引擎及其备份保持在原位置。

磁盘满留下一个长度为 0 的正式对象派发 manifest，导致恢复时 Unexpected EOF。已将该空文件移至 `tmp/tutor-terminal-recovery-20261008/incomplete-dispatch-a1319344454984458ada198cc50a0252cc89fd5bb7f2c7e5a25115022d8edffa.manifest.json` 保留，由同一引擎实例重新生成派发记录；恢复后引擎继续返回正常 SPAWN_EXECUTORS。

此前包含 BookStructure 重建的 `439952305339d1ca.json` 计划已被本次计划替代，不执行。

后续引擎确认 formal_objects 已接纳 148/338 项，剩余 190 项。tutor_216、217、218 已以三个新引用启动，继续 max_parallel=3；认知材料和教学发布仍等待本阶段完成。所有权和历史终态引用持续保存在同一 control.json。

正式对象阶段后续达到 171/338 已接纳，剩余 167 项，超过当前已发现任务的一半。tutor_253、254、255 已以三个新引用启动；持续沿原计划和 invocation 续派。认知材料与教学发布仍等待依赖，整个计划尚未完成。

后续引擎确认 formal_objects 已接纳 200/338 项，剩余 138 项。tutor_292、293、294 已以三个新引用启动，继续 max_parallel=3。认知材料和教学发布仍等待依赖；整个计划尚未 DONE。

后续引擎确认 formal_objects 已接纳 240/338 项，剩余 98 项。tutor_366、367、368 已以三个新引用启动，继续 max_parallel=3。认知材料和教学发布仍等待依赖；整个计划尚未 DONE。

正式对象后续达到 252/338 已接纳，剩余 86 项；tutor_411、412、413 三路仍在运行。根账本累计历史终态引用超过 Windows 命令行长度上限后，改用文件工具直接写 control.json，并将纯生命周期 refill 请求存入同目录 refill-request.json 后由短命令送入已安装引擎的 stdin。请求字段、历史终态引用、所有权、同一 invocation 均保留；恢复后的 build.refill 已返回正常结构化结果。

### 释放空间后的恢复进度及重试边界

沿用原已确认计划及调用，在三路执行下推进至 formal_objects 259/338 已接纳，79 项尚未完成。Pass1、profile_sidecar、BookStructure 均保持 complete；Pass2 关闭；cognitive_materials 与 teaching_publish 仍 awaiting_dependencies。

引擎返回 NEEDS_USER：reason=retry_exhausted，message="Semantic retries are exhausted and require explicit recovery."；projection={category:internal, code:multiple_failure_causes, required_recovery:forward_fix, stage:formal_objects, work_unit_count:3}。边界涉及 3 项，不能把 3 项误读为全部剩余工作。唯一选择 retry_current（Validate recovery and retry），要求完成必要恢复动作后重新读取持久状态。

request_id=abreq1_063061adec5a6325616183ef6be5a113de76006b5d69827ac6dd179b340c9445。未提交恢复选择，未重新创建计划或调用，未继续盲目重试。

控制状态保存在 tmp/tutor-published-reuse-20261008/control.json，保留完整 completed_refs、live_by_slot、pending_failures，并保存 last_user_request 与 pending_terminal_children。tutor_445、tutor_446 的终止通知在此 NEEDS_USER 边界尚未交给下一次 refill；恢复时必须先消费这两条通知，不得作为仍在运行的孩子，也不得丢失原引用。

用户授权重试后，向同一调用提交 retry_current（request_id=abreq1_063061adec5a6325616183ef6be5a113de76006b5d69827ac6dd179b340c9445），同时消费 tutor_445 与 tutor_446 的终止通知。引擎返回 NEEDS_USER(reason=recovery_not_satisfied)，message="The bound terminal state does not satisfy same-scope retry recovery."；projection 仍为 internal / multiple_failure_causes / forward_fix / formal_objects / work_unit_count=3。完成数保持 259/338，剩余 79。引擎尚未允许再次执行；必须先完成 forward_fix。已保存新的用户边界及空的 live_by_slot；completed_refs 保留全部历史终止引用，pending_failures 未丢失。

### 259/338 重试阻塞的根因诊断

诊断依据为当前请求绑定的三个工作单元的 failure.json、metrics.json、validation.json 及对应实现；未读取书籍语义输入或候选正文，未修改构建状态。

| 工作单元 | 第一次失败 | 第二次失败 | 第三次失败 |
| --- | --- | --- | --- |
| objects-3-fragment-0-418d8d9911423a68 | composite object requires components | candidate_request_too_large | candidate_request_too_large |
| objects-3-fragment-1-3ee27cd4d42e248d | composite object requires components | composite object requires components | composite object requires components |
| objects-3-fragment-2-37164636d26352fe | composite object requires components | learnable relation requires participants and roles | candidate_request_too_large |

**内容校验与纠错脱节。** teaching-map.ts 中 ObjectProposalZ 允许 component_keys、participants 为空；acceptFormalObjects 后续要求 composite 至少一个组成对象、relation 至少两个参与者。agents/formal-objects-extractor.md 列出字段，但未明确写出这两个按 kind 区分的最小数量约束。teaching-build.ts 的 catch 将这些失败统一标成 schema/semantic_output_invalid/artifact_writer，只有 expected 消息而无 json_pointer。extractor-contract.ts 的 isAutomaticBuildCorrectableCandidateFailure 仅接受 schema_invalid 且同时有 json_pointer、expected，因此实际错误不进入现有候选纠错路径；automatic-build-task-store.ts 的 readAutomaticBuildCandidateRetryFeedback 也不向后续尝试回传这类错误。三次失败后被映射为 publish_new_policy，而不是 authorize_candidate_retry。

**输出容量与失败恢复脱节。** 教学任务的模型预算预留 4096 token 输出；Codex 提交通道上限为整个请求 2048 个估算 token、32768 字节。代码扣除请求封装后，候选净额实际只有 1981 个估算 token、32501 字节。fragment-0 与 fragment-2 第三次失败发生在 generation，writer_started=false，候选还未落盘；超限直接终结该次尝试，映射为 operator_fix。现存失败收据未保留超限测量值，无法判断各次是 token 上限还是字节上限先触发，不能报告其确切超额大小。

**重试被拒绝的直接原因。** 当前请求绑定 operator_fix / publish_new_policy / operator_fix 三种恢复要求。prepareRetryRecoveries 只允许 authorize_transient_retry 或 authorize_candidate_retry 打开同范围重试窗口，故 retry_current 确定性返回 recovery_not_satisfied。multiple_failure_causes 是三项诊断的汇总，不是额外故障。

最小诊断复现 tmp/tutor-published-reuse-20261008/diagnose-root-cause.ts 已运行通过：现有有效测试夹具正常接纳；清空 composite 的 component_keys 或 relation 的 participants 都能通过结构 schema、随后被 writer 以实际记录中的同一错误拒绝；读取真实末次失败诊断验证三项均不允许候选纠错，并计算出上述真实传输限额。未改生产代码或开启新重试。

修复落点：对这两种明确字段错误返回带字段位置的可纠正 schema_invalid 并补齐生成约束；使任务输出规模适配传输预算，或在保持相同 schema 与证据要求下为超限提供有界缩减重提路径；通过现有恢复机制接续失败任务，保留已接纳成果。

### 2026-10-09 候选纠错修复

用户已授权修复并恢复构建。保持原计划、输入、提取提示词及策略身份，保留已接纳成果。新字段错误由 teaching-map 的 checkObjectCardinality 产生带 JSON pointer 的 schema_invalid；对象对齐复用同一校验。teaching-build 保留 ExtractorContractError，避免重新包装成不可纠正的泛化错误。

extractor-contract 的 automaticBuildCandidateCorrection 统一给出重试纠错反馈：现有字段错误；已持久化的两个明确 Tutor cardinality 错误；生成阶段的 candidate_request_too_large。其他未知语义错误仍保持原恢复边界。task-store 在同范围后续尝试中转交反馈。超限失败现在记录测得的 token/字节数、实际请求上限和保持必需字段、覆盖及证据的缩减要求；保持原传输限制及有界尝试次数。

恢复判定按当前引擎对同一失败的分类重新计算，原始失败收据保持不变。既有 Driver 已允许修复后恢复建议变化而保持失败身份，无需修改计划或手改失败记录。定向测试先出现 5 个预期失败，修复后字段纠错、旧诊断恢复分类、超限反馈到下一次生成并提交均通过；Core 类型检查及 529 模块编译通过。相关回归仍在运行，教学重放部分触及默认 5 秒超时，待单独提高测试时限复跑。

修复验证完成：相关 8 个测试文件共 176 项；初次 170 项通过、5 项教学重放达到默认 5 秒时限、1 项新增旧诊断测试夹具没有按真实教学 writer 方式构造错误。5 项重放改为测试调用时使用 60 秒时限后全部通过；夹具改为保留真实旧 failure_diagnostic 后，3 种旧恢复请求用例全部通过。Core 类型检查与限定文件 diff 检查通过，未放宽生产校验或传输限制。

修复版已编译到 tmp/tutor-candidate-recovery-20261009/understand-book-build-x86_64-pc-windows-msvc.exe 并安装至 E:\allwork\Understand Book\understand-book-build.exe（106876416 字节）。安装前版本保留为 E:\allwork\Understand Book\understand-book-build.before-candidate-recovery-20261009.exe（106874368 字节）。下一步沿原 invocation 提交已授权 retry_current，依据引擎投影续派。

安装后原 request_id 的 retry_current 已获引擎接受，返回 SPAWN_EXECUTORS；原 formal_objects 259/338 成果仍在，剩余 79。tutor_447、448、449 三个新的专用执行子代理已启动，原计划与 invocation 继续，BookStructure 已完成、Pass2 关闭。恢复边界已解除，但完整预构建尚未 DONE。

### 2026-10-09 实跑验证与汇总恢复提示修复

本轮调度选择了另外三个待处理任务，三个子代理均已结束，未新增接纳结果。新失败收据证明安装版纠错信息已生效：objects-3-fragment-4 的请求测得 2052 token / 7725 字节；objects-4-fragment-2 为 2258 token / 9032 字节，均超过原 2048 token 请求上限；objects-4-fragment-0 返回 schema_invalid，定位 /objects/6/component_keys，并明确要求 composite 至少一个组成对象。三项底层恢复要求均为 authorize_candidate_retry。

实跑同时发现 Driver 汇总错误：不同 code 或 phase 的可纠正失败被合并成 multiple_failure_causes，随后仅按这个泛化诊断生成 forward_fix，丢失了各项均可纠正这一事实。failureProjectionFor 现从原始诊断判断全部任务是否支持候选纠错；全部支持时投影 confirm_candidate_retry。混有真正需要人工修复的错误仍保持 forward_fix；即使错误 code 相同、恢复要求不同，也不再用第一项掩盖其余任务。

新增三项汇总回归全部通过，覆盖全部可纠正、不同 code 混有人工修复、相同 code 混有人工修复。结合前述回归，共 179 项相关用例分批通过；Core 类型检查、限定修改文件 diff 检查及 529 模块编译通过。

最终安装版为 E:\allwork\Understand Book\understand-book-build.exe，106876928 字节；上一候选纠错版保留为 understand-book-build.before-candidate-projection-20261009.exe。使用最终安装版沿原 invocation 调用 build.refill，消费 tutor_448、449 的待处理终止通知，未提交新的重试选择、未派发新任务。返回的恢复投影已验证为 confirm_candidate_retry。

当前状态：formal_objects 259/338 已接纳、79 待处理；BookStructure complete；Pass2 关闭；cognitive_materials 与 teaching_publish 等待依赖。控制记录 live_by_slot 为空，pending_terminal_children 为空，保留全部历史 completed_refs。

当前边界 request_id=abreq1_17b205a5ae8240bf152c168d7dc7d224a3ac2936e2bacc58a5955ca2e0dd1f96；reason=retry_exhausted；message="Semantic retries are exhausted and require explicit recovery."；projection={category:internal, code:multiple_failure_causes, required_recovery:confirm_candidate_retry, stage:formal_objects, work_unit_count:3}。唯一选择 retry_current，label="Validate recovery and retry"，consequence="Re-read durable state after the required recovery action is complete."。此选择将开启当前 schema 下另一个有界纠正窗口；按 Build skill 在此 NEEDS_USER 边界等待用户选择。

### 2026-10-09 继续纠正重试：260/338

用户选择继续，沿原 invocation 提交上一边界的 retry_current。引擎接受恢复并返回三个新引用，由 tutor_450、451、452 三个专用代理并发执行。tutor_450 返回 committed，下一次 refill 确认 formal_objects 已接纳数从 259 增至 260，剩余 78/338；tutor_451、452 返回 retryable_failure。

当前新边界 request_id=abreq1_f29da96f6d4ee8174f718f785949141035a0f0aa7c486e5fc8881b78fdc9a5e7；reason=retry_exhausted；message="Semantic retries are exhausted and require explicit recovery."；projection={category:schema, code:schema_invalid, phase:artifact_writer, required_recovery:confirm_candidate_retry, stage:formal_objects, work_unit_count:2}。唯一选择 retry_current，label="Validate recovery and retry"，consequence="Re-read durable state after the required recovery action is complete."。未提交新边界的选择。

BookStructure 仍 complete，Pass2 关闭，cognitive_materials 和 teaching_publish 等待依赖，尚未 DONE。control.json 保留全部 completed_refs，以及 tutor_451、452 的 pre-release 归属和 pending_terminal_children；它们实际已终止，下一次获用户选择后须用 build.refill 消费终止通知再按返回投影续派。

### 2026-10-09 再次继续：本轮无新增接纳

用户再次选择继续，沿原 invocation 提交上一 request_id 的 retry_current，并用 build.refill 消费 tutor_451、452 的终止通知。引擎接受恢复，返回三个新引用，由 tutor_453、454、455 三个专用代理并发执行。三者均返回 retryable_failure。下一次 refill 确认 formal_objects 仍为 260/338 已接纳、78 待处理，本轮无新增接纳。

当前边界 request_id=abreq1_936a0d906eb42e0cae17470c4306d69819fbecb3d15f1dbf82929b4f693a776d；reason=retry_exhausted；message="Semantic retries are exhausted and require explicit recovery."；projection={category:schema, code:schema_invalid, phase:artifact_writer, required_recovery:confirm_candidate_retry, stage:formal_objects, work_unit_count:3}。唯一选择 retry_current，label="Validate recovery and retry"，consequence="Re-read durable state after the required recovery action is complete."。未提交此新边界的选择。BookStructure complete、Pass2 关闭；后续两阶段等待依赖，尚未 DONE。

control.json 已保存当前边界、全部历史 completed_refs，以及实际已终止的 tutor_454、455 的 pre-release 归属与 pending_terminal_children。下一次获用户选择后须先通过 build.refill 消费这两条终止通知，并按返回投影续派。

### 2026-10-09 重试推进至 262/338

用户授权重试，沿原 invocation 提交上一边界的 retry_current，同时通过 build.refill 消费 tutor_454、455 的终止通知。引擎接受恢复，返回三个新引用，由 tutor_456、457、458 三个专用代理并发执行。tutor_456、458 返回 committed，tutor_457 返回 retryable_failure；下一次 refill 确认 formal_objects 已接纳数从 260 增至 262，剩余 76/338。

当前边界 request_id=abreq1_ab8e83a2c259cd3dba3bb8cb08a53a5e30d7e258d44b9e2c23b02d65859dffc9；reason=retry_exhausted；message="Semantic retries are exhausted and require explicit recovery."；projection={category:schema, code:schema_invalid, phase:artifact_writer, required_recovery:confirm_candidate_retry, stage:formal_objects, work_unit_count:1}。唯一选择 retry_current，label="Validate recovery and retry"，consequence="Re-read durable state after the required recovery action is complete."。未提交此新边界的选择。BookStructure complete、Pass2 关闭；后续认知材料与教学发布阶段等待依赖，尚未 DONE。

control.json 保留全部历史 completed_refs，以及实际已终止的 tutor_458、457 的 pre-release 归属与 pending_terminal_children。下一次获用户选择后须先通过 build.refill 消费这两条终止通知，并按返回投影续派。

### 2026-10-09 继续推进至 263/338

用户选择继续，沿原 invocation 提交上一边界的 retry_current，并通过 build.refill 消费 tutor_458、457 的终止通知。引擎接受恢复，返回三个新引用，由 tutor_459、460、461 三个专用代理并发执行。tutor_461 返回 committed；tutor_459、460 返回 retryable_failure。下一次 refill 确认 formal_objects 已接纳数从 262 增至 263，剩余 75/338。

当前边界 request_id=abreq1_0a7cb1064164cc129c69a0b22b5ffeec108a6f0c1a3c33da03e15a5efd80d917；reason=retry_exhausted；message="Semantic retries are exhausted and require explicit recovery."；projection={category:internal, code:multiple_failure_causes, required_recovery:confirm_candidate_retry, stage:formal_objects, work_unit_count:2}。唯一选择 retry_current，label="Validate recovery and retry"，consequence="Re-read durable state after the required recovery action is complete."。未提交此新边界的选择。BookStructure complete、Pass2 关闭；后续认知材料与教学发布阶段等待依赖，尚未 DONE。

control.json 保留全部历史 completed_refs，以及实际已终止的 tutor_461、459 的 pre-release 归属与 pending_terminal_children。下一次获用户选择后须先通过 build.refill 消费这两条终止通知，并按返回投影续派。

### 2026-10-09 用户授权自动重试

用户新规则：“不用问了，自己重试即可”。此授权覆盖后续当前计划内的 retry_current 纠正重试边界；不再逐轮请求确认。用户指令优先于 Build skill 的逐轮 NEEDS_USER 选择规则。仍通过结构化 request_id + choice_id 开启每个有界重试窗口，保持原计划、原 invocation、3 并发、BookStructure 已完成、Pass2 关闭和已接纳成果。control.json 保存 auto_retry_current=true 供续接使用。

自动续跑首批进展：tutor_462 至 tutor_467 均 retryable_failure，两轮没有新增接纳；对应可纠正边界已按授权自动提交 retry_current。tutor_468 至 tutor_470 中一项被接纳，进度到 264/338；tutor_471 至 tutor_473 中又一项被接纳，进度到 265/338，剩余 73。当前两项 candidate_request_too_large / generation 边界仍为 confirm_candidate_retry，已自动提交恢复。

对连续无新增的两轮做了限定诊断：仅查看请求绑定身份及三个 objects-6 工作单元的失败诊断元数据，未读取语义输入或候选正文。末次分别为实际测量输出超限（2105 估算 token / 7951 字节，原上限 2048 token / 32768 字节）、关系参与者不足（/objects/5/participants）、复合对象组成不足（/objects/1/component_keys）。三项均已有明确反馈、底层恢复为 authorize_candidate_retry；未发现需要改变恢复规则的新故障。

tutor_474 至 tutor_476 中两项被接纳，进度到 267/338，剩余 71。重试请求产生时其余两个任务尚未接纳，随后预算证据中的 remaining.work_units 从 73 变为 71、dispatch_plan_digest 改变，旧 retry_current 返回 plan_changed 且 choices=[]。限定比较两个边界的 state：build_plan_digest 和 descriptor_plan_digest 均相同，权威计划并未变化。通过不带决定的 build.refill 消费剩余终止通知并读取当前状态，得到新的 retry_exhausted / confirm_candidate_retry 请求（abreq1_54eb4392a934d579a4912a9c183f1d86c09cad35262f39fc297fe9402bcea414），已按自动重试授权提交。未修改计划或跳过预算判断。

自动续跑进展到 270/338：tutor_477 至 479 接纳一项（268/338）；tutor_480 至 482 无新增；tutor_483 至 485 接纳两项，最终回读为 270/338、剩余 68。本轮晚到的提交结果在自动选择前通过新的 build.refill 消费，获得绑定当前剩余工作量的 retry_current 请求，避免使用旧预测边界。随后已自动授权并启动 tutor_486、487、488 三个新的专用执行代理。仍沿用原计划、原 invocation、3 并发，BookStructure complete、Pass2 关闭，后续两阶段等待依赖。

自动续跑进展到 273/338：tutor_486 至 488 无新增；tutor_489 至 491 接纳一项（271/338）；tutor_492 至 494 接纳两项（273/338）。本轮单项输出超限的有界重试请求已自动提交并获接受，tutor_495、496、497 三个新专用代理已启动。当前剩余 65 项，尚未进入后续认知材料或教学发布阶段。

自动续跑进展到 277/338：tutor_495 至 497 接纳两项（275/338）；tutor_498 至 500 再接纳两项（277/338）。单项输出超限的恢复请求均已自动提交并获接受，tutor_501、502、503 三个新专用代理已启动。当前正式对象剩余 61 项，其余计划范围保持不变。

tutor_501 至 tutor_509 三轮无新增接纳，均已按授权自动恢复。限定诊断比较这三轮边界绑定身份：涉及九个不同 objects-11 / objects-12 片段的第三次语义尝试；失败仍为实际输出超限、关系参与者不足、复合对象组成不足，均已有明确纠正反馈。未把连续无新增误判成同三个工作单元的重复死循环。当前 277/338、剩余 61，tutor_510、511、512 三个新专用代理已启动。

tutor_510 至 512 无新增，已自动恢复；tutor_513 至 515 接纳一项，进度到 278/338，剩余 60。两项可纠正的恢复请求已自动提交并获接受，tutor_516、517、518 三个新专用代理已启动。自动重试授权继续有效，原计划和调用保持不变。

tutor_516 至 518 接纳一项（279/338），tutor_519 至 521 无新增；各次可纠正恢复均已自动提交并获接受。下一次引擎只返回一个 ready executor，已启动 tutor_522；并发上限仍为 3，按引擎实际投影派发，未制造额外任务。正式对象当前 279/338 已接纳、剩余 59；后续两阶段等待依赖。

### 2026-10-09 合并正式对象纠正要求

tutor_522、523 至 525、526 至 528 均无新增接纳，当前正式对象 279/338。限定检查失败元数据发现，原三个 objects-3 片段的第四次尝试在输出超限与组成对象缺失之间切换；同一任务上一条反馈确实被读取，没有跨作用域丢失。反馈仅强调最近错误，使缩减输出与保持必要组成信息容易交替失败。

readAutomaticBuildCandidateRetryFeedback 现在仅在 formal_objects 阶段的可纠正反馈后，同时提醒 composite.component_keys 至少一项、relation.participants 至少两个有角色的本地对象键，以及包含请求外壳的 token/字节上限。保留原具体 code、pointer 与测量说明；其他阶段、未知错误、作用域隔离和重试次数规则不变。未改提示词资产、计划或已接纳成果。

新增真实正式对象租约重试测试先复现旧反馈缺少 component_keys 要求，修改后通过；连同两个字段纠正测试共 3 项通过。任务存储 12 项通过，执行器字段反馈与超限反馈的两个端到端用例通过，Core 类型检查与限定 diff 检查通过。中止无关的完整执行器测试运行，改为上述两个相关用例。编译 529 模块并安装新引擎，旧引擎保留为 E:\allwork\Understand Book\understand-book-build.before-combined-feedback-20261009.exe。

继续沿原 invocation 自动提交当前 retry_current，同时消费 tutor_527、528 的终止通知；自动重试授权持续有效。
合并反馈修正后的首轮 tutor_529 至 531 接纳两项，进度 281/338、剩余 57；单项实际输出超限恢复已自动提交并接受，tutor_532 至 534 三个新专用代理已启动。
tutor_532 至 534 接纳两项（283/338）；tutor_535 至 537 接纳一项（284/338、剩余 54）。实际输出超限的恢复请求均已自动提交并接受，tutor_538 至 540 三个新专用代理已启动。
tutor_538 至 540 接纳一项（285/338）；tutor_541 至 543 全部接纳，进度到 288/338、剩余 50。消费晚到终止通知后获得三个 ready executor，tutor_544 至 546 已并发启动。
tutor_544 至 546 接纳两项（290/338）；tutor_547 至 549 再接纳两项（292/338、剩余 46）。超限边界自动恢复均接受，tutor_550 至 552 已并发启动。
tutor_550 至 552 无新增，自动恢复；tutor_553 至 555 接纳两项，进度 294/338、剩余 44。单项超限恢复已自动提交并接受，tutor_556 至 558 已启动。
tutor_556 至 558 接纳两项（296/338）；tutor_559 至 561 接纳一项（297/338、剩余 41）。超限恢复自动接受，tutor_562 至 564 已并发启动。
tutor_562 至 564 接纳一项（298/338）；tutor_565、566 均接纳，进度 300/338、剩余 38。tutor_567 返回精确 interrupted/bootstrap/bootstrap_unavailable；向 refill 发送该 owned ref 的 bootstrap_failure，引擎仅更新启动恢复引用，同槽由新 tutor_568 执行。已消费 tutor_565、566 的终止通知，补充 tutor_569、570，保持最多三个并发。
tutor_568 超限，自动恢复；tutor_569、570 接纳两项（302/338）；tutor_571 至 573 接纳两项，先消费晚到接纳通知并获得当前边界后自动恢复，进度 304/338、剩余 34。tutor_574 至 576 已启动。
tutor_574 至 576 接纳两项（306/338）；tutor_577 至 579 接纳两项（308/338、剩余 30）。超限恢复已自动接受，tutor_580 至 582 已并发启动。
tutor_580 至 582 接纳两项（310/338）；tutor_583 至 585 接纳一项（311/338、剩余 27）。超限恢复自动接受，tutor_586 至 588 已并发启动。
tutor_586 至 588 接纳一项（312/338）；tutor_589 至 591 接纳两项（314/338、剩余 24）。超限恢复自动接受，tutor_592 至 594 已并发启动。
tutor_592 至 594、tutor_595 至 597 连续两轮无新增，全部为实际输出超限；各次恢复均自动提交并接受，tutor_598 至 600 已启动。当前 314/338、剩余 24。
tutor_598 至 600 接纳一项（315/338）；tutor_601 至 603 接纳两项（317/338、剩余 21）。超限恢复自动接受，tutor_604 至 606 已启动。
tutor_604 至 606 全部接纳，正式对象 320/338、剩余 18。消费全部终止通知后，tutor_607 至 609 三个新专用代理已启动。
tutor_607 至 609 接纳两项（322/338、剩余 16）；引擎下一次仅签发一个可执行引用，tutor_610 超限，自动恢复后已签发三个引用，由 tutor_611 至 613 并发执行。
tutor_611 至 613 接纳一项（323/338）；tutor_614 至 616 接纳一项（324/338、剩余 14）。超限恢复自动接受，tutor_617 至 619 已并发启动。
tutor_617 至 619 接纳两项（326/338）；tutor_620 至 622 接纳两项（328/338、剩余 10）。超限恢复自动接受，tutor_623 至 625 已启动。
tutor_623 至 625 接纳一项（329/338）；自动恢复后仅签发一个引用，tutor_626 接纳（330/338、剩余 8）。下一步签发三个引用，由 tutor_627 至 629 并发执行。
tutor_627 至 629 接纳一项（331/338）；tutor_630 至 632 接纳两项（333/338、剩余 5）。超限恢复自动接受，本次引擎签发两个引用，tutor_633、634 已并发启动。
tutor_633、634 接纳一项（334/338、剩余 4）。超限恢复自动接受，tutor_635 至 637 已并发启动。
tutor_635 至 637 接纳一项（335/338、剩余 3）；下一次仅签发一个引用，tutor_638 超限，自动恢复后由 tutor_639 至 641 三个新代理执行剩余三项。
tutor_639 至 641 接纳一项，正式对象 336/338、剩余 2。两项超限恢复自动接受，tutor_642、643 已并发启动。
tutor_642、643 接纳一项，正式对象 337/338、剩余 1。最后一项超限恢复已自动接受，tutor_644 已启动。
tutor_644 超限，自动恢复；限定检查最后一个 objects-5-fragment-0 工作单元第 6 至 11 次失败诊断，实际 request token 从 2680 降至 2070 后反复到 2108、2413，反馈均保留测量和上限。tutor_645 第 12 次语义尝试接纳，原 338 个任务全部完成。引擎随后签发一个依赖后续任务，计数变为 338/339，formal_objects 仍 pending；tutor_646 已启动，未宣称阶段完成。
原 338 个任务完成后，进入动态顺序签发的跨块对象对齐。tutor_646、647 各接纳一步，进度 340/341；tutor_648 已启动。用户询问剩余数量：现有 build_progress 仅 scope=discovered，不包含全对齐对象及后续认知材料/发布检查的完整总数，已明确说明原 338 不是全流程总数，未用 pending=1 冒充全流程只剩一项。
为回答用户的剩余数量问题，补充只读 object_alignment 计数投影，直接使用引擎已加载的最新对齐输入统计总候选、已解决候选、剩余候选；仅返回三个数值，不返回语义正文/任务身份，不改任务、策略或计划。新增回归覆盖动态单任务与剩余候选数量的区别、最新输入和已移除键；6 个进度投影测试通过，Core 类型检查和限定 diff 检查通过。编译并安装，旧引擎保留为 understand-book-build.before-alignment-progress-20261009.exe。tutor_648 接纳第 3 步，tutor_649 正在执行下一步。
新计数首次回读：formal_objects.object_alignment={total_objects:1266,resolved_objects:0,remaining_objects:1266}。已接纳 4 个对齐中间动作，尚未完成首个对象；此前 pending=1 仅代表当前可执行动作。已向用户明确报告还剩 1266 个候选对象需对齐，后续认知材料与发布检查数量尚未展开。tutor_650 已启动，自动重试授权继续有效。
用户询问对齐是否可并发。检查当前代码：alignmentFocus 选择第一个未解决对象，routeTeachingBuildStages 仅签发下一步，resolve 必须覆盖当前 focus 且会全局改写引用/重定向；没有并行准备与统一提交接口。已说明直接把执行器数量调到 3 不能分摊对象，需要执行链路改造。目前 tutor_650、651 均接纳中间动作，剩余对象 1266；tutor_652 正在执行。
tutor_652 接纳第 7 个对齐中间动作，计数 345/346，仍 remaining_objects=1266。tutor_653 retryable_failure 后，refill 在原额度内签发纠正引用；tutor_654 已启动。

### 2026-10-09 对齐读取范围纠正

tutor_653、654、655 三次失败均是 align-v3-0-7 工作单元请求读取超出 lid=3.2 原文长度 138 的范围。诊断为 schema/semantic_output_invalid/artifact_writer，未作为可纠正反馈传入后续尝试，第三次边界要求 publish_new_policy_scope。

advanceObjectAlignment 现对无效 read 范围发出 ExtractorContractError(schema_invalid, /end)，明确 source_length_utf16、最大跨度 2000 和实际 start；未知 LID 指向 /lid。原范围和证据门禁不变。automaticBuildCandidateCorrection 仅识别已保存的该格式范围失败，把实际长度提供为 /end 纠正信息；未知语义错误仍走策略恢复。旧回执原样保留。

两个回归先复现缺少结构化反馈及旧失败不可恢复，修正后对象对齐 16 项、Tutor 恢复 5 项共 21 项通过，Core 类型检查与限定 diff 检查通过。编译安装，旧引擎保留为 understand-book-build.before-alignment-range-feedback-20261009.exe。安装后先不带决定回读当前边界，再根据新的纠正投影自动恢复。
新引擎回读边界为 confirm_candidate_retry（abreq1_6ccfad000cf9dd89c54903b7df5f97cbdf92180f59c1ab8549eb7ae681077cbc），自动选择已接受。tutor_656 接纳该范围纠正，第 8 个对齐中间动作完成，剩余对象仍 1266；tutor_657 已启动。
tutor_657 接纳第一个对象对齐判定并合并重复候选；object_alignment={total_objects:1265,resolved_objects:1,remaining_objects:1264}，动作计数 347/348。tutor_658 已启动。
tutor_658、659 各接纳一个中间动作，计数 349/350，object_alignment 仍 total=1265,resolved=1,remaining=1264；tutor_660 已启动。
tutor_660、661 各接纳一个中间动作，计数 351/352，仍已对齐 1、剩余 1264；tutor_662 已启动。

### 2026-10-09 对齐三路并发

用户要求对齐并发，保留 max_parallel=3、Pass2 关闭及自动重试授权。tutor_662 至 664 继续接纳旧串行动作，计数 354/355；tutor_665 执行已签发的旧动作。没有清空片段、旧任务、回执或计划。

调度改为每轮冻结三个不同候选的分支账本，分别推进查找、检视、原文读取；三个 resolve 全部接纳后按固定分支顺序归并到共享提案。直接比较所选、已检视及对象引用依赖；前一个合并修改依赖时，保留该分支不可变回执，未解决对象下一轮重新判定。旧串行已签发任务及失效动作的 -replay 替代先沿原位置续跑，再从下一个未签发动作进入并发。身份归属与 finish 继续沿原门禁完成。

公开进度使用归并后的共享账本数值，不使用某个并行分支的冻结输入。提取提示、候选格式、策略身份及语义输入预算保持不变。

新三路调度、兄弟输入稳定、旧串行读取续接、完整 formal_objects 发布及身份复用、三路检索和显式查询局部刷新已通过；对象对齐及进度投影共 24 项通过。旧回执重放回归发现失效串行动作误转并行的分支条件，已修正为保留原已签发位置；完整教学构建回归正在复验。磁盘回放案例使用 120 秒测试时限；原 5 秒时限下部分案例仅因耗时超限失败。

完整 teaching-build 25 项回归通过，其中包括原回执门禁、失效动作独立替代、检索供应商变更、缓存丢失后的身份恢复、Pass2 开/关发布、来源审阅及局部修复。加上对象对齐 17 项和进度投影 7 项，共 49 项通过；Core 类型检查与限定 diff 检查通过。并发版编译安装，旧引擎保留为 understand-book-build.before-parallel-alignment-20261009.exe；tutor_665 接纳后使用同一 invocation 继续回读，自动重试授权仍保留。

同一 invocation 首次实际签发三个 alignment public slots：formal_objects committed=355,total=358,pending=3，共享 object_alignment 仍 total=1265,resolved=1,remaining=1264。tutor_666、667、668 三个全新专用执行器已并发启动，旧 338 个片段及 17 个串行对齐动作保留。

tutor_666、667、668 均接纳各自的一个中间动作，committed=358,total=361,pending=3。先通过首次完成 refill，再消费该调用期间到达的两项终止通知，按最新容量重新签发；tutor_669、670、671 三路继续推进，未重复占用或遗漏兄弟槽位。

tutor_669、670、671 均接纳下一中间动作，committed=361,total=364,pending=3；tutor_672、673、674 三路继续推进。共享候选 remaining=1264，未把中间动作接纳当成对象解决。

tutor_672、673、674 均接纳下一中间动作，committed=364,total=367,pending=3；tutor_675、676、677 三路继续推进，remaining=1264。

tutor_675、677 接纳，tutor_676 为 retryable_failure。引擎原额度内签发重试并保留另外两路，committed=366,total=369,pending=3；tutor_678、679、680 已启动，其中 679 为失败槽位的全新专用执行器。自动重试没有要求用户确认。

六个已有大文件/多步骤磁盘回放用例显式采用 120 秒用例时限，保持所有断言及生产预算不变；完整回归实测这些用例为 6 至 45 秒，原默认 5 秒会产生超时失败。单独复验最长的累计读取/不可变回执用例，确保普通测试入口也采用该时限，而非依赖临时命令行参数。

最长累计读取/不可变回执用例在普通测试入口通过，实测约 55 秒。tutor_678、679、680 均 committed，首次并行失败在一次重试后已接纳；终止通知一起进入下一次 refill。

引擎确认上述三项接纳，committed=369,total=372,pending=3；tutor_681、682、683 三路继续推进，remaining=1264。

tutor_681、682、683 返回 committed 后，首次回读在 freezeTeachingTask 写入时报 ENOSPC。请求 abreq1_359de50705811075312c675c5291a03b56fc2d5a82acff88700ce6510f4c5c3d 为 build_engine_failed，未把此次调用当成成功接纳投影。维护诊断确认 E 盘空间不足；检查时 E 可用 116,375,552 字节、C 可用约 1.44 GB。仅取文件大小元数据：教学 frozen_task 373 份约 209.9 MB、writer_context 34 份约 163.4 MB；没有读取语义正文。E 空间重新出现后，保留 invocation、已接纳成果及两项待消费终止通知，自动重新回读。

Root 状态投影保存逻辑同时修正：内部错误响应没有 build_progress 时保存 null，防止 undefined 的保存异常掩盖真实引擎失败。

自动重试回读请求 abreq1_b690e14e9ae7ddc60a7a159e19bae3c9f56d2110b811c67772acb73ac306b238 仍为 build_engine_failed，但诊断已变为 freezeTeachingTask 解析旧任务的 Unexpected EOF：此前 ENOSPC 留下了未完整写入的 frozen task。检查时 E 已恢复约 4.28 GB；这次剩余障碍为未接纳派生文件的截断。所有终止通知已通过 refill 消费，live_by_slot 为空。

修复 freezeTeachingTask：读取到损坏 JSON 时只重建尚无语义接纳产物的派生任务，已有接纳产物则保留原记录并报错；首次 create-only 写入的 ENOSPC 删除本次半截文件；可变 current writer context 先写临时文件再替换，失败保留上一份完整上下文。没有改动语义产物或策略。

先复现未接纳截断文件恢复失败，再修正；两个磁盘故障回归通过，其中真实写入半截文件后注入 ENOSPC，验证清理新任务、保留前一份上下文及不触碰已接纳任务。Core 类型检查通过；正在复验并发、旧串行续跑和旧回执重放路径。

上述四条既有路径通过，限定 diff 检查通过。磁盘写入恢复版编译安装，旧并发引擎保留为 understand-book-build.before-task-write-recovery-20261009.exe；同一 invocation 自动回读，无待消费终止通知、无活跃执行器，没有修改计划或已接纳产物。

恢复后的引擎正式确认 committed=372,total=374,pending=2，并签发两个后续动作。三路冻结轮次中已有一条 resolve 被接纳，等待另外两条完成后按次序归并；共享 remaining=1264。tutor_684、685 已启动。截断文件由程序恢复，无人工重写任务、回执或候选。

tutor_684、685 接纳两个中间动作，committed=374,total=376,pending=2；tutor_686、687 继续本轮剩余两路，remaining=1264。

引擎确认 tutor_686、687 接纳，committed=376,total=378,pending=2；tutor_688、689 已并行启动。本轮已有一支完成，剩余两支继续，归并后恢复最多三支。

引擎确认 tutor_688、689 接纳，committed=378,total=380,pending=2；晚到的 688 终止通知在启动前单独消费，ready ref 保持不变。tutor_690、691 同时运行，共享 remaining=1264。

tutor_690 至 693 四个动作均由引擎确认接纳，committed=382,total=384,pending=2；tutor_694、695 并行继续。本轮另外两支仍需要后续查证，共享 remaining=1264。

tutor_694 至 697 四个动作均由引擎确认接纳，committed=386,total=388,pending=2；tutor_698、699 已并行启动。未出现新的失败或用户确认边界，共享 remaining=1264。

tutor_698、699 已由引擎确认接纳，committed=388,total=390,pending=2；699 在回读期间晚到的终止通知已在派发前消费，ready ref 保持不变。tutor_700、701 并行继续，共享 remaining=1264。

tutor_700 至 703 四个动作已由引擎确认接纳，committed=392,total=394,pending=2；tutor_704、705 并行继续，共享 remaining=1264。执行器无失败，最新回读仍为 SPAWN_EXECUTORS。

tutor_704 至 707 四个动作已由引擎确认接纳，committed=396,total=398,pending=2；tutor_708、709 并行继续。当前共享 remaining=1264；本轮一支已完成，另外两支仍需查证，保留轮次归并次序。

tutor_708 至 711 四个动作已由引擎确认接纳，committed=400,total=401,pending=1；本轮第二支已完成，最后一支继续，尚未整轮归并，共享 remaining=1264。晚到的 710 终止通知在派发前消费；tutor_712 已启动。

tutor_712、713 两项后续动作由引擎确认接纳，committed=402,total=403,pending=1；tutor_714 继续本轮最后一支查证。共享 remaining=1264，尚未整轮归并。

tutor_714、715 两项后续动作由引擎确认接纳，committed=404,total=405,pending=1；tutor_716 继续本轮最后一支查证。共享 remaining=1264，当前无失败或确认请求。

tutor_716、717 两项后续动作由引擎确认接纳，committed=406,total=407,pending=1；tutor_718 继续本轮最后一支查证，共享 remaining=1264。当前没有需要用户裁决的状态。

tutor_718、719 两项后续动作由引擎确认接纳，committed=408,total=409,pending=1；tutor_720 继续本轮最后一支查证，共享 remaining=1264。当前仍为正常 SPAWN_EXECUTORS。

tutor_720、721 两项后续动作由引擎确认接纳，committed=410,total=411,pending=1；tutor_722 继续本轮最后一支查证，共享 remaining=1264。当前没有失败或用户确认请求。

tutor_722、723 两项动作接纳后引擎返回 foundation_required，无可选恢复项。初始投影丢失具体教学阻塞原因和已接纳进度。补入教学预算的有界 violations 投影并临时保留 32 步停止时的路由进度，同一 invocation 回读确认 alignment_no_progress_step_limit actual=32,limit=32；committed=412,total=412,pending=0，remaining=1264。不是候选失败或磁盘错误。

根因是查证额度耗尽后路由在生成最终判断任务前中止。修复保留 32 步查证上限，将额度校验放到 Core 动作接纳处：额度用完的全新任务仅允许当前 focus 的 resolve/identity/finish，输入明确列出唯一允许动作；禁止 search/inspect/read，以 /kind 字段诊断提供当前任务修正反馈。收尾不再创建检索准备页，来源阅读、绑定、对象基数及合并检查继续执行。32 步前的历史模型输入逐字保持原样，接纳记录和计划不变；没有扩大查证额度或自动合成语义判断。临时 32 步阻塞投影的专用状态随后移除，普通教学输入超预算保留准确 tokens/limit 投影。

验证：18 条对象对齐测试通过（包括额度耗尽后禁止继续查证、允许已读依据的最终判断、拒绝未读引用）；串行累计回放通过；5 条教学路径通过（真实语义检索的收尾不重开准备、三路任务及冻结兄弟输入、并行全轮归并与身份复用、旧串行任务续跑、各分支检索刷新）。Core 类型检查及限定 diff 检查通过。同步磁盘回放的用例间增加第二次事件循环让出，使上一项 RPC 结果可先送达；复跑进程 exit=0。

收尾修复版已编译安装，上一版留存为 understand-book-build.before-alignment-final-decision-20261009.exe；继续回读同一 invocation，无活跃执行器、无待消费终止通知。

引擎确认恢复至 SPAWN_EXECUTORS，committed=412,total=413,pending=1，tutor_724 获得全新收尾任务。首次返回 retryable_failure；引擎原额度内自动重试，tutor_725 已启动，无须用户确认。未将失败计入接纳数，shared remaining=1264。

tutor_725、726 也返回 retryable_failure，全部终止通知已消费。引擎最终 request_id=abreq1_2a53dfa3db291c325b9f210809b019df25b8e4f3e9b10444e2c50270a579eb5f，reason=retry_exhausted，projection 为 schema/semantic_output_invalid、phase=artifact_writer、required_recovery=publish_new_policy_scope、work_unit_count=1。仅提供 retry_current（Validate recovery and retry），其前提为所需恢复已完成；不能将此边界当成 confirm_candidate_retry 自动开启同范围纠错窗口。当前没有活跃执行器或回读进程，committed=412,total=413,pending=1，shared remaining=1264。保留原计划、控制文件、接纳回执和所有失败记录；未发布新策略范围，未把未接纳候选视为完成，整个书籍构建尚未 DONE。

用户明确要求“重试”后，提交原 request_id 的 retry_current 决定，并回读同一 invocation。引擎返回 recovery_not_satisfied：The bound terminal state does not satisfy same-scope retry recovery；required_recovery 仍为 publish_new_policy_scope，没有签发新执行器。committed=412,total=413,pending=1，remaining=1264；控制状态已保存，无活跃执行器或回读进程。没有重复提交不会改变恢复条件的同一请求。

用户要求找到根因后，仅从三份失败回执提取诊断字段，并与源码中的固定错误字符串对照，未读取候选正文。第 1、2 次为 Zod too_small/type=array/minimum=1；expected 恰为 120 字符，缺少 path 和 json_pointer，现存有界诊断无法确定具体数组字段。第 3 次固定错误为 alignment cites unread source range，即候选引用范围未被 read_ranges 覆盖。

失败链已定位：AlignmentActionZ.parse 的普通 ZodError，以及 requireReadBindings 的普通 Error，都被 acceptTeachingCandidate 的通用 catch 包成 schema/semantic_output_invalid，expected 使用 message.slice(0,120)，丢失字段定位。automaticBuildCandidateCorrection 不识别这些通用错误；requiredRecoveryForAutomaticBuildFailure 因此落入 schema 默认分支 publish_new_policy_scope。先前将该恢复标签解释为“确实需要发布新策略”的结论过早；它是当前分类器的默认结果，不是策略不兼容的证明。

另一个直接相关问题来自收尾修复：steps_since_progress 达到 32 后只允许 resolve，禁止 read；requireReadBindings 同时仍要求引用范围全部被显式读取。第 3 个候选触发了未读证据检查，却没有补读恢复路径。现有测试证明“已有足够已读证据时可收尾”和“未读引用仍拒绝”，没有证明缺证据的耗尽状态可恢复。仅重试候选或只更改失败分类不足以解决这条链。应保留 Zod 字段诊断，并为缺证据的收尾提供有范围限制的补读恢复，再按具体错误处理候选修正。此次为根因定位，未发布新策略或启动新的语义执行器。

2026-10-09 用户授权修复后，保留并复用 extractor-contract 的结构化 Zod 诊断，教学 writer 将失败字段的 JSON pointer、约束与有界实际值转为 schema_invalid，不再截断整个 Zod 文本。未读证据也指向 resolve 的 /object/source_bindings/<index> 或 identity 的 /source_bindings/<index>，给出需要阅读的范围，进入现有候选纠错路径。

32 步之后仍禁止 search/inspect，但允许读取当前 focus 和已 inspected 对象引用段落的未读区间；每次最多 2000 UTF-16 单位，必须完全位于累计阅读覆盖的缺口内，因而不能重复读取或扩大到新段落。输入给出 next_read 和允许动作；已读证据足够时可直接提交最终判断。若第 32 步记录了待准备的 search，收尾任务不再等待检索准备，并隐藏不能继续使用的搜索页。32 步前的输入结构保持原样，未改计划、prompt、策略版本或既有接纳记录。

验证共 43 项通过：19 项对象对齐、2 项教学写入与累计回放、17 项提取契约、5 项 Tutor 候选恢复。新增用例覆盖阅读缺口、跨已读范围/新段落/重复补读拒绝、最后一步搜索后补读再提交、空 keys/空 source_bindings 的字段诊断及冻结输入回执保留。第一轮业务断言通过但 Vitest onTaskUpdate 通知超时；两个长回放循环逐步让出事件循环后原测试完整 exit=0，未改生产逻辑或放宽断言。Core 类型检查通过。补读版已编译安装；安装前版本保留为 understand-book-build.before-alignment-supplemental-read-20261009.exe。正在用原 invocation 回读恢复投影。

实际工作区恢复检查完成：原 invocation 返回 SPAWN_EXECUTORS，签发 1 个新的 opaque handoff；此前 recovery_not_satisfied 阻塞已解除。committed=412,total=413,pending=1，既有 412 项接纳仍复用，object_alignment remaining=1264/resolved=1/total=1265。按本轮“修完告诉我”交付修复与恢复结果，尚未启动该新 handoff；控制文件已保存、无活跃子执行器或回读进程。整本 Tutor 预构建尚未 DONE，后续继续时由同一 invocation 重读并派发。

用户要求继续后，tutor_727、728 的后续动作由引擎确认接纳，原轮次完成归并：remaining 从 1264 降到 1260，resolved=4,total=1264，committed=414,total_tasks=417,pending=3。新轮次 tutor_729、730 也已由引擎确认接纳，最新 committed=416,total_tasks=419,pending=3；tutor_731 已启动。宿主仍列出两个早已接纳结束的旧子执行器 tutor_719、723 为 pending_init，造成 native agent thread limit；interrupt 返回 pending_init 后仍未释放，因此按实际剩余名额将 refill capacity_limit 暂设为 1，保持原 invocation 的 max_parallel=3，不改计划。没有复用旧 executor 或将未启动 handoff 记录为已启动。当前有一个活跃执行器，后续由同一控制记录继续。

2026-10-10 用户继续后，恢复中断点：原 tutor_731 已由上一轮引擎确认接纳；tutor_732 的 spawn 成功但中断发生于保存归属之前，补记该已知 launch 归属并将无活跃 native agent 的终止观察交给 refill。原 invocation 确認 committed=417,total_tasks=420,pending=3，为未接纳工作签发新 ref，未重新计入已接纳项。当前宿主只有 root，capacity_limit 恢复为 3；tutor_733、734、735 三路全部提交成功并由引擎确认接纳，committed=420,total_tasks=423,pending=3，remaining=1260/resolved=4/total_objects=1264。回读期间晚到的两个终止通知在派发前再经 refill 消费，全部名额正确释放；tutor_736、737、738 已三路启动。未更改计划或候选正文，仍未 DONE。

tutor_736 至 741 六项动作由引擎确认接纳，最新 committed=426,total_tasks=429,pending=3，remaining=1260/resolved=4/total_objects=1264。晚到终止通知按原归属逐次经 refill 消费，旧 ref 保留于 completed_refs，未释放替换子执行器的名额。tutor_742、743、744 三路已启动；没有候选失败或用户确认边界。
