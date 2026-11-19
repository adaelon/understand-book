use super::mu5_tests::Fixture;
use runtime::{model_settings::ModelSettings, ProviderConfig};
use serde_json::{json, Value};

#[test]
fn model_settings_admin_api_enforces_permissions_and_redacts_credentials() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    let input = serde_json::to_value(ModelSettings::from_legacy(
        &ProviderConfig::from_values(
            "native",
            "private-settings-secret",
            "https://models.example/v1",
            "fixture-model",
        )
        .unwrap(),
    ))
    .unwrap();
    assert_eq!(
        f.call(&f.a, "GET", "/api/admin/model-settings", Value::Null)
            .0,
        403
    );
    assert_eq!(
        f.call(&f.a, "POST", "/api/admin/model-settings", input.clone())
            .0,
        403
    );
    let saved = f.ok(&f.b, "POST", "/api/admin/model-settings", input);
    assert_eq!(saved["source"], "settings");
    assert_eq!(saved["settings"]["revision"], 1);
    assert_eq!(saved["settings"]["services"][0]["api_key_configured"], true);
    assert!(!saved.to_string().contains("private-settings-secret"));
    let loaded = f.ok(&f.b, "GET", "/api/admin/model-settings", Value::Null);
    assert_eq!(loaded, saved);
    let mut update = saved["settings"].clone();
    update["models"][0]["model_id"] = json!("second");
    let second = f.ok(&f.b, "POST", "/api/admin/model-settings", update.clone());
    assert_eq!(second["settings"]["revision"], 2);
    assert_ne!(
        f.call(&f.b, "POST", "/api/admin/model-settings", update).0,
        200
    );
    let persisted: Value =
        serde_json::from_slice(&std::fs::read(f.root.path().join("model-settings.json")).unwrap())
            .unwrap();
    assert_eq!(
        persisted["settings"]["services"][0]["api_key"],
        "private-settings-secret"
    );
    assert_eq!(persisted["history"].as_array().unwrap().len(), 1);
    assert_eq!(
        f.call(
            &f.a,
            "POST",
            "/api/admin/model-settings/test",
            json!({"model_config_id":"default-model"})
        )
        .0,
        403
    );
}

#[test]
fn model_settings_admin_draft_test_preserves_saved_catalog_and_uses_operator_ledger() {
    let fixture = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}", fixture.server_addr());
    let mut f = Fixture::new();
    super::adm5_tests::seed(&mut f, &url, 1_000_000);
    let input = serde_json::to_value(ModelSettings::from_legacy(
        &ProviderConfig::from_values(
            "native",
            "draft-retained-key",
            "https://saved.example/v1",
            "saved-model",
        )
        .unwrap(),
    ))
    .unwrap();
    let saved = f.ok(&f.b, "POST", "/api/admin/model-settings", input);
    let path = f.root.path().join("model-settings.json");
    let original = std::fs::read(&path).unwrap();
    let mut draft = saved["settings"].clone();
    draft["services"][0]["base_url"] = json!(url);
    draft["models"][0]["model_id"] = json!("fixture-model");
    let response = std::thread::spawn(move || {
        let mut request = fixture
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap()
            .expect("draft test request");
        assert_eq!(
            request
                .headers()
                .iter()
                .find(|header| header.field.equiv("Authorization"))
                .unwrap()
                .value
                .as_str(),
            "Bearer draft-retained-key"
        );
        let body: Value = serde_json::from_reader(request.as_reader()).unwrap();
        assert_eq!(body["model"], "fixture-model");
        request.respond(tiny_http::Response::from_string(json!({"choices":[{"message":{"content":"OK"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":10,"prompt_cache_hit_tokens":0,"prompt_cache_miss_tokens":10,"completion_tokens":1,"total_tokens":11}}).to_string())
            .with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
    });
    let result = f.ok(
        &f.b,
        "POST",
        "/api/admin/model-settings/test",
        json!({"model_config_id":"default-model","input":draft}),
    );
    response.join().unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(
        f.ok(&f.b, "GET", "/api/admin/model-settings", Value::Null),
        saved
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
    let (count,owner,debit):(i64,String,i64)=f.control.connection.query_row(
        "SELECT COUNT(*),cost_owner,account_debit_micro_cny FROM model_call_charges WHERE purpose='connection_test'",[],
        |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
    assert_eq!((count, owner.as_str(), debit), (1, "platform", 0));
}
