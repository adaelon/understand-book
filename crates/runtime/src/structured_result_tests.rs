use super::*;

fn fixture() -> Book {
    Book::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../book-tool-contracts/fixtures/example-book"
    ))
    .unwrap()
}

fn project(tool: &str, input: Value, raw: &Value, limit: usize) -> ToolResultDraft {
    let mut policy = crate::orchestrator::resident_tool_registry()
        .registration(tool)
        .unwrap()
        .output_policy;
    policy.max_model_body_bytes = limit;
    let result = project_tool_result(
        tool,
        &input.to_string(),
        &raw.to_string(),
        policy,
        limit,
        &fixture(),
    );
    assert!(result.model_body_json().len() <= limit);
    assert_eq!(
        serde_json::from_str::<Value>(&result.model_body_json()).unwrap(),
        result.model_body
    );
    if let Some(
        ToolContinuation::NextCall {
            tool, arguments, ..
        }
        | ToolContinuation::RefineCall {
            tool, arguments, ..
        }
        | ToolContinuation::ToolCursor { tool, arguments },
    ) = &result.continuation
    {
        crate::orchestrator::resident_tool_registry()
            .registration(tool)
            .unwrap()
            .validate_arguments(&arguments.to_string())
            .unwrap();
    }
    if let Ok(directory) = std::env::var("TO11_SNAPSHOTS") {
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(std::path::Path::new(&directory).join(format!("{tool}-{limit}.json")),
            serde_json::to_vec_pretty(&json!({"tool":tool,"input":input,"raw":raw,
                "limit":limit,"raw_bytes":json_len(raw),"delivered_bytes":result.model_body_json().len(),
                "status":result.status,"truncated":result.truncated,"delivered":result.model_body,
                "continuation":result.continuation})).unwrap()).unwrap();
    }
    result
}

#[test]
fn to11_discovery_keeps_complete_purpose_and_capability_recovery() {
    let sample: Value = serde_json::from_str(include_str!(
        "../../../docs/performance/tool-usability/to7/budget-observation.json"
    ))
    .unwrap();
    let raw = &sample["raw"];
    let draft = project("tool.search", sample["input"].clone(), raw, 4096);
    assert_eq!(draft.model_body["matches"].as_array().unwrap().len(), 5);
    for (actual, original) in draft.model_body["matches"]
        .as_array()
        .unwrap()
        .iter()
        .zip(raw["matches"].as_array().unwrap())
    {
        for field in [
            "name",
            "use_when",
            "avoid_when",
            "matched_capabilities",
            "visibility",
            "effect_mode",
            "preconditions",
        ] {
            assert_eq!(actual[field], original[field], "{field}");
        }
    }
    for key in [
        "activated",
        "unmet_capabilities",
        "blocked",
        "next_searches",
    ] {
        assert_eq!(draft.model_body.get(key), raw.get(key), "{key}");
    }
    assert_eq!(
        draft.model_body["delivery"]["collections"]["/matches"],
        json!({"total":5,"delivered":5})
    );
}

#[test]
fn to11_context_distinguishes_selected_candidates_from_delivery() {
    let items: Vec<Value> = (0..40).map(|i| json!({"lid":format!("1.{i}"),"layer":"tree",
        "via":{"kind":"tree","relation":"child"},"next_calls":[{"tool":"book.text","arguments":{"lid":format!("1.{i}")}}]})).collect();
    let raw = json!({"anchor":"1","total_candidates":60,"returned_count":40,"truncated":true,"items":items});
    let draft = project("book.context", json!({"lid":"1","k":40}), &raw, 1024);
    for key in ["anchor", "total_candidates", "returned_count", "truncated"] {
        assert_eq!(draft.model_body[key], raw[key]);
    }
    let delivered = draft.model_body["items"].as_array().unwrap().len();
    assert!(delivered > 0 && delivered < 40);
    assert_eq!(draft.model_body["items"], json!(&items[..delivered]));
    assert_eq!(
        draft.model_body["delivery"]["collections"]["/items"],
        json!({"total":40,"delivered":delivered})
    );
    let Some(ToolContinuation::RefineCall {
        tool, arguments, ..
    }) = draft.continuation
    else {
        panic!("refine expected")
    };
    assert_eq!(tool, "book.context");
    assert_eq!(arguments["k"], delivered);
    validate_input(BookToolId::Context, arguments).unwrap();
}

