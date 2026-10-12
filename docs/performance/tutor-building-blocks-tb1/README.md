# TB1：真实材料与教学行为基线

2026-10-12，**TB1 完成，未提交、未部署**。共同代码基线 `93b476da49b005def3f52b7b95bdaff4867e6e7b`，TO0–TO2 实现 `b45c2b6602c72d3cd9604fa45c2df7ce72a97f3a`；启动 HEAD `9f1ea18` 只增加接手文档。本片只新增实验入口与记录，生产教学指导和工具契约保持共同基线。

## 实施步骤

- [x] 阅读冷启动入口，冻结材料、输入、初始状态和运行约定。
- [x] 验证真实原文切片、两种起点、实际源读取和脚本身份，编译原例及两个反例。
- [x] 按固定四组各一次执行实模，保留请求、工具返回、最终交付和失败；两次入口修复均只补跑直接讲解组。
- [x] 分别报告工具结果、教学支撑与读者表现，更新方案、代码链路和 checkpoint。

## 材料与输入

[source.txt](source.txt) 是本机已导入《精通 Rust》中文阅读版的逐字截取，版本 `tb1.mastering-rust.borrowing.v1`。原书 `mastering-rust` 的 UTF-16 范围 `[134450,135041)`，原 LID `127.1–127.7`，对应“借用”起始说明与 `borrowing_basics.rs`；完整映射见 [material.json](material.json)。片段内未改写文字，实验 Book 把该完整片段绑定到 `1.1`。原材料已有译注修订，材料版本指向此中文阅读版本。

[scenarios.json](scenarios.json) 固定同一自然问题的两个明确起点、一个复制误解和一个直接讲解请求。四组均从空白私人记忆和新聊天开始，Tutor 开启、会话默认引导，未提供 TeachingMap、正式评估或既有能力。用户背景与请求按原文字段拼接发送，不预置理解假设。全部为脚本化输入，真实读者人数为 0；模型之后没有预写读者答案。

## 固定运行约定

- Provider：沿主工作区 `.env` 的原 endpoint/key，仅在进程内加载；native、`deepseek-v4-flash`、temperature=0，采用项目 CatalogMatch 运行配置。观测上传关闭。
- 四组各 1 次，共 4 个独立 Resident 回合，顺序与 scenarios 一致。直接讲解因录制入口缺陷另有两次局部补跑，各运行一次；基线失败照实保存。
- 调用前沿已有 `ModelSpendPort` 预留整个模型窗口 1,000,000 token 加实际输出上限。首轮与首次补跑共用 2,000,000 token／48 次 HTTP；结构化转发修复后的末次补跑前，将累计上限明确调整为 3,000,000 token／60 次 HTTP，继承已用 944,158 token／42 次 HTTP。结算用 Provider usage；未知 usage 或失败停止后续调用并保留预留。每组最多 12 次实际 HTTP 请求，单调用超时 120 秒。限额属于实验入口，不修改生产停机策略。
- 普通文字请求采用现有 32,768 输出上限；未指定上限的辅助 CompletionRequest 在实验录制入口补同一 32,768 上限，已有显式上限保持。其他能力以实际请求为准并计入同一预留。费用金额未由 Provider 返回时记录未知，token 不能换称货币费用。
- 不发起额外后台画像审核；模型辅助调用同样走 recorder 与预算。记录原生请求、响应与 Provider usage；私有续接推理字段不进入公开证据，保留其字符量。工具返回保留在下一次请求及最终消息中。
- Rust `+1.95.0`；独立 `CARGO_TARGET_DIR=D:/codex-build/understand-book-tb-195`，dev/test debug=0、incremental=0、`-j 2`；TEMP/TMP=`D:/codex-build/tb1-temp`。

## 检查目的与失败处置

