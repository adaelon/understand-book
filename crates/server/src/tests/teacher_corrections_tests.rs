use super::*;
use memory::teaching::TeachingBinding;
use runtime::Role;

const WRONG: &str = "在两段路程相同的运动中，平均速度就是两个速度的算术平均值。例如10和30时是20。";
const SOURCE: &str = "Average speed is total distance divided by total elapsed time. For two completed equal-distance legs with positive speeds u and v, the average speed is 2uv/(u+v), not (u+v)/2. Equal time intervals instead allow the arithmetic mean of the two speeds. A complete trip requires both legs to finish; a zero speed that persists forever cannot complete a positive-distance leg. The completed-trip formula must not be applied to an unfinished trip. Total elapsed time must be positive.";

struct CorrectionScriptAdapter(ChatStubAdapter);
impl ModelAdapter for CorrectionScriptAdapter {
    fn complete(&self, request: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        self.0.complete(request)
    }
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
        if request
            .system
            .starts_with("You extract one explicit reader-profile memory operation.")
        {
            // This fixture corrects the teacher's arithmetic, not a reader trait.
            return Ok(json!({"intent":"none"}));
        }
        self.0.complete_structured(request)
    }
    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        self.0.chat(request)
    }
}
fn execute_script(
    state: &mut AppState,
    run: agent_run::PreparedAgentChat,
    script: Vec<AssistantTurn>,
) -> Value {
    let adapter = CorrectionScriptAdapter(ChatStubAdapter::scripted(script));
    let result = agent_run::execute_prepared(
        &agent_run::BorrowedAppPort(RefCell::new(state)),
        &adapter,
        run,
        Default::default(),
    );
    assert_eq!(result.reply.status, 200, "{}", result.reply.body);
    serde_json::from_str(&result.reply.body).unwrap()
}

fn ordinary() -> (tempfile::TempDir, AppState) {
    let mut base = sample_base();
    for node in &mut base.lid_nodes {
        node.span.end = SOURCE.encode_utf16().count();
    }
    let (root, mut state) = fixture_with_book(false, Book::new(base, SOURCE), SOURCE);
    state.user.store = MemoryStore::open(root.path().join("ordinary/memory.json")).unwrap();
    (root, state)
}
fn ask(state: &mut AppState, message: &str) -> agent_run::PreparedAgentChat {
    prepare_agent_chat(
        state,
        &json!({"message":message}).to_string(),
        "2026-11-13T00:00:00Z",
    )
    .unwrap_or_else(|r| panic!("{}", r.body))
}
fn binding(run: &agent_run::PreparedAgentChat) -> TeachingBinding {
    serde_json::from_value(run.tutor.as_ref().unwrap()["binding"].clone()).unwrap()
}
fn wrong_delivery(state: &mut AppState) -> String {
    let run = ask(
        state,
        "两段等路程时，平均速度能直接取两个速度的算术平均值吗？",
    );
    let b = binding(&run);
    execute_script(state, run, vec![answer(WRONG)]);
    state
        .user
        .learning_store()
        .unwrap()
        .teaching_turn_events(&b.tutor_session_id, &b.turn_id)
        .unwrap()
        .into_iter()
        .find(|e| {
            e.kind == TeachingFact::UsageObserved
                && e.payload["usage"]["kind"] == "assistant_delivery"
        })
        .unwrap()
        .event_id
}
fn correction(previous: &str) -> Value {
    json!({"operation":"corrections","correction_action":"record","operation_id":"correct-speed",
        "correction":{"previous_event_ref":previous,"mistake_quote":WRONG,
            "corrected_claim":"等路程、正速度且完成两段时，平均速度用总路程/总时间，即2uv/(u+v)。",
            "applies_when":"两段等路程、正速度、都走完；不能套到等时间区间。",
            "topics":["平均速度","等路程","average speed"],
            "source_quotes":[{"lid":"1.1","quote":"For two completed equal-distance legs with positive speeds u and v, the average speed is 2uv/(u+v), not (u+v)/2."}],"supersedes":[]}})
}

#[test]
fn teacher_corrections_delivery_recovery_preserves_older_receipts() {
    let (root, mut state) = ordinary();
    let run = ask(&mut state, "解释一下平均速度。");
    let turn = run.turn_ref.clone();
    let b = binding(&run);
    execute_script(
        &mut state,
        run,
        vec![answer("平均速度是总路程除以总时间。")],
    );
    let id = format!("usage:{}:assistant:{}", b.tutor_session_id, b.turn_id);
    let db = rusqlite::Connection::open(root.path().join("ordinary/learning.db")).unwrap();
    // This is the persisted shape before completion state was recorded.
    assert_eq!(db.execute("UPDATE teaching_trace SET event=json_remove(event,'$.payload.incomplete') WHERE event_id=?",[&id]).unwrap(),1);
    teaching::record_user_delivery(&state.user, &turn, "2026-11-13T00:01:00Z").unwrap();
    let old = state
        .user
        .learning_store()
        .unwrap()
        .teaching_event(&id)
        .unwrap();
    assert!(old.payload.get("incomplete").is_none());
    assert_eq!(old.payload["answer"], "平均速度是总路程除以总时间。");
}

