# EX11.1–EX11.2：正式 Konva 与制作指导

日期：2026-09-28。代码基线 `c2ff3e1`。EX11.1 实现与验收完成；EX11.2 制作指导已接入 `ex11.v5`，最终模型复验已交付并通过尺寸循环、数值/图元、源码续读及桌面/窄屏 Reader 验证。640×240 Reader 原生滑块拖动仍失败，留在 EX11.5 定位。工作树已有其他未提交工作。

## 实现

正式 `presentation.author.write` 接受可选 `libraries:["konva"]`，固定 Konva 10.7.0/MIT。产品资产位于 `assets/presentation/konva-10.7.0/` 并内嵌进 Server；候选保存独立库文件、许可和短 script 引用，资源正文计入既有 1 MiB 上限。

Author read 返回可编辑页面和库名、版本、许可路径、字节数；读取托管库路径返回元数据。based_on 修订去掉旧托管引用，按本次 libraries 显式重新装配并去重。预览解析版本库，Reader 复用既有 `presentationDocument` 本地脚本解析。同版重开使用保存资源，更新宿主库不改变旧页面。

真实修订发现通用结果投影会裁短源码 text，却保留原 next_offset。Runtime 增加源码专用投影：在输出预算内返回完整连续字符前缀，按实际返回字符重算 next_offset，不向可编辑代码插入 `[truncated]`。保留库元数据。中文源码逐段重建测试已覆盖该问题。

实际加载的方法为 `ex11.v5`。布局类改为按需使用；指导包含主图优先、合并重复读数、表示选择、响应尺寸、44px 命中区、坐标映射、确定性时间、零位移和状态接口。状态恢复按 `saved.values.page`；触控区域要避开绘图区裁剪和画布边界；ResizeObserver 合并到下一帧并跳过相同尺寸，避免展开时同步改变观察目标高度。未激活 Author 的普通回答不加载制作方法。方法 frontmatter 同时接受 LF/CRLF，覆盖本项目 Windows Git 自动换行转换。

## 工程验证

- Runtime presentation 相关 46 项通过；最终 ex11.v5 方法版本与按需加载测试通过；输出投影 9 项通过，包含连续中文源码重建。
- Server Author 串行回归 7 项通过；旧版本库保存/修订测试 1 项通过。
- Web 文档装配 3 项通过，包含同版库在业务脚本前只加载一次。
- 正式 Author 工程场景完成 write → 三视口 preview → deliver；库版本读回为 10.7.0。
- 工程点线场景 Reader 三视口实际鼠标/触摸拖动、保存、重开与追问通过；只有一个 Konva stage。
- 未声明 libraries、未知库、重复库、read 元数据、based_on 移除依赖、旧版本不变、总大小上限均有对应覆盖。

[工程证据](ex11-local-demonstrations/ex11.1/) · [复现命令](ex11-local-demonstrations/README.md)。

## 真实 Agent 诊断

通过正式 Resident 入口使用 deepseek-v4-flash/native。adapter 记录实际请求与响应，不改工具参数和制作提示。学习率问题要求同图比较、拖动、播放、暂停和回退；后续输入明确指定已有版本并反馈独立发现的问题。原始模型页面、请求、预览、失败和交付回执全部保留。

| 运行 | 结果 | Provider 已报告 token |
| --- | --- | ---: |
| ex11.2-run1 / v1 | 实际收到指导，激活工具后只承诺制作，没有 write。 | 18,367 |
| ex11.2-run2 / v2 | 一稿预览；deliver 被输出为普通文本，未交付。重复读数、静态边界、恢复状态存在问题。 | 319,379 |
| ex11.2-run3 / v3 | 自行修正窄屏溢出并正式交付。126 组数值/图元及实际拖动通过；时间离散、改参未归零。 | 747,062 |
| run3/revision / v3 | read 页面与库元数据后，Provider length 截断，未返回 write。 | 45,904，另一次请求用量未知 |
| run3/revision-compact / v3 | 正式修订交付，连续时间、改参归零和恢复修复；滑块实际命中高度仅 38px。 | 543,339 |
| run3/revision-hitarea / v4 | 正式修订交付，252 组数值/图元和边界命中检查通过；Reader 展开触发 ResizeObserver 循环错误。 | 956,620 |
| run3/revision-resize / v5 | 请求已发送，HTTP 402 Insufficient Balance；无模型响应、无新页面。 | 未返回 |
| run3/revision-resize-resume1 / v5 | 用户恢复余额后，正式修订交付，回合正常完成；尺寸循环修复及桌面/窄屏 Reader 通过。 | 566,056 |

合计实际发送 54 次请求，Provider 已报告 **3,196,727 token**；两次请求用量缺失，总量保持未知。最新模型修订耗时 167.25 秒、12 次请求。全部请求与原始工具参数未含 Konva 库正文；每次保存的交付版本只有一个托管引用。模型修订实际使用 read/based_on 与显式 libraries。

源码投影缺陷的实际记录见 `revision-hitarea/request-04.json`：源码被投影成约 2014 字符，next_offset 却前进 4000。该批次在修复前运行；首次 v5 请求因余额不足未进入 read；恢复后的真实请求记录了 36 个源码片段观察，全部 next_offset 与实际字符前缀一致，补齐了模型链路验证。

