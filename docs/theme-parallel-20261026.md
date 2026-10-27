# Book Structure 主题并发与连续执行

用户要求：主题之间并发，一个主题由一个 subagent 持续整理，完成后重新构建插件。

## 实现

- `book-structure-organization.ts` 先按原输入复用已接受的历史动作，再为每个未完成主题生成一个稳定任务。所有主题固定共享计划，只合并各自的结果。
- `book-structure-theme-session.ts` 保存每个主题的内部动作，恢复 notes、查询、已查看候选和证据；writer 重放全部已接受动作并校验最终 resolve。
- `automatic-build-executor-session.ts` 在同一 handoff 和租约下连续返回新一轮输入，每轮使用独立输入、grant 和提交位置；只有主题定稿才返回 DONE。
- 语义检索按既有确认计划和预算准备，使用文件锁串行写入共享缓存和使用量。模型整理主题继续并发。
- 源码、发布插件、项目 executor 模板同步说明同一主题内继续消费新输入，按输入引用隔离各轮材料。

## 验证

102 项相关测试通过：执行器会话 50、MCP 适配 23、媒体回归 4、主题连续会话 7、主题证据与协调 11、主题定向修订 1、构建准备与恢复 6。Core 类型检查与插件源码发布合同通过。

主题会话测试包含 Codex/DSH 连续执行、交错完成、重复提交、证据拒绝、租约过期后换执行器接续、两个真实编译程序 MCP 连接，以及并发语义搜索的共享预算记录。

对真实 `western-economics-macro-8e` 工作区做只读路由：503/503 片段、23/23 章节和 2/8 已完成主题全部复用，剩余 6 个主题同时可执行。没有向原构建聊天发消息，也没有修改书籍成果。

既有缓存插件的实际启动脚本通过注册表路径启动新版程序，四个执行工具均可用。已安装引擎与本次产物逐字节一致。

## 构建

本次构建目录：`tmp/theme-parallel-20261026/`。插件版本：`0.1.0+codex.20261026053153`。
已编译 `build/understand-book-build-x86_64-pc-windows-msvc.exe`；构建日志在 `build.log`。
发布目录：`dist/understand-book-theme-parallel-20261026/`，含薄插件、构建引擎和配套资源。
已更新本机 `E:/allwork/Understand Book/understand-book-build.exe`，旧文件保留为
`understand-book-build.exe.before-theme-parallel-20261026`。现有 Codex 缓存插件仍为原缓存版本，继续通过原启动器调用更新后的引擎；发布目录中的薄插件版本为本次新版本。
项目 executor 注册模板已同步，已启动进程使用原程序，新连接使用新程序。
测试和打包临时目录设到 E 盘，因为 C 盘已满。

当前共享工作区存在大量其他任务的未提交改动，本次只编辑上述链路、对应测试及发布说明；没有重置其他文件。


## 已知验证问题

旧 `smoke-t7-executor-release.ts` 在发送不支持的 open 协议版本后，继续向已终止的连接发第 4 个请求。
旧版备份程序与新版都在相同位置退出，因此这条旧综合 smoke 不计为通过。
本次另以符合当前连接生命周期的两条真实编译 MCP 连接完成两个主题全流程，并验证已安装插件启动器。
对照日志：`tmp/theme-parallel-20261026/executor-release-debug.log` 与 `executor-release-baseline.log`。
