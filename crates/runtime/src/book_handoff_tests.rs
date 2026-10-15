use super::*;
use serde_json::{json, Value};

fn handoff_book() -> Book {
    Book::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../book-tool-contracts/fixtures/example-book"
    ))
    .unwrap()
}

fn handoff_cases() -> Vec<(&'static str, BookToolId, Value)> {
    vec![
        (
            "single",
            BookToolId::SearchText,
            json!({"query":"threshold","match_mode":"exact","order":"document","page_size":10}),
        ),
        (
            "range",
            BookToolId::SearchText,
            json!({"query":"10.\nThe beta","match_mode":"exact","order":"document","page_size":10}),
        ),
        (
            "empty",
            BookToolId::SearchText,
            json!({"query":"missing-term","match_mode":"exact","order":"document","page_size":10}),
        ),
        (
            "concept",
            BookToolId::Concept,
            json!({"query":"threshold","limit":5}),
        ),
        (
            "context",
            BookToolId::Context,
            json!({"lid":"1.1","granularity":"far","k":10}),
        ),
    ]
}

#[test]
fn to8_capture_candidate_results() {
    let book = handoff_book();
    let adapter = FakeAdapter::new(vec![], vec![]);
    let mut samples = Vec::new();
    for (label, id, input) in handoff_cases() {
        let (body, _) = dispatch_resident_book_tool(id, input.clone(), &book, &adapter);
        let result: Value = serde_json::from_str(&body).unwrap();
        assert!(result.get("error_code").is_none(), "{label}: {result}");
        if label == "range" {
            assert_eq!(result["occurrences"][0]["start_lid"], "1.1");
            assert_eq!(result["occurrences"][0]["end_lid"], "1.2");
        }
        samples.push(json!({"label":label,"tool":book_tool_contracts::contract_for(id).aliases.resident,"input":input,"result":result}));
    }
    if let Ok(path) = std::env::var("TO8_CANDIDATES") {
        std::fs::write(path, serde_json::to_vec_pretty(&samples).unwrap()).unwrap();
    }
}

fn candidate_calls(id: BookToolId, result: &Value) -> Vec<&Value> {
    let key = match id {
        BookToolId::SearchText => "occurrences",
        BookToolId::Concept => "candidates",
        BookToolId::Context => "items",
        _ => unreachable!(),
    };
    result[key]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|candidate| {
            let calls = candidate["next_calls"]
                .as_array()
                .expect("candidate read calls");
            let expected: Vec<Value> = match id {
                BookToolId::SearchText => vec![if candidate["start_lid"] == candidate["end_lid"] {
                    json!({"lid":candidate["start_lid"]})
                } else {
                    json!({"lid":candidate["start_lid"],"end_lid":candidate["end_lid"]})
                }],
                BookToolId::Concept => candidate["occurrences"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|lid| json!({"lid":lid}))
                    .collect(),
                BookToolId::Context => vec![json!({"lid":candidate["lid"]})],
                _ => unreachable!(),
            };
            assert_eq!(
                calls
                    .iter()
                    .map(|c| c["arguments"].clone())
                    .collect::<Vec<_>>(),
                expected
            );
            calls.iter()
        })
        .collect()
}

