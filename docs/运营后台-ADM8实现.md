# ADM8 读者额度界面

日期：2026-10-08。已实现并通过本地候选验收；工作树未提交、未部署。依据 [ADR-0156](adr/0156-reader-admin-and-account-allowance.md) 和[切片方案第 5、7–9 节](切片方案-运营后台与账号额度.md)。

## 个人额度与明细

Vue 网络 Reader 的书架、阅读账号菜单、附属窗口账号菜单提供“使用额度”。`NetworkApp.vue` 挂载独立 `AccountAllowance.vue` 对话框，打开、翻页和关闭均保留底层 Reader、阅读位置与输入草稿。

查询沿现有 `networkFetch` 调用 `GET /api/account/allowance` 与 `GET /api/account/usage?limit=20&offset=...`，无需 ReaderWorkspace，不附带目标账号。Cookie、no-store、身份失效和旧现场响应丢弃沿现有请求合同。

- 展示本期可用、授予（含调整）、已用、在途预留、待核算占用及有效期；日期显示香港时间。
- 金额从整数 micro_cny 格式化，最多保留六位小数，微小费用不显示为零；负数保留欠额，未知扣减显示待核算或尚未结算。
- 可用数值直接采用服务端投影，说明已扣除在途与待核算预留；使用额度与实际付款、现金余额分别表达。
- 明细跨全部额度期，每页 20 条；展示调用时间、模型、目的、状态、已扣减、当前占用，以及可展开的原期和任务归属。已释放预留的当前占用为零。
- 没有有效期、零余额、负余额、无明细、加载中和请求失败分别显示。刷新重读额度和第一页，不发起模型运行。

额度弹窗随身份 epoch、账号或会话变化关闭，旧请求不能回填；同身份的普通工作区 revision 更新保留弹窗。原文、私人历史和已有成果继续沿既有账号与材料授权访问。

## 费用停止、草稿与继续

`account-allowance.ts:spendNotice` 只识别 `category=model_spend` 和 ADM6 七类固定错误码。成功保存从 `snapshot.final_view.error` 或持久历史读取原因；未保存从 `TURN_UNSAVED.execution_error` 读取。保存成功显示“本次已完成内容已保存”，保存失败显示“尚未保存”，接单前拒绝显示“问题未提交”。普通错误文本包含费用码不触发此投影。

`App.vue` 将结构化提示传到 `RightRail.vue`，在费用停止旁显示“查看使用额度”；已交付部分回答与工具成果继续显示。原有“重试保存原结果”入口保留，仅重试保存。

发送时保留原输入、引用和目标的内存草稿，接单返回后按 `turn_id` 关联。首次发送可能创建新聊天并改变 workspace generation，Reader 会在接单返回前重建，因此 `network-context.ts:submittedRunDrafts` 在登录身份范围内持有这份草稿。费用停止时仅在当前输入为空且没有新引用时恢复，保留等待期间已写的新问题；恢复后或正常结束后移除，身份安装/退出时清空。晚到接单响应必须仍属于原账号与会话才能存入。

补额、续期、查看和刷新额度均不自动发起运行。读者返回聊天点击原“继续任务”，沿现有入口携带原 `goal_id`，由网络客户端生成新的请求号、Run 和调用身份；未完成 Goal、原历史与工具回执继续由 ADM6 合同持有。

## 验证与复跑

| 验证 | 结果与覆盖 |
| --- | --- |
| Reader 前端专项与受影响回归 | **88 通过**：精确金额/香港日期、未知与释放占用、分页/刷新、无期/零额/欠额、失败重试、旧身份响应、弹窗不卸载 Reader、草稿与引用、首次建聊天重建、保存/未保存、原 Goal 显式继续及既有网络恢复/历史展示 |
| Rust ADM5/ADM6 回归 | **13 通过**：真实 SQLite 个人查询隔离、预占结算/未知费用、零额阅读、保存失败与重试、补额不运行和新 Run/call 继续 |
| Vue 类型检查与生产构建 | 通过；使用网络入口构建 `dist-multi`，再装配既有 Admin 产物 |
| 真实 HTTPS 浏览器 | **8 组通过**：真实 Rust/SQLite/Nginx、21 条明细分页、零额度原文与草稿、真实费用停止、390px 手机补额后继续、1280px 桌面续期后继续、刷新历史不重放、过期历史访问与跨端退出/换账号；脚本错误 **0** |

浏览器模型端为本地可控 HTTP：零额与过期阶段发送数不增加；手机显式继续产生两次发送（保存笔记、回答），桌面显式继续增加一次回答。每次请求和调用身份不同，补额、续期、刷新额度与历史不会增加请求。

```powershell
pnpm -C packages/web test -- src/account-allowance.test.ts src/components/AccountAllowance.test.ts src/NetworkApp.recovery.test.ts src/NetworkApp.recap.test.ts src/network-client.test.ts src/agent-run-state.test.ts src/useAgentRun.test.ts src/App.startup.test.ts src/components/RightRail.test.ts
$env:VITE_MULTI_USER='1'; pnpm -C packages/web build --outDir dist-multi
$env:CARGO_INCREMENTAL='0'; cargo test -p server --lib -j 1 -- adm5 adm6 --test-threads=1
```

浏览器入口：显式运行 `adm8_candidate_host -- --ignored --nocapture`，设置绝对 `ADM8_CANDIDATE_INFO` 指向候选目录 `info.json`。它创建 A 普通读者、B 管理员（密码 `fixture-only-password`）、21 条本地待核算夹具、零可用额度和本地 HTTP Provider，监听 `127.0.0.1:18788`。HTTPS/Nginx 地址、模板替换和 Admin 装配沿 [ADM7 候选合同](运营后台-ADM7实现.md#验证与复跑)，运行 `node scripts/linux/smoke-reader-allowance.mjs`；可用 `ADM8_URL`、`ADM8_EVIDENCE` 指定站点与证据目录。每次全流程验收使用新的候选服务根；创建 info 同名 `.stop` 文件结束宿主。

日志：`tmp/adm8-tests-final.log`、`tmp/adm8-rust.log`、`tmp/adm8-build.log`。浏览器结果：`tmp/adm8-candidate/result.json`；截图：同目录 `allowance-desktop.png`、`allowance-mobile.png`、`continued-desktop.png`、`continued-mobile.png`；本地 Provider 请求计数：`info.provider.json`。

## 已知限制

- 正式 Linux 发布、真实 Provider 费用核对、首批费率与联系方式配置属于 ADM10。本次浏览器验收在本机 Windows 的 HTTPS/Nginx 候选完成。
- 个人接口在无有效期时返回 null，界面统一说明尚未开通或已到期；历史调用继续按原额度期展示。
- 生产构建仍有既有体积提示；Rust 保留既有 ts-rs 属性解析警告。
