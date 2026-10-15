# 内测 MCP 发布与恢复运行单

2026-10-12（Asia/Hong_Kong）。**MCP0–MCP10 已完成并上线。** 08:53:21 切换完成，08:57:06 完成公网与账本复核；发布窗口约 16.7 秒。完整证据见 [MCP10 验收记录](performance/mcp10-20261012/README.md)。

## 本次发布

- 应用源码：main 合并提交 `8497f29a6ab1315af89f65094033b03d767e999d`，包含 Tutor TC0–TC6、Reader 改进和 MCP0–MCP9。
- 目标 release：`/opt/understand-book/releases/mcp10-20261012`。Reader、Admin、server、manage_reader、publish_book、reader_maintenance、presentation_worker 从同一提交源码构建。
- 新数据根：`/opt/understand-book/data/multi-reader-mcp10-20261012`；控制库 schema 12。
- 原 release：`/opt/understand-book/releases/ux-trace-20261011`；原数据根：`/opt/understand-book/data/multi-reader-main-12b765b-20261010`，schema 9。
- 服务：`understand-book-multi.service`，`127.0.0.1:8788`，运行用户 `understand-book`。
- 正式入口：<https://understandbook.top/>；MCP：<https://understandbook.top/mcp>；本人接入页：<https://understandbook.top/?account=mcp>；后台：<https://understandbook.top/admin/>。
- 生产恢复点：`/opt/understand-book/backups/mcp10-20261012`；执行脚本和原始日志：`/opt/understand-book/acceptance/mcp10-20261012`。

模型、邮件凭证仍在原 `multi-reader.env`，0600。私人验收配置包含账号密码，仅存本机忽略目录和服务器 0600 文件，不进入源码或公开证据。

## 构建与发布步骤

1. 从上述提交导出源码，在最终 release 目录构建，保证编译进程序的运行时路径指向该目录。源码包需包含根 `tsconfig.json`、workspace 配置、crates、packages、apps、skills、agents、scripts 和引用资产。执行现有 `scripts/linux/build-multi-reader.sh`，设置 `VITE_READER_OPERATOR_CONTACT=19847595883`；本次 Linux 依赖缓存齐备，使用 `npm_config_offline=true`。
2. 在独立目录恢复 schema 9 快照，用新版升级到 schema 12。执行 `verify-mcp-upgrade.py`，比较旧控制表每列每行、模型额度／费用／收款、私人文件和内部 SQLite 表；发布包路径只允许根目录重定位。升级不得凭空创建 MCP 业务事实。
3. 用实际旧二进制尝试打开升级副本；再用旧二进制恢复 schema 9 快照并登录读取。用新版启动升级副本完成 MCP 调用，在副本中登记明确标记为模拟的调用包；停服备份、恢复 schema 12，再比较消费、凭证、赠送、调用包与收款，并登录读取恢复结果。模拟收款仅存在于隔离副本。
4. 发布前确认生产 `run_admissions` 中 preparing／queued／claimed 或 `unsaved != 0` 的记录数为 0。先让正式 Nginx 入口短暂返回维护状态，再复核在途记录，停止服务。原程序执行最新生产备份，新程序恢复到新独立根，完整比较通过后才继续。
5. 更新 systemd 的 release、环境文件的数据根、presentation worker 路径和 Nginx 静态根。保留原版本静态资产以支持已打开的标签页。MCP 精确代理块使用仓库 Nginx 模板，保留 Authorization、MCP-Protocol-Version、MCP-Session-Id 和 Origin；普通 `/api/` 继续原认证方式。域名、TLS 证书、首页和旧 IP 重定向沿用现网配置。
6. `nginx -t`、新服务启动、回环匿名认证 401、实际进程的程序路径均通过后恢复正式入口。开放流量前失败可恢复原配置和原 schema 9 根；开放流量后先保留新业务事实，按下一节处理。
7. 经公共 DNS 和正式 TLS 执行 `smoke-mcp-release.py`：先要求 `/api/auth/me` 的演示能力为 `reason=ready` 且 authoring／plot／animation／preview 全部开启，再验证原账号／后台登录、材料、模型额度、11 个工具、协议及代理请求头、原文与 UTF-16 来源、1／4／8 路并发和精确扣次。管理员赠送 15 次标明用途的额度，正常完整运行消费 15 次，原有免费余额和付费次数保持。结束时撤销验收凭证、退出两个测试会话，再核对账本及前后台静态资源。

`smoke-mcp-release.py` 的私人 JSON 输入包含 `origin`、`reader/admin: {username,password}`、唯一 `operation_id`、`book_id` 和已知能命中的 `query`。该脚本会赠送次数、创建并撤销凭证；同一操作号已存在时停止，重跑须使用新的操作号并确认上次消费。`transport_url` 只用于明确选择的隔离回环服务，正式验收省略此字段。

另用 `measure-mcp-pages.py` 补测完整的 8,000 字符页面，采用相同配置格式和新的操作号；该轮赠送、消费各 13 次。本次正式验收两轮合计赠送与消费各 28 次。

## 备份与恢复

后续生产备份使用 **schema 12 兼容的** `reader_maintenance`。先停止写入并关闭服务，再执行：

