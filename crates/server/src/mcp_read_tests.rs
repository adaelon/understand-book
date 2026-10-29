use super::*;
use crate::{
    auth::AuthService,
    control_store::{ControlStore, ServiceWriter},
    mcp_access::McpAccess,
};
use base_schema::{LidNode, NodeKind, Span};
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::Path,
};

const NOW: i64 = 1_790_000_000;
const PASSWORD: &str = "fixture-only-password";
include!("../../read-tools/testdata/math_book.rs");

#[test]
fn ranked_search_remote_hits_are_locators_until_their_source_read() {
    let f = Fixture::new();
    let search = f.call("alice", "book_search", json!({"published_book_ref":f.a,"query":"needle content","limit":2})).unwrap();
    assert_eq!(search["sources"], json!([]));
    assert!(!search["data"]["hits"].as_array().unwrap().is_empty());
    let call = &search["data"]["hits"][0]["next_calls"][0];
    assert_eq!(call["name"], "book_text");
    assert_eq!(call["arguments"]["published_book_ref"], json!(f.a));
    let read = f.next("alice", call).unwrap();
    assert!(!read["sources"].as_array().unwrap().is_empty());
    assert!(read["data"]["text"].as_str().unwrap().contains("needle"));
    let resident = crate::cross_book::read(&f.library, "alice", Some(&[f.a.clone()]), "library.read",
        json!({"operation":"search","published_book_ref":f.a,"query":"needle content","limit":2})).unwrap();
    assert_eq!(resident.body["sources"], json!([]));
    let resident_call = &resident.body["data"]["hits"][0]["next_calls"][0];
    assert_eq!(resident_call["name"], "library.read");
    assert_eq!(resident_call["arguments"]["operation"], "text");
    let resident_read = crate::cross_book::read(&f.library, "alice", Some(&[f.a.clone()]), "library.read", resident_call["arguments"].clone()).unwrap();
    assert!(resident_read.body["data"]["text"].as_str().unwrap().contains("needle"));
    assert!(!resident_read.body["sources"].as_array().unwrap().is_empty());
}

#[test]
fn workspace_search_requires_members_and_filters_before_top_k() {
    let f = Fixture::new();
    let search = |members: Option<&[PublishedBookRef]>, args: Value| crate::workspace_retrieval::search_with(
        &f.library, "alice", members, args, |_| Err("semantic retrieval is not configured".into()));
    let missing = search(None, json!({"query":"needle"})).err().unwrap();
    assert_eq!(missing.error_code, "WORKSPACE_REQUIRED");
    f.library.lock().unwrap().grant("alice", &f.b).unwrap();
    let members = vec![f.a.clone()];
    let result = search(Some(&members), json!({"query":"needle content","limit":1})).unwrap();
    assert!(result.source.is_none());
    assert_eq!(result.body["hits"].as_array().unwrap().len(), 1);
    assert_eq!(result.body["hits"][0]["published_book_ref"], json!(f.a));
    assert_eq!(result.body["retrieval"]["semantic"]["status"], "unavailable");
    assert_eq!(result.body["exhaustive"], false);
    let call = &result.body["hits"][0]["next_calls"][0];
    let read = crate::cross_book::read(&f.library, "alice", Some(&members), "library.read", call["arguments"].clone()).unwrap();
    assert!(read.body["data"]["text"].as_str().unwrap().contains("needle"));
    let outside = search(Some(&members), json!({"query":"needle","published_book_refs":[f.b]})).err().unwrap();
    assert_eq!(outside.error_code, "WORKSPACE_MATERIAL_UNAVAILABLE");
    f.library.lock().unwrap().revoke("alice", &f.a).unwrap();
    let revoked = search(Some(&[f.a.clone(), f.b.clone()]), json!({"query":"needle","limit":1})).unwrap();
    assert_eq!(revoked.body["hits"][0]["published_book_ref"], json!(f.b));
    let empty = search(Some(&[]), json!({"query":"needle"})).unwrap();
    assert_eq!(empty.body["hits"], json!([]));
}

#[test]
fn workspace_search_semantic_candidates_keep_versions_and_do_not_hold_library_lock() {
    let f = Fixture::new();
    let publication = f.library.lock().unwrap().load("alice", &f.a).unwrap();
    let span = publication.book.base.lid_nodes.iter().find(|n| n.lid == "1.2").unwrap().span.clone();
    let revision = publication.book.source_fingerprint().to_owned();
    let result = crate::workspace_retrieval::search_with(&f.library, "alice", Some(&[f.a.clone()]), json!({"query":"generalization to unfamiliar examples"}), |request| {
        assert!(f.library.try_lock().is_ok(), "semantic inference must not hold the library mutex");
        assert_eq!(request.publications.len(), 1);
        assert_eq!(request.publications[0].publication_id, f.a.publication_id);
        assert_eq!(request.publications[0].source_revision, revision);
        Ok(crate::semantic_retrieval::SemanticSearchResult { status:"ready".into(),reason:None,
            hits:vec![crate::semantic_retrieval::SemanticSearchHit { book_id:f.a.book_id.clone(),publication_id:f.a.publication_id.clone(),
                lid:"1.2".into(),start_utf16:span.start as u64,end_utf16:span.end as u64,score:0.9 }],
            indexed_publications:1,embedded_chunks:60,cache_hits:0,query_tokens:6,document_tokens:300 })
    }).unwrap();
    assert!(result.source.is_none());
    let hit = &result.body["hits"][0];
    assert_eq!(hit["source_revision"], revision);
    assert_eq!(hit["published_book_ref"], json!(f.a));
    assert_eq!(hit["semantic_rank"], 1);
    assert!(hit["lexical_rank"].is_null());
    assert_eq!(hit["next_calls"][0]["arguments"]["start_utf16"], span.start);
    assert_eq!(result.body["retrieval"]["semantic"]["status"], "ready");
}