## 密度、数值与操作结果

[同视口、同参数截图](ex11-local-demonstrations/density/observations.json) 固定 η=0.8、k=2、p=0。原第二批与命中区修订 v4 页的首图位置分别为：桌面 y=761.5 → 82.3，窄屏 y=1303.3 → 128.8，短屏 y=854.1 → 82.3。改进来自三类轨迹共用坐标、主图前移、合并重复读数；不使用密度评分。前后截图均保留，v4 原稿未改。

独立递推 `w(k+1)=w(k)−2η(w(k)−2)` 覆盖 7 个 η × 6 个步骤 × 2 个视觉进度 × 3 个视口，共 252 组。最后模型页与本地尺寸修正版均通过数值、实际图元、当前插值位置、零位移箭头检查。文本按页面显示的两位小数核对，图元按原始浮点坐标核对。

边界实测：时间拖动点在 k=0/8 时至少 44×44 CSS px；学习率拖动柄在 η=0/1.5 时为 48×48。改参立即暂停归零；中途暂停保持小数进度；保存/恢复 k=2、p=0.5 保持暂停。

## 本地尺寸修正与 Reader

[engineering-resize-repair](ex11-local-demonstrations/ex11.2-run3/engineering-resize-repair/) 是明确标注的**本地工程修正**：以最后模型原稿为基础，只替换尺寸回调，保存 before/after 和 origin。它不计入模型交付成功。

- 三视口下，时间点和学习率 Canvas 拖动共 6 组真实鼠标/触摸操作通过：按住时实时变化，松开才 commitState。
- Reader 960×720、320×420：展开、原生滑块拖动、播放/暂停、回退、保存、重开和追问通过；实际追问携带当前现场。
- 桌面约 5 秒播放观测：209 帧，帧间隔 P95 33.4ms，MutationObserver P95 2.2ms，1 次保存、206 次文本观察。该值为本机一次诊断记录。
- Reader 640×240：上一步按钮被宿主标题/追问表单拦截。完整三视口 Reader 测试因此失败；前两个视口的通过记录已保存。该问题归属既有 EX11.5。

[独立数值与状态](ex11-local-demonstrations/ex11.2-run3/engineering-resize-repair/independent-continuous.json) · [实际 Canvas 拖动](ex11-local-demonstrations/ex11.2-run3/engineering-resize-repair/pointer-check.json) · [Reader 结果](ex11-local-demonstrations/ex11.2-run3/engineering-resize-repair/reader-check.json)。

## 余额恢复后的真实模型复验

证据：[`revision-resize-resume1/verdict.json`](ex11-local-demonstrations/ex11.2-run3/revision-resize-resume1/verdict.json)。使用原 `repair-resize-input.txt`、确切 v4 已交付版本、ex11.v5 方法和正式 Resident 入口，新目录保留全部原始记录。

- 模型正式交付 revision 3，`incomplete=false`、无 warning。三视口 preview 完成，Reader 展开未再触发 ResizeObserver 循环错误。
- 12 次请求全部返回用量，共 566,056 token；耗时 167.25 秒。实际请求装载 ex11.v5，36 个源码片段观察的续读位置全部连续，未含 Konva 库正文，保存版本只有一个库引用。
- 原始模型新页通过 252 组独立数值/实际图元、零位移、边界命中、状态恢复与连续时间检查；6 组真实 Canvas 鼠标/触摸拖动和松开提交通过。无需本地修改模型页面。
- Reader 桌面、窄屏：展开、原生滑块拖动、播放暂停、回退、保存、重开和追问通过。桌面一次约 5 秒观测为 264 帧、帧间隔 P95 33.4ms、观察回调 P95 1.2ms、1 次保存/261 次文本观察。
- 模型还重写了布局和交互代码，并非最终回答声称的“只改尺寸调度”。实际差异保存在 `source-diff.txt`，因此重新跑了完整数值与行为验证。测试按实际页面更新 selector，并从轴刻度读取坐标范围；新页 clip 比坐标轴多出 2px，不能继续把 clip 当作轴范围，数学预期及误差容限保持。

## 已知问题与未闭环项

- 640×240 Reader 的原生 η 滑块在实际触摸拖动期间仍保持 1.1，拖动检查失败；其播放暂停、回退、恢复和追问通过。起点宿主命中为 iframe，不能仅凭历史记录断言仍是表单遮挡。具体原因留在 EX11.5；完整三视口 Reader 测试仍未全绿。
- 模型未严格限制改动范围，并在最终文字中错误声称其余逻辑未改；原始源码差异与测试结果已保留，后续 EX11.7 应继续观察修订稳定性。
- 旧 v4 模型页的尺寸循环错误、早期离散时间/恢复/命中区错误、length/402 失败与本地工程修正版都保留。402 已解除，不再是当前阻断。
- Manim、异步恢复、短屏 Reader 和回合收尾按 EX11.3–6 独立处理；跨题稳定性留在 EX11.7，学习效果尚未评估。
- Author 夹具的共享名称与毫秒文件名曾使早期并行回归碰撞；串行回归通过。工作树改动未提交，既有其他工作保留。模型总用量和费用不以缺失数据推算。
