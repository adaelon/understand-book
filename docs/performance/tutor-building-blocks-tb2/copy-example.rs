// Condition counterexample: being a struct does not by itself imply a move.
#[derive(Debug, Copy, Clone)]
struct Foo(u32);

fn main() {
    let foo = Foo(0);
    let bar = foo;
    println!("Foo is {:?}", foo);
    println!("Bar is {:?}", bar);
}
