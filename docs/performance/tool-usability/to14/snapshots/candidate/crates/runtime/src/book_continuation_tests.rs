use super::*;
use crate::tool_result::{project_tool_result, ToolContinuation};
use serde_json::{json, Value};

fn continuation_book() -> Book {
    let pieces = [
        "heading\n".to_owned(),
        "甲😀needle\n".repeat(1000),
        "\n\n".into(),
        "乙needle\n".repeat(80),
    ];
    let source = pieces.concat();
    let left = pieces[0].encode_utf16().count();
    let mid = left + pieces[1].encode_utf16().count();
    let right = mid + 2;
    let end = source.encode_utf16().count();
    Book::new(
        base_schema::ReadOnlyBase {
            book_id: "to10".into(),
            lid_nodes: vec![
                base_schema::LidNode {
                    lid: "1".into(),
                    path: vec![1],
                    kind: base_schema::NodeKind::Chapter,
                    span: base_schema::Span { start: 0, end },
                    children: vec!["1.0".into(), "1.1".into(), "1.2".into()],
                },
                base_schema::LidNode {
                    lid: "1.0".into(),
                    path: vec![1, 0],
                    kind: base_schema::NodeKind::Paragraph,
                    span: base_schema::Span {
                        start: 0,
                        end: left,
                    },
                    children: vec![],
                },
                base_schema::LidNode {
                    lid: "1.1".into(),
                    path: vec![1, 1],
                    kind: base_schema::NodeKind::Paragraph,
                    span: base_schema::Span {
                        start: left,
                        end: mid,
                    },
                    children: vec![],
                },
                base_schema::LidNode {
                    lid: "1.2".into(),
                    path: vec![1, 2],
                    kind: base_schema::NodeKind::Paragraph,
                    span: base_schema::Span { start: right, end },
                    children: vec![],
                },
            ],
            graph_nodes: vec![],
            graph_edges: vec![],
        },
        &source,
    )
}

fn project(
    book: &Book,
    tool: &str,
    input: &Value,
    limit: usize,
) -> crate::tool_result::ToolResultDraft {
    resident_tool_registry()
        .registration(tool)
        .unwrap()
        .validate_arguments(&input.to_string())
        .unwrap();
    let id = book_tool_contracts::from_resident_alias(tool).unwrap();
    let (raw, _) =
        dispatch_resident_book_tool(id, input.clone(), book, &FakeAdapter::new(vec![], vec![]));
    assert!(
        serde_json::from_str::<Value>(&raw)
            .unwrap()
            .get("error_code")
            .is_none(),
        "{raw}"
    );
    project_tool_result(
        tool,
        &input.to_string(),
        &raw,
        crate::tool_registry::ToolOutputPolicy {
            result_policy: crate::tool_registry::ToolResultPolicy::EvidenceProjection,
            max_model_body_bytes: limit,
        },
        limit,
        book,
    )
}

fn next(draft: &crate::tool_result::ToolResultDraft) -> Option<Value> {
    match &draft.continuation {
        Some(
            ToolContinuation::NextCall { arguments, .. }
            | ToolContinuation::ToolCursor { arguments, .. },
        ) => Some(arguments.clone()),
        None => None,
        other => panic!("continuation must be executable: {other:?}"),
    }
}

#[test]
fn to10_capture_baseline() {
    let book = continuation_book();
    let samples: Vec<_> = [
        ("single_leaf", "book.text", json!({"lid":"1.1"}), 1800),
        ("cross_leaf", "book.text", json!({"lid":"1"}), 1800),
        ("native_search", "book.search_text", json!({"query":"needle","page_size":1}), 16384),
        ("budget_search", "book.search_text", json!({"query":"needle","page_size":50}), 1800),
    ].into_iter().map(|(label,tool,input,limit)| {
        let draft=project(&book,tool,&input,limit);
        let mut ledger=TurnEvidenceLedger::default();
        observe_tool_evidence(&mut ledger,tool,&draft.evidence_arguments,&draft.model_body_json(),&book);
        json!({"label":label,"input":input,"limit":limit,"body":draft.model_body,"truncated":draft.truncated,"continuation":draft.continuation,"evidence":ledger.evidence_ranges()})
    }).collect();
    if let Ok(path) = std::env::var("TO10_SNAPSHOT") {
        std::fs::write(path, serde_json::to_vec_pretty(&samples).unwrap()).unwrap();
    }
}

