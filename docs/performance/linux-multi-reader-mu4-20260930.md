# MU4 应用登录与全入口授权

基线：2026-09-30，HEAD `5e10516`；基于 MU0–MU3 实时工作树，不回滚既有修改。

## 实施步骤

- [x] 读取 checkpoint 冷启动合同、领域归属与 MU0 路由清点。
- [x] 账号凭据、可撤销会话、CSRF/Origin 与登录限流。
- [x] 多人 Host 统一授权、材料/私人对象入口、最小登录页。
- [x] 隔离根正反例与本地兼容回归。
- [x] 更新架构、代码链路、方案与 checkpoint。

## 范围

按 ADR-0147 §2 和切片方案 §4/MU4 实现。Principal、AuthSession、AuthorizedContext 均采用已确认定义；UserRuntime、ReaderWorkspace、RunScope、PublishedBookRef 和 Tutor 归属保持不变。

多人 Host 不借用本地 AppState；尚未接入的现场生命周期和接单操作分别由 MU5/MU6 实现。统一权限入口必须在这些操作及所有旧旁路前执行。完整 Reader 前端接入由 MU8 负责；本片交付登录、授权目录和退出页面。

## 已实现合同

`--multi-user <absolute-root> --origin https://…` 使用独立 Host，限定 loopback 后端；本地/桌面入口仍为原显式适配。多人 Host 不提供任意静态文件/SPA fallback，只公开固定登录壳。其余请求经 Host/Origin、Cookie 会话、CSRF 和能力白名单，再按用户解析私人对象或确切发布。

身份沿 schema 2 既有 users/auth_sessions：Argon2id m=19456 KiB/t=2/p=1 与独立盐，32 字节随机会话令牌、SHA-256 摘要入库。Cookie 使用 __Host-、Secure、HttpOnly、SameSite=Lax、Path=/；有效期 12 小时，每用户最多 8 个。登录轮换原 Cookie，改密/禁用/重新启用/全会话撤销均不会复活旧登录。登录按账号每分钟 5 次、全站 60 次，密码哈希同时最多 1 个，不持鉴权数据库锁。

私人 History 只投影当前用户可见字段；同名聊天、回合和 Presentation 在所属用户根访问。Presentation 校验原聊天/回合、原发布、已交付版本与既有语义合同。资源先授权再处理 GET/HEAD；Range 明确返回完整 200，条件请求不使用未经授权的 304。所有数据与错误 no-store，存储错误不回显路径。

已保存回合的 SSE 沿 RunStream 快照返回。AuthorizedObservation 固定 Principal、原材料及 turn/session/book，活动流等待周期 250 ms，发送前复核账号/会话/材料；撤销后写分块结束标记。活动 Run 接单与执行取消仍由 MU6 接入，观察中断不会重新执行模型。

管理员维护使用共用 ServiceWriter 的 `manage_reader`（create/password/disable/enable/revoke）及 `publish_book revoke`。密码从 stdin 读入，命令不输出密码或令牌。具体命令、权限表及代理模板见 [MU4 候选入口](../Linux多人阅读-MU4候选入口.md)。

## 验证过程

1. `cargo check -p server --all-targets` 首次通过；新增依赖锁定为 argon2 0.5.3 / password-hash 0.5.0 / rand_core 0.6.4。见 [check-first.log](linux-multi-reader-mu4-20260930/check-first.log)。
2. 首次 7 项均在材料夹具被拒绝：图片清单缺 book_id。补齐夹具后再次 7 项在 History 校验被拒绝：缺 user_turn_ordinal 与合法终态。按原合同修正夹具，未放宽生产校验。第三轮有 2 项 Auth 测试通过，9 项仍使用编译前的旧 History 夹具失败。原始日志为 `tests-first/second/third.log`。
3. 第四轮授权核心已通过，PDF 夹具被拒绝；随后 HTTP SSE 客户端等待 EOF，停止该次测试进程。已确认缺分块终止标记，补 Transfer-Encoding 与终态/撤销后的零长度结束块；过程中一次编译发现缺右括号，见 `pdf-debug.log`，修复后相关 SSE 用例通过。PDF fixture 的原辅助函数重置 book_id、且 PDF 摘要为占位值，按真实发布合同修正 fixture，见 `pdf-sse-debug.log`。未更改 MU3 来源校验。
4. 第一轮收口 `cargo test -p server --lib mu4_ -- --test-threads=1 --nocapture`：**12 通过、0 失败，34.02 秒**。独立 TEMP/TMP 为 `tmp/mu4-tests`，见 [tests-final.log](linux-multi-reader-mu4-20260930/tests-final.log)。
5. `pnpm exec playwright test -c playwright.mu4.config.ts`：最终 **2 通过、0 失败**。路由截获提供虚构身份，真实加载 Server 内置 HTML/JS/CSS；覆盖错误密码、CSRF 退出、A→B 清理与启动身份晚到失效。截图 [login-mobile.png](linux-multi-reader-mu4-20260930/login-mobile.png) 经查看，无手机宽度横向溢出。见 [web-final.log](linux-multi-reader-mu4-20260930/web-final.log)。这些浏览器用例不冒充真实 TLS Cookie 验收。

