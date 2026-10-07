use serde_json::{json, Value};
use server::{control_store::{ControlStore, ServiceWriter}, published_library::PublishedLibrary,
    multi_user_host::{self, MultiUserConfig}};
use std::{path::PathBuf, time::{Duration, Instant}};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = PathBuf::from(&args[0]);
    let book = PathBuf::from(&args[1]);
    assert!(!root.exists(), "Canary requires a new isolated service directory");
    let publication = {
        let writer = ServiceWriter::acquire(&root).unwrap();
        let mut control = ControlStore::open(writer.clone()).unwrap();
        control.provision_password("fixture", "fixture-only-password", true).unwrap();
        let mut library = PublishedLibrary::new(ControlStore::open(writer).unwrap());
        let publication = library.publish(&book).unwrap().reference;
        library.grant("fixture", &publication).unwrap();
        publication
    };
    let provider = tiny_http::Server::http("127.0.0.1:0").unwrap();
    std::env::set_var("UNDERSTAND_BOOK_PROVIDER", "native");
    std::env::set_var("OPENCODE_BASE_URL", format!("http://{}", provider.server_addr()));
    std::env::set_var("OPENCODE_API_KEY", "fixture-only-key");
    std::env::set_var("FLUID_LLM_MODEL", "formal-admission-fixture");
    let model = std::thread::spawn(move || {
        for index in 0..2 {
            let mut request = provider.recv_timeout(Duration::from_secs(20)).unwrap().unwrap();
            let _: Value = serde_json::from_reader(request.as_reader()).unwrap();
            let message = if index == 0 { json!({"role":"assistant","content":null,"tool_calls":[{
                "id":"read-one","type":"function","function":{"name":"book_structure","arguments":"{}"}
            }]}) } else { json!({"role":"assistant","content":"你好！"}) };
            let body = json!({"choices":[{"message":message}],"usage":{
                "prompt_tokens":1000,"completion_tokens":20,"prompt_cache_hit_tokens":800,"total_tokens":1020
            }});
            request.respond(tiny_http::Response::from_string(body.to_string())
                .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())).unwrap();
        }
    });
    let host = multi_user_host::start(MultiUserConfig {
        root, addr: "127.0.0.1:0".parse().unwrap(), origin: "https://fixture.example".into(),
    }).unwrap();
    assert!(host.observability_status().enabled);
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).build();
    let login = agent.post(&format!("{}/api/auth/login", host.url))
        .set("Host", "fixture.example").set("Origin", "https://fixture.example")
        .send_json(json!({"username":"fixture","password":"fixture-only-password"})).unwrap();
    let cookie = login.header("Set-Cookie").unwrap().split(';').next().unwrap().to_owned();
    let login: Value = login.into_json().unwrap();
    let call = |method: &str, path: &str, body: Value| -> Value {
        let request = agent.request(method, &format!("{}{path}", host.url))
            .set("Host", "fixture.example").set("Origin", "https://fixture.example")
            .set("Cookie", &cookie).set("X-CSRF-Token", login["csrf_token"].as_str().unwrap());
        let response = if method == "GET" { request.call() } else { request.send_json(body) }.unwrap();
        response.into_json().unwrap()
    };
    let workspace = call("POST", "/api/workspaces", json!({"published_book_ref":publication,"attachment_id":"fixture"}));
    let stamp = |w: &Value| json!({"generation":w["generation"],"expected_revision":w["revision"],"attachment_id":"fixture"});
    let workspace = call("POST", &format!("/api/workspaces/{}/chat/new",workspace["workspace_id"].as_str().unwrap()),stamp(&workspace));
    let mut input = stamp(&workspace);
    input["session_id"] = workspace["selected_chat"].clone();
    input["client_request_id"] = json!("formal-canary");
    input["message"] = json!("你好，PRIVATE_FORMAL_CANARY");
    let accepted = call("POST", &format!("/api/workspaces/{}/agent/runs",workspace["workspace_id"].as_str().unwrap()),input);
    let turn = accepted["turn_id"].as_str().unwrap();
    let deadline = Instant::now()+Duration::from_secs(35);
    let finished = loop {
        let result = call("GET", &format!("/api/agent/runs/{turn}"),Value::Null);
        if result["dispatch_state"] == "settled" { break result; }
        assert!(Instant::now()<deadline, "formal admission did not settle: {result}");
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(finished["turn"]["status"], "completed", "{finished}");
    let activities = finished["turn"]["run_summary"]["activities"].as_array().unwrap();
    let models = activities.iter().filter(|a| a["kind"] == "model").collect::<Vec<_>>();
    assert_eq!(models.len(),2);
    assert!(activities.iter().any(|row| row["kind"] == "tool" && row["status"] == "succeeded"));
    for row in &models {
        assert_eq!(row["usage"]["input_tokens"],1000);
        assert_eq!(row["usage"]["cached_input_tokens"],800);
        assert!(row["request_diagnostics"].is_object());
    }
    model.join().unwrap();
    let deadline = Instant::now()+Duration::from_secs(35);
    loop {
        let status = host.observability_status();
        if status.sent>=8 && status.queued==0 && status.spool_pending==0 { break; }
        assert!(Instant::now()<deadline,"exports did not drain: {status:?}");
        std::thread::sleep(Duration::from_millis(100));
    }
    let status = host.observability_status();
    assert_eq!(status.dropped,0);
    assert!(status.last_error_code.is_none(),"{status:?}");
    println!("{}",json!({"turn_id":turn,"local_model_calls":2,"local_input_tokens":2000,
        "local_cached_input_tokens":1600,"status":status}));
    host.shutdown();
}
