# 画板元素与数学排版

日期：2026-10-28。状态：10:35:23 HKT 已与播放连线修复合并上线；10:37:25 HKT 公网复核通过。发布基于线上源码，保留空间笔记与账号功能，见[合并发布记录](deployment/README.md)。以下验证段落记录本地实现阶段。

## 呈现决定

参考用户提供的 Excalidraw 视频，原生图形采用浅色填充、轻微曲线双描边和暗色文字。短标题与正文区分字号、字重；连线标签更清楚，背景点阵减淡。原有画板位置、尺寸、正文、来源与持久化协议保持不变。

完整解释继续放在讲解区；画板节点保留短标题、少量说明和公式。后续创作提示要求对齐、分组留白、少量语义色与局部讲解焦点。`preview.html` 是用真实 `BoardCanvas` 按这些规则重排的同主题示例，数据只在此页面内存中；`existing-board.png` 使用上一轮的 11 节点播放夹具，未重排该夹具。

## 实现链路

- `BoardCanvas.vue → board-canvas-appearance.ts`：稳定的矩形／椭圆轮廓与全文排版。播放依据已排好的文字截取，不逐帧重新换行。
- `BoardDrawingText.vue → board-math.ts`：普通文字保留 SVG；含公式时使用 KaTeX 与 `foreignObject`，按照实际内容尺寸拟合。支持 `$…$`、`$$…$$`、`\(…\)`、`\[…\]`、分数、矩阵与上下标。明显的旧式状态方程和维度乘积仅在显示时规范化，普通字段名和路径保持原样。
- 数学表达整体揭示；正文字符隐藏时仍占据原位置。内容／尺寸／字体变化才触发测量，播放进度不重新解析数学或测量布局。
- `board-preview-assets.mjs → board-preview.ts`：独立预览保留内嵌 KaTeX 字体，等字体及拟合完成后报告就绪；对 HTML 内容和公式实际边界检查裁切。
- `crates/runtime/src/board_author.rs:spec`：短节点、完整讲解、空间组织、数学定界符与真实预览的创作约定。

## 验证

| 验证 | 结果 |
| --- | --- |
| `board-math.test.ts` | 20 项通过：四种定界符、跨行、旧公式、原文偏移、普通字段名、错误公式回退 |
| `board-canvas.test.ts` | 14 项通过：形状、绘制进度、文字位置、公式完整揭示、拖动、键盘、命中区域 |
| `board-workspace.test.ts`、`board-canvas-preview.test.ts` | 45 项通过：播放、保存重开、连线和候选预览范围 |
| `pnpm --filter @understand-book/web typecheck` | 通过 |
| `rustfmt --emit stdout crates/runtime/src/board_author.rs` | 只读解析通过；描述变更不涉及后端行为 |
| 真实浏览器 1440×1000 | 旧布局与重排示例文字／公式无裁切；旧路线末步保留 11 节点、10 连线；公式节点拖动与键盘选择正常 |
| 390×844 | 聚焦更新节点后显示宽 294px；公式完整、无溢出，可继续平移缩放 |
| 编译后的独立预览 | 5 个公式正常，3 个数学字体已加载，无外部字体请求；另验分数、求和、矩阵，无裁切和页面错误 |

浏览器证据：`verification.json`、`isolated-verification.json`。截图：`existing-board.png`、`board-overview.png`、`board-narrow.png`、`board-preview-isolated.png`、`board-complex-equations.png`。

本地开发服务入口为 `http://127.0.0.1:4184/@fs/E:/allwork/download/agent/understand-book/docs/performance/board-appearance-20261028/preview.html`。复验脚本为 `verify.mjs` 与 `verify-isolated.mjs`，后者使用 `packages/web/dist-board-appearance/board-preview` 中的编译资源。

## 已知边界

既有密集布局保留原位置与内容，视觉更新不会自动重排私人画板。新画板的内容密度仍需作者结合真实预览决定。无法解析的公式保留可读原文。线上渲染与创作指导已更新，用户已保存的画板内容保持原值。
