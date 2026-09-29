# SESSION_CHECKPOINT — 多 Harness 与 DeepSeek Harness — 2026-09-27

## 新鲜度自检

- 写入时 UB HEAD：`c2ff3e1 feat: ship reader, agent, viewport, and observability updates`；接手时对比 `git log -3`。
- DSH 源码：`E:/allwork/download/agent/deepseek-harness`，固定基线 `477b4f420553e8a52c2fbccc464d7561b239c443`。
- 官方 Windows x64 DSH `0.1.7-rc.2`：`E:/DeepSeekHarness`；旧第三方 `E:/DSH Desktop` 不作基线。

## 当前在做什么

用户本轮选择 DH6 的正式包和 Windows Desktop 安装生命周期两部分，已完成。DH1–DH5 保持有效；DH0 人工要求修改、DH6 真实模型质量/业务恢复/回滚、DH7 尚未完成。主 SESSION_CHECKPOINT.md 属于其他工作，未修改。

## 下一步（可直接接手）

1. 阅读 [DH6 安装报告](docs/performance/dsh-dh6-install-20260926.md) 与包内 README；发布资产在 `E:/UB DH6 安装验收/release-rc2`。
2. 继续 DH6 模型验收前，按切片方案 DH6 冻结授权技术书/论文、Pass2 开/关、重复次数、成功率和人工质量口径，然后执行并保留全部尝试。
3. 使用该独立包补带业务进度的 Desktop 整体重启恢复、提交后断连与受控回滚；不得把本轮空闲 Desktop 重启当业务恢复通过。
4. 若继续 DH0，重启 `packages/dsh-plugin/test/human-confirmation.probe.mjs`，设置 `DH0_HUMAN_ACTION=request_changes` 并启用 SidebarRight 修复，请用户真实操作；只有 ASK_CANCELLED 才是要求修改证据。

## 未提交 / 未完成

- DH1–DH6 源码、测试、文档纳入 2026-09-29 集成提交；未完成验收保持下述边界。
- 本轮主要新增：插件 build/config/bundle、两个发布脚本、只读 control capabilities、config/release/capabilities 测试；修复 index.ts 卸载漏注销 preset。
- 已同步插件 schemastery peer 与 pnpm lockfile；没有修改 DSH 安装文件或日常 Desktop profile。
- 官方 Desktop 当前打开，使用独立 `E:/UB DH6 安装验收/desktop-home` 与 `electron-user-data`；rc.2 已重装启用。用户在该窗口的账户状态保留。
- 后续重新打开此配置可执行 `E:/UB DH6 安装验收/Open-Desktop.ps1`；便携 ZIP 与发布目录同级。
- 本轮未启动真实模型构建。测试均已退出，无验收目录中的 Engine 进程残留。

## 本轮完成与证据

- 正式插件 `0.1.0-rc.2` tarball 仅四文件；角色/合同内嵌，DSH peers 外置，无源码/全局解释器依赖。
- 独立编译 Engine 与 tarball 位于 `E:/UB DH6 安装验收/release-rc2`，旧 Engine 保留在 `previous`。
- 启动先只读 `dsh_build_capabilities.v1`，要求 prepare/read V2、controller V1，再协商原 executor MCP 合同；缺失/旧 Engine 在工具注册前拒绝。
- 卸载等待 control/child/MCP 清理后注销专用 preset；同宿主重新启用不再报 Duplicate agent preset。
- 独立包验收 3/3：中文/空格外部目录、System32-only PATH、缺失/旧 Engine、取消/卸载/重新启用；24 个 Engine 子进程退出。日志 `E:/UB DH6 安装验收/分发验证 P0Xuid/verification.log`。
- 官方 Desktop 插件页安装 rc.1 → 升级 rc.2 → 全部进程退出重启 → 卸载 → 重装启用通过；文本快照与 pnpm 日志索引见报告。
- Desktop 重启也使用 System32-only PATH；用户 patch 在卸载/重装前后直接字节相等；profile 未安装第二份 @deepseek-ai。
- Core capabilities + 原 driver 52/52；插件 config/正式入口 2/2；插件 typecheck 通过（lockfile 更新后再次通过）。
- DH5 的并发、提交复用、用户边界与受控接手证据仍见 [DH5 报告](docs/performance/dsh-dh5-20260926.md)。

## 冷启动读序

1. 本页 → [DH6 安装报告](docs/performance/dsh-dh6-install-20260926.md) → `packages/dsh-plugin/README.md`。
2. [切片方案](docs/切片方案-多Harness预构建与DeepSeekHarness适配.md) DH6 与 §4；[ADR-0138](docs/adr/0138-multi-harness-prebuild-and-deepseek-harness-adapter.md) §6、§8、§10。
3. 发布脚本 → `packages/dsh-plugin/src/config.ts`、`index.ts`、`engine-client.ts`；`test/release.probe.ts` 是已编译安装态夹具。
4. 业务恢复续作时读 DH5 报告及 `build-control.ts`、`executor-host.ts`、`executor-bridge.ts`、driver 的 `dshControllerCommand` 与 `automaticBuildStep` lease 门。

## 已知限制

- 官方 pnpm 11.7.0 的共享 store 符号链接在本机 EPERM；独立验收 profile 添加 `storeDir: .pnpm-store` 后官方安装成功。没有提升权限或替换包管理器。
- 纯中文原始文件名不能自动派生 ASCII bookId，应先显式 bookId 准备有效工作区；中文目录与空格路径已通过。
- 官方计划全文入口仍有 SidebarRight 问题；人工要求修改仅有 ASK_ABORTED，不算通过。
- 真模型质量、带业务进度的 Desktop 重启恢复和回滚未做；CLI/其他平台/版本未验证。DH6 总项保持未完成，不宣布稳定支持。
- 正文/candidate 可能保存于 DSH child 日志；父上下文隔离不代表不落盘。
