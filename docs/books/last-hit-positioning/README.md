# 这一刀怎么补

副题：看懂对线站位与出手机会。

面向了解补刀、普攻和技能基本规则的普通玩家，以《英雄联盟》式局部对线为背景。正文从局面与动作进入，使用图解和少量直观计算；数学建模、参数口径与可运行代码放在附录。

## 阅读入口

- [完整书稿](book.md)
- [排版阅读版](book.html)
- [章节与写作范围](OUTLINE.md)
- [来源与核对记录](SOURCES.md)
- [续写记录](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/books/last-hit-positioning/SESSION_CHECKPOINT.md)

网站教学由 Agent 在阅读时生成。本目录提供正文、静态配图、可复算案例及来源，不预先编排网站教学流程。

## 文件组织

- `chapters/`：导读与七章正文，作为编辑源文件。
- `appendices/`：数学模型、计算接口、适用边界和来源说明。
- `assets/`：按本书算例生成的原创示意图。
- `examples/`：本书算例、结果与上游计算核心。
- `tools/`：组装书稿、绘制配图和检查交付文件的工具。
- `book.md`：从分章正文组装的单文件导入源；附带同目录的 `assets/`。

## 写作依据

材料读取日期：2026-10-12。

上游：[Deeki-Sora/last_hit_positioning](https://github.com/Deeki-Sora/last_hit_positioning)，固定提交 `50db387dd1218f5a0578183bdfd031cdb9375573`。本书不包含上游未提交内容。上游源代码与相关文档采用 MIT 许可证，随附于 [examples/upstream/LICENSE](examples/upstream/LICENSE)。

书中带数字的主线场景使用明确声明的练习参数，不充当某版本英雄属性。位置和速度统一采用练习格，时间采用秒。数学结论、数值模拟和局面判断在来源索引中分别标明。

完整初稿已经成稿：导读、七章正文、四个附录、来源说明和八幅原创图解。数学建模与可独立运行的参考实现已收录在书内，另提供随书 Python 文件。验证结果见续写记录。

## 编辑与重建

分章 Markdown 是编辑源，`book.md` 和 `book.html` 是组装产物。在当前仓库运行 `node docs/books/last-hit-positioning/tools/build_book.mjs` 可以重新生成两种阅读文件；该脚本使用网站项目已安装的 Markdown-it 和 KaTeX。

配图由 `tools/draw_figures.py` 生成，使用 Pillow 和本机微软雅黑字体。数字复算仅使用 Python 标准库。打包后的 HTML 连同 `assets/` 可直接离线阅读，阅读本身不需要 Node 或 Python。
