# 连续阅读版

《深入 Understand Book：从知识构建到阅读 Agent 的架构与实现》连续阅读版，编排日期2026-10-08。收录全书导读、24章正文和6份附录，沿用分章书稿的历史实验条件、源码读取日期和实际验证范围。

## 阅读文件

| 文件 | 用法 |
| --- | --- |
| [连续阅读版 Markdown](深入UnderstandBook-连续阅读版.md) | 单篇全文，包含目录和522个节锚点。流程图从同目录assets加载，公式保留TeX标记。 |
| [连续阅读版 EPUB](深入UnderstandBook-连续阅读版.epub) | 可单独导入支持EPUB 3的阅读器；流程图和公式图片已随书内嵌。 |
| [Markdown含插图压缩包](深入UnderstandBook-连续阅读版-Markdown含插图.zip) | 打包Markdown与26幅流程图，解压后保持assets与Markdown的位置关系。 |

Markdown的跨章引用在同一文件中跳转，源码与验证材料仍以相对路径连接项目文件。在原有exports目录阅读时可以继续查源码；压缩包未包含项目源码，移动到别处后这些项目文件链接需要回到仓库查阅。公式使用原稿的美元符号或反斜杠数学标记，需要阅读器支持相应TeX显示。

EPUB的跨章引用保留书内跳转。947处指向源码或原始材料的引用连接书末340个出处条目，每条给出相对于项目根目录的路径，原引用有段落锚点时同时列出锚点。电子书无需读取外部文件即可读完正文、表格、源码摘录和图表。

## 编排与更新

分章书稿仍是编辑入口。导出时按文件顺序合并chapters与appendices，保持150段非Mermaid代码或输入输出摘录；为全部章节与小节分配唯一锚点，转换跨章引用，保留140张表。部分过宽的横向流程图仅在导出时改为纵向布局，节点、边和标签沿用原稿。26幅流程图、9处独立公式和14处行内公式在EPUB中使用内嵌PNG。

本机重建从项目根目录运行：

```powershell
python -X utf8 docs/project-book/exports/tools/build.py
```

脚本只写exports目录。当前实际使用Pandoc 2.0.1.1、Node 24.9.0、Mermaid 10.9.3、KaTeX 0.16.47，以及Playwright 1.62.1驱动本机Edge。公式渲染使用项目现有KaTeX依赖，浏览器自动化使用本机工作区运行环境；转换器、Node、Playwright与浏览器路径可通过脚本中的BOOK_PANDOC、BOOK_NODE、BOOK_PLAYWRIGHT、BOOK_BROWSER环境变量调整。Mermaid渲染包和EPUBCheck 5.4.0保存在tools/vendor供本地重建与验证使用。

生成后可运行[内容与引用检查](tools/verify.py)、[版面检查](tools/preview.cjs)与EPUBCheck；它们检测导出丢字、摘录变化、资源缺失、断链、EPUB格式错误和页面溢出，不执行产品功能。重建新版本后再验相应输出，不把本版报告视为后续版本的自动验收。

## 实际验证与已知边界

本版通过[内容检查](tools/work/verification.json)、[EPUBCheck](tools/work/epubcheck.json)和[版面检查](tools/work/layout-verification.json)。对31份源稿逐章核对正文片段及摘录；Markdown的1210处本地引用、EPUB包内1392处资源或锚点引用均可达。EPUBCheck最终为零错误、零警告。

实际用无头浏览器在390与900像素宽度检查34个内容页面，共68次页面检查；另查看6张覆盖扉页、源码、表格、流程图及公式的截图。未发现缺图或横向溢出。长时序图在窄屏上仍需放大查看，具体分页、字体与图片缩放取决于阅读器；本轮没有进行实体墨水屏或第三方阅读器实机测试。

本轮没有重跑产品测试、模型、Embedding、费用或容量实验，也没有重新审定所有章节的实现结论。完整过程、首次问题及修正见[书稿来源与验证记录](../SOURCES.md#连续阅读版导出验证记录)，返回[分章阅读入口](../README.md)。
