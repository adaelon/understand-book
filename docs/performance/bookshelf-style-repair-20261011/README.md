# 书架脚本与样式配套修复

## 原因

2026-10-11 11:16 HKT 的教学活动展示登记修复生成了配套资源 `reader-CZ0ykojC.js` 和 `reader-CUvM77VD.css`，发布入口只替换了脚本，仍引用 `reader-DNabvP0n-loading-height-20261011.css`。

新脚本中的 NetworkApp 组件标记为 `data-v-808f600a`，旧样式的书架选择器只匹配 `data-v-bc7ea3bd`。书架容器、标题、网格和卡片布局因此全部失效，封面组件自身样式仍然生效，形成贴边标题、巨大封面和错位排列。加载检查区域同样失去匹配样式。

## 修复

将正式阅读器入口的样式引用改为当前脚本同次构建的 `reader-CUvM77VD.css`。该文件已发布到服务器，包含匹配的组件标记和 `.network-loading` 的 `height: 100%`。修复不需要重新构建应用。

后续发布必须使用同次构建的脚本、样式和依赖引用。修改组件内容会改变 Vue 的作用域标记，即使只改脚本逻辑，也不能保留上一构建的组件样式。资源 HTTP 200 不能证明组件样式生效。

## 验证

- 原正式入口与新样式入口使用同一正式构建脚本，以固定账号和八本材料替代接口响应，隔离真实账号与阅读操作。
- Chromium、WebKit 在 390px 和 1440px 下共四组复现原故障：网格缺失、边距缺失、卡片尺寸异常与首行错位。
- 切换配套样式后，两种浏览器在 320、390、844、1440px 下共八组通过：书架网格生效、至少 20px 外边距、卡片保持边界和同行对齐、页面无横向溢出，手机保持两列。
- 两种浏览器下加载动画占满 390×844 视口且居中；页面脚本错误与非预期接口请求均为零。

验证脚本：`tmp/bookshelf-style-repair-20261011/verify.cjs`；运行：`node tmp/bookshelf-style-repair-20261011/verify.cjs`。

证据：[本地验证](local-verification.json)、[修复前桌面](before-chromium-1440.png)、[修复后桌面](after-chromium-1440.png)、[修复后手机](after-chromium-390.png)。

## 发布状态

2026-10-11 11:49 HKT 经用户确认发布。正式阅读器入口已切换至配套样式 `reader-CUvM77VD.css`，沿用 `reader-CZ0ykojC.js`，后端服务未重启。

发布后通过正式网址完成 Chromium、WebKit 在 320、390、844、1440px 下共八组布局验证：网格、边距、卡片尺寸与同行对齐、手机两列及无横向溢出全部通过；加载动画铺满视口且居中，页面脚本错误与非预期接口请求均为零。

证据：[发布回执](deployment.json)、[公网验证](live-verification.json)、[公网桌面](live-chromium-1440.png)、[公网手机](live-webkit-390.png)。已打开的页面刷新后加载修复。

## 回退

发布前入口保存于 `/opt/understand-book/backups/bookshelf-style-20261011T034956Z/index.html`。需要撤销本次变更时，将备份入口原子恢复至 `/opt/understand-book/releases/ux-trace-20261011/packages/web/dist-multi/index.html`。回退会恢复本次已确认的脚本与样式不匹配状态；旧资源均保留。

## 验证边界

排版验证使用真实生产静态资源和固定接口数据；未登录真实读者账号或提交阅读、题目与模型请求。
