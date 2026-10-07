# MU8 登录、多现场与断线连续性验收

日期：2026-09-30。实现：[MU8 前端连续性](../Linux多人阅读-MU8前端连续性.md)。测试对象为实时工作树；HEAD `5e10516` 不包含 MU0–MU8 和其他既有未提交成果。

## 最终结果

| 检查 | 结果 | 检测目标与证据 |
|---|---|---|
| Rust 定向回归 | 34 通过、0 失败、2 条按原条件忽略 | MU5 11、MU8 3、profile 15、goal 5；检验借用私人权威、事务、画像治理及 Goal 取消；[日志](linux-multi-reader-mu8-20260930/rust-targeted.log) |
| Vitest | 60 通过、0 失败 | 10 个文件：原键提交/重试、账号晚到结果、代次、游标、历史演示、PDF 生命周期与 Tutor；[日志](linux-multi-reader-mu8-20260930/vitest.log) |
| Chromium 实际页面 | 4 通过、0 失败 | 四组完整 Reader 流程；[日志](linux-multi-reader-mu8-20260930/chromium.log) |
| WebKit 实际页面 | 4 通过、0 失败 | 同四组登录/多窗口/PDF/演示流程；[日志](linux-multi-reader-mu8-20260930/webkit.log) |
| 多人 Web 构建 | vue-tsc 与 Vite 通过 | 检查网络入口与共用组件类型、产物构建；[日志](linux-multi-reader-mu8-20260930/build.log) |

两个忽略项分别是需显式启动的浏览器服务夹具，以及原有真实模型 G6 用例；浏览器夹具已单独运行。Vite 保留既有大 chunk 提示，Rust 保留 ts-rs 属性解析警告。

## 浏览器验证内容

`playwright/multi-user-reader.spec.ts` 使用真实 Vue 构建、Cookie/CSRF、真实 Rust 多人 HTTP 路由、临时控制库与私人目录。模型使用固定回答替身。`playwright/mu8-host.mjs` 通过本地 HTTPS 服务器提供静态文件并流式代理 API；浏览器原生处理 Cookie，Playwright 只在指定提交/PDF 请求注入故障。

1. A 首次提问时创建原聊天，服务端接单后丢弃 POST 响应；页面按原键找到回答，恢复网络/页面事件后 POST 总数仍为 1。A 留下草稿再退出，B 登录没有 A 草稿或回答。
2. 两个标签页分别阅读两本材料，workspace ID 不同；390×844 与 844×390 间切换并触发 composition 事件，草稿保持；刷新后另一页工作区不变，无问题 POST。
3. 从历史打开私人演示，再显式打开附属窗口；真实 iframe 显示内容、来源可读，追问只提交到原 session。父页创建新聊天后附属 iframe 卸载。
4. A 读取仓库中的实际 PDF 测试文件并渲染文本层；复制标签页会 fork 活跃工作区。重载时延迟 PDF 响应，退出 A 并登录没有该书授权的 B，释放旧响应后 B 不出现 PDF 画布。

最终 Chromium 用例还验证附属来源导航通知原窗口重新读取 Reader 状态，selection 回到原引用 `1.1`。

## 复现

先构建测试二进制与多人 Web。在仓库根目录设置 `VITE_MULTI_USER=1`，执行 `pnpm --filter @understand-book/web build`，将 `packages/web/dist` 内容复制到 `tmp/mu8/web-dist`。`cargo test -p server --lib mu8 --no-run` 产生测试二进制。

给 Rust 测试设置本次独立的 TEMP/TMP 目录与 `MU8_STOP_FILE` 绝对路径，执行该测试二进制：`mu8_browser_fixture --ignored --nocapture`。看到 4188 就绪后，在 `packages/web` 执行 `pnpm exec playwright test --config playwright.mu8.config.ts`。测试自动启动 4189 的本地 HTTPS 代理；需要 PATH 中的 OpenSSL 生成临时测试证书。证书、密钥和全部服务数据位于被忽略的 `tmp/mu8`。

Chromium 为默认引擎。结束一组后创建 `MU8_STOP_FILE` 让夹具正常退出；以新的临时目录和停止文件启动新夹具，再设置 `MU8_BROWSER=webkit` 运行同一命令。每组四次 A 登录加夹具初始登录达到真实认证的每分钟五次限制，因此两种引擎使用独立夹具，不调整生产限额。

前端回归文件：`network-client.test.ts`、`network-run.test.ts`、`agent-run-state.test.ts`、`agent-submission-recovery.test.ts`、`reader-surface.test.ts`、`presentation-host.test.ts`、`components/AgentPresentation.test.ts`、`components/RightRail.test.ts`、`components/PdfReaderPane.test.ts`、`useTutorControl.test.ts`。

后端定向组按 `mu5`、`mu8`、`profile_`、`goal_` 过滤，以独立 TEMP/TMP 运行。完整宽过滤 `mu` 在前一轮为 103 通过、1 失败、1 忽略；失败是新增画像夹具错误假定文档 revision 为 0，改为读取真实 revision 后，最终上述 34 项全部通过。更早一轮宽过滤为 102/0。

## 测试中修正的问题

- 服务器重启后较小 SSE 序号曾被旧比较丢弃；按 observation epoch 与 reset 快照整体替换。
- 历史与聊天 ID 同轮更新时，演示装载集合被随后清理；在同一 watcher 中先清旧聊天再装载当前回答。
- Vue 响应式演示引用无法直接 postMessage；跨窗口传普通 ID/版本。
- 工作区绑定字段与原来源/Tutor 严格请求解析混在一起；绑定验证后仅传业务载荷。
- 退出释放工作区与认证/运行写入并发时出现 SQLite 事务升级失败；检查点使用 IMMEDIATE 事务。
- 切换聊天的网络页面在恢复完成前曾允许输入，重建后草稿丢失；恢复期间禁用输入，本地既有编辑行为保留。
- 最初全部 API 都由 Playwright 代收转发，WebKit 无法正常接收登录 Cookie；改为真实本地 HTTPS 代理。测试还修正了源标签/样本书 ID 假设、MJS MIME、收尾未等待退出以及重复登录触发真实限流的问题。

## 限制

本地 HTTPS 代理使用自签测试证书并允许测试浏览器忽略证书错误；不是生产 Nginx/TLS 验收。WebKit 是 Windows 上 Playwright 提供的引擎，视口和 composition 验证不替代实体 iPhone 的选择、软键盘或后台回收。

真实 Nginx + iPhone、生产断电恢复和五分钟容量门槛归 MU10/MU11；本次没有部署现网、迁移个人资料或调用付费模型。翻译、本地化与独立后台复核保持原多人能力边界。
