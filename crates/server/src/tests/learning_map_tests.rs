//! Private learning map construction (ADR-0178 §7, slice M1).
use super::*;
use super::tutor_loop_tests::{finish, fixture_with_book, outcome, step_with_design};
use std::sync::Mutex;

const SOURCE: &str = "Average speed is total distance divided by total time. A rest still counts toward the total time. ";

fn fixture() -> (tempfile::TempDir, AppState) {
    let source = SOURCE.repeat(3);
    fixture_with_book(true, Book::new(sample_base(), &source), &source)
}

/// Structured replies in order; records each request's purpose-free user packet.
struct Scripted { replies: Mutex<Vec<Value>>, packets: Mutex<Vec<Value>> }
impl Scripted {
    fn new(replies: Vec<Value>) -> Self { Self { replies: Mutex::new(replies), packets: Mutex::new(vec![]) } }
}
impl ModelAdapter for Scripted {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> { panic!("structured only") }
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
        self.packets.lock().unwrap().push(serde_json::from_str(&request.user).unwrap());
        Ok(self.replies.lock().unwrap().remove(0))
    }
    fn chat(&self, _: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> { panic!("no agent loop") }
}

fn explained(state: &mut AppState, move_id: &str) -> String {
    let turn = prepare_agent_chat(state, r#"{"message":"Explain average speed"}"#, "now").unwrap_or_else(|r| panic!("{}", r.body)).turn_ref;
    let book = &state.workspace.book;
    let movement = json!({"operation":"select","move":{"move_id":move_id,"object_ids":[],
        "target":{"learning_focus":"Average speed","expected_performance":"Explain why rests count","capability":"explanation",
            "source_bindings":[{"source_id":book.base.book_id,"source_revision":book.source_fingerprint(),"start_lid":"1.1","end_lid":"1.1"}],
            "object_refs":[]},
        "capability":"explanation","kind":"explain","interaction_intent":"neutral","intent_origin":"neutral",
        "prompt":"Average speed uses total distance divided by total time.","actions":[]}});
    let range = EvidenceRange { start_lid: "1.1".into(), end_lid: "1.1".into(), ranges: vec![] };
    step_with_design(&state.private_context(), &turn, movement, &[], &[range]).unwrap();
    finish(state, &turn, &outcome(None));
    format!("delivery:{}:{move_id}", turn.turn_id)
}

fn json_post(state: &mut AppState, path: &str, body: Value) -> Value {
    let reply = post(state, path, &body.to_string());
    assert_eq!(reply.status, 200, "{}", reply.body);
    serde_json::from_str(&reply.body).unwrap()
}

fn set_map(state: &mut AppState, operation: &str, enabled: bool) {
    let revision = state.user.learning_store().unwrap().state().unwrap().control.revision;
    let reply = post(state, "/tutor/mutate", &json!({"operation_id":operation,"expected_revision":revision,
        "action":{"kind":"set_learning_map","enabled":enabled}}).to_string());
    assert_eq!(reply.status, 200, "{}", reply.body);
}

fn open(state: &AppState) -> impl Fn(&memory::teaching::TeachingEvent) -> crate::learning_map_build::Opened {
    let book = state.workspace.book.clone();
    move |_| crate::learning_map_build::Opened::Book(book.clone(), Some("pub-1".into()))
}

fn proposition(key: &str, statement: &str, quote: &str) -> Value {
    json!({"key":key,"statement":statement,"start_lid":"1.1","end_lid":"1.1","quote":quote})
}

#[test]
fn the_map_is_opt_in_sourced_and_merges_only_on_explicit_identity() {
    let (_root, mut state) = fixture();
    let first = explained(&mut state, "first");
    assert!(crate::learning_map_build::next(&state.user, &open(&state)).unwrap().is_none(), "nothing is built before opt-in");
    set_map(&mut state, "map-on", true);
    let pending = crate::learning_map_build::next(&state.user, &open(&state)).unwrap().expect("opted in: the delivery awaits a build");
    assert_eq!(pending.delivery.event_id, first);
    assert!(pending.passages[0].text.contains("A rest still counts"));

    // A fabricated quote drops its relationship and every chain that needs it.
    let adapter = Scripted::new(vec![json!({"propositions":[
            proposition("total","Average speed divides total distance by total time.","total distance divided by total time"),
            proposition("rest","A rest counts toward the total time.","A rest still counts toward the total time"),
            proposition("made_up","Speed is always constant.","speed never changes")],
        "chains":[{"premises":["rest"],"conclusion":"total"},{"premises":["made_up"],"conclusion":"total"}]})]);
    let merge = crate::learning_map_build::build(&adapter, &pending).unwrap();
    assert_eq!(adapter.packets.lock().unwrap().len(), 1, "an empty map needs no identity judgment");
    assert_eq!(adapter.packets.lock().unwrap()[0]["explanation"], "Compare total distance and total time.");
    crate::learning_map_build::record(&state.user, &pending, Ok(merge)).unwrap();
    let store = state.user.learning_store().unwrap();
    let nodes = store.map_nodes().unwrap();
    assert_eq!(nodes.len(), 2);
    let source = &nodes[0].sources[0];
    assert_eq!((source.source_id.as_str(), source.source_revision.as_str(), source.start_lid.as_str(), source.delivery_ref.as_str()),
        (state.workspace.book.base.book_id.as_str(), state.workspace.book.source_fingerprint(), "1.1", first.as_str()));
    assert_eq!(store.map_chains().unwrap().len(), 1);
    let total = nodes.iter().find(|n| n.statement.starts_with("Average speed")).unwrap().node_id.clone();
    drop(store);
    assert!(crate::learning_map_build::next(&state.user, &open(&state)).unwrap().is_none());

    // A later delivery on the same passage: overlap nominates both nodes; the
    // model merges one and keeps a same-worded but different relationship apart.
    explained(&mut state, "second");
    let pending = crate::learning_map_build::next(&state.user, &open(&state)).unwrap().unwrap();
    let adapter = Scripted::new(vec![
        json!({"propositions":[proposition("again","Average speed is total distance over total time.","total distance divided by total time"),
            proposition("other","Average speed is the mean of the speeds.","Average speed is total distance")],"chains":[]}),
        json!({"links":[{"key":"again","node_id":total},{"key":"other","node_id":null}]})]);
    let merge = crate::learning_map_build::build(&adapter, &pending).unwrap();
    let identity = adapter.packets.lock().unwrap()[1].clone();
    assert!(identity["propositions"].as_array().unwrap().iter().all(|p| p["candidates"].as_array().unwrap().len() == 2));
    assert!(identity.get("learner").is_none() && !identity.to_string().contains("evidence"));
    crate::learning_map_build::record(&state.user, &pending, Ok(merge)).unwrap();
    let store = state.user.learning_store().unwrap();
    let nodes = store.map_nodes().unwrap();
    assert_eq!(nodes.len(), 3, "the different relationship stays a separate node");
    assert_eq!(nodes.iter().find(|n| n.node_id == total).unwrap().sources.len(), 2, "the same relationship gains a second delivery");
    drop(store);

    // Off deletes the derived map; on rebuilds it from the first delivery.
    set_map(&mut state, "map-off", false);
    assert!(state.user.learning_store().unwrap().map_nodes().unwrap().is_empty());
    set_map(&mut state, "map-on-again", true);
    assert_eq!(crate::learning_map_build::next(&state.user, &open(&state)).unwrap().unwrap().delivery.event_id, first);
}

#[test]
fn failed_builds_are_bounded_and_unreadable_sources_spend_nothing() {
    let (_root, mut state) = fixture();
    set_map(&mut state, "map-on", true);
    let id = explained(&mut state, "only");
    for _ in 0..memory::learning_map::MAX_BUILD_ATTEMPTS {
        let pending = crate::learning_map_build::next(&state.user, &open(&state)).unwrap().expect("still pending after a failure");
        assert_eq!(pending.delivery.event_id, id);
        crate::learning_map_build::record(&state.user, &pending, Err("provider unavailable".into())).unwrap();
    }
    assert!(crate::learning_map_build::next(&state.user, &open(&state)).unwrap().is_none());
    assert_eq!(state.user.learning_store().unwrap().map_coverage().unwrap(), (0, 1));
    // A book open elsewhere waits without spending an attempt; a gone source fails without a model call.
    use crate::learning_map_build::Opened;
    explained(&mut state, "elsewhere");
    for _ in 0..5 { assert!(crate::learning_map_build::next(&state.user, &|_| Opened::Elsewhere).unwrap().is_none()); }
    assert_eq!(state.user.learning_store().unwrap().map_coverage().unwrap(), (0, 1));
    assert!(crate::learning_map_build::next(&state.user, &open(&state)).unwrap().is_some(), "still pending once its book is open");
    for _ in 0..memory::learning_map::MAX_BUILD_ATTEMPTS { crate::learning_map_build::next(&state.user, &|_| Opened::Gone).unwrap(); }
    assert_eq!(state.user.learning_store().unwrap().map_coverage().unwrap(), (0, 2));
}

#[test]
fn shared_wording_only_nominates_candidates() {
    use crate::learning_map_build::similarity;
    assert!(similarity("Distance is rate times time.", "Distance equals rate times time.") > 0.5);
    assert!(similarity("Distance is rate times time.", "Photosynthesis needs light.") < 0.2);
    assert_eq!(similarity("", "anything"), 0.0);
}

fn note(state: &mut AppState, text: &str, context: Value) -> String {
    let content = serde_json::from_value(json!({"content":text,"authorship":"user","context":context})).unwrap();
    state.user.store.create_personal_note(content, &format!("create-{text}"), "1").unwrap().note_id
}

fn map_source(turn: &str, lid: &str, publication: Option<&str>) -> memory::learning_map::MapSource {
    memory::learning_map::MapSource { source_id: "book-a".into(), source_revision: "r1".into(), start_lid: lid.into(), end_lid: lid.into(),
        quote: "quoted".into(), delivery_ref: format!("delivery:{turn}:m"), turn_id: turn.into(), publication_id: publication.map(String::from) }
}

/// premise "rest" → conclusion "total"; "total" is taught in turn t2, "rest" in t1.
fn seeded_map(state: &mut AppState, operation: &str) {
    set_map(state, operation, true);
    let mut store = state.user.learning_store().unwrap();
    store.merge_map(&memory::learning_map::MapMerge { delivery_ref: "delivery:t1:m".into(),
        new_nodes: vec![("rest".into(), "A rest counts toward the total time.".into())],
        sources: vec![("rest".into(), map_source("t1", "1.1", Some("pub-1")))], chains: vec![] }).unwrap();
    store.merge_map(&memory::learning_map::MapMerge { delivery_ref: "delivery:t2:m".into(),
        new_nodes: vec![("total".into(), "Average speed divides total distance by total time.".into())],
        sources: vec![("total".into(), map_source("t2", "1.2", Some("pub-1")))], chains: vec![(vec!["rest".into()], "total".into())] }).unwrap();
}

#[test]
fn notes_touch_relationships_only_by_turn_or_a_pinned_anchor() {
    let (_root, mut state) = fixture();
    seeded_map(&mut state, "map-on");
    let by_turn = note(&mut state, "Rests still take time?", json!({"turn_id":"t1"}));
    let by_anchor = note(&mut state, "So the denominator is all the time", json!({"book_id":"book-a","publication_id":"pub-1","lid":"1.2.3"}));
    let other_revision = note(&mut state, "Same LID, another publication", json!({"book_id":"book-a","publication_id":"pub-2","lid":"1.2"}));
    let unrelated = note(&mut state, "Photosynthesis needs light", json!({}));
    let relationships = |state: &mut AppState, id: &str| json_post(state, "/me/notes/learning_map_relationships", json!({"note_id":id}))["relationships"].clone();
    assert_eq!(relationships(&mut state, &by_turn)[0]["via"], "turn");
    let anchored = relationships(&mut state, &by_anchor);
    assert_eq!((anchored[0]["node_id"].as_str(), anchored[0]["via"].as_str()), (Some("total"), Some("anchor")));
    assert_eq!(anchored[0]["sources"][0]["quote"], "quoted");
    assert!(relationships(&mut state, &other_revision).as_array().unwrap().is_empty(), "a bare LID never identifies a passage");
    assert!(relationships(&mut state, &unrelated).as_array().unwrap().is_empty());

    // Removal is the user's decision and survives a rebuild of the derived map.
    json_post(&mut state, "/me/notes/learning_map_remove", json!({"note_id":by_turn,"node_id":"rest"}));
    assert!(relationships(&mut state, &by_turn).as_array().unwrap().is_empty());
    set_map(&mut state, "map-off", false);
    assert_eq!(json_post(&mut state, "/me/notes/learning_map_relationships", json!({"note_id":by_anchor}))["enabled"], false);
    seeded_map(&mut state, "map-on-again");
    assert!(relationships(&mut state, &by_turn).as_array().unwrap().is_empty());
}

#[test]
fn structural_recall_runs_only_on_request_and_follows_premises() {
    let (_root, mut state) = fixture();
    seeded_map(&mut state, "map-on");
    let premise = note(&mut state, "Rests still take time?", json!({"turn_id":"t1"}));
    let focus = note(&mut state, "Whole trip time goes underneath", json!({"turn_id":"t2"}));
    note(&mut state, "Unrelated thought", json!({}));

    let plain = json_post(&mut state, "/me/notes/search", json!({"query":"denominator"}));
    assert!(plain["structural"].is_null());
    assert!(plain["hits"].as_array().unwrap().iter().all(|h| h.get("structural_rank").is_none()));

    // Focus on what turn t2 taught: its own note first, then the note on its premise.
    let structural = json_post(&mut state, "/me/notes/search", json!({"query":"denominator","structural":true,"turn_id":"t2"}));
    assert_eq!(structural["structural"]["status"], "used");
    let rank = |id: &str| structural["hits"].as_array().unwrap().iter().find(|h| h["reference"]["note_id"] == id).map(|h| h["structural_rank"].clone());
    assert_eq!(rank(&focus), Some(json!(1)));
    assert_eq!(rank(&premise), Some(json!(2)), "found only through the chain: neither wording nor anchor matches the query");

    // Without a turn, the query's closest map statements are the focus.
    let by_query = json_post(&mut state, "/me/notes/search", json!({"query":"average speed divides total distance by total time","structural":true}));
    assert!(by_query["hits"].as_array().unwrap().iter().any(|h| h["reference"]["note_id"] == premise.as_str() && h["structural_rank"].is_u64()));

    set_map(&mut state, "map-off", false);
    assert_eq!(json_post(&mut state, "/me/notes/search", json!({"query":"x","structural":true}))["structural"]["status"], "off");
}

#[test]
fn link_suggestions_span_one_argument_and_need_the_users_link() {
    let (_root, mut state) = fixture();
    seeded_map(&mut state, "map-on");
    let premise = note(&mut state, "Rests still take time?", json!({"turn_id":"t1"}));
    let conclusion = note(&mut state, "Whole trip time goes underneath", json!({"turn_id":"t2"}));
    let suggested = json_post(&mut state, "/me/notes/learning_map_suggestions", json!({}))["suggestions"].clone();
    assert_eq!(suggested.as_array().unwrap().len(), 1);
    assert_eq!((suggested[0]["from"]["note_id"].as_str(), suggested[0]["to"]["note_id"].as_str()), (Some(premise.as_str()), Some(conclusion.as_str())));
    assert_eq!(suggested[0]["premise"], "A rest counts toward the total time.");
    assert!(state.user.store.note_links(&premise).is_empty(), "a suggestion is not a link");
    json_post(&mut state, "/me/notes/link", json!({"from":suggested[0]["from"],"to":suggested[0]["to"],"reason":"Same argument"}));
    assert!(json_post(&mut state, "/me/notes/learning_map_suggestions", json!({}))["suggestions"].as_array().unwrap().is_empty());
}

#[test]
fn lid_ranges_are_hierarchical() {
    use crate::learning_map_recall::lid_within;
    assert!(lid_within("1.2", "1.1", "1.10"));
    assert!(lid_within("1.10.4", "1.2", "1.10"));
    assert!(!lid_within("1.11", "1.2", "1.10"));
    assert!(!lid_within("1.1", "1.2", "1.3"));
}
