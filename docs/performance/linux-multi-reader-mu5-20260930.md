# MU5 多窗口现场、失效读写与 Tutor 范围

基线：2026-09-30，HEAD `5e10516`，基于 MU0–MU4 实时工作树。

## 实施步骤

- [x] 阅读 checkpoint 冷启动读序与已确认 MU5 合同。
- [x] 实现有界现场注册表、挂接/分叉/接管、版本化检查点与恢复。
- [x] 接入授权 HTTP、唯一用户权威、Reader 与 Run 端口、用户级 Tutor。
- [x] 隔离服务根验证、相关回归、更新架构/代码链路/checkpoint。

## 实施合同

沿 ADR-0147 §5/§7 和多人切片方案 §5/MU5。现有 Principal、UserRuntime、ReaderWorkspace、PublishedBookRef、RunScope、Tutor 术语与归属不变。用户本轮明确要求实现 MU5；活动接单/调度属于 MU6，完整 Vue 接入属于 MU8。

现场分配经 PublishedLibrary；私人操作借用同一 UserRuntime。常驻上限每用户 4、全局 20，闲置 15 分钟且无外部运行句柄/未结接单时卸载。位置按服务端 CAS 和提交序列恢复。重复挂接默认分叉，显式接管换代；持久化失败不得返回成功或替换已提交现场。

## 验证记录

1. 首次 `cargo check -p server --all-targets` 发现仓库只启用 UUID v7，修正两处 v4 调用为既有 `now_v7`；见 [check-first.log](linux-multi-reader-mu5-20260930/check-first.log)。
2. `cargo test -p server --lib mu5_ -- --test-threads=1`：首轮 **6 通过、0 失败**，15.03 秒，见 [tests-first.log](linux-multi-reader-mu5-20260930/tests-first.log)。
3. 新增 `mu5_live_reader_observes_user_navigation_but_late_run_effect_does_not_override_it`，复现用户手动导航后晚到 Run 仍能覆盖位置；**1 失败**，见 [user-priority-red.log](linux-multi-reader-mu5-20260930/user-priority-red.log)。Run 端口固定效果 revision，自身提交后更新；用户的新动作使后续效果返回 `READER_USER_ACTION_SUPERSEDED`，同代次实时读取继续有效。
4. 扩展重启、附属页、演示状态、schema 升级和全局容量后，同一 MU5 定向命令 **11 通过、0 失败**，35.91 秒，见 [tests-second.log](linux-multi-reader-mu5-20260930/tests-second.log)。
5. `cargo test -p reader -p server -- --test-threads=1`：Reader **54 通过**；Server 库 **390 通过、1 失败、33 原有忽略**，495.95 秒。失败为最终补强的演示原发布匹配用例，再次导入封存包时返回 `PUBLICATION_STORAGE_FAILED`；见 [regression.log](linux-multi-reader-mu5-20260930/regression.log)。该命令在库测试失败后停止，二进制/集成测试在第 7 项单独补跑。
6. 定位为 `published_library::copy_file` 的 `fs::copy` 继承只读属性，暂存清单改写/封存前同步无法取得写权限。改为复制字节到新建可写暂存文件，原发布包不改动，后续校验和封存保持原合同。最终 `cargo test -p server --lib -- mu5_ mu3_ control_store::tests:: mu4_ --test-threads=1`：**38 通过、0 失败**，82.74 秒，包含 MU5 全部 11 项及原失败用例；见 [targeted-final.log](linux-multi-reader-mu5-20260930/targeted-final.log)。
7. `cargo test -p server --bins --test presentation_preview -- --test-threads=1`：Book MCP **5 通过**，Server CLI **2 通过**，演示预览 **1 通过、10 原有浏览器用例忽略**；管理/发布二进制编译成功、各 0 项测试。见 [binaries-final.log](linux-multi-reader-mu5-20260930/binaries-final.log)。
8. `cargo check -p server --all-targets`：**通过**，15.21 秒；见 [check-final.log](linux-multi-reader-mu5-20260930/check-final.log)。

上述测试使用隔离 TEMP/TMP；身份与材料均为虚构 fixture，SQLite、私人文件和 HTTP 为真实本地实现，未调用付费 Provider。

## MU5 验收覆盖

测试模块：`crates/server/src/tests/mu5_tests.rs`。以下是各组实际执行的行为；完整用例名见最终日志。

| 行为 | 实际证据 |
| --- | --- |
| 多人、多窗口与唯一私人权威 | 真实 HTTP 下 A 手机读 X、电脑读 Y，B 独立切书；A 两现场共用笔记权威，B 不可访问 A 现场 |
| 挂接、版本与持久恢复 | 刷新幂等、重复挂接分叉不建聊天、接管后旧请求失效、detach/reload；真正关闭并重开服务根后恢复原发布、聊天、位置和布局 |
| Run 私人提交与 Reader 用户优先 | 切换后的旧 Run 仍写原用户/原书，不能读写新现场；同代次读取用户最新导航，但晚到效果不能覆盖；撤权后原材料访问停止 |
| Tutor 控制 | 同用户窗口共享 operation_id/revision 权威；同版本竞争冲突，关闭再开启后原教学控制绑定仍失效 |
| 演示与原发布 | 附属页沿原现场/聊天，显式保存/恢复确切回执；用户改滑块后旧 Run 不回滚，分叉独立，接管使旧附属页失效；同内容重新发布也不能接入旧回合演示 |
| 有界常驻与继续阅读 | 每用户 4、全局 20；外部 Run 句柄及 queued 占位阻止卸载；闲置重载与 Book 活引用预算；继续阅读按服务提交序列 |
| 失败与升级 | 检查点 SQLite 故障不替换已提交 Reader；未知检查点版本拒绝；历史已删聊天清除残留选择；schema 2 升级保留 owner，schema 1 沿发布回归覆盖 |

## 已知限制

本轮使用 Windows 隔离服务根；Linux、真实 HTTPS/iPhone、活动 Run 接单和完整浏览器连续性按后续切片验收。

网络 `NetworkRunPort` 已实现并通过受限端口测试；活动执行接线归 MU6，当前普通库编译会提示该阶段入口尚无生产调用。保留原有 ts-rs 属性解析警告。本轮不是最终源码一次全量命令全绿。
