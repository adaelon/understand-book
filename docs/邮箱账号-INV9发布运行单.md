# 邮箱账号 INV9 验收与发布运行单

2026-10-09（Asia/Hong_Kong）：**INV0–INV9 与 T15–T18 已发布，公网验收通过。** 正式入口 [understandbook.top](https://understandbook.top/)，后台 [内测码管理](https://understandbook.top/admin/invites)。

## 发布版本与配置

- 发布目录：`/opt/understand-book/releases/inv9-20261009`；Reader、Admin 和五个程序由同一工作区源码构建。源码包与当前 `crates/packages/apps/skills` 内容直接比较无差异，包含 T15–T18，见 [source-comparison.json](performance/inv9-20261009/source-comparison.json) 与 [构建回执](performance/inv9-20261009/multi-reader-build.txt)。Git HEAD 为 `4e0b68e`，此次包含未提交成果，不能仅用该 commit 重建。
- 服务：`understand-book-multi.service`，后端 `127.0.0.1:8788`；Nginx 同步切换到本版 `dist-multi`。生产数据根：`/opt/understand-book/data/multi-reader-inv9-20261009`，schema 9。
- 生产环境：`/opt/understand-book/multi-reader.env`，0600，保留模型路由和完整观测配置；发件身份 `noreply@account.understandbook.top`，Resend Sending access key 私有保存。`UNDERSTAND_BOOK_ORIGIN=https://understandbook.top`，邮件链接使用正式域名。
- 本次未提交 Git；未创建生产测试账号，候选注册、改密和绑定均使用独立副本。生产原账号仍用原密码登录，邮箱未自动绑定。

## 数据升级与保留

先用 ADM10 停止候选快照 `/opt/understand-book/acceptance/adm10-20261008/candidate-backup` 在 Windows 和 Linux 演练 schema 8 → 9。候选对比保留 16 张旧控制表、1,651 个文件，其中 176 个私人文件。证据：[Windows](performance/inv9-20261009/upgrade.json)、[Linux](performance/inv9-20261009/linux-upgrade.json)。

正式发布前确认在途或未保存任务为 0，停止原服务，用原版 `reader_maintenance backup` 取得最新生产快照，再用新版 `reader_maintenance restore` 恢复到新目录。比对通过后才更新服务、环境和 Nginx。原目录 `/opt/understand-book/data/multi-reader` 保留。

生产对比保留：4 个账号、10 个书籍发布、39 条授权、9 个默认发布、47 个工作区、4 个额度期、5 条额度调整、45 条调用费用及其报告；1,655 个文件，其中 183 个私人文件、4 个 SQLite 文件。旧账号邮箱为空，五张新增 INV 表为空。完整结果：[production-upgrade.json](performance/inv9-20261009/production-upgrade.json)，[发布回执](performance/inv9-20261009/production-release.json)。

比对工具 `scripts/linux/verify-inv-upgrade.py` 在两端离线时读取快照及新根，逐列比较旧控制表；发布路径仅允许按新根重定位。其他文件直接比较字节，SQLite 文件比较业务表。使用 immutable 读取防止给快照添加 WAL/SHM。

```text
reader_maintenance restore <schema8-snapshot> <new-root>
python scripts/linux/verify-inv-upgrade.py <schema8-snapshot> <new-root>
python scripts/linux/test-verify-inv-upgrade.py
```

工具 4 项回归通过：正确恢复、额度丢失、私人笔记变化、WAL 模式快照文件清单保持不变。Windows 首轮数据库释放问题已修正为显式关闭连接；Linux 首轮只读连接生成空 sidecar 的问题由 immutable 读取修正。

## 真实邮件与完整流程

Linux 独立候选根 `acceptance/inv9-20261009/{candidate-root,binding-root}` 分别承载新注册和老账号绑定，使用同一运营者收件邮箱；两者隔离，避免邮箱唯一约束冲突。浏览器通过本机 CONNECT 代理将 `understandbook.top:443` 定向到候选 Nginx，后端可信 Origin 为正式域名。候选的 Origin CA 证书由测试浏览器专用配置接受；生产复核直接走公共 DNS 并验证公开证书。

`scripts/linux/smoke-inv-release.mjs` 使用真实浏览器、Nginx、服务与 Resend，没有 API 拦截或模型调用。凭证经 `INV_CONFIG` 指向的私人文件提供，验证码和链接从实际收件箱取得，未读取数据库凭证代替收信。

1. 管理员生成并复制内测码，注册实际收信并完成；同一码再次申请被拒绝。新账号没有材料、额度或管理权限。
2. 管理员显式开通材料和额度；新账号打开 Reader 并保存私人笔记。
3. 个人设置修改密码，旧密码失效，同源 Reader 和后台窗口退出。
4. 找回邮件进入收件箱，链接使用 `https://understandbook.top/?account=reset-password#token=…`。页面读取后清除地址栏令牌，提交新密码后可登录同一账号，额度与笔记完整。
5. 老账号绑定邮件进入收件箱；绑定后 user_id、管理员身份、书籍、额度及私人记录不变，可用邮箱及原密码登录。

三类邮件均由用户确认在**收件箱**。候选浏览器 6 组通过、0 页面错误、0 模型请求：[candidate-browser.json](performance/inv9-20261009/candidate-browser.json)、[mail-receipts.json](performance/inv9-20261009/mail-receipts.json)。首次注册邮件曾使用回环候选地址，已改为正式域名；后续重置邮件及浏览器路径已实际验证。首轮额度烟测遇到客户端时钟领先服务端约 1 秒，测试授权开始时间改为当前时间前 60 秒，复用原注册账号继续，未更改业务逻辑。

独立候选故障演练使用本机 HTTP 接收器和假 key：提供者 422 返回 `ACCOUNT_MAIL_REJECTED`；立即重试返回 `ACCOUNT_MAIL_RATE_LIMITED`；等待 61 秒可再次请求；延迟响应返回 `ACCOUNT_MAIL_TIMEOUT`，已投递的链接仍可完成重置。没有改动生产邮件配置。证据：[failure-recovery.json](performance/inv9-20261009/failure-recovery.json)。

## 公网复核与 Tutor 范围

真实公共 DNS 和 TLS 下，原管理员登录、`/admin/invites` 深链接与刷新、静态资源 MIME/404、注册/找回页面、书架及打开原书通过。页面错误 0，模型调用 0：[production-browser.json](performance/inv9-20261009/production-browser.json)、[Reader 截图](performance/inv9-20261009/production-reader.png)。

T15–T18 后端与前端均已上线。现有 10 个封存发布均缺少新版 `publication.json.tutor_readiness`，公网实际返回“当前发布缺少 Tutor 必要接纳回执”。此次未改书籍版本、默认选择或授权。后续用户明确要求直接使用已有基础成品；源码已改为检查 Pass1、discourse 与 BookStructure，缺回执无需重建或重新导入，此修复已于 20:32:02 HKT 部署并通过公网复核，见 [Tutor 上线记录](performance/tutor-artifacts-deployment-20261009/README.md)。T19 真实教学体验、学习效果和成本验收仍待实施，见 [Tutor 方案](切片方案-Tutor预构建不阻塞教学.md)与 [T18 验证](performance/tutor-t18-20261009.md)。

## 恢复点与已知限制

- 恢复点：`/opt/understand-book/backups/inv9-20261009/`，含 `snapshot/`、旧 `service.unit`、`nginx.conf`、`multi-reader.env`；旧程序 `/opt/understand-book/releases/adm10-20261008` 和旧生产根均保留。运行证据在远端 `/opt/understand-book/acceptance/inv9-20261009/`。
- 回退须先停写并备份新版根，确认如何保留发布后的新增账号和业务写入，再用旧程序匹配 schema 8 快照或保留的旧根，恢复备份配置并检查 Nginx 后启动。不能让旧程序直接打开 schema 9，也不能用旧快照覆盖发布后的数据。
- 服务器发布后可用磁盘约 1.6 GB；已清理四个未在用旧版本的可再生编译中间目录，保留程序、用户数据和恢复点。后续构建前需处理空间。
- 注册/绑定表单刷新需要重新开始；重置页面清除令牌后刷新需重开邮件链接。更换已绑定邮箱尚未实现。邮件入口限流为进程内窗口。
- 既有两项 MU4 历史夹具失败、前端包体积提示仍见 [INV 方案 §9](切片方案-邮箱账号与一次性内测码.md#9-已知限制与上线依赖)。

Resend 配置参考：[域名验证](https://resend.com/docs/dashboard/domains/introduction)、[Cloudflare](https://resend.com/docs/guides/dns/cloudflare)、[API keys](https://resend.com/docs/dashboard/api-keys/introduction)。
