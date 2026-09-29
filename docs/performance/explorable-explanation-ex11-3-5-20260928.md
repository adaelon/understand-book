# EX11.3–EX11.5：局部媒体、异步恢复与短屏操作

日期：2026-09-28。基线 `c2ff3e1`，在已有 EX11.1–2 及其他未提交工作之上实施。工程验证采用实际 Manim 资产和正式 Author/Reader 路径；没有调用真实生成模型。

## 实现

- **EX11.3**：`render_animation(code,data,size,cues)` 固定运行 Manim 0.21.0/Cairo、30fps、无音频 MP4。独立 Python 环境，180 秒超时与取消进程树；PyAV 完整解码后读取元数据、校验定位点并返回首帧及代表帧。资源通过 `asset_refs` 写入不可变版本；源码/数据可分页读，媒体只返回元数据；修订可复用，旧版本缺省空动画集合。限制为每段 MP4 8 MiB、每版本媒体和海报合计 24 MiB，文本仍为 1 MiB。部署见 [Manim 环境](../Manim-部署.md)。
- **EX11.4**：Reader 与预览装配同版视频及海报，CSP 增加 `media-src data:`。预览的组装后文档上限从 2 MiB 调整为 96 MiB，容纳内联媒体及重复引用。共享媒体生命周期提供隐藏原生控件、playsinline、暂停其他视频、离屏/隐藏暂停及动态节点处理。bridge 等待异步 restorer 后才认证、捕获；恢复期间的捕获排队，自动保存被抑制，恢复失败可重试。制作方法更新到 `ex11.v6`。
- **EX11.5**：展开面板允许整体滚动，iframe 保留 160px 最小高度，保存提示回到追问区正常文档流。原模型页面未修改。

## 确定性验证与证据

| 路径 | 结果及证据 |
| --- | --- |
| Manim 实际渲染 | 2.2 秒、640×360、30fps、11,944 字节 MP4；本机该次约 4.73 秒。4 张真实解码帧，首尾位置不同。[元数据](ex11-local-demonstrations/ex11.3/render.json)。 |
| 版本资产 | 正式 write、持久版本、read 源码/数据/媒体元数据、based_on 跨回合复用通过；媒体正文未进入 Author 返回体，未知和漏选引用被拒绝。 |
| 错误、取消与限额 | 实际 Python 错误和超出时长的定位点返回诊断；取消后父/子 Python 进程退出，临时目录消失。[取消证据](ex11-local-demonstrations/ex11.3/cancellation.json)。8/24 MiB 限额与旧版本缺省兼容单测通过。 |
| 正式 Author 预览和交付 | 三规定视口，中途、回退、重复定位均完成，实际 screenshot/DOM 同步；deliver 成功；未解析的媒体引用被 preview 拒绝。[交付记录](ex11-local-demonstrations/ex11.4/delivery.json)。 |
| Reader 实际媒体 | 三视口分别检查 0、1.5、0.5、重复 0.5、2.2 秒的实际解码图像，蓝点像素位置与独立几何预期一致，媒体时刻误差不超过一帧。原生 controls 隐藏，playsinline 开启。 |
| 多演示恢复与追问 | A 停在 1.5 秒、B 停在 1 秒，active_demo=B；真实保存、宿主重开、恢复后保持暂停，追问同时携带两处现场及可见文字。[Reader 记录](ex11-local-demonstrations/ex11.4/reader.json)。 |
| 生命周期 | 播放 B 暂停 A；真实隐藏宿主后暂停，重新显示保持暂停；滚离后暂停；动态插入/移除视频同样受管理。[记录](ex11-local-demonstrations/ex11.4/lifecycle.json)。 |
| 异步与旧同步 bridge | 8 项浏览器测试通过，包含延迟恢复前零 observe/state、排队捕获、恢复拒绝、旧同步恢复及可见结果快照。 |
| 短屏修复 | 640×240 鼠标和触摸可实时改变原生 η 滑块，最后一排预设按钮和追问可用；原模型页三视口拖动、播放/暂停、回退、保存/重开及追问全部通过。[操作记录](ex11-local-demonstrations/ex11.5/after/reader-check.json) · [几何与事件](ex11-local-demonstrations/ex11.5/after/geometry.json)。 |
| 相关回归 | Runtime presentation 52 项通过，新增动画 schema/history 1 项通过；Server Author 串行 8 项通过；Web 装配/组件 5 项通过，vue-tsc 通过。Manim 与真实预览的 ignored 测试已单独显式运行。 |

## 定位中发现并修正的问题

短屏原失败实际是 **iframe 被挤到约 1.09px，innerHeight=1**。44px 滑块在 iframe 内部 y≈−22，中心点仍落在 iframe 上，不能据此判断触摸可操作；实际触摸事件没有送入页面。鼠标可偶发成功，触摸保持 η=1.1。修复后 iframe 160px，滑块完整目标可访问，两种输入均改到约 0.46。[修复前证据](ex11-local-demonstrations/ex11.5/before/geometry.json) 保留，原 EX11.2 报告作为历史不改写。

最初工程媒体页在恢复过程中等待 `requestVideoFrameCallback`，真实 Reader 重开未及时完成恢复，保留 [失败记录](ex11-local-demonstrations/ex11.4/restore-failure.json)。修改工程页和实际制作指导：定位等待 `seeked`、`HAVE_CURRENT_DATA` 并通过 `drawImage` 取得已解码图像；播放中仍以帧回调缓存实际媒体时刻。恢复不依赖尚未展示页面的合成器回调。修复后重开、实际像素及追问一致性均通过。

首版版本复用测试在已结束的回合尝试写修订，被既有 pending-turn 约束正确拒绝；夹具改为新建修订回合后通过。开发过程中的 Rust 枚举匹配/测试类型编译错误均已修正。

## 已知限制

- 当前真实媒体验证范围是本机 Windows 的 Manim/Cairo、正式 Edge 预览与 Chromium Reader。其他发布宿主和桌面 WebView 的媒体验收仍需在对应环境执行。
- 本轮为工程能力验收；ex11.v6 的跨题真实 Agent 制作属于 EX11.7。EX11.6 回合收尾未实施，学习效果未评估。
- 媒体仍保存在版本 JSON 中并内联为 data URL；这是本切片既定存储合同。没有新增媒体服务。
- 全部改动留在工作树，未创建 commit/PR；既有其他工作保留。