#[test]
fn to10_text_reassembles_single_leaf_and_parent_without_skipping_headings_or_gaps() {
    let book = continuation_book();
    for lid in ["1.1", "1"] {
        let expected = book.text(lid, None).unwrap();
        let mut input = json!({"lid":lid});
        let mut restored = String::new();
        let mut ledger = TurnEvidenceLedger::default();
        for _ in 0..100 {
            let draft = project(&book, "book.text", &input, 1800);
            let text = draft.model_body["text"].as_str().expect("source text");
            assert!(!text.is_empty());
            assert!(draft.model_body_json().len() <= 1800);
            restored.push_str(text);
            assert!(
                expected.starts_with(&restored),
                "pages must be an exact prefix"
            );
            observe_tool_evidence(
                &mut ledger,
                "book.text",
                &draft.evidence_arguments,
                &draft.model_body_json(),
                &book,
            );
            let entries = ledger.evidence.len();
            observe_tool_evidence(
                &mut ledger,
                "book.text",
                &draft.evidence_arguments,
                &draft.model_body_json(),
                &book,
            );
            assert_eq!(
                ledger.evidence.len(),
                entries,
                "replaying a page must not duplicate evidence"
            );
            let consumed = book
                .base
                .lid_nodes
                .iter()
                .find(|n| n.lid == lid)
                .unwrap()
                .span
                .start
                + restored.encode_utf16().count();
            assert!(ledger
                .observed_intervals(&book)
                .iter()
                .all(|(_, end)| *end <= consumed));
            if lid == "1.1" {
                assert_eq!(
                    ledger.observed_intervals(&book),
                    vec![(8, consumed)],
                    "repeated text must retain its actual coordinates"
                );
            }
            if let Some(args) = next(&draft) {
                assert_eq!(args["lid"], lid);
                input = args;
            } else {
                break;
            }
        }
        assert_eq!(restored, expected);
        assert!(!ledger.teaching_source_ranges().is_empty());
        if lid == "1" {
            ledger
                .prepare_present(&book, &json!({"quote":"needle\n\n\n乙needle"}).to_string())
                .expect("the fully delivered cross-leaf passage must be citable");
        }
    }
}

#[test]
fn to10_search_continuations_preserve_request_and_reassemble_ordered_hits() {
    let book = continuation_book();
    for order in ["document", "reverse_document"] {
        for limit in [1800, 16384] {
            let original = json!({"query":"needle","match_mode":"exact","scope":{"within_lid":"1.2"},"order":order,"page_size":50});
            let mut input = original.clone();
            let mut ordinals = Vec::new();
            for _ in 0..100 {
                let draft = project(&book, "book.search_text", &input, limit);
                let hits = draft.model_body["occurrences"].as_array().unwrap();
                assert!(!hits.is_empty());
                ordinals.extend(hits.iter().map(|h| h["ordinal"].as_u64().unwrap()));
                if let Some(args) = next(&draft) {
                    for key in ["query", "match_mode", "scope", "order", "page_size"] {
                        assert_eq!(args[key], original[key], "{key}");
                    }
                    input = args;
                } else {
                    break;
                }
            }
            let mut expected: Vec<_> = (1..=80).collect();
            if order == "reverse_document" {
                expected.reverse();
            }
            assert_eq!(ordinals, expected);
        }
    }
}

struct ContinuationAdapter {
    plans: RefCell<Vec<crate::AgentRequestPlan>>,
}

impl ModelAdapter for ContinuationAdapter {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        panic!("deterministic reads only")
    }
    fn chat(&self, plan: &crate::AgentRequestPlan) -> Result<crate::AssistantTurn, AdapterError> {
        let step = self.plans.borrow().len();
        self.plans.borrow_mut().push(plan.clone());
        if step == 0 {
            return Ok(turn_calls(vec![call(
                "page-0",
                "book.text",
                r#"{"lid":"1.1"}"#,
            )]));
        }
        let message = plan
            .input
            .iter()
            .rev()
            .find(|m| m.role == Role::Tool)
            .unwrap();
        let envelope: Value = serde_json::from_str(message.content.as_ref().unwrap()).unwrap();
        assert_ne!(envelope["status"], "error", "{envelope}");
        Ok(if envelope["continuation"].is_object() {
            let next = &envelope["continuation"];
            turn_calls(vec![call(
                &format!("page-{step}"),
                next["tool"].as_str().unwrap(),
                &next["arguments"].to_string(),
            )])
        } else {
            turn_final("The complete requested passage has been read.")
        })
    }
}

