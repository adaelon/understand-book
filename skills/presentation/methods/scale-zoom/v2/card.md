# 保留定位的尺度缩放

method_id: `scale-zoom` · revision: `2` · 状态：联动升级版

## 要解决的理解障碍

读者看到放大细节，却丢失真实大小、单位和在整体中的位置。

## 联动作品

带物理坐标的标定条、固定全景窗口、中心位置轴与真实标尺。主图水平位置映射到 ±1 mm 的观察中心；下方中心定位轴也能定位。左右键每次移动0.05 mm。

## 表达配方

1. 先建立整体坐标和真实单位，指定要看的同一目标。
2. 用连续缩放连接尺度，上方保留整体定位与观察范围。
3. 同步更新标尺与倍率，局部对象的真实尺寸保持原值；允许随时回到整体。

## 可运行作品

[完整源码](example/index.html) · [运行与适配](example/README.md) · [Author 元数据](example/author.json) · [预期检查](checks.md)

从 1 m 的视野，逐级放大到 1 mm。被观察的青色线段始终长 1 mm。

## 适用与适配

可建立尺度与位置关系的对象；概念层级或示意大小不可伪装成物理比例。替换世界坐标、目标尺寸与单位换算；若不同层级并非真实缩放，改用层级展开并标明示意。

## 来源与采用

参考 [huashu-art-motion / y1_kurzgesagt.md](https://github.com/alchaincyf/huashu-art-motion/blob/2571ee0a9a0467c4d99e935ea9e0fadda19210b0/references/动画语法/y1_kurzgesagt.md) 的表达机制，按 Reader 交互合同重新实现。首批登记与验证见 [模板库说明](../../README.md)；参考映射和许可见 [来源说明](../../SOURCES.md)。
