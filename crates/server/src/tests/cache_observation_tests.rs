use super::*;
use crate::observability::{
    config::ObservabilityConfig,
    langsmith::{LangSmithTransport, TransportResult},
    queue::{ExportItem, ExportOperation},
    ObservabilityRuntime,
};

struct Capture(Arc<Mutex<Vec<ExportItem>>>);
impl LangSmithTransport for Capture {
    fn send(&mut self, item: &ExportItem, _: Duration) -> TransportResult {
        self.0.lock().unwrap().push(item.clone());
        TransportResult::Confirmed
    }
}

// Enter through the real multi-user admission, storage, provider and completion
// path. Only the external model and LangSmith service are fixtures.
#[test]
fn formal_admission_exports_usage_diagnostics_and_terminal_states() {
    for (mode, failure) in [("metadata","none"),("metadata","provider"),("metadata","save"),
        ("full","none"),("full","provider"),("full","save")] {
        let mut f = Fixture::new();
        let sent = Arc::new(Mutex::new(Vec::new()));
        let config = ObservabilityConfig::from_getter(|key| match key {
            "UB_OBSERVABILITY_MODE" => Some(mode.into()),
            "LANGSMITH_API_KEY" => Some("fixture-key".into()),
            "LANGSMITH_PROJECT" => Some("formal-admission-fixture".into()),
            _ => None,
        }, false).unwrap().unwrap();
        let observation = ObservabilityRuntime::with_transport(config, Box::new(Capture(sent.clone())));
        Arc::get_mut(&mut f.access).unwrap().runs.observability = observation.clone();

        let provider = tiny_http::Server::http("127.0.0.1:0").unwrap();
        crate::tests::adm5_tests::seed_model(&mut f, &format!("http://{}", provider.server_addr()), "formal-admission-fixture", 1_000_000);
        f.access.runs.configure(ProviderConfig {
            mode: runtime::ProviderMode::Native,
            api_key: "PRIVATE_MODEL_KEY".into(),
            base_url: format!("http://{}", provider.server_addr()),
            model: "formal-admission-fixture".into(),
        }).unwrap();
        let wire = std::thread::spawn(move || {
            let mut bodies = Vec::new();
            for index in 0..2 {
                let mut request = provider.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
                let body: Value = serde_json::from_reader(request.as_reader()).unwrap();
                bodies.push(body);
                if failure == "provider" && index == 1 {
                    request.respond(tiny_http::Response::from_string("fixture failure").with_status_code(400)).unwrap();
                    break;
                }
                let message = if index == 0 {
                    json!({"role":"assistant","content":null,"tool_calls":[{
                        "id":"read-one","type":"function","function":{
                            "name":"book_structure","arguments":"{}"
                        }
                    }]})
                } else { json!({"role":"assistant","content":"你好！"}) };
                let body = json!({"choices":[{"message":message}],"usage":{
                    "prompt_tokens":1000,"completion_tokens":20,
                    "prompt_cache_hit_tokens":800,"total_tokens":1020
                }});
                request.respond(tiny_http::Response::from_string(body.to_string())
                    .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())).unwrap();
            }
            bodies
        });
        let w = f.create(&f.a, &f.x, "page");
        let w = f.action(&f.a, &w, "page", "chat/new", json!({}));
        let mut input = Fixture::stamp(&w, "page");
        input["session_id"] = w["selected_chat"].clone();
        input["client_request_id"] = json!("key");
        input["message"] = json!("你好 PRIVATE_QUESTION");
        assert_eq!(admit(&f, &w, &input).0, 202);
        if failure == "save" {
            let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
            let mut user = handle.lock().unwrap();
            let log = user.session_store.as_mut().unwrap().logs
                .get_mut(input["session_id"].as_str().unwrap()).unwrap();
            log.fail_write = Some(false);
            log.fail_terminal_only = true;
        }
        assert!(f.access.runs.run_one(&f.access).unwrap());
        let bodies = wire.join().unwrap();
        assert_eq!(bodies.len(), 2);
        observation.shutdown();
        let sent = sent.lock().unwrap();
        let roots = sent.iter().filter(|item| item.parent_run_id.is_none()).collect::<Vec<_>>();
        assert_eq!(roots.len(), 2, "{failure}: formal admission must create and finish a trace");
        assert_eq!(roots[0].operation, ExportOperation::Create);
        assert_eq!(roots[1].operation, ExportOperation::Update);
        let root = &roots[1].payload["extra"]["metadata"]["ub_observation"];
        assert_eq!(root["execution_state"], if failure == "provider" { "failed" } else { "completed" });
        assert_eq!(root["persistence_state"], if failure == "save" { "failed" } else { "saved" });
        if failure == "provider" { assert_eq!(root["metadata"]["error_code"], "PROVIDER_ERROR"); }
        assert!(sent.iter().any(|item| {
            let detail = &item.payload["extra"]["metadata"]["ub_observation"];
            detail["kind"] == "tool" && detail["execution_state"] == "completed"
                && detail["metadata"]["registered_tool_name"] == "book.structure"
                && detail["metadata"]["error_code"].is_null()
        }));
        let models = sent.iter().filter(|item| item.operation == ExportOperation::Update
            && item.payload["extra"]["metadata"]["ub_observation"]["kind"] == "model")
            .collect::<Vec<_>>();
        assert_eq!(models.len(), 2);
        for (index, item) in models.iter().enumerate() {
            let metadata = &item.payload["extra"]["metadata"];
            let detail = &metadata["ub_observation"];
            let diagnostics = &detail["metadata"]["request_diagnostics"];
            assert_eq!(diagnostics["request_index"], index + 1);
            assert_eq!(diagnostics["message_count"], bodies[index]["messages"].as_array().unwrap().len());
            if failure != "provider" || index == 0 {
                assert_eq!(detail["usage"]["cached_input_tokens"], 800);
                assert_eq!(metadata["usage_metadata"]["input_tokens"], 1000);
                assert_eq!(metadata["usage_metadata"]["input_token_details"]["cache_read"], 800);
            }
        }
        if mode == "metadata" {
            assert!(!format!("{sent:?}").contains("PRIVATE_"));
        } else {
            assert_eq!(roots[0].payload["inputs"]["message"], "你好 PRIVATE_QUESTION");
            if failure != "provider" {
                assert_eq!(roots[1].payload["outputs"]["answer"], "你好！");
            } else {
                assert_eq!(roots[1].payload["outputs"]["error_code"], "PROVIDER_ERROR");
            }
            for (index, item) in models.iter().enumerate() {
                assert_eq!(item.payload["inputs"], bodies[index], "export actual provider request");
                if failure != "provider" || index == 0 {
                    assert!(item.payload["outputs"]["response"]["choices"].is_array());
                    assert!(item.payload["outputs"]["parsed"].is_object());
                } else { assert!(item.payload["outputs"]["error"].is_string()); }
            }
            let tool = sent.iter().find(|item| item.payload["outputs"]["result"].is_object()).unwrap();
            assert_eq!(tool.payload["inputs"]["arguments"], json!({}));
            assert!(!tool.payload["outputs"]["result"].as_object().unwrap().is_empty());
            assert!(!format!("{sent:?}").contains("PRIVATE_MODEL_KEY"));
        }
    }
}
