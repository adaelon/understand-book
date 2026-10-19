//! Version binding tests use real private storage. Provider/preview acceptance below
//! separately exercises actual request assembly with controlled model outputs.
use super::*;
use runtime::presentation::*;
use std::cell::RefCell;

fn invoke(port: &mut impl ResidentStatePort, args: Value) -> Result<AuthorResult, ToolError> {
    port.author_presentation(serde_json::from_value(args).unwrap(), &[], &[], &CancellationToken::default())
}

fn write_args() -> Value {
    json!({"operation":"write","title":"Update","html":"<p>old local relation</p>","readable_content":"A local relation."})
}

fn se3_setup(name: &str) -> (tempfile::TempDir, AppState, AgentTurnRef) {
    let root = tempfile::tempdir().unwrap();
    let mut state = state_named(name);
    state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
    state.user.history_path = Some(root.path().join("history.json"));
    let book = state.workspace.book.base.book_id.clone();
    let turn = precommit_agent_turn(&mut state, &book, "method provenance".into(), None, None, None, "2026-10-16T00:00:00Z").unwrap();
    (root, state, turn)
}

#[test]
fn se3_method_use_requires_registered_observed_identity() {
    let (_root, mut state, turn) = se3_setup("se3-method-use");
    let app = BorrowedAppPort(RefCell::new(&mut state));
    let scope = app.with_app(|s| crate::run_scope::RunScope::capture(s, &turn, "se3", None, None));
    let mut port = RuntimeStatePort { scope:&scope, port:&app, turn_ref:&turn,
        previewed:Default::default(), animations:Default::default(), plots:Default::default() };
    let mut args = write_args();
    args["method_refs"] = json!([{"method_id":"missing","revision":1}]);
    assert_eq!(invoke(&mut port,args.clone()).err().unwrap().error_code, "PRESENTATION_METHOD_NOT_FOUND");
    for method in runtime::presentation_methods::catalog()["methods"].as_array().unwrap() {
        args["method_refs"] = json!([{"method_id":method["id"],"revision":method["revision"]}]);
        port.record_presentation_methods(&PresentationMethodObservations { catalog:Some(runtime::presentation_methods::catalog().clone()), reads:vec![] }).unwrap();
        assert!(invoke(&mut port,args.clone()).err().unwrap().message.contains("subsequent sampling"));
        let read = invoke(&mut port,json!({"operation":"read_method","method_id":method["id"],"revision":method["revision"]})).unwrap();
        assert!(invoke(&mut port,args.clone()).is_err(), "executed but unobserved source cannot justify use");
        // Simulate the Provider-observation boundary for this storage contract test.
        port.record_presentation_methods(&PresentationMethodObservations { catalog:None, reads:vec![PresentationMethodRead {
            tool_call_id:"observed-card".into(),method_id:method["id"].as_str().unwrap().into(),revision:method["revision"].as_u64().unwrap(),
            file:"card.md".into(),offset:0,characters:read.body["text"].as_str().unwrap().chars().count(),
            end_offset:read.body["end_offset"].as_u64().unwrap() as usize,total_characters:read.body["total_characters"].as_u64().unwrap() as usize,
        }] }).unwrap();
        let saved = invoke(&mut port,args.clone()).unwrap();
        assert_eq!(saved.body["method_refs"][0]["basis"]["kind"], "read");
        let id = saved.body["candidate_id"].as_str().unwrap();
        let candidate = app.with_app(|s| s.private_context().read_presentation_candidate(&turn.session_id,id).unwrap());
        assert_eq!(candidate.content.method_observations.reads[0].file, "card.md");
        assert!(candidate.content.method_observations.reads.iter().all(|r| r.file == "card.md"), "reading cards does not mean reading code");
        let mut duplicate = args.clone();
        duplicate["method_refs"].as_array_mut().unwrap().push(args["method_refs"][0].clone());
        assert!(invoke(&mut port,duplicate).err().unwrap().message.contains("Duplicate"));
        let mut wrong_revision = args.clone(); wrong_revision["method_refs"][0]["revision"] = json!(999);
        assert_eq!(invoke(&mut port,wrong_revision).err().unwrap().error_code,"PRESENTATION_METHOD_NOT_FOUND");
    }
}

