# 八个解释模板的联动升级

2026-10-16，用户要求把首批八个模板升级到已认可的完整演示标准。保留主题、数学范围及旧版本；新增 discrete-update@3 和其余七项 @2。spatial-linked-views@1 保持原实现。

## 实施切片

1. 提取适合二维解释的共同机制：统一状态、确定性定位、直接操作、主图与细节图同源、现场恢复、可读 DOM 和响应布局。
2. 分别完成八个独立作品：离散迭代、剪切变换、单因素运动、分配与守恒、数据流程、证据与结论、图表尺度、连续缩放。每个作品包含自己的图形、模型、操作和解释，不以换标题代替主题实现。
3. 更新语法卡、完整源码、适配说明、Author 元数据、预期观察；登记 catalog revision 4 和总览入口。
4. 用独立公式检查图形对应关系；在 Reader 的正式装配、状态桥和沙箱中检查手机/短屏/桌面、直接操作、反向定位、重开和导出。
5. 通过正式方法读取、Author 三环境预览和交付后保存证据与接手记录。

## 验证目的

数学检查检测公式与实际 SVG 几何的偏差；失败时修正模型或坐标映射。浏览器检查检测操作失联、状态丢失、布局遮挡和静态捕获丢图；失败时修正相应共同机制或主题实现。Author 检查检测正式目录读不到完整源码、元数据与候选不匹配或宿主不接受作品；失败时修正登记与内容合同。

## 交付的八个作品

| 方法与版本 | 本版的关系与操作 |
|---|---|
| discrete-update@3 | 函数曲线、切线、更新方向、真实迭代与误差轨迹；点击轨迹点或拖动细节图定位 |
| object-transform@2 | 推拉剪切图形，联动角点、水平切片、基向量、面积和周长 |
| compare-one-factor@2 | 共用时钟的两辆车、固定刻度跑道、距离曲线与差值；直接拖动时间 |
| part-whole@2 | 连续条形拆成三行，与百格图保持份额一致；直接改变 A，核对两种百分比的分母 |
| process-reveal@2 | 点选一个输入，追踪加数、筛选、求和；保留输入身份与被筛除的位置 |
| evidence-chain@2 | 原始记录、均值、前提与结论；切换比较条件，保留算术结果并收紧结论 |
| chart-reveal@2 | 固定零线的柱图、原数表、差值线与比例；包括正值、负起点、零起点 |
| scale-zoom@2 | 连续十倍缩放，联动真实标尺、固定全景窗口、中心定位轴与 1 mm 目标 |

[打开总览](http://127.0.0.1:4197/) · [正式目录](../../../skills/presentation/methods/catalog.json) · [Agent 使用入口](../../../skills/presentation/methods/README.md)

每个新版本均有卡片、完整可运行 HTML、Author 元数据、具体适配说明和预期观察。可维护源为各版本 topic.js 与共用的 `_shared/linked-v1.js`、`linked-v1.css`；打包后的 HTML 包含全部业务代码，Agent 可直接读取改写。八个作品没有新增外部依赖。

## 验证结果

| 范围 | 实际结果 |
|---|---|
| 数学与真实 SVG | 104 组，独立推导闭式、面积、距离、分配、均值、柱形基线及尺度；核对实际顶点、线、柱和窗口 |
| 真实 Reader | 8 × 3 环境，使用生产 presentationDocument、CSP、不透明 iframe、bridge 与静态捕获 |
| 操作与恢复 | 24 组直接指针/键盘操作、播放暂停、反向定位与现场重开；实际 SVG 和读数保持一致，重开暂停 |
| 静态输出 | 24 次保留主图、细节图与当前结论，输出过程不改变现场 |
| 布局 | 320×420、640×240、960×720，无横向溢出；原生控件至少 44px；实际截图逐主题观察 |
| 总览 | 320/960/1440 三宽度均能切换九个单元，标题与确切版本元数据一致 |
| 正式读取 | 2 个服务测试通过；所有登记文件可精确分片重建，所选源码进入下一次受控 Provider 请求 |
| 正式 Author | 8 套作品 × 3 环境 = 24 组 preview，8 次确切版本交付；最终预览错误为空 |

浏览器结果见 [browser-report.json](browser-report.json) 和 [gallery-report.json](gallery-report.json)。[截图目录](screenshots) 保存八主题的手机/桌面主图与代表性的联动讲解。正式 Agent 读取和 Author 回执归档到 [acceptance.json](acceptance.json) 与 [author/results.json](author/results.json)。

观察截图后修正了流程未选对象的对比度、负数标签与类别的间距、公式换行。最终离散更新补上方向箭头；实际几何、操作、重开和导出已对该版本单独复验。元数据按新版模型重新编写，消除了旧图表的单位与刻度描述。

![联动升级后的模板总览](gallery.png)

## 复现入口

```powershell
node scripts/build-linked-methods.mjs
node scripts/verify-linked-methods.mjs
node scripts/verify-presentation-method-library.mjs --gallery-only
cargo test -p server --lib se2_ -- --nocapture
$env:METHOD_LIBRARY_AUTHOR_OUTPUT = 'E:\allwork\download\agent\understand-book\tmp\linked-methods-author-final'
$env:METHOD_LIBRARY_IDS = 'discrete-update,object-transform,compare-one-factor,part-whole,process-reveal,evidence-chain,chart-reveal,scale-zoom'
cargo test -p server --lib method_library_author_preview_and_deliver -- --ignored --nocapture
```

浏览器脚本可以在末尾指定方法 id，只检查对应单元。总验证入口不带 `--gallery-only` 时按选择分派二维/三维验证，再检查总览。以上不调用付费模型。

## 已知限制

本批验证的是制作资产及正式接入，未恢复 SE8 收益实验，未建立 Agent 自然选型、独立迁移收益或学习效果结论。当前为本地实现与验证；线上采用仍需后续构建发布。旧版及第九个三维模板保留原身份。