#[test]
fn teacher_corrections_host_checks_actual_reads_and_quotes_then_recalls_after_delivery() {
    let (_root, mut state) = ordinary();
    let previous = wrong_delivery(&mut state);
    let run = ask(&mut state, "刚才说错了，请核对原文并纠正。");
    let b = binding(&run);
    let request = correction(&previous);
    assert!(teaching::step(
        &state.private_context(),
        &run.turn_ref,
        request.clone(),
        &[],
        &[]
    )
    .is_err());
    let mut fabricated = request.clone();
    fabricated["correction"]["source_quotes"][0]["quote"] = json!("Fabricated source quote");
    assert!(teaching::step(
        &state.private_context(),
        &run.turn_ref,
        fabricated,
        &[],
        &[range()]
    )
    .is_err());
    let result = execute_script(
        &mut state,
        run,
        vec![
            call("book.text", json!({"lid":"1.1"})),
            call("tutor.step", request),
            answer("刚才的算法有误。等路程时须先分别求用时再相加，10和30对应的全程平均速度是15。"),
        ],
    );
    assert!(result["answer"].as_str().unwrap().contains("15"));
    let found = state
        .user
        .learning_store()
        .unwrap()
        .search_teacher_corrections(&b, &["平均速度".into()], 0)
        .unwrap();
    assert_eq!(found["total"], 1, "{result}");
    let reference = found["candidates"][0]["correction_ref"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(state
        .user
        .learning_store()
        .unwrap()
        .current_interpretations(&b.source_id, &b.source_revision, 0, 20)
        .unwrap()
        .is_empty());
    assert_eq!(post(&mut state, "/agent/new", "{}").status, 200);
    let next = ask(&mut state, "等时间区间的平均速度呢？");
    assert_eq!(
        next.tutor.as_ref().unwrap()["teacher_corrections"]["record_count"],
        1
    );
    assert_ne!(binding(&next).chat_session_id, b.chat_session_id);
    let result = execute_script(
        &mut state,
        next,
        vec![
            call(
                "tutor.step",
                json!({"operation":"corrections","correction_action":"search","queries":["平均速度"]}),
            ),
            call(
                "tutor.step",
                json!({"operation":"corrections","correction_action":"use","operation_id":"different-condition","correction_ref":reference,
            "decision":"not_applicable","reason":"本次等时间，旧纠正针对等路程；两者的权重不同。"}),
            ),
            answer("这次每段时间相同，可以用两个速度的算术平均值。"),
        ],
    );
    assert!(result["answer"].as_str().unwrap().contains("可以"));
}

fn live_turn(
    state: &mut AppState,
    message: &str,
    rows: &mut Vec<Value>,
    output: &std::path::Path,
) -> TeachingBinding {
    let run = ask(state, message);
    let b = binding(&run);
    let hint = run.tutor.as_ref().unwrap()["teacher_corrections"].clone();
    let adapter = ProviderRegistry::adapter_from_config(ProviderConfig::from_env().unwrap());
    let started = std::time::Instant::now();
    let result = agent_run::execute_prepared(
        &agent_run::BorrowedAppPort(RefCell::new(state)),
        adapter.as_ref(),
        run,
        Default::default(),
    );
    let body: Value =
        serde_json::from_str(&result.reply.body).unwrap_or(json!({"raw":result.reply.body}));
    let facts = state
        .user
        .learning_store()
        .unwrap()
        .teaching_turn_events(&b.tutor_session_id, &b.turn_id)
        .unwrap();
    let teacher: Vec<_> = facts
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                TeachingFact::TeacherCorrectionRecorded | TeachingFact::TeacherCorrectionUsed
            )
        })
        .collect();
    let messages = &state
        .user
        .agent_history
        .sessions
        .iter()
        .find(|s| s.id == b.chat_session_id)
        .unwrap()
        .messages;
    let start = messages
        .iter()
        .rposition(|m| m.role == Role::User)
        .unwrap_or(0);
    let messages = &messages[start..];
    let calls: BTreeMap<_, _> = messages
        .iter()
        .flat_map(|m| m.tool_calls.iter())
        .filter(|call| call.name == "tutor.step")
        .map(|call| (call.id.as_str(), call))
        .collect();
    let correction_calls:Vec<_>=messages.iter().filter(|m|m.role==Role::Tool).filter_map(|m| {
        let call=calls.get(m.tool_call_id.as_deref()?)?;
        let args:Value=serde_json::from_str(&call.arguments).ok()?;
        (args["operation"]=="corrections").then(||json!({"arguments":args,
            "result":serde_json::from_str::<Value>(m.content.as_deref().unwrap_or_default()).unwrap_or(Value::Null)}))
    }).collect();
    rows.push(json!({"message":message,"binding":b,"hint":hint,"status":result.reply.status,"elapsed_ms":started.elapsed().as_millis(),
        "answer":body["answer"],"incomplete":body["incomplete"],"diagnostics":body["diagnostics"],"trace":body["trace"],"teacher_events":teacher,"correction_calls":correction_calls}));
    std::fs::write(
        output,
        serde_json::to_string_pretty(
            &json!({"model":ProviderConfig::from_env().ok().map(|c|c.model),"source":SOURCE,
        "controlled_initial_wrong_delivery":WRONG,"turns":rows}),
        )
        .unwrap(),
    )
    .unwrap();
    println!(
        "TEACHER_CORRECTION_LIVE {}",
        json!({"turn":rows.len(),"message":message,"answer":body["answer"],"teacher_events":teacher})
    );
    assert_eq!(result.reply.status, 200, "{body}");
    assert_ne!(body["incomplete"], true, "{body}");
    b
}