#[test]
fn workspace_search_rechecks_authorization_after_inference_before_final_limit() {
    let f = Fixture::new();
    f.library.lock().unwrap().grant("alice", &f.b).unwrap();
    let result = crate::workspace_retrieval::search_with(&f.library, "alice", Some(&[f.a.clone(),f.b.clone()]),
        json!({"query":"needle content","limit":1}), |_| {
            f.library.lock().unwrap().revoke("alice", &f.a).unwrap();
            Err("test lexical fallback".into())
        }).unwrap();
    assert_eq!(result.body["hits"].as_array().unwrap().len(), 1);
    assert_eq!(result.body["hits"][0]["published_book_ref"], json!(f.b));
}

#[test]
fn math_structure_remote_and_reference_reads_follow_same_publication() {
    let f = Fixture::new();
    let path = f.input.path().join("optimization");
    fs::create_dir_all(&path).unwrap();
    let (base, source, index) = optimization_math_material();
    fs::write(path.join("base.json"), serde_json::to_vec(&base).unwrap()).unwrap();
    fs::write(path.join("source.txt"), source).unwrap();
    fs::write(path.join("math_structure.json"), index.to_string()).unwrap();
    let reference = {
        let mut library = f.library.lock().unwrap();
        let reference = library.publish(&path).unwrap().reference;
        library.grant("alice", &reference).unwrap();
        reference
    };
    let args = json!({"published_book_ref":reference,"lid":"1.8","end_lid":"1.9"});
    let remote = f.call("alice", "book_text", args.clone()).unwrap();
    assert_eq!(remote["data"]["text"], optimization_math_book().text("1.8", Some("1.9")).unwrap());
    let next = &remote["data"]["math_context"][0]["next_calls"][0];
    assert_eq!(next["name"], "book_text");
    assert_eq!(next["arguments"]["published_book_ref"], json!(reference));
    let results = f.next("alice", next).unwrap();
    assert!(results["data"]["text"].as_str().unwrap().contains("定理 6.5"));
    let context = f.call("alice", "book_context", json!({"published_book_ref":reference,"lid":"1.9","k":2})).unwrap();
    assert_eq!(context["data"]["items"][1]["via"]["type"], "asks_to_prove");
    let mut library_args = args;
    library_args["operation"] = json!("text");
    let local = crate::cross_book::read(&f.library, "alice", None, "library.read", library_args).unwrap().body;
    let next = &local["data"]["math_context"][0]["next_calls"][0];
    assert_eq!(next["name"], "library.read");
    assert_eq!(next["arguments"]["published_book_ref"], json!(reference));
    let results = crate::cross_book::read(&f.library, "alice", None, next["name"].as_str().unwrap(), next["arguments"].clone()).unwrap();
    assert!(results.body["data"]["text"].as_str().unwrap().contains("定理 6.5"));
    if let Ok(dir) = std::env::var("TO12_DIR") {
        let tools = tool_definitions();
        let samples: Vec<_> = [("book_text", remote), ("book_context", context)].into_iter().map(|(name, body)| {
            let mut schema = tools.iter().find(|t| t["name"] == name).unwrap()["outputSchema"].clone();
            // McpRead owns content; the HTTP handler adds usage after charging.
            schema["required"].as_array_mut().unwrap().retain(|key| key != "usage");
            json!({"schema":schema,"body":body})
        }).collect();
        fs::write(Path::new(&dir).join("math-remote.json"), serde_json::to_vec_pretty(&samples).unwrap()).unwrap();
    }
}

#[test]
fn w2_library_scope_filters_before_paging_and_keeps_live_authorization() {
    let f = Fixture::new();
    let c = {
        let mut library = f.library.lock().unwrap();
        library.grant("alice", &f.b).unwrap();
        material(&f.input.path().join("c"), "gamma", false);
        let c = library.publish(&f.input.path().join("c")).unwrap().reference;
        library.grant("alice", &c).unwrap();
        c
    };
    let members = vec![f.a.clone(), c.clone()];
    let page = crate::cross_book::read(&f.library, "alice", Some(&members), "library.list", json!({"limit":1})).unwrap().body;
    assert_eq!(page["items"][0]["published_book_ref"], json!(f.a));
    let next = &page["next_calls"][0];
    let last = crate::cross_book::read(&f.library, "alice", Some(&members), "library.list", next["arguments"].clone()).unwrap().body;
    assert_eq!(last["items"][0]["published_book_ref"], json!(c));
    assert_eq!(last["complete"], true);
    let outside = crate::cross_book::read(&f.library, "alice", Some(&members), "library.read", json!({"operation":"text","published_book_ref":f.b,"lid":"1.2"})).err().unwrap();
    assert_eq!(outside.error_code, "WORKSPACE_MATERIAL_UNAVAILABLE");
    let args = json!({"operation":"text","published_book_ref":c,"lid":"1.2"});
    assert!(crate::cross_book::read(&f.library, "alice", Some(&members), "library.read", args.clone()).is_ok());
    // A later member set excludes gamma; the admitted set keeps it until its grant is revoked.
    assert!(crate::cross_book::read(&f.library, "alice", Some(&[f.a.clone()]), "library.read", args.clone()).is_err());
    assert!(crate::cross_book::read(&f.library, "alice", Some(&members), "library.read", args.clone()).is_ok());
    f.library.lock().unwrap().revoke("alice", &c).unwrap();
    assert!(crate::cross_book::read(&f.library, "alice", Some(&members), "library.read", args).is_err());
    let empty = crate::cross_book::read(&f.library, "alice", Some(&[]), "library.list", json!({})).unwrap().body;
    assert_eq!(empty["items"], json!([]));
    assert_eq!(empty["complete"], true);
}

