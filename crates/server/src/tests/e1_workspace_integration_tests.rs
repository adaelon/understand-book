use super::*;
use runtime::run_context::ResidentStatePort;
use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use base64::Engine;

// Real provider HTTP/usage and the ordinary RunAdmission tool loop. Only the
// semantic responses are scripted; no test-owned implementation of product tools.
struct Provider {
    url: String,
    responses: Arc<Mutex<VecDeque<Value>>>,
    requests: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Provider {
    fn new() -> Self {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr().to_ip().unwrap());
        let responses = Arc::new(Mutex::new(VecDeque::<Value>::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (queue, observed, done) = (responses.clone(), requests.clone(), stop.clone());
        let worker = std::thread::spawn(move || while !done.load(Ordering::SeqCst) {
            let Some(mut request) = server.recv_timeout(Duration::from_millis(100)).unwrap() else { continue };
            let mut body = String::new(); request.as_reader().read_to_string(&mut body).unwrap();
            observed.lock().unwrap().push(serde_json::from_str(&body).unwrap());
            let message = queue.lock().unwrap().pop_front().expect("unexpected extra provider call");
            request.respond(tiny_http::Response::from_string(json!({
                "choices":[{"message":message,"finish_reason":"stop"}],
                "usage":{"prompt_tokens":9,"prompt_cache_hit_tokens":0,"completion_tokens":1,"total_tokens":10}
            }).to_string()).with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
        });
        Self { url, responses, requests, stop, worker:Some(worker) }
    }
    fn configure(&self, f: &Fixture) {
        f.access.runs.configure(ProviderConfig::from_values("native","fixture-key",self.url.clone(),"fixture-model").unwrap()).unwrap();
    }
    fn text(&self, text: &str) { self.responses.lock().unwrap().push_back(json!({"role":"assistant","content":text})); }
    fn tool(&self, name: &str, arguments: Value) {
        self.responses.lock().unwrap().push_back(json!({"role":"assistant","content":null,"tool_calls":[{
            "id":format!("e1-{}",uuid::Uuid::now_v7()),"type":"function","function":{"name":name,"arguments":arguments.to_string()}
        }]}));
    }
}
impl Drop for Provider {
    fn drop(&mut self) { self.stop.store(true,Ordering::SeqCst); if let Some(worker)=self.worker.take(){worker.join().unwrap();} }
}
fn fixture(provider: &Provider) -> (Fixture, String) {
    let mut f=Fixture::new();
    let period=super::super::adm5_tests::seed(&mut f,&provider.url,730_000);
    f.ok(&f.b,"POST","/api/admin/users/A/allowance-basis",json!({"operation_id":"e1-basis","period_id":period,"revision":1,"basis":{"version":"e1-basis","basis_micro_cny":1_000_000}}));
    provider.configure(&f); (f,period)
}
fn space(f:&Fixture,goal:&str)->String {
    f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":goal},"members":[f.x,f.y]}))["learning_workspace_id"].as_str().unwrap().into()
}
fn scene(f:&Fixture,scope:&str,page:&str)->Value {
    let w=f.ok(&f.a,"POST","/api/workspaces",json!({"published_book_ref":f.x,"attachment_id":page,"learning_workspace_id":scope}));
    f.action(&f.a,&w,page,"chat/new",json!({}))
}
fn send(f:&Fixture,w:&Value,page:&str,key:&str)->Value {
    let mut input=Fixture::stamp(w,page);input["session_id"]=w["selected_chat"].clone();input["client_request_id"]=json!(key);input["message"]=json!("我把速度与变化混在了一起，请结合两份资料解释。");
    let (status,result)=admit(f,w,&input);assert_eq!(status,202,"{result}");result
}
fn evidence(f:&Fixture,w:&Value,name:&str)->String {
    super::w6_learning_records_tests::reference_learning(f,w["selected_chat"].as_str().unwrap(),name).0
}
fn report(case:&str,value:Value) {
    let directory=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/performance/workspace-e1-20261018");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join(format!("scenario-{case}.json")),serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}

#[test]
fn e1_a_two_workspaces_tool_loop_private_revision_background_and_restart() {
    let provider=Provider::new();let(f,_)=fixture(&provider);
    let (a,b)=(space(&f,"学习深度学习"),space(&f,"期末考试冲刺"));
    let w=scene(&f,&a,"e1-a");
    provider.tool("book.text",json!({"lid":"1.1"}));
    provider.tool("library.read",json!({"published_book_ref":f.y,"operation":"text","lid":"1.1"}));
    provider.text("可以把书中的速度和另一份资料中的变化联系起来理解。");
    let accepted=send(&f,&w,"e1-a","e1-a-first");
    let switched=f.action(&f.a,&f.get(&f.a,&w),"e1-a","book/open",json!({"published_book_ref":f.x,"learning_workspace_id":b}));
    f.ok(&f.a,"POST",&format!("/api/learning-workspaces/{a}/goal"),json!({"goal":{"text":"实现梯度并解释变化"}}));
    assert!(f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(f.get(&f.a,&switched)["learning_workspace_id"],b);
    let id=evidence(&f,&w,"e1-a-observation");
    let handle=context(&f,&f.a).user;
    {
        let user=handle.lock().unwrap();
        let mut first=user.learning_store_for_workspace(Some(&a)).unwrap();
        first.correct_evidence(&id,"e1-a-correction","我需要区分速度与加速度").unwrap();
        assert!(user.learning_store_for_workspace(Some(&b)).unwrap().evidence(&id).is_err());
        let session=user.agent_history.sessions.iter().find(|s|s.id==w["selected_chat"]).unwrap();
        assert_eq!(session.turns[0].domain.scene.as_ref().unwrap().workspace_goal.as_ref().unwrap().revision,1);
        assert_eq!(session.turns[0].status,AgentAssistantStatus::Completed,"{:?}",session.turns[0].error);
    }
    provider.text("{\"candidate_facts\":[],\"intent_observations\":[]}");
    assert_eq!(crate::multi_user_review::tick(&f.access,&Default::default(),(multi_user_host::now() as u64+65)*1000),1);
    let charges=f.ok(&f.b,"GET","/api/admin/charges?user_id=A&limit=100",Value::Null);
    let rows=charges["items"].as_array().unwrap();assert_eq!(rows.len(),4);
    let review=rows.iter().find(|c|c["purpose"]=="memory_review").unwrap();
    assert!(review["learning_workspace_id"].is_null());assert_eq!(review["related_learning_workspace_ids"],json!([a]));
    assert!(rows.iter().filter(|c|c["purpose"]!="memory_review").all(|c|c["learning_workspace_id"]==a&&c["parent_task_ref"]==accepted["turn_id"]));
    assert_eq!(provider.requests.lock().unwrap().len(),4);
    drop(handle);
    let f=reopen(f,&Probe::default());provider.configure(&f);
    let attached=f.ok(&f.a,"POST","/api/workspaces",json!({"published_book_ref":f.x,"attachment_id":"e1-a","learning_workspace_id":b}));
    let restored=f.action(&f.a,&attached,"e1-a","book/open",json!({"published_book_ref":f.y,"learning_workspace_id":a}));
    assert_eq!(restored["selected_chat"],w["selected_chat"]);
    let history=f.action(&f.a,&restored,"e1-a","chat/history",json!({}));assert_eq!(history["result"]["current"]["turns"].as_array().unwrap().len(),1);
    provider.text("继续按新的工作区目标解释梯度。");send(&f,&restored,"e1-a","e1-a-after-restart");assert!(f.access.runs.run_one(&f.access).unwrap());
    assert!(provider.requests.lock().unwrap().last().unwrap().to_string().contains("实现梯度并解释变化"));
    assert_eq!(context(&f,&f.a).user.lock().unwrap().learning_store_for_workspace(Some(&a)).unwrap().evidence("correction:e1-a-correction").unwrap().supersedes.as_deref(),Some(id.as_str()));
    report("a",json!({"status":"passed","provider_calls":5,"same_material_two_workspaces":true,"frozen_goal_then_new_goal":true,"private_revision_preserved":true,"background_origin_preserved":true,"restart_chat_preserved":true,"charges_before_restart":charges}));
}

fn prepare_personal(f:&Fixture,w:&str,id:&str,period:&str,provider:&str) {
    let endpoint=format!("/api/learning-workspaces/{w}/preparations");
    let bytes=b"# Transactions\n\nAtomic change.";
    f.ok(&f.a,"POST",&endpoint,json!({"action":"upload","upload_id":id,"filename":"transactions.md","total":bytes.len(),"offset":0,"chunk_base64":base64::engine::general_purpose::STANDARD.encode(bytes)}));
    f.ok(&f.a,"POST",&endpoint,json!({"action":"create","operation_id":"e1-create","task_id":id,"upload_id":id}));
    let task=f.ok(&f.a,"GET",&endpoint,Value::Null)["items"][0].clone();
    f.ok(&f.a,"POST",&endpoint,json!({"action":"prepare","operation_id":"e1-prepare","task_id":id,"expected_revision":task["revision"]}));
    f.ok(&f.b,"POST","/api/admin/operator-builds/next",json!({"host_session_id":"e1-host"}));
    let dir=f.access.library.lock().unwrap().load("A",&f.y).unwrap().directory().to_path_buf();
    f.ok(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/materials/source/progress"),json!({"host_session_id":"e1-host","state":"planned","workspace_dir":dir,"plan":{"workspace_dir":dir,"build_plan_path":dir.join("plan.json"),"plan":{"private_artifacts":[],"estimate":{"input_tokens":{"lower":100,"upper":500},"output_tokens":{"lower":10,"upper":100},"unknown_stages":[]}},"model_runtime":{"provider":provider,"model":"fixture-model","max_output_tokens":100,"context_window_tokens":1000},"review_markdown":"准备原文与基础理解"}}));
    f.ok(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/finish"),json!({"host_session_id":"e1-host","state":"awaiting_confirmation"}));
    let task=f.ok(&f.a,"GET",&endpoint,Value::Null)["items"][0].clone();
    f.ok(&f.a,"POST",&endpoint,json!({"action":"start","operation_id":"e1-start","task_id":id,"expected_revision":task["revision"],"period_id":period,"max_milli_percent":2298}));
}
fn metered(f:&Fixture,provider:&str,call:&str,attempt:u32)->Value {
    f.ok(&f.b,"POST","/api/admin/build-tasks/e1-personal/metered-admit",json!({"execution_id":"e1-execution","call_id":call,"logical_call_id":"e1-logical","attempt":attempt,"granularity":"calls","provider":provider,"model":"fixture-model","purpose":"external_build","budget_enforced":true,"receipt_capability":"complete","request":{"model":"fixture-model","input_tokens_estimate":100,"max_tokens":100}}))
}
fn receipt(f:&Fixture,call:&str,status:&str,tokens:Option<u32>,succeeded:bool)->Value {
    f.ok(&f.b,"POST","/api/admin/build-tasks/e1-personal/metered-receipts",json!({"call_id":call,"status":status,"usage":tokens.map(|t|json!({"input_tokens":t,"cached_input_tokens":0,"output_tokens":0,"total_tokens":t})),"evidence":{"source":"E1 controlled receipt"},"succeeded":succeeded}))
}
#[test]
fn e1_c_answer_background_preparation_retry_cancel_late_receipt_and_120_7_percent() {
    let provider=Provider::new();let(f,period)=fixture(&provider);let scope=space(&f,"期末考试冲刺");let w=scene(&f,&scope,"e1-c");
    provider.text("先区分速度与加速度。");send(&f,&w,"e1-c","e1-c-answer");f.access.runs.run_one(&f.access).unwrap();
    provider.text("{\"candidate_facts\":[],\"intent_observations\":[]}");
    assert_eq!(crate::multi_user_review::tick(&f.access,&Default::default(),(multi_user_host::now() as u64+65)*1000),1);
    prepare_personal(&f,&scope,"e1-personal",&period,&provider.url);
    metered(&f,&provider.url,"e1-failed",1);receipt(&f,"e1-failed","confirmed",Some(100),false);
    metered(&f,&provider.url,"e1-retry",2);receipt(&f,"e1-retry","confirmed",Some(150),true);
    metered(&f,&provider.url,"e1-cancel",3);receipt(&f,"e1-cancel","not_sent",None,false);
    metered(&f,&provider.url,"e1-late",4);receipt(&f,"e1-late","unknown",None,false);
    let before=f.ok(&f.b,"GET","/api/admin/build-tasks/e1-personal",Value::Null);assert_eq!(before["pending_micro_cny"],200);
    let f=reopen(f,&Probe::default());provider.configure(&f);
    let late=receipt(&f,"e1-late","confirmed",Some(23000),true);assert_eq!(receipt(&f,"e1-late","confirmed",Some(23000),true),late);
    assert_eq!(late["budget"]["debited_micro_cny"],22980);assert_eq!(late["budget"]["platform_covered_micro_cny"],270);assert_eq!(late["budget"]["pending_micro_cny"],0);
    let history=f.ok(&f.a,"GET","/api/account/usage",Value::Null);
    f.ok(&f.b,"POST","/api/admin/users/A/percentage-receipts",json!({"operation_id":"e1-topup","period_id":period,"revision":2,"basis_version":"e1-basis","delta_milli_percent":50000,"amount_fen":123,"paid_at":multi_user_host::now(),"channel":"fixture","note":"E1 test receipt"}));
    let account=f.ok(&f.a,"GET","/api/account/allowance",Value::Null);
    assert_eq!(account["current_allowance"]["balance"]["percentages"]["available"]["value"],"120.7");
    assert_eq!(f.ok(&f.a,"GET","/api/account/usage",Value::Null),history);
    let summary=f.ok(&f.b,"GET","/api/admin/cost-summary",Value::Null);
    let sums:(i64,i64,i64)=f.control.connection.query_row("SELECT sum(provider_cost_micro_cny),sum(account_debit_micro_cny),sum(provider_cost_micro_cny-account_debit_micro_cny) FROM model_call_charges",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(sums,(23270,23000,270));assert_eq!(provider.requests.lock().unwrap().len(),2);
    report("c",json!({"status":"passed","actual_answer_and_review_calls":2,"external_receipts":"controlled injected failure/retry/not-sent/unknown/late","pending_before_recovery":before["pending_micro_cny"],"provider_cost_micro_cny":sums.0,"account_debit_micro_cny":sums.1,"platform_covered_micro_cny":sums.2,"balance":account,"summary":summary,"history_unchanged_after_topup":true}));
}

#[test]
fn e1_b_shared_video_two_users_personal_preparation_selected_update_and_original_source() {
    let provider=Provider::new();let(f,_)=fixture(&provider);
    let directory=f.root.path().join("e1-video");let(book,_,request)=crate::video_frames::tests::fixture(&directory);
    std::fs::write(directory.join("base.json"),serde_json::to_vec(&book.base).unwrap()).unwrap();std::fs::write(directory.join("source.txt"),"A frame.").unwrap();
    let video=f.access.library.lock().unwrap().publish_online(&crate::online_publication::PublicationRequest {operation_id:"e1-video".into(),input_directory:directory.clone(),grant_users:vec!["A".into(),"B".into()]}).unwrap().reference;
    f.ok(&f.b,"POST","/api/admin/build-tasks/e1-public/budget",json!({"cost_owner":"platform","max_micro_cny":100,"confirmation":"e1-public"}));
    f.ok(&f.b,"POST","/api/admin/build-tasks/e1-public/admit",json!({"execution_id":"e1-public","call_id":"e1-public-call","logical_call_id":"e1-public-call","attempt":1,"granularity":"execution_total","reserved_micro_cny":20,"provider":"fixture","model":"fixture","purpose":"prebuild","budget_enforced":true,"receipt_capability":"complete"}));
    f.ok(&f.b,"POST","/api/admin/build-tasks/e1-public/receipts",json!({"call_id":"e1-public-call","status":"confirmed","provider_cost_micro_cny":12,"evidence":{"source":"controlled public build receipt"},"succeeded":true}));
    let publish=|version:u32,reference:&base_schema::PublishedBookRef| {
        let mut input=super::super::s1_workspace_publication_tests::publication(&f,&format!("e1-release-{version}"),version-1);
        input["manifest"]["materials"]=json!([{"published_book_ref":f.x,"required_capabilities":["text"]},{"published_book_ref":reference,"required_capabilities":["text","video"]}]);
        input["manifest"]["topic_links"]=json!([]);input["manifest"]["suggested_route"]=json!([]);
        f.ok(&f.b,"POST","/api/admin/workspace-releases",input)
    };
    publish(1,&video);
    let mut before=f.ok(&f.b,"GET","/api/admin/cost-summary",Value::Null);
    before.as_object_mut().unwrap().remove("as_of");
    let a=f.ok(&f.a,"POST","/api/workspace-releases/exam-course/versions/1/use",json!({"operation_id":"e1-a-use","goal":{"text":"结合视频学习深度学习"}}));
    let b=f.ok(&f.b,"POST","/api/workspace-releases/exam-course/versions/1/use",json!({"operation_id":"e1-b-use","goal":{"text":"期末冲刺"}}));
    let scope=a["workspace"]["learning_workspace_id"].as_str().unwrap();let second=b["workspace"]["learning_workspace_id"].as_str().unwrap();
    let w=scene(&f,scope,"e1-b");let mut run=f.run(&w,"e1-b");
    let frame=run.read_video_frames(Some(&video),request,&Default::default()).unwrap();assert_eq!(frame.images.len(),2);assert_eq!(frame.publication,video);
    let media=run.media_source_snapshot(serde_json::from_value(frame.projection()["frames"][0]["source_target"].clone()).unwrap()).unwrap();
    let revision=f.access.library.lock().unwrap().load("A",&video).unwrap().book.source_fingerprint().to_string();
    let binding=SourceBinding {source_ref_id:"e1-video-source".into(),book_id:video.book_id.clone(),publication_id:Some(video.publication_id.clone()),source_revision:Some(revision),evidence_range:None,evidence_text_digest:String::new(),label_snapshot:"原始画面".into(),preview_snapshot:"A frame.".into(),media:Some(media)};
    let turn=AgentTurnRef {session_id:run.scope.chat_session_id.clone(),turn_id:run.scope.turn_id.clone(),user_turn_ordinal:1};
    let handle=context(&f,&f.a).user;
    crate::session_runtime::append(&mut handle.lock().unwrap(),&turn,"e1-media",crate::session_event::EventBody::SourcesBound {bindings:vec![binding.clone()]}).unwrap();
    let observed=run.library_learning(json!({"published_book_ref":video,"operation":"observe","operation_id":"e1-understanding","source_ref_ids":[binding.source_ref_id],"learner_quote":"Explain this","learning_focus":"画面与原文","expected_performance":"解释联系","capability":"explanation","interpretation":"需要结合画面理解","teaching_implication":"回到对应书中概念"}),std::slice::from_ref(&binding)).unwrap();
    {
        let user=handle.lock().unwrap();let saved=user.learning_store_for_workspace(Some(scope)).unwrap().evidence(observed["evidence_ref"].as_str().unwrap()).unwrap();
        assert!(saved.target.as_ref().unwrap().source_bindings.is_empty(),"pure visual evidence must not invent transcript positions");
        assert_eq!(saved.target.as_ref().unwrap().media_bindings.as_ref().unwrap()[0].target,binding.media.as_ref().unwrap().target);
        assert_eq!(saved.source_quotes[0]["media"]["publication"],json!(video));
    }
    let personal=f.ok(&f.a,"POST",&format!("/api/learning-workspaces/{scope}/preparations"),json!({"action":"create","operation_id":"e1-private-reuse","task_id":"e1-private","name":"个人补充资料","published_book_ref":f.y}));assert_eq!(personal["state"],"ready");
    let newer=f.access.library.lock().unwrap().publish(&directory).unwrap().reference;for user in ["A","B"]{f.access.library.lock().unwrap().grant(user,&newer).unwrap();}
    publish(2,&newer);
    let endpoint=format!("/api/learning-workspaces/{scope}/shared-release");
    f.ok(&f.a,"GET",&format!("{endpoint}/preview?version=2"),Value::Null);
    assert_eq!(f.ok(&f.a,"GET",&endpoint,Value::Null)["instance"]["version"],1);
    let updated=f.ok(&f.a,"POST",&format!("{endpoint}/update"),json!({"operation_id":"e1-update","expected_version":1,"version":2}));
    assert_eq!(updated["workspace"]["goal"]["text"],"结合视频学习深度学习");
    assert!(updated["workspace"]["members"].as_array().unwrap().iter().any(|m|m["published_book_ref"]==json!(f.y)));
    assert_eq!(f.ok(&f.b,"GET",&format!("/api/learning-workspaces/{second}/shared-release"),Value::Null)["instance"]["version"],1);
    let user=context(&f,&f.b).user;assert!(user.lock().unwrap().learning_store_for_workspace(Some(second)).unwrap().evidence(observed["evidence_ref"].as_str().unwrap()).is_err());
    let current=f.get(&f.a,&w);
    let resolved=f.action(&f.a,&current,"e1-b","agent/source.resolve",json!({"turn_id":turn.turn_id,"source_ref_id":binding.source_ref_id}));assert_eq!(resolved["result"]["media"]["publication"],json!(video));
    let mut after=f.ok(&f.b,"GET","/api/admin/cost-summary",Value::Null);
    after.as_object_mut().unwrap().remove("as_of");
    assert_eq!(after,before);assert!(provider.requests.lock().unwrap().is_empty());
    report("b",json!({"status":"passed","shared_cost_micro_cny":12,"new_model_calls":0,"two_users_separate":true,"one_user_updated":true,"private_preparation_retained":true,"original_video_reference":video,"selected_video_reference":newer,"real_decoded_frames":frame.images.len(),"original_source":resolved["result"]["media"],"summary":before}));
}

#[test]
fn e1_d_existing_period_and_legacy_chat_upgrade_select_once_and_preserve_evidence() {
    let provider=Provider::new();let mut f=Fixture::new();
    let period=super::super::adm5_tests::seed(&mut f,&provider.url,730000);provider.configure(&f);
    let legacy=f.create(&f.a,&f.x,"e1-d");let legacy=f.action(&f.a,&legacy,"e1-d","chat/new",json!({}));
    provider.text("先明确你说的变化关系。");send(&f,&legacy,"e1-d","e1-d-selected");f.access.runs.run_one(&f.access).unwrap();let selected=evidence(&f,&legacy,"e1-d-selected");
    let other=f.action(&f.a,&f.get(&f.a,&legacy),"e1-d","chat/new",json!({}));
    provider.text("这段另行保留。");send(&f,&other,"e1-d","e1-d-unselected");f.access.runs.run_one(&f.access).unwrap();let unselected=evidence(&f,&other,"e1-d-unselected");
    let chat=legacy["selected_chat"].as_str().unwrap().to_owned();
    let account=context(&f,&f.a);let user=account.user.lock().unwrap();
    let path=user.session_store.as_ref().unwrap().paths.session(&chat);let original=std::fs::read(&path).unwrap();let fact=json!(user.learning_store().unwrap().evidence(&selected).unwrap());drop(user);drop(account);
    let charges=f.ok(&f.a,"GET","/api/account/usage",Value::Null);
    // Recreate the supported pre-external-build v15 control boundary, leaving
    // existing allowance records and the original unscoped learning files intact.
    f.control.connection.execute_batch("DROP TABLE workspace_release_instance_private_materials; DROP TABLE workspace_release_instance_operations; DROP TABLE workspace_release_instance_history; DROP TABLE workspace_release_instances; DROP TABLE workspace_release_archives; DROP TABLE operator_builds; DROP TABLE workspace_releases; DROP TABLE external_build_calls; DROP TABLE external_build_executions; DROP TABLE build_budget_confirmations; DROP TABLE build_task_budgets; DROP TABLE allowance_offers; PRAGMA user_version=15;").unwrap();
    let f=reopen(f,&Probe::default());provider.configure(&f);
    assert_eq!(f.control.connection.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),18);
    assert_eq!(f.ok(&f.a,"GET","/api/account/usage",Value::Null),charges);
    f.ok(&f.b,"POST","/api/admin/users/A/allowance-basis",json!({"operation_id":"e1-legacy-basis","period_id":period,"revision":1,"basis":{"version":"e1-old-basis","basis_micro_cny":1000000}}));
    let scope=space(&f,"只接续选定的旧学习记录");
    let detail=f.ok(&f.a,"GET",&format!("/api/me/learning-records/{chat}"),Value::Null);
    let endpoint=format!("/api/learning-workspaces/{scope}/records");let input=json!({"session_id":chat,"through_seq":detail["through_seq"],"goal_revision":1});
    let linked=f.ok(&f.a,"POST",&endpoint,input.clone());assert_eq!(f.ok(&f.a,"POST",&endpoint,input),linked);
    let target=scene(&f,&scope,"e1-d-new");let mut run=f.run(&target,"e1-d-new");
    assert_eq!(run.workspace_records(json!({"operation":"list"})).unwrap()["records"].as_array().unwrap().len(),1);
    let read=run.workspace_records(json!({"operation":"read","chat_session_id":chat})).unwrap();assert_eq!(read["current_mastery"],false);
    assert!(run.workspace_records(json!({"operation":"read","chat_session_id":other["selected_chat"]})).is_err());drop(run);
    let account=context(&f,&f.a);let user=account.user.lock().unwrap();
    assert_eq!(json!(user.learning_store().unwrap().evidence(&selected).unwrap()),fact);
    let store=user.learning_store_for_workspace(Some(&scope)).unwrap();assert!(store.evidence(&selected).is_err());
    let bundles=store.linked_record_bundles().unwrap();assert_eq!(bundles.len(),1);assert!(bundles[0].evidence.iter().all(|e|e.evidence_id!=unselected));drop(store);drop(user);drop(account);
    assert_eq!(std::fs::read(&path).unwrap(),original);
    let f=reopen(f,&Probe::default());
    assert_eq!(f.ok(&f.a,"GET",&endpoint,Value::Null)["records"].as_array().unwrap().len(),1);assert_eq!(std::fs::read(path).unwrap(),original);
    assert_eq!(provider.requests.lock().unwrap().len(),2);
    report("d",json!({"status":"passed","upgraded_control_schema":18,"original_charges_retained":charges["total"],"original_jsonl_unchanged":true,"original_evidence_unchanged":true,"selected_record_count":1,"duplicate_link_no_new_fact":true,"unselected_record_excluded":true,"provider_calls_before_upgrade":2,"provider_calls_for_upgrade_and_link":0}));
}

#[test]
#[ignore = "E1 real HTTP host for browser and compiled DSH preparation acceptance"]
fn e1_browser_host() {
    let mut f=Fixture::new();let period=super::super::adm5_tests::seed_model(&mut f,"probe","scripted",1_000_000);
    f.control.connection.execute("INSERT INTO allowance_bases VALUES('e1-http-basis',1000000)",[]).unwrap();
    f.control.connection.execute("INSERT INTO allowance_period_bases VALUES(?,'e1-http-basis')",[&period]).unwrap();
    configure(&f,&Probe::default());
    let directory=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/b2-v1-20261018/video-workspace");
    let video=f.access.library.lock().unwrap().publish_online(&crate::online_publication::PublicationRequest {operation_id:"e1-browser-video".into(),input_directory:directory.clone(),grant_users:vec!["A".into(),"B".into()]}).unwrap().reference;
    let newer=f.access.library.lock().unwrap().publish(&directory).unwrap().reference;
    for user in ["A","B"]{f.access.library.lock().unwrap().grant(user,&newer).unwrap();}
    let publish=|expected:u32,reference:&base_schema::PublishedBookRef| {
        let mut input=super::super::s1_workspace_publication_tests::publication(&f,&format!("e1-browser-release-{expected}"),expected);
        input["manifest"]["name"]=json!("深度学习 · 书与视频");
        input["manifest"]["materials"]=json!([{"published_book_ref":f.x,"required_capabilities":["text"]},{"published_book_ref":reference,"required_capabilities":["text","video"]}]);
        input["manifest"]["topic_links"]=json!([]);input["manifest"]["suggested_route"]=json!([]);
        f.ok(&f.b,"POST","/api/admin/workspace-releases",input)
    };
    publish(0,&video);
    let origin="https://127.0.0.1:4190";
    let server=multi_user_host::start_with_access("127.0.0.1:4190".parse().unwrap(),Arc::new(Site::new(origin).unwrap()),f.access.clone()).unwrap();
    let control=tiny_http::Server::http("127.0.0.1:4191").unwrap();println!("E1_BROWSER_READY");
    let until=std::time::Instant::now()+Duration::from_secs(3600);
    while std::time::Instant::now()<until {
        let Some(request)=control.recv_timeout(Duration::from_secs(1)).unwrap() else{continue};
        if request.url()=="/stop"{request.respond(tiny_http::Response::from_string("stopped")).unwrap();break;}
        let result=if request.url()=="/publish-next"{publish(1,&newer)}else{json!({"first":f.x,"second":f.y,"video":video,"new_video":newer,"root":f.root.path(),"period_id":period,"origin":origin,
            "reader_cookie":format!("{}={}",crate::auth::COOKIE,f.a),"reader_csrf":crate::auth::csrf(&f.a),"admin_cookie":format!("{}={}",crate::auth::COOKIE,f.b),"admin_csrf":crate::auth::csrf(&f.b)})};
        request.respond(tiny_http::Response::from_string(result.to_string()).with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
    }
    server.shutdown();
}
