# Understand Book DSH 构建适配器

在 DeepSeek Harness 中确认并执行 Understand Book 的标准预构建计划。DSH 提供模型和原生 child；Build Engine 负责计划、预算、候选接纳、质量门和发布。

当前提供 Windows x64 预发布包 `0.1.0-rc.2`，固定宿主为官方 DSH Desktop `0.1.7-rc.2`。插件与 Engine 分开交付，运行时不需要源码仓库或全局 Node/Bun/Cargo。真实模型完整构建与语义质量验收仍属于 DH6 后续工作。

## Windows Desktop 安装

发布目录包含插件 `understand-book-dsh-plugin-0.1.0-rc.2.tgz`、`engine/understand-book-build.exe`、本说明和 `release.json`。Engine 可保存在任意固定目录，支持中文和空格；升级时保留原 Engine 和 registry。

1. 配置 Engine：已安装的兼容 Reader 可通过 `HKCU\Software\UnderstandBook\InstallDir` 自动发现；独立 Engine 可通过环境变量 `UNDERSTAND_BOOK_DSH_ENGINE` 指定完整 exe 路径，然后启动 Desktop。也可以在 Desktop 的插件配置中设置 `executable`。
2. 打开 Desktop 的「插件 → 添加插件」，输入 tarball 的完整路径，点击「安装」，安装成功后点击「立即启用」。插件详情应显示 `understand-book-build` 正在运行。
3. 插件启动先验证控制协议和 executor MCP 协议。`build_engine_missing` 表示路径缺失；`build_engine_incompatible` 表示无法协商本包所需的控制/执行协议。修正 Engine 后重新启用。该验证不创建构建 invocation。
4. 升级时通过同一插件页安装新版本 tarball，再完整退出并重新打开 Desktop，使宿主加载新模块。通过插件详情可独立禁用或卸载；书籍工作区、构建记录和已接纳产物保留。

例如在 PowerShell 中启动独立 Engine 组合：

```powershell
$env:UNDERSTAND_BOOK_DSH_ENGINE = 'E:/Understand Book/engine/understand-book-build.exe'
$env:UNDERSTAND_BOOK_AUTOMATIC_BUILD_DRIVER_ROOT = 'E:/Understand Book/build-registry'
& 'E:/DeepSeekHarness/DeepSeek Harness.exe'
```

首次选择 registry 后保持它不变，控制调用与全部 executor 连接共用该目录；省略时沿用 Engine 默认临时目录下的 `understand-book-automatic-build-driver-v1`。改变目录不会迁移已有 invocation。

Windows 上宿主自带 pnpm 11.7.0 可能在共享 store 的 `projects` 目录创建符号链接时出现 `EPERM`。若安装详情明确是这一错误，可在当前 Desktop profile 的 `pnpm-workspace.yaml` 中添加 `storeDir: .pnpm-store`，保留其余内容，再从插件页重试。验收使用该设置；无需提升权限或更换宿主包管理器。

## 入口与配置

Cordis 发布入口为 `dist/index.js`，导出 `name`、`inject`、`Config`、`apply`；bundle 入口为 `cordis.patch.yml`。角色与四工具合同编入同一入口，所有 DSH 包保留为宿主提供的 peer。配置：

| 配置 | 含义 |
|---|---|
| `executable` | Build Engine 的绝对路径；空值依次使用环境变量、Reader 安装位置。 |
| `driverRoot` | 驱动记录目录绝对路径；空值依次使用环境变量、Engine 默认目录。 |
| `maxOutputTokens` | 每个 child 的明确输出上限，例如本次探针使用 4096。 |
| `safetyMarginTokens` | 模型上下文安全余量，例如本次探针使用 4096。 |
| `handoffsPerCall` | 一次前台调用的 handoff 启动上限；达到后等待本次 owned child 全部清理，再返回原 invocation。 |

模型 provider、model、reasoning 与上下文窗口来自宿主当前实际模型资料。批准时保存配置，后续执行读取同一快照。`prefixArgs` 仅供源码测试启动测试 Engine，不是模型可指定的命令参数。

## 使用流程

1. 根会话调用 `ub_build_prepare_and_confirm`，指定 `target_input`、`root_dir`、`pass2`（`enabled` 或 `disabled`），可附计划预算与 `max_parallel`（1–3，默认 1）。
2. 完整计划显示材料类型、阶段闭包、生成/复用/排除项、预算与模型配置。明确批准后返回持久 `invocation_ref`。此调用不启动抽取 child。
3. 调用 `ub_build_run`，只提交上述引用。使用批准时保存的并发上限，最多同时运行三个 child。
4. 正常让出后继续调用同一引用。取消后资源清理完成，仍保留同一引用供恢复。遇到 Engine 用户边界时由根问答服务请求决定。
5. 只有 Engine `DONE` 代表计划完成。child final 不作为完成凭据。

