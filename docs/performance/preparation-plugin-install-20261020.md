# 提前准备优化的本机插件安装

日期：2026-10-20。承接 [Profile discourse](profile-discourse-preparation-20261020/README.md) 和 [BookStructure](book-structure-preparation-20261020/README.md) 实验，将通过验证的最终候选安装到本机。

## 安装结果

- 插件从 `0.1.0+codex.20261019134109` 更新到 `0.1.0+codex.20261020070752`，通过现有 `understand-book` 本地市场执行 Codex CLI 安装，实际状态为 `installed=true`、`enabled=true`。
- 安装缓存：`C:/Users/Lenovo/.codex/plugins/cache/understand-book/understand-book/0.1.0+codex.20261020070752`。新 build skill 包含两个阶段的 `build.prepare` 和 BookStructure 的 `outline_pending` 接续规则。
- Build Engine：`E:/allwork/Understand Book/understand-book-build.exe`，107200512 字节。直接采用上一轮最终 `compiled-release` 产物，包含 Profile discourse 普通/fragment、BookStructure 提前准备和输入接收优化，以及已有预算批准恢复修复。
- 旧引擎备份：`E:/allwork/Understand Book/understand-book-build.before-preparation-20261020070752.exe`，107187712 字节。
- 根和发布副本的 plugin manifest 同步新版本；源码合同检查通过。安装缓存全部 11 个文件与发布副本逐字节一致，安装引擎与通过实验的最终产物逐字节一致。
- Codex 解析到的 Executor MCP 工作目录已指向新缓存。取消引擎环境变量覆盖后，新缓存启动器经注册表定位正式安装目录，初始化及四工具发现成功。

## 生效方式

新建构建聊天并读取新版本 build skill，以原先确认的计划和 invocation 继续构建。已经运行的聊天和 MCP 进程可能仍使用先前加载的指令或程序映像；此次部署保留它们，没有终止任何已有进程。现有书籍工作区、计划、invocation 和生成成果未因本次安装被重置。

## 验证依据

本轮复用先前已通过的 113 项回归、Core 类型检查和最终编译版实验，不重复完整性能测量。安装时额外检查发布合同、所有安装文件和正式启动路径，并用安装后的引擎运行两份现有交接测试。

安装态测试共 2 文件、4 项通过，耗时 96.30 秒：Profile discourse 普通提取与 fragment，以及 BookStructure 短章与分片两类场景。每个场景都实际调用正式安装路径的 `build.prepare`，验证三槽位中一份提交后可以补位，其余两份归属保留；成功提交只有一次语义尝试和一次持久提交。1 项可选路由计时测试按默认条件跳过。测试全部使用隔离夹具，没有对真实书籍调用准备、派发或提交。

操作与原始证据保存在 `tmp/preparation-plugin-install-20261020/`：

- `engine-installation.json`、`plugin-update.json`、`plugin-state.json`：引擎、备份、插件版本及启用状态。
- `installed-verification.json`、`resolved-executor-mcp.json`：文件直比与正式启动路径。
- `source-contract.log`、`installed-handoff-smoke.log`：发布合同和安装态交接检查。
- `install-engine.ps1`、`verify-installation.mjs`：本次操作入口。

## 已知边界

本轮更新本机插件和 Build Engine，没有重新制作 Windows Setup 或发布远程市场。安装不改变性能实验的结论：Profile discourse 场景已有实测收益；BookStructure 的整阶段路由减少耗时，完整交接耗时尚未证明稳定改善。交接检查使用固定候选与真实引擎、存储和 MCP，不包含原生子代理创建或真实模型生成。
