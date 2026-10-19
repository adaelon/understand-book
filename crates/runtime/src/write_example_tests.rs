use super::*;
use serde_json::{json, Value};

fn examples(name: &str) -> Vec<Value> {
    resident_tool_registry().registration(name).unwrap().spec.parameters["examples"]
        .as_array().unwrap().clone()
}

fn source_book() -> Book {
    let mut base = book_leaves(1).base;
    base.graph_nodes.push(GraphNode {
        identity: None, evidence: vec![],
        id: "concept:premise".into(), node_type: GraphNodeType::Concept,
        name: "前提".into(), occurrences: vec!["1.1".into()], source_lid: None,
    });
    Book::new(base, "先核对前提，再下结论")
}

fn save_evidence(name: &str, value: Value) {
    if let Ok(directory) = std::env::var("TO5_DIR") {
        std::fs::write(PathBuf::from(directory).join(name), serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    }
}

fn delivered_results(adapter: &RequestPlanRecordingAdapter) -> Vec<Value> {
    adapter.seen_plans.borrow().iter().skip(1).map(|plan| {
        let message = plan.input.iter().rev().find(|m| m.role == Role::Tool).unwrap();
        let envelope: Value = serde_json::from_str(message.content.as_ref().unwrap()).unwrap();
        assert_eq!(envelope["status"], "ok");
        envelope["model_body"].clone()
    }).collect()
}

#[test]
fn to5_reader_examples_persist_notes_and_exact_or_whole_highlights() {
    let registry = resident_tool_registry();
    let mut cases = vec![("note", "reader.note", examples("reader.note")[0].clone(), "把‘先核对前提再下结论’记为这一段的笔记", "note")];
    for (i, input) in examples("reader.highlight").into_iter().enumerate() {
        cases.push((if i == 0 { "quote" } else { "passage" }, "reader.highlight", input,
            if i == 0 { "请高亮这一段的‘先核对前提’" } else { "请高亮这一整段" }, "highlight"));
    }
    let mut evidence = Vec::new();
    for (label, name, input, question, mem_type) in cases {
        let b = source_book();
        let path = tmp(&format!("to5-reader-{label}"));
        let mut store = MemoryStore::open(&path).unwrap();
        let mut reader = Reader::new(&b, DEFAULT_RADIUS);
        registry.registration(name).unwrap().validate_arguments(&input.to_string()).unwrap();
        let mut turns = vec![turn_calls(vec![call("save", name, &input.to_string())])];
        if mem_type == "note" {
            turns.push(turn_calls(vec![call("same-note", name, &input.to_string())]));
        }
        turns.push(turn_final("已保存。"));
        let adapter = RequestPlanRecordingAdapter::new(turns, vec![]);
        let out = run(&b, &mut store, &mut reader, &adapter, &mut new_session(), question, "t0", OuterConfig::default()).unwrap();
        assert!(!out.incomplete, "{label}: {:?}", out.trace);
        assert_eq!(out.effects.len(), 1);
        let results = delivered_results(&adapter);
        assert_eq!(results[0]["ok"], true);
        let id_field = if mem_type == "note" { "note_id" } else { "highlight_id" };
        if mem_type == "note" {
            assert_eq!(results[0]["status"], "CREATED");
            assert_eq!(results[1]["status"], "EXISTING");
            assert_eq!(results[0][id_field], results[1][id_field]);
        }
        drop(store);
        let reopened = MemoryStore::open(&path).unwrap();
        let records = reopened.recall(&RecallQuery::default());
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(results[0][id_field], record.mem_id);
        assert_eq!(record.mem_type, mem_type);
        assert_eq!(record.layer, "session");
        assert_eq!(record.anchor.lid.as_deref(), Some("1.1"));
        assert_eq!(record.citations[0].lid, "1.1");
        if mem_type == "note" {
            assert_eq!(record.content, input["text"].as_str().unwrap());
            assert_eq!(serde_json::to_value(&record.note_placement).unwrap()["lid"], "1.1");
        } else if let Some(quote) = input["quote"].as_str() {
            assert_eq!(record.content, quote);
            assert_eq!(record.range.as_ref().unwrap().start, 0);
            assert_eq!(record.range.as_ref().unwrap().end, quote.encode_utf16().count() as u32);
        } else {
            assert_eq!(record.content, b.text("1.1", None).unwrap());
            assert!(record.range.is_none());
        }
        evidence.push(json!({"tool":name,"input":input,"question":question,"results":results,"records":records,"effects":out.effects}));
    }
    save_evidence("reader.json", json!(evidence));
}

#[test]
fn to5_memory_examples_save_only_their_public_record_types() {
    let b = source_book();
    let path = tmp("to5-memory");
    let mut store = MemoryStore::open(&path).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let reader_before = serde_json::to_value(reader.state()).unwrap();
    // Feed the existing query engine's PlanGate and support-assessment protocols.
    let adapter = FakeAdapter::new(vec![], [
        json!({"plan_gate":{"valid":true,"missing_requirements":[],"target_issues":[]},
            "candidate_fits":[{"target_index":0,"candidate_id":"concept:premise","fit":"direct_match","reason":"named premise"}],"probes":[]}),
        json!({"answer":"结论依赖前提，需先核对。",
            "assessments":[{"obligation_index":0,"verdict":"supported","citation_lids":["1.1"],"support_note":"原文要求先核对前提"}],
            "citations":[{"lid":"1.1","text":"先核对前提，再下结论","role":"support"}],"model_supplement":[]}),
    ].into_iter().map(|value| ParsedResponse {
        sufficient:true, answer:Some(value.to_string()), citations:vec![], model_supplement:vec![],
    }).collect());
    let question = "为什么要先核对前提？";
    let query = json!({"query":question,"intent":"definition","targets":["前提"],"obligations":[{"requirement":"解释核对前提的作用"}],"anchor_lid":"1.1"});
    let (answer, _) = dispatch("book.query", &query.to_string(), &b, &mut store, &mut reader, &adapter, "t0");
    assert!(tool_result_error_code(&answer).is_none(), "{answer}");
    assert!(answer.contains("结论依赖前提"));
    let mut calls = Vec::new();
    for input in examples("memory.save") {
        resident_tool_registry().registration("memory.save").unwrap().validate_arguments(&input.to_string()).unwrap();
        let (body, effect) = dispatch("memory.save", &input.to_string(), &b, &mut store, &mut reader, &adapter, "t1");
        assert!(effect.is_none());
        let record: Value = serde_json::from_str(&body).unwrap();
        assert!(record.get("error_code").is_none(), "{record}");
        assert_eq!(record["type"], input["type"]);
        assert_eq!(record["content"], input["content"]);
        assert_eq!(record["anchor"]["lid"], input["anchor_lid"]);
        assert_eq!(record["layer"], if input["type"] == "position" { "session" } else { "long_term" });
        if input["type"] == "qa" { assert_eq!(record["content"], question); }
        if input["type"] == "context" { assert_eq!(record["citations"], json!([])); }
        assert!(record.get("note_id").is_none());
        calls.push(json!({"tool":"memory.save","input":input,"record":record}));
    }
    assert_eq!(serde_json::to_value(reader.state()).unwrap(), reader_before);
    drop(store);
    let reopened = MemoryStore::open(&path).unwrap();
    let records = reopened.recall(&RecallQuery::default());
    assert_eq!(records.len(), 4);
    assert!(records.iter().all(|r| r.mem_type != "note" && r.range.is_none()));
    for call in &calls {
        let record = records.iter().find(|r| r.mem_id == call["record"]["mem_id"]).unwrap();
        assert_eq!(serde_json::to_value(record).unwrap(), call["record"]);
    }
    save_evidence("memory.json", json!({"source":b.text("1.1",None).unwrap(),"query":query,"answer":serde_json::from_str::<Value>(&answer).unwrap(),"context_user_message":"我有 Rust 基础，读这一段时请先给例子","calls":calls,"records":records,"reader_unchanged":true}));
}

#[test]
fn to5_explanation_only_keeps_memory_and_reader_unchanged() {
    let b = source_book();
    let path = tmp("to5-explain");
    let mut store = MemoryStore::open(&path).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let before = serde_json::to_value(reader.state()).unwrap();
    let revision = store.document_revision();
    let adapter = RequestPlanRecordingAdapter::new(vec![turn_final("这里要求先确认条件成立，再判断结论是否适用。")], vec![]);
    let out = run(&b, &mut store, &mut reader, &adapter, &mut new_session(), "只解释选中的原文‘先核对前提’，不保存笔记、高亮或记忆", "t0", OuterConfig::default()).unwrap();
    assert!(!out.incomplete);
    assert!(out.trace.is_empty() && out.effects.is_empty() && out.memory_updates.is_empty());
    assert_eq!(store.document_revision(), revision);
    assert_eq!(serde_json::to_value(reader.state()).unwrap(), before);
    let plans = adapter.seen_plans.borrow();
    assert!(plans[0].tools.iter().all(|s| !["reader.note", "reader.highlight", "memory.save"].contains(&s.name.as_str())));
    drop(store);
    assert!(MemoryStore::open(&path).unwrap().recall(&RecallQuery::default()).is_empty());
    save_evidence("explain.json", json!({"native":crate::native_chat_request_projection("test",&plans[0]).0,"react":crate::react_chat_request_projection("test",&plans[0]),"answer":out.answer,"effects":out.effects,"reader_before":before,"reader_after":reader.state(),"records":[]}));
}
