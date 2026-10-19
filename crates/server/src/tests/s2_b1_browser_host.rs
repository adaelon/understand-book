use super::mu5_tests::Fixture;
use serde_json::{json, Value};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[test]
#[ignore = "reopen the persisted B1 acceptance service after a process restart"]
fn b1_resume_browser_host() {
    let path = std::env::var("B1_ACCEPTANCE_FIXTURE").expect("saved fixture JSON path");
    let fixture: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let users = crate::user_registry::UserRegistry::open(std::path::Path::new(
        fixture["root"].as_str().unwrap(),
    ))
    .unwrap();
    let access = Arc::new(crate::authorization::Authorization::new(users).unwrap());
    let server = crate::multi_user_host::start_with_access(
        "127.0.0.1:4190".parse().unwrap(),
        Arc::new(crate::multi_user_host::Site::new(fixture["origin"].as_str().unwrap()).unwrap()),
        access,
    )
    .unwrap();
    let control = tiny_http::Server::http("127.0.0.1:4191").unwrap();
    println!("B1_RESUME_BROWSER_READY");
    let until = Instant::now() + Duration::from_secs(2400);
    while Instant::now() < until {
        let Some(request) = control.recv_timeout(Duration::from_secs(1)).unwrap() else {
            continue;
        };
        if request.url() == "/stop" {
            request
                .respond(tiny_http::Response::from_string("stopped"))
                .unwrap();
            break;
        }
        let result = if request.url() == "/fixture" {
            fixture.clone()
        } else {
            Value::Null
        };
        request
            .respond(
                tiny_http::Response::from_string(result.to_string()).with_header(
                    tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                ),
            )
            .unwrap();
    }
    server.shutdown();
}

#[test]
#[ignore = "actual HTTP fixture for shared workspace and operator build browser/DSH acceptance"]
fn s2_b1_browser_host() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    let directory = f
        .access
        .library
        .lock()
        .unwrap()
        .load("A", &f.x)
        .unwrap()
        .directory()
        .to_path_buf();
    let new_first = f
        .access
        .library
        .lock()
        .unwrap()
        .publish_online(&crate::online_publication::PublicationRequest {
            operation_id: "s2-new-material".into(),
            input_directory: directory,
            grant_users: vec!["A".into(), "B".into()],
        })
        .unwrap()
        .reference;
    let source = f
        .access
        .library
        .lock()
        .unwrap()
        .load("A", &f.y)
        .unwrap()
        .directory()
        .to_path_buf();
    let mut release = super::s1_workspace_publication_tests::publication(&f, "s2-initial", 0);
    release["release_id"] = json!("shared-course");
    release["manifest"]["name"] = json!("深度学习入门");
    release["manifest"]["public_goal"] = json!("理解神经网络基础");
    release["manifest"]["materials"] =
        json!([{"published_book_ref":f.x,"required_capabilities":["text","tutor"]}]);
    release["manifest"]["topic_links"] = json!([]);
    release["manifest"]["suggested_route"] = json!([{"title":"从基本概念开始","description":"阅读基础概念并检查理解","materials":[f.x]}]);
    f.ok(
        &f.b,
        "POST",
        "/api/admin/workspace-releases",
        release.clone(),
    );
    let origin = "https://127.0.0.1:4190";
    let server = crate::multi_user_host::start_with_access(
        "127.0.0.1:4190".parse().unwrap(),
        Arc::new(crate::multi_user_host::Site::new(origin).unwrap()),
        f.access.clone(),
    )
    .unwrap();
    let control = tiny_http::Server::http("127.0.0.1:4191").unwrap();
    println!("S2_B1_BROWSER_READY");
    let until = Instant::now() + Duration::from_secs(2400);
    while Instant::now() < until {
        let Some(request) = control.recv_timeout(Duration::from_secs(1)).unwrap() else {
            continue;
        };
        let result = match request.url() {
            "/stop" => {
                request
                    .respond(tiny_http::Response::from_string("stopped"))
                    .unwrap();
                break;
            }
            "/fixture" => {
                json!({"first":f.x,"second":f.y,"new_first":new_first,"release_id":"shared-course","root":f.root.path(),"source_directory":source,"origin":origin,"reader_user_id":"A","admin_user_id":"B","visible_to_users":["A","B"],"reader_cookie":format!("{}={}",crate::auth::COOKIE,f.a),"admin_cookie":format!("{}={}",crate::auth::COOKIE,f.b),"admin_csrf":crate::auth::csrf(&f.b)})
            }
            "/publish-next" => {
                let mut next = release.clone();
                next["operation_id"] = json!("s2-next");
                next["expected_version"] = json!(1);
                next["manifest"]["public_goal"] = json!("理解神经网络并完成应用练习");
                next["manifest"]["materials"][0]["published_book_ref"] = json!(new_first);
                next["manifest"]["suggested_route"][0]["materials"] = json!([new_first]);
                next["manifest"]["suggested_route"][0]["description"] = json!("阅读新版后完成练习");
                f.ok(&f.b, "POST", "/api/admin/workspace-releases", next)
            }
            "/archive" | "/restore" => {
                let archive = request.url() == "/archive";
                f.ok(&f.b,"POST","/api/admin/workspace-releases/shared-course/archive",json!({"operation_id":if archive{"s2-archive"}else{"s2-restore"},"archived":archive}))
            }
            "/revoke-reader-material" => {
                f.access
                    .library
                    .lock()
                    .unwrap()
                    .revoke("A", &new_first)
                    .unwrap();
                json!({"revoked":true})
            }
            "/grant-reader-material" => {
                f.access
                    .library
                    .lock()
                    .unwrap()
                    .grant("A", &new_first)
                    .unwrap();
                json!({"granted":true})
            }
            _ => Value::Null,
        };
        request
            .respond(
                tiny_http::Response::from_string(result.to_string()).with_header(
                    tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                ),
            )
            .unwrap();
    }
    server.shutdown();
}
