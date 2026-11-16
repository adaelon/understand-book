use super::*;
use base_schema::PublishedBookRef;
use crate::agent_run::{AppStatePort, RuntimeStatePort};
use crate::agent_stream::{RunDescriptor, RunStream};
use runtime::{music::*, run_context::{CancellationToken, ResidentStatePort}};

fn fixture() -> (tempfile::TempDir, AppState, AgentTurnRef, PublishedBookRef) {
    let root = tempfile::tempdir().unwrap();
    let mut state = state_named(&format!("music-runtime-{}", root.path().file_name().unwrap().to_string_lossy()));
    state.workspace.book_dir = root.path().to_path_buf();
    state.user.history_path = Some(root.path().join("history.json"));
    let id = state.workspace.book.base.book_id.clone();
    let reference = PublishedBookRef { book_id:id.clone(), publication_id:"original".into() };
    let manifest = json!({"version":"music_source_manifest.v1","book_id":id,"title":"A repeated phrase",
        "work":{"work_id":"work","title":"A repeated phrase","composer":"Fixture","movement":null},
        "score":{"source":{"score_id":"score","revision":"1"},"title":"Fixture score","musicxml_path":"score.xml"},
        "recording":{"source":{"recording_id":"recording","revision":"1"},"title":"Fixture recording","performers":"Fixture","media_path":"recording.wav","content_type":"audio/wav","duration_ms":2000,"kind":"performance"},
        "rights":[],"parts":[{"part_id":"piano","name":"Piano","instrument":"Piano","group":"keyboard","voices":["1"]}],
        "pages":[{"page_id":"page1","path":"score/page1.svg","width":100,"height":100}],
        "score_objects":[{"source_id":"note1","part_id":"piano","measure":"1","beat":0,"duration_beats":1,"voice":"1","midi_pitch":60,"rest":false,"tie":"none","page_id":"page1","svg_element_id":"n1"}],
        "occurrences":[{"occurrence_id":"first","order":0,"measure":"1","start_ms":0,"end_ms":1000,"score_object_ids":["note1"]},{"occurrence_id":"repeat","order":1,"measure":"1","start_ms":1000,"end_ms":2000,"score_object_ids":["note1"]}],
        "note_alignments":[],"coverage":[],"provenance":[],"events":[],"structure":[],"demonstrations":[]});
    std::fs::write(root.path().join("music_source_manifest.json"),manifest.to_string()).unwrap();
    std::fs::write(root.path().join("publication.json"),json!({"version":"publication.v1","reference":reference,"source_fingerprint":"fixture","files":{},"capabilities":{},"artifact_versions":{},"teaching_readiness":{},"build_readiness":{},"resident_bytes":0}).to_string()).unwrap();
    let turn = precommit_agent_turn(&mut state,&id,"Listen".into(),None,None,None,"now").unwrap();
    (root,state,turn,reference)
}

fn request(reference: &PublishedBookRef) -> MusicInteractRequest {
    MusicInteractRequest::Observe {publication:reference.clone(), recording:base_schema::MusicRecordingRef {recording_id:"recording".into(),revision:"1".into()}}
}
fn start(state: &mut AppState, turn: &AgentTurnRef) -> Arc<RunStream> {
    let stream = RunStream::new(RunDescriptor {book_id:state.workspace.book.base.book_id.clone(),session_id:turn.session_id.clone(),turn_id:turn.turn_id.clone()});
    state.workspace.active_agent_stream = Some(Arc::downgrade(&stream));stream
}
fn run(state: Arc<Mutex<AppState>>, turn: AgentTurnRef, request: MusicInteractRequest, token: CancellationToken) -> std::thread::JoinHandle<Result<MusicPlaybackReceipt,ToolError>> {
    std::thread::spawn(move || {
        let scope=state.with_app(|s|crate::run_scope::RunScope::capture(s,&turn,"Listen",None,None));
        let mut port=RuntimeStatePort {port:&state,turn_ref:&turn,scope:&scope,previewed:Default::default(),animations:Default::default(),plots:Default::default()};
        port.interact_music(request,&token)
    })
}
fn wait(stream:&RunStream)->MusicCommand {
    let until=std::time::Instant::now()+std::time::Duration::from_secs(5);
    loop {if let Some(command)=stream.snapshot().music_command{return command;} assert!(std::time::Instant::now()<until,"music command missing");std::thread::sleep(std::time::Duration::from_millis(5));}
}
fn claim(command:&MusicCommand)->Value {
    json!({"operation":"claim","command_id":command.command_id,"run_turn_id":command.run_turn_id,"publication":command.request.publication(),"recording":command.request.recording(),"player_id":"player1"})
}
fn complete(command:&MusicCommand,status:&str,paused:bool)->Value {
    json!({"operation":"complete","command_id":command.command_id,"run_turn_id":command.run_turn_id,"player_id":"player1",
        "receipt":{"receipt_id":"browser-id","command_id":command.command_id,"publication":command.request.publication(),"recording":command.request.recording(),"status":status,"current_ms":100,"paused":paused,"playback_rate":1,"loop":null,"focused_part_ids":[],"occurrence_id":"first","demonstration_id":null,"error":null}})
}
fn post_music(state:&Arc<Mutex<AppState>>,input:&Value)->Reply {state.with_app(|s|post(s,"/agent/music/respond",&input.to_string()))}

