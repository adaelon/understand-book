use super::*;

/// Real multi-user HTTP routes and temporary published books; only the model
/// response and the local reverse-proxy identity are controlled by the test.
#[test]
#[ignore = "bounded HTTP host for learning-workspace.spec.ts"]
fn w5_browser_host() {
    let f = Fixture::new();
    configure(&f, &Probe::default());
    let server = multi_user_host::start_with_access(
        "127.0.0.1:4176".parse().unwrap(),
        Arc::new(Site::new("https://reader.example").unwrap()), f.access.clone(),
    ).unwrap();
    let control = tiny_http::Server::http("127.0.0.1:4177").unwrap();
    let mut gate: Option<Arc<(Mutex<bool>, Condvar)>> = None;
    println!("W5_BROWSER_READY");
    let deadline = std::time::Instant::now() + Duration::from_secs(900);
    while std::time::Instant::now() < deadline {
        let Some(mut request) = control.recv_timeout(Duration::from_secs(1)).unwrap() else { continue; };
        let path = request.url().to_owned();
        let value = match path.as_str() {
            "/fixture" => json!({"first":f.x,"second":f.y,
                "cookie":format!("{}={}",crate::auth::COOKIE,f.a), "csrf":crate::auth::csrf(&f.a)}),
            "/model" => {
                let input: Value = serde_json::from_reader(request.as_reader()).unwrap();
                let quote = input["book"].as_str().map(|which| {
                    let reference = if which == "second" { &f.y } else { &f.x };
                    f.access.library.lock().unwrap().load("A", reference).unwrap().book.text("1.1", None).unwrap()
                });
                gate = (input["blocked"] == true).then(|| Arc::new((Mutex::new(false), Condvar::new())));
                configure(&f, &Probe { primary_quote:quote, block:gate.clone(), ..Default::default() });
                json!({"ok":true})
            }
            "/release" => {
                if let Some(gate) = gate.take() {
                    *gate.0.lock().unwrap() = true; gate.1.notify_all();
                }
                json!({"ok":true})
            }
            "/stop" => {
                if let Some(gate) = gate.take() {
                    *gate.0.lock().unwrap() = true; gate.1.notify_all();
                }
                request.respond(tiny_http::Response::from_string("stopped")).unwrap();
                break;
            }
            _ => json!({"error":"unknown test control"}),
        };
        request.respond(tiny_http::Response::from_string(value.to_string()).with_header(
            tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())).unwrap();
    }
    server.shutdown();
}
