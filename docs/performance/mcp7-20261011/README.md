# MCP7 验证记录

日期：2026-10-11。实现见 [MCP7](../../内测MCP-MCP7实现.md)。

## 运行目的

- Web 组件／网络回归检测一次性凭证残留、账号切换接收旧响应、无材料账号无法接入、次数或到期展示错误、登录丢失来源和误开默认版本；失败修正对应组件或账号／来源衔接。
- 来源范围与 Reader 测试检测跨叶段、续页、标题显示偏移、UTF-16 与长段滚动错误，以及错误版本被当作成功；失败修正范围投影或导航。
- 真实 HTTP／SQLite 凭证与远程合同检测新账号响应破坏鉴权／隔离、购买链接缺失、发现和扣次行为回归；失败修正对应服务端接点。
- Web 类型检查检测新增 props、API 类型和组件方法不一致；失败修正类型或接线。

## 结果

| 检查 | 结果／证据 |
| --- | --- |
| Server 库测试构建 | 通过，2m 56s；[build.log](build.log) |
| 凭证、远程协议与 Agent 合同 | 16 通过，0 失败，0 忽略；[server-tests.log](server-tests.log) |
| Web 组件／网络／Reader 回归 | 12 文件、125 通过，0 失败；其中 MCP7 新增 15 项；[web-tests.log](web-tests.log) |
| Web 类型检查 | 退出码 0；[typecheck.log](typecheck.log) |

首轮 Web 定向运行 71 通过、1 失败，原因是新增 App 测试复用浅渲染替身，而该替身没有生产组件已存在的 `ReaderWorkspace.showReader` 方法。补齐替身后单项通过，再运行最终 125 项回归全部通过。原失败与修正证据：[首轮](web-targeted.log)、[单项](reader-source.log)。未修改产品断言来绕过失败。

组件测试通过真实 `networkFetch` 和模拟 HTTP 响应检查 UI；Rust 测试通过真实 HTTP 和临时 SQLite 检查服务端。两者均未调用线上账号、模型或实际收款。

## 重跑

```powershell
$env:TEMP='D:\codex-build-cache\understand-book-reading-share-mcp4\temp'
$env:TMP=$env:TEMP
$env:CARGO_INCREMENTAL='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
cargo rustc -j 1 -p server --lib --profile test -- -C link-arg=/DEBUG:NONE
& 'target/debug/deps/server-6b365f9831000c84.exe' mcp_access:: remote_mcp::tests::
pnpm --filter @understand-book/web exec vitest run src/components/AccountMcp.test.ts src/mcp-source.test.ts src/NetworkApp.mcp.test.ts src/App.startup.test.ts src/NetworkApp.recovery.test.ts src/NetworkApp.resumption.test.ts src/NetworkApp.chat.test.ts src/NetworkApp.recap.test.ts src/network-client.test.ts src/components/AccountSettings.test.ts src/components/ReaderPane.test.ts src/reader-navigation.test.ts
pnpm --filter @understand-book/web typecheck
```

测试程序路径以后续 Cargo 实际产物为准。沿 MCP6 使用单并发、关闭增量与测试调试信息、D 盘 TEMP；本轮未搬动或删除缓存／构建文件。

## 已知限制

- Computer Use 保持此前用户停止状态，未恢复浏览器操作；没有声称已做实体浏览器视觉验收。
- 真实 Agent 客户端和 HTTPS 代理待 MCP9／MCP10；未提交／发布。
- 构建仍有既有 ts-rs 属性警告，happy-dom 测试仍有 KaTeX quirks-mode 提示；最终检查通过。
- 原本机 MCP schema 断言、MU4 JSONL 样本等历史失败未改动或重跑，沿 checkpoint 保留。
