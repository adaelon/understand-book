# SE4：版本级反馈存储

2026-10-16。状态：完成。基于 HEAD `402d9be` 的现有工作树与 SE1–SE3。下一入口为 SE5 读者反馈界面。

## 实施步骤

- [x] 共享反馈类型、私有当前记录、本地 read/set/clear/attach_state 接口；选择与现场分开保存。
- [x] 多人对话与工作区路由复用当前用户、交付和出版授权；附属窗口沿现有访问合同。
- [x] 验证保存/重开、改选/撤回、重试、用户和 revision 隔离、不可访问交付、迟到现场与写入失败。
- [x] 更新方案、代码链路、架构和主/专线 checkpoint，下一入口 SE5。

## 合同

反馈按当前用户的私有目录与 `PresentationRef` 存一份当前记录。set 完整替换选择、原因和补充，相同内容重试返回原回执；有效变更推进 `feedback_revision` 并移除旧现场。clear 保存空选择及推进后的修订号，重复撤回不重复写入。初次读取返回修订号 0 和空选择。

现场通过现有 state.save 保存后，以 `PresentationFollowUp` 和原 `feedback_revision` 附加。必须匹配当前有效选择、交付和内容版本；保存失败或迟到返回错误而保持已保存选择。已经附加的确切现场不以较新现场替换。

反馈不产生模型调用、学习证据或画像变更。反馈界面由 SE5 接入。

## API 与 SE5 接入

所有端点使用 POST。公共请求字段为 `session_id`、`turn_id`、`reference:{presentation_id,revision}`，不接受客户端指定用户。

| 操作 | 本地端点 | 其他请求字段 |
|---|---|---|
| 读取 | `/agent/presentation.feedback.read` | 无 |
| 保存或改选 | `/agent/presentation.feedback.set` | `selection:{choice,reasons?,comment?}` |
| 撤回 | `/agent/presentation.feedback.clear` | 无 |
| 附加确切现场 | `/agent/presentation.feedback.attach_state` | `feedback_revision`、`saved_state:PresentationFollowUp` |

多人服务提供同名 `/api/agent/...` 与 `/api/me/...` 端点，从当前用户的原交付回合加载确切出版并检查授权。工作区入口为 `/api/workspaces/{id}/presentation/feedback/{read|set|clear|attach_state}`，另带既有 `attachment_id`、`generation`、`expected_revision`；返回工作区投影及 `result` 中的反馈。工作区要求当前对话和原出版匹配，支持现有附属窗口 attachment。反馈操作不推进工作区 revision、不修改 Reader 或当前现场。

`choice` 为 `like | dislike`。原因取值为 `easier_to_understand`、`apt_example`、`useful_interaction`、`unclear`、`incorrect_content`、`hard_to_read`、`hard_to_operate`；原因和补充均可省略，保存为 `[]` 与 `""`。

所有成功响应返回 `PresentationFeedback`：

~~~json
{
  "session_id":"原会话",
  "turn_id":"原交付回合",
  "reference":{"presentation_id":"原演示","revision":1},
  "feedback_revision":1,
  "selection":{"choice":"like","reasons":[],"comment":""},
  "saved_state":null,
  "created_at":"服务端时间",
  "updated_at":"服务端时间"
}
~~~

同一选择、原因和补充的重复 set 原样返回；真实变更才推进反馈修订号。clear 后 selection/saved_state 为 null，保留 created_at 和新修订号。现场附加不改变选择的修订号，重复附加同一回执原样返回；另一回执返回 `PRESENTATION_FEEDBACK_STATE_CONFLICT`。过时或已撤回的选择返回 `PRESENTATION_FEEDBACK_STALE`，两者均为 409。

SE5 调用顺序：`set → 收到保存回执 → 尝试采集并调用现有 state.save/presentation/save → attach_state(原反馈修订号,确切现场回执)`。快照失败时已保存反馈仍有效，saved_state 为 null；失败重试保持原请求身份。切页、改选或撤回后不把旧快照移交给新选择。原聊天失效后反馈沿交付访问检查拒绝读取，独立笔记不会赋予此端点原聊天权限。

## 存储与代码落点