#[test]
fn music_player_requires_actual_matching_ack_and_cancel_closes_command() {
    let (_root,mut state,turn,reference)=fixture();let stream=start(&mut state,&turn);let state=Arc::new(Mutex::new(state));
    let task=run(state.clone(),turn.clone(),request(&reference),CancellationToken::default());let command=wait(&stream);
    assert_eq!(post_music(&state,&claim(&command)).status,200);
    assert!(!task.is_finished(),"claim does not prove observation or playback");
    assert!(post_music(&state,&claim(&command)).body.contains("\"claimed\":false"));
    let mut wrong=complete(&command,"observed",true);wrong["receipt"]["publication"]["publication_id"]=json!("newer");
    assert_ne!(post_music(&state,&wrong).status,200);
    assert_eq!(post_music(&state,&complete(&command,"observed",true)).status,200);
    let observed=task.join().unwrap().unwrap();assert_ne!(observed.receipt_id,"browser-id");
    assert!(stream.snapshot().music_command.is_none());
    let act=MusicInteractRequest::Act {publication:reference,recording:observed.recording.clone(),based_on:observed.receipt_id,action:MusicAction::Play{start_ms:Some(0.0),end_ms:Some(1000.0),cue_event_id:None}};
    let token=CancellationToken::default();let task=run(state.clone(),turn,act,token.clone());let command=wait(&stream);
    assert_eq!(post_music(&state,&claim(&command)).status,200);
    assert_ne!(post_music(&state,&complete(&command,"applied",true)).status,200,"paused audio is not a completed play action");
    token.cancel();stream.cancelling();assert_eq!(task.join().unwrap().unwrap_err().error_code,"AGENT_RUN_CANCELLED");
    assert_ne!(post_music(&state,&complete(&command,"applied",false)).status,200,"cancelled command cannot replay");
    assert!(stream.snapshot().music_command.is_none());
}

#[test]
fn music_player_replacement_and_navigation_do_not_act_on_another_scene() {
    let (_root,mut state,turn,reference)=fixture();let stream=start(&mut state,&turn);let state=Arc::new(Mutex::new(state));
    let task=run(state.clone(),turn.clone(),request(&reference),CancellationToken::default());let command=wait(&stream);
    let mut no_player=claim(&command);no_player["player_id"]=Value::Null;
    assert!(post_music(&state,&no_player).body.contains("\"claimed\":false"));
    assert_eq!(task.join().unwrap().unwrap_err().error_code,"MUSIC_PLAYER_UNAVAILABLE");
    let task=run(state.clone(),turn,request(&reference),CancellationToken::default());wait(&stream);
    state.with_app(|s|s.workspace.invalidate());
    assert_eq!(task.join().unwrap().unwrap_err().error_code,"WORKSPACE_STALE");
}