- `tutor_tb1_fixture_replays_distinct_starts_and_source`：检测源范围错误、背景未进入冻结上下文、默认教法不成立或隔离状态污染；失败修夹具，不改 Tutor 行为。
- `tutor_tb1_budget_stops_before_overspend_and_on_unknown_usage`：检测实验入口在调用数、token 或未知费用结算后仍放行；失败修实验预算，再运行实模。
- `tutor_tb1_auxiliary_request_is_bounded_before_provider_dispatch`：检测真实 query 辅助路径遗漏输出上限或覆盖已有上限；失败修录制入口的转发参数，原始失败单列，随后只补跑受影响组。
- `verify.py`：检测原例不编译、去掉 `&` 后未产生 E0382、共享引用写入未产生 E0594；失败核对摘录/编译样例，再判定教学内容。
- `tutor_tb1_live_fixed_baseline`：检测当前模型的工具误用、交付失败、起点不适配和直接请求被覆盖；保留实际结果，教学缺口交 TB2，工具契约缺口交 TO3。通过表示证据捕获完成，不等同教学内容合格。
- 最终证据核对：每组必须有输入、冻结上下文、请求/响应、最终存储结果；缺失时修录制入口或明确未完成，不伪造交付。

## 复现入口

确定性入口：`cargo +1.95.0 test -p server --lib tutor_tb1_ -- --test-threads=1`，实模默认忽略。
实模入口：`./docs/performance/tutor-building-blocks-tb1/run.ps1 -OutputDirectory <新的绝对目录>`；换机器时用 `-ProviderEnv <原 Provider 配置文件>`。脚本固定构建环境、加载所需 Provider 字段、关闭观测上传，并显式执行 `tests::tutor_building_blocks_tests::tutor_tb1_live_fixed_baseline --ignored --exact --nocapture`。
编译内容核验：`python docs/performance/tutor-building-blocks-tb1/verify.py`。
证据汇总：`python docs/performance/tutor-building-blocks-tb1/summarize.py <实模输出目录>`。

## 确定性结果

- [deterministic.log](deterministic.log)：2 通过，实模入口 1 项默认忽略；夹具测试实际走完四组的 prepare → 原文读取 → select → 保存交付，核对两个背景与相同问题、默认引导及完整来源。替身回答只用于入口验证。
- [auxiliary-red.log](auxiliary-red.log) 复现辅助请求缺少上限；修复后 [deterministic-final.log](deterministic-final.log) 为 3 通过、1 默认忽略，覆盖无上限补齐和已有 1,024 上限保持。
- 最终 [deterministic-structured.log](deterministic-structured.log) 为 3 通过、1 默认忽略。辅助测试还穿过两层实际 recorder，确认无 answer 字段的 Query JSON 保持结构化返回，不进入普通回答解析。
- [content-check.json](content-check.json)：原文切片逐字相等，Rust 1.95.0 原例编译并输出两行 `Foo(0)`；去掉 `&` 报 E0382，通过共享引用写入报 E0594。
- 调试记录保留于 `build-first.log`、`build-role-import.log`、`fixture-first.log`、`fixture-role.log`：分别修正测试字段/导入、主动 Tutor 先 select 的夹具顺序，以及内部 Role 的枚举序列化断言。没有通过修改产品合同让测试转绿。

## 首轮真实结果

[baseline](baseline/configuration.json) 固定四组各一次。实模入口的通过表示捕获完成；逐组是否完整交付见下表，[summary.json](baseline/summary.json) 可由原始记录重新汇总。

| 输入 | 实际 HTTP 调用 | Provider 总 token | 耗时 | 教学交付 |
|---|---:|---:|---:|---|
| 只懂赋值 | 10 | 215,613 | 83.798 秒 | 有公开回答；incomplete=true，MessageDelivered=0 |
| 已懂所有权 | 12 | 303,406 | 99.089 秒 | 完整，MessageDelivered=1；两次 select 拒绝后修正 |
| 引用等于复制 | 5 | 99,209 | 42.028 秒 | 完整，MessageDelivered=1；工具错误 0 |
| 直接讲解 | 4 | 64,915 | 11.955 秒 | 无终答；实验入口拒绝无显式上限的辅助请求 |

