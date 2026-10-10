# checkpoint_INV — 2026-10-09 20:05（Asia/Hong_Kong）

## 新鲜度自检

- 写入时最新 commit：`4e0b68e feat: integrate book structure, tutoring and multi-user reader`。
- 接手时对比 `git log -3`；本次以当前工作树源码构建，包含多项未提交成果，不能仅用该 commit 还原。
- 当前生产 `CONTROL_SCHEMA_VERSION = 9`，保留其他任务的未提交修改。

## 当前在做什么

**INV0–INV9 已完成并上线；用户要求的 T15–T18 已随同版发布。** 注册、绑定、密码找回三类真实邮件均进入用户收件箱；完整候选流程、失败恢复和公网复核通过。

## 下一步（可直接接手）

1. 用户实际使用：打开 `https://understandbook.top/admin/invites` 生成内测码；受邀者在正式网站注册并输入邮箱验证码，管理员分别开通材料与额度。
2. 老账号在个人设置首次绑定邮箱；原用户名/密码继续有效，邮箱不会因候选验收自动绑定到生产账号。
3. Tutor 后续：按 `SESSION_CHECKPOINT.md` 接手 T19；从具有基础接纳回执的完整原工作区重新导入需要启用教学的旧书，不能直接给封存发布伪造 ready。
4. 如需回退，先读取发布运行单“恢复点与已知限制”，停写并保全新版新增数据，再恢复匹配旧程序的 schema 8 根；不可直接让旧程序打开 schema 9。

## 现网与证据

- 正式入口：`https://understandbook.top/`；后台 `https://understandbook.top/admin/`。
- release：`/opt/understand-book/releases/inv9-20261009`；同版 Reader、Admin 和五个程序，源码包与当前业务源码直接比较无差异，包含 T15–T18。
- 数据根：`/opt/understand-book/data/multi-reader-inv9-20261009`；服务 `understand-book-multi.service`；原 `/opt/understand-book/data/multi-reader` 保留。
- 恢复点：`/opt/understand-book/backups/inv9-20261009/`，含最新生产 schema 8 快照及旧 service/Nginx/env；旧版 `adm10-20261008` 保留。
- 邮件：Resend，发件 `noreply@account.understandbook.top`；凭证在服务器私有 `multi-reader.env`（0600），链接 Origin 为正式域名。
- Windows/Linux schema 8 → 9 演练通过；比对工具 4 项回归通过，检测额度或私人记录丢失，immutable 读取保持快照清单不变。
- 生产停写备份、独立新根恢复后比对：4 账号、10 发布、39 授权、47 工作区、4 额度期、5 额度调整、45 调用费用与报告保留；1,655 文件，其中 183 私人文件、4 SQLite 文件。五张新增 INV 表为空，旧邮箱保持未绑定。
- 候选真实浏览器 6 组通过：发码/注册/重复码拒绝/初始无授权；显式开通后阅读及私人笔记；改密和全部旧窗口退出；邮件找回及原记录；老账号绑定并保留身份、授权、额度和私人记录。
- 注册、找回、绑定邮件均由用户确认进入收件箱；重置链接使用正式域名并完成操作。独立候选副本使用同一测试邮箱，生产未创建测试账号或修改原密码。
- 候选故障演练：提供者拒绝、立即重试限流、61 秒后重试、发送超时且已投递链接仍可重置全部通过；使用假 key，生产配置未改动。
- 公网公共 DNS/TLS：原管理员登录、内测码深链接/刷新、静态资源 MIME/404、注册/找回页、书架/打开原书通过；0 页面错误、0 模型调用。
- 完整结果：`docs/performance/inv9-20261009/`；实际输入、路径、操作与限制见 `docs/邮箱账号-INV9发布运行单.md`。
- INV1–INV8 已有服务端 50 项、Reader 30 项、后台 15 项、内置壳 2 项、浏览器 4 组通过；命令及覆盖见 INV 方案第 8 节。

## 未提交 / 未完成

- INV0–INV9 均为未提交成果；另有 ADM、Tutor、Reader 等修改，保留原状态。本轮完成部署，未创建 Git commit。
- T15–T18 程序已上线；现网 10 个封存发布均缺少 Tutor 必要接纳回执，新版教学入口返回准备中。需从完整原工作区重新导入；T19 真实教学效果和成本待验收。
- 三类真实邮件验证在同版隔离候选执行，公网验证使用原账号；正常用户现在应在正式域名页面自行输入验证码或新密码。
- 注册/绑定表单刷新需重新开始；重置页面清除令牌后刷新需重开邮件链接。更换已绑定邮箱未纳入；邮件限流窗口为进程内。
- 既有两项 MU4 Host 历史夹具因 SESSION_EVENT_WRITE_REQUIRED 待适配；前端主包大于 500 kB 提示保留，详见 INV 方案第 9 节。
- 服务器可用磁盘约 1.6 GB，后续构建前需处理空间；旧程序、业务数据和恢复点均保留。

## 冷启动读序

1. [INV9 发布运行单](docs/邮箱账号-INV9发布运行单.md)全文；[INV 方案](docs/切片方案-邮箱账号与一次性内测码.md)第 7 节 INV9、第 8–9 节。
2. [ADR-0159](docs/adr/0159-email-accounts-and-single-use-beta-invites.md)、[架构](docs/架构.md)顶部 INV 节、[代码链路](docs/代码链路.md)顶部 INV9 条目。
3. [生产发布](docs/performance/inv9-20261009/production-release.json)、[数据比对](docs/performance/inv9-20261009/production-upgrade.json)、[候选浏览器](docs/performance/inv9-20261009/candidate-browser.json)、[公网浏览器](docs/performance/inv9-20261009/production-browser.json)、[失败恢复](docs/performance/inv9-20261009/failure-recovery.json)。
4. [升级比对](scripts/linux/verify-inv-upgrade.py)、[比对回归](scripts/linux/test-verify-inv-upgrade.py)、[真实邮件烟测](scripts/linux/smoke-inv-release.mjs)、[Linux 运行说明](docs/Linux多人阅读-现网运行说明.md)。
5. 如接手 Tutor：先读 [主 checkpoint](SESSION_CHECKPOINT.md)，按其冷启动读序执行 T19。

## 本会话决策摘要

- 邮件与验收均使用正式域名；候选浏览器通过专用代理定向隔离副本，生产切换后用户直接在网站完成验证码与改密操作。
- 同版部署 INV 与 T15–T18；生产先停写备份，再恢复到独立 schema 9 新根，数据比对通过后切换；见发布运行单。