#[test]
fn resident_library_uses_same_reads_without_mcp_identity_or_usage_and_preserves_versions() {
    let f = Fixture::new();
    let catalog = crate::cross_book::read(&f.library, "alice", None, "library.list", json!({})).unwrap();
    assert_eq!(catalog.body["items"].as_array().unwrap().len(), 1);
    assert_eq!(catalog.body["items"][0]["published_book_ref"], json!(f.a));
    assert!(catalog.body["items"][0]["available_operations"].as_array().unwrap().contains(&json!("text")));
    let args = json!({"operation":"text","published_book_ref":f.a,"lid":"1.1"});
    let first = crate::cross_book::read(&f.library, "alice", None, "library.read", args).unwrap();
    assert_eq!(first.source.as_ref().unwrap().reference, f.a);
    assert_eq!(first.body["data"]["lid"], "1.1");
    assert!(first.body.get("usage").is_none());
    assert!(first.body["sources"][0]["source_url"].as_str().unwrap().starts_with("/?book_id="));
    let next = &first.body["next_calls"][0];
    assert_eq!(next["name"], "library.read");
    let second = crate::cross_book::read(&f.library, "alice", None, next["name"].as_str().unwrap(), next["arguments"].clone()).unwrap();
    assert_eq!(second.body["published_book_ref"], json!(f.a));
    assert_ne!(first.body["data"]["source_range"], second.body["data"]["source_range"]);
    assert!(crate::cross_book::read(&f.library, "bob", None, "library.read", next["arguments"].clone()).is_err());
    f.library.lock().unwrap().revoke("alice", &f.a).unwrap();
    assert!(crate::cross_book::read(&f.library, "alice", None, "library.read", next["arguments"].clone()).is_err());
}

#[test]
fn cross_book_source_preview_and_open_resolve_exact_authorized_publication() {
    let f=Fixture::new();
    let publication=f.library.lock().unwrap().load("alice",&f.a).unwrap();
    let range=read_tools::EvidenceRange {start_lid:"1.2".into(),end_lid:"1.2".into(),ranges:vec![]};
    let resolved=publication.book.resolve_source(&range,"zh-CN",None).unwrap();
    let binding=runtime::orchestrator::SourceBinding { music: None, media: None,source_ref_id:"ref-rust".into(),book_id:f.a.book_id.clone(),
        publication_id:Some(f.a.publication_id.clone()),source_revision:Some(publication.book.source_fingerprint().into()),evidence_range:Some(range),evidence_text_digest:resolved.evidence_text_digest,
        label_snapshot:"Rust".into(),preview_snapshot:resolved.preview};
    let view=crate::cross_book::source_view(&f.library,"alice",&binding,false).unwrap();
    assert!(view["highlighted_quote"].as_str().unwrap().contains("needle 2"));
    assert!(view["source_url"].as_str().unwrap().contains(&format!("publication_id={}",f.a.publication_id)));
    let opened=crate::cross_book::source_view(&f.library,"alice",&binding,true).unwrap();
    assert_eq!(opened["opened"],false);
    assert_eq!(opened["source_url"],view["source_url"]);
    assert!(crate::cross_book::source_view(&f.library,"bob",&binding,false).is_err());
}

#[test]
fn board_source_window_cross_book_retains_publication_and_opens_the_browsed_position() {
    let f = Fixture::new();
    let publication = f.library.lock().unwrap().load("alice", &f.a).unwrap();
    let range = read_tools::EvidenceRange { start_lid: "1.2".into(), end_lid: "1.2".into(), ranges: vec![] };
    let resolved = publication.book.resolve_source(&range, "zh-CN", None).unwrap();
    let binding = runtime::orchestrator::SourceBinding { music: None, media: None, source_ref_id: "window-exact".into(),
        book_id: f.a.book_id.clone(), publication_id: Some(f.a.publication_id.clone()),
        source_revision: Some(publication.book.source_fingerprint().into()), evidence_range: Some(range),
        evidence_text_digest: resolved.evidence_text_digest, label_snapshot: resolved.label, preview_snapshot: resolved.preview };
    let metadata_path = f.input.path().join("a/paper_metadata.json");
    let mut metadata: Value = serde_json::from_slice(&fs::read(&metadata_path).unwrap()).unwrap();
    metadata["title"]["value"] = json!("New edition");
    fs::write(metadata_path, metadata.to_string()).unwrap();
    let newer = {
        let mut library = f.library.lock().unwrap();
        let newer = library.publish(&f.input.path().join("a")).unwrap().reference;
        library.grant("alice", &newer).unwrap();
        newer
    };
    assert_ne!(newer, f.a);
    let read = crate::cross_book::source_window(&f.library, "alice", &binding, &json!({"anchor_lid":"1.40"})).unwrap();
    assert_eq!(read["published_book_ref"], json!(f.a));
    assert_eq!(read["focus_lid"], "1.40");
    assert!(read["segments"].as_array().unwrap().iter().any(|segment| segment["text"].as_str().unwrap().contains("needle 40")));
    let open = crate::cross_book::source_view_at(&f.library, "alice", &binding, true, Some("1.40")).unwrap();
    let url = url::Url::parse(&format!("https://reader.example{}", open["source_url"].as_str().unwrap())).unwrap();
    let query: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(query["publication_id"], f.a.publication_id);
    assert_eq!(query["start_lid"], "1.40");
    assert_eq!(query["end_lid"], "1.40");
    assert!(!query.contains_key("start_utf16"));
    assert!(!query.contains_key("end_utf16"));
    assert!(crate::cross_book::source_window(&f.library, "bob", &binding, &json!({})).is_err());
    f.library.lock().unwrap().revoke("alice", &f.a).unwrap();
    assert!(crate::cross_book::source_window(&f.library, "alice", &binding, &json!({})).is_err(), "newer authorized edition must not replace the citation's unavailable edition");
}

#[test]
fn cross_book_learning_survives_reopen_and_updates_only_from_real_user_feedback() {
    cross_book_learning_scenario(None);
}

#[test]
fn w3_cross_book_understanding_stays_in_original_workspace_with_tutor_off() {
    cross_book_learning_scenario(Some("workspace-a"));
}