首轮累计 31 次 HTTP，683,143 token。直接讲解中的第五次逻辑调用是 `book.query` 的辅助判断，`wire-request-04.json` 没有 max_tokens，预算入口在实际发送前拒绝；没有该调用的 transport 回执。这是实验入口缺口，不能归为模型讲解能力失败。首轮记录完整保留；补固定辅助输出上限后仅补跑该组，累计原有额度。

## 直接讲解入口修复

[direct-repair](direct-repair/configuration.json) 仅执行原 `direct-explanation` 输入，继承首轮 31 次调用／683,143 token；材料、初始状态、Tutor/TO 生产代码和模型设置保持。唯一入口修正是给原来未指定上限的辅助请求补 32,768 输出额度。用 `run.ps1 -OutputDirectory <新目录> -Case direct-explanation -PriorOutput <baseline 绝对目录>` 可重现这次补跑条件。

首次补跑结果：11 次实际 HTTP（10 次 Resident 采样、1 次 Query 辅助调用）、261,015 token、122.860 秒；incomplete=false，MessageDelivered=1。其中 Query 报“缺 answer”进一步定位为 recorder 没有转发 complete_structured：Provider 实际返回合法的 candidate_fits／plan_gate／probes 对象，却被普通 ParsedResponse 解析。随后模型首次 select 漏 operation，下一次修正成功。见 [summary.json](direct-repair/summary.json)、[原响应](direct-repair/direct-explanation/wire-response-03.json)及[运行日志](direct-repair.log)。该组保留为入口调试证据，不再作为干净的直接讲解对照；不是 Book/Provider 产品缺陷。

首轮与首次补跑合计 **42 次 HTTP、944,158 token**，费用金额未知，未知 usage=0；未触发当时的 2,000,000 token／48 调用上限，[预算记录](direct-repair/budget.json)累计一致。各次活动输入估计与实际 input/output usage 分别保存在 summary；例如两个起点活动输入从 13,147→28,099、13,222→44,296，并非上述累计 token。每次最终原生请求保留独立 wire-request，参数、工具定义、指导、工具返回和公开响应可回读。

最后一项修复分别在 TutorRequestRecorder 与 WireRecorder 透传 complete_structured，后者仍加固定输出上限并捕获真实线请求；仅改变测试录制入口。baseline 的前三组未调用结构化辅助接口，其输入与结果保持有效。对照的生产代码核对结果：`crates/runtime`、`crates/memory`、`crates/read-tools`、`crates/book-tool-contracts` 与 `server/teaching.rs` 相对 `93b476d` 无差异。

## 最终直接讲解基线与后续配对

[direct-structured](direct-structured/configuration.json) 采用修复后的录制器，继承先前全部用量。Query 已按原结构化协议执行，返回 unresolved，符合冻结片段缺少前一节所有权内容的实际范围。模型随后两次 select 仍将字段放在 move 外，依次被 move_id、object_ids 必填检查拒绝；下一次采样前达到单组 12 次实际 HTTP 上限，保存失败并停止，未交付终答。没有扩大单组上限或再次重试。

该组为 **12 次实际 HTTP（10 次成功 Resident 采样、2 次 Query 辅助调用，另一次 Resident 请求发送前被拒绝）、243,465 token、92.025 秒**。见 [summary.json](direct-structured/summary.json)、[result.json](direct-structured/direct-explanation/result.json)、[运行日志](direct-structured.log)。记录中的 ALLOWANCE_INSUFFICIENT 来自实验单组调用上限，不表示真实账户余额不足。累计 **54 次实际 HTTP、1,187,623 token**，费用金额未知；[最终预算](direct-structured/budget.json)未达到全组 60 次／3,000,000 token 上限，没有未知 usage。

