# EX10 第三批：修复装配后的 SVG / Konva 对照

日期：2026-09-28。用户要求重新做批次后，以相同任务和资料独立运行 B1/C1。**两组这次均保存了正式交付页面，C 不再因库源码进入历史而失败；但两份页面均未通过完整质量验收，第三批按合同停止。** C 的最终 Resident 回执另外标记 `incomplete=true / TURN_LIMIT_EXCEEDED`。

[操作最新两页及历史样本](ex10-svg-konva/index.html) · [第三批冻结记录](ex10-svg-konva/batch3/frozen.json) · [成本数据](ex10-svg-konva/batch3/cost-summary.json) · [前两批报告](explorable-explanation-ex10-svg-konva-20260928.md)。

## 批次条件与实际经过

第三批于 09:49 冻结。仍为 deepseek-v4-flash、native、temperature=0、ed3.v1，input/common/B-api/C-api 与前两批逐字一致。继续使用 Konva 10.7.0 和修复后的 test-only Author Write 装配。真实请求 `output_token_limit=null`，没有新增累计 token 或墙钟门槛；沿用 Runtime 原有停止机制。先运行的两项装配回归均通过。

B1 两稿交付。Agent 在预览中发现短屏场景过高、坐标轴离开可视区域，自行修订布局。C1 四稿交付：依次修正 semantic_state 类型、写只读 initialState、播放未暂停即回读、短屏标签裁切和窄屏横向溢出。以上都是同一次会话中的预览修订，全部计入成本，候选未被人工修改。

C 的实际请求历史中，write HTML 保持 36–38 KB 的原始场景和库标记；保存的完整候选约 230 KB。逐次历史检查 `history_contains_fixed_library=false`，证据见 [C1 audit](ex10-svg-konva/batch3/runs/C1/audit.json)。这给出了真实 Provider 路径上的装配修复证据。

C 第 10 个工具循环完成 deliver，随后有最终回答请求，最终回执为 12 turns、`TURN_LIMIT_EXCEEDED`。已保存的版本可以通过实际 presentation.read 读取，但整个回答回合被标为不完整。本次未出现 Provider `length` 或 `ACTIVE_CONTEXT_EXHAUSTED`；页面交付与运行完整性分别记录。

## 验收结果

每组覆盖三视口 × 7 个 η × k=0…5，共 126 组。独立递推核对模型闭式输出，并读取真实 SVG 变换坐标或 Konva 对象；同时检查实际截图，避免只核对自报数值。

| 项 | B1 · SVG | C1 · Konva |
| --- | --- | --- |
| HTML 数值与表格 | 126 组通过 | 126 组通过 |
| 实际端点与箭头线段坐标 | 126 组通过 | 126 组通过；零位移箭头头部另有错误 |
| 边界解释 | η=0.5 标题误写“来回跳”；静态说明及最终回答把 η=0 纳入单调靠近 | 7 个测试 η 的动态判据正确，包括 0、0.5、1、1.2 |
| 中途回退 | 三视口独立操作正确 | Reader 桌面、窄屏正确 |
| 用户定位动画时间 | 缺少进度控件，seek 只在程序接口中 | 能真实拖动圆点定位当前一步，鼠标与两种触屏尺寸均通过 |
| 触控目标 | HTML 控件已记录尺寸 | 圆点命中区域桌面 24×25、窄屏 15×17、短屏 16×17 CSS px，低于约定 44×44 |
| 主动状态提交 | 使用 commitState，五秒播放记录 6 次保存 | 调用不存在的 presentation.commit，五秒播放仅开始时保存 1 次 |
| Reader 保存重开与追问 | 桌面、窄屏匹配；短屏回退按钮被宿主表单遮挡 | 桌面、窄屏匹配；短屏补验同样被宿主表单遮挡 |
| 页面交付 / 最终回执 | 已交付 / complete | 已交付 / incomplete，TURN_LIMIT_EXCEEDED |

B 在 η=0.5 时多个重合端点标签仅分两行，w₁…w₅ 重叠而无法逐个读清，见 [桌面截图](ex10-svg-konva/batch3/runs/B1/independent-960-eta0.5-k1.png)。其动态详细解释正确写出“一步到达”，但标题与之矛盾；静态 η<0.5 的结论仍漏掉零学习率边界。

C 的零位移图形需区分线段数据与实际绘制：η=0，以及 η=0.5 到达后，箭头端点虽重合，Konva.Arrow 仍绘制有方向的头部。η=0 的实际图元包围盒为 9.6×9.6，而更新量为零。因此线段坐标通过不等于完整箭头表达通过，见 [独立记录 zero_update_arrowheads](ex10-svg-konva/batch3/runs/C1/independent-check.json)及[截图](ex10-svg-konva/batch3/runs/C1/independent-960-eta0.5-k1.png)。

