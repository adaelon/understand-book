# 命令行书籍热发布 — 2026-10-15

用户确认先支持命令行不停服发布，并授权 Linux 验证通过后直接上线。该任务已完成。

## 当前状态

2026-10-15 22:44 HKT 上线 `online-publication-20261015`，schema14，服务 PID4476。22:48 HKT 实际在线发布与权限验收通过，热发布前后 PID4476 不变。

《深入 Understand Book》新默认发布为 `understandbook / 01a14006-1291-7dd1-8641-9c4a18148419`，只授权 admin、adaelon，无未来账号自动授权；原发布和原阅读引用保留。操作编号 `understandbook-online-20261015-v1` 已完成，再用该编号只返回原结果。

本地在线发布3项、书库15项、schema相关22项通过。Linux4项通过；正式 server 与全部维护工具构建通过。正式 HTTPS 登录、书架、正文、图片、Tutor ready、reader 拒绝访问通过。模型调用0。

首次启用停启15.06秒；后续发书不需要停服。数据根沿用 `/opt/understand-book/data/multi-reader-to-tb-20261012`。完整备份 `/opt/understand-book/backups/online-publication-20261015`。旧程序及静态资源目录 `current-20261015` 被新 release 引用，必须保留。

## 证据与操作

- `docs/书籍不停服发布.md`：命令及行为。
- `docs/performance/online-publication-20261015/README.md`：部署、验收、恢复点和已知问题。
- 同目录 `deployment.json`、`hot-verification.json`、`publication.json`、`test-bounded.log`、`build.log`：原始证据。
- `tmp/online-publication-20261015/release.py`：上传、执行和取回证据；其 `deploy.py` 已执行成功，不能再次运行。

SSH恢复后构建成功，过程中握手仍偶有断开。服务器约4GiB内存、无swap。用户随后要求清理磁盘：23:01 HKT清理8个release的32个Rust编译缓存目录，释放4.07GiB，目前可用4.30GiB、使用率89%。PID4476不变，新旧程序、资源、用户数据和备份保留，公网入口验证通过。证据 `docs/performance/online-publication-20261015/disk-cleanup.json`。

其他会话的插件、BookStructure 和文档改动均保留。
