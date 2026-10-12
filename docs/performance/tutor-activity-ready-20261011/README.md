# 对话教学活动展示登记修复

2026-10-11 11:16 HKT 发布，11:18 HKT 公网资源验证通过。

后续发现本次发布只切换脚本、保留旧样式，导致 NetworkApp 组件作用域标记不匹配，书架和加载区域布局失效。原资源可达性验证未覆盖实际排版。原因、配套样式修复与浏览器验证见[书架样式修复](../bookshelf-style-repair-20261011/README.md)。

## 原因与修复

普通对话入口 `RightRail.vue` 未向 `TutorActivities.vue` 传入 `ready`。Vue 将省略的 Boolean 参数转为 `false`，因此 `markDisplayed` 在题目进入视口后仍提前返回，没有登记 `Displayed` 事实。服务端拒绝没有展示记录的回答与提示操作；多人网站将校验错误统一显示为 `Invalid request`。

`TutorActivities.vue` 使用 `withDefaults` 将 `ready` 默认为 `true`。演示入口显式传入 `false` 时仍等待演示就绪。

## 验证与发布

- `TutorActivities.test.ts` 修复前两项失败，修复后三项通过：普通对话题目登记展示后提交回答、获取提示；演示明确未就绪时等待，变为就绪后登记。
- 前端 `vue-tsc --noEmit` 通过。
- 从线上 `ux-trace-20261011` 取得源代码快照，仅修改题目组件的默认值，Vite 生产构建成功。
- 阅读器入口仅切换主脚本至 `/assets/reader-CZ0ykojC.js`，保留现有入口内容和加载区域样式引用。旧资源保留，后端进程保持 `513730`，用户数据未迁移。
- 公网阅读器入口引用新脚本；主脚本与构建产物逐字节一致，编译后的题目组件包含 `ready` 默认值；入口引用的脚本与样式均返回 200。Cloudflare 在 HTML 中注入站点统计脚本，入口按本应用资源引用比对。

发布回执：[deployment.json](deployment.json)。公网验证：[live-verification.json](live-verification.json)。

## 回退

备份目录：`/opt/understand-book/backups/tutor-activity-ready-20261011T031621Z`。将其中 `index.html` 原子恢复至线上阅读器入口，并恢复 `TutorActivities.vue` 源文件即可；旧脚本仍在原位置。

## 已知限制

已打开的页面需要刷新后加载新脚本。本次功能验证为组件回归测试，公网验证覆盖发布资源；未代替用户提交真实练习答案。