#[test]
fn to11_query_preserves_verdicts_and_omitted_citation_identity() {
    let raw = json!({"status":"partial","answer":"The result applies only under the stated assumptions. ".repeat(100),
        "citations":[{"lid":"1.1","text":"A".repeat(1200),"role":"support"},{"lid":"1.2","text":"B".repeat(1200),"role":"caveat"}],
        "bindings":[{"target":"alpha","candidate_id":"concept:alpha","kind":"concept","canonical_label":"alpha","source_lids":["1.1"]}],
        "support":[{"obligation_index":0,"verdict":"uncertain","citation_lids":["1.1"],"support_note":"Only one direction is supported; the converse has not been established."}],
        "model_supplement":[]});
    let draft = project(
        "book.query",
        json!({"intent":"explanation","targets":["alpha"],"query":"Explain alpha","anchor_lid":"1.1","obligations":[{"requirement":"Explain the assumptions"}]}),
        &raw,
        1024,
    );
    assert_eq!(draft.model_body["support"], raw["support"]);
    assert_eq!(draft.model_body["bindings"], raw["bindings"]);
    assert_eq!(draft.model_body["status"], "partial");
    assert_eq!(draft.model_body["answer"], Value::Null);
    assert_eq!(
        draft.model_body["delivery"]["read_lids"],
        json!(["1.1", "1.2"])
    );
    let Some(ToolContinuation::NextCall {
        tool, arguments, ..
    }) = draft.continuation
    else {
        panic!("read expected")
    };
    assert_eq!(tool, "book.text");
    assert_eq!(arguments, json!({"lid":"1.1"}));
    serde_json::from_value::<crate::QueryOutcome>(draft.model_body).unwrap();
}

#[test]
fn to11_structure_and_profile_keep_warnings_identity_and_whole_items() {
    let book = fixture();
    let mut sidecar: read_tools::BookStructureSidecar = serde_json::from_str(include_str!(
        "../../book-tool-contracts/fixtures/example-book/book_structure.json"
    ))
    .unwrap();
    let thread = sidecar.throughlines[0].clone();
    sidecar.throughlines = (0..50)
        .map(|i| {
            let mut t = thread.clone();
            t.id = format!("thread-{i}");
            t
        })
        .collect();
    let structure = serde_json::to_value(
        fixture()
            .with_book_structure(Some(sidecar))
            .structure(Some("1.1"))
            .unwrap(),
    )
    .unwrap();
    let draft = project("book.structure", json!({"at":"1.1"}), &structure, 1500);
    assert!(draft.truncated);
    assert_eq!(draft.model_body["warning"], structure["warning"]);
    assert_eq!(draft.model_body["available"], true);
    assert_eq!(draft.model_body["spine_unit"], structure["spine_unit"]);
    assert!(draft.model_body["throughlines"]
        .as_array()
        .unwrap()
        .iter()
        .all(|s| structure["throughlines"].as_array().unwrap().contains(s)));
    let manifest = serde_json::to_value(book.profile_manifest()).unwrap();
    let profile = project("profile.manifest", json!({}), &manifest, 2200);
    assert_eq!(profile.model_body["profile_id"], manifest["profile_id"]);
    assert_eq!(
        profile.model_body["profile_version"],
        manifest["profile_version"]
    );
    assert_eq!(
        profile.model_body["memory_policy"],
        manifest["memory_policy"]
    );
    assert!(
        profile.continuation.is_none(),
        "manifest has no narrower call or paging API"
    );
    assert!(profile.model_body["delivery"]["recovery"]
        .as_str()
        .unwrap()
        .contains("No paging"));
    for key in ["projections", "ui_slots", "layout_presets", "agent_tools"] {
        for item in profile.model_body[key].as_array().unwrap() {
            assert!(manifest[key].as_array().unwrap().contains(item));
        }
    }
}

