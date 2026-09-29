# ED1–ED2：连续交互与静态绘图验收记录

日期：2026-09-25。对应 [ADR-0133](../adr/0133-agent-led-explorable-explanations-and-evolving-presentation-brief.md) 与[实施方案](../切片方案-Agent自主表达设计与PresentationBrief.md)。

## ED1 连续交互

桥接脚本首次加载仍等待公开语义回执。后续 DOM 语义变化只遮住变化区域，并用上一份已接受内容占位；数字、来源与复杂子树替换继续交给原 `/agent/presentation.observe` 判定。接受回执后发布对应版本的 DOM，拒绝沿用原错误终态。等待期间的现场快照排队至确切修订获准，旧回执不释放较新快照。控件继续留在原 iframe 中；覆盖层位于关闭的 Shadow DOM，不进入页面的选择器、来源扫描或状态采集。

组件把“正在准备内容…”留给首次加载；校验不再重置 ready，保存提示不占演示区布局。iframe 高度由稳定的视口规则决定，移除了以自身撑大的父容器高度反复计算 iframe 高度的反馈环。

浏览器回归先复现等待回执时 `body` 隐藏，再验证等待期间逐帧可见、连续滑块与焦点、旧回执、快照、复杂子树的旧内容占位。真实 Rust 测试宿主的来源拒绝、来源打开、保存、追问、重开、同版 iframe 以及提示前后边界均通过。固定 640×680 视口中提示出现前后的 iframe 边界和 load count 相同。

## ED2 静态绘图

`presentation.author(render_plot)` 在本次制作的临时目录中启动 Python/Matplotlib Agg，传入 JSON `data` 和尺寸，提供 `plt/fig/ax` 及字体、字号、线条、色彩、边距默认值。执行受 20 秒、代码/数据与 SVG/PNG 大小限制，取消停止 Python；错误以有界 stderr 返回。宿主返回 SVG `asset_ref`/`asset_path`、MIME、尺寸与字体，同时把实际 PNG 作为下一次 native/react 模型采样的图像。新图须在模型收到图像的下一采样后才可被 `write.asset_refs` 引用。

`write` 把选中的 SVG、复绘 `.py` 和输入 `.json` 放入原 `PresentationContent.content_files`，将整份候选限制在 1 MiB。精确版本 `read` 列出图像 ref，代码与输入可按文件名分段读取。候选预览将该 SVG 装成 data URL；Reader 的 `presentationDocument` 同样装配版本内 SVG。交付与重开读取同一版本字节，不运行 Python。修改版可以复用旧图或附加新图，旧引用仍读取原版本。

本机 Windows Python 3.14 / Matplotlib 3.10.9 / Microsoft YaHei 生成 640×400 示例图：`(0,20)、(1,25)、(2,30)`，横轴秒、纵轴摄氏度、中文图例均正确，未见标签或图例裁切。[实际 PNG](ed2-windows/plot.png)与[SVG](ed2-windows/plot.svg)已保存。浏览器排练生成真实截图，保存版本后用新 `AppState` 读取私有磁盘文件，SVG 与绘图代码一致。错误回传与运行中取消也已验证。

真实 Resident 回合从自然请求发现制作能力，首次绘图失败后重试，下一采样收到图像，再写入页面、完成 320×420 touch / 640×240 touch / 960×720 mouse 三个预览并交付。公开 `presentation.read` 返回了固定版本的 SVG、复绘代码和输入；[该次实际 View](ed2-windows/real-view.json)又经 Web 资源装配放入 Reader iframe，图像成功解码。三个视口图片均作了人工查看：[Python 窄屏](ed2-windows/python-narrow.png)、[桌面](ed2-windows/python-desktop.png)。

同一数据与解释要求另跑现有浏览器内联 SVG 路径，均未调用书内检索。两次各是单次模型采样实验，未形成统计意义上的质量或成本结论。

| 路径 | 结果 | 模型回合 | tokens_spent | 墙钟时间 | 视觉观察 |
| --- | --- | ---: | ---: | ---: | --- |
| Matplotlib SVG 版本资源 | 成功交付；先失败一次并重画；三个视口预览通过 | 7 | 79,824 | 39.6 秒 | [窄屏](ed2-windows/python-narrow.png)文字/图例可见但图较小；[桌面](ed2-windows/python-desktop.png)坐标与数值清楚 |
| 浏览器内联 SVG | 成功交付；三个视口预览通过 | 5 | 88,293 | 75.4 秒 | [窄屏](ed2-windows/browser-narrow.png)图占空间更大、标签更易读；[桌面](ed2-windows/browser-desktop.png)数据和图例清楚 |

两版的 `readable_content` 均说明示例数据和单位；浏览器版在窄屏图的可读性更好。本次没有观察到 Python 版的解释优势。Python 路径把图形计算/排版从网页代码移入 Matplotlib，并提供复绘文件；单次用量低于浏览器版，但两个生成过程的随机性和一次失败重试使这不能被解释为稳定成本收益。

制作宿主须安装 [固定 Python 依赖](../../scripts/presentation-plot-requirements.txt)，并让 `UNDERSTAND_BOOK_PLOT_PYTHON` 指向该环境的 Python；未设时使用宿主 PATH 中的 `python`（Windows）或 `python3`（Linux）。运行时不安装包。中文绘图需要 Microsoft YaHei、Noto Sans CJK SC、Noto Sans CJK 或 WenQuanYi Micro Hei；缺字库时绘图返回错误。

## 验证与已知问题

- ED1：最小 bridge 浏览器组 6/6；真实 Rust 宿主的演示、追问、恢复、交付图表装配及 bridge 联合浏览器组 15/15；组件实例、Web 类型检查通过。
- ED2：Server 绘图生成、错误、取消 2/2；真实浏览器预览与磁盘重开 1/1；Runtime 图像进入下一采样及过早写入拒绝 1/1；真实 Resident 绘图与浏览器基线各 1/1；实际交付 View 的 Web iframe 装配 1/1。Server/Runtime 定向 author 测试及类型检查通过。
- Linux 实机与桌面安装包没有在这台 Windows 工作区验收：原 Linux 主机的 SSH 当前拒绝连接或认证。发布 Linux 及未配置 Python 的桌面环境前，需按本页的依赖合同在目标宿主运行绘图、取消、浏览器预览与重开验证。
- 以上仅为同题各一次的技术与表达对照，尚不能推断稳定的解释效果或成本优势。手机只需显示已保存 SVG，真实手机入口尚未执行。
