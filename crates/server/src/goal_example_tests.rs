use super::*;
use std::time::Instant;

fn receipt(step: &ProviderStep) -> Value {
    let message = step.request["messages"].as_array().unwrap().iter().rev()
        .find(|m| m["role"] == "tool").unwrap();
    serde_json::from_str::<Value>(message["content"].as_str().unwrap()).unwrap()["model_body"].clone()
}

#[test]
fn to5_goal_examples_use_real_turns_and_persist_before_next_sampling() {
    let provider = Provider::new();
    let mut sequence = 0;
    let mut send = |step: ProviderStep, arguments: Value| {
        sequence += 1;
        step.finish(json!({"role":"assistant","content":null,"tool_calls":[{
            "id":format!("to5-{sequence}"),"type":"function",
            "function":{"name":"goal.update","arguments":arguments.to_string()}
        }]}));
    };
    let (running, _) = fixture("to5-goal");
    running.set_provider_config(provider.config.clone());
    let worker = chat(&running, "解释这一段并保存一条笔记");
    let first = provider.next();
    let schema = &first.request["tools"].as_array().unwrap().iter()
        .find(|t| t["function"]["parameters"]["properties"]["operation"]["enum"] == json!(["working","refine","revise"]))
        .unwrap()["function"]["parameters"];
    let examples = schema["examples"].as_array().unwrap().clone();
    let (path, original) = {
        let state = running.state.lock().unwrap();
        (state.user.history_path.clone(), state.user.agent_history.sessions[0].goals[0].clone())
    };
    let load_goal = || crate::session_store::load_chat_storage(&path).unwrap().0.sessions[0].goals[0].clone();
    // Verify the no-change domain result without issuing a redundant tool call.
    let unchanged = json!({"operation":"refine","interpretation":original.interpretation,"requirements":original.requirements});
    let mut same = original.clone();
    assert!(!same.apply_update(serde_json::from_value(unchanged.clone()).unwrap(), &original.origin_turn_id, "解释这一段并保存一条笔记").unwrap());
    assert_eq!(same, original);
    let mut calls = Vec::new();
    let input = examples.iter().find(|e| e["operation"] == "working").unwrap().clone();
    send(first, input.clone());
    let second = provider.next();
    let working = load_goal();
    assert_eq!(working.requirements, original.requirements);
    assert_eq!(working.working.items.len(), 2);
    assert_eq!(receipt(&second), json!({"goal_id":original.id,"revision":working.revision,"changed":true}));
    calls.push(json!({"input":input,"receipt":receipt(&second),"persisted":working}));

    let mut refine = examples.iter().find(|e| e["operation"] == "refine").unwrap().clone();
    for requirement in refine["requirements"].as_array_mut().unwrap() {
        requirement["basis_turn_id"] = json!(original.origin_turn_id);
    }
    send(second, refine.clone());
    let third = provider.next();
    let refined = load_goal();
    assert_eq!(refined.working, working.working);
    assert!(refined.objective_gap(0, 0).is_some());
    assert_eq!(receipt(&third)["revision"], refined.revision);
    calls.push(json!({"input":refine,"receipt":receipt(&third),"persisted":refined}));
    third.answer("笔记尚未保存。");
    let deadline = Instant::now() + DEADLINE;
    while !worker.is_finished() {
        assert!(Instant::now() < deadline, "expected incomplete goal finalization");
        if let Ok(step) = provider.steps.recv_timeout(Duration::from_millis(100)) {
            step.answer("笔记尚未保存。");
        }
    }
    assert_eq!(worker.join().unwrap().0, 200);
    assert_eq!(load_goal().status, runtime::goal::GoalStatus::Open);

    let current_message = "改成只解释，不要保存笔记";
    let url = running.url.clone();
    let goal_id = original.id.clone();
    let worker = thread::spawn(move || http(&url, "POST", "/agent/chat", json!({"message":current_message,"goal_id":goal_id})));
    let next = provider.next();
    let current = load_goal();
    assert_eq!(current.id, original.id);
    // Use the actual pending turn and explicitly selected existing Goal.
    let history = crate::session_store::load_chat_storage(&path).unwrap().0;
    let current_turn = &history.sessions[0].turns.last().unwrap().turn_id;
    assert_ne!(current_turn, &original.origin_turn_id);
    assert_eq!(current.user_message_refs.last(), Some(current_turn));
    assert!(next.request.to_string().contains(current_turn));
    // Every example reference is replaced using the actual persisted turn.
    let mut revise = examples.iter().find(|e| e["operation"] == "revise").unwrap().clone();
    revise["basis_turn_id"] = json!(current_turn);
    revise["requirements"][0]["basis_turn_id"] = json!(current_turn);
    assert!(current_message.contains(revise["basis_quote"].as_str().unwrap()));
    let mut invalid_quote = revise.clone();
    invalid_quote["basis_quote"] = json!("用户未说过的话");
    send(next, invalid_quote.clone());
    let rejected = provider.next();
    assert!(rejected.request.to_string().contains("GOAL_UPDATE_INVALID"));
    assert_eq!(load_goal(), current);
    let mut failures = vec![json!({"input":invalid_quote,"request":rejected.request})];
    let mut stale_turn = revise.clone();
    stale_turn["basis_turn_id"] = json!(original.origin_turn_id);
    // Additional invalid bases use the exact domain handler; repeated invalid
    // model-tool batches are intentionally terminated by the Runtime guard.
    for input in [stale_turn, refine] {
        let mut candidate = current.clone();
        let error = candidate.apply_update(serde_json::from_value(input.clone()).unwrap(), current_turn, current_message).unwrap_err();
        assert_eq!(candidate, current);
        failures.push(json!({"input":input,"domain_error":error}));
    }
    send(rejected, revise.clone());
    let final_step = provider.next();
    let revised = load_goal();
    assert_eq!(receipt(&final_step)["revision"], revised.revision);
    assert_eq!(revised.requirements.len(), 1);
    assert_eq!(revised.requirements[0].verification, runtime::goal::GoalVerification::Content);
    assert_eq!(revised.working, working.working);
    assert_eq!(revised.status, runtime::goal::GoalStatus::Open);
    calls.push(json!({"input":revise,"receipt":receipt(&final_step),"persisted":revised}));
    final_step.answer("这里要求先确认条件成立，再判断结论是否适用。");
    assert_eq!(worker.join().unwrap().0, 200);
    let final_goal = load_goal();
    assert_eq!(final_goal.status, runtime::goal::GoalStatus::Completed);
    if let Ok(directory) = std::env::var("TO5_DIR") {
        std::fs::write(PathBuf::from(directory).join("goal.json"), serde_json::to_vec_pretty(&json!({
            "calls":calls,"rejected":failures,"final_goal":final_goal,
            "unchanged_domain_update":{"input":unchanged,"changed":false,"goal":same},
            "origin_message":"解释这一段并保存一条笔记","revision_message":current_message,
            "history":crate::session_store::load_chat_storage(&path).unwrap().0
        })).unwrap()).unwrap();
    }
    running.shutdown();
}
