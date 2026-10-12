# MCP8 验证记录

日期：2026-10-11。实现见 [MCP8](../../内测MCP-MCP8实现.md)。

## 运行目的

- 后台组件与收款回归检测包数／金额不一致、混用金额与次数、两类收款更正入口错误、过期次数显示错误、超时刷新生成新操作号以及重复提交；失败修正对应表单、展示或恢复接线。
- 服务端管理合同检测普通读者越权、混合收款误入模型额度、同号更正重复发放、错误回滚及消费事实被覆盖；失败修正相应服务端路径。
- Admin 类型检查检测新增组件、回执字段和测试所用语言能力与当前编译目标不一致；失败修正类型或调用方式。

## 最终结果

| 检查 | 结果／证据 |
| --- | --- |
| Admin 全部测试 | 6 文件、30 通过，0 失败；新增 MCP8 组件 15 项。[admin-tests.log](admin-tests.log) |
| MCP 调用包／赠送、ADM2／ADM3 回归 | 18 通过，0 失败，0 忽略。[server-tests.log](server-tests.log) |
| Admin 类型检查 | 退出码 0。[typecheck.log](typecheck.log) |

首轮后台 28 项通过，但弹窗关闭时购包字段被复用为赠送字段，产生受控输入警告；按动作区分弹窗实例后消失，并增加重复提交与账号不可用两项用例。首轮类型检查发现新增测试使用 ES2023 `findLastIndex`，改为当前 ES2022 支持的操作，未修改编译目标或放宽断言。原日志：[测试首轮](admin-tests-first.log)、[类型首轮](typecheck-first.log)。

组件测试挂载真实 React、QueryClient、OperationJournal 与管理组件，通过模拟 HTTP 响应驱动表单和回执。服务端回归使用真实 HTTP 与临时 SQLite。本片未修改 Rust 源码，复用 MCP7 当日已构建的相同工作树测试程序；未调用线上收款或模型。

## 重跑

```powershell
pnpm --filter @understand-book/admin test
pnpm --filter @understand-book/admin typecheck
$env:TEMP='D:\codex-build-cache\understand-book-reading-share-mcp4\temp'
$env:TMP=$env:TEMP
& 'target/debug/deps/server-6b365f9831000c84.exe' mcp_pack tests::adm2_tests:: tests::adm3_tests:: mcp_usage::tests::mcp_usage_gifts
```

若 Rust 源码变化或测试产物缺失，沿 [MCP7 构建命令](../mcp7-20261011/README.md)重建，以实际产物为准。后台增加既有工作区版本的 `happy-dom` 开发依赖，Vitest 纳入 `.test.tsx`，无需新的测试框架。

## 已知限制

- Computer Use 保持此前用户停止状态，未做实体浏览器视觉验收。
- 本片未运行生产构建；集成构建、真实 Agent 客户端与 HTTPS 发布沿 MCP9／MCP10。
- 既有本机 MCP schema 断言、MU4 JSONL 样本和其他工作线失败未改动或复验，沿 checkpoint 保留。
