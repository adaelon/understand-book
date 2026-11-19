# 引用后缀回归样例

`let item = &value;` 新建的是引用值。[source_ref_b7cdf804a5f27a0d]

这里的 `item` 是 `&Value`，可以读取它指向的数据。[source_ref_9af277d1d3d785d4]

把 `&value` 放在赋值右侧后，原变量仍然可读。[source_ref_1a3ec5f7971d4632] 两个变量都能用于只读展示。[source_ref_e2f1f69dfb7b32e3]

重复提到引用值时，继续使用同一个来源。[source_ref_b7cdf804a5f27a0d]

这个例子没有添加复制能力。[source_ref_4c3de5a75d5bc2f7]