fn cross_book_learning_scenario(scope: Option<&str>) {
    let f=Fixture::new();
    let path=f.root.path().join("alice-memory.json");
    let mut user=crate::user_runtime::UserRuntime::local(memory::MemoryStore::open(&path).unwrap(),None,Default::default(),None);
    let mut session=crate::new_agent_session("main-book","now",1);
    session.learning_workspace_id=scope.map(str::to_owned);
    session.turns.push(serde_json::from_value(json!({"turn_id":"t1","user":"I do not understand borrowing.","status":"pending_assistant"})).unwrap());
    let turn=crate::AgentTurnRef {session_id:session.id.clone(),turn_id:"t1".into(),user_turn_ordinal:1};
    user.agent_history.sessions.push(session);
    let publication=f.library.lock().unwrap().load("alice",&f.a).unwrap();
    let range=read_tools::EvidenceRange {start_lid:"1.2".into(),end_lid:"1.2".into(),ranges:vec![]};
    let resolved=publication.book.resolve_source(&range,"zh-CN",None).unwrap();
    let source=runtime::orchestrator::SourceBinding { music: None, media: None,source_ref_id:"rust-source".into(),book_id:f.a.book_id.clone(),publication_id:Some(f.a.publication_id.clone()),source_revision:Some(publication.book.source_fingerprint().into()),
        evidence_range:Some(range),evidence_text_digest:resolved.evidence_text_digest,label_snapshot:"Rust".into(),preview_snapshot:resolved.preview};
    macro_rules! context { ($user:expr) => {crate::PrivateBookContext {user:$user,learning_workspace_id:scope,book:&publication.book,book_dir:publication.directory(),messages:&[],selected_chat:Some(turn.session_id.as_str())}}; }
    let mut request=json!({"operation":"observe","operation_id":"difficulty","source_ref_ids":["rust-source"],
        "learner_quote":"I do not understand borrowing.","learning_focus":"borrowing","expected_performance":"explain a borrow","capability":"explanation",
        "interpretation":"The learner explicitly asks for help with borrowing.","teaching_implication":"Explain a shared borrow with a concrete example."});
    assert!(crate::cross_book::learning(&context!(&user),&turn,&f.a,&request,&[],"now").is_err());
    request["learner_quote"]=json!("I have mastered borrowing.");
    assert!(crate::cross_book::learning(&context!(&user),&turn,&f.a,&request,&[source.clone()],"now").is_err());
    request["learner_quote"]=json!("I do not understand borrowing.");
    let first=crate::cross_book::learning(&context!(&user),&turn,&f.a,&request,&[source.clone()],"now").unwrap();
    assert_eq!(first["status"],"unassessed");
    assert_eq!(crate::cross_book::learning(&context!(&user),&turn,&f.a,&request,&[source.clone()],"later").unwrap(),first);
    user.store=memory::MemoryStore::open(&path).unwrap();
    let read=crate::cross_book::learning(&context!(&user),&turn,&f.a,&json!({"operation":"read"}),&[],"now").unwrap();
    assert_eq!(read["interpretations"][0]["evidence_id"],first["evidence_ref"]);
    if scope.is_some() {
        let other_scope=crate::PrivateBookContext { learning_workspace_id:Some("workspace-b"), ..context!(&user) };
        let isolated=crate::cross_book::learning(&other_scope,&turn,&f.a,&json!({"operation":"read"}),&[],"now").unwrap();
        assert_eq!(isolated["interpretations"],json!([]));
        assert_eq!(isolated["recent_facts"],json!([]));
        let legacy=user.learning_store().unwrap();
        assert!(legacy.current_interpretations(&f.a.book_id,publication.book.source_fingerprint(),0,12).unwrap().is_empty());
        assert!(!user.learning_store_for_workspace(scope).unwrap().state().unwrap().control.enabled);
    }
    assert!(read["rows"].as_array().unwrap().iter().all(|r|r["independent_support"]==0));
    // Local LearningMemory is scoped to its directory, as in the service's per-user roots.
    let other_root=tempfile::tempdir().unwrap();
    let other=crate::user_runtime::UserRuntime::local(memory::MemoryStore::open(other_root.path().join("memory.json")).unwrap(),None,Default::default(),None);
    let isolated=crate::cross_book::learning(&context!(&other),&turn,&f.a,&json!({"operation":"read"}),&[],"now").unwrap();
    assert!(isolated["interpretations"].as_array().unwrap().is_empty());
    let outcome:runtime::orchestrator::OuterOutcome=serde_json::from_value(json!({"answer":"A shared borrow allows reading.","incomplete":false,"turns":1,"tokens_spent":1,"effects":[],"trace":[]})).unwrap();
    user.agent_history.sessions[0].turns[0].source_bindings=vec![source.clone()];
    user.agent_history.sessions[0].turns[0].outcome=Some(outcome);
    crate::teaching::record_user_delivery(&user,&turn,"now").unwrap();
    let before=crate::cross_book::learning(&context!(&user),&turn,&f.a,&json!({"operation":"read"}),&[],"now").unwrap();
    assert!(!before["recent_facts"].as_array().unwrap().iter().any(|e|e["payload"]["usage"]["kind"]=="assistant_delivery"));
    // Completed delivery is persisted once, and becomes assistance for the real follow-up.
    user.agent_history.sessions[0].turns[0].status=crate::AgentAssistantStatus::Completed;
    crate::teaching::record_user_delivery(&user,&turn,"now").unwrap();
    crate::teaching::record_user_delivery(&user,&turn,"retry").unwrap();
    user.agent_history.sessions[0].turns.push(serde_json::from_value(json!({"turn_id":"t2","user":"A shared borrow reads without taking ownership.","status":"pending_assistant"})).unwrap());
    let next=crate::AgentTurnRef {session_id:turn.session_id.clone(),turn_id:"t2".into(),user_turn_ordinal:2};
    request["learner_quote"]=json!("A shared borrow reads without taking ownership.");
    request["supersedes"]=first["evidence_ref"].clone();
    request["interpretation"]=json!("The user distinguishes reading from ownership in this example.");
    let revised=crate::cross_book::learning(&context!(&user),&next,&f.a,&request,&[source],"later").unwrap();
    let evidence=user.learning_store_for_workspace(scope).unwrap().evidence(revised["evidence_ref"].as_str().unwrap()).unwrap();
    assert_eq!(evidence.assistance_refs.len(),1);
    assert_eq!(evidence.supersedes.as_deref(),first["evidence_ref"].as_str());
    let reopened=crate::teaching::understanding(&context!(&user),&f.a.book_id,None,0).unwrap();
    assert_eq!(reopened["interpretations"].as_array().unwrap().len(),1);
    assert_eq!(reopened["interpretations"][0]["evidence_id"],revised["evidence_ref"]);
}

