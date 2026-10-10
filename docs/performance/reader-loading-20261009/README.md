# 无文字翻页加载动画：发布与验收

2026-10-09 15:26 HKT 发布至[正式阅读器](https://understandbook.top/)，15:32 HKT 公网验收通过。动画采用用户确认的轻翻书页设计，56px 图形、1.85 秒一轮，加载区域没有可见文字。

## 发布内容

入口 HTML 内嵌启动动画与样式；正式样式提前下载，加载结束后挂载界面。登录恢复、初始书架、工作区和阅读回顾加载使用同一书页图形。内容就绪立即显示正式页面；已可读正文的历史与画像继续后台加载。减少动态效果时显示静态书页。

现网沿用 `/opt/understand-book/releases/adm10-20261008`。更新六个前端源码文件和网页入口，新资源为 `/assets/index-DUvK8rGY.js`、`/assets/index-B-e5NHGq.css`。发布前后 `understand-book-multi.service` 均 active，PID 为 382648。

设计见[加载动画](../../design/reader-loading-20261009.md)，发布回执见[deployment.json](deployment.json)。

## 验证结果

- App 启动、NetworkApp 恢复与阅读回顾：25 项测试通过。
- 类型检查及网络前端生产构建：通过。
- 正式构建浏览器：前端脚本和样式尚未下载时动画正常播放；样式等待、样式失败、身份恢复与初始书架切换通过；减少动态效果使用静态图形。
- 1440px 桌面、390px 与 320px 手机：横向溢出与居中偏差均为 0。
- 公网：入口、脚本和样式与通过验证的构建逐字节一致；启动与身份恢复动画、真实登录、打开授权书籍、正文就绪及刷新恢复通过；页面脚本错误 0，问答模型请求 0，验收会话已退出。

证据：[正式构建验证](local-verification.json)、[公网验证](live-verification.json)、[桌面首屏](live-startup-1440.png)、[手机首屏](live-startup-390.png)、[工作区加载](live-workspace-loading.png)。

## 回退

原网页入口与五个已有源码文件保存于 `/opt/understand-book/backups/reader-loading-20261009T072629Z`。恢复该目录的 `index.html` 至当前 release 的 `packages/web/dist-multi/index.html` 即可恢复原网页版本，旧静态资源已保留。源码回退将备份的 `source/index.html`、`App.vue`、`NetworkApp.vue`、`main.ts`、`vite.config.ts` 分别放回原路径；新增 `LoadingAnimation.vue` 在原页面中没有引用。

网页回退无需重启后端服务。
