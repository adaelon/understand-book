//! The actual limiter plus HTTP adapter, with one shared credential and retries.
use super::*;
use runtime::{ProviderConfig, ProviderMode, ProviderRegistry};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Instant,
};

#[test]
fn provider_capacity_stages_keep_user_isolation_limits_and_release_retry_slots() {
    let mut reports = Vec::new();
    for model_slots in [2, 8, 16] {
        let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let url = format!("http://{}", server.server_addr());
        let stopping = Arc::new(AtomicBool::new(false));
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let sends = Arc::new(Mutex::new(BTreeMap::<String, usize>::new()));
        let active_users = Arc::new(Mutex::new(BTreeMap::<String, usize>::new()));
        let resources = Resources::new(Arc::new(ServiceLimits {
            model_slots,
            ..Default::default()
        }));
        let started = Instant::now();
        let barrier = std::sync::Barrier::new(32);
        std::thread::scope(|scope| {
            let server = &server;
            let stopping = &stopping;
            let active = &active;
            let peak = &peak;
            let sends = &sends;
            let active_users = &active_users;
            let fixture = scope.spawn(move || {
                std::thread::scope(|handlers| {
                    while !stopping.load(Ordering::Acquire) {
                        let Some(mut request) = server.recv_timeout(Duration::from_millis(20)).unwrap() else { continue; };
                        handlers.spawn(move || {
                            assert!(request.headers().iter().any(|h| h.field.equiv("Authorization") && h.value.as_str() == "Bearer shared-fixture-key"));
                            let body: Value = serde_json::from_reader(request.as_reader()).unwrap();
                            let owner = body["messages"].as_array().unwrap().last().unwrap()["content"].as_str().unwrap().to_string();
                            {
                                let mut users = active_users.lock().unwrap();
                                let count = users.entry(owner.clone()).or_default();
                                *count += 1;
                                assert_eq!(*count, 1, "one user's requests overlapped");
                            }
                            let attempt = {
                                let mut counts = sends.lock().unwrap();
                                let count = counts.entry(owner.clone()).or_default();
                                *count += 1;
                                *count
                            };
                            peak.fetch_max(active.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
                            std::thread::sleep(Duration::from_millis(60));
                            // Every user's first request encounters the shared provider limit.
                            let (status, body) = if attempt == 1 {
                                (429, json!({"error":{"code":"rate_limit_exceeded"}}))
                            } else {
                                (200, json!({"choices":[{"message":{"content":json!({"owner":owner}).to_string()},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}))
                            };
                            active.fetch_sub(1, Ordering::SeqCst);
                            *active_users.lock().unwrap().get_mut(&owner).unwrap() -= 1;
                            request.respond(tiny_http::Response::from_string(body.to_string()).with_status_code(status)
                                .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())).unwrap();
                        });
                    }
                });
            });
            let jobs: Vec<_> = (0..32)
                .map(|i| {
                    let url = &url;
                    let resources = &resources;
                    let barrier = &barrier;
                    scope.spawn(move || {
                        let owner = format!("reader-{}", i % 16);
                        let provider = ProviderRegistry::adapter_from_config_with_timeout(
                            ProviderConfig {
                                agent_reasoning_effort: Default::default(),
                                mode: ProviderMode::Native,
                                api_key: "shared-fixture-key".into(),
                                base_url: url.clone(),
                                model: "fixture-model".into(),
                            },
                            Duration::from_secs(20),
                        );
                        let adapter = LimitedAdapter {
                            owner: owner.clone(),
                            background: false,
                            inner: provider.as_ref(),
                            resources: resources.clone(),
                            cancellation: Default::default(),
                            usage: Mutex::new(Default::default()),
                            authorization: None,
                            stream: None,
                        };
                        barrier.wait();
                        let response = adapter
                            .complete_structured(CompletionRequest {
                                system: "Return the owner as JSON.".into(),
                                user: owner.clone(),
                                images: vec![],
                                followup_user: None,
                                output_token_limit: Some(100),
                                reasoning_effort: None,
                            })
                            .unwrap();
                        assert_eq!(
                            response["owner"], owner,
                            "shared credential mixed response contexts"
                        );
                    })
                })
                .collect();
            let results: Vec<_> = jobs.into_iter().map(|job| job.join()).collect();
            stopping.store(true, Ordering::Release);
            fixture.join().unwrap();
            for result in results {
                result.unwrap();
            }
        });
        assert!(peak.load(Ordering::SeqCst) <= model_slots);
        assert!(
            peak.load(Ordering::SeqCst) > 1,
            "requests unexpectedly serialized"
        );
        assert_eq!(sends.lock().unwrap().len(), 16);
        assert!(sends.lock().unwrap().values().all(|count| *count == 3));
        assert_eq!(
            resources.measured_usage(),
            (0, 0, 0),
            "retry left a capacity permit or waiter behind"
        );
        reports.push(json!({"model_slots":model_slots,"users":16,"logical_calls":32,"physical_requests":48,
            "injected_429":16,"successful_calls":32,"peak_http_requests":peak.load(Ordering::SeqCst),
            "elapsed_ms":started.elapsed().as_millis(),"idle_resources":resources.measured_usage()}));
    }
    println!("PROVIDER_CAPACITY {}", json!(reports));
    if let Some(path) = std::env::var_os("UB_PROVIDER_CAPACITY_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&reports).unwrap()).unwrap();
    }
}