struct Fixture {
    library: Mutex<PublishedLibrary>,
    access: McpAccess,
    control: ControlStore,
    tokens: BTreeMap<String, String>,
    root: tempfile::TempDir,
    input: tempfile::TempDir,
    a: PublishedBookRef,
    b: PublishedBookRef,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let input = tempfile::tempdir().unwrap();
        let writer = ServiceWriter::acquire(root.path()).unwrap();
        let mut control = ControlStore::open(writer.clone()).unwrap();
        let auth = AuthService::new(ControlStore::open(writer.clone()).unwrap());
        let access = McpAccess::new(ControlStore::open(writer.clone()).unwrap());
        let mut tokens = BTreeMap::new();
        for user in ["alice", "bob"] {
            control.provision_password(user, PASSWORD, true).unwrap();
            let cookie = auth.login(user, PASSWORD, None, NOW).unwrap();
            let principal = auth.authenticate(&cookie, NOW).unwrap();
            tokens.insert(
                user.into(),
                access
                    .create(&principal, "Reader Agent", NOW)
                    .unwrap()
                    .token,
            );
        }
        let mut library = PublishedLibrary::new(ControlStore::open(writer).unwrap());
        material(&input.path().join("a"), "alpha", true);
        material(&input.path().join("b"), "beta", false);
        let a = library.publish(&input.path().join("a")).unwrap().reference;
        let b = library.publish(&input.path().join("b")).unwrap().reference;
        library.grant("alice", &a).unwrap();
        library.grant("bob", &b).unwrap();
        Self {
            library: Mutex::new(library),
            access,
            control,
            tokens,
            root,
            input,
            a,
            b,
        }
    }
    fn call(&self, user: &str, name: &str, args: Value) -> Result<Value, ToolError> {
        let identity = self
            .access
            .authenticate(Some(&format!("Bearer {}", self.tokens[user])))?;
        McpRead::new(&self.library, "https://reader.example")
            .unwrap()
            .call(&identity, name, args)
    }
    fn next(&self, user: &str, call: &Value) -> Result<Value, ToolError> {
        self.call(
            user,
            call["name"].as_str().unwrap(),
            call["arguments"].clone(),
        )
    }
}
pub(crate) fn material(path: &Path, id: &str, enriched: bool) {
    fs::create_dir_all(path).unwrap();
    let mut source = "\n\n".to_string();
    let mut base = base_schema::sample_base();
    base.book_id = id.into();
    let mut nodes = Vec::new();
    for i in 1..=60 {
        let start = source.encode_utf16().count();
        source.push_str(&format!(
            "needle {i} 中文😀 {}\n\n",
            if i == 1 {
                "甲".repeat(16_005)
            } else {
                "content".into()
            }
        ));
        nodes.push(LidNode {
            lid: format!("1.{i}"),
            path: vec![1, i],
            kind: NodeKind::Paragraph,
            span: Span {
                start,
                end: source.encode_utf16().count(),
            },
            children: vec![],
        });
    }
    base.lid_nodes = vec![LidNode {
        lid: "1".into(),
        path: vec![1],
        kind: NodeKind::Chapter,
        span: Span {
            start: 0,
            end: source.encode_utf16().count(),
        },
        children: nodes.iter().map(|n| n.lid.clone()).collect(),
    }];
    base.lid_nodes.extend(nodes);
    base.graph_nodes[0].occurrences = (1..=60).map(|i| format!("1.{i}")).collect();
    fs::write(path.join("base.json"), serde_json::to_vec(&base).unwrap()).unwrap();
    fs::write(path.join("source.txt"), source).unwrap();
    if enriched {
        // Paper fixtures enter through the same source-readiness gate as published materials.
        let book = read_tools::Book::load(path.to_str().unwrap()).unwrap();
        fs::create_dir_all(path.join(".build/source-reconciliation")).unwrap();
        fs::write(
            path.join(".build/source-reconciliation/report.json"),
            json!({"version":"source_reconciliation_report.v1","book_id":id,"unresolved":[]})
                .to_string(),
        )
        .unwrap();
        let unavailable = json!({"status":"unavailable","reason":"text-only test material"});
        fs::write(
            path.join("source_manifest.json"),
            json!({"version":"source_manifest.v2","book_id":id,
            "canonical_source":{"path":"source.txt","sha256":book.source_fingerprint()},
            "capabilities":{"view_pdf":unavailable,"project_lid_to_pdf":unavailable,
                "project_ranges_to_pdf":unavailable,"resolve_pdf_selection":unavailable}})
            .to_string(),
        )
        .unwrap();
        let header = json!({"book_id":id,"book_version":"v1","profile_id":"paper","profile_version":"paper_v0",
            "core_schema_version":"v1","generated_at":"2026-10-11T00:00:00Z"});
        let anchored = json!({"text":"A built explanation", "evidence_lids":["1.1"]});
        let spine: Vec<_> = (1..=60)
            .map(|i| {
                json!({"lid":format!("1.{i}"),"role":"foundation","summary":anchored,
            "key_stop_ids":[],"depends_on":[]})
            })
            .collect();
        let threads: Vec<_> = (1..=60).map(|i|json!({"id":format!("t{i}"),"name":format!("Thread {i}"),"summary":anchored,
            "lids":if i == 1 { (1..=60).map(|i|format!("1.{i}")).collect::<Vec<_>>() } else { vec!["1.1".into()] },"key_stop_ids":[]})).collect();
        fs::write(
            path.join("book_structure.json"),
            json!({"header":header,"spine":spine,"throughlines":threads,"key_stops":[]})
                .to_string(),
        )
        .unwrap();
        fs::write(path.join("paper_metadata.json"), json!({"header":header,"title":{"value":"Fixture Paper","source":"front_matter","evidence_lids":["1.1"],"confidence":1.0},
            "references":{"value":(1..=60).map(|i|json!({"raw":format!("Reference {i}"),"identifiers":null})).collect::<Vec<_>>(),
                "source":"paper_text","evidence_lids":["1.60"],"confidence":1.0}}).to_string()).unwrap();
        fs::write(path.join("paper_lexicon.json"), json!({"header":header,"entries":(1..=60).map(|i|json!({
            "term":format!("term{i}"),"term_type":"term","occurrences_lids":[format!("1.{i}")],"defined_at_lid":format!("1.{i}"),"aliases":[]})).collect::<Vec<_>>()}).to_string()).unwrap();
    }
}

