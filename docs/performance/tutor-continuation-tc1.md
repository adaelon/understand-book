# TC1 — Tutor 最小合同、来源标识与兼容读取

日期：2026-10-11（Asia/Hong_Kong）。依据 [ADR-0163](../adr/0163-tutor-continuation-and-contextual-reading-entry.md) 与[切片方案 TC1](../切片方案-Tutor下一步协商与低门槛阅读入口.md#tc1最小合同来源标识和兼容读取)。

状态：**TC1 已完成并通过定向验证**。工作树基于 `12b765bb33fe42fcd30fcda743187241ed56d1f3`，包含 TC0 及其他在途修改；本片没有提交、部署或调用真实模型。

## 1. 实现与边界

- `runtime::tutor_continuation`：`TeachingContinuation`、`ContinuationOption`、受限方向、三个 `EntryAction`、`AnswerRef`、`TeachingChoice`、来源标记 `InteractionAction`、`ResolvedInteraction`、`InteractionResolution` 和公开 `TutorContinuationView`。
- 建议为 1–2 项，标识唯一，推荐标识必须属于集合；标识上限 64 个 Unicode 字符，label／purpose 各 240 个 Unicode 字符，文字不可全空白。数量和字段界限共同限制总载荷；Runtime ToolSpec 和语义校验共用常量与方向枚举。未知字段拒绝，不能添加新 target、工具参数、评分或帮助字段。
- `TeachingMove.continuation` 可缺省。公开 DTO 使用允许字段集合，私有 assessment／hint／explanation 不进入公开建议；原工具权限仍不包含 Reader 写入能力。TC3 接通建议生产及公开文字／来源核验前，服务端明确拒绝带 continuation 的实际 select。
- `interaction_input::parse` 被本地输入验证和多人 `normalize` 共用。选择／回答／原文引用分别校验，拒绝双入口、悬空引用和 teaching_ref／note／goal／retry 混用。只有具备合法动作形状时允许空 message；普通空白请求仍拒绝。
- 归一化保存动作、回答、摘录、选项和 head 的差异；普通请求不增加缺省空字段，避免升级后重试旧 key 时产生虚假冲突。
- 回答引用先查本聊天内已成功交付的公开回答，摘录须属于单段连续公开文字；选择先查本聊天同材料的 MessageDelivered 及其选项。TC1 中结构合法的动作仍返回暂不可用，不接纳新回合。
- `TurnDomain.interaction` 使用原 `turn.accepted`／JSONL／历史投影保存解析快照，旧记录缺省为 None。本地已保存回合和多人待接纳候选回合分别向 freeze_prepare 显式传入快照，形成 `current_action`；真实用户补充独立保留，不接受客户端提交解析快照。`intent_origin=current_action` 必须匹配冻结动作的教法，不能用伪造 intent_quote 通过。
- Memory 增加 `ContinuationDisplayed`、`ContinuationChosen` 事件及载荷类型；它们不能成为正式表现或独立理解假设的依据。近期查询、展示／选择事件生产及完整意图接续由 TC4／TC6 接通。

## 2. 错误与接续合同

| 情况 | 代码／处理 |
|---|---|
| 新输入格式、组合、引用或摘录无效 | `INTERACTION_INVALID` / validation，返回具体原因 |
| 动作合同已识别，执行路径尚未开放 | `INTERACTION_UNAVAILABLE` / unavailable，提示“此阅读操作暂不可用，请继续使用输入框提问” |
| 多人聊天 head 已变化 | 沿用 `CHAT_HEAD_CHANGED` / conflict |
| 相同 key 的不同归一化输入 | 沿用 `REQUEST_KEY_CONFLICT` / conflict |
| 已接纳请求的同 key 重试 | 沿用原 Run 返回协议；不增加平行 ALREADY_ACCEPTED 状态码 |
| 原题帮助分流 | `InteractionResolution::ActivityHelp { delivery_refs }`；TC2 接通实际路由 |
| 正常动作解析结果 | `InteractionResolution::RunInput { interaction }`；TC2／TC4 接通实际接纳 |

选择携带原建议的 head；回答继续／纠偏携带点击时当前 head，可指向历史回答。TC1 校验必要字段并保留差异，完整来源版本、控制 revision、授权及消费状态判断仍归 TC2／TC4。

## 3. 兼容与类型导出

| 数据方向 | 结果与限制 |
|---|---|
| 新版读取旧对象式 Move、source target Move | 沿用原字段，continuation 缺省为 None；不重写旧 Move |
| 新版读取旧聊天／JSONL | interaction 可缺省；旧记录不生成虚构动作快照 |
| 新版重开新快照与两种新教学事件 | 使用原 JSONL／learning.db 合同；无需新数据库或迁移框架 |
| 旧版读取带 continuation 的 TeachingMove | **明确不兼容**：旧结构含 `deny_unknown_fields`，不认识 continuation |
| 旧版读取两种新 TeachingFact | **明确不兼容**：旧枚举没有两个新值 |
| 旧版读取新增 interaction | 旧结构可能忽略未知字段，但会丢失其语义；不作为安全回退依据 |

旧二进制未实际启动验证。本片明确将写入新 Move／新事件后的直接旧版回退列为不兼容；应使用保留新格式读取能力的同版本降级。正式发布／备份恢复演练属于 TC8，不删除事件或改写历史伪装兼容。

Rust 的 ts-rs 导出新增 16 个 Web 类型（Runtime 11、Memory 5），`api.ts` 接入新输入字段和历史 domain.interaction。三个带标签枚举使用显式 ts 标签，回归断言对照 Rust JSON 的 origin／kind／status，防止 ts-rs 忽略组合 serde 属性后导出不同形状。新增文件均来自导出测试。

## 4. 验证

每次运行用于定位以下具体失败，失败时修正对应合同或调用路径；不把零命中或忽略用例计为通过。

| 检查 | 检测失败及处理 | 结果 |
|---|---|---|
| Runtime Tutor 合同与既有 Tutor 测试 | schema／方向／长度边界、旧 Move 读取、公开字段或类型导出漂移；修正共同合同 | **21 通过、487 过滤**；含 11 项导出；[日志](tutor-continuation-tc1/runtime-tutor-final.log) |
| Server Tutor 与 TC1 定向测试 | 混用误记作答、输入差异丢失、引用无效仍接纳、JSONL 快照丢失或伪造 current_action；修正相应输入／存储路径 | **49 通过、10 忽略、650 过滤**；[日志](tutor-continuation-tc1/server-tutor-final.log) |
| Memory 全套测试 | 新事实污染表现／假设、旧事件回读或已有学习存储退化；修正事实边界 | **149 通过**，含 5 项新导出；[日志](tutor-continuation-tc1/memory-final.log) |
| Web 网络输入相关单测 | 原请求恢复或页面输入退化；修正元数据传递 | **2 文件、82 项通过**；[日志](tutor-continuation-tc1/web-input-tests.log) |
| Web typecheck | 导出类型不闭合；修正类型引用 | **通过**；[日志](tutor-continuation-tc1/web-typecheck.log) |

首轮 Runtime 3 项合同测试和 11 项导出通过；随后检查导出内容发现带标签枚举形状漂移，已修复并补断言，最终重跑为准。

最终命令（仓库根目录，Rust 共用下述构建环境）：

```powershell
$env:CARGO_TARGET_DIR=(Join-Path (Get-Location) 'tmp/tutor-continuation-tc1/target')
$env:CARGO_PROFILE_DEV_DEBUG='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
$env:CARGO_INCREMENTAL='0'
$env:TEMP=(Resolve-Path 'tmp/tutor-continuation-tc1').Path
$env:TMP=$env:TEMP
cargo test -j 1 -p server --lib tutor
cargo test -j 1 -p runtime --lib tutor
cargo test -j 1 -p memory --lib
pnpm --filter @understand-book/web test src/network-client.test.ts src/App.startup.test.ts
pnpm --filter @understand-book/web typecheck
```

构建环境：初次 Server 构建因磁盘空间不足中断；尝试清理缓存被自动审批拒绝，未执行删除。D 盘独立构建遇到系统拒绝访问。随后将本次两份 Runtime 缓存移至 `D:/understand-book-tc1-20261011/runtime-failed-cache`、`runtime-test-cache` 保留，并使用项目内 `tmp/tutor-continuation-tc1/target` 单路构建。设置 `CARGO_INCREMENTAL=0`、`CARGO_PROFILE_DEV_DEBUG=0`、`CARGO_PROFILE_TEST_DEBUG=0`，TEMP/TMP 指向本片目录。构建失败不记为行为测试失败或通过。

首次 Server 行为运行 47 通过／1 失败／10 忽略。失败夹具只修改了旧显示字段 quote，而已核验的 provenance 范围仍有效；核对 parse_question_quote 后改用真实越界范围。另补了多人候选回合尚未保存时的快照冻结用例；最终两项均通过。

## 5. TC0 用例迁移与已知限制

- 两个 teaching_ref 混用用例已移除 ignore，要求在任何 chat_response 写入前拒绝。
- 原“两个占位 choice 必须先接纳一个”的用例改为归一化差异与未知引用拒绝；TC0 已说明占位 delivery 不是合法建议。真实已交付选项的同 key 冲突和集合单次消费仍由 TC3／TC4 建夹具验收。
- 回答摘录进入真实模型请求的用例保留 ignore，归 TC5；TC1 验证字段保留、实际引用核验、快照存储及上下文来源，不提前开放纠偏执行。
- 本地查重、暂停后的合法 choice、两类原题帮助分流仍由 TC2／TC4／TC6 实现。TC1 的入口未开放错误不算这些行为已验收。
- TC0 已记录的 MU10 两项恢复失败保持待办，本片未修复也未重复运行；TC4／TC7 必须回看。
- 浏览器新入口、建议实际生产、完整宿主恢复矩阵、真实模型及真实读者均未验收。
