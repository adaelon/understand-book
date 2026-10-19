# 演示长图：高清与顶部品牌栏

2026-10-13 23:44（香港时间）已发布，23:45 完成线上入口与程序文件核对。

`presentation-share.ts` 在原 HTML 静态画面顶部增加独立的 72px 品牌栏，复用 `src/assets/brand/understand-book-logo.svg` 和站内产品名 `understand-book`。正文只向下平移，宽度、排版和控件大小保持不变。PNG 从 2 倍提高到 4 倍像素，预览与下载仍使用同一文件。

## 实际样张

- [完整高清长图](mha-long-4x.png)：3832 × 11376，约 4.14 MB。
- [顶部局部预览](brand-header-preview.png)。
- 样本为用户反馈的「MHA / GQA / MQA：一个旋钮，两笔账」实际 HTML，回归样本保存于 `packages/web/playwright/fixtures/mha-gqa-mqa.html`。

## 验证

- 提高清晰度前，实际样本仅导出 1916 × 5544；新增尺寸断言失败。调整为 4 倍后通过。
- 新增品牌栏前，高度断言失败；增加顶部栏后，品牌 logo 和文字的像素检查通过。
- 比较实际页面与导出静态页面的按钮文字边界，比例按钮、8192 按钮及判断题均无出框，尺寸误差小于 0.05 CSS 像素。
- 1/4 CSS 像素宽的黑白矢量条纹在输出中逐像素分辨，确认直接绘制高分辨率细节。
- 最终 9 项浏览器回归通过：实际样本、细节分辨率、当前状态及 SVG/Canvas/公式、保留现场、恢复失败、资源失败、长页首尾、原生控件与当前值；包含手机预览与预览/下载字节一致检查。
- 相关状态与组件测试 23 项通过；最终多用户生产构建（含类型检查）通过。构建目录：`tmp/presentation-export-quality/dist-multi`，日志同目录上一级 `build-production.log`。

## 发布状态与限制

本次 4 倍清晰度与顶部品牌栏已发布到 `https://understandbook.top/index.html?account=login`，程序为 `reader-CI6pWOjO.js`。线上程序字节与通过检查的多用户生产构建一致，入口为 `no-store`、CDN 状态 `DYNAMIC`。原 HTML 长图入口由此前发布上线，按钮为「导出长图」；已打开的旧页面需要刷新后使用本次更新。

仅更新现有 release 的前端入口、一个新增程序文件和 `presentation-share.ts`，后端 PID 3526 保持运行。旧入口与源文件已备份于 `/opt/understand-book/backups/presentation-export-quality-20261013T154355Z`，旧静态资源保留。[发布回执](deployment.json)、[线上核对](live-verification.json)。

原 HTML 中已有位图或 Canvas 的细节受其原始分辨率限制；文字与 SVG 在输出分辨率下重新绘制。
