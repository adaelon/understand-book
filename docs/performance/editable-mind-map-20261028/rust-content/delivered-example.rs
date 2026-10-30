fn main() {
    let mut v = vec![10, 20, 30, 40];
    let s: &mut [i32] = &mut v[1..3]; // 框住 v[1]、v[2]
    s[0] = 99;                        // 偏移 0 → v[1]
    println!("{:?}", v);              // [10, 99, 30, 40]
}
