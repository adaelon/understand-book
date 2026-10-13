# YY5 首页情境融合（2026-10-13）

YY5 已在本地完成。首页由产品价值、抱书页页和两张连续学习情境卡组成首屏；手机依次呈现价值与入口、页页、第一次学习、接着往下学。下一片为 YY6 受控播放器、待机与长等待。范围见 [实施切片](../../切片方案-页页品牌融入与动效.md#yy5-首页情境融合)。

## 实现

- [ProductHome.vue](../../../packages/web/src/components/ProductHome.vue):`.home-hero-companion / .home-brand-symbol` — 复用静态 `YeyeMascot`，首屏使用日常抱书形象，导航使用 32px 专用头像。角色旁标明“页页 · 陪你把一本书读懂”，图片保持装饰语义。
- [home.css](../../../packages/web/src/home.css):`.home-hero-illustration` — 用随内容撑开的网格组织角色、两张卡片与图注。桌面页页在卡片左侧、视线朝向卡片；卡片保留轻微倾斜，前后内容完整可读。
- 1440px 下全身图占位 220px；1001–1100px 为 160px；1000px 及以下价值文案与情境组纵向排列。640px 及以下页页与名字一行、两张卡片顺序向下展开，卡片取消倾斜；全身图占情境组宽度的 62%，上限 220px。390×844 下主行动及完整角色均在首屏内。
- 互动示例仍由 `HomeLearningDemo` 提供猜想、围栏尝试、反馈、条件变化与接续记录；原登录、内测注册、截图放大和页面锚点继续使用。

## 验证

| 验证 | 检出的具体失败与处理 | 结果 |
| --- | --- | --- |
| [开发页面浏览器回归](browser-tests.log) | 角色或卡片遮挡、窄屏裁切、阅读顺序颠倒、开始入口不可操作时修首页接入与布局；示例流程失败时定位受影响入口 | 10 项通过 |
| [Web 生产构建](build.log) | Vue／TypeScript 合同错误、资源导入或打包失败时修引用与类型 | 类型检查与构建通过 |
| [生产页面浏览器回归](production-browser-tests.log) | 构建后的角色图片缺失、样式差异、图片放大或示例操作失效时修生产资源与入口 | 同一组 10 项通过 |

[product-home.spec.ts](../../../packages/web/playwright/product-home.spec.ts) 新增 6 个情境融合场景，覆盖 320／390／768／1000／1024／1440px。断言图片完成解码、文案／角色／卡片互不遮挡、两张卡片先后顺序、手机阅读顺序、无横向溢出、主行动键盘进入体验、内测锚点及继续阅读地址。1440×900 和 390×844 另断言主行动及完整角色在首屏内。

既有 4 项继续覆盖两种宽度的猜想→尝试→反馈→条件验证→下次接续、跳过步骤不虚构尝试、刷新清理示例状态、四种宽度的示例布局和产品截图放大。开发与生产各运行一次，共 10 个不同场景，页面未出现脚本错误。

已查看生产首屏及完整情境截图：角色、两张卡片、产品价值和入口层级清楚，奶油书封与暖纸底边缘正常。桌面两张卡片完整呈现；手机角色先于两张卡片，第一次学习和下一次学习内容均完整。

- 首屏：[1440px 桌面](home-viewport-1440.png)、[390px 手机](home-viewport-390.png)。
- 完整情境：[1440px 桌面](home-scene-1440.png)、[390px 手机](home-scene-390.png)。

从仓库根目录复现：

```powershell
pnpm --filter @understand-book/web test:e2e -- playwright/product-home.spec.ts --workers=2
pnpm --filter @understand-book/web build
pnpm --filter @understand-book/web test:e2e --config ../../docs/performance/yeye-brand-yy5-20261013/playwright.config.ts
```

[生产测试配置](playwright.config.ts) 自动在 `127.0.0.1:4176` 启动并关闭 Vite preview；测试加载真实生产 `home.html` 和构建图片。截图由上述用例保存，当前留存版本来自生产验证。

## 已知限制

- 本轮为本地实现，未提交、推送或发布；YY6 动效和 YY7 完成反馈仍待实施。
- 视口验证来自 Chromium 模拟；实体手机显示仍待验收。首页内嵌的两张产品截图沿用此前保存的资产。
- 构建保留既有 PDF 静态／动态混合导入与大 chunk 提示。
