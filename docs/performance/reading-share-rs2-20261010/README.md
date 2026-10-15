# RS2 摘录入口与完整内容输出

2026-10-10，Windows 上的 Chromium。接续 RN1–RN4、RS1 未提交工作树；未提交、未发布。功能与 Web 验收完成；实体平台验收由 RS3 接手。

## 已实现

Markdown 跨段选区沿原 `rangeToMarkdown`／精确范围合同进入分享草稿；PDF 仅开放 resolved 选区，原文字层文本按纯文字输出，页码来自该选区已有页面信息。已存高亮从 annotations 重新读取真实记录，跨段组按原文顺序合并全文；PDF 高亮来源优先复用已有范围投影。Markdown 批注、PDF 高亮浮层和 Notes 高亮详情共用 App 选材入口。材料名称读取与身份／材料／发布／聊天清理沿 RS1。

分享使用既有 Markdown 与 KaTeX 渲染，支持段落、标题、强调、链接、引用、列表、内联代码、代码块、公式和表格。PDF 原始选区、分享标题和感想按文字呈现。选定正文只读，内容身份与感想分区，源笔记和记忆不变。

`reading-share-layout.ts` 在 1080×1440 页面上等待实际中文及公式字体，按真实布局分页。段落／代码跨页沿文字行切分，列表保留序号；公式和表格行保持完整，续表复用列宽与表头。续页携带标题、内容身份、页序和出处。过宽对象、过高单个对象、无法解析的公式及内嵌资源在输出前明确报错。

同一排版使用自包含 SVG／Canvas 生成逐页 PNG；中文字体子集和 KaTeX 字体沿项目已分发资源内嵌，无新依赖。预览与保存共用各页同一个 Blob，上一页／下一页可查看全部输出，逐图下载使用带页序的文件名。编辑后全部旧页面失效，失败保留分享草稿。

## 验证

| 检查 | 结果与实际范围 |
| --- | --- |
| Vitest 定向 | 8 文件 117 项通过；内容身份、格式转换、分组选材、来源、分享与笔记草稿独立、页序保存、失败重试及现场切换晚到结果隔离 |
| Chromium | 10 项通过：RS2 5 项（390／1440 px 六页富文本、PDF 原文与高亮／partial 禁止分享、Markdown 跨段及第二页真实下载、过宽拒绝与长列表／代码完整性）；RS1 2 项及 RN3／原 PDF 3 项回归 |
| PNG | 实际解码 1080×1440，逐页非空像素；各页合并保留完整长段、代码、列表、表格行和感想；续页标题区域逐像素相等，第二页下载与预览逐字节相等 |
| Web 构建 | vue-tsc 与 Vite 生产构建通过；保留原 PDF 混合导入和大包提示 |

实际六页样例：[格式与列表](rs2-page-1.png)、[长段续页一](rs2-page-2.png)、[长段续页二](rs2-page-3.png)、[代码与分式](rs2-page-4.png)、[表格](rs2-page-5.png)、[续表与感想](rs2-page-6.png)。[用户入口实际下载第二页](rs2-second-page.png)。已查看中文、公式与表格，另用 PNG 解码确认六页页头一致。

Chromium 的 SVG decode 早于 foreignObject 字体布局与绘制完成，初次验收发现页头错位；生成 PNG 前等待布局／绘制后修复，逐页像素用例覆盖此回归。

复现命令：

```text
pnpm --filter @understand-book/web exec vitest run src/reading-share.test.ts src/reading-share-save.test.ts src/components/ShareImagePanel.test.ts src/components/NoteDetail.share.test.ts src/App.startup.test.ts src/components/RightRail.test.ts src/components/ReaderPane.test.ts src/components/PdfReaderPane.test.ts --maxWorkers=2
pnpm --filter @understand-book/web exec playwright test playwright/pdf-selection-actions.spec.ts --grep "RS1|RS2|RN3|resolved real PDF selection performs" --workers=1
pnpm --filter @understand-book/web build
```

## 已知限制

- 内嵌图片、视频等资源尚未接入此文字分享输出；明确阻止导出并给出重新选材入口。
- 超过单页宽度的公式／表格、不能拆分的高内容块要求缩小选材；字号保持可读，不裁切下载。
- 本轮浏览器业务响应使用固定材料样本，真实执行 App、选区、字体、分页、PNG 编解码和下载。Windows 安装包原生保存、实体手机与 Safari 等平台行为在 RS3 验收。
- 分享草稿仍驻留当前页面；本轮不改变 RN4 平台待验及 MU10 恢复失败状态。
