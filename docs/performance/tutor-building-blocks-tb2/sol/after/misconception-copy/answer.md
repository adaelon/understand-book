该改的是这一步：**你把右侧的 `&foo` 理解成了“复制一份 Foo”。** `&foo` 实际上创建的是对原来那个值的引用；借用让代码临时访问值，并不取得这个值的所有权。[[source:source_ref_b7cdf804a5f27a0d]]

沿用你熟悉的赋值思路，把这一行拆开看：

- 先看右侧 `&foo`：产生一个不可变引用。
- 再看 `let bar = ...`：把这个引用绑定给 `bar`，所以 `bar` 的类型是 `&Foo`，不是 `Foo`。[[source:source_ref_9af277d1d3d785d4]]

可以用下面这张关系图理解原例：

```text
foo 拥有 ────────────> Foo(0)
                         ↑
bar 保存的不可变引用 ────┘
```

图里只有原来的那个 `Foo(0)`。`bar` 提供了访问它的另一条路径，但不能通过 `bar` 修改其中的值。[[source:source_ref_b7cdf804a5f27a0d]][[source:source_ref_9af277d1d3d785d4]]

现在请你用自己的话解释原例中的这两行：

```rust
println!("Foo is {:?}", foo);
println!("Bar is {:?}", bar);
```

**它们为什么都能使用？** 试着把“谁仍拥有那个 Foo”和“bar 保存了什么”连起来说明，不再用“复制了两份 Foo”作为理由。
