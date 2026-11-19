use super::*;


#[test]
fn focus_visible_position_network_checkpoint_round_trip() {
    let f = mu5_tests::Fixture::new();
    let w = f.create(&f.a, &f.x, "fp5");
    let position = json!({"lid":"1.29","ranges":[{"lid":"1.29","start":2,"end":6}]});
    let moved = f.action(&f.a, &w, "fp5", "reader/position", position.clone());
    assert_eq!(moved["reader"]["visible_position"], position);
    assert_eq!(moved["reader"]["viewport"]["visible_lids"], json!(["1.29"]));
    assert_eq!(moved["reader"]["layout"], w["reader"]["layout"]);
    let restored = f.get(&f.a, &moved);
    assert_eq!(restored["reader"]["visible_position"], position);
}
