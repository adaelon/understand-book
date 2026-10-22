# SESSION_CHECKPOINT — Search/Fetch、分账与缓存已上线

用户授权的发布已完成。2026-10-21 14:01:20 HKT，网站与后台切换至 `/opt/understand-book/releases/search-cache-20261021`，PID 520445，schema 20，维护窗口 27.63 秒。

- Search Key 显式复用现有官方 DeepSeek Key，Search/Fetch 费率各 ¥0.005，QPS 100/25；模型渠道保持。
- 34 个范围内文件、9 个已发布 Reader 文件；其他未发布改动排除。安装包 0.3.1 原字节保留。
- Linux 6 程序构建、Reader/Admin 构建与6项费用测试、隔离迁移、最新生产数据保留、静态入口与公网费用页面全部通过。
- 实际工具链：tool.search 两次、web.search 一次、web.fetch 一次；Search/Fetch HTTP200。最终模型5次 ¥0.108462（含后台 memory_review ¥0.0068）、搜索 ¥0.005、Fetch ¥0.005，总 ¥0.118462，7条全结清、未知0。专用账号 search-release-20261021 已停用，记录保留。
- 最终费用以 `final-accounting.json` 为准；`live-smoke.json` 的前台完成快照早于后台记忆审阅，因此是 ¥0.111662。浏览器已核对最终5/1/1分类和金额。
- 正式证据、已知限制、费率有效期及恢复路径：`docs/performance/web-search-jsonl/release-20261021/README.md`。
- 生产备份：远端 `backups/search-cache-20261021/mutable.tar.gz` 与 `backups/search-cache-20261021-publications/published`；本机 `tmp/search-release-20261021/production-backup/` 保存可变数据和私有原配置。不可变材料快照为同机硬链接；旧版不得打开schema20或覆盖上线后数据。
- 构建遗漏的crate内可见性依赖已补齐；升级验收排除既有维护时间戳；脚本使用默认视频产生的无费用拒绝已记录，最终改为已有文本材料且未改产品约束。所有临时验收副本已清理，原版本和备份保留。

后续按用户要求，Windows 0.3.2 已于 2026-10-21 15:34:07 HKT 发布，4,899,996 字节。真实 WebView2 本机阅读、离线读取、线上 Reader 入口及返回本机书库通过；公网两种下载地址与候选字节一致；服务 PID 不变。本轮付费调用 0。详见 `docs/performance/windows-release-032-20261021/README.md`。

当前无剩余发布工作。未提交或推送 Git。
