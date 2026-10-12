先把 `foo` 和 `bar` 分清：这里有一个 `Foo(0)` 值，`foo` 拥有它，`bar` 借用它。借用让代码临时访问值，却不取得它的所有权。[[source:source_ref_b7cdf804a5f27a0d]]

你可以先把 `Foo(0)` 看成一个装着整数 `0` 的小盒子；这是帮助理解的比喻。原文的 `struct Foo(u32);` 定义了这种包装整数的类型，`Foo(0)` 就是其中一个值。[[source:source_ref_4c3de5a75d5bc2f7]]

为了读懂这一步，补充一条原文没有展开的 Rust 规则：**所有权决定谁负责一个值；对没有实现 `Copy` 的类型，把值直接赋给另一个变量会转移所有权。** 你熟悉的整数实现了 `Copy`，所以整数赋值可以复制。这个例子的 `Foo` 只派生了 `Debug`，没有实现 `Copy`；`Debug` 用来支持 `{:?}` 打印，并不让 `Foo` 具有整数那样的复制能力。

现在按原文执行：

```rust
let foo = Foo(0);
let bar = &foo;
println!("Foo is {:?}", foo);
println!("Bar is {:?}", bar);
```

- `let foo = Foo(0);`：创建值，让 `foo` 拥有它。
- `let bar = &foo;`：右边的 `&foo` 创建引用，把这个引用赋给 `bar`。`bar` 的类型是 `&Foo`，可以读作“对一个 `Foo` 值的共享引用”。这里没有把 `Foo` 的所有权交给 `bar`，也没有复制出第二个 `Foo`。[[source:source_ref_b7cdf804a5f27a0d]] [[source:source_ref_9af277d1d3d785d4]]
- 第一条 `println!` 仍能通过 `foo` 读取原来的值；第二条能通过 `bar` 的引用读取同一个值。因此两条打印都可以执行。原文明确说明这个程序可以编译。[[source:source_ref_e2f1f69dfb7b32e3]] [[source:source_ref_1a3ec5f7971d4632]]

再补充一点：这里的 `println!` 格式化打印会借用它要显示的内容，不会因为打印而把 `Foo` 的所有权拿走。两次看到的值都显示为 `Foo(0)`。

所以这一步的关系是：**`foo` 拥有值，`bar` 持有访问这个值的引用。** `bar` 是不可变引用，因此不能通过它修改 `Foo` 中的整数。[[source:source_ref_9af277d1d3d785d4]]

现在轮到你判断，只改第二行，其他行保持原样：

```rust
let foo = Foo(0);
let bar = foo; // 去掉了 &
println!("Foo is {:?}", foo);
println!("Bar is {:?}", bar);
```

用刚才补充的规则想一想：第二行之后，谁拥有这个 `Foo(0)`？接下来的 `println!("Foo is {:?}", foo)` 还能通过编译吗？请把这两个判断连成一句理由。
