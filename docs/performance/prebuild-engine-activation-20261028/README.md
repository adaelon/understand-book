# 本项目新版 Build Engine 接入完成

2026-10-28 通过当前 Codex CLI 正规安装独立运营插件，并完成本项目配置与真实无模型连接验收。

- 插件：`understand-book@understand-book-operator-20261028`
- 版本：`0.1.0+codex.20261028131533`
- 已安装缓存：`C:/Users/Lenovo/.codex/plugins/cache/understand-book-operator-20261028/understand-book/0.1.0+codex.20261028131533`
- 引擎：`E:/allwork/Understand Book/operator-builds/20261028-prebuild-reuse/understand-book-build-x86_64-pc-windows-msvc.exe`

## 已生效的配置

用户级新插件默认关闭，原插件继续全局启用。本项目 `.codex/config.toml` 单独关闭原插件、启用新插件，主控 shell 的 `UNDERSTAND_BOOK_BUILD_EXE` 与插件 MCP 的显式 env 指向同一个最终引擎。原来的 sandbox、verbosity 和 startup grace 三项设置完整保留。

配置修改前已备份；确切备份路径见 `config-activation.json`。本轮只增加插件启用和 shell env，没有新增用户级、项目级或角色内的重复 MCP transport。

## 实际验收

1. `plugin add ... --json` 成功，见 `plugin-install.json`。
2. 本项目 `mcp get understand_book_build_executor --json` 显示独立缓存、最终引擎 env、原四工具及正确超时，见 `effective-executor.json`。
3. 本项目 `plugin list` 显示新插件启用；项目外同一命令显示关闭，见 `project-plugin-list.json` 与 `outside-project-plugin-list.json`。
4. 从有效配置中的实际缓存 launcher 启动 MCP，完成 `initialize`、`notifications/initialized`、`tools/list`；工具清单严格匹配四工具。Windows 进程观察确认该 launcher 的子进程实际为最终 exe。标准输入关闭后探针进程退出码 0，见 `installed-launcher-probe.json`。
5. 原项目 placement 合同再次验证 compatible，原三项设置保留，见 `activated-placement-validation.json`。
6. 接入前记录的 20 个旧引擎进程仍全部运行，见 `old-process-preservation.json`。没有替换它们的可执行文件或重连当前会话。

本轮探针模型调用 0、构建任务创建 0。最终引擎内容与 S1a/S2a/S3/S4 打包验收由 root 的发布记录覆盖；这里验收实际插件配置与启动路径。

## 生效范围与回退

**本项目下一次新建 Codex 会话会读取新配置。** 当前对话和已有 MCP 连接继续使用各自的旧进程；本轮没有中断它们。主控 shell 和专用执行器必须在新会话里共同加载，避免同一任务混用配置。

后台视频、音乐 operator 和 Reader 本地校准有独立配置，不受本项目 Codex 插件自动切换；本轮未发现活跃本机运营 profile，没有创建后台宿主、接管队列或部署远端。具体依据和最小后台接入动作见 `operator-routing.md`。

需要回退时，只移除本轮项目新增的两个插件 `enabled` 设置与 shell `UNDERSTAND_BOOK_BUILD_EXE` 设置，按 `config-activation.json` 中的备份还原这三项原值；保留其余后续配置变更。用户级新插件继续关闭，原插件全局启用，旧引擎与所有工作区保持可用。回退同样从新会话生效，无需覆盖或删除任一引擎文件。