#[test]
fn to8_returned_calls_read_canonical_ranges_without_promoting_candidates() {
    let book = handoff_book();
    let adapter = FakeAdapter::new(vec![], vec![]);
    let registry = resident_tool_registry();
    let mut snapshots = Vec::new();
    for (label, id, input) in handoff_cases() {
        let tool = book_tool_contracts::contract_for(id)
            .aliases
            .resident
            .unwrap();
        let (raw, _) = dispatch_resident_book_tool(id, input.clone(), &book, &adapter);
        let draft = crate::tool_result::project_tool_result(
            tool,
            &input.to_string(),
            &raw,
            registry.registration(tool).unwrap().output_policy,
            48 * 1024,
            &book,
        );
        assert!(!draft.truncated, "{label}");
        let body = draft.model_body_json();
        let mut locators = TurnLocatorLedger::default();
        let mut evidence = TurnEvidenceLedger::default();
        locators.observe_tool_result(tool, &body, &book);
        observe_tool_evidence(&mut evidence, tool, &input.to_string(), &body, &book);
        assert!(evidence.teaching_source_ranges().is_empty());
        assert!(evidence
            .evidence
            .iter()
            .all(|e| e.claim_kind == EvidenceClaimKind::LiteralOccurrence));
        if id != BookToolId::SearchText {
            assert!(evidence.evidence.is_empty());
        }
        let calls = candidate_calls(id, &draft.model_body);
        if label == "empty" {
            assert!(calls.is_empty());
        }
        let mut reads = Vec::new();
        for next in calls {
            assert_eq!(next["tool"], "book.text");
            let args = next["arguments"].clone();
            let BookToolInput::Text(parsed) =
                validate_input(BookToolId::Text, args.clone()).unwrap()
            else {
                panic!()
            };
            authorize_book_text(&args.to_string(), &locators).unwrap();
            let (text, _) =
                dispatch_resident_book_tool(BookToolId::Text, args.clone(), &book, &adapter);
            let result: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(
                result["text"],
                book.text(&parsed.lid, parsed.end_lid.as_deref()).unwrap()
            );
            let mut read_evidence = TurnEvidenceLedger::default();
            observe_tool_evidence(
                &mut read_evidence,
                "book.text",
                &args.to_string(),
                &text,
                &book,
            );
            assert_eq!(
                read_evidence.teaching_source_ranges(),
                vec![EvidenceRange {
                    start_lid: parsed.lid.clone(),
                    end_lid: parsed.end_lid.unwrap_or(parsed.lid),
                    ranges: vec![],
                }]
            );
            reads.push(json!({"call":next,"result":result,"accepted":read_evidence.teaching_source_ranges()}));
        }
        snapshots.push(json!({"label":label,"delivered":draft.model_body,"truncated":draft.truncated,"candidate_source_evidence":evidence.teaching_source_ranges(),"reads":reads}));
    }
    if let Ok(path) = std::env::var("TO8_DELIVERED") {
        std::fs::write(path, serde_json::to_vec_pretty(&snapshots).unwrap()).unwrap();
    }
}

struct HandoffAdapter {
    tool: &'static str,
    input: Value,
    selected: String,
    plans: RefCell<Vec<crate::AgentRequestPlan>>,
}

impl ModelAdapter for HandoffAdapter {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        panic!("deterministic reads only")
    }
    fn chat(&self, plan: &crate::AgentRequestPlan) -> Result<crate::AssistantTurn, AdapterError> {
        let step = self.plans.borrow().len();
        self.plans.borrow_mut().push(plan.clone());
        Ok(match step {
            0 => turn_calls(vec![call("locate", self.tool, &self.input.to_string())]),
            1 => {
                let message = plan
                    .input
                    .iter()
                    .rev()
                    .find(|m| m.role == Role::Tool)
                    .unwrap();
                let result: Value =
                    serde_json::from_str(message.content.as_ref().unwrap()).unwrap();
                assert_eq!(result["truncated"], false);
                let next = result["model_body"].pointer(&self.selected).unwrap();
                turn_calls(vec![call(
                    "read",
                    next["tool"].as_str().unwrap(),
                    &next["arguments"].to_string(),
                )])
            }
            _ => turn_final("The selected source has been read."),
        })
    }
}

