# 注册表单底部按钮修复

2026-10-09 21:41 HKT 发布至[正式注册页](https://understandbook.top/?account=register)。

## 原因与修复

应用根节点固定为可见窗口高度并禁止外层滚动。账号表单原有 `overflow: auto`，但高度随内容增长，超出窗口时底部按钮被外层裁掉。为 `AccountAccess.vue` 的 `.account-access` 增加 `max-height: 100%`，让滚动发生在窗口内；注册、找回密码和重置密码使用同一个表单容器。

## 验证

- 浏览器回归五项：原实现均因“返回登录”不在可见区域失败，修复后全部通过。覆盖 390×600、320×568 手机，1280×600 桌面，以及 390×320 下的找回与重置密码。
- 现有账号表单单元测试五项通过。
- 基于生产 `inv9-20261009` 源码，只加入这一行修复的类型检查、网络模式生产构建通过。
- 正式构建六种布局通过滚动、按钮完整显示与返回登录验证；注册页增加 671×832、22px 字号场景，横向溢出和页面脚本错误均为零。

证据：[本地正式构建验证](local-verification.json)、[本地手机按钮](local-register-mobile.png)、[发布回执](deployment.json)。

## 发布与回退

发布仅更新线上账号表单源码、网页入口和新脚本/样式，现网目录 `/opt/understand-book/releases/inv9-20261009`。前后服务 active、PID 401087，后端未重启。

新资源为 `/assets/index-Bnl5_5rZ.js`、`/assets/index-bLKArjUS.css`。原入口与源码保存于 `/opt/understand-book/backups/account-layout-20261009T134110Z`，旧资源保留。恢复该目录的 `index.html` 至当前 release 的 `packages/web/dist-multi/index.html`，恢复 `AccountAccess.vue` 至 `packages/web/src/components/AccountAccess.vue`，即可回退，无需重启服务。

## 公网验收

2026-10-09 21:46 HKT 公网验收通过。正式域名返回的 HTML、脚本和样式与已验证的发布构建逐字节一致。四种注册页布局及两种短窗口账号恢复页面均能滚到“返回登录”并成功切回登录；“发送注册验证码”在注册页滚动后完整显示，横向溢出为零。页面脚本错误零，未发起任何业务写请求。

证据：[公网验证](live-verification.json)、[线上手机按钮](live-register-mobile.png)。