#[test]
#[ignore = "real configured provider; teacher correction production path across multiple turns and chats"]
fn teacher_corrections_live_multiturn() {
    let output = std::path::PathBuf::from(
        std::env::var_os("TEACHER_CORRECTION_EVAL_OUTPUT").expect("set an output JSON path"),
    );
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    let (_root, mut state) = ordinary();
    let config = ProviderConfig::from_env().unwrap();
    println!("TEACHER_CORRECTION_PROVIDER {}", config.model);
    state.services.adapter = ProviderRegistry::adapter_from_config(config);
    wrong_delivery(&mut state);
    let mut rows = Vec::new();
    let first=live_turn(&mut state,"刚才那句话不对。请核对原文：两段等路程，以10 km/h和30 km/h行驶，全程平均速度应是多少？直接把原因和结果讲完整。",&mut rows,&output);
    let first_cards = state
        .user
        .learning_store()
        .unwrap()
        .search_teacher_corrections(&first, &["平均速度".into(), "average speed".into()], 0)
        .unwrap();
    assert!(
        !first_cards["candidates"].as_array().unwrap().is_empty(),
        "A natural correction was not saved; inspect the recorded live trace"
    );
    // Controlled unrelated traffic makes the old correction fall outside the
    // recent-turn window. Only the six substantive turns use the real model.
    for i in 0..12 {
        let run = ask(&mut state, &format!("无关排版记录{i}：请只回复收到。"));
        execute_script(&mut state, run, vec![answer("收到。")]);
    }
    assert_eq!(post(&mut state, "/agent/new", "{}").status, 200);
    live_turn(&mut state,"两段距离一样长，第一段每小时12千米、第二段每小时36千米。平均速度怎么算？这次请直接完整讲解，不要反问。",&mut rows,&output);
    live_turn(
        &mut state,
        "这次换成两段时间一样长，速度还是12和36。能直接取算术平均吗？请完整给出结果。",
        &mut rows,
        &output,
    );
    live_turn(&mut state,"如果两段时间仍相等，各1小时，第一小时一直停着、速度为0，第二小时以36千米/小时行驶，算术平均还成立吗？请直接给出结果和适用条件，不要反问。",&mut rows,&output);
    live_turn(&mut state,"再修正一下上次等路程的说法：公式还必须限定两个速度为正，而且两段路程都实际完成。第二段速度为0、一直没有前进时，不能套公式说完成全程的平均速度是0。请核对原文，把此前那条纠正补充严谨。",&mut rows,&output);
    assert_eq!(post(&mut state, "/agent/new", "{}").status, 200);
    live_turn(&mut state,"两段等路程，速度分别为12和0，第二段一直没有前进。代入公式能得出完成全程的平均速度是0吗？直接说明，不用出题。",&mut rows,&output);
    assert!(
        rows[1]["teacher_events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "teacher_correction_used" && e["payload"]["decision"] == "apply"),
        "New-chat answer did not use retrieved correction"
    );
    assert!(
        rows[2..4]
            .iter()
            .flat_map(|row| row["teacher_events"].as_array().unwrap())
            .any(|e| e["kind"] == "teacher_correction_used"
                && e["payload"]["decision"] == "not_applicable"),
        "Equal-time request needs different applicability"
    );
    // The two numeric answers also need manual semantic review of their stated
    // conditions; a not_applicable receipt alone did not catch the run-02 error.
    assert!(rows[2]["answer"].as_str().unwrap().contains("24"));
    assert!(rows[3]["answer"].as_str().unwrap().contains("18"));
    let latest = rows
        .iter()
        .rev()
        .flat_map(|row| row["teacher_events"].as_array().unwrap().iter().rev())
        .find(|event| event["kind"] == "teacher_correction_recorded")
        .unwrap();
    assert!(rows[5]["correction_calls"].as_array().unwrap().iter().any(|call| {
        let result=call["result"].get("model_body").unwrap_or(&call["result"]);
        result["candidates"].as_array().into_iter().flatten().any(|card|card["correction_ref"]==latest["event_id"])
    }),"The last new chat must actually retrieve the latest correction, not just independently answer the math problem");
}
