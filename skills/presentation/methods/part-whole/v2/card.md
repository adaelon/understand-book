# 整体与部分保持对应

method_id: `part-whole` · revision: `2` · 状态：联动升级版

## 要解决的理解障碍

拆开以后丢失整体，或把面积、长度和占比混为一谈。

## 联动作品

条形拆分、100 单位格、整体比例与剩余比例。水平拖动改变 A，保留当前展开程度；百格图按行列定位 A 的数量。

## 表达配方

1. 先明确总量与互斥部分，用一条完整条形建立整体。
2. 保持每部分的长度、颜色和名称，将它们连续移到分解位置。
3. 同时显示各部分与总量；合回时恢复原位置，零项保持正确数值。

## 可运行作品

[完整源码](example/index.html) · [运行与适配](example/README.md) · [Author 元数据](example/author.json) · [预期检查](checks.md)

把总量 100 拆成 A、B、C 三部分。展开前后，每一块的长度、颜色和数值都保持对应。

## 适用与适配

互不重叠且穷尽整体的非负部分；重叠集合和有负贡献的数据须另建表示。替换部分定义、总量与分配关系，统一单位；使用有重叠或负贡献的数据时另选集合或差额表示。

## 来源与采用

参考 [huashu-art-motion / y1_kurzgesagt.md](https://github.com/alchaincyf/huashu-art-motion/blob/2571ee0a9a0467c4d99e935ea9e0fadda19210b0/references/动画语法/y1_kurzgesagt.md) 的表达机制，按 Reader 交互合同重新实现。首批登记与验证见 [模板库说明](../../README.md)；参考映射和许可见 [来源说明](../../SOURCES.md)。
