use super::*;
use std::{
    sync::{mpsc, Barrier},
    thread,
};

#[derive(Default)]
struct TestMeter {
    admissions: Mutex<Vec<(WebServiceKind, String, u32)>>,
    receipts: Mutex<Vec<(String, WebChargeOutcome, Value)>>,
}
impl Meter for TestMeter {
    fn admit(
        &self,
        kind: WebServiceKind,
        logical: &str,
        attempt: u32,
    ) -> Result<String, ToolError> {
        let mut admissions = self.admissions.lock().unwrap();
        admissions.push((kind, logical.into(), attempt));
        Ok(format!("test_usage_{}", admissions.len()))
    }
    fn receipt(
        &self,
        id: &str,
        outcome: WebChargeOutcome,
        evidence: Value,
    ) -> Result<Value, ToolError> {
        self.receipts
            .lock()
            .unwrap()
            .push((id.into(), outcome, evidence));
        let cost = match outcome {
            WebChargeOutcome::Billed => Some(5000),
            WebChargeOutcome::Free => Some(0),
            WebChargeOutcome::Unknown => None,
        };
        Ok(
            json!({"charge":{"state":match outcome {WebChargeOutcome::Billed=>"settled",WebChargeOutcome::Free=>"released",WebChargeOutcome::Unknown=>"pending"},"provider_cost_status":if cost.is_some(){"estimated"}else{"pending"},"provider_cost_micro_cny":cost,"account_debit_micro_cny":cost}}),
        )
    }
}
fn request(name: &str, args: Value) -> Request {
    Request::parse(name, args).unwrap()
}
fn mock(
    responses: Vec<(u16, String, Duration)>,
) -> (Client, mpsc::Receiver<Value>, thread::JoinHandle<()>) {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let base = format!("http://{}", server.server_addr());
    let (tx, rx) = mpsc::channel();
    let task = thread::spawn(move || {
        for (status, body, delay) in responses {
            let mut req = server
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .expect("expected upstream request");
            let mut input = String::new();
            req.as_reader().read_to_string(&mut input).unwrap();
            tx.send(
                json!({"path":req.url(),"body":serde_json::from_str::<Value>(&input).unwrap()}),
            )
            .unwrap();
            thread::sleep(delay);
            req.respond(
                tiny_http::Response::from_string(body)
                    .with_status_code(status)
                    .with_header(
                        tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                    ),
            )
            .unwrap();
        }
    });
    (
        Client::new(Config {
            base,
            key: "test-key".into(),
            search_qps: 100,
            fetch_qps: 25,
        }),
        rx,
        task,
    )
}
fn ok(value: Value) -> (u16, String, Duration) {
    (200, value.to_string(), Duration::ZERO)
}
fn search_result(content: &str) -> Value {
    json!({"results":[{"url":"https://docs.example.test/cache","title":"Cache conditions","summary":"Candidates need source checks","content":content,"authority":4,"rerank_score":1.9}]})
}

#[test]
fn fetch_pagination_reuses_full_unicode_body_and_refresh_is_explicit() {
    let (client, rx, server) = mock(vec![
        ok(json!({"title":"Page","content":"甲乙🙂abcxyz"})),
        ok(json!({"title":"Page","content":"new text"})),
    ]);
    let meter = TestMeter::default();
    let cancel = CancellationToken::default();
    let url = "https://docs.example.test/cache";
    let first = client
        .call(
            request("web.fetch", json!({"url":url,"max_chars":4})),
            &meter,
            &cancel,
        )
        .unwrap();
    assert_eq!(first["content"], "甲乙🙂a");
    assert_eq!(first["next_offset"], 4);
    let next = client
        .call(
            request("web.fetch", json!({"url":url,"offset":4,"max_chars":5})),
            &meter,
            &cancel,
        )
        .unwrap();
    assert_eq!(next["content"], "bcxyz");
    assert!(next["next_offset"].is_null());
    assert_eq!(next["usage"]["physical_requests"], 0);
    assert_eq!(next["usage"]["cache"], "hit");
    assert_eq!(meter.admissions.lock().unwrap().len(), 1);
    let refreshed = client
        .call(
            request("web.fetch", json!({"url":url,"refresh":true})),
            &meter,
            &cancel,
        )
        .unwrap();
    assert_eq!(refreshed["content"], "new text");
    assert_eq!(meter.admissions.lock().unwrap().len(), 2);
    for sent in rx.try_iter() {
        assert_eq!(sent["path"], "/v1/tools/fetch");
        assert_eq!(sent["body"], json!({"url":url}));
    }
    server.join().unwrap();
}