#[test]
fn mcp_read_catalog_isolates_accounts_pages_versions_and_avoids_private_or_resident_state() {
    let f = Fixture::new();
    let metadata_path = f.input.path().join("a/paper_metadata.json");
    let mut metadata: Value = serde_json::from_slice(&fs::read(&metadata_path).unwrap()).unwrap();
    metadata["title"]["value"] = json!("Updated publication metadata");
    fs::write(metadata_path, metadata.to_string()).unwrap();
    let second = {
        let mut library = f.library.lock().unwrap();
        let second = library
            .publish(&f.input.path().join("a"))
            .unwrap()
            .reference;
        library.grant("alice", &second).unwrap();
        second
    };
    let first = f.call("alice", "book_list", json!({"limit":1})).unwrap();
    assert_eq!(first["items"][0]["published_book_ref"], json!(f.a));
    assert_eq!(first["items"][0]["name"], "Fixture Paper");
    assert_eq!(
        first["items"][0]["available_tools"]
            .as_array()
            .unwrap()
            .len(),
        10
    );
    assert_eq!(first["items"][0]["is_default"], false);
    assert_eq!(first["complete"], false);
    let last = f.next("alice", &first["next_calls"][0]).unwrap();
    assert_eq!(last["items"][0]["published_book_ref"], json!(second));
    assert_eq!(last["complete"], true);
    assert_eq!(last["items"][0]["is_default"], true);
    assert_eq!(last["items"][0]["name"], "Updated publication metadata");
    let bob = f.call("bob", "book_list", json!({})).unwrap();
    assert_eq!(bob["items"][0]["published_book_ref"], json!(f.b));
    assert_eq!(bob["items"].as_array().unwrap().len(), 1);
    assert_eq!(f.library.lock().unwrap().resident_usage(), (0, 0));
    for (user, reference) in [("alice", &f.b), ("bob", &f.a)] {
        assert_eq!(
            f.call(
                user,
                "book_text",
                json!({"published_book_ref":reference,"lid":"1"})
            )
            .unwrap_err()
            .error_code,
            "MCP_MATERIAL_UNAVAILABLE"
        );
    }
    for reference in [&f.a, &second] {
        let result = f
            .call(
                "alice",
                "book_text",
                json!({"published_book_ref":reference,"lid":"1.2"}),
            )
            .unwrap();
        assert_eq!(result["published_book_ref"], json!(reference));
        assert_eq!(result["sources"][0]["published_book_ref"], json!(reference));
        let url = url::Url::parse(result["sources"][0]["source_url"].as_str().unwrap()).unwrap();
        assert_eq!(url.origin().ascii_serialization(), "https://reader.example");
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "publication_id")
                .unwrap()
                .1,
            reference.publication_id
        );
        let metadata = f
            .call(
                "alice",
                "book_paper_metadata",
                json!({"published_book_ref":reference}),
            )
            .unwrap();
        assert_eq!(
            metadata["data"]["title"]["value"],
            if reference == &f.a {
                "Fixture Paper"
            } else {
                "Updated publication metadata"
            }
        );
    }
    assert!(!f.root.path().join("users").exists());
    assert_eq!(
        f.control
            .connection
            .query_row("SELECT count(*) FROM reader_workspaces", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn mcp_read_text_pages_keep_exact_source_and_recheck_cached_grants_on_continuation() {
    let mut f = Fixture::new();
    let pinned = f.library.lock().unwrap().load("alice", &f.a).unwrap();
    let expected = pinned.book.text("1.1", Some("1.60")).unwrap();
    let mut result = f
        .call(
            "alice",
            "book_text",
            json!({"published_book_ref":f.a,"lid":"1.1","end_lid":"1.60"}),
        )
        .unwrap();
    let first = result.clone();
    let mut text = String::new();
    let mut position = result["data"]["requested_range"]["start"].as_u64().unwrap();
    loop {
        assert_eq!(result["data"]["source_range"]["start"], position);
        let fragment = result["data"]["text"].as_str().unwrap();
        assert!(fragment.chars().count() <= 8_000);
        position += fragment.encode_utf16().count() as u64;
        assert_eq!(result["data"]["source_range"]["end"], position);
        assert_eq!(
            result["sources"][0]["source_range"],
            result["data"]["source_range"]
        );
        let url = url::Url::parse(result["sources"][0]["source_url"].as_str().unwrap()).unwrap();
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "end_utf16")
                .unwrap()
                .1,
            position.to_string()
        );
        text.push_str(fragment);
        if result["complete"] == true {
            break;
        }
        result = f.next("alice", &result["next_calls"][0]).unwrap();
    }
    assert_eq!(text, expected);
    f.library.lock().unwrap().revoke("alice", &f.a).unwrap();
    assert_eq!(
        f.next("alice", &first["next_calls"][0])
            .unwrap_err()
            .error_code,
        "MCP_MATERIAL_UNAVAILABLE"
    );
    assert_eq!(
        f.call("alice", "book_list", json!({})).unwrap()["items"],
        json!([])
    );
    f.library.lock().unwrap().grant("alice", &f.a).unwrap();
    f.control.set_user_disabled("alice", true).unwrap();
    assert_eq!(
        f.next("alice", &first["next_calls"][0])
            .unwrap_err()
            .error_code,
        "MCP_AUTH_REQUIRED"
    );
}

fn reassemble(f: &Fixture, name: &str, args: Value) -> Value {
    let first = f.call("alice", name, args).unwrap();
    let mut data = first["data"].clone();
    let mut pending: VecDeque<_> = first["next_calls"]
        .as_array()
        .unwrap()
        .iter()
        .cloned()
        .collect();
    let mut pages = 0;
    while let Some(call) = pending.pop_front() {
        let result = f.next("alice", &call).unwrap();
        if name == "book_paper_metadata" {
            assert!(
                !result["sources"].as_array().unwrap().is_empty(),
                "metadata page must retain enclosing evidence"
            );
        }
        let path = result["data_path"].as_str().unwrap();
        let target = data.pointer_mut(path).unwrap().as_array_mut().unwrap();
        assert_eq!(
            target.len(),
            call["arguments"]["page"]["offset"].as_u64().unwrap() as usize
        );
        assert!(result["data"].as_array().unwrap().len() <= MAX_PAGE_ITEMS);
        target.extend(result["data"].as_array().unwrap().iter().cloned());
        pending.extend(result["next_calls"].as_array().unwrap().iter().cloned());
        pages += 1;
        assert!(pages < 100);
    }
    assert!(pages > 0, "fixture must exercise continuation");
    data
}