拒绝、空回答、批准附带修改文字、无 answerer 或取消都不创建 invocation；批准后计划身份或源内容改变也会拒绝创建。运行引用与批准它的根会话绑定。首版仅接受标准公共计划，私有产物和自然语言目标计划在启动前拒绝。

预算示例：`{"on_exceed":"needs_user","max_total_tokens":100000,"max_wall_clock_minutes":60}`。省略预算时沿用 Engine 标准计划规则；超预算不会自动扩大授权。

## 并发与恢复

容量取已确认并发、当前 owned child、宿主实时 Agent 注册表和在途创建的交集。固定宿主已验证三并发；原生 spawn 没有实时配额 API，因此按所有当前子代理占用保守扣减，返回 `agent_registry_conservative` 来源。先完成的 child 清理后立即推进 Engine，正常让出和用户边界会收尾已有 child。

同一实例中的重叠继续请求返回 `build_already_running`；另一插件实例或进程遇到 Engine 保存的活动 controller 返回 `build_owner_active`。Engine 只有证明原宿主进程已退出才允许重新取得控制权；无法证明时保持阻塞。不同 invocation 的工作区竞争继续使用已有 dispatch/lease 所有权门。

每次启动的有界观察保存在 Engine registry 的 `dsh-observations/<invocation_ref>/<launch_id>.json`。bootstrap 零调用、精确 open 拒绝、连接中断、运行时故障、候选回执和取消分别保存；没有调用证据时为 `null`。已提交但响应丢失时继续原引用，由 Engine 重读持久成果；未知故障不会自动变成语义重试。跨宿主接手需停止旧 owner，重新评估并确认新 invocation，已有有效产物继续复用。

## 数据与生命周期

每个 handoff 使用新的原生 child、专用空 preset、Native 四工具及独占 Engine stdio 进程。四工具参数来自 Core 单源合同。Engine 进程不接收模型凭据。每个 child 的累计模型请求都核对冻结路由与上下文容量；超限时停止，不裁剪正文。

根结果只包含控制投影，候选、分片和字段修正反馈留在 child 与 Engine 路径。正文及候选经所选模型路由处理，并可能保留在 DSH child 会话日志中；日志保留由宿主管理。

## 验证与限制

维护者构建与独立发布验证：

```powershell
node scripts/build-dsh-prebuild-release.mjs 'E:/UB Release'
node scripts/verify-dsh-prebuild-release.mjs 'E:/UB Release' 'E:/DeepSeekHarness' 'E:/Previous Engine/understand-book-build.exe'
```

构建机需要 Bun 与 npm，必要时通过 `BUN_BINARY`、`NPM_CLI_JS` 指定。验证器解包到源码目录之外，使用宿主安装版与编译 Engine，并把 PATH 限制为 Windows 系统目录；验证缺失/旧 Engine 拒绝、原生 child 取消、插件卸载与重新启用。构建工具不会随插件分发。CLI、其他平台及其他 DSH 版本保持未验证。

安装态测试入口：

```powershell
node packages/dsh-plugin/test/run-installed.mjs E:/DeepSeekHarness test/control-integration.test.ts test/control-confirmation.test.ts
node packages/dsh-plugin/test/run-installed.mjs E:/DeepSeekHarness test/plugin-entry.test.ts test/standard-preset.probe.mjs
```

这些测试使用合成材料、脚本模型与真实 Engine 源码 stdio；不会构建用户书籍。Pass2 开/关的技术书闭环已通过 writer、quality 和 publication。开启样例包含硬取消恢复与一次候选修正；单 claim 没有可用跨段配对，Pass2 由 Engine 判定无模型任务后关闭。论文已验证计划闭包与预算边界，未宣称论文完整模型构建通过。

官方 `0.1.7-rc.2` 的计划全文入口存在已定位的 SidebarRight 绑定问题。全文展示通过的组合是官方包加本地 SidebarRight 修复；实际人工批准已取得，“要求修改”人工操作仍待验收。原始材料若使用纯中文文件名，现有 Engine 不能自动派生 ASCII bookId；应先按现有流程明确 bookId 并准备有效工作区。首次稳定发布还须完成 DH0 的人工要求修改补验与 DH6 的真实模型完整构建。

源码仓库中的详细证据：`docs/performance/dsh-prebuild-compatibility-20260925.md`、`docs/performance/dsh-dh6-install-20260926.md`。
