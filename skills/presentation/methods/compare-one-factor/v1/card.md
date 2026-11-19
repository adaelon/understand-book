# 固定条件，只改变一个因素

method_id: `compare-one-factor` · revision: `1` · 状态：首批正式库

## 要解决的理解障碍

读者把初始条件、时间和参数变化混在一起，无法解释结果差异。

## 表达配方

1. 列出固定条件、唯一变化因素及结果量，画出共用尺度。
2. 同一时间推进两组；参数切换同时重置初始状态。
3. 在同一时刻显示两组值和差值，明确模型成立条件。

## 可运行作品

[完整源码](example/index.html) · [运行与适配](example/README.md) · [Author 元数据](example/author.json) · [预期检查](checks.md)

两条轨道从同一起点出发，经过同样的 10 秒。A 固定为 1 m/s，只改变 B 的速度。

## 适用与适配

有共同起点、时间和单位的明确模型；观察性数据不能直接解释为因果。替换关系式、参数和单位；保留共同条件、共用坐标和同步时钟。实际实验需要重新建立控制条件与来源。

## 来源与采用

参考 [huashu-art-motion / t3_finance_chart.md](https://github.com/alchaincyf/huashu-art-motion/blob/2571ee0a9a0467c4d99e935ea9e0fadda19210b0/references/动画语法/t3_finance_chart.md) 的表达机制，按 Reader 交互合同重新实现。首批登记与验证见 [模板库说明](../../README.md)；参考映射和许可见 [来源说明](../../SOURCES.md)。
