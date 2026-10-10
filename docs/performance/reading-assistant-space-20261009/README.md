# 阅读助手空间调整：发布与验收

2026-10-09 14:55 HKT 发布，15:00 HKT 完成 [正式阅读器](https://understandbook.top/) 公网验收。交互采用连续对话，设计见 [阅读助手空间调整](../../design/reading-assistant-space-20261009.md)。

## 发布内容

输入框默认一行，长问题输入时展开，主动收起保留草稿；运行时原发送位置显示停止。未完成任务显示单行状态，详情浮层显示要求与工作计划。问答/成果、工具、会话及全屏合并为一行顶部工具栏。快捷键和按钮遵循相同发送条件，连续发送后再次输入仍可展开。

现网 release 沿用 `/opt/understand-book/releases/adm10-20261008`。改动前 294 个阅读器运行源码文件与本地一致；本次更新 `packages/web/src/components/RightRail.vue` 与 `packages/web/dist-multi` 的前端静态资源。后端、运营后台及私人数据保持，服务 `understand-book-multi.service` 持续 active，发布前后 PID 均为 382648。

最终入口引用 `/assets/index-D-8Ot4bM.js`、`/assets/index-pA-drGep.css`。公网实际加载内容与通过验证的构建逐字节一致。回执：[最终发布](deployment.json)、[首次发布](deployment-initial.json)、[公网验收](live-verification.json)。旧资源继续保留。

## 验证结果

- RightRail 组件测试：42 项通过，覆盖任务详情、单一主操作及快捷键运行条件。
- 实际组件浏览器测试：3 项通过，覆盖 1440px 窄侧栏、390px 手机、任务继续/结束、草稿保留、阅读位置以及连续多行输入。
- 既有手机长回答与软键盘回归：2 项通过。
- 网络前端类型检查与生产构建：通过。
- 公网：真实登录、打开授权书籍、查看历史、收起草稿、后台入口与登录 API 通过；脚本错误 0，模型请求 0；验收账号会话已退出。

| 公网视口宽度 | 侧栏高度 | 对话正文高度 | 正文占比 | 收起输入高度 | 工具栏高度 | 横向溢出 |
| --- | --- | --- | --- | --- | --- | --- |
| 1440px | 844px | 731px | 86.6% | 42px | 51px | 0px |
| 390px | 791px | 678px | 85.7% | 42px | 51px | 0px |

## 回退

发布前原界面的 `index.html` 与 `RightRail.vue` 保存在 `/opt/understand-book/backups/reader-space-20261009T064517Z`。若需要恢复原布局，将这两个文件分别恢复到当前 release 的 `packages/web/dist-multi/index.html` 和 `packages/web/src/components/RightRail.vue`。旧 index 引用的资源仍在，无需重启服务或恢复数据库；浏览器刷新加载回退后的入口。

最终输入焦点修补前的中间版本另存于 `/opt/understand-book/backups/reader-space-20261009T065520Z`，该备份对应首次发布后的布局。

## 验收边界

公网布局数据来自没有未完成任务的实际阅读现场。有任务时的状态行、详情浮层、继续/结束行为由实际组件浏览器测试覆盖。线上验收没有启动模型任务，以免产生费用；运行与发送的交互由定向测试验证。