```bash
release=/opt/understand-book/releases/mcp10-20261012
data=/opt/understand-book/data/multi-reader-mcp10-20261012
# snapshot 和 restored 必须换成本次新建、独立且尚不存在的目录。
snapshot=/opt/understand-book/backups/mcp12-NEW-DATE
restored=/opt/understand-book/acceptance/mcp12-restore-NEW-DATE
"$release/target/release/reader_maintenance" backup "$data" "$snapshot"
"$release/target/release/reader_maintenance" restore "$snapshot" "$restored"
PYTHONPATH=/opt/understand-book/acceptance/adm10-20261008/python-deps \
  python3.12 "$release/scripts/linux/verify-mcp-upgrade.py" "$snapshot" "$restored"
```

现网 Python 链接 SQLite 3.34，不能读取 MCP 的 STRICT 表；比较工具在该环境使用已安装的 pysqlite3（SQLite 3.46.1）。SQLite ≥ 3.37 的 Python 直接使用标准库。恢复工具本身使用 Rust 内置 SQLite。所有根目录都离线时才运行比较；工具使用 immutable 只读连接，避免给封存快照生成 WAL/SHM。

成套备份包括 control.sqlite、用户私人根、全部发布包和根内配置。恢复到独立新目录后，再检查外部模型／邮件环境、worker 路径和文件归属，启动隔离实例核对账号、材料、余额和收款，最后在停写窗口切换数据根。保留原根和本次备份。

### 演练执行器隔离

恢复副本会保留 `presentation-sandbox.json`，其中的 `instance` 也原样保留。每个独立运行的实例必须在启动前改为独立名称，例如 `mcp10-measure`；生产的 `reader` 不能给演练复用。该值同时决定 `/var/tmp/ub-render-<instance>`、systemd slice 和待清理任务，不是只用于日志的标签。独立数据根与端口不能隔离这些资源。

先完成离线数据保留比较，再修改副本的执行器配置；不要修改备份。实例名限 1–32 个 ASCII 字母、数字或连字符。副本配置指向对应 release 的 worker，使用实际服务账户启动与验证。制作验收需要单独实例的真实 write／preview／deliver；原文与 MCP 验收不能替代制作验收。2026-10-13 已修正服务器历史 `rehearse.py` 与 `check-measure-cli.py` 的启动配置，故障与修复见 [演示环境恢复记录](performance/presentation-recovery-20261013/README.md)。

**不能把旧二进制直接切到 schema 12。** 旧版本的实际行为由本次演练记录。需要回到 schema 9 时，旧二进制只能使用兼容的 schema 9 成套恢复点。发布后若已有新账号、聊天、笔记、赠送、收款、调用包或消费，旧快照不含这些事实；不得用旧备份覆盖或丢弃当前根。先停写并以新版完整备份，优先修复当前版本或发布仍支持 schema 12 的版本。任何需要降回 schema 9 的操作都必须先明确保留、迁移和核对新增事实的方法。

## 验收记录

- Windows／Linux 比较工具 5 项针对性测试通过；Linux Reader、Admin 和五个程序构建通过。
- schema 9 → 12 比较通过；schema 12 → 12 恢复保留凭证、15 条消费、赠送、1 个模拟调用包及其收款。恢复后登录与余额／收款 API 一致。旧程序打开 schema 12 返回 `CONTROL_SCHEMA_INCOMPATIBLE`、退出码 2；旧 schema 9 快照恢复后原账号可读 10 个发布。
- 最新生产快照保留 4 个账号、10 个发布、39 条授权、67 个工作区、4 个模型额度期、280 条模型费用及报告；1,738 个文件中包含 265 个私人文件、4 个 SQLite 文件。实际逐行／逐文件比较见 [production-preservation.json](performance/mcp10-20261012/production-preservation.json)。
- 正式 HTTPS 完成无 Origin Bearer、11 工具、通知／错误协议、原文与 Reader UTF-16 来源核对；首页、接入深链接、后台及资源版本匹配，原书目录和模型额度与发布前一致，PDF Range 和 Tutor 就绪接口正常。
- 公网完整页每次 8,000 字符、37,738 字节；1／4／8 路全部成功，最大耗时分别 1.641／2.053／2.163 秒，包含客户端网络与 TLS。首批仍按 4 个活跃任务规划。
- 生产账本恰有 28 条免费消费（检索 1、原文 27），2 次赠送共 28 次，reader 剩余仍为 1,000 次；付费消费 0、新增模型费用 0、正式收款 0。两个测试凭证已撤销，测试会话已退出。

## 已知限制

- 首批仍按 4 个活跃 MCP 读取任务规划；1／4／8 路验收只测本次材料、响应大小和网络，不构成持续吞吐承诺。
- Tutor TC7／TC8 的真实读者、工程矩阵和旧教学事件兼容验收尚未完成；此次应用发布不等于这些切片完成。
- 整合时保留 1 项原有 Core BookStructure 路线测试失败，详见 [整合记录](performance/integration-mcp-tutor-20261012.md)。MCP10 不修改该构建路线。
- 本轮不调用模型，不新增正式收款；MCP9 的新 Agent 阅读验收与本轮 HTTPS 协议验收分别记录。Computer Use 保持停止，网页验收以资源和接口为准。
