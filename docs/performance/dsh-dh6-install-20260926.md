# DH6：正式包与 Windows 安装生命周期

范围：用户选择 DH6 的分发与安装两部分。术语沿用 CONTEXT 与 ADR-0138；真实模型质量、回滚到 Codex 和 DH0 人工确认另有验收。

## 实施步骤

- [x] 构建正式 DSH bundle tarball 与独立 Windows x64 Engine；包内带角色及合同，DSH 依赖保持 external/peer。
- [x] 在源码目录之外，用安装包宿主及编译 Engine 验证中文/空格路径、缺失/旧 Engine 拒绝、启停与取消清理。
- [x] 经 Desktop 官方插件管理安装、完整进程重启、rc.1 → rc.2 升级和卸载；卸载前后用户 patch 原始字节一致。
- [x] 保存实际证据，更新安装说明与 Harness checkpoint。

## 交付与实现

完成日期：2026-09-27。UB HEAD `c2ff3e1`；在 DH1–DH5 及其他未提交改动上增量实现，未提交。官方安装为 `E:/DeepSeekHarness`，版本 `0.1.7-rc.2`。

正式包 `@understand-book/dsh-plugin@0.1.0-rc.2` 的 tarball 只有 `package.json`、`dist/index.js`、`cordis.patch.yml` 和 `README.md`。Bun 将角色、封闭四工具 schema、控制合同及必要纯函数编入入口；DSH 的包全部 external，并声明固定版本 peer。包中没有 Engine、node_modules 或运行时源码路径依赖。

`scripts/build-dsh-prebuild-release.mjs` 构建 tarball 和独立 `engine/understand-book-build.exe`；sidecar 原有默认输出路径保持，新增明确输出目录选项。发布目录：`E:/UB DH6 安装验收/release-rc2`。构建使用 Bun/npm，运行只使用官方安装的 Electron 和编译 Engine。

便携交付 ZIP：`E:/UB DH6 安装验收/understand-book-dsh-0.1.0-rc.2-windows-x64.zip`（39,738,287 字节），归档目录已核对含 tarball、Engine、INSTALL 与 release.json。`E:/UB DH6 安装验收/Open-Desktop.ps1` 可重新打开保留此次配置的独立 Desktop；该脚本是本机验收辅助，不属于通用发布包。

插件配置支持显式路径、环境变量与 Reader 自有注册表安装位置。注册表查询指定 UTF-8，保留中文路径；控制调用与 MCP 共用解析后的 executable/driverRoot。注册工具前先查询只读 `dsh_build_capabilities.v1`（prepare/read V2、controller V1），再协商原有 executor 合同；握手无 handoff 领取。旧 Engine 只可能保存其原有不支持命令诊断，不创建 invocation。

卸载路径修复：等待 control 与全部 child/连接收尾后调用 preset 注册返回的 disposer，释放 `understand-book-build-executor`。此前独立包红测在重新启用时报 `Duplicate agent preset`；修复后同宿主重新挂载通过。

## 实际验证

| 场景 | 结果与证据 |
|---|---|
| rc.2 正式 tarball + 编译 Engine | `scripts/verify-dsh-prebuild-release.mjs` 3/3。日志 `E:/UB DH6 安装验收/分发验证 P0Xuid/verification.log`。PATH 只含 System32，工作目录在仓库之外。 |
| 缺失/旧 Engine | 拒绝挂载、零构建工具、零 invocation；旧二进制为本轮开始前保留的 sidecar。 |
| 原生 child 执行中取消/卸载 | 每个场景实际 open 到 DELIVER_INPUT 后中止；宿主只剩原根 Agent，累计 24 个 Engine 子进程均观察到退出，持久 invocation 保留。 |
| 重新启用 | 同宿主卸载后重新挂载成功，工具恢复，无 preset 冲突。 |
| Desktop 官方安装 | 官方插件页输入 rc.1 tarball 安装并启用，详情显示 1 个组件运行中。`desktop-rc1-running.txt`。 |
| Desktop 升级 | 官方插件页安装 rc.2，包清单和详情版本均为 rc.2；`desktop-upgraded.txt` 及 profile `.plugin-manager/logs/operation-UzXabk/pnpm.log`。 |
| Desktop 完整重启 | 原全部 Desktop 进程退出后重新启动；仅 System32 PATH，使用 rc.2 Engine，详情仍为 1 个组件运行中。`desktop-restarted.txt`；新主进程 PID 29428。 |
| Desktop 卸载 | 官方插件页卸载，依赖与 bundle 选择均移除，详情列表不再含插件；`desktop-uninstalled.txt`、`operation-SwRdEj/pnpm.log`。用户 `cordis.patch.yml` 前后直接字节比较相同；无验收目录中的 Engine 进程残留。 |
| Desktop 重装 | rc.2 重装并启用后再次显示 1 个组件运行中；`desktop-reinstalled.txt`。profile node_modules 仅有 @understand-book 业务包与 pnpm 元数据，没有另一份 @deepseek-ai；用户 patch 仍保持原字节。 |
| Core 回归 | capabilities 1 + 原 driver 51，共 52/52；2026-09-27 执行约 197 秒。 |
| 插件回归 | config + 正式入口 2/2；正式入口经过源码 Engine 的计划确认及零预算门。插件 typecheck 通过，更新 lockfile 后再次通过。 |

上述 `desktop-*.txt` 均位于 `E:/UB DH6 安装验收`。源码单测与安装态生命周期均使用合成材料或脚本模型；没有发出真实模型构建请求。宿主账户状态不作为此次通过条件。

## 安装环境与故障记录

官方 pnpm 11.7.0 首次安装失败：共享 store 在 `E:/.pnpm-store/v11/projects` 创建符号链接时返回 `EPERM`。原日志 `operation-knZvDb/pnpm.log` 保留。仅在自建验收 profile 的 `pnpm-workspace.yaml` 添加受支持配置 `storeDir: .pnpm-store` 后，官方插件页重试成功；未提升权限、替换宿主包管理器或修改日常 Desktop profile。

早期验收夹具纠正：Node assert.rejects 不直接接受 Cordis Fiber，改为 async 回调；旧 Engine 会写诊断，因此断言改为零 invocation；官方工具层把调用者 signal 取消转换为 ABORTED，测试按真实工具合同断言；Windows PID 可被复用，退出判定改为持有的 ChildProcess 退出事实。原红日志保留于 `分发验证 XB606a`、`fSAlJV`、`F5VPtE`，其中最后一份包含真实 preset 泄漏的红测。

## 验证目的

包检查检测漏打包与错误内联宿主依赖，失败则修分发入口；安装态检查检测源码/全局解释器依赖与版本误接，失败则修构建或启动握手；生命周期检查检测卸载遗留子代理或 Engine 进程，失败则修资源所有权。未实际执行的组合不标记支持。

## 已知问题

DH0 人工要求修改仍未完成；官方 0.1.7-rc.2 计划全文入口的 SidebarRight 问题沿用既有记录。本报告不作为真实模型质量或稳定发布证明。

原始输入使用纯中文文件名时，现有 Engine 无法自动推导 ASCII bookId，准备返回失败。中文目录、空格路径及合法源文件名已通过；纯中文源文件名应先用既有流程显式 bookId 准备有效工作区。CLI、其他宿主版本与平台尚未验收。真实模型质量、带业务进度的 Desktop 重启恢复、升级 Engine 后的已接纳产物复用及回滚仍在 DH6 后续范围。
