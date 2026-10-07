# EX12.3 长页阅读位置预览

2026-10-02。状态：**EX12.3 已完成，下一入口 EX12.4。** 合同见[切片方案](../切片方案-演示页全局框架与分阶段制作.md)与 [ADR-0154](../adr/0154-presentation-global-framework-and-staged-authoring.md)。本地基于 `5e10516` 的现有工作树，方法资产为 `ex12.v3`。

## 调用与观察合同

```json
{
  "operation": "preview",
  "candidate_id": "本轮候选",
  "viewport": {"width": 960, "height": 720, "input": "mouse"},
  "actions": [{"kind": "scroll", "y": 3600}]
}
```

`y` 是文档顶部起算的非负整数 CSS 像素，类型为 u32。超过页底时停在实际页底；缺少 y、负数或小数不符合参数合同。每次 preview 从新页面开始，同批动作按顺序执行。scroll 使用浏览器即时文档滚动，不聚焦、点击或发送按键；原有四动作限制、取消、超时和候选归属继续生效。

每次观察（包括初始加载、click/key/seek 后）包含 `step`、`action` 和 `scroll`：实际 x/y、max_y、viewport_width/height。DOM 新增 `visible_text`，从与视口相交的文本节点提取文字；原全页文字、控件和元素坐标继续保留。截图说明同时带候选、环境、步骤、动作和实际位置。`read_selector` 的原子 reading 也携带实际位置。

长页结果超预算时，Runtime 先省略完整 DOM/layout，再缩短视口文字，保留每次观察的动作、位置、场景和问题；若这些元数据本身也放不下，返回明确的预算回执，要求减少单次动作。历史精简保留 preview 的候选、视口、动作和读取区域参数；历史回执仍只证明先前调用，不恢复图片观察或候选资格。

## 实际长页结果

固定三段测试页在 320×420 touch、640×240 touch、960×720 mouse 三个环境下执行中段、超过页底、返回顶部。断言浏览器 layout 的 pageY、DOM 元素坐标、PNG 尺寸与指定位置的背景像素一致；按钮计数和滑块值始终未变。页面启用 CSS 平滑滚动，scroll 仍立即定位。

| 环境 | 初始 | 中段 | 超界请求后的实际页底 | 返回顶部 |
| --- | ---: | ---: | ---: | ---: |
| 320×420 touch | 0 | 950 | 2280 | 0 |
| 640×240 touch | 0 | 950 | 2460 | 0 |
| 960×720 mouse | 0 | 950 | 1980 | 0 |

证据：[窄视口](presentation-staged-authoring-ex12-3/narrow-content.json)、[短视口](presentation-staged-authoring-ex12-3/short-content.json)、[桌面](presentation-staged-authoring-ex12-3/desktop-content.json)；每份记录对应同目录 `环境名-0..3.png`。

原 EX12.0 归档页也通过生产 preview probe 在 960×720 重放：首屏 y=0；请求 y=3600 到达“第 3 步：为什么是 max，而不是相加？”附近；请求 y=999999 停在实际页底 y=6347，显示模型条件、三个常见误解与结尾总结。三个观察均无浏览器错误或布局问题，所有原生控件记录相同。页面与 Konva 资源来自原归档，未调用模型重生成。

![原问题页中段](presentation-staged-authoring-ex12-3/baseline-1.png)

![原问题页页底](presentation-staged-authoring-ex12-3/baseline-2.png)

完整记录为 [baseline-observations.json](presentation-staged-authoring-ex12-3/baseline-observations.json)，复跑入口为 [replay-baseline.py](presentation-staged-authoring-ex12-3/replay-baseline.py)。

## 验证

运行前确定的失败目标：滚动未执行、请求位置被误报为实际位置、截图与 DOM 不对应、滚动触发控件、大 DOM 裁剪丢失位置、图片或预览资格串到另一候选，以及旧 click/key/seek/取消行为回退。失败时修对应执行、投影或绑定路径。

| 验证 | 结果 | 日志 |
| --- | --- | --- |
| Runtime EX12 定向 | 12 通过，其中新增 3 项 | [runtime-tests.log](presentation-staged-authoring-ex12-3/runtime-tests.log) |
| 三环境真实滚动、像素与 DOM | 1 通过，覆盖三个环境 | [browser-tests.log](presentation-staged-authoring-ex12-3/browser-tests.log) |
| 原有浏览器预览路径 | 11 通过，含点击、按键、结果回读、错误、超时、取消、进程清理 | [browser-regression.log](presentation-staged-authoring-ex12-3/browser-regression.log) |
| Server author 图片、reading 与候选资格 | 1 通过，覆盖三个环境 | [author-tests.log](presentation-staged-authoring-ex12-3/author-tests.log) |
| 原 seek 定位与错误回报 | 2 通过 | [seek-regression.log](presentation-staged-authoring-ex12-3/seek-regression.log) |
| 原 EX12.0 长页重放 | 通过，首屏／中段／页底可观察 | [baseline-replay.log](presentation-staged-authoring-ex12-3/baseline-replay.log) |
| Runtime 全库 | 436 通过、1 失败、3 忽略；失败用例单独复跑通过 | [完整回归](presentation-staged-authoring-ex12-3/runtime-regression.log)、[单例复跑](presentation-staged-authoring-ex12-3/runtime-retry.log) |

新增 Server author 测试的两个断言已校正：宿主公共 CSS 带 12px body padding，应按实际布局检查页底；JSON 原始整数和类型化 f64 应按数值比较。这两项校正未改生产代码。原长页重放最初按 id 比控件，但来源按钮有重复空 id，改为按文档顺序比较整个控件数组。

## 已知限制

Runtime 全库的 `native_adapter_maps_provider_safe_tool_names_back_to_runtime_names` 首次运行中，本地测试服务读到空请求后断言失败，随后客户端连接被拒绝；其请求读取超时为 500ms。单独复跑通过，未修改该无关用例，保留首次失败记录。编译仍有既有 ts-rs serde 属性和 Server 未使用函数警告。

本切片证明长页观察能力；未进行真实模型同底座生成对照、Linux 部署或 Reader 重开恢复，后续入口为 EX12.4/5。`visible_text` 是与视口相交文本节点的摘要，不能代替对遮挡、图形、裁剪的截图判断。scroll 定位顶层文档，不定位页面自行创建的嵌套滚动容器。
