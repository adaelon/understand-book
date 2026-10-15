# YY6 受控播放器、待机与长等待（2026-10-13）

YY6 已在本地完成。首页、书架接续和助手空态支持低频眨眼；桌面全屏问答在当前运行持续等待时翻页。首屏静态图、原状态文字和操作独立可用。下一片为 YY7 真实任务的一次完成反馈。范围见 [实施切片](../../切片方案-页页品牌融入与动效.md#yy6-受控播放器待机与长等待)。

## 实现

- [YeyeMascot.vue](../../../packages/web/src/components/YeyeMascot.vue) 新增 `motion: static | idle | waiting` 和 `active`。默认静态；欢迎静止 16 秒后播放一次 4 秒日常动画，播完再等 16 秒。阅读姿态先静态占位，持续等待 1.2 秒后循环 5 秒翻页动画。头像保持静态。
- `active ∧ 页面可见 ∧ 角色与视口相交 ∧ 未要求减少动态效果` 决定能否调度。宿主隐藏、滚出视口、后台或偏好变化时取消计时和资源请求，暂停并销毁播放器；返回时重新开始当前状态的间隔。卸载释放播放器、观察器和监听，迟到的加载结果不创建播放器。
- [yeye-assets.ts](../../../packages/web/src/yeye-assets.ts) 使用 Vite 的独立 JSON 资源 URL，播放时并行加载当前 JSON 与 `lottie-web@5.13.0`。原动画的 `approved-art` 图片引用映射到同画布的生产 WebP，保留 1254px 动画坐标。`idle.json` 69,728 字节、`reading.json` 53,491 字节；复用已有静态配图。动画、运行库或图片加载失败时保留静态图及宿主文字，不持续重试。
- [NetworkApp.vue](../../../packages/web/src/NetworkApp.vue) 将 `!libraryOpen` 显式传给已挂载的阅读器；接续读取、错误或正在操作时停止书架待机。[App.vue](../../../packages/web/src/App.vue) 合并阅读器可见性、阅读表面、专注状态与实际辅助区布局；宽屏辅助区按当前 CSS 的常驻列处理。
- `App.waitingTurnId` 只认当前聊天中已连接、无错误、结果仍在执行、处于排队／等待模型／运行且没有回答草稿的回合。`RightRail` 仅在 ≥1280px 的全屏问答中使用阅读插画；普通侧栏、手机及演示讨论沿静态头像与原进度。流式内容、停止、重连、保存、失败会退出翻页。
- Web 依赖与锁文件固定版本；[运行库 MIT 许可](../../../packages/web/public/licenses/lottie-web-MIT.txt) 随生产目录分发。美术和动画来源见 [production-v1](../../../assets/brand/yeye/production-v1/README.md)。

## 验证

| 验证 | 检出的具体失败与修正方向 | 结果 |
| --- | --- | --- |
| [组件测试](component-tests.log) | 计时提前加载、隐藏后播放、迟到请求复活、播放器／监听未清理、失败丢静态图、宿主状态未传递 | 83 项通过：YeyeMascot 14、RightRail 58、书架接续 11 |
| [生产构建](build.log) | Vue／TypeScript 合同错误、运行库／JSON／配图打包失败 | 多用户生产构建及类型检查通过 |
| [生产浏览器验证](browser-tests.log)、[定向复验](browser-retry.log) | 真播放器无图、提前加载全部动画、离开／返回后时序错误、书架覆盖与专注隐藏仍活动、运行文字或输入回归 | 29 个不同场景通过：7 个 YY6、10 个首页、12 个接续；整批通过 27 项，另两项在修正测试同步与工具栏操作后定向通过 |

[yeye-integration.spec.ts](../../../packages/web/playwright/yeye-integration.spec.ts) 加载真实生产首页与 `NetworkApp → App → RightRail`。验证先有静态图，16 秒后才请求运行库和当前 idle JSON；不会预载 reading／success。SVG 的日常与奶油书封阅读 WebP URL 均返回 200，许可可读取。网络记录：[1440px](home-network-1440.json)、[390px](home-network-390.json)。

生命周期覆盖滚出视口→返回、减少动态效果的初始值与动态切换、页面后台→恢复、书架覆盖→继续阅读、专注→退出、手机返回正文、长等待→流式回答、停止、断线恢复、保存中和保存失败。书架覆盖期间只有书架角色活动；返回后草稿保留。组件测试覆盖卸载与迟到响应清理；生产浏览器另外覆盖动画请求 503 的静态回退。浏览器用例未记录页面脚本异常。

已查看实际截图：全身形象与动画局部衔接正常，首页文字和卡片无遮挡，桌面等待保留准确阶段文字与停止入口，手机头像、等待文字和常驻输入可见。

- 首页：[桌面播放](home-playing-1440.png)、[手机播放](home-playing-390.png)。
- 阅读：[书架播放](bookshelf-playing.png)、[桌面全屏等待](waiting-fullscreen.png)、[手机空态](mobile-welcome-playing.png)、[手机等待](mobile-waiting.png)。

从仓库根目录复现；独立配置自动管理 4177 生产预览，当前测试服务已结束：

```powershell
pnpm --filter @understand-book/web test -- src/components/YeyeMascot.test.ts src/components/RightRail.test.ts src/NetworkApp.resumption.test.ts
$env:VITE_MULTI_USER = '1'
pnpm --filter @understand-book/web build
pnpm --filter @understand-book/web test:e2e --config ../../docs/performance/yeye-brand-yy6-20261013/playwright.config.ts
```

## 已知限制

- 本轮为本地实现，未提交、推送或发布；完成反馈按 YY7 实施，success 资源尚未引入产品。
- 浏览器使用 Chromium、固定接口事实和可控 EventSource 测试传输；后台切换通过页面可见性事件模拟，手机为视口模拟。实体手机和原生 WebView 待验。
- 使用原 PNG／矢量混合动画和同画布 WebP，未增加独立肢体骨骼。
- 构建保留原有 PDF 混合导入及大 chunk 提示；完整 lottie-web 包另有其表达式实现的 `eval` 构建提示。本轮使用的角色动画没有表达式。
