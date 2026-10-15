你已经知道 `Foo` 的赋值会移动；这里关键是先看**等号右边产生的是什么**。

```rust
let foo = Foo(0);
let bar = &foo;
```

第一行让 `foo` 拥有一个 `Foo` 值。第二行的右侧不是直接把这个 `Foo` 值交出去，而是用 `&foo` 创建一个指向它的共享引用。因此，赋给 `bar` 的是引用，`bar` 的类型是 `&Foo`；原来的 `Foo` 仍由 `foo` 拥有。借用就是这样在不取得所有权的情况下临时访问值。[[source:source_ref_b7cdf804a5f27a0d]] [[source:source_ref_9af277d1d3d785d4]]

可以把此时的关系画成：

```text
foo ──拥有──▶ Foo(0)
bar ──引用──▶ 同一个 Foo(0)
```

这里没有产生第二个 `Foo`。所以后面的两次打印走的是两条访问路径：`println!("Foo is {:?}", foo)` 使用仍然拥有值的 `foo`；`println!("Bar is {:?}", bar)` 通过引用 `bar` 显示它所引用的值。原文的代码允许这两次打印，正是因为 `let bar = &foo;` 没有移走 `foo` 拥有的值。[[source:source_ref_e2f1f69dfb7b32e3]] [[source:source_ref_1a3ec5f7971d4632]] [[source:source_ref_9af277d1d3d785d4]]

但这份访问权限有边界：`bar` 是不可变引用，你可以通过它访问这个值，却不能通过它修改 `Foo` 中的内容。借用并没有放宽所有权规则，它本身就是所有权模型的一部分。[[source:source_ref_9af277d1d3d785d4]] [[source:source_ref_b7cdf804a5f27a0d]]

现在只去掉第二行的 `&`，其他行保持原样：

```rust
let foo = Foo(0);
let bar = foo;
println!("Foo is {:?}", foo);
println!("Bar is {:?}", bar);
```

请你判断：这次赋给 `bar` 的是什么类型的值？赋值后谁拥有那个 `Foo`，两次打印中哪一次会出问题？