#[test]
fn to10_resident_next_sampling_executes_continuations_and_receipts_only_cover_delivered_text() {
    let source = "UNIQUE_HEAD".to_owned() + &"甲😀\"\\\n".repeat(4000) + "UNIQUE_TAIL";
    let book = Book::new(
        base_schema::ReadOnlyBase {
            book_id: "to10-resident".into(),
            lid_nodes: vec![base_schema::LidNode {
                lid: "1.1".into(),
                path: vec![1, 1],
                kind: base_schema::NodeKind::Paragraph,
                span: base_schema::Span {
                    start: 0,
                    end: source.encode_utf16().count(),
                },
                children: vec![],
            }],
            graph_nodes: vec![],
            graph_edges: vec![],
        },
        &source,
    );
    let adapter = ContinuationAdapter {
        plans: RefCell::new(vec![]),
    };
    let out = run(
        &book,
        &mut MemoryStore::open(tmp("to10-loop")).unwrap(),
        &mut Reader::new(&book, DEFAULT_RADIUS),
        &adapter,
        &mut new_session(),
        "Read the complete source at LID 1.1.",
        "t0",
        OuterConfig::default(),
    )
    .unwrap();
    assert!(!out.incomplete, "{:?}", out.warning);
    let plans = adapter.plans.borrow();
    let mut joined = String::new();
    let mut samples = Vec::new();
    for plan in plans.iter().skip(1) {
        let message = plan
            .input
            .iter()
            .rev()
            .find(|m| m.role == Role::Tool)
            .unwrap();
        let envelope: Value = serde_json::from_str(message.content.as_ref().unwrap()).unwrap();
        let body = &envelope["model_body"];
        assert_eq!(body["source_range"]["start"], joined.encode_utf16().count());
        joined.push_str(body["text"].as_str().unwrap());
        assert_eq!(body["source_range"]["end"], joined.encode_utf16().count());
        let accepted: Vec<EvidenceRange> =
            serde_json::from_value(envelope["receipt"]["accepted_evidence"].clone()).unwrap();
        let intervals: Vec<_> = accepted
            .iter()
            .flat_map(|range| evidence_intervals(&book, range).unwrap())
            .collect();
        assert_eq!(
            intervals,
            vec![(
                body["source_range"]["start"].as_u64().unwrap() as usize,
                joined.encode_utf16().count()
            )]
        );
        for wire in [
            crate::native_chat_request_projection("test", plan).0,
            crate::react_chat_request_projection("test", plan),
        ] {
            assert!(wire.to_string().contains("start_utf16"));
        }
        samples.push(envelope);
    }
    assert_eq!(joined, source);
    assert!(samples.len() > 2);
    if let Ok(path) = std::env::var("TO10_RESIDENT") {
        std::fs::write(path, serde_json::to_vec_pretty(&samples).unwrap()).unwrap();
    }
}

#[test]
fn to10_native_text_pages_and_exhausted_budget_have_distinct_signals() {
    let book = continuation_book();
    let input = json!({"lid":"1.1","start_utf16":8});
    let page = project(&book, "book.text", &input, 48000);
    assert!(!page.truncated);
    assert!(page.continuation.is_some());
    assert_eq!(
        page.model_body["text"].as_str().unwrap().chars().count(),
        8000
    );
    let empty = project(&book, "book.text", &input, 0);
    assert!(empty.truncated);
    assert!(empty.model_body.is_null());
    assert_eq!(next(&empty).unwrap(), input);
    let mut ledger = TurnEvidenceLedger::default();
    observe_tool_evidence(
        &mut ledger,
        "book.text",
        &empty.evidence_arguments,
        &empty.model_body_json(),
        &book,
    );
    assert!(ledger.evidence.is_empty());
}
