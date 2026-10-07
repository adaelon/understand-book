# MU6a–MU6b 持久接单、恢复与运行生命周期

基线：2026-09-30，HEAD `5e10516`，MU0–MU5 实时工作树。保留既有未提交工作。

## 实施步骤

- [x] 读取 checkpoint 冷启动读序与 §5.4、§6.1–§6.4、MU6a/MU6b 合同。
- [x] 抽出用户级准备、执行与终态入口；冻结教学载荷与可续接 Pending Turn。
- [x] 实现单槽持久接单、原键比较、领取及启动对账，接入授权 HTTP 与旧 chat 别名。
- [x] 实现指定取消、删除占位、未保存结果重试与残留现场清理。
- [x] 故障注入、真实 Host/fake Provider 验证；完整共享核心回归另见下表。
- [x] 更新接口、架构、代码链路与 checkpoint。

## 实施合同

用户要求顺序完成 MU6a → MU6b。沿既有 ADR-0147 和多人切片方案，领域术语、用户权威与原材料归属不变。跨 control.sqlite、History、Learning 的准备使用稳定 ID、冻结载荷及回执对账；已领取执行不自动重放。固定一个执行槽，公平调度属 MU6c，新 SSE 恢复协议属 MU6d。

## 验证记录

验证只针对具体失败：原 Agent/Tutor 回归用于识别端口拆分引入的行为变化；接单故障注入用于识别重复执行、错误恢复和提前接受；取消/删除交错用于识别回合丢失或错误释放占位。失败时修正对应路径并复跑受影响用例。

所有测试使用仓库 `tmp/mu6-tests` 或 `tmp/mu6-full-tests` 作为独立 TEMP/TMP；未调用付费 Provider。命令输出保存在 [日志目录](linux-multi-reader-mu6ab-20260930/)。

| 命令 / 阶段 | 结果 | 证据 |
| --- | --- | --- |
| `cargo test -p server --lib -- agent_run_tests:: tests::tutor_loop_tests:: --test-threads=1`，拆分前 | 38 通过、4 原有忽略，182.06 秒 | `baseline.log` |
| 同一 Agent/Tutor 组，拆分后 | 38 通过、4 原有忽略，183.22 秒 | `extract-regression-2.log` |
| `cargo check -p server --all-targets`，Host 接线后 | 通过，15.41 秒 | `host-check.log` |
| `cargo test -p server --lib -- mu6 mu5_ mu4_ mu3_ control_store::tests:: --test-threads=1` | 55 通过、0 失败，136.61 秒 | `targeted.log` |
| 教学交付回执写入失败后重启补齐（修复前） | 预期失败：History 已 completed，缺少 `followup:{turn}` 回执 | `delivery-red.log` |
| `cargo test -p server --lib mu6 -- --test-threads=1`，修复后含新增恢复/取消用例 | 20 通过、0 失败，67.48 秒 | `mu6-final-2.log` |
| `cargo test -p memory -p reader -p runtime -p server -- --test-threads=1`，终态恢复修复后、turn 分配修复前 | 1,015 通过、0 失败、46 原有忽略；各组详见下文 | `full-regression.log` |
| `cargo test -p server --lib -- mu6 agent_history goal --test-threads=1`，turn 分配修复后的最终源码 | 33 通过、0 失败、1 原有忽略，119.24 秒 | `final-affected-2.log` |

初次扩展测试的敏感确认用例用了不受支持的“确认”；改为既有协议的“确认保存”后通过。两次增量编译发现本地 profile 薄适配和私人演示读取可见性遗漏，补齐后通过；没有修改敏感确认语义来适配测试。

完整回归分组：Memory 125；Reader 54；Runtime 库 411、集成 6（另 3 忽略）；Server 库 411（另 33 忽略，570.00 秒）；Book MCP/CLI 7；演示预览 1（另 10 忽略）。不重复计入 Server 写锁测试内部启动的子进程用例。最后的 turn ID 分配修复另跑受影响组；不是最终源码一次全量命令全绿的声明。

## 实现与覆盖

`run_admission.rs` 在 schema 4 保存轻量接单状态；私人 Pending Turn 的 `FrozenTurn` v1 持有业务输入，LearningStore 持有教学事实。`validate_agent_input / append_pending_agent_turn / execute_model / run_precommitted_with_ports / finalize_user_agent_turn` 由本地和网络共用。网络端直接借用 UserStatePort、NetworkRunPort 与原 RunScope，不创建影子 AppState 或用户 Store 副本。

