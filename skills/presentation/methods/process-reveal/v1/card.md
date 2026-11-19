# 沿关系逐步展开过程

method_id: `process-reveal` · revision: `1` · 状态：首批正式库

## 要解决的理解障碍

读者只记住步骤名字，无法追踪每一步如何改变同一批输入。

## 表达配方

1. 先规划完整依赖图，再从输入节点开始展开。
2. 画出连线时保持上一步节点可见；节点显示真实计算结果。
3. 区分流程完成步和讲解过渡；回退重建已展开部分，结束时保留全图。

## 可运行作品

[完整源码](example/index.html) · [运行与适配](example/README.md) · [Author 元数据](example/author.json) · [预期检查](checks.md)

从 [2,3,4] 出发，逐项加数、按阈值筛选，再求和。每一步保留前面的结果，沿箭头追踪数据。

## 适用与适配

有明确输入、变换和依赖的有限流程；并行与循环需明确表示，不能画成错误线性顺序。替换输入、各步纯计算和依赖图；新增分支、并行或循环时明确画出对应结构。不要用连线位置冒充时间或因果。

## 来源与采用

参考 [huashu-art-motion / y3_whiteboard.md](https://github.com/alchaincyf/huashu-art-motion/blob/2571ee0a9a0467c4d99e935ea9e0fadda19210b0/references/动画语法/y3_whiteboard.md) 的表达机制，按 Reader 交互合同重新实现。首批登记与验证见 [模板库说明](../../README.md)；参考映射和许可见 [来源说明](../../SOURCES.md)。
