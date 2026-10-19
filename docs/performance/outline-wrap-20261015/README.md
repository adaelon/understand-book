# 目录长标题重叠修复

2026-10-15 18:24:16 HKT 已发布至 [正式站点](https://understandbook.top/)，沿用 `current-20261015`。目录 Grid 行按内容高度展开，长标题完整换行，列表继续独立滚动。

## 实现与验证

`packages/web/src/style.css` 的 `.outline-list` 增加 `grid-auto-rows: max-content`。原布局在有限高度内将目录行压到44px，超过行高的标题居中溢出；本次让标题及内边距共同决定行高。

`packages/web/playwright/fixtures/outline-follow.html` 支持注入目录样本；`outline-follow.spec.ts` 使用真实长标题和四级缩进，检查文字边界、滚动、搜索及当前项居中。修复前两种桌面尺寸均检出30项文字越界。手机样本只有两行，测试的多行前置条件由三行修正为至少两行。

命令：`pnpm exec playwright test --config packages/web/playwright.config.ts packages/web/playwright/outline-follow.spec.ts --workers=1 --reporter=line`。修复后8项通过，包含1280×900、1100×440、390×844的新增排版回归及原目录跟随、首尾边界和进度位置用例。

线上基于既有 release 源码只修改该CSS规则，类型检查与生产构建通过，见 [构建日志](build.log)。首次构建缺少已声明的 `lottie-web`，自动恢复源码后按锁文件补齐依赖并重新构建；原日志保留于 `build-first.log`。

正式页面登录后打开 `ai-agent-engineering`，三种尺寸均检查全部292项目录，结果见 [线上验证](public-verification.json)：

| 视口 | 文字越界项 | 学习机制标题高度 | 对应目录项高度 | 搜索及滚动 |
| --- | ---: | ---: | ---: | --- |
| 1440×900 | 0 | 60.47px | 71.66px | 通过 |
| 1100×440 | 0 | 60.47px | 71.66px | 通过 |
| 390×844 | 0 | 40.31px | 51.50px | 通过 |

页面脚本错误0。截图：[桌面](public-outline-1440.png)、[矮窗口](public-outline-1100.png)、[手机](public-outline-390.png)。首次线上脚本切换手机尺寸后停留问答页，未返回阅读即寻找目录入口而超时；补齐真实返回操作后完整通过，保留 `public-first.json`。

## 发布与恢复

- [发布回执](deployment.json)：`/opt/understand-book/releases/current-20261015`；服务 active，PID 101669保持，后端未重启。
- 候选产物与原HTML/CSS备份：`/opt/understand-book/acceptance/outline-wrap-20261015/`，原文件在 `before/`。
- 新入口使用 `reader-BQU03EsC.js` / `reader-Bsd8E_fs.css`；旧静态资源保留。首页及既有公共字体文件沿用。
- 回退时将 `before/index.html`、`before/home.html` 恢复至当前release的 `packages/web/dist-multi/`，将 `before/style.css` 恢复至 `packages/web/src/style.css`。无数据库变更，无需重启服务。
- 发布与验证脚本位于 `tmp/outline-wrap-20261015/`。本次未提交Git，工作区其他改动保留。
