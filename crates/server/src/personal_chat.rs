//! Cloud personal-chat entry uses the shared private conversation implementation.
pub use reader_service::personal_chat::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use std::sync::{Arc, Mutex};
    #[derive(Clone, Default)]
    struct Probe(Arc<Mutex<Vec<String>>>);
    impl ModelAdapter for Probe {
        fn complete(
            &self,
            _: runtime::CompletionRequest,
        ) -> Result<runtime::ParsedResponse, runtime::AdapterError> {
            Err(runtime::AdapterError { message: "PN interface fixture does not provide background profile completions".into(), spend_stop: None })
        }
        fn complete_structured(&self, request: runtime::CompletionRequest) -> Result<Value, runtime::AdapterError> {
            crate::tests::tutor_loop_tests::fixture_coach_guidance(&request)
                .ok_or_else(|| runtime::AdapterError { message: "PN interface fixture supplies no additional learner judgment".into(), spend_stop: None })
        }
        fn chat(
            &self,
            plan: &runtime::AgentRequestPlan,
        ) -> Result<runtime::AssistantTurn, runtime::AdapterError> {
            let text = serde_json::to_string(&plan.ordered_messages()).unwrap();
            self.0.lock().unwrap().push(text);
            Ok(runtime::AssistantTurn {
                provider_continuation: None,
                text: Some(
                    "参数是已学到的固定值，激活还依赖这次输入。可以用同一函数代入不同输入来比较。"
                        .into(),
                ),
                tool_calls: vec![],
                usage_total_tokens: Some(17),
            })
        }
    }
    #[test]
    fn personal_notes_network_admission_is_owned_idempotent_and_cancellable() {
        let f = crate::tests::mu5_tests::Fixture::new();
        let probe = Probe::default();
        let adapter = probe.clone();
        f.access
            .runs
            .configure_fake(move || Box::new(adapter.clone()));
        let call = |method, path, input| f.call(&f.a, method, path, input);
        let (status, chat) = call("POST", "/api/me/notes/chat_create", json!({}));
        assert_eq!(status, 200, "{chat}");
        let request = json!({"scope":"personal","session_id":chat["session_id"],"message":"没有打开资料，也想讨论自己的想法","client_request_id":"personal-network","expected_chat_head":null});
        let (status, accepted) = call("POST", "/api/agent/runs", request.clone());
        assert_eq!(status, 202, "{accepted}");
        let again = call("POST", "/api/agent/runs", request.clone());
        assert_eq!(again.1["turn_id"], accepted["turn_id"]);
        let other = f
            .access
            .auth
            .login(
                "B",
                "fixture-only-password",
                None,
                crate::service_error::now(),
            )
            .unwrap();
        assert_ne!(
            f.call(
                &other,
                "POST",
                "/api/me/notes/chat_history",
                json!({"session_id":chat["session_id"]})
            )
            .0,
            200
        );
        assert!(f.access.runs.run_one(&f.access).unwrap());
        let (_, history) = call(
            "POST",
            "/api/me/notes/chat_history",
            json!({"session_id":chat["session_id"]}),
        );
        assert_eq!(history["turns"][0]["status"], "completed", "{history}");
        let path = format!("/api/agent/runs/{}", accepted["turn_id"].as_str().unwrap());
        assert_eq!(call("GET", &path, json!({})).0, 200);
        let mut second = request;
        second["client_request_id"] = json!("personal-cancel");
        second["expected_chat_head"] = accepted["turn_id"].clone();
        let (status, queued) = call("POST", "/api/agent/runs", second);
        assert_eq!(status, 202, "{queued}");
        let path = format!(
            "/api/agent/runs/{}/cancel",
            queued["turn_id"].as_str().unwrap()
        );
        assert_eq!(call("POST", &path, json!({})).0, 200);
        let (_, history) = call(
            "POST",
            "/api/me/notes/chat_history",
            json!({"session_id":chat["session_id"]}),
        );
        assert_eq!(history["turns"][1]["status"], "cancelled", "{history}");
        assert_eq!(probe.0.lock().unwrap().len(), 1);
    }
    #[test]
    #[ignore = "PN browser acceptance host; POST :4231/stop to finish"]
    fn personal_notes_browser_host() {
        let f = crate::tests::mu5_tests::Fixture::new();
        let probe = Probe::default();
        let model = probe.clone();
        f.access
            .runs
            .configure_fake(move || Box::new(model.clone()));
        let site = Arc::new(crate::multi_user_host::Site::new("https://reader.example").unwrap());
        let server = crate::multi_user_host::start_with_access(
            "127.0.0.1:4230".parse().unwrap(),
            site,
            f.access.clone(),
        )
        .unwrap();
        let control = tiny_http::Server::http("127.0.0.1:4231").unwrap();
        println!("PN_BROWSER_READY");
        loop {
            let request = control.recv().unwrap();
            if request.url() == "/stop" {
                request
                    .respond(tiny_http::Response::from_string("stopped"))
                    .unwrap();
                break;
            }
            let data = if request.url() == "/fixture" {
                json!({"cookie":format!("{}={}",crate::auth::COOKIE,f.a),"publication":f.x})
            } else {
                json!({"requests":*probe.0.lock().unwrap()})
            };
            request
                .respond(
                    tiny_http::Response::from_string(data.to_string()).with_header(
                        tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                    ),
                )
                .unwrap();
        }
        server.shutdown();
    }
}
