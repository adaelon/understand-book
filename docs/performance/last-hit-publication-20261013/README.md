# 补刀站位、默认材料授权与演示长图导出

2026-10-13 22:41 HKT 发布完成。入口：[阅读器](https://understandbook.top/index.html)。

《这一刀怎么补》（last-hit-positioning）已导入为默认发布，4 个现有账号获得访问权，未来账号创建时自动授予。演示工具栏“导出长图”保留当前 HTML 的完整布局、样式、参数和页面高度；预览与下载复用同一份 PNG。

## 验证

- 默认授权和 schema 12→13：2 项测试通过。
- 账号注册及历史 schema 升级：12 项回归通过。
- 图片分享：15 项组件测试通过。
- 长图浏览器链路：7 项通过，涵盖当前状态、保留现场、恢复失败、缺失资源、长页首尾及原生控件；实际成图位于[长图验证产物](../presentation-html-export-20261013/)。
- Web 类型检查与生产构建通过；Linux 同版后端 release 编译通过。
- 原生 CLI 隔离演练确认既有和未来账号均获得授权，见 [候选回执](candidate-verification.json)。
- 正式 HTTPS 登录、默认书库、正文、8 张图片、Tutor 状态及网页资源核对通过，模型调用 0，见[公网验证](live-verification.json)。

## 部署与恢复

新 release：`/opt/understand-book/releases/last-hit-default-grants-20261013`。数据根：`/opt/understand-book/data/multi-reader-to-tb-20261012`，schema 13。

发布前备份：`/opt/understand-book/backups/last-hit-publication-20261013`。旧 release 与配置原件保留。已接收新写入的 schema 13 数据不可用旧库直接覆盖，恢复流程见[发布说明](../../默认材料授权与补刀站位发布.md)。

- [部署回执](deployment.json)
- [原生材料发布清单](publication.json)
- [后端构建日志](build-production.log)

## 已知限制

公开范围为现有和未来的站内账号，沿用登录规则。可选正式教学素材尚未发布；Reader 与 Tutor 已就绪。实体手机和原生桌面保存延续原项目待验范围，本次完成的是 Web 发布与浏览器验证。