#[test]
fn search_cache_uses_complete_parameters_and_full_content_satisfies_fetch() {
    let (client, rx, server) = mock(vec![
        ok(search_result(&"甲".repeat(9000))),
        ok(search_result("changed")),
    ]);
    let meter = TestMeter::default();
    let cancel = CancellationToken::default();
    let args = json!({"query":"prefix caching","ori_question":"What keeps a prefix stable?","localization":"8.8.8.8","need_content":true});
    let first = client
        .call(request("web.search", args.clone()), &meter, &cancel)
        .unwrap();
    assert_eq!(
        first["results"][0]["content"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        2000
    );
    assert_eq!(first["results"][0]["next_offset"], 2000);
    let repeated = client
        .call(request("web.search", args.clone()), &meter, &cancel)
        .unwrap();
    assert_eq!(repeated["usage"]["cache"], "hit");
    let page = client
        .call(
            request(
                "web.fetch",
                json!({"url":"https://docs.example.test/cache","offset":2000,"max_chars":7000}),
            ),
            &meter,
            &cancel,
        )
        .unwrap();
    assert_eq!(page["content"].as_str().unwrap().chars().count(), 7000);
    assert_eq!(page["usage"]["physical_requests"], 0);
    let mut different = args;
    different["ori_question"] = json!("What invalidates a cached prefix?");
    client
        .call(request("web.search", different), &meter, &cancel)
        .unwrap();
    let sent: Vec<_> = rx.try_iter().collect();
    assert_eq!(sent.len(), 2);
    assert_eq!(
        sent[0]["body"],
        json!({"query":"prefix caching","ori_question":"What keeps a prefix stable?","localization":"8.8.8.8","need_content":true,"count":3})
    );
    assert_eq!(meter.admissions.lock().unwrap().len(), 2);
    server.join().unwrap();
}

#[test]
fn retry_records_each_attempt_and_known_failure_is_free() {
    let (client, rx, server) = mock(vec![
        (429, "{}".into(), Duration::ZERO),
        ok(search_result("")),
    ]);
    let meter = TestMeter::default();
    let result = client
        .call(
            request("web.search", json!({"query":"cache"})),
            &meter,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(result["usage"]["physical_requests"], 2);
    let admissions = meter.admissions.lock().unwrap();
    assert_eq!(admissions[0].1, admissions[1].1);
    assert_eq!((admissions[0].2, admissions[1].2), (1, 2));
    let receipts = meter.receipts.lock().unwrap();
    assert_eq!(receipts[0].1, WebChargeOutcome::Free);
    assert_eq!(receipts[1].1, WebChargeOutcome::Billed);
    assert_eq!(rx.try_iter().count(), 2);
    server.join().unwrap();
}

#[test]
fn empty_results_are_free_and_malformed_success_stays_pending() {
    let (client, rx, server) = mock(vec![
        ok(json!({"results":[]})),
        (200, "{broken".into(), Duration::ZERO),
    ]);
    let meter = TestMeter::default();
    let cancel = CancellationToken::default();
    client
        .call(
            request("web.search", json!({"query":"empty"})),
            &meter,
            &cancel,
        )
        .unwrap();
    client
        .call(
            request("web.search", json!({"query":"empty"})),
            &meter,
            &cancel,
        )
        .unwrap();
    let error = client
        .call(
            request("web.search", json!({"query":"malformed"})),
            &meter,
            &cancel,
        )
        .unwrap_err();
    assert_eq!(error.error_code, "WEB_RETRIEVAL_FAILED");
    let repeated = client
        .call(
            request("web.search", json!({"query":"malformed"})),
            &meter,
            &cancel,
        )
        .unwrap_err();
    assert_eq!(repeated.error_code, "WEB_RETRIEVAL_FAILED");
    let receipts = meter.receipts.lock().unwrap();
    assert_eq!(receipts.len(), 2);
    assert_eq!(receipts[0].1, WebChargeOutcome::Free);
    assert_eq!(receipts[1].1, WebChargeOutcome::Unknown);
    assert_eq!(rx.try_iter().count(), 2);
    server.join().unwrap();
}

#[test]
fn a_billed_blocked_page_is_not_marked_as_readable_evidence() {
    let (client, rx, server) = mock(vec![ok(
        json!({"title":"Bilibili","content":"该内容被 AdGuard / AdBlock 类插件屏蔽。请检查插件以恢复正常内容展示。"}),
    )]);
    let meter = TestMeter::default();
    let result = client
        .call(
            request("web.fetch", json!({"url":"https://www.bilibili.com/"})),
            &meter,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(result["content_status"], "blocked");
    assert_eq!(
        meter.receipts.lock().unwrap()[0].1,
        WebChargeOutcome::Billed
    );
    assert_eq!(rx.try_iter().count(), 1);
    server.join().unwrap();
}

#[test]
fn cancellation_in_queue_sends_and_charges_nothing() {
    let client = Client::new(Config {
        base: "http://127.0.0.1:1".into(),
        key: "test".into(),
        search_qps: 1,
        fetch_qps: 1,
    });
    client.state.lock().unwrap().search_next = Some(Instant::now() + Duration::from_secs(2));
    let meter = TestMeter::default();
    let cancel = CancellationToken::default();
    thread::scope(|scope| {
        scope.spawn(|| {
            thread::sleep(Duration::from_millis(40));
            cancel.cancel();
        });
        let err = client
            .call(
                request("web.search", json!({"query":"queued"})),
                &meter,
                &cancel,
            )
            .unwrap_err();
        assert_eq!(err.error_code, "AGENT_RUN_CANCELLED");
    });
    assert!(meter.admissions.lock().unwrap().is_empty());
    assert!(meter.receipts.lock().unwrap().is_empty());
}

#[test]
fn simultaneous_identical_requests_share_one_physical_attempt() {
    let (client, rx, server) = mock(vec![(
        200,
        search_result("").to_string(),
        Duration::from_millis(100),
    )]);
    let meter = TestMeter::default();
    let barrier = Barrier::new(2);
    let results = thread::scope(|scope| {
        let call = || {
            barrier.wait();
            client
                .call(
                    request("web.search", json!({"query":"same query"})),
                    &meter,
                    &CancellationToken::default(),
                )
                .unwrap()
        };
        let first = scope.spawn(call);
        let second = scope.spawn(call);
        vec![first.join().unwrap(), second.join().unwrap()]
    });
    assert_eq!(
        results
            .iter()
            .map(|v| v["usage"]["physical_requests"].as_u64().unwrap())
            .sum::<u64>(),
        1
    );
    assert!(results.iter().any(|v| v["usage"]["cache"] == "coalesced"));
    assert_eq!(meter.admissions.lock().unwrap().len(), 1);
    assert_eq!(rx.try_iter().count(), 1);
    server.join().unwrap();
}

#[test]
fn cancellation_after_send_still_settles_the_supplier_charge() {
    let (client, rx, server) = mock(vec![(
        200,
        search_result("").to_string(),
        Duration::from_millis(100),
    )]);
    let meter = TestMeter::default();
    let cancel = CancellationToken::default();
    let signal = cancel.clone();
    thread::scope(|scope| {
        scope.spawn(move || {
            rx.recv().unwrap();
            signal.cancel();
        });
        let err = client
            .call(
                request("web.search", json!({"query":"sent"})),
                &meter,
                &cancel,
            )
            .unwrap_err();
        assert_eq!(err.error_code, "AGENT_RUN_CANCELLED");
    });
    assert_eq!(meter.admissions.lock().unwrap().len(), 1);
    assert_eq!(
        meter.receipts.lock().unwrap()[0].1,
        WebChargeOutcome::Billed
    );
    let reused = client
        .call(
            request("web.search", json!({"query":"sent"})),
            &meter,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(reused["usage"]["cache"], "hit");
    assert_eq!(reused["usage"]["physical_requests"], 0);
    assert_eq!(reused["results"][0]["title"], "Cache conditions");
    assert_eq!(meter.admissions.lock().unwrap().len(), 1);
    assert_eq!(meter.receipts.lock().unwrap().len(), 1);
    server.join().unwrap();
}

#[test]
fn expired_continuation_does_not_purchase_another_page() {
    let client = Client::new(Config {
        base: "http://127.0.0.1:1".into(),
        key: "test".into(),
        search_qps: 100,
        fetch_qps: 25,
    });
    let meter = TestMeter::default();
    let err = client
        .call(
            request(
                "web.fetch",
                json!({"url":"https://docs.example.test/cache","offset":2000}),
            ),
            &meter,
            &CancellationToken::default(),
        )
        .unwrap_err();
    assert_eq!(err.error_code, "WEB_PAGE_EXPIRED");
    assert!(meter.admissions.lock().unwrap().is_empty());
}
