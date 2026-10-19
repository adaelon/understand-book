# 三维交互与图解联动模板

2026-10-16。用户认可完整齿轮作品后，要求提取三维交互与图解联动，供 Agent 制作其他主题。正式目录已登记 `spatial-linked-views@1`，catalog revision 3 共九个单元。

## 交付

- [语法卡](../../../skills/presentation/methods/spatial-linked-views/v1/card.md)、[改写指南](../../../skills/presentation/methods/spatial-linked-views/v1/example/README.md)、[预期观察](../../../skills/presentation/methods/spatial-linked-views/v1/checks.md)。
- [齿轮自包含 HTML](../../../skills/presentation/methods/spatial-linked-views/v1/example/index.html) 与 [弹簧自包含 HTML](../../../skills/presentation/methods/spatial-linked-views/v1/adaptations/spring.html)，均附对应 Author 元数据。
- 两个主题由同一份 [runtime.js](../../../skills/presentation/methods/spatial-linked-views/v1/runtime.js) 生成，提取的是渲染、观察、同步与保存机制。主题适配器提供模型公式、部件、二维映射与解释文字。
- Three.js 0.180.0 作为宿主固定库，与每个演示版本一同保存。静态导出支持保留 drawing buffer 的 WebGL 画布，Reader 现场通过原有桥恢复。

运行本地模板服务后打开 [齿轮预览](http://127.0.0.1:4197/spatial-linked-views/v1/preview.html?topic=gear) 或 [弹簧预览](http://127.0.0.1:4197/spatial-linked-views/v1/preview.html?topic=spring)。服务命令见库 README。

## 验证结果

| 验证 | 实际结果 |
|---|---|
| 独立公式与实际几何 | 42 组；检查 Three 对象变换、Canvas 标记/能量柱、速度/扭矩/能量与周期 |
| 正式 Author | 2 套作品 × 3 环境 = 6 组实际浏览器 preview，2 次版本交付；错误为空 |
| Reader 制作合同 | 真正的 presentationDocument、CSP、不透明 iframe、生产 bridge；6 次保存重开，参数/相机/拆解/分数与三维几何一致 |
| 静态导出 | 6 次三维+二维导出；三维 PNG 均有非透明像素与丰富颜色，导出前后状态相同 |
| 无 WebGL | 显示说明，二维图、参数、结果继续可用 |
| 丢弃 WebGL buffer | 导出明确要求 preserveDrawingBuffer；不会悄悄保存空画面 |
| Agent 读取 | 新模板 23 次分片读取进入 25 次受控 Provider 请求；完整登记文件均可还原，无付费模型调用 |
| Runtime 回归 | 110 个 presentation 相关测试通过，含指导选择、schema、目录登记、状态、压缩接续与交付 |
| 库版本与读取 | 2 个 managed library 测试、2 个 SE2 服务端测试通过 |
| 前端资源装配 | 5 个 document 测试通过，覆盖 Konva 和 Three 的确切版本与加载次序 |
| 原库与总览 | object-transform 的 12 组几何、3 次重开通过；总览 320/960/1440 宽度均可切换全部 9 单元 |

浏览器环境为本机 Edge，真实 WebGL。三环境为 320×420 touch、640×240 touch、960×720 mouse。导出核对使用生产静态捕获返回的三维 PNG 解码像素；没有以模拟的 Three 或手写 snapshot 替代实际渲染。

证据：[acceptance.json](acceptance.json)、[browser-report.json](browser-report.json)、[author/results.json](author/results.json)。Author 文件保留预览回执、截图和写入/交付回执。Reader 截图按实际滚动位置采集，避免把离屏 iframe 拉长截图导致 Chromium 漏绘文字。

![弹簧三维与图解的实际联动](spring-linked.png)

## 可重跑入口

```powershell
node scripts/build-spatial-method.mjs
node scripts/verify-spatial-method.mjs
node scripts/verify-presentation-method-library.mjs object-transform
cargo test -p runtime --lib presentation
cargo test -p server --lib presentation_libraries
cargo test -p server --lib se2_
pnpm --dir packages/web test presentation-document.test.ts
$env:METHOD_LIBRARY_AUTHOR_OUTPUT = 'E:\allwork\download\agent\understand-book\tmp\spatial-author-verification'
$env:METHOD_LIBRARY_IDS = 'spatial-linked-views'
cargo test -p server --lib method_library_author_preview_and_deliver -- --ignored --nocapture
```

库资源重建入口为 `node scripts/build-presentation-three.mjs`；读取已固定的上游副本，无运行期下载。Author 测试会同时选择本方法的齿轮主例和弹簧改写。

## 已知限制

本轮证明了共用核心可用于两个主题，以及正式读取、制作、保存和导出链可用。Agent 自然选型、独立迁移收益和读者学习效果没有本轮结论。SE8 继续暂缓，未调用付费模型。当前完成本地实现与验证，线上仍需后续构建发布。

运行核心采用正交相机、四周期观察窗和适合本例尺寸的灯光/投影范围。新模型应把物理单位明确换成场景坐标；明显超出本例约 ±150 场景单位时应同步调整灯光与相机范围。复杂碰撞、弹性接触、阻尼或任意外部三维资产需要对应的新模型和资源能力。
