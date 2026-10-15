需要改的是这一环：**把 `&foo` 理解成“复制一份 `Foo`”**。`&foo` 创建的是对原值的不可变引用，赋给 `bar` 后，`bar` 的类型是 `&Foo`，不是 `Foo`。它借用的是 `foo` 中那个值，也不能通过它修改这个值。[[source:source_ref_9af277d1d3d785d4]]

因此，两个 `println!` 都能用，原因是借用没有把原值的所有权从 `foo` 转走：`foo` 仍拥有原值，`bar` 则通过引用访问它；这不表示复制出了两份 `Foo`。借用本来就是在不取得所有权的情况下临时访问值。[[source:source_ref_b7cdf804a5f27a0d]] [[source:source_ref_e2f1f69dfb7b32e3]]

现在你试着用自己的话解释：执行 `let bar = &foo;` 后，`foo` 和 `bar` 各自是什么关系，为什么接下来的两个 `println!` 都能使用它们？
