# 首页：工作区、视频与音乐

2026-10-24 更新并发布 Understand Book 首页。产品主线为「阅读、视频与音乐的学习工作区」，保留持续学习与动手体验。

## 页面与源码

- `packages/web/src/components/ProductHome.vue`：真实阅读首屏、目标工作区介绍、四个学习场景切换，以及截图原始尺寸查看。
- `packages/web/src/home.css`：沿用书页底色、橙色品牌与页页，适配桌面和手机。场景图完整显示，键盘支持左右、Home/End；大图关闭后回到触发按钮。
- `packages/web/home.html`、`src/home.ts`：同步标题和描述；保留已登录用户从根路径返回 Reader 的逻辑。
- `HomeLearningDemo.vue`：保留花园互动体验，移至真实产品介绍之后，更新章节编号。
- `packages/i18n/src/en.json`：新增 83 条英文文案。音乐按精选作品、看谱、片段与截图提问描述。

截图来自用户指定聊天 `01a16bd2-a346-7be2-8860-08ee7a8a4899` 的 `output/understand-book-article/assets/`；没有重绘或修改截图界面：

| 首页资源 | 原素材 | 尺寸 |
| --- | --- | --- |
| reading-workspace-20261024.jpg | 01-reading-workspace-20261024.jpg | 1914 × 877 |
| video-understanding-20261024.jpg | 06-video-understanding-20261024.jpg | 1280 × 720 |
| music-workspace-20261023.png | 05-music-workspace.png | 1500 × 1000 |
| course-workspace-20261021.png | 04-cs336-workspace.png | 978 × 725 |
| interactive-explanation.png | 03-interactive-explanation.png，同首页原有文件 | 1908 × 876 |

## 验证

- Web 完整构建通过；首页独立生产构建通过。
- `pnpm --filter @understand-book/web exec vitest run src/home.test.ts src/i18n-catalog.test.ts`：7 项通过。
- `pnpm --filter @understand-book/web exec playwright test --config playwright.home.config.ts`：12 项通过，包含 320/390/768/1000/1024/1440 宽度、锚点、场景切换、键盘、大图和原花园实验。
- `pnpm --filter @understand-book/web exec playwright test playwright/i18n.spec.ts --grep 'English home' --output test-results-home-en`：3 项通过，英文 320/390/1440 宽度、切换回中文与刷新。
- 最终独立生产包在中英两种语言、1440/390 宽度验证四个场景资源、完整图像、放大关闭、工作区图片、入口及横向溢出。结果见 `built-verification.json`；实际查看首屏、视频与音乐的桌面/手机截图。
- 公网同场景与根路径、登录入口验证见 `live-verification.json`，截图为 `live-*.png`。

## 发布与恢复

2026-10-24 20:28:55 HKT，仅追加 7 个首页资源并原子替换 `home.html`，现网仍为 `english-20261024`。服务 PID 34889、Reader `index.html` 与旧资源保持；无服务重启。发布详情见 `deployment.json`。

首页由 `tmp/build-home-workspaces-20261024.mjs` 独立构建到 `packages/web/dist-home-20261024`。本轮不将整个工作区构建覆盖到线上；远端运行目录的源码未覆盖，后续从远端源码重建时须带入本次首页源文件及英文文案，或以当前本地源码构建。

备份首页位于 `/opt/understand-book/acceptance/home-workspaces-20261024-122852/home.html`。将它先复制到同一现网静态目录中的临时文件，再原子重命名为 `home.html` 即恢复首页；保留旧资源，不需要重启服务。

## 已知事项

原内测群二维码注明 10 月 22 日前有效，已撤下。新的群入口待提供，页面说明「内测群入口更新中」，已有内测码注册、登录和 Windows 下载保留。