- [Runtime 类型](../../../../crates/runtime/src/presentation.rs)：选择、原因、完整反馈回执；[生成的前端类型](../../../../packages/web/src/generated/PresentationFeedback.ts)可直接供 SE5 使用。
- [presentation_store](../../../../crates/server/src/presentation_store.rs)：`read_presentation_feedback`、`set_presentation_feedback`、`attach_presentation_feedback_state`；当前用户的 `presentation_root/feedback/{presentation_id}/{content_revision}.json` 持有一份当前记录，临时文件完成写入和同步后原子替换。
- 私有额度仍统计整个演示目录；替换当前记录仅计一次新文件及其大小。候选、版本和现场继续走原来的不可覆盖写入路径。
- [presentation_api](../../../../crates/server/src/presentation_api.rs)：`feedback_route` 解析输入、复用确切交付检查并返回保存结果；[本地路由](../../../../crates/server/src/lib.rs)、[多人权限路由](../../../../crates/server/src/authorization.rs)、[多人宿主](../../../../crates/server/src/multi_user_host.rs)、[工作区](../../../../crates/server/src/workspace_registry.rs)各自提供现有上下文。
- 调用沿既有当前用户锁执行；内容版本、现场记录、聊天和教学事实不会被反馈写入改写。

## 验证结果

| 检查 | 结果与证据 |
|---|---|
| SE4 定向验收 | 5 项通过：[日志](targeted-tests-fixed-fixture.log)、[测试源码](../../../../crates/server/src/tests/presentation_se4_tests.rs) |
| 当前反馈生命周期 | 空读、最小选择、选填原因/补充、改选、撤回、再选择、重读和新宿主重开；重试不改回执、不增文件；内容/聊天/记忆保持原值 |
| 确切现场 | 冻结原现场而非最新现场；重复回执不写入；不同回执拒绝；改选、撤回、再选择后的旧修订号拒绝；错 session/turn/内容 revision/现场 revision/回执 ID 均拒绝 |
| 权限与版本 | 未交付、缺失版本、错误回合/会话/书籍、客户端冒充用户、删除聊天拒绝；不同内容 revision 独立；两个真实服务用户具有相同对象 ID 时选择仍独立 |
| 多人入口 | 对话别名、工作区、附属窗口、服务重开、出版撤权及聊天删除验证通过；反馈不改工作区 revision 或 Reader；已有真实 HTTP 读取回归通过 |
| 保存与额度 | 实际阻塞目录不给保存回执；达到文件数上限仍可替换；超字节额度保留原选择；现场附加失败保留已保存选择且可重试 |
| 原有存储回归 | 23 项最终通过，5 项原有 ignored 未启动；[组运行](store-regression.log)为 21 通过、2 失败；两项独立临时目录重跑均通过：[追问](followup-isolated.log)、[EX13](ex13-isolated.log) |
| 原有额度与用户隔离 | 2 项通过：[日志](regression-presentation_store--mu2_tests.log) |
| 原有多人读取与附属窗口 | 各 1 项通过：[确切交付 HTTP](regression-mu4_same_legacy_ids_and_delivered_presentation_stay_in_user_root.log)、[附属窗口](regression-mu5_presentation_linked_view_keeps_chat_and_late_restore_cannot_replace_user_state.log) |
| 共享类型导出 | 4 项通过，四份新 TypeScript 类型已生成至前端目录：[日志](shared-types-correct-directory.log) |
| 接手文档 | 主/专线 checkpoint 为 37/38 行；两份 checkpoint 与本记录的 33 个本地链接存在，相关 Rust 改动的 diff 空白检查通过 |

复现定向验收，在仓库根目录运行：

~~~powershell
cargo test -p runtime -p server --lib se4_ -- --nocapture
cargo test -p runtime -p server --lib export_bindings_presentationfeedback -- --nocapture
~~~

本次相关回归复用上述 Cargo 生成的 Server 测试程序，过滤 `presentation_store_tests --skip ::se4:: --test-threads=1`、`presentation_store::mu2_tests` 及表中两个多人测试。旧追问用例各用新的 TEMP/TMP 单独运行，保留原失败输出。

## 已知限制与失败记录

- SE5 界面、浏览器交互验收和反馈追问接续待下一片；本片只验证后端和共享类型。所有选择均为工程测试数据，未收集真实读者反馈，未运行付费模型实验、发布或部署。
- [首轮定向测试](targeted-tests.log)中 4 项通过，网络用例的第二用户夹具未先创建会话日志，导致 `session log missing`；补齐既有 create/accept 初始化后全部通过。
- 组运行的两项旧追问测试复现 SE3 已记录的共享学习存储冲突 `TEACHING_INVALID`；分别隔离临时目录后通过，旧测试夹具本片未改。编译保留既有 ts-rs serde 属性警告。
- 首次直接运行类型导出程序使用了仓库根目录，导出到了其相对路径；已改从 `crates/runtime` 执行并移除本次误导出的四份新类型。复现时使用 Cargo 命令会选择正确工作目录。
