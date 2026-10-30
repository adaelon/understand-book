fn main() {
    let v = vec![10, 20, 30, 40];
    let s: &[i32] = &v[1..3];
    assert_eq!(s, &[20, 30]);
    assert_eq!(s.len(), 2);
    println!("shared: {:?}; length={}", s, s.len());

    let mut v = vec![10, 20, 30, 40];
    let s: &mut [i32] = &mut v[1..3];
    s[0] = 99;
    assert_eq!(v, vec![10, 99, 30, 40]);
    println!("write_first: {:?}", v);

    let mut v = vec![10, 20, 30, 40];
    let s: &mut [i32] = &mut v[1..3];
    s[0] = 99;
    s[1] = 88;
    assert_eq!(v, vec![10, 99, 88, 40]);
    println!("write_both: {:?}", v);
}