#[test]
fn mcp_read_to8_nested_candidate_calls_remain_executable() {
    let f = Fixture::new();
    let args = json!({"published_book_ref":f.a,"query":"needle","limit":1});
    let first = f.call("alice", "book_concept", args.clone()).unwrap();
    assert_eq!(first["complete"], false);
    assert!(first["next_calls"].as_array().unwrap().iter().any(|c| c["arguments"]["page"]["path"] == "/candidates/0/next_calls"));
    let data = reassemble(&f, "book_concept", args);
    let candidate = &data["candidates"][0];
    let calls = candidate["next_calls"].as_array().unwrap();
    assert_eq!(calls.len(), 60);
    for (lid, call) in candidate["occurrences"].as_array().unwrap().iter().zip(calls) {
        assert_eq!(call["arguments"]["lid"], *lid);
        assert_eq!(call["arguments"]["published_book_ref"], json!(f.a));
    }
    let last = f.next("alice", calls.last().unwrap()).unwrap();
    assert!(last["data"]["text"].as_str().unwrap().starts_with("needle 60 "));
    assert_eq!(last["sources"][0]["start_lid"], "1.60");
    assert_eq!(last["sources"][0]["published_book_ref"], json!(f.a));
}

#[test]
fn mcp_read_to9_context_pagination_and_missing_structure_are_independent() {
    let f = Fixture::new();
    let mut samples = Vec::new();
    // The chapter has 60 children. The transport page size is 20, independent of k.
    for k in [0, 10, 40, 60] {
        let args = json!({"published_book_ref":f.b,"lid":"1","granularity":"far","k":k});
        let first = f.call("bob", "book_context", args.clone()).unwrap();
        assert_eq!(first["data"]["total_candidates"], 60);
        assert_eq!(first["data"]["returned_count"], k);
        assert_eq!(first["data"]["truncated"], k < 60);
        assert_eq!(first["complete"], k <= 20);
        let mut items = first["data"]["items"].as_array().unwrap().clone();
        let mut pending = VecDeque::from(first["next_calls"].as_array().unwrap().clone());
        let mut pages = Vec::new();
        while let Some(next) = pending.pop_front() {
            let page = f.next("bob", &next).unwrap();
            assert_eq!(page["data_path"], "/items");
            assert_eq!(page["total_items"], k);
            assert_eq!(page["published_book_ref"], json!(f.b));
            items.extend(page["data"].as_array().unwrap().iter().cloned());
            pending.extend(page["next_calls"].as_array().unwrap().iter().cloned());
            pages.push(page);
            assert!(pages.len() <= 2);
        }
        assert_eq!(items.len(), k as usize);
        for (i, item) in items.iter().enumerate() {
            assert_eq!(item["lid"], format!("1.{}", i + 1));
            let next = &item["next_calls"][0];
            assert_eq!(next["name"], "book_text");
            assert_eq!(next["arguments"]["published_book_ref"], json!(f.b));
        }
        if let Some(last) = items.last() {
            let read = f.next("bob", &last["next_calls"][0]).unwrap();
            assert!(read["data"]["text"].as_str().unwrap().contains(&format!("needle {k}")));
        }
        samples.push(json!({"input":args,"first":first,"pages":pages,"reconstructed_count":items.len()}));
    }
    let list = f.call("bob", "book_list", json!({})).unwrap();
    let available = list["items"][0]["available_tools"].as_array().unwrap();
    assert!(available.contains(&json!("book_context")));
    assert!(!available.contains(&json!("book_structure")));
    let missing = f.call("bob", "book_structure", json!({"published_book_ref":f.b})).unwrap_err();
    assert_eq!(missing.error_code, "MCP_CAPABILITY_UNAVAILABLE");
    if let Ok(path) = std::env::var("TO9_REMOTE") {
        fs::write(path, serde_json::to_vec_pretty(&json!({
            "samples":samples,"list":list,"missing_structure":missing,
        })).unwrap()).unwrap();
    }
}

#[test]
fn mcp_read_nested_structure_and_paper_collections_reassemble_without_omissions() {
    let f = Fixture::new();
    let publication = f.library.lock().unwrap().load("alice", &f.a).unwrap();
    let book = &publication.book;
    for (name, expected) in [
        ("book_structure", json!(book.structure(None).unwrap())),
        ("book_guide_path", json!(book.guide_path(None).unwrap())),
        (
            "book_paper_metadata",
            json!(book.paper_metadata_projection()),
        ),
        ("book_paper_lexicon", json!(book.paper_lexicon_projection())),
        (
            "book_paper_reading_guide",
            json!(book.paper_reading_guide(None, None).unwrap()),
        ),
    ] {
        assert_eq!(
            reassemble(&f, name, json!({"published_book_ref":f.a})),
            expected,
            "{name}"
        );
    }
    // Collection reconstruction compares the remote shape, including version-bound calls.
    let mut expected = json!(book.concept_candidates("command", None, None).unwrap());
    for candidate in expected["candidates"].as_array_mut().unwrap() {
        candidate["next_calls"] = json!(candidate["occurrences"].as_array().unwrap().iter()
            .map(|lid| json!({"name":"book_text","arguments":{"lid":lid,"published_book_ref":f.a}}))
            .collect::<Vec<_>>());
    }
    assert_eq!(
        reassemble(
            &f,
            "book_concept",
            json!({"published_book_ref":f.a,"query":"command"})
        ),
        expected
    );
    let scoped = f
        .call(
            "alice",
            "book_structure",
            json!({"published_book_ref":f.a,"at":"1.1"}),
        )
        .unwrap();
    assert_eq!(scoped["data"]["spine_unit"]["lid"], "1.1");
    let first = f
        .call("alice", "book_structure", json!({"published_book_ref":f.a}))
        .unwrap();
    f.library.lock().unwrap().revoke("alice", &f.a).unwrap();
    assert_eq!(
        f.next("alice", &first["next_calls"][0])
            .unwrap_err()
            .error_code,
        "MCP_MATERIAL_UNAVAILABLE"
    );
}

