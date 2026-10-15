# MCP10 Linux 发布、恢复与 HTTPS 验收

2026-10-12（Asia/Hong_Kong）。**已发布并通过验收。** 应用源码为 main 合并提交 `8497f29a6ab1315af89f65094033b03d767e999d`，包含 Tutor TC0–TC6、Reader 和 MCP0–MCP9。运行单见 [内测 MCP 发布运行单](../../内测MCP-发布运行单.md)。

正式 MCP 为 `https://understandbook.top/mcp`。08:53:05 开始切换，08:53:21 开放新版本，08:57:06 完成账本复核，发布窗口约 16.7 秒。[发布回执](deployment.json)记录实际 PID、release、新旧数据根与恢复点。

## 构建及工具验证

Reader、Admin 和五个 Rust 程序在最终 release 目录构建通过。Linux Rust 1.98.1／Node 24.20.0／pnpm 10.34.2，锁文件保持；Rust 编译约 9 分 41 秒。[完整日志](build.log)仅去除终端颜色与行尾空白，原始日志在服务器 acceptance 目录；[构建回执](multi-reader-build.txt)。

`verify-mcp-upgrade.py` 直接比较旧业务表与私人文件，只有已发布包根路径允许重定位。Windows／Linux 的 5 项验证覆盖保留、模型额度丢失、收款变化、schema 12 消费丢失及笔记变化；[Linux 日志](verifier-tests.log)。现网 Python 的 SQLite 3.34 不支持 STRICT 表，使用原有 pysqlite3／SQLite 3.46.1。

本轮首次源码包漏带根 `tsconfig.json`，导致前端编译失败；补齐提交内配置和引用目录后成功。联网安装完成后进程迟迟未退出，改用现网已有离线缓存完成安装。均发生在新 release 构建期间，原服务持续运行。保留的 ts-rs、未使用内部方法和前端包体积提示未导致构建失败。

## 独立恢复演练

1. 原程序恢复 schema 9 快照并登录，10 个资料发布、账号与模型额度可读。
2. 新程序恢复并升级同一快照至 schema 12，21 张旧控制表及 1,698 个文件（225 个私人文件）完整保留。[升级比较](rehearsal-upgrade.json)。
3. 实际旧服务打开 schema 12 返回 `CONTROL_SCHEMA_INCOMPATIBLE`，退出码 2。[旧版本行为](old-binary-schema12.json)。
4. 在独立升级副本通过协议、来源、15 次赠送消费及 1／4／8 路调用。[副本 MCP](candidate-mcp.json)。另在该副本登记 1 个明确标记为模拟的付费调用包。
5. 停止副本后备份 schema 12，再恢复到另一独立目录。凭证、赠送、15 条消费、模拟收款和调用包逐行保持；启动后账号、余额与收款 API 一致。[恢复比较](rehearsal-restore12.json)、[演练结论](rehearsal.json)。

模拟收款只在隔离目录中，未进入生产。

## 最新生产数据保留

正式发布在 Nginx 维护窗口内，原程序备份最新 schema 9 根，新程序恢复到新的 schema 12 根。逐项比较完成后才开放新版本。

| 保留对象 | 数量 |
| --- | ---: |
| 原账号／发布版本／材料授权 | 4／10／39 |
| 默认发布／阅读工作区 | 9／67 |
| 模型额度期／额度调整 | 4／5 |
| 模型费用／费用报告 | 280／280 |
| 文件／其中私人文件／内部 SQLite 文件 | 1,738／265／4 |

[备份回执](production-backup.json)、[恢复回执](production-restore.json)、[完整数据比较](production-preservation.json)。原程序、原数据根和成套备份均保留，后续恢复须遵循运行单，不能覆盖发布后的新增事实。

## 正式 HTTPS 和账本

- 原 Reader 和 Admin 登录、书架、模型额度与收款 API 正常；Bearer 无 Origin 可以接入，初始化返回协议 2025-11-25 和使用说明，发现 11 个工具。
- 通知 202、GET 405、错误 Origin 403、错误协议 400、无凭证 401 均符合合同，验证正式代理传递认证与协议头。
- 读取 `mastering-rust` 确切发布；返回来源 URL 的书籍／版本与 UTF-16 `[4,41)` 坐标一致，文字与 Reader 同版本接口逐字匹配。
- 第一轮赠送并消费各 15 次；补测完整 8,000 字符页赠送并消费各 13 次。账本合计 28 条（检索 1、原文 27），全部扣免费来源，reader 原有 1,000 次余额保持。两个验收凭证均已撤销，验收会话退出。
- 最终旧账号、授权、模型额度、收款和费用记录与发布前一致；付费调用 0、模型费用新增 0、真实收款新增 0。

[公网 MCP 结果](public-mcp.json)、[完整页并发](public-full-pages.json)、[生产账本核对](production-ledger.json)。

首页、MCP 接入深链接、后台和引用的静态资源可读，资源版本与新 release 一致；原材料目录和模型额度与发布前响应一致，PDF Range 和 Tutor 就绪接口正常。[发布前](public-web-before.json)、[发布后](public-web-after.json)。Cloudflare 注入统计脚本且 HTML 保留混合换行，比较采用页面引用的应用资源版本。

## 并发与容量

公网客户端经公共 DNS、Cloudflare、源站 Nginx 和服务读取已缓存的 Rust 材料。每次原文 8,000 字符，完整响应 37,738 字节，带续页信息；计时包含新连接、TLS 和网络。

| 同时调用 | 成功 | 单次耗时范围 |
| ---: | ---: | --- |
| 1 | 1 | 1.641 秒 |
| 4 | 4 | 1.554–2.053 秒 |
| 8 | 8 | 1.588–2.163 秒 |

首批维持 4 个活跃 MCP 读取任务的规划。复测脚本为 `scripts/linux/measure-mcp-pages.py`，从已验收的本轮执行脚本整理为可指定私人配置和输出文件的入口；整理后的 CLI 又在独立恢复副本上通过完整 13 次调用，生产账本未增加。[CLI 验证](measure-cli.json)。

## 已知限制

- 单轮缓存材料并发结果不等于持续吞吐、混合材料或模型任务并发能力。
- 本轮以协议客户端验证正式 HTTPS，未重复调用新 Agent 或模型；MCP9 的真实 Agent 验收见 [先前证据](../beta-mcp/README.md)。未进行浏览器视觉或实体设备验收。
- Tutor TC7／TC8 仍未完成；其真实读者和教学事件兼容范围不由 MCP10 代替。原 Core BookStructure 测试的 1 项失败仍保留，见 [整合记录](../integration-mcp-tutor-20261012.md)。