C 圆点尺寸依据 Konva 实际命中图逐像素读取，真实触摸拖动成功不抵消小于 44 px 的目标要求。状态提交错误同时有原稿代码、独立宿主 commitState 调用数为零，以及 Reader 五秒播放只保存一次的证据；最终回答却声称每跨一步都会记录状态，这一声明不成立。

完整记录：[B1 独立检查](ex10-svg-konva/batch3/runs/B1/independent-check.json)、[B1 Reader](ex10-svg-konva/batch3/runs/B1/reader-check.json)、[B1 判定](ex10-svg-konva/batch3/runs/B1/verdict.json)；[C1 独立检查](ex10-svg-konva/batch3/runs/C1/independent-check.json)、[C1 Reader](ex10-svg-konva/batch3/runs/C1/reader-check.json)、[C1 判定](ex10-svg-konva/batch3/runs/C1/verdict.json)。

## 成本及归因

| 指标 | B1 · SVG | C1 · Konva |
| --- | ---: | ---: |
| 模型请求 | 8 | 12 |
| Provider 总 tokens | 705,631 | 1,303,284 |
| 生成总墙钟 | 451.101 秒 | 514.168 秒 |
| write / preview / deliver | 2 / 7 / 1 | 4 / 12 / 1 |
| 成功 preview 回执 | 6 | 7 |
| 首次 write 时间 | 265.774 秒 | 264.924 秒 |
| 最终模型原稿 HTML | 29,711 字节 | 37,588 字节 |
| 最终场景 JS | 20,682 字节 | 25,764 字节 |
| 装配后完整 HTML | 29,711 字节 | 230,479 字节 |

本批共 **2,008,915 tokens**，20 次请求用量均已返回，输入/输出拆分仍未知。C 的 tokens 比 B 多约 84.7%，总墙钟多约 14.0%，写入和修订次数更多；这一对样本没有显示降低制作成本。C 库装配四次分别耗时 4,815 / 894 / 806 / 874 微秒。首次真实拖动可用时间未在生成期间测量，不能用首次 write 代替。

五秒桌面 Reader 观察：B 帧间隔 P95 33.4 ms、观察回调 P95 4.1 ms、6 次保存、202 次 observe；C 为 33.3 ms、2.6 ms、1 次保存、113 次 observe。C 页面内容及保存行为不同，且漏了提交，不能把较少 observe 或较低局部开销当作库的性能优势。

## 结论与后续入口

第三批回答了此前缺失的一点：**修复装配后，Agent 可以使用 Konva 完成页面制作、预览修订并交付版本；库源码没有再次进入调用历史。** 当前这对样本中，C 的边界解释和可拖动动画位置优于 B，代价是更多请求、tokens、代码和修订。

**采用结论仍是暂时保留现状，不将 Konva 设为默认能力。** 理由是 C 存在触控、状态提交、零位移图形错误，最终回执不完整，又只有一对样本，未达到合同要求的三次合格且重复出现收益。这个决策不意味着 SVG 已被证明更好；本批 B 同样不合格。

第三批在 B1/C1 后停止，不运行 B2/C2/B3/C3，不手修原稿补算成功。后续工作已从“验证库能否装配”转为真实缺口：Agent 正确使用状态提交接口、Canvas 触控与零位移表达、Reader 短屏遮挡，以及 Runtime 回合上限下的完整收尾。是否再做新批次，应以这些缺口的处理为基础。生产保持 ed3.v1，EX2/EX7 的状态不变。

## 已知限制与用户反馈

- 报告初次收口时尚无用户反馈；2026-09-28 后续用户明确认可 Konva 的排版与效果，并要求继续开发通用能力。该反馈支持后续投入；跨主题稳定性和学习收益尚未测量。后续方向见 [通用能力工作记录](ex10-konva-studio/README.md)。
- Reader 重开测试显式调用 commitState 后核对宿主链路；这不代表 C 原稿能主动提交。C 的实际提交缺陷已单独判失败。
- C Reader 初次运行在关闭已完成的浏览器上下文时遇到测试清理错误；桌面/窄屏记录已保存。修复测试清理后仅补验短屏，真实按钮遮挡复现，分别见 [初次日志](ex10-svg-konva/batch3/runs/C1/reader-run-01-cleanup-error.log)及[短屏日志](ex10-svg-konva/batch3/runs/C1/reader-run-02-short.log)。这不是重新采样或修改候选。
- SVG 屏幕变换使用浏览器浮点坐标，测得最大数值偏差约 1.81×10⁻⁶，几何容差采用 10⁻⁵ 模型单位。原稿未修改。
