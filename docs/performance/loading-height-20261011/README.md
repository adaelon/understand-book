# 加载区域高度修复

2026-10-11 10:24 HKT 发布，10:25 HKT 公网验证通过。阅读器登录检查期间的加载区域现在铺满页面，翻书动画水平、垂直居中。

## 修改

`packages/web/src/NetworkApp.vue` 的 `.network-loading` 增加 `height: 100%`。原区域直接位于非 Flex 布局的 `#app` 内，`flex: 1` 无法撑高，实测高度只有 112px。

以线上 `ux-trace-20261011` 的现有样式文件为基础，仅插入这一条声明，发布为新资源 `reader-DNabvP0n-loading-height-20261011.css`，原子切换阅读器入口的样式引用，并同步线上源文件。加载流程保持原样。后端服务未重启。

## 验证

浏览器延迟返回身份检查结果，以稳定观察加载区域；返回未登录结果后验证登录表单出现。页面及脚本、样式使用本地线上快照或正式站点资源。

- 修复前：Chromium、WebKit 在 390 × 844 下均复现区域高 112px、动画垂直偏移 −366px。
- 修复后及公网：两个引擎在 390 × 844、320 × 640、844 × 390、1440 × 900 下均铺满视口；居中偏差小于 0.02px，无横向溢出。
- 登录页切换通过，页面脚本错误为 0。

证据：[本地验证](local-verification.json)、[公网验证](live-verification.json)、[发布记录](deployment.json)、[手机截图](live-webkit-390.png)。

## 回退

备份目录：`/opt/understand-book/backups/loading-height-20261011T022436Z`。

将备份的 `index.html` 恢复到 `/opt/understand-book/releases/ux-trace-20261011/packages/web/dist-multi/index.html`，将备份的 `NetworkApp.vue` 恢复到该 release 的 `packages/web/src/NetworkApp.vue`。旧样式资源已保留，回退无需重启服务。
