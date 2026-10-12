先只连接两件事：**这一行之后，变量还能不能读取值 → 下一次打印能不能使用它。**

原文的 `let bar = &foo;` 让 `bar` 借用值，`foo` 仍拥有值，所以 `foo` 仍可读取；`bar` 也能通过引用读取同一个值。[[source:source_ref_b7cdf804a5f27a0d]] [[source:source_ref_9af277d1d3d785d4]]

下面是补充示范，用另一个类型只看一个打印：

```rust
#[derive(Debug)]
struct Parcel(u32); // 没有 Copy 能力

let a = Parcel(0);
let b = a;
println!("{:?}", a);
```

我这样逐行判断：

1. `let a = Parcel(0);` 后，`a` 拥有值，可以读取它。
2. `let b = a;` 的右边没有 `&`，而 `Parcel` 没有 `Copy` 能力，所以值的所有权交给 `b`。现在 `b` 可读取这个值，`a` 已不能读取它。
3. 打印写的是 `a`。**打印需要先能读取所写变量的值**，但第二步已经让 `a` 不能读取，所以这条打印不能编译。若这里打印的是 `b`，则可以。

这里用到的补充规则是：`println!` 的这种格式化打印会借用内容，不会拿走所有权；但借用也要求变量此时仍可读取，不能把已经交走的值重新变回来。

现在只留一个判断给你。原文的 `Foo` 只声明了 `Debug`，没有 `Copy` 能力。[[source:source_ref_4c3de5a75d5bc2f7]] 把第二行改成没有 `&` 的写法，暂时只看第一条打印：

```rust
let foo = Foo(0);
let bar = foo;
println!("Foo is {:?}", foo);
```

**这条 `println!` 还能使用 `foo` 吗？**你可以只填一句：‘第二行之后，`foo` ______读取那个值，所以这条打印______。’