#[test]
fn to11_minimum_semantics_fail_explicitly_without_corrupting_warnings_or_error_reason() {
    let raw = json!({"available":false,"warning":"BookStructure is not attached. ".repeat(60),"key_stops":[],"throughlines":[]});
    let draft = project("book.structure", json!({}), &raw, 512);
    assert_eq!(draft.model_body["projection"], "semantic_budget_exceeded");
    assert_eq!(draft.model_body["available"], false);
    assert_eq!(draft.model_body["delivery"]["status"], "omitted");
    assert!(
        draft.model_body.get("warning").is_none(),
        "do not present a mangled limitation as the original"
    );
    let error = json!({"error_code":"LID_NOT_FOUND","category":"not_found","required_capability":"structural_index",
        "message":"Requested LID does not exist.","details":{"locations":vec!["1.1";1000]}});
    let projected = project("book.text", json!({"lid":"9.9"}), &error, 512);
    assert_eq!(projected.status, ToolResultStatus::Error);
    for key in ["error_code", "category", "required_capability", "message"] {
        assert_eq!(projected.model_body[key], error[key]);
    }
}

#[test]
fn to11_paper_warnings_and_source_references_survive_large_bodies() {
    let mut sidecar: read_tools::BookStructureSidecar = serde_json::from_str(include_str!(
        "../../book-tool-contracts/fixtures/example-book/book_structure.json"
    ))
    .unwrap();
    sidecar.header.profile_id = "paper".into();
    let book = fixture().with_book_structure(Some(sidecar));
    let mut guide = serde_json::to_value(
        book.paper_reading_guide(Some("deep"), Some("critical"))
            .unwrap(),
    )
    .unwrap();
    assert!(!guide["questions"].as_array().unwrap().is_empty());
    assert!(!guide["warnings"].as_array().unwrap().is_empty());
    for question in guide["questions"].as_array_mut().unwrap() {
        question["question"] = json!(question["question"].as_str().unwrap().repeat(100));
    }
    let draft = project(
        "book.paper_reading_guide",
        json!({"mode":"deep","stage":"critical"}),
        &guide,
        3500,
    );
    assert!(draft.truncated);
    for field in ["warnings", "available", "mode", "stage"] {
        assert_eq!(draft.model_body[field], guide[field]);
    }
    assert_eq!(
        draft.model_body["codebook"]["warnings"],
        guide["codebook"]["warnings"]
    );
    assert_eq!(
        draft.model_body["abstract_aid"]["warning"],
        guide["abstract_aid"]["warning"]
    );
    let Some(ToolContinuation::RefineCall { arguments, .. }) = draft.continuation else {
        panic!("smaller guide expected")
    };
    validate_input(BookToolId::PaperReadingGuide, arguments.clone()).unwrap();
    book.paper_reading_guide(arguments["mode"].as_str(), arguments["stage"].as_str())
        .unwrap();
    let source = json!({"source_ref_id":"source_ref_0123456789abcdef","label":"Activation conditions","preview":"Canonical excerpt. ".repeat(1000)});
    let result = project(
        "source.present",
        json!({"quote":"Canonical excerpt."}),
        &source,
        512,
    );
    assert_eq!(result.model_body["source_ref_id"], source["source_ref_id"]);
    assert_eq!(result.model_body["label"], source["label"]);
    assert!(result.model_body.get("preview").is_none());
    for budget in [4, 100, 300] {
        let tiny = project(
            "book.structure",
            json!({}),
            &json!({"available":false,"warning":"Missing structure. ".repeat(500)}),
            budget,
        );
        assert_eq!(tiny.status, ToolResultStatus::Partial);
        assert!(tiny.continuation.is_none());
    }
}