#[test]
fn music_demonstration_receipts_keep_main_recording_and_synthesis_clocks_separate() {
    let (root,mut state,turn,reference)=fixture();
    let path=root.path().join("music_source_manifest.json");
    let mut manifest:Value=serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    manifest["provenance"]=json!([{"provenance_id":"synthesis","kind":"score_synthesis","description":"fixture synthesis","source_url":null,"processor":"fixture"}]);
    manifest["demonstrations"]=json!([{"demonstration_id":"upper-line","title":"Upper line","kind":"score_synthesis","media_path":"demo.wav","content_type":"audio/wav","duration_ms":500,"part_ids":["piano"],"score_object_ids":["note1"],"provenance_id":"synthesis"}]);
    manifest["events"]=json!([{"event_id":"upper-line-event","title":"Upper line","explanation":"A synthesized upper line","listen_for":"Follow the upper line.",
        "start_ms":0,"end_ms":1000,"roles":[{"part_id":"piano","role":"theme"}],"evidence":[{"provenance_id":"synthesis","score_object_ids":["note1"],"audio":null}],"relations":[],"concept_tags":[]}]);
    std::fs::write(path,manifest.to_string()).unwrap();
    let stream=start(&mut state,&turn);let state=Arc::new(Mutex::new(state));
    let task=run(state.clone(),turn.clone(),request(&reference),CancellationToken::default());let command=wait(&stream);
    post_music(&state,&claim(&command));
    assert_eq!(post_music(&state,&complete(&command,"observed",true)).status,200);
    let observed=task.join().unwrap().unwrap();
    let act=MusicInteractRequest::Act {publication:reference.clone(),recording:observed.recording,based_on:observed.receipt_id,action:MusicAction::Demonstration{demonstration_id:"upper-line".into(),cue_event_id:Some("upper-line-event".into())}};
    let task=run(state.clone(),turn.clone(),act,CancellationToken::default());let command=wait(&stream);
    post_music(&state,&claim(&command));
    let mut result=complete(&command,"applied",true);
    result["receipt"]["current_ms"]=json!(1500);result["receipt"]["occurrence_id"]=json!("repeat");
    result["receipt"]["demonstration_id"]=json!("upper-line");
    result["receipt"]["displayed_cue"]=json!({"event_id":"upper-line-event","text":"Follow the upper line."});
    result["receipt"]["demonstration_state"]=json!({"current_ms":100,"paused":true});
    assert_ne!(post_music(&state,&result).status,200,"a paused demonstration has not started");
    result["receipt"]["demonstration_state"]=json!({"current_ms":600,"paused":false});
    assert_ne!(post_music(&state,&result).status,200,"demo time must fit its own duration");
    result["receipt"]["demonstration_state"]=json!({"current_ms":100,"paused":false});
    result["receipt"]["displayed_cue"]=Value::Null;
    assert_ne!(post_music(&state,&result).status,200,"the requested listening cue must actually be displayed");
    result["receipt"]["displayed_cue"]=json!({"event_id":"upper-line-event","text":"Unpublished instructions"});
    assert_ne!(post_music(&state,&result).status,200,"cue text must come from the published event");
    result["receipt"]["displayed_cue"]=json!({"event_id":"upper-line-event","text":"Follow the upper line."});
    assert_eq!(post_music(&state,&result).status,200,"the original recording can be paused beyond the short demonstration duration");
    task.join().unwrap().unwrap();

    let task=run(state.clone(),turn.clone(),request(&reference),CancellationToken::default());let command=wait(&stream);
    post_music(&state,&claim(&command));
    result["command_id"]=json!(command.command_id);result["receipt"]["command_id"]=json!(command.command_id);
    result["receipt"]["status"]=json!("observed");result["receipt"]["demonstration_state"]=json!({"current_ms":500,"paused":true});
    assert_eq!(post_music(&state,&result).status,200,"an ended demonstration remains observable");
    let observed=task.join().unwrap().unwrap();
    let act=MusicInteractRequest::Act {publication:reference,recording:observed.recording,based_on:observed.receipt_id,action:MusicAction::Play{start_ms:None,end_ms:None,cue_event_id:None}};
    let task=run(state.clone(),turn,act,CancellationToken::default());let command=wait(&stream);
    post_music(&state,&claim(&command));
    result["command_id"]=json!(command.command_id);result["receipt"]["command_id"]=json!(command.command_id);
    result["receipt"]["status"]=json!("applied");result["receipt"]["paused"]=json!(false);
    result["receipt"]["loop"]=json!({"start_ms":1000,"end_ms":2000});
    assert_eq!(post_music(&state,&result).status,200,"resuming the original recording may preserve its loop and an inactive demo panel");
    task.join().unwrap().unwrap();
}

