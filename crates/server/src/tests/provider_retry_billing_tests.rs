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

fn multi_protocol_fixture(contract: &str, cache_write_rate: Option<u64>) -> Fixture {
    let mut f = Fixture::new();
    seed(&mut f, "fixture", 1_000_000);
    let mut configured = rates("fixture");
    configured["rates"][0]["usage_contract"] = json!(contract);
    configured["rates"][0]["image_meter"] = json!("unsupported");
    if contract == "input_output" {
        configured["rates"][0]["cached_input_micro_cny_per_million"] = json!(1_000_000);
    }
    if let Some(rate) = cache_write_rate {
        configured["rates"][0]["cache_creation_input_micro_cny_per_million"] = json!(rate);
    }
    std::fs::write(
        f.root.path().join("model-rates.json"),
        configured.to_string(),
    )
    .unwrap();
    Arc::get_mut(&mut f.access).unwrap().spend = Arc::new(ModelSpendStore::new(
        ControlStore::open(f.control.writer.clone()).unwrap(),
    ));
    f
}

#[test]
fn provider_multi_protocol_real_billing_accepts_wire_budgets_and_settles_without_cache_metadata() {
    let f = multi_protocol_fixture("input_output", None);
    let spend = port(&f);
    let requests = [
        (
            "chat",
            json!({"model":"fixture-model","max_tokens":80,"messages":[{"role":"user","content":"hello"}]}),
        ),
        (
            "responses",
            json!({"model":"fixture-model","max_output_tokens":80,"input":[{"role":"user","content":[{"type":"input_text","text":"hello"}]}]}),
        ),
        (
            "generate",
            json!({"generationConfig":{"maxOutputTokens":80},"contents":[{"role":"user","parts":[{"text":"hello"}]}]}),
        ),
        (
            "interactions",
            json!({"model":"fixture-model","generation_config":{"max_output_tokens":80},"input":[{"role":"user","content":[{"type":"text","text":"hello"}]}]}),
        ),
        (
            "ollama",
            json!({"model":"fixture-model","options":{"num_predict":80},"messages":[{"role":"user","content":"hello"}]}),
        ),
        (
            "bedrock",
            json!({"inferenceConfig":{"maxTokens":80},"messages":[{"role":"user","content":[{"text":"hello"}]}]}),
        ),
    ];
    for (name, request) in requests {
        let identity = id(&format!("protocol-{name}"));
        spend.before_send(&identity, &request).unwrap();
        let receipt = SendOutcome {
            evidence: SendEvidence::Response,
            succeeded: true,
            usage: Some(ModelUsage {
                input_tokens: Some(10),
                output_tokens: Some(5),
                reasoning_output_tokens: Some(2),
                total_tokens: Some(15),
                ..Default::default()
            }),
        };
        spend.after_send(&identity, &receipt).unwrap();
        spend.after_send(&identity, &receipt).unwrap();
        let charge = charge(&f, &identity.call_id);
        assert_eq!(charge["state"], "settled", "{name}: {charge}");
        assert_eq!(charge["provider_cost_micro_cny"], 15);
        assert_eq!(charge["account_debit_micro_cny"], 15);
        assert_eq!(charge["reports"]["total"], 1);
    }
    assert_eq!(balance(&f)["debited_micro_cny"], 90);
    assert_eq!(balance(&f)["active_reserved_micro_cny"], 0);
    assert_eq!(balance(&f)["pending_micro_cny"], 0);
}

#[test]
fn provider_multi_protocol_real_billing_keeps_missing_units_pending_and_prices_cache_writes_once() {
    for (contract, cache_rate, complete, missing, expected) in [
        (
            "input_output",
            None,
            ModelUsage {
                input_tokens: Some(10),
                output_tokens: Some(5),
                total_tokens: Some(15),
                ..Default::default()
            },
            ModelUsage {
                input_tokens: Some(10),
                ..Default::default()
            },
            15,
        ),
        (
            "cached_input_output",
            Some(2_000_000),
            ModelUsage {
                input_tokens: Some(10),
                cached_input_tokens: Some(4),
                cache_creation_input_tokens: Some(2),
                output_tokens: Some(5),
                total_tokens: Some(15),
                ..Default::default()
            },
            ModelUsage {
                input_tokens: Some(10),
                cached_input_tokens: Some(4),
                output_tokens: Some(5),
                ..Default::default()
            },
            14,
        ),
        (
            "cohere_billed_units",
            None,
            ModelUsage {
                input_tokens: Some(100),
                output_tokens: Some(20),
                total_tokens: Some(120),
                billed_input_tokens: Some(7),
                billed_output_tokens: Some(3),
                ..Default::default()
            },
            ModelUsage {
                input_tokens: Some(100),
                output_tokens: Some(20),
                total_tokens: Some(120),
                ..Default::default()
            },
            10,
        ),
    ] {
        let f = multi_protocol_fixture(contract, cache_rate);
        let spend = port(&f);
        let identity = id("complete-units");
        spend.before_send(&identity, &request(80)).unwrap();
        spend
            .after_send(
                &identity,
                &SendOutcome {
                    evidence: SendEvidence::Response,
                    succeeded: true,
                    usage: Some(complete),
                },
            )
            .unwrap();
        assert_eq!(
            charge(&f, &identity.call_id)["provider_cost_micro_cny"],
            expected,
            "{contract}"
        );
        let pending = id("missing-units");
        spend.before_send(&pending, &request(80)).unwrap();
        spend
            .after_send(
                &pending,
                &SendOutcome {
                    evidence: SendEvidence::Response,
                    succeeded: true,
                    usage: Some(missing),
                },
            )
            .unwrap();
        let unknown = charge(&f, &pending.call_id);
        assert_eq!(unknown["state"], "pending", "{contract}");
        assert!(unknown["provider_cost_micro_cny"].is_null());
        assert!(unknown["account_debit_micro_cny"].is_null());
        assert_eq!(balance(&f)["debited_micro_cny"], expected);
    }
}