#[test]
fn to8_resident_copies_delivered_calls_in_the_next_sampling() {
    let book = handoff_book();
    let mut snapshots = Vec::new();
    for (label, id, input) in handoff_cases()
        .into_iter()
        .filter(|(label, _, _)| *label != "empty")
    {
        let tool = book_tool_contracts::contract_for(id)
            .aliases
            .resident
            .unwrap();
        let selected = match id {
            BookToolId::Concept => "/candidates/1/next_calls/0",
            BookToolId::Context => "/items/1/next_calls/0",
            _ => "/occurrences/0/next_calls/0",
        };
        let adapter = HandoffAdapter {
            tool,
            input,
            selected: selected.into(),
            plans: RefCell::new(vec![]),
        };
        let mut messages = new_session();
        let out = run(
            &book,
            &mut MemoryStore::open(tmp(&format!("to8-{label}"))).unwrap(),
            &mut Reader::new(&book, DEFAULT_RADIUS),
            &adapter,
            &mut messages,
            "Locate the threshold near LID 1.1 and read the chosen source.",
            "t0",
            OuterConfig::default(),
        )
        .unwrap();
        assert!(!out.incomplete, "{label}: {:?}", out.warning);
        assert_eq!(
            out.trace
                .iter()
                .map(|s| s.tool.as_str())
                .collect::<Vec<_>>(),
            vec![tool, "book.text"]
        );
        let plans = adapter.plans.borrow();
        assert_eq!(plans.len(), 3);
        let body = |index: usize| -> Value {
            serde_json::from_str(
                plans[index]
                    .input
                    .iter()
                    .rev()
                    .find(|m| m.role == Role::Tool)
                    .unwrap()
                    .content
                    .as_ref()
                    .unwrap(),
            )
            .unwrap()
        };
        let located = body(1);
        let read = body(2);
        let next = located["model_body"].pointer(selected).unwrap();
        assert_eq!(read["receipt"]["locator_args"], next["arguments"]);
        assert_eq!(
            read["receipt"]["accepted_evidence"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            read["model_body"]["text"],
            book.text(
                next["arguments"]["lid"].as_str().unwrap(),
                next["arguments"]["end_lid"].as_str()
            )
            .unwrap()
        );
        for wire in [
            crate::native_chat_request_projection("test", &plans[1]).0,
            crate::react_chat_request_projection("test", &plans[1]),
        ] {
            assert!(wire.to_string().contains("next_calls"));
        }
        snapshots.push(json!({"label":label,"located":located,"read":read,"active_context":plans[1].active_context}));
    }
    if let Ok(path) = std::env::var("TO8_REQUESTS") {
        std::fs::write(path, serde_json::to_vec_pretty(&snapshots).unwrap()).unwrap();
    }
}

#[test]
fn to8_budgeted_concept_calls_keep_their_locator_provenance() {
    let mut base = book_leaves(300).base;
    let mut concept = handoff_book().base.graph_nodes[0].clone();
    concept.occurrences = base
        .lid_nodes
        .iter()
        .skip(1)
        .map(|n| n.lid.clone())
        .collect();
    base.graph_nodes.push(concept);
    let book = Book::new(base, &"X".repeat(3000));
    let raw =
        serde_json::to_string(&book.concept_candidates("alpha", None, Some(1)).unwrap()).unwrap();
    let draft = crate::tool_result::project_tool_result(
        "book.concept",
        r#"{"query":"alpha","limit":1}"#,
        &raw,
        resident_tool_registry()
            .registration("book.concept")
            .unwrap()
            .output_policy,
        48 * 1024,
        &book,
    );
    assert!(draft.truncated);
    let mut locators = TurnLocatorLedger::default();
    locators.observe_tool_result("book.concept", &draft.model_body_json(), &book);
    let calls: Vec<_> = draft.model_body["candidates"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|c| c["next_calls"].as_array().into_iter().flatten())
        .collect();
    assert!(
        !calls.is_empty(),
        "the bounded result must still offer retained locations"
    );
    for next in &calls {
        assert!(
            authorize_book_text(&next["arguments"].to_string(), &locators).is_ok(),
            "orphaned call: {next}"
        );
    }
    if let Ok(path) = std::env::var("TO8_BUDGET") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({
            "occurrences":300,"raw_bytes":raw.len(),"delivered_bytes":draft.model_body_json().len(),
            "returned_calls":calls.len(),"truncated":draft.truncated,"delivered":draft.model_body,"continuation":draft.continuation,
        })).unwrap()).unwrap();
    }
}