#[test]
fn se3_patch_rewrite_legacy_and_retired_versions_keep_exact_provenance() {
    let (_root, mut state, turn) = se3_setup("se3-method-inheritance");
    let app = BorrowedAppPort(RefCell::new(&mut state));
    let scope = app.with_app(|s| crate::run_scope::RunScope::capture(s,&turn,"se3",None,None));
    let mut port = RuntimeStatePort { scope:&scope,port:&app,turn_ref:&turn,
        previewed:Default::default(),animations:Default::default(),plots:Default::default() };
    let first = invoke(&mut port,write_args()).unwrap();
    let first_id = first.body["candidate_id"].as_str().unwrap();
    let mut content = app.with_app(|s| s.private_context().read_presentation_candidate(&turn.session_id,first_id).unwrap().content);
    assert_eq!(content.method_refs,Some(vec![]));
    // A genuine pre-SE3 payload has neither field. It must remain unrecorded.
    let mut legacy = serde_json::to_value(&content).unwrap();
    legacy.as_object_mut().unwrap().remove("method_refs");
    legacy.as_object_mut().unwrap().remove("method_observations");
    let legacy:PresentationContent = serde_json::from_value(legacy).unwrap();
    assert!(legacy.method_refs.is_none());
    let legacy_ref = app.with_app(|s| {
        let candidate=s.private_context().create_presentation_candidate(&turn.session_id,&turn.turn_id,None,legacy).unwrap();
        s.private_context().persist_presentation_candidate(&turn.session_id,&turn.turn_id,&candidate.candidate_id).unwrap()
    });
    assert!(invoke(&mut port,json!({"operation":"read","reference":legacy_ref})).unwrap().body["method_refs"].is_null());
    let patch = |target:Value, refs:Option<Value>| {
        let mut args=json!({"operation":"patch","edits":[{"old_text":"old","new_text":"revised"}]});
        args.as_object_mut().unwrap().extend(target.as_object().unwrap().clone());
        if let Some(refs)=refs {args["method_refs"]=refs;}
        args
    };
    assert!(invoke(&mut port,patch(json!({"reference":legacy_ref}),None)).unwrap().body["method_refs"].is_null());
    // Historical method removed from the current catalog, retained in an exact version.
    content.method_refs=Some(["left","right"].into_iter().map(|scope| PresentationMethodUse {
        method:PresentationMethodRef {method_id:"retired".into(),revision:7,scope:Some(scope.into())},
        basis:PresentationMethodBasis::Read {turn_id:"original-turn".into(),tool_call_ids:vec!["original-read".into()]},
    }).collect());
    let base_ref=app.with_app(|s| {
        let candidate=s.private_context().create_presentation_candidate(&turn.session_id,&turn.turn_id,None,content.clone()).unwrap();
        s.private_context().persist_presentation_candidate(&turn.session_id,&turn.turn_id,&candidate.candidate_id).unwrap()
    });
    let inherited=invoke(&mut port,patch(json!({"reference":base_ref}),None)).unwrap();
    assert_eq!(inherited.body["method_refs"].as_array().unwrap().len(),2);
    assert_eq!(inherited.body["method_refs"][0]["basis"],json!({"kind":"version","reference":base_ref}));
    let child=invoke(&mut port,json!({"operation":"patch","candidate_id":inherited.body["candidate_id"],"edits":[{"old_text":"revised","new_text":"replaced"}],"method_refs":[{"method_id":"retired","revision":7,"scope":"right"}]})).unwrap();
    assert_eq!(child.body["method_refs"].as_array().unwrap().len(),1);
    assert_eq!(child.body["method_refs"][0]["basis"],json!({"kind":"candidate","candidate_id":inherited.body["candidate_id"]}));
    let mut rewrite=write_args();rewrite["based_on"]=json!(base_ref);
    assert!(invoke(&mut port,rewrite.clone()).err().unwrap().message.contains("explicitly redeclare"));
    rewrite["method_refs"]=json!([{"method_id":"retired","revision":7,"scope":"new location"}]);
    assert_eq!(invoke(&mut port,rewrite.clone()).unwrap().body["method_refs"][0]["basis"]["kind"],"version");
    rewrite["method_refs"]=json!([]);
    assert_eq!(invoke(&mut port,rewrite).unwrap().body["method_refs"],json!([]));
    app.with_app(|s| {
        let private=s.private_context();
        let reference=private.persist_presentation_candidate(&turn.session_id,&turn.turn_id,child.body["candidate_id"].as_str().unwrap()).unwrap();
        assert_eq!(reference.revision,base_ref.revision+1);
        assert_eq!(private.read_presentation(&turn.session_id,&base_ref).unwrap().content,content);
        let version=private.read_presentation(&turn.session_id,&reference).unwrap();
        assert_eq!(serde_json::to_value(version.content.method_refs).unwrap(),child.body["method_refs"]);
        assert_eq!(private.persist_presentation_candidate(&turn.session_id,&turn.turn_id,child.body["candidate_id"].as_str().unwrap()).unwrap(),reference);
        assert_eq!(private.read_presentation_candidate(&turn.session_id,inherited.body["candidate_id"].as_str().unwrap()).unwrap().content.method_refs.unwrap().len(),2);
    });
}

