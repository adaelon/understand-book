# 解释语法模板库

正式目录 revision 5 包含 9 个可组合能力单元、10 个登记版本。首批八单元包含双图同源、图形直接操作与完整讲解；单因素对照的新版本支持同页连续演示、显现帮助和用户接手，原 @2 仍可读取。每个版本保存语法卡、完整可运行作品、适配说明、Author 元数据和具体预期观察。三维联动模板从用户认可的齿轮作品提取，并提供弹簧振子的跨题改写。

[打开可操作总览](index.html) · [机器目录](catalog.json) · [来源与许可](SOURCES.md) · [制作与验证记录](../../../docs/performance/presentation-method-library-v1/README.md)

## 按理解障碍选择

| 当前需要解释 | 方法 | 示例 |
|---|---|---|
| 当前状态怎样得到下一状态？ | [追踪一次离散更新](discrete-update/v3/card.md) | [运行](discrete-update/v3/example/index.html) |
| 对象如何变，哪些性质保持？ | [保持身份的对象变换](object-transform/v2/card.md) | [运行](object-transform/v2/example/index.html) |
| 差异由哪一个变量造成？ | [固定条件，只改变一个因素](compare-one-factor/v3/card.md) | [运行](compare-one-factor/v3/example/index.html) |
| 这些部分如何组成整体？ | [整体与部分保持对应](part-whole/v2/card.md) | [运行](part-whole/v2/example/index.html) |
| 输入经过哪些步骤得到结果？ | [沿关系逐步展开过程](process-reveal/v2/card.md) | [运行](process-reveal/v2/example/index.html) |
| 这个结论具体由什么支持？ | [从证据走到有边界的结论](evidence-chain/v2/card.md) | [运行](evidence-chain/v2/example/index.html) |
| 这些数字的差异有多大？ | [先建立尺度，再揭示数据](chart-reveal/v2/card.md) | [运行](chart-reveal/v2/example/index.html) |
| 局部细节在整体的什么位置？ | [保留定位的尺度缩放](scale-zoom/v2/card.md) | [运行](scale-zoom/v2/example/index.html) |
| 立体结构、运动与公式如何对应？ | [三维结构与图解共用一个模型](spatial-linked-views/v1/card.md) | [齿轮](spatial-linked-views/v1/preview.html?topic=gear) · [弹簧](spatial-linked-views/v1/preview.html?topic=spring) |

## 选用和改写

先写清读者卡住的关系，选择零个或多个方法。只需一个定义或小幅措辞修改时，使用文字或静态关系即可。读取所选卡片、完整 HTML、适配说明及 author.json；根据新任务重新建立公式、变量、数据、来源、边界和检查。共用色彩只表达同一身份；相机、强调、描线服务于当前关系。

正式路径：Author 暴露短目录 → `read_method` 读取确切版本和文件 → 按新内容改写 → `write` / `patch` 声明 `method_refs` → preview → deliver。完整源码按需读取，未选模板不会自动进入上下文。默认构建读取本目录。

author.json 是示例的内容元数据，HTML 单独读取后填入 write；method_refs 在实际读取后填写。保存重开由 Reader 宿主负责。Three 模板要声明 `libraries:["three"]`，宿主随版本保存离线依赖。总览页只用于选型和观察；在仓库根目录运行 `python -m http.server 4197 --bind 127.0.0.1 --directory skills/presentation/methods` 后打开本地总览。所有依赖在本机，三维预览通过本地服务加载。

## 三维联动模板

`spatial-linked-views@1` 将三维渲染、稳定相机取景、指针与键盘旋转、拆解、同源多图更新、时间定位、现场恢复抽成同一份运行核心。改主题时替换公式、部件、图解和文字，保留同步和生命周期机制。完整的 Agent 改写入口见 [适配说明](spatial-linked-views/v1/example/README.md)，实际验证见 [三维模板记录](../../../docs/performance/presentation-spatial-template-v1/README.md)。

本地维护执行 `node scripts/build-spatial-method.mjs` 生成两套自包含 HTML；`node scripts/verify-spatial-method.mjs` 检查真实 WebGL、独立数学和几何、正式状态桥重开、静态导出及无 WebGL 的二维回退。总验证脚本也会调用该专用检查器。

## 本批采用记录

2026-10-16：正式目录从 revision 1 的空目录更新为 revision 2。按本轮用户要求，先建立可用模板库，以具体作品正确性、交互恢复和正式读取作为本批登记依据。SE8 收益对照暂缓，SE10 保持由真实漏检触发。

discrete-update@2 沿用 @1 的 HTML、公式和交互，补充机器可读的 Author 元数据及正式登记说明。@1 候选和 SE1–SE7 证据保留原身份。其余 7 个方法为首次登记。历史演示继续引用其原版本；回退采用时修改后续构建目录，已有交付源码和现场由原版本保有。

同日，用户认可齿轮演示并要求抽成模板；revision 3 登记 spatial-linked-views@1。两个主题共用同一核心，真实 Author 各完成三环境预览与交付，读取链和 Reader 状态桥通过验证。首批八单元原版本不变。

## 八单元联动升级

同日按用户要求登记 discrete-update@3 和其余七项 @2。保留旧版本与原验证记录。新作品共享二维状态、播放、恢复和布局机制，每个主题有独立模型、主图、细节图、直接操作与三段探索。生成入口为 `node scripts/build-linked-methods.mjs`；验证入口为 `node scripts/verify-linked-methods.mjs`；[本批记录](../../../docs/performance/presentation-method-library-v2/README.md)。

## 已知限制

本库交付的是可改写的制作资产。三维模板已完成齿轮到弹簧的确定性改写验证；Agent 自然选卡、独立迁移的收益及读者学习效果尚无本批结论。线上生效需要后续构建发布。示例只说明其具体关系和支持域，不能替代新书籍内容的核对。