| 范围 | 已验证行为 |
| --- | --- |
| 幂等接单 | 同键并发一个 turn；同键内容变化冲突；原键不受后来切书影响；不同键不能绕过同聊天占位；202 只在 queued 完成后返回 |
| 提交点恢复 | preparing 无私人输入闭键；Pending 已保存、准备标记已保存或 queued 已保存时复用同回合；claimed 记中断；History 已终态只修复回执/索引 |
| 教学续接 | 回应回执已写而绑定未完成时复用原 attempt、原控制版本和原载荷；后来关闭 Tutor 不重算；终态交付回执失败后重启补齐且不调模型 |
| 原始依赖 | 冻结原 Book/Reader/消息/Provider；切书后笔记仍归原材料，旧效果不覆盖新现场；确认候选不落盘，重启明确要求重新确认；撤权/禁用/Provider 变化不开始新调用 |
| 单槽执行 | 活动执行期间另一用户仍可读取和接单；第二个模型不进入槽；停止后不领取；剩余 queued 在下次启动执行 |
| 指定取消 | B 不能取消 A；queued 取消不调模型；取消与领取交错最多一次；claimed 取消保留已写笔记，实际退出后释放计算槽 |
| 未保存 | History 原子替换故障保留 pending 和占位；retry-save 保留原输出且不调模型；queued 取消保存失败同样占位；进程丢失内存结果后记中断 |
| 聊天删除 | preparing/queued/claimed/unsaved 均拒绝；成功删除清除多个现场并换代；History 已删而现场未清理的故障可恢复；旧键不能复活聊天 |
| HTTP / 兼容入口 | 真实 Host 并发同键；旧 `/agent/chat` 等待同一回合；本地 fake HTTP Provider 走真实适配器传输；丢失接受响应后可按 key 查询；密钥不进入 History |
| 原上下文恢复 | 模拟 claimed 执行已安装临时上下文后重启，恢复冻结消息前缀和问题，运行内工具状态不进入下一轮 |

故障点使用 `preparing / pending / teaching_receipt / prepared / queued / claimed / terminal / chat_deleted`，配合真实 SQLite 失败触发器、History 替换目标写入失败及关闭/重新打开服务根。测试覆盖实际跨存储提交序列，平台 kill/掉电仍属下节限制。

教学交付恢复修复的原因：History 保存成功后 `MessageDelivered` 写入失败，本轮已完成但接单仍未结。原启动分支只释放索引会丢失已交付事实。现以 `record_user_delivery` 复用原稳定 ID/原终态/私人演示版本，补齐事实及复核游标后才 settled；此路径无需当前 Book 或当前 Tutor 控制，也不重放模型。

接单身份修复的原因：preparing 已提交、私人 History 未保存时，恢复会关闭旧键；如果新请求仍按聊天序号生成相同 turn ID，会碰到 `UNIQUE(owner_user_id, turn_id)`，使该聊天无法继续。网络接单现预先分配并持久保存唯一 turn ID，Pending Turn 和 Goal 沿用这同一身份；本地入口保持原生成规则。补测在关闭旧键后用新键完成同一聊天的新回合，旧键仍关闭。

首次身份修复复验编译发现项目只启用了 UUID v7；改为与 workspace/chat/boot 相同的 `Uuid::now_v7()`，沿用现有依赖配置后重新运行。

接口见 [MU6 运行接口](../Linux多人阅读-MU6运行接口.md)，实现路径见 [代码链路](../代码链路.md) 的 MU6 两节，结构见 [架构](../架构.md) 的持久接单节。沿 ADR-0147，无新增领域决策。

## 已知限制

本轮使用 Windows 隔离服务根和本地测试 Provider；Linux kill/掉电、真实 HTTPS/iPhone、生产部署按后续切片验收。

- 固定单执行槽和每用户 4/全局 20 未结接单，按接单顺序；公平调度、独立资源槽和统一配置归 MU6c。
- 旧同步 `/agent/chat` 等待占用 HTTP 工作线程；四个线程同时等待会延迟其他请求。异步接单立即返回；同步等待容量与普通请求隔离列为 MU6c 接手项。
- 使用既有 SSE，排队返回一次快照；新跨重启游标/观察恢复归 MU6d。完整 Vue 接入归 MU8，生成代码执行隔离归 MU7。
- 编译保留原 ts-rs 属性解析警告；NetworkRunPort 的 `capture / restore_presentation` 目前只由测试调用，普通库有 dead_code 警告。