struct MethodProvider {
    calls: RefCell<std::collections::VecDeque<Value>>,
    plans: RefCell<Vec<AgentRequestPlan>>,
    seen: RefCell<std::collections::HashSet<String>>,
    source: RefCell<std::collections::BTreeMap<String,String>>,
    candidate: RefCell<Option<String>>,
    delivered: RefCell<Option<PresentationRef>>,
}
impl ModelAdapter for MethodProvider {
    fn complete(&self,_:CompletionRequest)->Result<ParsedResponse,AdapterError>{unreachable!()}
    fn chat(&self,plan:&AgentRequestPlan)->Result<AssistantTurn,AdapterError>{
        self.plans.borrow_mut().push(plan.clone());
        for message in plan.ordered_messages() {
            if message.role!=runtime::Role::Tool {continue;}
            let Some(id)=message.tool_call_id else {continue;};
            if !self.seen.borrow_mut().insert(id) {continue;}
            let envelope:Value=serde_json::from_str(message.content.as_deref().unwrap()).unwrap();
            assert_ne!(envelope["status"],"error","{envelope}");
            let body=&envelope["model_body"];
            assert!(body.get("error_code").is_none(),"{body}");
            match body["status"].as_str() {
                Some("method_read")=>{
                    let mut sources=self.source.borrow_mut();
                    let source=sources.entry(body["file"].as_str().unwrap().into()).or_default();
                    assert_eq!(body["offset"].as_u64().unwrap() as usize,source.chars().count());
                    source.push_str(body["text"].as_str().unwrap());
                },
                Some("candidate_saved")=>*self.candidate.borrow_mut()=Some(body["candidate_id"].as_str().unwrap().into()),
                Some("version_saved")=>*self.delivered.borrow_mut()=Some(serde_json::from_value(body["reference"].clone()).unwrap()),
                Some("preview_environment_recorded"|"preview_ready_for_inspection")=>{
                    assert_eq!(body["errors"],json!([]));
                    assert!(body["reading"]["text"].as_str().unwrap().contains("当前 xₖ"));
                    assert_eq!(body["reading"]["scene"]["eta"],json!(0.5));
                    assert_eq!(body["reading"]["scene"]["semantic_state"],1);
                    assert!(!plan.preview_images.is_empty(),"preview must reach a later Provider sampling");
                },
                _=>{},
            }
        }
        let next=self.calls.borrow_mut().pop_front().map(|mut args|{
            let name=if args["operation"]=="discover" {
                args=json!({"task":"adapt an explanation method","required_capabilities":["presentation_authoring"],"scope":"passage","operation":"explain","effect_mode":"read_only","max_results":1});
                "tool.search"
            }else{
                if args["operation"]=="write" {
                    let sources=self.source.borrow();
                    args["html"]=json!(sources["example/index.html"].replace("defaults: { eta: 0.25 }", "defaults: { eta: 0.5 }"));
                    let author:Value=serde_json::from_str(&sources["example/author.json"]).unwrap();
                    args["readable_content"]=author["readable_content"].clone();
                }
                if matches!(args["operation"].as_str(),Some("preview"|"deliver")) {
                    args["candidate_id"]=json!(self.candidate.borrow().as_ref().unwrap());
                }
                "presentation.author"
            };
            runtime::ToolCall {id:format!("se3-call-{}",self.plans.borrow().len()),name:name.into(),arguments:args.to_string()}
        });
        Ok(AssistantTurn {provider_continuation:None,text:next.is_none().then(||"已按 η=0.5 展示这次离散更新。".into()),tool_calls:next.into_iter().collect(),usage_total_tokens:None})
    }
}

