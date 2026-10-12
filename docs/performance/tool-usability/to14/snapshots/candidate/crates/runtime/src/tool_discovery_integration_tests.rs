use super::*;

#[test]
fn tool_search_to7_delivers_recovery_and_never_executes_reader_or_memory_writes() {
    for (label, question, authorized) in [
        ("read-only", "Explain this passage", false),
        ("note-request", "记一条笔记", true),
    ] {
        let b = book();
        let mut store = MemoryStore::open(tmp(&format!("to7-discovery-{label}"))).unwrap();
        let fake = RequestPlanRecordingAdapter::new(
            vec![
                turn_calls(vec![
                    call(
                        "reader",
                        "tool.search",
                        r#"{"task":"Save a note on this passage","required_capabilities":["reader_write"],"scope":"passage","operation":"mutate_reader","effect_mode":"reader_mutation_explicitly_requested","max_results":1}"#,
                    ),
                    call(
                        "memory",
                        "tool.search",
                        r#"{"task":"Save reusable reading context","required_capabilities":["memory_write"],"scope":"passage","operation":"explain","effect_mode":"read_only","max_results":1}"#,
                    ),
                ]),
                turn_final("Discovery complete."),
            ],
            vec![],
        );
        let mut reader = Reader::new(&b, DEFAULT_RADIUS);
        let before = serde_json::to_value(reader.state()).unwrap();
        let before_records = serde_json::to_value(store.recall(&RecallQuery::default())).unwrap();
        let mut messages = new_session();
        let out = run(
            &b,
            &mut store,
            &mut reader,
            &fake,
            &mut messages,
            question,
            "t0",
            OuterConfig::default(),
        )
        .unwrap();
        assert!(out.effects.is_empty());
        assert!(out.memory_updates.is_empty());
        assert_eq!(serde_json::to_value(reader.state()).unwrap(), before);
        assert_eq!(
            serde_json::to_value(store.recall(&RecallQuery::default())).unwrap(),
            before_records
        );
        assert!(out.trace.iter().all(|entry| entry.tool == "tool.search"));
        let plans = fake.seen_plans.borrow();
        let results = plans[1]
            .input
            .iter()
            .filter(|m| m.role == Role::Tool)
            .map(|m| {
                let envelope: serde_json::Value = serde_json::from_str(m.content.as_ref().unwrap()).unwrap();
                assert_eq!(envelope["truncated"], false);
                envelope["model_body"].clone()
            })
            .collect::<Vec<_>>();
        assert_eq!(results.len(), 2);
        let reader_result = &results[0];
        if authorized {
            assert_eq!(reader_result["matches"][0]["name"], "reader.note");
            assert!(reader_result["matches"][0]["use_when"].is_array());
            assert_eq!(reader_result["unmet_capabilities"], serde_json::json!([]));
        } else {
            assert_eq!(
                reader_result["blocked"][0]["reason"],
                "explicit_effect_intent_required"
            );
            assert_eq!(
                reader_result["blocked"][0]["next_searches"][0]["required_capabilities"],
                serde_json::json!(["reader_read"])
            );
        }
        if authorized {
            assert_eq!(results[1]["matches"][0]["name"], "memory.save");
            assert!(results[1]["matches"][0]["avoid_when"].is_array());
        } else {
            // An ordinary request has not observed or located this passage.
            // Reader anchor promotion belongs to the explicit intent seed.
            assert_eq!(results[1]["blocked"][0]["reason"], "locator_required");
        }
        for result in &results {
            for field in ["task_need", "request_audit", "capability_plan"] {
                assert!(result.get(field).is_none());
            }
        }
        let native = crate::native_chat_request_projection("test", &plans[1]).0;
        let react = crate::react_chat_request_projection("test", &plans[1]);
        for projected in [native.to_string(), react.to_string()] {
            assert!(projected.contains("use_when"));
            assert!(projected.contains("unmet_capabilities"));
            if !authorized {
                assert!(projected.contains("explicit_effect_intent_required"));
            }
        }
        if let Ok(directory) = std::env::var("TO7_REQUESTS") {
            std::fs::write(std::path::Path::new(&directory).join(format!("{label}.json")),
                serde_json::to_vec_pretty(&serde_json::json!({
                    "question":question,"results":results,"native":native,"react":react,
                    "active_context":plans[1].active_context,
                    "effects":out.effects,"reader_before":before,"reader_after":reader.state(),
                    "memory_before":before_records,"memory_after":store.recall(&RecallQuery::default())
                })).unwrap()).unwrap();
        }
    }
}
