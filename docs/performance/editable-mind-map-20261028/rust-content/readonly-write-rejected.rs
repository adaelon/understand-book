// Expected compile failure: E0594 on the assignment.
fn main() {
    let v = vec![10, 20, 30, 40];
    let s: &[i32] = &v[1..3];
    s[0] = 99;
}
