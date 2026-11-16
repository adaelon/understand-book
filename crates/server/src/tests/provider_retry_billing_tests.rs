use super::*;

#[test]
fn provider_retry_real_billing_keeps_unknown_attempt_and_settles_success_once() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}", server.server_addr());
    let mut f = Fixture::new();
    seed(&mut f, &url, 1_000_000);
    let worker = std::thread::spawn(move || {
        for status in [429, 200] {
            let mut request = server
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .expect("missing request");
            let _: Value = serde_json::from_reader(request.as_reader()).unwrap();
            let body = if status == 429 {
                json!({"error":{"code":"rate_limit_exceeded"}})
            } else {
                json!({"choices":[{"message":{"content":"{\"ok\":true}"},"finish_reason":"stop"}],
                    "usage":{"prompt_tokens":10,"prompt_cache_hit_tokens":0,"prompt_cache_miss_tokens":10,"completion_tokens":5,"total_tokens":15}})
            };
            request
                .respond(
                    tiny_http::Response::from_string(body.to_string())
                        .with_status_code(status)
                        .with_header(
                            tiny_http::Header::from_bytes("Content-Type", "application/json")
                                .unwrap(),
                        ),
                )
                .unwrap();
        }
    });
    let adapter = ProviderRegistry::adapter_from_config_with_timeout(
        ProviderConfig::from_values("native", "fixture-key", &url, "fixture-model").unwrap(),
        Duration::from_secs(10),
    );
    adapter.set_spend_context(scope(), Some(port(&f)));
    assert_eq!(
        adapter
            .complete_structured(CompletionRequest {
                system: "Return JSON.".into(),
                user: "fixture".into(),
                images: vec![],
                followup_user: None,
                output_token_limit: Some(100),
                reasoning_effort: None,
            })
            .unwrap(),
        json!({"ok":true})
    );
    worker.join().unwrap();
    let mut query = f.control.connection.prepare("SELECT call_id,logical_call_id,attempt,state,provider_cost_micro_cny,account_debit_micro_cny FROM model_call_charges ORDER BY attempt").unwrap();
    let rows: Vec<_> = query
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, u32>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, Option<i64>>(5)?,
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].0, rows[1].0);
    assert_eq!(rows[0].1, rows[1].1);
    assert_eq!((rows[0].2, rows[1].2), (1, 2));
    assert_eq!(rows[0].3, "pending");
    assert_eq!((rows[0].4, rows[0].5), (None, None));
    assert_eq!(rows[1].3, "settled");
    assert_eq!((rows[1].4, rows[1].5), (Some(15), Some(15)));
    let balance = balance(&f);
    assert_eq!(balance["debited_micro_cny"], 15);
    assert_eq!(balance["active_reserved_micro_cny"], 0);
    assert!(balance["pending_micro_cny"].as_i64().unwrap() > 0);
}
