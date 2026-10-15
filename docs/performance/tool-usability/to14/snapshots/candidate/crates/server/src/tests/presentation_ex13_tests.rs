use super::*;
use crate::agent_run::{BorrowedAppPort, RuntimeStatePort};
use runtime::{
    presentation_author::AuthorResult,
    run_context::{CancellationToken, ResidentStatePort},
};

fn author(port: &mut impl ResidentStatePort, args: Value) -> Result<AuthorResult, ToolError> {
    port.author_presentation(
        serde_json::from_value(args).unwrap(),
        &[],
        &[],
        &CancellationToken::default(),
    )
}

fn write_args() -> Value {
    json!({"operation":"write","title":"Revised example","html":"<p>Revised example</p>",
        "readable_content":"Revised example","state_contract":{"count":"integer from 0 to 3"},"initial_state":{"count":0}})
}

struct RecordingAdapter(Arc<Mutex<Vec<Value>>>);
impl ModelAdapter for RecordingAdapter {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        unreachable!()
    }
    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        self.0.lock().unwrap().push(json!({"instructions":request.instructions,
            "messages":request.ordered_messages(),"tools":request.tools.iter().map(|tool| json!({"name":tool.name,"description":tool.description,"parameters":tool.parameters})).collect::<Vec<_>>(),"usage":null}));
        Ok(AssistantTurn {
            provider_continuation: None,
            text: Some("Saved state observed".into()),
            tool_calls: vec![],
            usage_total_tokens: None,
        })
    }
}

#[test]
fn ex13_follow_up_projects_locators_and_reads_the_exact_old_version_on_demand() {
    let (_root, mut state, first) = fixture();
    let mut draft = content(&state, "Original");
    draft.readable_content = "OLD_BODY_ONLY: two of three required pieces. ".repeat(800);
    let candidate = state
        .private_context()
        .create_presentation_candidate(&first.session_id, &first.turn_id, None, draft.clone())
        .unwrap();
    let original = persist(&mut state, &first, &candidate);
    finish(&mut state, &first, &original).unwrap();
    let observation = PresentationState {
        values: json!({"page":{"count":1,"mode":"compare"},"controls":[{"id":"mode","value":"compare"}]}),
        visible_step: Some("compare".into()),
        observed_result: "Found one piece".into(),
        source_ref_ids: vec!["source-rp2".into()],
    };
    let saved = state
        .private_context()
        .save_presentation_state(
            &first.session_id,
            &first.turn_id,
            &original,
            observation.clone(),
        )
        .unwrap();
    save_scene(&mut state, &first, &original, 3);
    let second = next_turn(&mut state);
    let edit = create(&mut state, &second, Some(original.clone()), "Newer version");
    let updated = persist(&mut state, &second, &edit);
    finish(&mut state, &second, &updated).unwrap();
    let goal = &mut state.user.agent_history.sessions[0].goals[0];
    goal.status = runtime::goal::GoalStatus::Open;
    goal.requirements[0].description = "Keep the full explanation and exercises".into();
    goal.working.items = serde_json::from_value(json!([{"id":"remaining","description":"Check the remaining exercises","status":"in_progress"}])).unwrap();
    let goal_id = goal.id.clone();
    let prepared = prepare_agent_chat(&mut state, &json!({"message":"Explain the saved comparison","presentation_follow_up":saved,"goal_id":goal_id}).to_string(), "now").unwrap_or_else(|r| panic!("{}",r.body));
    let context = prepared.agent_message.clone();
    let turn = prepared.turn_ref.clone();
    let scope = prepared.scope.clone();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let adapter = RecordingAdapter(requests.clone());
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let reply =
        crate::agent_run::execute_prepared(&app, &adapter, prepared, CancellationToken::default());
    assert_eq!(reply.reply.status, 200, "{}", reply.reply.body);
    let recorded = requests.lock().unwrap();
    if let Ok(directory) = std::env::var("EX13_FOLLOW_UP_RECORDING_DIR") {
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            std::path::Path::new(&directory).join("request.json"),
            serde_json::to_vec_pretty(&recorded[0]).unwrap(),
        )
        .unwrap();
        std::fs::write(std::path::Path::new(&directory).join("context.json"), serde_json::to_vec_pretty(&json!({"context":context,"readable_content_bytes":draft.readable_content.len(),"usage":null})).unwrap()).unwrap();
    }
    let wire = recorded[0].to_string();
    assert!(
        !wire.contains("OLD_BODY_ONLY"),
        "full body must be loaded on demand"
    );
    assert!(wire.contains("Keep the full explanation and exercises"));
    assert!(wire.contains("Check the remaining exercises"));
    let data: Value = serde_json::from_str(context.lines().last().unwrap()).unwrap();
    assert_eq!(data["receipt"], json!(saved));
    assert_eq!(data["title"], "Original");
    assert_eq!(data["state"], json!(observation));
    assert_eq!(data["assumptions"], json!(draft.assumptions));
    assert!(data.get("readable_content").is_none());
    assert_eq!(data["content_access"]["entrypoint"], "index.html");
    assert_eq!(data["content_access"]["files"], json!(["index.html"]));
    assert_eq!(data["content_access"]["tool"], "presentation.author");
    let mut port = RuntimeStatePort {
        scope: &scope,
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(),
        plots: Default::default(),
    };
    let read = author(&mut port, data["content_access"]["read"].clone()).unwrap();
    assert_eq!(read.body["reference"], json!(original));
    assert_eq!(read.body["file"], "readable_content");
    assert_eq!(read.body["text"], draft.readable_content);
    assert!(read.body.get("readable_content").is_none());
    let found = author(&mut port, json!({"operation":"search","reference":original,"file":"readable_content","query":"OLD_BODY_ONLY","max_matches":1})).unwrap();
    assert_eq!(found.body["matches"][0]["offset"], 0);
    let read = author(&mut port, json!({"operation":"read","reference":original,"file":"readable_content","offset":0,"length":13})).unwrap();
    assert_eq!(read.body["text"], "OLD_BODY_ONLY");
    let found = author(&mut port, json!({"operation":"search","reference":original,"file":"index.html","query":"window.count=2"})).unwrap();
    let read = author(&mut port, json!({"operation":"read","reference":original,"file":"index.html","offset":found.body["matches"][0]["offset"],"length":14})).unwrap();
    assert_eq!(read.body["text"], "window.count=2");
    assert!(read.body.get("readable_content").is_none());
}

