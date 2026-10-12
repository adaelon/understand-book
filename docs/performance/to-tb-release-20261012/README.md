# TO／TB 生产发布 — 2026-10-12

状态：**已上线并通过正式 HTTPS 与账本验收。** 2026-10-12 22:36:57（Asia/Hong_Kong）完成切换，22:38:58 完成账本复核；维护窗口约 14.6 秒。应用源码为 `55e767fbfc5dded3b07479a0fbe93b8f5a20a7cf`，包含 TO0–TO14、TB1–TB5、选区保存修复及交互学习首页。

正式入口：[Understand Book](https://understandbook.top/)，MCP 为 `https://understandbook.top/mcp`。应用 main 已推送。

## 运行版本与恢复点

- 当前 release：`/opt/understand-book/releases/to-tb-20261012`；数据根：`/opt/understand-book/data/multi-reader-to-tb-20261012`。
- 上一 release：`/opt/understand-book/releases/mcp10-20261012`，应用提交 `8497f29`；原数据根：`/opt/understand-book/data/multi-reader-mcp10-20261012`。
- 控制库保持 schema 12。服务 `understand-book-multi.service`，回环端口 8788；实际运行程序与新 release 路径一致，presentation worker 使用同版程序。
- 恢复点：`/opt/understand-book/backups/to-tb-20261012`，包含停写后的成套 snapshot 和 systemd／Nginx／环境配置。执行脚本与日志：`/opt/understand-book/acceptance/to-tb-20261012`。
- 保留旧程序、原数据根和旧静态资产。切换前确认无在途／未保存问答；未开放新流量时失败会恢复原配置和原根。本次没有触发回滚。
- 开放后新增事实属于当前数据根。若需恢复，先用 schema 12 兼容的维护程序备份当前根，并沿 [MCP 运行单的恢复流程](../../内测MCP-发布运行单.md#备份与恢复)处理，不用旧快照覆盖新增记录。

## 验收结果

| 验收 | 结果 | 证据 |
|---|---|---|
| Linux 构建 | Reader、Admin 与 server、manage_reader、publish_book、reader_maintenance、presentation_worker 全部通过；Rust release 构建 9 分 57 秒 | [构建日志](build.log)、[源码与构建回执](multi-reader-build.txt) |
| 生产备份／恢复 | 新程序恢复至独立目录，schema 12 → 12 | [备份](production-backup.json)、[恢复](production-restore.json) |
| 数据保留 | 控制表逐行保持；4 个账号、10 个发布、39 条授权、68 个工作区；1,738 个文件，其中 265 个私人文件、4 个 SQLite 文件 | [逐项比较](production-preservation.json) |
| 服务切换 | 新进程路径、匿名认证和 Nginx 配置通过；旧资源保留 | [部署回执](deployment.json) |
| 正式 HTTPS | 首页、MCP 接入页和后台引用新资源；Reader 登录、材料、搜索、manifest、Tutor 就绪与 PDF Range 正常 | [发布前](public-before.json)、[发布后](public-after.json) |
| 新 MCP 合同 | 11 工具包含输出 schema、注解、有效示例；正文参数包含 UTF-16 续取入口 | [正式接口验收](public-after.json) |
| 实际原文读取 | 一次 MCP 读取的发布版本及正文与 Reader 一致；精确消费 1 次免费额度，付费 0 | [正式接口验收](public-after.json)、[账本复核](production-ledger.json) |
| 原业务与费用 | 原账号、授权、模型额度、收款、赠送及调用包保持；新增模型费用 0、正式收款 0；验收凭证撤销、会话退出 | [账本复核](production-ledger.json) |

本轮使用 Linux Rust 1.98.1、Node 24.20.0、pnpm 10.34.2；从固定提交导出源码并在最终 release 目录构建，依赖按锁文件离线安装。构建保留已有 ts-rs 属性解析、未使用方法和前端包体积提示。

## 发布步骤与验证目的

- 从固定提交导出源码，在最终 Linux release 目录构建 Reader、Admin 和五个服务程序。检测漏打包、Linux 编译及静态资源构建失败；失败修正构建准备，保持旧服务。
- 沿用 [TO／TB 集成验证](../integration-to-tb-20261012/README.md)，不重复无变化的模型实验或全套本地测试。
- 发布前记录正式 HTTPS 的资源、Reader 登录、材料清单、模型额度和 MCP 余额。检测访问故障和发布基线漂移；异常先查明，不切换。
- 短暂维护窗口内确认无在途或未保存问答，旧程序完整备份，新程序恢复至独立数据根，逐表逐文件比较。检测账号、额度、费用、收款、聊天、笔记和 MCP 事实丢失；失败恢复原配置及原根。
- 启动新服务，核对实际程序路径和匿名认证，再开放流量。保留旧静态资源，支持已打开标签页。
- 正式 HTTPS 复核新资源和原数据，检查新 MCP schema、示例、续取参数，并执行一次免费原文读取，核对 Reader 正文、版本和实际扣次。失败定位资源、代理或契约接点；开放后保留新增事实，不用旧快照覆盖。

## 已知问题

TB6、TC7／TC8 和 TC2 的剩余验收仍保留；EXISTING 笔记的 Goal 完成判断尚待修复。历史模型实验没有证明学习收益，本次发布不新增模型调用。网站的视觉与交互行为沿用本轮集成浏览器证据，正式站点以资源、接口及账本验收为准。本地安装态插件及 Windows 安装包未纳入本次网站发布。
