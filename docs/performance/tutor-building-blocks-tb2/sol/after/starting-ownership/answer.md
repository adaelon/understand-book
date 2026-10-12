你已经知道非 Copy 的值交给新绑定会移动。这里要接上的一步是：**先看右侧表达式产生什么，再看它交给了哪个绑定。**

在原文这两行里：

```rust
let foo = Foo(0);
let bar = &foo;
```

第一行让 `foo` 拥有一个 `Foo` 值。第二行的右侧是 `&foo`：它创建一个指向这个值的不可变引用，类型是 `&Foo`；`bar` 接收的是这个引用。创建引用不取得被引用值的所有权，所以 `Foo` 的所有权仍在 `foo` 那里。[[source:source_ref_b7cdf804a5f27a0d]][[source:source_ref_9af277d1d3d785d4]]

可以把此时的关系写成：

```text
foo ──拥有──▶ Foo(0)
bar ──引用──▶ 同一个 Foo(0)
```

因此，`Foo` 和 `&Foo` 的区别在这里具体表现为：前者是那个值的类型，后者是访问那个值的引用类型。`bar` 并没有得到另一个 `Foo` 值，也没有接走 `foo` 的所有权；它能通过引用读取这个值，但不能通过这个不可变引用修改它。[[source:source_ref_9af277d1d3d785d4]]

原文随后分别打印 `foo` 和 `bar`，因为 `foo` 仍然拥有这个值，而 `bar` 可以借用访问它。这两次打印体现了两条访问路径，不表示有两个各自拥有的 `Foo`。原文给出的整个程序可以编译。[[source:source_ref_e2f1f69dfb7b32e3]][[source:source_ref_1a3ec5f7971d4632]]

现在只改第二行，其他行保持原样：

```rust
let foo = Foo(0);
let bar = foo;
println!("Foo is {:?}", foo);
println!("Bar is {:?}", bar);
```

请沿着“右侧产生什么 → 谁拥有值 → 哪个绑定还能用”判断一次：**此时 `bar` 的类型是什么，`Foo` 的所有权归谁，两行打印中哪一行会不能编译？** 用你已经知道的非 Copy 移动规则说明原因。