TB2 对照固定为 **baseline 前三组 + direct-structured 直接讲解组**；其中所有权／误解两组完整，初学者／直接讲解两组未完整。baseline 的原直接讲解失败和 direct-repair 全部作为录制入口调试证据保留，不替换成成功样本。采用当前脚本、四组各一次、同材料和配置即可重跑；若 TO3 更新影响调用，两个教学版本都应放到同一新工具基线上重新配对。

## 已有支撑与教学缺口

- [只懂赋值的回答](baseline/starting-assignment/answer.md) 从整数赋值接到 `Foo`，给出 `Foo / &Foo` 对照表，并明确整数复制只是补充类比。这是可保留的起点适配。末尾让读者判断去掉 `&`，但尚未向这个明确“不知道所有权”的起点交代非 Copy 值赋值会移动、旧绑定不能再用的规则；答案选项 B 只是“因为不再借用”，不足以解释为什么有些不借用的赋值仍可用。TB2 应补足这一个必要连接，再让读者判断具体使用点。
- [已懂所有权的回答](baseline/starting-ownership/answer.md) 复用读者已说明的 move 规则，聚焦赋过去的是 `Foo` 值还是引用值，并让读者定位哪一行失效。这与初学者的整数对照确实不同，属于应保留行为。最终答案在读者回应处停止。
- [误解纠正的回答](baseline/misconception-copy/answer.md) 准确指出 `&foo` 没有复制 Foo、`bar` 是 `&Foo`，将“两份值”改成“同一值的临时访问”，再请读者自行解释。已完成局部纠正；目前只观察到复述邀请，尚无读者实际解释或在变化条件下运用的表现。
- 最终直接讲解组没有终答，不能判断本组是否完整尊重直接请求。入口调试阶段的 [direct-repair 回答](direct-repair/direct-explanation/answer.md)曾覆盖五点，实际 move 为 explain / direct_explanation / current_request；它受结构化转发缺陷影响，只保留作诊断。其对照表把移动后的 bar 写成“另一个 Foo”，应收紧为“从 foo 移来的 Foo 值”，避免复制误解。

## TO3 接手证据

- `starting-assignment/response-06.json` 至 `response-08.json` 连续三次 select：把 move_id、object_ids、interaction_intent 等字段放到顶层，依次被 `$.move.move_id`、`$.move.object_ids`、`$.move.interaction_intent` 拒绝，最终未建立教学交付。原请求、实际拒绝和最终存储结果均在 [result.json](baseline/starting-assignment/result.json)。
- 所有权组第一次 select 同时提供正式 assessment 与 continuation，被原活动合同拒绝；下一次含 request_hint 却漏 hint，再被拒绝，第三次修正成功。见 [result.json](baseline/starting-ownership/result.json)。
- 这些场景归 TO3 的参数嵌套、有效动作及提示条件示例。本片保持生产契约。后续若合入 TO3，TB 前后教学版本须同时置于新 TO 基线上重新配对；本组旧证据保留。
- 首次补跑的 select 缺 operation 见 `direct-repair/direct-explanation/response-07.json`。该组 Query 错误已定位并修复录制器转发，不交给 TO 修产品。
- 最终直接讲解组重复出现 move 内字段遗漏，见 [最终工具错误](direct-structured/summary.json)。补齐录制器后仍可达，属于 TO3 的真实案例；不能以原先受干扰的完整回答覆盖这次失败。

## 已知限制

本片只观察当前 Tutor 的交付行为；真实读者是否使用支撑及能否迁移由 TB6 验收。四组一次采样不代表成功率。原 TC2 内容质量与 TC7/TC8 待办保持各自状态。

冻结片段包含共享借用的定义和代码，不含前一节完整 move/Copy 讲解。本文用原例变体的实际编译核对扩展问题；模型若补足这部分规则，应保留补充解释身份。原始 Provider 请求和响应省略私有推理正文，`redacted_reasoning_chars` 保留其长度；其余公开文字、参数、工具结果和实际 usage 可回读。
