# 手机演示工具与来源图表发布

2026-10-17 **09:21:19 HKT** 已发布到 [understandbook.top](https://understandbook.top/index.html)，**09:22:25 HKT** 完成线上资源核对。

手机演示页将缩放、导出长图、来源、新窗口和评价集中到工具面板，主栏保留标题、记录与展开／收起；学习与讨论入口压缩为紧凑布局。来源弹层支持 EPUB 无分隔线表格、HTML 表格和按书籍出版版本解析的图片。

## 发布范围

现网基线 `/opt/understand-book/releases/teaching-scaffold-20261017` 与本地生产源码对比，仅有 `AgentPresentation.vue`、`RightRail.vue`、`SourceExcerpt.vue`、`md.ts`、`api.ts` 这 5 个文件不同。多用户正式构建通过，阅读器入口结构与线上相同，首页资源保持相同。

新增 `reader-Dn8YVSQY.js` 与 `reader-WvkzKM4B.css` 后原子替换阅读器入口；旧资源保留。主服务持续 active，PID **110819** 未变化，无后端重启或数据迁移。[发布回执](deployment.json) · [源码差异](source-comparison.json) · [构建日志](build-production.log)。

## 验证

发布前相关单元测试 97 项、Chromium 浏览器用例 17 项和 WebKit 用例 7 项通过，覆盖窄屏、低高度、缩放与 iframe 状态保留、来源表格和图片、反馈、导出与失败提示。

发布后从服务器访问正式域名，首页与阅读器均 HTTP 200，入口 `Cache-Control: no-store`、Cloudflare `DYNAMIC`。两入口共 9 项引用资源与正式构建逐字节相同；源站两份 HTML 与构建相同。新版程序包含工具面板及来源渲染修复。详见 [线上核对](live-verification.json)。本机 Python 首次公网 TLS 连接中断，改用服务器访问公网完成核对。

## 回退

旧入口和 5 个源文件备份在 `/opt/understand-book/backups/mobile-presentation-20261017T012105Z`。需要回退时恢复该目录的 `index.html` 到现网 `packages/web/dist-multi/index.html`，并将同名源文件恢复到发布回执记录的路径；保留新旧静态资源，不涉及用户数据。发布和验收脚本位于 `tmp/mobile-presentation-20261017/`，远端验收记录位于 `/opt/understand-book/acceptance/mobile-presentation-20261017/`。

## 已知限制

手机交互、表格和图片的浏览器验证使用受控数据；此次线上验收核对正式资源与服务状态，未操作真实用户会话。已经打开的页面需刷新后加载新版本。