6. 完整 `cargo test -p server -- --test-threads=1`：**388 通过、0 失败、43 原有忽略**。库测试 380，二进制与集成 8；独立 TEMP/TMP 为 `tmp/mu4-regression`，见 [regression.log](linux-multi-reader-mu4-20260930/regression.log)。覆盖原本地入口、Book MCP、Agent SSE 及既有私人状态合同。
7. 新增 HEAD 长度断言复现实际错误：GET 内容长度 14，HEAD 为 0，见 [head-red.log](linux-multi-reader-mu4-20260930/head-red.log)。移除提前清空 body，让 HTTP 库抑制 HEAD 正文并保留 GET 长度，同时为观察入口初始拒绝响应补齐 JSON/no-store。最终 `cargo test -p server --lib -- mu4_ agent_stream::tests:: --test-threads=1`：**14 通过、0 失败，35.00 秒**，见 [targeted-final.log](linux-multi-reader-mu4-20260930/targeted-final.log)。
8. 最终 `cargo check -p server --all-targets` 通过，见 [check-final.log](linux-multi-reader-mu4-20260930/check-final.log)。完整回归后有第 7 步收口修改；最终状态由相应定向测试与全目标编译验证，不表述为最终源码一次全量命令全绿。

## 定向断言范围

| 用例 | 具体失败与结果 |
| --- | --- |
| `mu4_mu0_inventory_has_no_anonymous_or_legacy_bypass` | 真实 Host 遍历 MU0 111 个 HTTP 入口及其 `/api` 别名，222 个匿名请求均 401；已登录的模型/构建/诊断/目录旁路拒绝 |
| `mu4_real_host_login_cookie_rotation_csrf_logout_and_no_static_fallback` | Cookie 属性、登录轮换旧 Cookie、缺 CSRF 无注销副作用、退出撤销和未知静态路径无兜底 |
| `mu4_identity_origin_csrf_and_directory_injection_fail_before_side_effects` | 伪造身份/forwarded 头不换用户；body/query 目录与 user_id 注入拒绝；坏/缺/null Origin、坏 Host、重复 Cookie 拒绝 |
| `mu4_private_objects_are_owner_scoped_at_real_host` | 自己聊天可读；他人的 chat/workspace/turn/events/cancel/delete/presentation 均 404；自己已保存 SSE 可读 |
| `mu4_same_legacy_ids_and_delivered_presentation_stay_in_user_root` | A/B 使用相同 chat/turn/presentation 字符串，均只返回本人的实际已保存演示版本；缺版本 404 |
| `mu4_resources_authorize_before_head_range_conditionals_cache_and_revoke` | 正文/图片缓存、HEAD/Range/If-None-Match 不绕授权；路径穿越、未声明资源拒绝；撤权后缓存资源与 SSE 拒绝 |
| `mu4_pdf_real_host_is_bound_and_private_storage_failure_is_sanitized` | A 的真实二进制 PDF 响应通过；B 的 GET/HEAD 及撤权后 HEAD 拒绝；私人根故障无真实路径 |
| `mu4_sessions_survive_restart_expire_and_revoke_without_plaintext_storage` | 实际数据库重开、会话有效期、改密/禁用/重新启用/撤销、其他用户仍有效；密码 PHC 与令牌摘要实值断言 |
| `mu4_open_sse_closes_on_logout_disable_and_material_revocation` | 受控活动 RunStream 走真实 socket；退出/禁用/材料撤权后连接结束，后续哨兵事件未送出；不能取得他人的观察许可 |
| Auth 容量与限流、显式模式用例 | 每用户/全站尝试上限、时间窗回收、最多 8 会话、过期会话清理、哈希槽不阻塞已有鉴权、拒绝公网 bind 与 HTTP Origin |

T01/T02/T03/T05/T06/T07/T09/T10/T12/T13/T14 对应上述本地 HTTP 正反例；T08/T21 覆盖私人演示与同名对象。T04/T11/T66 的账号/对象拒绝、会话撤销和活动观察撤销已验证，活动执行的取消/领取复核由 MU6 补齐。本报告不将 T01–T79 全部标为完成。

## 已知限制

网络现场创建/挂接/操作、聊天选择和删除占位、活动 Run 接单与取消仍按 MU5/MU6 阶段接入；在本片入口返回明确未启用。原模型、翻译、预览、绘图和构建旁路均未对多人账号开放。完整 Reader 前端状态和设备连续性属于 MU8。

管理命令需停服务取得写锁；没有在线管理界面、开放注册或角色管理。Linux 只读挂载、进程 kill/掉电、真实 Nginx HTTPS/Safari、容量与上线门槛未执行；没有连接服务器、迁移真实用户或部署。使用虚构账号、隔离 TEMP/TMP、真实本地数据库和 HTTP，未调用付费 Provider。保留既有 ts-rs serde 属性解析警告。
