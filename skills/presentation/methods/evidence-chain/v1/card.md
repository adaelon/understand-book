# 从证据走到有边界的结论

method_id: `evidence-chain` · revision: `1` · 状态：首批正式库

## 要解决的理解障碍

读者看到结论，却分不清原始记录、计算、前提和可推断范围。

## 表达配方

1. 直接展示材料与明确出处，分开记录和作者推断。
2. 让计算紧邻原始数值，逐步画出支持关系。
3. 显式列出结论所需条件；缺条件时同步收窄结论，不仅改变颜色。

## 可运行作品

[完整源码](example/index.html) · [运行与适配](example/README.md) · [Author 元数据](example/author.json) · [预期检查](checks.md)

用三组构造的耗时记录练习核对结论：先看记录，再算均值，最后检查比较条件。

## 适用与适配

每条关键主张有可展示依据；构造材料必须标明，统计描述不能自行升级为因果或总体结论。替换原始材料、来源引用、计算与成立条件；正文和 source_ref_ids 一起重新建立，不能保留构造记录当真实证据。

## 来源与采用

参考 [huashu-art-motion / y2_vox.md](https://github.com/alchaincyf/huashu-art-motion/blob/2571ee0a9a0467c4d99e935ea9e0fadda19210b0/references/动画语法/y2_vox.md) 的表达机制，按 Reader 交互合同重新实现。首批登记与验证见 [模板库说明](../../README.md)；参考映射和许可见 [来源说明](../../SOURCES.md)。