#[test]
fn ex13_follow_up_write_requires_an_exact_base_or_explicit_new_object() {
    let (root, mut state, first) = fixture();
    let base = create(&mut state, &first, None, "Original");
    let reference = persist(&mut state, &first, &base);
    finish(&mut state, &first, &reference).unwrap();
    let scene = |count| PresentationState {
        values: json!({"page":{"count":count},"controls":[]}),
        visible_step: Some("compare".into()),
        observed_result: format!("Found {count} pieces"),
        source_ref_ids: vec!["source-rp2".into()],
    };
    let receipt = state
        .private_context()
        .save_presentation_state(&first.session_id, &first.turn_id, &reference, scene(1))
        .unwrap();
    let second = next_turn(&mut state);
    let newer = create(&mut state, &second, Some(reference.clone()), "Newer");
    let newer_ref = persist(&mut state, &second, &newer);
    finish(&mut state, &second, &newer_ref).unwrap();
    let prepared = prepare_agent_chat(
        &mut state,
        &json!({"message":"Revise this example","presentation_follow_up":receipt}).to_string(),
        "now",
    )
    .unwrap_or_else(|r| panic!("{}", r.body));
    state
        .private_context()
        .save_presentation_state(&first.session_id, &first.turn_id, &reference, scene(3))
        .unwrap();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let mut port = RuntimeStatePort {
        scope: &prepared.scope,
        port: &app,
        turn_ref: &prepared.turn_ref,
        previewed: Default::default(),
        animations: Default::default(),
        plots: Default::default(),
    };
    let candidate_count = || {
        std::fs::read_dir(root.path().join("agent-history.presentations/candidates"))
            .unwrap()
            .count()
    };
    let before = candidate_count();
    for choice in [
        json!({}),
        json!({"new_object":false}),
        json!({"new_object":true,"based_on":reference}),
        json!({"based_on":newer_ref}),
    ] {
        let mut args = write_args();
        args.as_object_mut()
            .unwrap()
            .extend(choice.as_object().unwrap().clone());
        let error = author(&mut port, args)
            .err()
            .expect("ambiguous or conflicting base must be rejected");
        assert_eq!(error.category, "validation");
        assert!(error.message.contains("based_on"), "{}", error.message);
        assert_eq!(
            candidate_count(),
            before,
            "invalid choices must not save a candidate"
        );
    }
    let mut args = write_args();
    args["based_on"] = json!(reference);
    args["source_ref_ids"] = json!(["source-rp2"]);
    let revised = author(&mut port, args).unwrap();
    assert_eq!(revised.body["based_on"], json!(reference));
    assert_eq!(revised.body["initial_state"], json!({"count":1}));
    let candidate_id = revised.body["candidate_id"].as_str().unwrap().to_owned();
    let candidate = app
        .with_app(|s| {
            s.private_context()
                .read_presentation_candidate(&first.session_id, &candidate_id)
        })
        .unwrap();
    assert_eq!(
        candidate.content.source_bindings,
        base.content.source_bindings
    );
    assert!(author(
        &mut port,
        json!({"operation":"deliver","candidate_id":candidate_id})
    )
    .is_err());
    // Storage/intent test: seed the existing host receipt; browser behavior is unchanged.
    port.previewed.insert(
        candidate_id.clone(),
        std::collections::HashSet::from(["legacy".into()]),
    );
    let delivered = author(
        &mut port,
        json!({"operation":"deliver","candidate_id":candidate_id}),
    )
    .unwrap()
    .delivered
    .unwrap();
    assert_eq!(delivered.presentation_id, reference.presentation_id);
    assert_eq!(delivered.revision, newer_ref.revision + 1);
    let wrong_patch = author(
        &mut port,
        json!({"operation":"patch","reference":newer_ref,"edits":[{"old_text":"window.count=2","new_text":"window.count=1"}]}),
    );
    assert!(wrong_patch.err().unwrap().message.contains("exact version"));
    let mut new_args = write_args();
    new_args["new_object"] = json!(true);
    let mut borrowed_source = new_args.clone();
    borrowed_source["source_ref_ids"] = json!(["source-rp2"]);
    assert_eq!(
        author(&mut port, borrowed_source).err().unwrap().error_code,
        "PRESENTATION_SOURCE_UNKNOWN"
    );
    let new = author(&mut port, new_args).unwrap();
    assert!(new.body["based_on"].is_null());
    assert_eq!(new.body["initial_state"], json!({"count":0}));
    let patched = author(&mut port, json!({"operation":"patch","candidate_id":new.body["candidate_id"],"edits":[{"old_text":"Revised example","new_text":"Independent example"}]})).unwrap();
    let id = patched.body["candidate_id"].as_str().unwrap().to_owned();
    assert!(author(&mut port, json!({"operation":"deliver","candidate_id":id})).is_err());
    port.previewed.insert(
        id.clone(),
        std::collections::HashSet::from(["legacy".into()]),
    );
    let independent = author(&mut port, json!({"operation":"deliver","candidate_id":id}))
        .unwrap()
        .delivered
        .unwrap();
    assert_ne!(independent.presentation_id, reference.presentation_id);
    assert_eq!(independent.revision, 1);
    let patched = author(&mut port, json!({"operation":"patch","reference":reference,"edits":[{"old_text":"window.count=2","new_text":"window.count=1"}]})).unwrap();
    assert_eq!(patched.body["based_on"], json!(reference));
    assert_eq!(patched.body["initial_state"], json!({"count":1}));
    assert_eq!(
        app.with_app(|s| s
            .private_context()
            .read_presentation(&first.session_id, &reference))
            .unwrap()
            .content,
        base.content
    );
}

#[test]
fn ex13_ordinary_creation_still_allows_omission_and_explicit_new_object() {
    let (_root, mut state, turn) = fixture();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope = app.with_app(|s| crate::run_scope::RunScope::capture(s, &turn, "test", None, None));
    let mut port = RuntimeStatePort {
        scope: &scope,
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(),
        plots: Default::default(),
    };
    for choice in [None, Some(false), Some(true)] {
        let mut args = write_args();
        if let Some(choice) = choice {
            args["new_object"] = json!(choice);
        }
        let saved = author(&mut port, args).unwrap();
        assert!(saved.body["based_on"].is_null());
    }
}
