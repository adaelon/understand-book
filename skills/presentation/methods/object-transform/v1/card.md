# 保持身份的对象变换

method_id: `object-transform` · revision: `1` · 状态：首批正式库

## 要解决的理解障碍

读者能看见前后两张图，但无法对应同一对象或说明不变量。

## 表达配方

1. 先画原对象、坐标和可追踪标记，确定每个对应点。
2. 按同一映射移动形状和标记，原形保留为参照。
3. 让不变量读数与变化参数同时可见；停下时保留当前形状。

## 可运行作品

[完整源码](example/index.html) · [运行与适配](example/README.md) · [Author 元数据](example/author.json) · [预期检查](checks.md)

把单位正方形剪切成平行四边形。追踪同一个角点，观察面积为什么始终为 1。

## 适用与适配

同一对象有明确映射和对应点；不同量纲或无对应关系的对象不作形变。保留对象身份和由参数重算全部点的结构；重写映射、定义域、不变量和坐标。面积不变只对本剪切成立。

## 来源与采用

参考 [huashu-art-motion / t1_3b1b.md](https://github.com/alchaincyf/huashu-art-motion/blob/2571ee0a9a0467c4d99e935ea9e0fadda19210b0/references/动画语法/t1_3b1b.md) 的表达机制，按 Reader 交互合同重新实现。首批登记与验证见 [模板库说明](../../README.md)；参考映射和许可见 [来源说明](../../SOURCES.md)。
