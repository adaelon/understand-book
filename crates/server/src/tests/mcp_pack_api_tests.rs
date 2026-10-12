use super::mu5_tests::Fixture;
use serde_json::{json, Value};

#[test]
fn mcp_pack_admin_api_mixed_receipts_gifts_and_corrections() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    let post =
        |path: &str, body: Value| f.ok(&f.b, "POST", &format!("/api/admin/users/A/{path}"), body);
    let get = |path: &str| {
        f.ok(
            &f.b,
            "GET",
            &format!("/api/admin/users/A/{path}"),
            json!({}),
        )
    };
    let now = crate::multi_user_host::now();
    let period = post(
        "allowance-periods",
        json!({"operation_id":"period","starts_at":now-10,"expires_at":now+1000}),
    );
    let period = &period["allowance_period"]["period_id"];
    let model = post(
        "receipts",
        json!({"operation_id":"model","period_id":period,"revision":0,"amount_fen":123,"delta_micro_cny":1000,"paid_at":now,"channel":"wechat","external_ref":"model-tx","note":"model"}),
    );
    let body = json!({"operation_id":"buy","pack_count":2,"paid_at":now,"channel":"wechat","external_ref":"mcp-tx","note":"confirmed"});
    let bought = post("mcp/packs", body.clone());
    assert_eq!(post("mcp/packs", body.clone()), bought);
    assert_eq!(get("mcp")["paid_remaining"], 2000);
    assert_eq!(get("mcp")["free_remaining"], 1000);
    let mixed = get("receipts");
    assert_eq!(mixed["total"], 2);
    let rows = mixed["items"].as_array().unwrap();
    let mcp = rows
        .iter()
        .find(|r| r["receipt_id"] == bought["receipt_id"])
        .unwrap();
    assert_eq!(mcp["entitlement_kind"], "mcp_call_pack");
    assert_eq!(mcp["period_id"], Value::Null);
    assert_eq!(mcp["pack_id"], bought["call_pack"]["pack_id"]);
    let model_row = rows
        .iter()
        .find(|r| r["receipt_id"] == model["receipt_id"])
        .unwrap();
    assert_eq!(model_row["entitlement_kind"], "model_allowance");
    assert_eq!(&model_row["period_id"], period);
    let corrected = post(
        "mcp/corrections",
        json!({"operation_id":"mcp-fix","receipt_id":bought["receipt_id"],"revision":0,"delta_fen":-1000,"delta_units":-1000,"reason":"one pack entered twice"}),
    );
    assert_eq!(corrected["call_pack"]["available_units"], 1000);
    post(
        "receipt-corrections",
        json!({"operation_id":"model-fix","receipt_id":model["receipt_id"],"period_id":period,"revision":1,"delta_fen":-23,"delta_micro_cny":-100,"reason":"correct model grant"}),
    );
    assert_eq!(get("receipt-corrections")["total"], 2);
    assert_eq!(
        f.ok(&f.b, "GET", "/api/admin/users/A", json!({}))["current_allowance"]["balance"]
            ["available_micro_cny"],
        900
    );
    let gift = json!({"operation_id":"gift","units":50,"reason":"reading feedback"});
    let gifted = post("mcp/gifts", gift.clone());
    assert_eq!(post("mcp/gifts", gift), gifted);
    assert_eq!(get("mcp")["free_remaining"], 1050);
    assert_eq!(get("receipts")["total"], 2);
    assert_eq!(
        f.ok(&f.b, "GET", "/api/admin/operations/buy", json!({})),
        bought
    );
    // Cash transaction references are shared across both receipt purposes.
    let mut duplicate = body.clone();
    duplicate["operation_id"] = json!("dup");
    duplicate["external_ref"] = json!("model-tx");
    assert_eq!(
        f.call(&f.b, "POST", "/api/admin/users/A/mcp/packs", duplicate)
            .1["error_code"],
        "RECEIPT_EXTERNAL_REF_CONFLICT"
    );
    let wrong_kind = json!({"operation_id":"wrong","receipt_id":model["receipt_id"],"revision":0,"delta_fen":-1,"delta_units":0,"reason":"wrong"});
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            "/api/admin/users/A/mcp/corrections",
            wrong_kind
        )
        .0,
        404
    );
    let wrong_kind = json!({"operation_id":"wrong-model","receipt_id":bought["receipt_id"],"period_id":period,"revision":2,"delta_fen":-1,"delta_micro_cny":0,"reason":"wrong"});
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            "/api/admin/users/A/receipt-corrections",
            wrong_kind
        )
        .0,
        404
    );
    assert_eq!(get("allowance-periods")["items"][0]["revision"], 2);
    assert_eq!(f.call(&f.b,"POST","/api/admin/users/B/mcp/corrections",json!({"operation_id":"other","receipt_id":bought["receipt_id"],"revision":1,"delta_fen":-1,"delta_units":0,"reason":"other"})).0,404);
    for (path, body) in [
        ("mcp/packs", body),
        (
            "mcp/gifts",
            json!({"operation_id":"forbidden-gift","units":1,"reason":"x"}),
        ),
        ("mcp/corrections", json!({"operation_id":"forbidden-fix"})),
    ] {
        assert_eq!(
            f.call(&f.a, "POST", &format!("/api/admin/users/A/{path}"), body)
                .0,
            403
        );
    }
    assert_eq!(
        f.call(&f.a, "GET", "/api/admin/users/A/mcp", json!({})).0,
        403
    );
    assert_eq!(
        f.call(&f.b, "GET", "/api/admin/users/A/mcp?user_id=B", json!({}))
            .0,
        400
    );
    assert_eq!(f.call(&f.b,"POST","/api/admin/users/A/mcp/packs",json!({"operation_id":"bad","pack_count":1,"paid_at":now,"channel":"wechat","external_ref":null,"note":"x","amount_fen":1})).0,400);
    assert!(!f.root.path().join("users/A").exists());
}