#[test]
fn to9_context_selection_coverage_survives_resident_delivery() {
    let book = handoff_book().with_book_structure(None);
    let registry = resident_tool_registry();
    let adapter = HandoffAdapter {
        tool: "book.context",
        input: json!({"lid":"1.1","granularity":"far","k":1}),
        selected: "/items/0/next_calls/0".into(),
        plans: RefCell::new(vec![]),
    };
    let mut samples = Vec::new();
    for k in [0, 1, 2, 10] {
        let input = json!({"lid":"1.1","granularity":"far","k":k});
        let (raw, _) = dispatch_resident_book_tool(BookToolId::Context, input.clone(), &book, &adapter);
        let draft = crate::tool_result::project_tool_result(
            "book.context", &input.to_string(), &raw,
            registry.registration("book.context").unwrap().output_policy, 48 * 1024, &book,
        );
        assert!(!draft.truncated, "delivery is complete even when k trims candidates");
        assert_eq!(draft.model_body["total_candidates"], 2);
        assert_eq!(draft.model_body["returned_count"], k.min(2));
        assert_eq!(draft.model_body["truncated"], k < 2);
        samples.push(json!({"input":input,"model_body":draft.model_body,"delivery_truncated":draft.truncated}));
    }
    let (raw, _) = dispatch_resident_book_tool(BookToolId::Structure, json!({}), &book, &adapter);
    let structure = crate::tool_result::project_tool_result(
        "book.structure", "{}", &raw,
        registry.registration("book.structure").unwrap().output_policy, 48 * 1024, &book,
    );
    assert_eq!(structure.model_body["available"], false);
    assert_eq!(structure.model_body["warning"], "book_structure.json not attached");
    let mut messages = new_session();
    let out = run(
        &book, &mut MemoryStore::open(tmp("to9-context")).unwrap(),
        &mut Reader::new(&book, DEFAULT_RADIUS), &adapter, &mut messages,
        "Locate context near LID 1.1 and read the chosen source.", "t0", OuterConfig::default(),
    ).unwrap();
    assert!(!out.incomplete, "{:?}", out.warning);
    let plans = adapter.plans.borrow();
    assert_eq!(plans.len(), 3);
    let delivered: Value = serde_json::from_str(plans[1].input.iter().rev()
        .find(|m| m.role == Role::Tool).unwrap().content.as_ref().unwrap()).unwrap();
    assert_eq!(delivered["truncated"], false);
    assert_eq!(delivered["model_body"]["total_candidates"], 2);
    assert_eq!(delivered["model_body"]["returned_count"], 1);
    assert_eq!(delivered["model_body"]["truncated"], true);
    let receipt: crate::tool_result::HistoricalToolReceipt =
        serde_json::from_value(delivered["receipt"].clone()).unwrap();
    assert!(receipt.accepted_evidence.is_empty());
    for wire in [crate::native_chat_request_projection("test", &plans[1]).0,
        crate::react_chat_request_projection("test", &plans[1])] {
        assert!(wire.to_string().contains("total_candidates"));
    }
    if let Ok(path) = std::env::var("TO9_RESIDENT") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({
            "samples":samples,"structure":structure.model_body,"next_sampling":delivered,
        })).unwrap()).unwrap();
    }
}

struct BudgetHandoffAdapter {
    plans: RefCell<Vec<crate::AgentRequestPlan>>,
}

impl ModelAdapter for BudgetHandoffAdapter {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        panic!("structured recovery must not rerun semantic inference")
    }
    fn chat(&self, plan: &crate::AgentRequestPlan) -> Result<crate::AssistantTurn, AdapterError> {
        let step = self.plans.borrow().len();
        self.plans.borrow_mut().push(plan.clone());
        Ok(match step {
            0 => turn_calls(vec![call(
                "locate",
                "book.concept",
                r#"{"query":"alpha","limit":1}"#,
            )]),
            1 => {
                let delivered: Value = serde_json::from_str(
                    plan.input
                        .iter()
                        .rev()
                        .find(|m| m.role == Role::Tool)
                        .unwrap()
                        .content
                        .as_ref()
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(delivered["truncated"], true);
                assert!(delivered["receipt"]["accepted_evidence"]
                    .as_array()
                    .is_none_or(Vec::is_empty));
                assert_eq!(
                    delivered["model_body"]["delivery"]["collections"]["/candidates/0/occurrences"],
                    json!({"total":300,"delivered":300})
                );
                let next = &delivered["continuation"];
                assert_eq!(next["kind"], "next_call");
                turn_calls(vec![call(
                    "read",
                    next["tool"].as_str().unwrap(),
                    &next["arguments"].to_string(),
                )])
            }
            _ => turn_final("The chosen occurrence has been read."),
        })
    }
}