#[test]
fn mcp_read_search_context_empty_results_and_remote_parameter_validation() {
    let f = Fixture::new();
    // Search has independent occurrence-batch and collection (including section-count) continuations.
    for page_size in [3, 50] {
        let mut pending = VecDeque::from([json!({"name":"book_search_text",
            "arguments":{"published_book_ref":f.a,"query":"needle","page_size":page_size}})]);
        let mut ordinals = Vec::new();
        let mut calls = 0;
        while let Some(call) = pending.pop_front() {
            let result = f.next("alice", &call).unwrap();
            let occurrences = if result["data_path"] == "" {
                assert_eq!(result["data"]["total_occurrences"], 60);
                Some(&result["data"]["occurrences"])
            } else if result["data_path"] == "/occurrences" {
                Some(&result["data"])
            } else {
                None
            };
            if let Some(occurrences) = occurrences {
                assert!(!result["sources"].as_array().unwrap().is_empty());
                ordinals.extend(
                    occurrences
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v["ordinal"].as_u64().unwrap()),
                );
            }
            pending.extend(result["next_calls"].as_array().unwrap().iter().cloned());
            calls += 1;
            assert!(calls < 100);
        }
        ordinals.sort_unstable();
        assert_eq!(ordinals.len(), 60);
        assert!(ordinals.windows(2).all(|pair| pair[0] + 1 == pair[1]));
    }
    let empty = f
        .call(
            "alice",
            "book_search_text",
            json!({"published_book_ref":f.a,"query":"not-in-fixture"}),
        )
        .unwrap();
    assert_eq!(empty["data"]["occurrences"], json!([]));
    assert_eq!(empty["complete"], true);
    let context = f
        .call(
            "alice",
            "book_context",
            json!({"published_book_ref":f.a,"lid":"1.1","granularity":"near","k":3}),
        )
        .unwrap();
    assert_eq!(context["data"]["anchor"], "1.1");
    assert!(!context["sources"].as_array().unwrap().is_empty());
    for name in [
        "book_query",
        "book_synthesize",
        "book_guide",
        "book_manifest",
        "mcp_usage",
        "unknown",
    ] {
        assert_eq!(
            f.call("alice", name, json!({})).unwrap_err().error_code,
            "MCP_TOOL_UNAVAILABLE"
        );
    }
    for (name, args) in [
        ("book_text", json!({"lid":"1"})),
        (
            "book_text",
            json!({"published_book_ref":f.a,"lid":"1","user_id":"bob"}),
        ),
        (
            "book_text",
            json!({"published_book_ref":f.a,"lid":"1","start_utf16":-1}),
        ),
        (
            "book_text",
            json!({"published_book_ref":f.a,"lid":"1","page":{"path":""}}),
        ),
        (
            "book_structure",
            json!({"published_book_ref":f.a,"page":{"path":"/throughlines","offset":999}}),
        ),
        (
            "book_structure",
            json!({"published_book_ref":f.a,"page":{"path":"/available"}}),
        ),
        (
            "book_structure",
            json!({"published_book_ref":f.a,"page":{"path":"/throughlines","limit":0}}),
        ),
        (
            "book_structure",
            json!({"published_book_ref":f.a,"page":{"path":"/throughlines","limit":51}}),
        ),
        (
            "book_search_text",
            json!({"published_book_ref":f.a,"query":"needle","page_size":51}),
        ),
        ("book_list", json!({"limit":0})),
        ("book_list", json!({"user_id":"bob"})),
        (
            "book_list",
            json!({"after":{"book_id":"alpha","publication_id":f.a.publication_id,"extra":1}}),
        ),
    ] {
        assert_eq!(
            f.call("alice", name, args.clone()).unwrap_err().category,
            "validation",
            "{name}: {args}"
        );
    }
    for name in [
        "book_structure",
        "book_guide_path",
        "book_paper_metadata",
        "book_paper_lexicon",
        "book_paper_reading_guide",
    ] {
        assert_eq!(
            f.call("bob", name, json!({"published_book_ref":f.b}))
                .unwrap_err()
                .error_code,
            "MCP_CAPABILITY_UNAVAILABLE"
        );
    }
    let definitions = tool_definitions();
    assert_eq!(definitions.len(), 11);
    for tool in definitions.iter().skip(1) {
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert!(tool["inputSchema"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("published_book_ref")));
    }
}

#[test]
fn mcp_read_to10_reverse_search_preserves_native_and_collection_continuations() {
    let f = Fixture::new();
    let input = json!({"published_book_ref":f.a,"query":"needle","match_mode":"exact","scope":{"within_lid":"1"},"order":"reverse_document","page_size":50});
    let mut pending = VecDeque::from([json!({"name":"book_search_text","arguments":input})]);
    let mut hits = Vec::new();
    let mut calls = Vec::new();
    while let Some(call) = pending.pop_front() {
        for key in [
            "published_book_ref",
            "query",
            "match_mode",
            "scope",
            "order",
            "page_size",
        ] {
            assert_eq!(call["arguments"][key], input[key]);
        }
        let result = f.next("alice", &call).unwrap();
        let occurrences = if result["data_path"] == "" {
            Some(&result["data"]["occurrences"])
        } else if result["data_path"] == "/occurrences" {
            Some(&result["data"])
        } else {
            None
        };
        if let Some(occurrences) = occurrences {
            let page: Vec<_> = occurrences
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["ordinal"].as_u64().unwrap())
                .collect();
            assert!(page.windows(2).all(|p| p[0] == p[1] + 1));
            hits.extend(page);
        }
        pending.extend(result["next_calls"].as_array().unwrap().iter().cloned());
        calls.push(json!({"call":call,"result":result}));
        assert!(calls.len() < 20);
    }
    hits.sort_unstable();
    assert_eq!(hits, (1..=60).collect::<Vec<_>>());
    if let Ok(path) = std::env::var("TO10_REMOTE_SEARCH") {
        fs::write(path, serde_json::to_vec_pretty(&calls).unwrap()).unwrap();
    }
}
