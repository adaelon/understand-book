use super::mu5_tests::Fixture;
use super::*;
use crate::multi_user_host::{self, Site};

fn paper(f: &Fixture) -> crate::published_library::PublishedBookRef {
    let source = tempfile::tempdir().unwrap();
    let mut state = state_named("pdf-translation-service");
    state.workspace.book_dir = source.path().into();
    attach_paper_profile(&mut state);
    write_note_pdf_route_artifacts(&mut state);
    let path = source.path().join("source_manifest.json");
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    manifest["original_pdf"]["sha256"] = json!(sha256_hex(
        &std::fs::read(source.path().join("paper.pdf")).unwrap()
    ));
    std::fs::write(path, manifest.to_string()).unwrap();
    std::fs::write(
        source.path().join("alignment_report.json"),
        r#"{"config_hash":"cfg-a"}"#,
    )
    .unwrap();
    std::fs::create_dir_all(source.path().join(".build/source-reconciliation")).unwrap();
    std::fs::write(
        source
            .path()
            .join(".build/source-reconciliation/report.json"),
        json!({"book_id":state.workspace.book.base.book_id,"unresolved":[]}).to_string(),
    )
    .unwrap();
    let reference = f
        .access
        .library
        .lock()
        .unwrap()
        .publish(source.path())
        .unwrap()
        .reference;
    f.access
        .library
        .lock()
        .unwrap()
        .grant("A", &reference)
        .unwrap();
    reference
}

fn post(base: &str, token: &str, path: &str, input: Value) -> (u16, Value) {
    let r = ureq::post(&format!("{base}{path}"))
        .timeout(Duration::from_secs(15))
        .set("Host", "reader.example")
        .set("Origin", "https://reader.example")
        .set("Cookie", &format!("{}={token}", crate::auth::COOKIE))
        .set("X-CSRF-Token", &crate::auth::csrf(token))
        .send_json(input);
    let r = match r {
        Ok(r) | Err(ureq::Error::Status(_, r)) => r,
        Err(e) => panic!("{e}"),
    };
    (r.status(), r.into_json().unwrap())
}

#[test]
fn pdf_translation_real_http_charges_tasks_without_blocking_reading_or_writing_history() {
    for allowance in [1_000_000, 1] {
        let fake = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let provider_url = format!("http://{}", fake.server_addr().to_ip().unwrap());
        let mut f = Fixture::new();
        super::adm5_tests::seed(&mut f, &provider_url, allowance);
        f.access
            .runs
            .configure(
                ProviderConfig::from_values("native", "fixture-key", provider_url, "fixture-model")
                    .unwrap(),
            )
            .unwrap();
        let reference = paper(&f);
        let w = f.create(&f.a, &reference, "pdf-page");
        let mut input = Fixture::stamp(&w, "pdf-page");
        input["status"] = json!("resolved");
        input["raw_quote"] = json!("PDF");
        input["resolved_quote"] = json!("PDF");
        input["ranges"] = json!([{"lid":"1.1","range":{"start":0,"end":3}}]);
        let path = format!(
            "/api/workspaces/{}/reader/selection.translate",
            w["workspace_id"].as_str().unwrap()
        );
        let history_before = f
            .access
            .users
            .lock()
            .unwrap()
            .get("A", "now")
            .unwrap()
            .lock()
            .unwrap()
            .agent_history
            .sessions
            .len();
        let server = multi_user_host::start_with_access(
            "127.0.0.1:0".parse().unwrap(),
            Arc::new(Site::new("https://reader.example").unwrap()),
            f.access.clone(),
        )
        .unwrap();
        assert_eq!(post(&server.url, &f.b, &path, input.clone()).0, 404);
        let mut stale = input.clone();
        stale["generation"] = json!(999);
        assert_eq!(post(&server.url, &f.a, &path, stale).0, 409);
        let mut wrong = input.clone();
        wrong["resolved_quote"] = json!("not the source");
        assert_eq!(post(&server.url, &f.a, &path, wrong).0, 400);
        if allowance == 1 {
            let (status, reply) = post(&server.url, &f.a, &path, input);
            assert_eq!(status, 503, "{reply}");
            assert_eq!(reply["error_code"], "ALLOWANCE_INSUFFICIENT");
            assert!(fake.try_recv().unwrap().is_none());
        } else {
            let calls: Vec<_> = (0..4)
                .map(|_| {
                    let (base, token, path, input) =
                        (server.url.clone(), f.a.clone(), path.clone(), input.clone());
                    std::thread::spawn(move || post(&base, &token, &path, input))
                })
                .collect();
            let first = fake.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while f.access.resources.measured_usage().2 != 3 && std::time::Instant::now() < deadline
            {
                std::thread::sleep(Duration::from_millis(5));
            }
            assert_eq!(f.access.resources.measured_usage(), (1, 5, 3));
            // All four translation requests are pending, but normal HTTP workers are free.
            let read_path = format!(
                "/api/workspaces/{}/reader/state",
                w["workspace_id"].as_str().unwrap()
            );
            assert_eq!(
                post(
                    &server.url,
                    &f.a,
                    &read_path,
                    Fixture::stamp(&w, "pdf-page")
                )
                .0,
                200
            );
            let mut checkpoint = Fixture::stamp(&w, "pdf-page");
            checkpoint["top_lid"] = json!("1.1");
            let write_path = format!(
                "/api/workspaces/{}/checkpoint",
                w["workspace_id"].as_str().unwrap()
            );
            assert_eq!(post(&server.url, &f.a, &write_path, checkpoint).0, 200);
            respond_translation(first);
            for _ in 0..3 {
                respond_translation(fake.recv_timeout(Duration::from_secs(10)).unwrap().unwrap());
            }
            for call in calls {
                let (status, reply) = call.join().unwrap();
                assert_eq!(status, 200, "{reply}");
                assert_eq!(reply["result"]["translation_markdown"], "论文译文");
            }
            let counts: (i64, i64) = f.control.connection.query_row(
                "SELECT count(*),sum(account_debit_micro_cny) FROM model_call_charges WHERE user_id='A' AND task_ref IS NOT NULL AND run_ref IS NULL AND purpose='selection_translation' AND state='settled'", [], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
            assert_eq!(counts, (4, 120));
        }
        assert_eq!(
            f.access
                .users
                .lock()
                .unwrap()
                .get("A", "now")
                .unwrap()
                .lock()
                .unwrap()
                .agent_history
                .sessions
                .len(),
            history_before
        );
        server.shutdown();
    }
}

fn respond_translation(request: tiny_http::Request) {
    request.respond(tiny_http::Response::from_string(json!({
        "choices":[{"message":{"content":"{\"translation_markdown\":\"论文译文\"}"},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":20,"prompt_cache_hit_tokens":0,"completion_tokens":10,"total_tokens":30}
    }).to_string()).with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())).unwrap();
}
