# 《深入 Understand Book》发布记录

2026-10-15 21:27:57 HKT，《深入 Understand Book：从知识构建到阅读 Agent 的架构与实现》已发布至 [正式阅读器](https://understandbook.top/index.html)。

书籍 ID 为 `understandbook`，发布 ID 为 `01a13fbf-8230-7173-856d-1f388850f13f`。访问权仅授予 `admin`、`adaelon`；没有未来账号自动授权。复用当前完整书籍成果，现网程序仍为 `current-20261015`。

## 发布与验证

- 现网 `publish_book` 在独立目录试导入成功后执行正式导入。
- 停服前没有活动或未保存回答；通过 `reader_maintenance backup` 保留完整服务恢复点，导入后分别授权两个账号。停止到启动共约 8.57 秒，服务已恢复 active。
- 导入后读取正式授权表，结果恰好为 `adaelon`、`admin`，该书自动授权记录为零。
- 从服务器经正式 HTTPS 域名登录 admin，确认书库可见、正文可读、图片与本地源文件一致，Tutor 状态为 ready。
- 经正式 HTTPS 域名登录 reader，确认书库不可见，直接访问该书 manifest 返回 HTTP 403。
- 验证没有发起模型调用。

[发布回执](deployment.json)、[原生发布清单](publication.json)、[公网验证](verification.json)。

## 恢复位置

服务数据根为 `/opt/understand-book/data/multi-reader-to-tb-20261012`。发布前完整备份为 `/opt/understand-book/backups/understandbook-publication-20261015`。发布输入与远端回执位于 `/opt/understand-book/acceptance/understandbook-publication-20261015`。

## 验证限制

adaelon 的权限通过正式授权表确认；服务器没有保存其登录密码，本次未代登录。未授权账号的完整授权表检查涵盖所有账号，实际 HTTP 拒绝检查使用 reader。

本地 Python 的公网 TLS 连接曾中断，最终公网验证从服务器访问同一正式 HTTPS 域名完成。