#[test]
fn music_question_freezes_repeat_and_original_time_in_run_and_history() {
    let (_root,mut state,_,reference)=fixture();
    let context=json!(serde_json::from_value::<base_schema::MusicQuestionContext>(json!({"draft_id":"question1","publication":reference,"audio":{"recording":{"recording_id":"recording","revision":"1"},"start_ms":1200,"end_ms":1500},
        "score":{"score":{"score_id":"score","revision":"1"},"source_ids":["note1"]},"occurrence_id":"repeat","focused_part_ids":["piano"]})).unwrap());
    let input=json!({"message":"为什么这里变了？","music_question":context});
    let prepared=prepare_agent_chat(&mut state,&input.to_string(),"now").map_err(|r|r.body).unwrap();
    assert_eq!(prepared.scope.music_question.as_ref().unwrap().occurrence_id.as_deref(),Some("repeat"));
    let source=crate::music_interact::source_context(&prepared.scope).unwrap().unwrap();
    assert_eq!(source["question_context"],context);
    let turn=state.user.agent_history.sessions.iter().flat_map(|s|&s.turns).find(|t|t.turn_id==prepared.turn_ref.turn_id).unwrap();
    assert_eq!(json!(turn.music_question),context);
    assert_eq!(crate::authorization::turn_json(turn)["music_question"],context);
    let mut moved=input.clone();moved["music_question"]["audio"]["start_ms"]=json!(1300);
    assert_ne!(crate::run_admission::normalize("workspace",&input).unwrap(),crate::run_admission::normalize("workspace",&moved).unwrap(),"a retry key cannot silently change its music question");
    assert_eq!(crate::music_interact::source_context(&prepared.scope).unwrap().unwrap()["question_context"]["audio"]["start_ms"].as_f64(),Some(1200.0));
    let mut wrong=input;wrong["music_question"]["publication"]["publication_id"]=json!("newer");
    assert!(crate::music_interact::question(&state.workspace,&wrong).is_err());
}

#[test]
fn music_question_pixels_survive_admission_and_history_without_an_indexed_score_target() {
    use base64::Engine;
    let (_root, mut state, _, reference) = fixture();
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(png::ColorType::Rgb);
        encoder.write_header().unwrap().write_image_data(&[255, 0, 0, 0, 255, 0]).unwrap();
    }
    let image = json!({"png_base64":base64::engine::general_purpose::STANDARD.encode(bytes),"width":2,"height":1,"origin":"selection"});
    let input = json!({"message":"这条线是什么意思？","music_question":{
        "draft_id":"staff-lines","publication":reference,
        "audio":{"recording":{"recording_id":"recording","revision":"1"},"start_ms":1200,"end_ms":1500},
        "score":null,"occurrence_id":null,"focused_part_ids":[],"image":image}});
    let prepared = prepare_agent_chat(&mut state, &input.to_string(), "now").map_err(|r|r.body).unwrap();
    assert_eq!(json!(prepared.scope.music_question.as_ref().unwrap().image), image);
    let turn = state.user.agent_history.sessions.iter().flat_map(|s| &s.turns).find(|t| t.turn_id == prepared.turn_ref.turn_id).unwrap();
    assert_eq!(crate::authorization::turn_json(turn)["music_question"]["image"], image);
    let mut invalid = input.clone(); invalid["music_question"]["image"]["width"] = json!(3);
    assert_eq!(crate::music_interact::question(&state.workspace, &invalid).unwrap_err().error_code, "MUSIC_QUESTION_INVALID");
    invalid["music_question"]["image"]["width"] = json!(2);
    invalid["music_question"]["image"]["png_base64"] = json!("broken");
    assert!(crate::music_interact::question(&state.workspace, &invalid).is_err());
}

