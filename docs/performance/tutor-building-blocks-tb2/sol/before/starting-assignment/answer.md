`let bar = &foo;` 之后还能分别打印，是因为这一步让 `bar` **借用 `foo` 的值，没有把值的所有权拿走**。先把“所有权”理解为：这个值归谁持有；借用则是临时访问这个值。[[source:source_ref_b7cdf804a5f27a0d]]

我们沿着原例走一遍：

```rust
let foo = Foo(0);
let bar = &foo;
println!("Foo is {:?}", foo);
println!("Bar is {:?}", bar);
```

第一行的 `Foo(0)` 是一个装着整数 `0` 的 `Foo` 值，`foo` 是持有它的变量。这里 `Foo` 是原文用 `struct Foo(u32);` 定义的类型名。你暂时只需把它看成一个装整数的小盒子。[[source:source_ref_4c3de5a75d5bc2f7]] [[source:source_ref_e2f1f69dfb7b32e3]]

第二行先计算右边的 `&foo`：`&` 创建一个引用，让代码可以借着这个引用访问 `foo` 的值。再把这个引用赋给 `bar`。所以这行赋值给 `bar` 的是**引用**，而借用本身没有取得那个 `Foo` 值的所有权。[[source:source_ref_b7cdf804a5f27a0d]] [[source:source_ref_9af277d1d3d785d4]]

`&Foo` 是 `bar` 的**类型名**，读作“对 `Foo` 的不可变引用”。表达式 `&foo` 负责创建引用，类型 `&Foo` 说明创建出来的是什么。你可以这样记这两者的关系：`foo` 持有那个小盒子，`bar` 是访问那个盒子的引用；不能通过 `bar` 修改盒子里的数。[[source:source_ref_9af277d1d3d785d4]]

于是，第三行仍可通过 `foo` 打印它持有的值，第四行则通过 `bar` 的引用打印所借用的值。原例的两行都能执行，关键就在第二行的 `&`：`bar` 获得了访问途径，`foo` 没有因这次借用失去值的所有权。[[source:source_ref_e2f1f69dfb7b32e3]] [[source:source_ref_b7cdf804a5f27a0d]] [[source:source_ref_1a3ec5f7971d4632]]

现在由你判断：保持原例的 `Foo` 和两行打印不变，只把第二行改成下面这样：

```rust
let bar = foo;
```

后面还能照旧先打印 `foo`，再打印 `bar` 吗？说说你的猜测，以及你认为这次赋给 `bar` 的是引用还是 `Foo` 值。
