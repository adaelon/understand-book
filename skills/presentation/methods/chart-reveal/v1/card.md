# 先建立尺度，再揭示数据

method_id: `chart-reveal` · revision: `1` · 状态：首批正式库

## 要解决的理解障碍

动画先吸引注意，读者却找不到单位、基线或数值差异。

## 表达配方

1. 先固定坐标、单位、零基线和来源；保留原始数据表。
2. 按真实数值从零展开柱形，过渡与实际观测值明确区分。
3. 聚焦一处比较，算式与图上数据一致；不同口径不直接形变连接。

## 可运行作品

[完整源码](example/index.html) · [运行与适配](example/README.md) · [Author 元数据](example/author.json) · [预期检查](checks.md)

固定坐标和单位，依次展示四批数据，再聚焦首末差异。切换数据可观察负值与零值。

## 适用与适配

准确数据、统一单位和明确基线；负起点不用增长倍数，示意数据不得冒充观测。替换序列、标签、单位、来源及坐标域。柱形必须保留零基线，负值从零向下。涉及百分比时重新判断分母与解释口径。

## 来源与采用

参考 [huashu-art-motion / t3_finance_chart.md](https://github.com/alchaincyf/huashu-art-motion/blob/2571ee0a9a0467c4d99e935ea9e0fadda19210b0/references/动画语法/t3_finance_chart.md) 的表达机制，按 Reader 交互合同重新实现。首批登记与验证见 [模板库说明](../../README.md)；参考映射和许可见 [来源说明](../../SOURCES.md)。