#[test]
fn knowledge_tree_keeps_score_only_music_understanding_at_the_material_root() {
    let f = super::mu5_tests::Fixture::new();
    let input = f.root.path().join("music-understanding-root");
    super::music_publication_tests::fixture(&input);
    let publication = f.access.library.lock().unwrap().publish(&input).unwrap().reference;
    f.access.library.lock().unwrap().grant("A", &publication).unwrap();
    let workspace = f.create(&f.a, &publication, "music-understanding");
    let workspace = f.action(&f.a, &workspace, "music-understanding", "chat/new", json!({}));
    let mut port = f.run(&workspace, "music-understanding");
    let turn = AgentTurnRef { session_id: port.scope.chat_session_id.clone(),
        turn_id: port.scope.turn_id.clone(), user_turn_ordinal: 1 };
    let revision = port.scope.reading_book().source_fingerprint().to_owned();
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let mut evidence_ids = Vec::new();
    for (kind, request) in [("score", json!({"view":"score","source_ids":["s1"]})),
        ("event", json!({"view":"events","event_id":"event"}))] {
        let material = port.read_music(None, serde_json::from_value(request).unwrap()).unwrap();
        let source = material.sources[0].clone();
        assert_eq!(source.target.event_ids.is_empty(), kind == "score");
        let binding = SourceBinding {
            music: Some(source.clone()), media: None, source_ref_id: format!("music-{kind}"),
            book_id: publication.book_id.clone(), publication_id: Some(publication.publication_id.clone()),
            source_revision: Some(revision.clone()), evidence_range: None, evidence_text_digest: String::new(),
            label_snapshot: source.label.clone(), preview_snapshot: source.preview.clone(),
        };
        let mut user = handle.lock().unwrap();
        crate::session_runtime::append(&mut user, &turn, "source",
            crate::session_event::EventBody::SourcesBound { bindings: vec![binding.clone()] }).unwrap();
        let private = port.scope.private_user(&user);
        let hypothesis = crate::cross_book::learning(&private, &turn, &publication, &json!({
            "operation":"observe","operation_id":format!("understanding-{kind}"),
            "source_ref_ids":[binding.source_ref_id],"learner_quote":"Explain this",
            "learning_focus":format!("understanding {kind}"),"expected_performance":"Explain the original source",
            "capability":"explanation","interpretation":"The learner requested an explanation.",
            "teaching_implication":"Compare the original notation and recording."}), &[binding], "now").unwrap();
        evidence_ids.push(hypothesis["evidence_ref"].clone());
    }
    let workspace = f.get(&f.a, &workspace);
    let root = f.action(&f.a, &workspace, "music-understanding", "knowledge/tree", json!({"node_id":"root"}));
    let fruits = root["result"]["fruits"].as_array().unwrap();
    assert_eq!(fruits.len(), 2);
    for id in &evidence_ids {
        assert!(fruits.iter().any(|item| item["evidence_ref"] == *id));
    }
    let event = f.action(&f.a, &root, "music-understanding", "knowledge/tree", json!({"reference":{
        "source_id":publication.book_id,"source_revision":revision,"publication_id":publication.publication_id,
        "artifact":"music_event","entry_id":"event"}}));
    let fruits = event["result"]["fruits"].as_array().unwrap();
    assert_eq!(fruits.len(), 1, "score-only understanding must not be assigned to a nearby event");
    assert_eq!(fruits[0]["evidence_ref"], evidence_ids[1]);
}

