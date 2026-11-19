use super::*;
use runtime::run_context::CancellationToken;

fn fixture(root: &Path) -> AgentConfig {
    let fixture = root.join("reader ACP fixture.cjs");
    fs::write(&fixture, r#"
const fs = require("node:fs");
const rl = require("node:readline").createInterface({input:process.stdin});
const send = value => process.stdout.write(JSON.stringify({jsonrpc:"2.0",...value})+"\n");
const update = (sessionId,text) => send({method:"session/update",params:{sessionId,
    update:{sessionUpdate:"agent_message_chunk",content:{type:"text",text}}}});
let waiting;
rl.on("line", line => {
    const {id,method,params,result} = JSON.parse(line);
    if (method === "initialize") {
        if (params.protocolVersion !== 1) process.exit(10);
        send({id,result:{protocolVersion:1,agentInfo:{name:"fixture"},agentCapabilities:{}}});
    } else if (method === "session/new") {
        if (params.cwd !== process.cwd() || params.mcpServers[0].name !== "reader") process.exit(11);
        if (params._meta?.mcpStartupAwaitTimeoutMs !== 20000) process.exit(12);
        send({id,result:{sessionId:"current"}});
    } else if (method === "session/prompt") {
        if (params.prompt[0].text === "permit") {
            waiting = id;
            send({id:"approval-9", method:"session/request_permission",params:{sessionId:"current",
                toolCall:{toolCallId:"tool-5",title:"Run book search",kind:"execute",rawInput:{query:"moon"}},
                options:[{optionId:"ask-ok",name:"Allow once",kind:"allow_once"},{optionId:"no",name:"Reject",kind:"reject_once"}]}});
            return;
        }
        update("previous","late");
        update("current","first");
        if (params.prompt[0].text === "stop") return;
        update("current"," second");
        send({id,result:{stopReason:params.prompt[0].text === "limit" ? "max_tokens" : "end_turn",
            ...(params.prompt[0].text === "usage" ? {usage:{inputTokens:60,outputTokens:20,totalTokens:80}} : {})}});
    } else if (method === "session/cancel") {
        fs.writeFileSync("cancelled.txt",params.sessionId);
    } else if (id === "approval-9" && !method) {
        update("current",result.outcome.optionId || result.outcome.outcome);
        send({id:waiting,result:{stopReason:"end_turn"}});
    }
});
"#).unwrap();
    AgentConfig {
        id: "fixture".into(),
        name: "Test Agent".into(),
        command: "node".into(),
        args: vec![fixture.to_string_lossy().into_owned()],
    }
}

fn sample_mcp() -> Vec<Value> {
    vec![json!({"name":"reader","command":"reader","args":["mcp"],"env":[]})]
}

#[test]
fn unanswered_permission_expires_and_removes_its_card() {
    let dir = tempfile::tempdir().unwrap();
    let events = runtime::run_events::RunEvents::default();
    let context = permission::PermissionContext {
        profile: dir.path(),
        turn_id: "expired",
        events: &events,
    };
    let mut process = std::process::Command::new("node")
        .args(["-e", "process.stdin.resume()"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let outcome = permission::choose(
        &context,
        &json!({"toolCall":{"title":"Read"},"options":[{"name":"Allow","kind":"allow_once","optionId":"ok"}]}),
        &CancellationToken::default(),
        &mut process,
        std::time::Duration::from_millis(20),
    );
    drop(process.stdin.take());
    process.wait().unwrap();
    assert_eq!(outcome["outcome"], "cancelled");
    assert!(
        permission_response(dir.path(), &json!({"action":"get","turn_id":"expired"})).unwrap()
            ["permission"]
            .is_null()
    );
}

#[test]
fn operator_permission_is_bound_to_the_current_profile_turn_and_option() {
    use runtime::run_events::RunEvents;
    use std::time::{Duration, Instant};
    for choice in ["ask-ok", "no", "cancel-run"] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let agent = fixture(root);
        let token = CancellationToken::default();
        let events = RunEvents::default();
        std::thread::scope(|s| {
            let task = s.spawn(|| {
                run_interactive(
                    &agent,
                    root,
                    &sample_mcp(),
                    "permit",
                    &token,
                    |_| {},
                    root,
                    "turn9",
                    &events,
                )
            });
            let get = json!({"action":"get","turn_id":"turn9"});
            let deadline = Instant::now() + Duration::from_secs(10);
            let card = loop {
                let card = permission_response(root, &get)
                    .unwrap()
                    .get("permission")
                    .cloned()
                    .unwrap();
                if !card.is_null() {
                    break card;
                }
                assert!(Instant::now() < deadline, "permission not published");
                std::thread::sleep(Duration::from_millis(20));
            };
            assert!(
                permission_response(&root.join("other"), &get).unwrap()["permission"].is_null()
            );
            assert!(
                permission_response(root, &json!({"action":"get","turn_id":"other"})).unwrap()
                    ["permission"]
                    .is_null()
            );
            assert_eq!(card["title"], "Run book search");
            assert!(card["details"].as_str().unwrap().contains("moon"));
            assert_eq!(card["options"][0]["kind"], "allow_once");
            let mut response =
                json!({"action":"respond","turn_id":"turn9","request_id":"wrong","option_id":"no"});
            assert!(permission_response(root, &response).is_err());
            response["request_id"] = card["request_id"].clone();
            response["option_id"] = json!("unknown");
            assert!(permission_response(root, &response).is_err());
            response["option_id"] = json!(choice);
            if choice == "cancel-run" {
                token.cancel();
                assert!(task.join().unwrap().unwrap_err().contains("取消"));
            } else {
                assert_eq!(permission_response(root, &response).unwrap()["ok"], true);
                assert_eq!(task.join().unwrap().unwrap(), choice);
            }
            assert!(permission_response(root, &response).is_err());
            assert!(permission_response(root, &get).unwrap()["permission"].is_null());
        });
    }
}

#[test]
fn real_stdio_stream_keeps_session_updates_and_terminal_reason() {
    let folder = tempfile::tempdir().unwrap();
    let agent = fixture(folder.path());
    let token = CancellationToken::default();
    let mut seen = Vec::new();
    assert_eq!(
        run(
            &agent,
            folder.path(),
            &sample_mcp(),
            "read",
            &token,
            |event| seen.push(event["content"]["text"].clone())
        )
        .unwrap(),
        "first second"
    );
    assert_eq!(seen, vec![json!("first"), json!(" second")]);
    let mut usage = Value::Null;
    run(
        &agent,
        folder.path(),
        &sample_mcp(),
        "usage",
        &token,
        |update| {
            if update["sessionUpdate"] == "reader_prompt_usage" {
                usage = update["usage"].clone();
            }
        },
    )
    .unwrap();
    assert_eq!(
        usage,
        json!({"inputTokens":60,"outputTokens":20,"totalTokens":80})
    );
    assert!(run(
        &agent,
        folder.path(),
        &sample_mcp(),
        "limit",
        &token,
        |_| {}
    )
    .unwrap_err()
    .contains("max_tokens"));
    assert!(run(
        &agent,
        folder.path(),
        &[json!({"type":"http","url":"http://localhost/mcp"})],
        "read",
        &token,
        |_| {}
    )
    .unwrap_err()
    .contains("http"));
}

#[test]
fn cancellation_is_delivered_to_the_active_real_process() {
    let folder = tempfile::tempdir().unwrap();
    let agent = fixture(folder.path());
    let token = CancellationToken::default();
    assert!(
        run(&agent, folder.path(), &sample_mcp(), "stop", &token, |_| {
            token.cancel()
        })
        .unwrap_err()
        .contains("取消")
    );
    assert_eq!(
        fs::read_to_string(folder.path().join("cancelled.txt")).unwrap(),
        "current"
    );
}

#[test]
fn profile_settings_save_probe_select_disconnect_reconnect_and_remove() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let config = fixture(root);
    update(root, &json!({"action":"save","agent":config})).unwrap();
    assert_eq!(
        update(root, &json!({"action":"probe","id":"fixture"})).unwrap()["probe"]["agent_info"]
            ["name"],
        "fixture"
    );
    update(root, &json!({"action":"select","id":"fixture"})).unwrap();
    assert_eq!(settings(root).unwrap()["selected_agent_id"], "fixture");
    update(root, &json!({"action":"disconnect","id":"fixture"})).unwrap();
    assert!(resolve(root, "fixture").is_err());
    assert!(settings(root).unwrap()["selected_agent_id"].is_null());
    update(root, &json!({"action":"connect","id":"fixture"})).unwrap();
    assert_eq!(resolve(root, "fixture").unwrap().args, config.args);
    assert!(update(root, &json!({"action":"uninstall","id":"fixture"})).is_err());
    update(root, &json!({"action":"remove","id":"fixture"})).unwrap();
    assert!(resolve(root, "fixture").is_err());
}
