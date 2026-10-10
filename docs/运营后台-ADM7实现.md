# ADM7 后台入口与真实管理页面

日期：2026-10-08。ADM7a/ADM7b 已实现并通过本地候选站点验收；当前工作树未提交、未部署。依据 [ADR-0156 §9](adr/0156-reader-admin-and-account-allowance.md#9-后台前端选型) 和[切片方案第 7–9 节](切片方案-运营后台与账号额度.md)。

## 后台与身份

`apps/admin` 采用 React、Vite、Tailwind、shadcn/ui 和 TanStack Router/Query/Table。按需导入上游 shadcn-admin 2.2.1 的组件、侧栏、主题与移动端 hook；采用提交、导入范围和 MIT 位置见 [UPSTREAM](../apps/admin/UPSTREAM.md)。真实业务从 Rust 查询，无模拟账号、统计或认证。

`/admin/` 进入账号列表，`/admin/users/{userId}` 直接访问和刷新均保留目标。未登录进入 `/admin/login`，登录成功重读 `/api/auth/me` 后恢复目标；普通读者显示无管理权限。请求固定同源 Cookie、no-store、JSON 与写请求 CSRF；管理操作不创建 ReaderWorkspace。

`AdminSession` 持有身份 epoch。认证通知或退出即时取消查询、清理查询缓存和界面；重新聚焦/可见时读取当前身份，同账号新会话、角色变化也使旧响应失效。401 清身份，403 重新核对权限，CSRF 拒绝刷新身份并要求显式确认，写入不自动重放。退出响应丢失也发出原 `understand-book:authentication` 的 `changed` 通知。

Reader 的书架、阅读账号菜单和附属窗口根据 `capabilities.admin` 显示新标签页入口，原阅读材料和草稿保留。Reader 重新读取身份时更新能力；通知遇到已有身份检查时等待旧检查结束后重新核对。

## 管理操作与数据

账号详情支持创建/改密、启停、撤销会话、授撤确切材料发布、额度期、赠送/下调、编辑有效期、原子登记收款与授额、追加收款更正、费用核算与账号免扣。输入金额用十进制字符串转整数分或 micro_cny；日期按香港时间解释。供应商成本未知显示未知，免扣不填成本时保留原事实。

`OperationJournal` 在实际发送前将操作号、路径和非密码参数保存至该账号的 sessionStorage。单标签页同账号有未确认操作时先处理原操作；刷新后继续显示原冻结内容，查询原回执，未找到才允许显式按原号重试。成功回执移除待核对记录；明确参数/版本拒绝允许修正，连接失败、服务端不可用和身份过期保留原号。密码只在提交时使用，重试改密/创建时需要重新输入，不落浏览器存储。组件按身份 epoch 重建，旧身份异步结果不能回填。

查询在已有管理员授权后增加以下字段与入口，schema 保持 7：

| 查询 | ADM7 增量 |
| --- | --- |
| `GET /api/admin/users` | `search` 为账号字面子串，忽略 ASCII 大小写；`disabled=true/false` 可选；原 limit/offset；每项带 `current_allowance`，筛选总数、期间和余额在同一读取事务内取得 |
| `GET /api/admin/users/{id}/operations` | `{items,total,limit,offset}`；项为 actor、operation_id、kind、created_at；不存在账号返回 404，不含密码或私人内容 |
| `GET /api/admin/users/{id}/receipts` | 每项增加原收款所属 `period_id`，用于选择原期追加更正 |

明细均可分页；当前额度、旧额度期、收款原额与更正后额、额度调整、收款更正、账号管理操作、发送回执与人工核算分别展示。已有业务写请求继续采用 ADM2–ADM5 的参数、revision 和原回执合同。

## 构建与路由

`build-multi-reader.sh` 先构建 Vue Reader，再构建 Admin，完整复制到 `packages/web/dist-multi/admin`，Rust 与两套前端全部成功才写构建收据；收据带 admin 路径。MIT 随 Admin 静态包复制。桌面 Reader 原构建入口不变。

Nginx 将 `/admin` 跳至 `/admin/`，后台页面回退 `/admin/index.html`；后台 assets 和其他静态扩展名缺失均 404，缺失 index 也返回 404。两套前端脚本/CSS 分别由各自 HTML 加载，`/api/` 沿原 Rust 代理。

## 验证与复跑

| 验证 | 结果与检测的失败 |
| --- | --- |
| Admin 单元测试 | **8 通过**：同账号新会话/角色变化、旧身份响应、CSRF 不重放、401 清理、操作号恢复和账号隔离、密码不保存、明确冲突与未知结果区分、精确金额与香港时间 |
| Reader 相关回归 | **16 通过**：网络请求、私人历史恢复、现场与草稿恢复；管理入口保留草稿另由真实浏览器覆盖 |
| Rust ADM2/ADM3/ADM7 | **12 通过、1 忽略**；忽略项为本次显式启动用的候选宿主。最终增加操作列表后，ADM7 查询专项 **1 项复测通过** |
| 发布装配 | **3 通过**：Reader/Admin 次序、完整复制、MIT、任一后续构建失败不保留成功收据 |
| Vue / React 生产构建 | 两端类型检查与生产构建通过 |
| 真实 HTTPS 浏览器 | **13 组通过**：同模板 Nginx + Rust + SQLite；深链接/资源 MIME/404、真实登录/权限/CSRF、业务全流程、已提交收款响应丢失后的刷新防重、核算、草稿与手机布局、双向退出/换账号、退出响应丢失；捕获的脚本错误为 0 |

```powershell
pnpm -C apps/admin build
pnpm -C apps/admin test
$env:VITE_MULTI_USER='1'; pnpm -C packages/web build --outDir dist-multi
pnpm -C packages/web test -- src/NetworkApp.recovery.test.ts src/NetworkApp.recap.test.ts src/network-client.test.ts
$env:CARGO_INCREMENTAL='0'; cargo test -p server --lib -j 1 -- adm2 adm3 adm7 --test-threads=1
$env:MU11_BASH='D:/Program Files/Git/bin/bash.exe'; python scripts/linux/test-build-multi-reader.py
```

浏览器候选：显式运行 `adm7_candidate_host -- --ignored --nocapture`，设置绝对 `ADM7_CANDIDATE_INFO`。该测试启动临时真实服务与 A/B 夹具账号（密码 `fixture-only-password`），B 为管理员；本地费用夹具只经过真实费用端口，不发送模型请求。测试 Host 为 `https://localhost:18443`，后端 `127.0.0.1:18788`；临时 Nginx 从正式模板只替换监听地址、Host、证书和静态根。装配 Admin 后运行 `node scripts/linux/smoke-admin.mjs` 和 `node scripts/linux/smoke-admin-identity.mjs`；候选 info 放在 `ADM7_EVIDENCE/info.json`，默认 `tmp/adm7-candidate`。创建与 info 同名的 `.stop` 文件结束候选宿主。

证据：`tmp/adm7-{admin-build,reader-build,admin-tests,rust-tests,rust-query}.log`；`tmp/adm7-candidate/{result,identity-result}.json`、`account-desktop.png`、`account-mobile.png`。临时服务已停止。可交接入口见 [checkpoint_ADM](../checkpoint_ADM.md)。

## 已知限制

- 最近阅读/提问、全站用量与回访已由 [ADM9](运营后台-ADM9实现.md) 接入，个人额度由 [ADM8](运营后台-ADM8实现.md) 接入。
- 候选验证在本机 Windows HTTPS/Nginx 完成；正式 Linux release 构建、投产费率、旧库迁入及真实 Provider 验收属于 ADM10。本次未部署或调用真实 Provider。
- 生产构建保留产物体积提示（Admin 主脚本约 533 kB、gzip 168 kB）；Rust 保留既有 ts-rs 属性解析警告。
