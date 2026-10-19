# 参考来源与实现关系

参考项目：[alchaincyf/huashu-art-motion](https://github.com/alchaincyf/huashu-art-motion)。读取日期：2026-10-16；参考版本：`2571ee0a9a0467c4d99e935ea9e0fadda19210b0`。

本库借鉴其“语法卡配可运行片段”的组织方式和具体表达机制，围绕交互解释关系编写自包含 HTML/SVG/JavaScript。每张卡链接确切参考版本。方法命名沿本项目的理解障碍与关系，不以频道名或作者风格作为选用条件。

| 参考 | 提取的机制 | 本库落点 |
|---|---|---|
| [t1_3b1b](https://github.com/alchaincyf/huashu-art-motion/blob/2571ee0a9a0467c4d99e935ea9e0fadda19210b0/references/动画语法/t1_3b1b.md)、[片段实现](https://github.com/alchaincyf/huashu-art-motion/blob/2571ee0a9a0467c4d99e935ea9e0fadda19210b0/scripts/engine/clips/t1_3b1b.js) | 同一对象前后连续、语义色、对应点 | object-transform、discrete-update |
| [y3_whiteboard](https://github.com/alchaincyf/huashu-art-motion/blob/2571ee0a9a0467c4d99e935ea9e0fadda19210b0/references/动画语法/y3_whiteboard.md)、[片段实现](https://github.com/alchaincyf/huashu-art-motion/blob/2571ee0a9a0467c4d99e935ea9e0fadda19210b0/scripts/engine/clips/y3_whiteboard.js) | 预先建立关系图，逐步画出连接，保留上下文 | process-reveal |
| [y2_vox](https://github.com/alchaincyf/huashu-art-motion/blob/2571ee0a9a0467c4d99e935ea9e0fadda19210b0/references/动画语法/y2_vox.md) | 直接展示材料、标出所用依据、保留结论范围 | evidence-chain |
| [t3_finance_chart](https://github.com/alchaincyf/huashu-art-motion/blob/2571ee0a9a0467c4d99e935ea9e0fadda19210b0/references/动画语法/t3_finance_chart.md)、[片段实现](https://github.com/alchaincyf/huashu-art-motion/blob/2571ee0a9a0467c4d99e935ea9e0fadda19210b0/scripts/engine/clips/t3_finance_chart.js) | 先坐标后数据、基线与单位、一次强调一处差异 | chart-reveal、compare-one-factor |
| [y1_kurzgesagt](https://github.com/alchaincyf/huashu-art-motion/blob/2571ee0a9a0467c4d99e935ea9e0fadda19210b0/references/动画语法/y1_kurzgesagt.md) | 整体和局部连续对应、带定位的尺度变化 | part-whole、scale-zoom |
| [t2_keynote_ui](https://github.com/alchaincyf/huashu-art-motion/blob/2571ee0a9a0467c4d99e935ea9e0fadda19210b0/references/动画语法/t2_keynote_ui.md) | 同一对象展开、焦点明确、落定后停住 | 部分展开与总览的信息层次 |

## 授权

三维联动模板提取自本仓库用户已认可的 [齿轮完整作品](../../../examples/gear-transmission/README.md)。其讲解组织参考 [Ciechanowski 的 Gears](https://ciechanow.ski/gears/) 与 [Mechanical Watch](https://ciechanow.ski/mechanical-watch/)，不复制网站源码或素材。两个主题的模板与数学模型为本项目实现；Three.js 0.180.0 来自官方发行包，采用 MIT，[许可副本](vendor/THREE-LICENSE.txt) 随离线资源保留。宿主资产来源及重建方法见 [Three 资源说明](../../../assets/presentation/three-0.180.0/README.md)。

上游代码与文档采用 MIT；许可正文保存在 [LICENSE.huashu-art-motion.txt](LICENSE.huashu-art-motion.txt)。本批示例代码为本项目重新实现，页面无上游图片、角色、字体或影片资源。上游对字体、笔顺数据及角色形象有单独说明，详见其 [README 许可段](https://github.com/alchaincyf/huashu-art-motion/blob/2571ee0a9a0467c4d99e935ea9e0fadda19210b0/README.md#许可证)。

## 适配边界

原项目面向按口播时间生成影片；本库面向读者操作、参数比较、前后定位和现场恢复。示例由同一状态同步驱动 SVG、读数及控件，采用现有 presentationScene 和 presentation bridge。原项目的节奏统计和 QA 分数没有作为本项目的验收阈值。每个示例的预期结果由明确模型或展示数据决定。
