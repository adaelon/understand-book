# 解释语法模板库

正式目录 revision 6 收录 11 种解释方法、12 个登记版本。每个版本提供方法卡、可运行作品、改写说明、制作元数据和预期观察。新增的两种方法共含五个场景。同一个范数问题可以画等值边界、按步行走或对齐读数。观察内部空间可以拆开装配件，也可以移动截面。

[打开可操作总览](index.html) · [机器目录](catalog.json) · [来源与许可](SOURCES.md) · [制作与验证记录](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/presentation-method-library-v1/README.md)

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
| 对象相同，换规则后为什么读数或排序会变？ | [固定对象，换规则比较结果](compare-rules/v1/card.md) | [边界蓝图](compare-rules/v1/example/index.html) · [像素街区](compare-rules/v1/routes/index.html) · [孔版印痕](compare-rules/v1/prints/index.html) |
| 哪些部件或截面藏在外表之下？ | [拆开或剖切，辨认被遮挡的关系](inspect-space/v1/card.md) | [相机装配](inspect-space/v1/preview.html?scene=assembly) · [点云剖层](inspect-space/v1/preview.html?scene=section) |

## 选用和改写

先写清读者要解释的关系。对有场景可选的方法，教练在已有的 adaptation 中写出所选文件、如何按新材料重做，以及读者能看见什么。目录的 appearance 描述这个场景的主画面。作者读取该卡、所选完整 HTML、配套 author.json、改写说明和反例，从示范的绘图区开始改写。证明或查阅可以保留连续的正文与公式。

正式路径：Author 暴露短目录 → `read_method` 读取确切版本和文件 → 按新内容改写 → `write` / `patch` 声明 `method_refs` → preview → deliver。完整源码按需读取，未选模板不会自动进入上下文。默认构建读取本目录。

author.json 是示例的内容元数据，HTML 单独读取后填入 write；method_refs 在实际读取后填写。保存重开由 Reader 宿主负责。Three 模板要声明 `libraries:["three"]`，宿主随版本保存离线依赖。总览页只用于选型和观察；在仓库根目录运行 `python -m http.server 4197 --bind 127.0.0.1 --directory skills/presentation/methods` 后打开本地总览。所有依赖在本机，三维预览通过本地服务加载。

## 三维联动模板

`spatial-linked-views@1` 将三维渲染、稳定相机取景、指针与键盘旋转、拆解、同源多图更新、时间定位、现场恢复抽成同一份运行核心。改主题时替换公式、部件、图解和文字，保留同步和生命周期机制。完整的 Agent 改写入口见 [适配说明](spatial-linked-views/v1/example/README.md)，实际验证见 [三维模板记录](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/presentation-spatial-template-v1/README.md)。

本地维护执行 `node scripts/build-spatial-method.mjs` 生成两套自包含 HTML；`node scripts/verify-spatial-method.mjs` 检查真实 WebGL、独立数学和几何、正式状态桥重开、静态导出及无 WebGL 的二维回退。总验证脚本也会调用该专用检查器。

## 场景参照与维护

相机与剖层各有一份场景源码 `scene.html`，只共用轨道相机和取景。修改后运行 `node scripts/build-inspection-scenes.mjs`。总览使用 `node scripts/build-method-gallery.mjs` 从正式目录刷新。二维的三份作品可直接改写各自的 HTML。五个场景、29 个视频时间点和实际验证结果见场景制作记录。

## 本批采用记录

2026-10-16：正式目录从 revision 1 的空目录更新为 revision 2。按本轮用户要求，先建立可用模板库，以具体作品正确性、交互恢复和正式读取作为本批登记依据。SE8 收益对照暂缓，SE10 保持由真实漏检触发。

discrete-update@2 沿用 @1 的 HTML、公式和交互，补充机器可读的 Author 元数据及正式登记说明。@1 候选和 SE1–SE7 证据保留原身份。其余 7 个方法为首次登记。历史演示继续引用其原版本；回退采用时修改后续构建目录，已有交付源码和现场由原版本保有。

同日，用户认可齿轮演示并要求抽成模板；revision 3 登记 spatial-linked-views@1。两个主题共用同一核心，真实 Author 各完成三环境预览与交付，读取链和 Reader 状态桥通过验证。首批八单元原版本不变。

## 八单元联动升级

同日按用户要求登记 discrete-update@3 和其余七项 @2。保留旧版本与原验证记录。新作品共享二维状态、播放、恢复和布局机制，每个主题有独立模型、主图、细节图、直接操作与三段探索。生成入口为 `node scripts/build-linked-methods.mjs`；验证入口为 `node scripts/verify-linked-methods.mjs`；[本批记录](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/presentation-method-library-v2/README.md)。

## 已知限制

三维联动模板的齿轮与弹簧，以及新场景的自然请求，分别有各自的验证记录。案例能说明当次制作和操作结果。自然选型的稳定性及读者学习效果仍需更多实例。本批改动已于 2026-11-03 21:03 HKT 随 Reader 发布。上线记录