#[test]
fn to11_concept_preserves_all_300_locations_and_next_sampling_executes_recovery() {
    let mut base = book_leaves(300).base;
    let mut concept = handoff_book().base.graph_nodes[0].clone();
    concept.occurrences = base
        .lid_nodes
        .iter()
        .skip(1)
        .map(|n| n.lid.clone())
        .collect();
    base.graph_nodes.push(concept);
    let book = Book::new(base, &"X".repeat(3000));
    let adapter = BudgetHandoffAdapter {
        plans: RefCell::new(vec![]),
    };
    let mut messages = new_session();
    let out = run(
        &book,
        &mut MemoryStore::open(tmp("to11-concept")).unwrap(),
        &mut Reader::new(&book, DEFAULT_RADIUS),
        &adapter,
        &mut messages,
        "Locate alpha and read a chosen occurrence.",
        "t0",
        OuterConfig::default(),
    )
    .unwrap();
    assert!(!out.incomplete, "{:?}", out.warning);
    assert_eq!(
        out.trace
            .iter()
            .map(|t| t.tool.as_str())
            .collect::<Vec<_>>(),
        vec!["book.concept", "book.text"]
    );
    let plans = adapter.plans.borrow();
    assert_eq!(plans.len(), 3);
    let envelope = |index: usize| -> Value {
        serde_json::from_str(
            plans[index]
                .input
                .iter()
                .rev()
                .find(|m| m.role == Role::Tool)
                .unwrap()
                .content
                .as_ref()
                .unwrap(),
        )
        .unwrap()
    };
    let located = envelope(1);
    let read = envelope(2);
    assert_eq!(located["model_body"]["returned_count"], 1);
    assert_eq!(located["model_body"]["truncated"], false);
    assert_eq!(
        located["model_body"]["candidates"][0]["occurrences"]
            .as_array()
            .unwrap()
            .len(),
        300
    );
    assert!(located["model_body"].to_string().len() <= 16 * 1024);
    assert_eq!(
        read["receipt"]["locator_args"],
        located["continuation"]["arguments"]
    );
    assert_eq!(read["model_body"]["text"], "XXXXXXXXXX");
    assert_eq!(
        read["receipt"]["accepted_evidence"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let mut locators = TurnLocatorLedger::default();
    locators.observe_tool_result("book.concept", &located["model_body"].to_string(), &book);
    for lid in located["model_body"]["candidates"][0]["occurrences"]
        .as_array()
        .unwrap()
    {
        authorize_book_text(&json!({"lid":lid}).to_string(), &locators).unwrap();
    }
    for wire in [
        crate::native_chat_request_projection("test", &plans[1]).0,
        crate::react_chat_request_projection("test", &plans[1]),
    ] {
        assert!(wire.to_string().contains("/candidates/0/occurrences"));
        assert!(wire.to_string().contains("next_call"));
    }
    if let Ok(path) = std::env::var("TO11_RESIDENT") {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&json!({"located":located,"read":read})).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn to11_omitted_query_quotes_authorize_reads_but_never_source_claims() {
    let mut base = book_leaves(2).base;
    for node in &mut base.lid_nodes {
        node.span.start *= 100;
        node.span.end *= 100;
    }
    let book = Book::new(base, &format!("{}{}", "A".repeat(1000), "B".repeat(1000)));
    let raw = json!({"status":"partial","answer":null,"citations":[
        {"lid":"1.1","text":book.text("1.1",None).unwrap(),"role":"support"},
        {"lid":"1.2","text":book.text("1.2",None).unwrap(),"role":"caveat"}],
        "bindings":[],"support":[],"model_supplement":[]});
    let mut policy = resident_tool_registry()
        .registration("book.query")
        .unwrap()
        .output_policy;
    policy.max_model_body_bytes = 512;
    let draft = crate::tool_result::project_tool_result(
        "book.query",
        "{}",
        &raw.to_string(),
        policy,
        512,
        &book,
    );
    assert_eq!(
        draft.model_body["delivery"]["read_lids"],
        json!(["1.1", "1.2"])
    );
    let mut locators = TurnLocatorLedger::default();
    let mut evidence = TurnEvidenceLedger::default();
    locators.observe_tool_result("book.query", &draft.model_body_json(), &book);
    observe_tool_evidence(
        &mut evidence,
        "book.query",
        "{}",
        &draft.model_body_json(),
        &book,
    );
    assert!(evidence.evidence_ranges().is_empty());
    for lid in ["1.1", "1.2"] {
        authorize_book_text(&json!({"lid":lid}).to_string(), &locators).unwrap();
    }
    assert!(authorize_book_text(r#"{"lid":"1"}"#, &locators).is_err());
    let Some(crate::tool_result::ToolContinuation::NextCall {
        tool, arguments, ..
    }) = draft.continuation
    else {
        panic!("missing read")
    };
    let adapter = FakeAdapter::new(vec![], vec![]);
    let (text, _) =
        dispatch_resident_book_tool(BookToolId::Text, arguments.clone(), &book, &adapter);
    observe_tool_evidence(&mut evidence, &tool, &arguments.to_string(), &text, &book);
    assert_eq!(evidence.evidence_ranges().len(), 1);
    assert_eq!(evidence.evidence_ranges()[0].start_lid, "1.1");
}

#[test]
fn to11_structure_and_context_recovery_execute_on_actual_collections() {
    let mut sidecar: read_tools::BookStructureSidecar = serde_json::from_str(include_str!(
        "../../book-tool-contracts/fixtures/example-book/book_structure.json"
    ))
    .unwrap();
    let unit = sidecar.spine[0].clone();
    sidecar.spine = (1..=60)
        .map(|i| {
            let mut u = unit.clone();
            u.lid = format!("1.{i}");
            u.summary.evidence_lids = vec![u.lid.clone()];
            u
        })
        .collect();
    sidecar.throughlines.clear();
    let book = book_leaves(60).with_book_structure(Some(sidecar));
    let registry = resident_tool_registry();
    let adapter = FakeAdapter::new(vec![], vec![]);
    let mut snapshots = Vec::new();
    for (tool, id, input) in [
        ("book.guide_path", BookToolId::GuidePath, json!({})),
        (
            "book.context",
            BookToolId::Context,
            json!({"lid":"1","granularity":"near","k":60}),
        ),
    ] {
        let (raw, _) = dispatch_resident_book_tool(id, input.clone(), &book, &adapter);
        let draft = crate::tool_result::project_tool_result(
            tool,
            &input.to_string(),
            &raw,
            registry.registration(tool).unwrap().output_policy,
            1500,
            &book,
        );
        assert!(draft.truncated);
        assert!(draft.model_body_json().len() <= 1500);
        let mut locators = TurnLocatorLedger::default();
        let mut evidence = TurnEvidenceLedger::default();
        locators.observe_tool_result(tool, &draft.model_body_json(), &book);
        observe_tool_evidence(
            &mut evidence,
            tool,
            &input.to_string(),
            &draft.model_body_json(),
            &book,
        );
        assert!(evidence.evidence_ranges().is_empty());
        let next = serde_json::to_value(draft.continuation.as_ref().unwrap()).unwrap();
        let next_tool = next["tool"].as_str().unwrap();
        let next_args = next["arguments"].clone();
        registry
            .registration(next_tool)
            .unwrap()
            .validate_arguments(&next_args.to_string())
            .unwrap();
        let next_id = if tool == "book.guide_path" {
            assert_eq!(next_tool, "book.structure");
            authorize_book_text(&json!({"lid":next_args["at"]}).to_string(), &locators).unwrap();
            BookToolId::Structure
        } else {
            assert_eq!(next_tool, "book.context");
            assert!(next_args["k"].as_u64().unwrap() < 60);
            BookToolId::Context
        };
        let (result, _) = dispatch_resident_book_tool(next_id, next_args.clone(), &book, &adapter);
        let result: Value = serde_json::from_str(&result).unwrap();
        assert!(result.get("error_code").is_none(), "{result}");
        snapshots.push(json!({"tool":tool,"input":input,"raw":serde_json::from_str::<Value>(&raw).unwrap(),"delivered":draft.model_body,"continuation":next,"result":result}));
    }
    if let Ok(path) = std::env::var("TO11_COLLECTIONS") {
        std::fs::write(path, serde_json::to_vec_pretty(&snapshots).unwrap()).unwrap();
    }
}
