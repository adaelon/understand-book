你说这次是猜的，那我们就把“选哪一行”接成一条能用的理由。

先看原文的 `let bar = &foo;`：`&foo` 创建的是引用，类型为 `&Foo`；`bar` 接收这个引用，`foo` 仍拥有原来的 `Foo`。因此可以借助 `foo` 访问值，也可以借助 `bar` 的不可变引用访问同一个值，但不能通过 `bar` 修改它。[[source:source_ref_9af277d1d3d785d4]] 原文的两次打印都成立。[[source:source_ref_1a3ec5f7971d4632]]

接下来补上一个用于判断赋值的规则：**在这里没有显式类型标注时，新绑定的类型由右侧决定；把一个非 Copy 的值交给新绑定，会转移这个值的所有权，原绑定随后不能再用来访问这个已移出的值。** 这是补充的 Rust 规则。原文的 `Foo` 只派生了 `Debug`，没有实现 `Copy`，所以这里适用非 Copy 的移动规则。[[source:source_ref_4c3de5a75d5bc2f7]]

用一个不同的例子把规则走通：

```rust
let old = String::from("hello");
let new = old;
```

这是补充例子。第二行右侧是 `old` 中的 `String` 值，没有 `&` 来创建引用，所以 `new` 的类型是 `String`。`String` 是非 Copy 类型，这次赋值把值交给 `new`；之后可以使用 `new`，却不能再通过 `old` 使用那个值。注意：移动改变所有权归属，不把 `String` 变成 `&String`。

现在请你把这条链用于你刚才的例子：

```rust
let foo = Foo(0);
let bar = foo;
println!("Foo is {:?}", foo);
println!("Bar is {:?}", bar);
```

请用自己的话接完一句：**“第二行右侧是___，因此 `bar` 的类型是___；因为 `Foo` 是非 Copy，赋值后值由___拥有，所以打印___的那一行不能成立。”** 这次先从右侧表达式往下推，不从刚才选中的行倒猜。