#[test]
fn music_network_receipt_releases_workspace_locks_and_sources_keep_original_publication() {
    let f = super::mu5_tests::Fixture::new();
    let input=f.root.path().join("music-player");
    super::music_publication_tests::fixture(&input);
    let publication=f.access.library.lock().unwrap().publish(&input).unwrap().reference;
    f.access.library.lock().unwrap().grant("A",&publication).unwrap();
    let workspace=f.create(&f.a,&publication,"music-window");
    let workspace=f.action(&f.a,&workspace,"music-window","chat/new",json!({}));
    let mut port=f.run(&workspace,"music-window");
    let material=port.read_music(None,serde_json::from_value(json!({"view":"events","event_id":"event"})).unwrap()).unwrap();
    assert!(!material.heard_audio);
    let saved=material.sources[0].clone();
    let turn=AgentTurnRef {session_id:port.scope.chat_session_id.clone(),turn_id:port.scope.turn_id.clone(),user_turn_ordinal:1};
    let handle=f.access.users.lock().unwrap().get("A","now").unwrap();
    let binding=SourceBinding {
        music:Some(saved.clone()),media:None,source_ref_id:"music-source".into(),book_id:publication.book_id.clone(),publication_id:Some(publication.publication_id.clone()),source_revision:Some(port.scope.reading_book().source_fingerprint().into()),
        evidence_range:None,evidence_text_digest:String::new(),label_snapshot:saved.label.clone(),preview_snapshot:saved.preview.clone(),
    };
    {
        let mut user=handle.lock().unwrap();
        crate::session_runtime::append(&mut user,&turn,"source",crate::session_event::EventBody::SourcesBound{bindings:vec![binding.clone()]}).unwrap();
        let private=port.scope.private_user(&user);
        let hypothesis=crate::cross_book::learning(&private,&turn,&publication,&json!({"operation":"observe","operation_id":"music-understanding",
            "source_ref_ids":["music-source"],"learner_quote":"Explain this","learning_focus":"repeated phrase","expected_performance":"Locate its return","capability":"explanation",
            "interpretation":"The learner requested explanation; understanding is not yet observed.","teaching_implication":"Compare the two original occurrences without claiming mastery."}),&[binding],"now").unwrap();
        assert_eq!(hypothesis["nature"],"hypothesis");
        let evidence=private.user.learning_store().unwrap().evidence(hypothesis["evidence_ref"].as_str().unwrap()).unwrap();
        assert_eq!(evidence.target.unwrap().music_bindings.unwrap()[0].target,saved.target);
    }
    let other=f.access.users.lock().unwrap().get("B","now").unwrap();
    assert!(other.lock().unwrap().learning_store().unwrap().teaching_recent(&format!("observation:{}",publication.book_id),12).unwrap().is_empty());
    let stream=RunStream::new(RunDescriptor {book_id:publication.book_id.clone(),session_id:turn.session_id.clone(),turn_id:turn.turn_id.clone()});
    port.observe(&stream);
    let observe=request(&publication);
    let task=std::thread::spawn(move||port.interact_music(observe,&CancellationToken::default()));
    let command=wait(&stream);
    let claimed=f.action(&f.a,&workspace,"music-window","music/respond",claim(&command));
    assert_eq!(claimed["result"]["claimed"],true,"ack must acquire workspace locks while the tool waits");
    let mut done=complete(&command,"observed",true);done["receipt"]["occurrence_id"]=json!("o1");
    let done=f.action(&f.a,&claimed,"music-window","music/respond",done);
    assert_eq!(done["result"]["accepted"],true);
    assert_eq!(task.join().unwrap().unwrap().status,MusicReceiptStatus::Observed);
    let source=f.action(&f.a,&done,"music-window","agent/source.resolve",json!({"turn_id":turn.turn_id,"source_ref_id":"music-source"}));
    assert_eq!(source["result"]["music"],json!(saved));
    assert_eq!(source["result"]["media_url"],publication.url("music/audio"));
    assert!(source["result"]["music_score_pages"].as_array().is_some_and(|p|!p.is_empty()));
}

