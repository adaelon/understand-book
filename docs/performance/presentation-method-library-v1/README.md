# 首批解释语法模板库

2026-10-16。状态：完成。用户要求参考 huashu-art-motion 建立可用模板库；SE8 收益对照暂缓。正式 catalog revision 2 登记 8 个单元、40 个资产文件。

## 交付步骤

- [x] 核对现有目录、源码读取与现场合同，读取参考项目语法卡和示例实现。
- [x] 制作 8 个完整能力单元及可操作总览，保存参考来源和适配边界。
- [x] 通过作品数值/图形、三视口交互和真实现场桥接恢复检查。
- [x] 登记正式目录，验证编译资产与 Author 读取，更新项目状态和采用记录。

## 使用入口

- [可操作总览](../../../skills/presentation/methods/index.html)：切换方法，直接操作每个完整示例。
- [模板库说明](../../../skills/presentation/methods/README.md)、[正式目录](../../../skills/presentation/methods/catalog.json)、[参考映射与许可](../../../skills/presentation/methods/SOURCES.md)。
- [最终验收汇总](acceptance.json)：逐单元连接最终浏览器、预览与交付证据；确切交付 HTML 与当前资产逐字一致。

## 范围

沿用 `skills/presentation/methods/<id>/v<revision>/`、正式 catalog 和现有 `read_method`。首批为离散更新、对象变换、单因素对照、整体与部分、过程展开、证据到结论、数据图表及尺度缩放。每个版本包含语法卡、自包含 HTML/SVG/JavaScript 示例、运行/适配说明、针对性检查和机器可读的 Author 内容元数据。

参考版本：alchaincyf/huashu-art-motion `2571ee0a9a0467c4d99e935ea9e0fadda19210b0`。提炼对象连续性、语义色、逐步画关系、证据标注、坐标先行及尺度定位，按本项目解释关系重写实现。原离散更新 v1 及其候选实验保持原版本；正式库使用 v2。

## 验收与失败后的动作

- 独立核对真实 SVG 几何、数值与边界；错位、比例或推导错误时修改对应示例。
- 在 320×420、640×240、960×720 操作控件、前后定位，并通过现有 presentation bridge 保存和重开；失败时修正布局或同一状态的恢复路径。
- 编译登记和 Author 分段读取应与文件逐字相同；缺文件、漏登记或截断错误时修正目录或接入。
- 仅目录可进入初始指导，示例按需读取；若正文被自动注入，修正登记/装配。

## 实际结果

| 检查 | 结果 | 记录 |
|---|---|---|
| 数值、SVG 几何和边界 | 8 单元共 70 组初验通过；图例、负值标签和尺度说明修订后重验对应 3 单元，勾选框修订后重验证据链 | [初验](verification/browser-results.json)、[三单元复验](verification-final/browser-results.json)、[证据链复验](verification-evidence-final/browser-results.json) |
| 控件、定位与恢复 | 每单元在 320×420 touch、640×240 touch、960×720 mouse 操作前后步、播放暂停、键盘定位；24 次关闭页面后以真实 presentation bridge 恢复，参数、图形、读数一致且暂停 | 同上，最终对应关系见 acceptance.json |
| 编译与按需目录 | 4 项 runtime 检查通过：正式资产、目录装配、精确读取合同及预算续读 | [runtime 日志](runtime.log) |
| Author 读取与请求装配 | 2 项集成检查通过；40 文件分段读回，8 个独立方法运行共 56 次正文读取、72 次受控 Provider 边界请求；初始请求仅目录，所读正文进入后续请求 | [读取日志](author-reads.log) |
| 实际 Author 制作交付 | 8 次 write → 每单元三环境真实 preview → deliver → 私有存储读取；24 组预览无错误，小数位置回退再定位一致，最终 HTML 与资产相同 | [前五个单元](author/)、[后三个单元](author-final/results.json)、[最终日志](author-deliver-final.log)、acceptance.json |
| 总览与文档 | 320、960、1440 宽度切换全部八项正常，无横向溢出；35 份说明中的 180 个本地链接有效 | [总览截图](verification-final/gallery-1440.png)、[链接结果](document-links.json) |

实际查看了桌面总览、手机布局、负值图表、整体图例、尺度图和证据链截图；每个单元的桌面与手机截图随对应浏览器记录保存。Author 截图位于对应预览目录。

## 修复记录

- 初版几何序列比较把空 `style` 属性与缺少该属性视为不同。二者不影响图形，比较器去除空属性后继续比较全部几何、读数和控件。
- 原 SE2 测试在单次运行读取整个目录，扩库后触及运行限额。改为每个选中单元一个独立运行，并读取清单中全部文件。运行限额未改；[首次结果](author-reads-initial.log)保留。
- 图例补上 A/B/C 对应；负柱数值移到柱内并与批次标签分离；总览采用简短方法名。
- Author 首轮通过前五个单元后，发现证据链 checkbox 的点击区域为 22×22。改成 44×44 点击区域、22×22 图标，并将浏览器触摸检查覆盖 checkbox；后三单元的实际预览和交付复验通过。[首次 Author 结果](author-deliver.log)保留。

## 复现

从仓库根目录运行：

~~~powershell
node scripts/verify-presentation-method-library.mjs
cargo test -p runtime --lib se2_
cargo test -p server --lib se2_ -- --nocapture
$env:METHOD_LIBRARY_AUTHOR_OUTPUT = Join-Path (Get-Location) 'tmp/method-library-author-new'
cargo test -p server --lib method_library_author_preview_and_deliver -- --ignored --nocapture
~~~

Author 验收使用默认正式目录，先取消实验用 UNDERSTAND_BOOK_METHOD_CATALOG；需要全量复现时不设置 METHOD_LIBRARY_IDS。浏览器脚本默认使用 Edge；可用 METHOD_LIBRARY_BROWSER 指定本地 Playwright 浏览器通道，METHOD_LIBRARY_OUTPUT 指定结果目录。脚本后附方法 id 可只重验受影响单元。以上不调用付费模型。

## 已知限制

本批验证制作资产的正确性、实际 Author 制作和读取接入。Provider 输入检查采用受控返回，不证明自然选卡；跨题收益及读者学习收益需实际使用材料支持。页面重开使用真实 bridge 和浏览器页面，未重复 SE1 的完整 Reader 服务重启实验。已完成测试构建，尚未发布或部署到线上；现有运行实例需后续构建发布才能取得新目录。构建日志中的既有 ts-rs serde 属性警告仍在。