fn provider_flow(preview:bool) {
    let root=tempfile::tempdir().unwrap();
    let mut state=state_named("se3-provider");
    state.user.store=MemoryStore::open(root.path().join("memory.json")).unwrap();
    state.user.history_path=Some(root.path().join("history.json"));
    let prepared=prepare_agent_chat(&mut state,&json!({"message":"使用离散更新方法做 D1 教学例子，固定 f(x)=(x−2)²、x₀=4，初始 η=0.5，解释一次更新到最小值。"}).to_string(),"2026-10-16T00:00:00Z").unwrap_or_else(|r|panic!("{}",r.body));
    let session_id=state.user.agent_history.sessions.last().unwrap().id.clone();
    let turn_id=state.user.agent_history.sessions.last().unwrap().turns.last().unwrap().turn_id.clone();
    let mut calls=std::collections::VecDeque::from([json!({"operation":"discover"})]);
    let method=runtime::presentation_methods::catalog()["methods"].as_array().unwrap().first();
    let mut expected_reads=0;
    if let Some(method)=method {
        for file in method["files"].as_array().unwrap() {
            let (_,source)=runtime::presentation_methods::source(method["id"].as_str().unwrap(),method["revision"].as_u64().unwrap(),file.as_str().unwrap()).unwrap();
            for offset in (0..source.chars().count()).step_by(4000) {
                calls.push_back(json!({"operation":"read_method","method_id":method["id"],"revision":method["revision"],"file":file,"offset":offset,"length":4000}));
                expected_reads+=1;
            }
        }
        calls.push_back(json!({"operation":"write","title":"η=0.5：一次更新到达最小值","html":"from observed source","readable_content":"from observed guide",
            "method_refs":[{"method_id":method["id"],"revision":method["revision"],"scope":"离散更新图与公式同步"}],
            "state_contract":{"eta":"fixed dimensionless learning rate in 0..1.2","semantic_state":"completed discrete updates, integer 0..8","transition_progress":"visual interpolation toward next update in [0,1); zero at step 8"},
            "initial_state":{"eta":0.5,"semantic_state":0,"transition_progress":0},
            "assumptions":["固定一维二次函数、学习率和初值 4，无动量与噪声；无量纲教学模型。"]}));
        if preview {
            for (_,viewport) in REQUIRED_PREVIEW_ENVIRONMENTS {
                calls.push_back(json!({"operation":"preview","candidate_id":"from write","viewport":viewport,"read_selector":"#readout","actions":[{"kind":"seek","semantic_state":1,"transition_progress":0},{"kind":"scroll","y":110}]}));
            }
            calls.push_back(json!({"operation":"deliver","candidate_id":"from write"}));
        }
    }else{assert!(!preview,"D1 acceptance requires the candidate build");}
    let adapter=MethodProvider {calls:RefCell::new(calls),plans:RefCell::new(vec![]),seen:Default::default(),source:Default::default(),candidate:Default::default(),delivered:Default::default()};
    let app=BorrowedAppPort(RefCell::new(&mut state));
    let report=crate::agent_run::execute_prepared(&app,&adapter,prepared,CancellationToken::default());
    assert_eq!(report.reply.status,200,"{}",report.reply.body);
    assert!(adapter.calls.borrow().is_empty(),"{}",report.reply.body);
    let observations=app.with_app(|s|s.private_context().presentation_method_observations(&session_id,&turn_id).unwrap());
    assert_eq!(observations.catalog.as_ref(),Some(runtime::presentation_methods::catalog()));
    assert_eq!(observations.reads.len(),expected_reads);
    let activities=app.with_app(|s| s.user.agent_history.sessions.last().unwrap().turns.last().unwrap().run_summary.as_ref().unwrap().activities.clone().unwrap());
    assert_eq!(activities.iter().filter(|a|a.name=="presentation.methods.catalog").count(),1);
    assert_eq!(activities.iter().filter(|a|a.name=="presentation.methods.read").count(),expected_reads);
    assert_eq!(activities.iter().filter(|a|a.name=="presentation.methods.declared").count(),usize::from(method.is_some()));
    for read in &observations.reads {
        let plans=adapter.plans.borrow();
        let body=plans.iter().flat_map(|p|p.ordered_messages()).find_map(|m|{
            if m.role!=runtime::Role::Tool||m.tool_call_id.as_deref()!=Some(&read.tool_call_id){return None;}
            let v:Value=serde_json::from_str(m.content.as_deref()?).ok()?;
            Some(v["model_body"].clone())
        }).unwrap();
        assert_eq!(read.characters,body["text"].as_str().unwrap().chars().count());
        assert_eq!(read.offset,body["offset"].as_u64().unwrap() as usize);
    }
    let candidate=adapter.candidate.borrow().as_ref().map(|id|app.with_app(|s|s.private_context().read_presentation_candidate(&session_id,id).unwrap()));
    if let Some(candidate)=&candidate {
        assert_eq!(candidate.content.method_observations,observations);
        let refs=candidate.content.method_refs.as_ref().unwrap();
        assert_eq!(refs.len(),1);
        assert!(matches!(&refs[0].basis,PresentationMethodBasis::Read {turn_id:origin,tool_call_ids} if origin==&turn_id && tool_call_ids.len()==expected_reads));
        assert!(candidate.content.content_files["index.html"].contains("defaults: { eta: 0.5 }"));
        for (file,source) in adapter.source.borrow().iter() {
            assert_eq!(source,runtime::presentation_methods::source(&refs[0].method.method_id,refs[0].method.revision,file).unwrap().1);
        }
    }
    let version=adapter.delivered.borrow().as_ref().map(|reference|app.with_app(|s|s.private_context().read_presentation(&session_id,reference).unwrap()));
    if preview {
        let version=version.as_ref().expect("version delivered");
        assert_eq!(version.content,candidate.as_ref().unwrap().content);
        assert_eq!(version.candidate_id,candidate.as_ref().unwrap().candidate_id);
    }
    // A fresh host reads persisted facts and content, without a live run ledger.
    let mut reopened=state_named("se3-reopened");
    reopened.user.history_path=Some(root.path().join("history.json"));
    reopened.user.agent_history=load_agent_history(&reopened.user.history_path).unwrap();
    assert_eq!(reopened.private_context().presentation_method_observations(&session_id,&turn_id).unwrap(),observations);
    if let Some(version)=&version {
        assert_eq!(reopened.private_context().read_presentation(&session_id,&version.reference).unwrap(),*version);
    }
    if let Some(output)=std::env::var_os("SE3_OUTPUT") {
        let output=std::path::PathBuf::from(output);std::fs::create_dir_all(&output).unwrap();
        let plans:Vec<_>=adapter.plans.borrow().iter().map(|p|json!({"instructions":p.instructions,"instruction_assets":p.instruction_assets,"messages":p.ordered_messages(),"preview_images":p.preview_images.iter().map(|i|json!({"candidate_id":i.candidate_id,"environment_name":i.environment_name,"caption":i.caption})).collect::<Vec<_>>()})).collect();
        for (name,value) in [("provider-inputs.json",json!(plans)),("observations.json",json!(observations)),("candidate.json",json!(candidate)),("version.json",json!(version)),("activities.json",json!(activities))] {
            std::fs::write(output.join(name),serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        }
        if preview {
            use base64::Engine as _;
            let mut last_images=std::collections::BTreeMap::new();
            for plan in adapter.plans.borrow().iter() { for image in &plan.preview_images {
                last_images.insert(image.environment_name.clone().unwrap(),image.png_base64.clone());
            }}
            for (name,png) in last_images {std::fs::write(output.join(format!("{name}.png")),base64::engine::general_purpose::STANDARD.decode(png).unwrap()).unwrap();}
        }
    }
    eprintln!("SE3: {} actual requests, {} observed ranges, candidate={}, delivered={}",adapter.plans.borrow().len(),expected_reads,candidate.is_some(),version.is_some());
}

#[test]
fn se3_provider_records_actual_inputs_and_candidate_binding(){provider_flow(false);}

#[test]
#[ignore="requires Chromium/Edge, the candidate catalog and a fresh SE3_OUTPUT directory"]
fn se3_d1_reads_adapts_previews_and_delivers(){provider_flow(true);}