#[test]
fn music_shared_prebuild_and_two_readers_keep_existing_cost_owners() {
    use runtime::model_spend::{ChargeAttribution,ChargeScope,SendEvidence,SendIdentity,SendOutcome};
    use runtime::provider_stream::ModelUsage;
    let mut f=super::mu5_tests::Fixture::new();
    super::adm5_tests::seed(&mut f,"fixture",10_000);
    let now=crate::multi_user_host::now();
    let period=f.ok(&f.b,"POST","/api/admin/users/B/allowance-periods",json!({"operation_id":"music-b-period","starts_at":now-100,"expires_at":now+3600}))["allowance_period"]["period_id"].clone();
    f.ok(&f.b,"POST","/api/admin/users/B/allowance-adjustments",json!({"operation_id":"music-b-grant","period_id":period,"revision":0,"delta_micro_cny":20_000,"reason":"fixture"}));
    let input=f.root.path().join("shared-music-cost");super::music_publication_tests::fixture(&input);
    let publication=f.access.library.lock().unwrap().publish(&input).unwrap().reference;
    for owner in ["A","B"] {
        f.access.library.lock().unwrap().grant(owner,&publication).unwrap();
        f.control.connection.execute("INSERT INTO learning_workspaces(owner_user_id,learning_workspace_id,created_at,updated_at) VALUES(?,?,0,0)",rusqlite::params![owner,format!("music-{owner}")]).unwrap();
    }
    let request=json!({"model":"fixture-model","messages":[{"role":"user","content":"music contract fixture"}],"max_tokens":20});
    let send=|call:&str,scope:ChargeScope,purpose:&str,tokens:u32,parent:&str,workspace:Option<&str>| {
        let port=match &scope {
            ChargeScope::OperatorTask{task_ref}=>f.access.spend.operator_port(task_ref.clone(),Default::default()),
            _=>f.access.spend.port_with_attribution(scope.clone(),publication.clone(),ChargeAttribution {parent_task_ref:parent.into(),learning_workspace_id:workspace.map(str::to_owned),related_learning_workspace_ids:vec![]},Default::default()),
        };
        let identity=SendIdentity {call_id:call.into(),logical_call_id:call.into(),attempt:1,scope:Some(scope),purpose:purpose.into(),model:"fixture-model".into(),provider:"fixture".into()};
        port.before_send(&identity,&request).unwrap();
        port.after_send(&identity,&SendOutcome {evidence:SendEvidence::Response,usage:Some(ModelUsage {input_tokens:Some(tokens),cached_input_tokens:Some(0),output_tokens:Some(0),total_tokens:Some(tokens),..Default::default()}),succeeded:true}).unwrap();
    };
    send("music-prebuild",ChargeScope::OperatorTask{task_ref:"shared-music-build".into()},"prebuild",40,"shared-music-build",None);
    for (owner,tokens) in [("A",10),("B",15)] {
        let task=format!("music-run-{owner}");let workspace=format!("music-{owner}");
        send(&task,ChargeScope::ReaderRun{user_id:owner.into(),run_ref:task.clone()},"outer",tokens,&task,Some(&workspace));
        send(&format!("music-coach-{owner}"),ChargeScope::ReaderTask{user_id:owner.into(),task_ref:format!("tutor-coach:{task}")},"tutor_coach",2,&task,Some(&workspace));
    }
    for (call,owner,cost) in [("music-prebuild",None,40),("music-run-A",Some("A"),10),("music-coach-A",Some("A"),2),("music-run-B",Some("B"),15),("music-coach-B",Some("B"),2)] {
        let charge=f.ok(&f.b,"GET",&format!("/api/admin/charges/{call}"),json!({}));
        assert_eq!(charge["user_id"].as_str(),owner);assert_eq!(charge["provider_cost_micro_cny"],cost);
        assert_eq!(charge["account_debit_micro_cny"],if owner.is_some(){cost}else{0});
        assert_eq!(charge["cost_owner"],if owner.is_some(){"account"}else{"platform"});
        if let Some(owner)=owner {
            assert_eq!(charge["parent_task_ref"],format!("music-run-{owner}"));
            assert_eq!(charge["learning_workspace_id"],format!("music-{owner}"));
        }
    }
    let a=f.access.library.lock().unwrap().load("A",&publication).unwrap();
    let b=f.access.library.lock().unwrap().load("B",&publication).unwrap();
    assert!(Arc::ptr_eq(&a.book,&b.book));
    assert_eq!(f.control.connection.query_row("SELECT count(*) FROM model_call_charges WHERE cost_owner='platform'",[],|row|row.get::<_,u64>(0)).unwrap(),1,"reusing the published recording never creates another prebuild charge");
    assert_eq!(f.ok(&f.a,"GET","/api/account/usage",json!({}))["total"],2);
    assert_eq!(f.ok(&f.b,"GET","/api/account/usage",json!({}))["total"],2);
}
