# 章节导读 A、B 上线记录

2026-10-17 18:06 HKT 完成服务发布；18:09 HKT 补发手机长章名换行修复，随后公网复验通过。地址：https://understandbook.top/index.html 。

## 发布内容

版本为 `chapter-reading-ab-20261017`。以当时线上 `teaching-demonstration-20261017` 为基底，仅更新章节入口、章范围与续接、问题选择、原先表达—实际帮助—后续判断关联所需的 13 个源文件。当前线上其他功能、管理界面和公共演示资源随原版本保留。主服务、演示 worker 和 Nginx 页面目录均已指向新版本。

使用现有 schema14 数据，无数据库迁移。服务停服到本机恢复验收用时 16.01 秒。发布脚本整体还包含前置检查与公网验证，不能把该时间当作整个发布流程时长。完整数据备份约 335 MiB。

## 验证

- 复用切片 B 的 38 项相关回归与五轮真实章节验证；候选的 13 个源文件逐字节核对了冻结副本与已验证工作树。
- 新版本 Linux 正式构建通过，耗时 6 分 04 秒；五个正式程序均构建完成。
- 从线上前端源码组装独立发布副本，类型检查和多用户正式构建通过；章节入口的 8 项定向测试通过。
- 发布前、本机切换后、公网三次核验均通过：reader/admin 登录、授权书库、目录和正文读取、受限材料隔离、四项演示制作能力。前后授权书目一致。
- 新服务进程、worker 路径、Nginx 静态根目录与发布版本一致；发布 socket 权限为 0600。
- 源站 HTML 与实际发布文件相同；公网入口引用及其资源内容与构建结果相同。
- 实际公网浏览器以现有 reader 验收账号打开材料，在 1440×900 和 390×900 两种尺寸确认入口显示章名、可见且可操作，无页面运行错误。
- 上线验证的模型请求为 0。线上浏览器使用可操作性试点击，不发起新的教学请求；教学行为依据本次实现阶段保存的真实章节实验。

公网首次验收发现实际英文章名使手机按钮宽达 706.97 像素，超出 390 像素屏幕。已在 `TopBar.vue` 对章节按钮增加窄屏换行样式，重新通过类型检查和正式构建，再以先上传资源、后原子替换入口的方式发布。后端无需重启。复验按钮宽 348 像素，章名在菜单内换行。失败现场与修复后截图均保留。

## 恢复

- 旧程序：`/opt/understand-book/releases/teaching-demonstration-20261017`。
- 本次数据备份：`/opt/understand-book/backups/chapter-reading-ab-20261017`。
- 切换前配置：`/opt/understand-book/acceptance/chapter-reading-ab-20261017/previous-configuration-final`，含 systemd、Nginx 与演示 sandbox 配置。
- 程序回滚恢复上述旧配置并重启旧程序，继续使用当前 schema14 数据，避免覆盖上线后的新对话。数据备份用于数据恢复，不作为普通程序回滚的必经步骤。
- 手机样式补发前入口和组件另保存在同一验收目录的 `index-before-mobile-fix.html`、`TopBar-before-mobile-fix.vue`。

服务器原剩余约 288 MiB。为容纳独立构建与完整备份，将三个旧验收书库副本逐文件与保留快照比较，完全相同后释放重复副本；旧 mu7 测试运行环境压缩归档并用 tar 内容比较确认后释放原目录。生产数据与历史生产备份未删除。恢复位置见 [space-recovery.json](space-recovery.json)。上线后剩余约 1.02 GiB，见 [post-deployment.json](post-deployment.json)。

## 证据

- [首次服务发布回执](deployment.json)、[手机样式补发回执](mobile-deployment.json)、[最终公网资源核验](frontend-final.json)。
- [构建与验证说明](validation.json)、[Linux 构建日志](build.log)、[发布源文件清单](release-source.json)。
- [公网浏览器记录](public-browser.json)、[桌面截图](chapter-entry-public-1440.png)、[手机截图](chapter-entry-public-390.png)。
- [手机修复前记录](public-browser-before-mobile-fix.json)、[修复前截图](chapter-entry-public-390-before.png)。
- 实施脚本与最终冻结源码位于仓库 `tmp/chapter-reading-release-20261017/`；补发之后的完整源码包为 `source-final.tar.gz`。

## 验证边界

真实章节实验使用隔离模拟读者，未增加真人学习收益或长期保持结论。切片 A 原有的来源 UTF-16 定位测试失败仍按其实现记录单独保留，本轮未把它报告为已修复。
