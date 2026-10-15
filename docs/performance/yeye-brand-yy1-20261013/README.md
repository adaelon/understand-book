# YY1 统一品牌色（2026-10-13）

YY1 已在本地完成。首页、书架与阅读器共用焙橙 `#B65A17`，主操作使用白字；小字号链接和浅底选中态使用深色 `#91420F`。接手入口：[实施切片](../../切片方案-页页品牌融入与动效.md#yy1-统一品牌色)。

## 实现

- [brand.css](../../../packages/web/src/brand.css) 保存主色、深色、浅色、悬停衬底、边框和白色前景。`style.css` 与 `home.css` 导入同一份色表，现有变量调用名称继续可用。
- 首页主按钮、链接、品牌图标、步骤选中态、单选项、滑块与局部装饰采用共同色组；末尾邀请区使用深色中性底和浅杏按钮。花园图解和学习结果的内容配色保留原职责。
- 阅读器主按钮、书架“继续阅读”、桌面／手机发送、历史数量徽标、笔记标签与 PDF 标记完成配色对齐。全局主按钮的禁用态保持中性，悬停使用深色；焦点环使用实色，深背景首页区使用浅色焦点环。
- `--accent` 使用深色以保证暖纸底上的小字可读；`--reader-coral-soft` 保留透明混色，使 PDF 定位覆盖层透出原文。字号、行距、内容宽度及当前手机常驻输入布局沿原样式保留。

## 验证

[生产构建](build.log) 通过，包含 `vue-tsc --noEmit` 与两个生产页面入口的 Vite 构建。

[浏览器回归](browser-tests.log) 共 15 项通过：首页互动与窄屏 4 项，桌面／手机教学布局 4 项，手机问答操作 4 项，PDF 笔记 2 项，Markdown 笔记 1 项。覆盖草稿保留、作答提交、选区笔记、焦点、阅读位置及视口边界。

[颜色与状态记录](verification.json) 来自 Chromium 中的实际计算样式，共 43 次采样。启用控件文字要求至少 4.5:1，焦点环至少 3:1；禁用态单独记录其状态与样式。1440×900、390×900 的首页、书架、问答与笔记页面无横向溢出，无页面脚本错误。

| 实际组合 | 对比度 |
| --- | ---: |
| 白字／焙橙主按钮 | 4.70:1 |
| 白字／深色悬停按钮 | 7.02:1 |
| 首页小字链接／暖纸底 | 6.49:1 |
| 首页选中步骤／浅杏底 | 5.70:1 |
| PDF 笔记数量／卡片底 | 6.77:1 |
| 焦点环／书架接续卡底 | 3.89:1 |

问答样本的正文仍为 `17px / 31.45px`，颜色 `rgb(37,35,32)`。主色直接用作暖纸底小字时只有约 4.35:1，因此小字改用深色派生；主按钮保留主色。

截图：[桌面首屏](home-hero-1440.png)、[手机首屏](home-hero-390.png)、[完整首页](home-1440.png)、[手机完整首页](home-390.png)、[手机选中态](home-selected-390.png)、[桌面书架](library-1440.png)、[手机书架](library-390.png)、[桌面问答](teaching-1440.png)、[手机问答](teaching-390.png)、[手机作答](responding-390.png)、[禁用发送](chat-disabled-390.png)、[PDF 笔记](pdf-notes-390.png)、[接续错误提示](library-error-390.png)。

复现命令从仓库根目录执行；颜色脚本需要另一个终端启动 Web 开发服务器：

```powershell
pnpm --filter @understand-book/web dev --host 127.0.0.1 --port 4174
node docs/performance/yeye-brand-yy1-20261013/verify.mjs
pnpm --filter @understand-book/web build
pnpm --filter @understand-book/web test:e2e -- playwright/product-home.spec.ts playwright/conversation-learning-design.spec.ts playwright/mobile-chat-interactions.spec.ts playwright/pdf-annotation.spec.ts playwright/reader-annotations.spec.ts --workers=2
```

## 已知限制

- 本次为本地实现和 Chromium 视口验证，尚未提交、推送或发布；实体手机软键盘仍沿原手机预览记录待验。
- 书架和问答使用固定测试数据，PDF 颜色采样为现有标注预览搭配仓库中的许可 PDF；不访问真实账号。首页内嵌的两张产品截图仍是此前保存的页面资产。
- 构建保留现有的大 chunk 提示。页页角色素材、头像和动画接入依 YY2–YY7 继续。
