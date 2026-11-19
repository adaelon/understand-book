pub use reader_service::tests::tutor_loop_tests::closed_book_tests::*;
use super::*;
#[path = "model_trial_budget.rs"]
mod model_trial_budget;


#[test]
#[ignore = "paid single-sample course reuse trial; up to three operator-answered followups; retains isolated evidence"]
fn closed_book_course_live_model_trial() {
    use memory::course::{CourseAction, CourseLessonView, CourseMutation, CoursePreset, CourseResponse, CourseStage};
    use runtime::run_context::CancellationToken;
    use std::{path::Path, sync::{Arc, Mutex}, time::{Duration, Instant}};

    fn write(directory: &Path, name: &str, value: &Value) {
        std::fs::write(directory.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
    }
    // The production background Coach deadline also bounds each trial step,
    // including Resident tool calls and its ordinary after-delivery Coach.
    fn bounded<T>(run: impl FnOnce(CancellationToken) -> T) -> T {
        let cancel=CancellationToken::default();
        let done=std::sync::atomic::AtomicBool::new(false);
        std::thread::scope(|scope| {
            let cancel=&cancel;let done=&done;
            scope.spawn(move || {
                let started=Instant::now();
                while !done.load(std::sync::atomic::Ordering::Acquire) {
                    if started.elapsed() >= Duration::from_secs(180) { cancel.cancel();break; }
                    std::thread::sleep(Duration::from_millis(100));
                }
            });
            let result=run(cancel.clone());
            done.store(true,std::sync::atomic::Ordering::Release);
            result
        })
    }
    fn act(store: &mut memory::learning::LearningStore, view: &CourseLessonView, id: &str,
        action: CourseAction, goal: u32) -> Result<CourseLessonView,String> {
        store.mutate_course(&CourseMutation { operation_id:id.into(),run_id:Some(view.progress.run_id.clone()),
            expected_revision:view.progress.revision,release_version:view.progress.release_version,action },None,goal,"trial",None)
            .map_err(|error|error.message)
    }
    fn cost(usage: &crate::service_limits::RunUsage, official_flash: bool) -> Value {
        // A tariff estimate is separate from an account bill. The exact request
        // time band is intentionally represented as the published price range.
        let totals=usage.calls.iter().try_fold((0u64,0u64,0u64),|sum,call| {
            let call=call.as_ref()?;
            let input=call.input_tokens?;let cached=call.cached_input_tokens?;let output=call.output_tokens?;
            if cached>input || call.cache_creation_input_tokens.is_some_and(|n|n>0) { return None; }
            Some((sum.0+u64::from(input-cached),sum.1+u64::from(cached),sum.2+u64::from(output)))
        });
        let estimate=totals.filter(|_|official_flash).map(|(input,cached,output)|
            (input as f64*0.15+cached as f64*0.003+output as f64*0.60)/1_000_000.0);
        json!({"kind":"published_tariff_estimate_not_account_bill","currency":"USD",
            "off_peak":estimate,"peak":estimate.map(|n|n*2.0),"usage_breakdown_complete":totals.is_some(),
            "applies_to_official_flash_endpoint":official_flash,"tariff_checked_on":"2026-11-16",
            "tariff_source":"https://api-docs.deepseek.com/quick_start/pricing/",
            "rates_per_million":{"off_peak":{"uncached_input":0.15,"cached_input":0.003,"output":0.60},
                "peak":{"uncached_input":0.30,"cached_input":0.006,"output":1.20}},
            "unknown_reason":if !official_flash {Some("The configured provider/model has no verified tariff attached to this trial")}
                else if totals.is_none(){Some("The provider omitted a required input/output/cache usage component")}else{None}})
    }

    let directory=std::env::var("CLOSED_BOOK_TRIAL_SOURCE").expect("CLOSED_BOOK_TRIAL_SOURCE");
    let output=std::path::PathBuf::from(std::env::var("CLOSED_BOOK_COURSE_TRIAL_OUTPUT").expect("CLOSED_BOOK_COURSE_TRIAL_OUTPUT"));
    assert!(!output.join("trial-result.json").exists(),"Choose a new evidence directory; do not overwrite a previous attempt");
    std::fs::create_dir_all(&output).unwrap();
    std::env::set_var("COURSE_FEEDBACK_CAPTURE",output.join("coach-captures"));
    let source=std::fs::read_to_string(Path::new(&directory).join("source.txt")).unwrap();
    let book=Book::load(&directory).unwrap();
    let (trial_root,mut state)=fixture_with_book(false,book,&source);
    let retained_state=trial_root.keep();
    write(&output,"private-state-location.json",&json!({"directory":retained_state,"history_path":state.user.history_path,
        "purpose":"Retained isolated trial fixture for continuing the same real conversation after inspection; no production learner state"}));
    post(&mut state,"/agent/new","{}");
    let config=runtime::ProviderConfig::from_env().unwrap();
    let model=config.model.clone();
    let official_flash=config.base_url.strip_prefix("https://api.deepseek.com").is_some_and(|path|path.is_empty()||path.starts_with('/'))
        && matches!(model.as_str(),"deepseek-flash"|"deepseek-v4-flash"|"deepseek-v4-flash-vision-exp");
    write(&output,"selected-provider.json",&json!({"model":model,"api_protocol":config.options.api_protocol,
        "binding":config.options.binding,"provider_origin":url::Url::parse(&config.base_url).unwrap().origin().ascii_serialization()}));
    let provider=runtime::ProviderRegistry::adapter_from_config_with_timeout(config,Duration::from_secs(180));
    provider.set_spend_context(runtime::model_spend::ChargeScope::OperatorTask { task_ref:"course-model-trial".into() },
        Some(Arc::new(model_trial_budget::TrialBudget::open(&output))));
    let adapter=crate::service_limits::LimitedAdapter { inner:provider.as_ref(),
        resources:crate::service_limits::Resources::new(Arc::new(crate::service_limits::ServiceLimits::default())),
        owner:"closed-book-course-trial".into(),background:false,cancellation:CancellationToken::default(),
        usage:Mutex::new(Default::default()),stream:None,authorization:None };
    let began=Instant::now();
    let replay=|state:&mut AppState,view:&CourseLessonView,index:usize,message:&str| -> Result<Value,String> {
        let input=json!({"message":message,"course_context":{"run_id":view.progress.run_id,"revision":view.progress.revision,"surface":"course"}});
        let run=prepare_agent_chat(state,&input.to_string(),"trial").map_err(|error|error.body)?;
        let turn=run.turn_ref.clone();
        let frozen_course=run.tutor.as_ref().map(|context|context["inquiry_course"].clone());
        let calls_before=adapter.usage.lock().unwrap().calls.len();
        let step_started=Instant::now();
        let result=bounded(|cancel|reader_service::tests::host_calls::agent_run::execute_prepared(&reader_service::tests::host_calls::borrowed_app_port(&mut *state),&adapter,run,cancel));
        let store=state.user.learning_store().map_err(|error|error.message)?;
        let proposal=store.teaching_event(&format!("closed-book-proposal:{}",turn.turn_id)).ok();
        let practice=store.closed_book(&turn.session_id,&state.workspace.book.base.book_id).map_err(|error|error.message)?;
        let ordinary_coach=store.coach_job_for_source(&view.progress.run_id,memory::tutor_coach::CoachJobOrigin::AfterTurn,&turn.turn_id)
            .map_err(|error|error.message)?;
        let response=state.user.agent_history.sessions.iter().find(|session|session.id==turn.session_id).unwrap()
            .turns.iter().find(|saved|saved.turn_id==turn.turn_id).unwrap();
        let calls=adapter.usage.lock().unwrap().calls[calls_before..].to_vec();
        let report=json!({"turn_ref":turn.turn_id,"message":message,"reply_status":result.status,
            "elapsed_ms":step_started.elapsed().as_millis(),"frozen_course":frozen_course,
            "answer":response.outcome.as_ref().and_then(|outcome|outcome.answer.as_ref()),"error":response.error,
            "decision":proposal.map(|event|event.payload),"practice":practice,"ordinary_after_turn_coach":ordinary_coach,
            "usage_calls":calls,"usage_scope":"Resident and its ordinary after-delivery Coach; the original-course reuse Coach runs separately"});
        write(&output,&format!("turn-{index}.json"),&report);
        eprintln!("Course reuse trial turn {index}: {}",report["decision"]["decision"]["action"]);
        if result.status!=200 { return Err(format!("Resident turn {index} failed with HTTP {}",result.status)); }
        Ok(report)
    };
    let attempt=(|| -> Result<Value,String> {
        let mut course:memory::course::InquiryCourse=serde_json::from_str(include_str!("../../../../docs/examples/inquiry-course/ai-infra-resource-bounds.course.json")).unwrap();
        let lesson=&mut course.lessons[0];
        lesson.summary.source.publication.book_id=state.workspace.book.base.book_id.clone();
        lesson.summary.source.publication.publication_id="local".into();
        lesson.summary.source.source_revision=state.workspace.book.source_fingerprint().into();
        let source_excerpt=state.workspace.book.text(&lesson.summary.source.start_lid,Some(&lesson.summary.source.end_lid)).map_err(|error|error.message)?;
        let mut store=state.user.learning_store().map_err(|error|error.message)?;
        let goal=store.state().map_err(|error|error.message)?.workspace_goal_revision.unwrap_or(0);
        let mut view=store.mutate_course(&CourseMutation{operation_id:"trial-course-start".into(),run_id:None,expected_revision:0,
            release_version:1,action:CourseAction::Start{lesson_id:lesson.summary.id.clone()}},Some(("trial-formal-course",1,lesson)),goal,"trial",None).map_err(|error|error.message)?;
        view=act(&mut store,&view,"trial-begin",CourseAction::Advance,goal)?;
        view=act(&mut store,&view,"trial-predict",CourseAction::Predict{response:CourseResponse{choice:"compute".into(),reason:String::new()}},goal)?;
        for (id,preset) in [("baseline",CoursePreset::Baseline),("compute",CoursePreset::DoubleCompute),("bandwidth",CoursePreset::DoubleBandwidth)] {
            view=act(&mut store,&view,&format!("trial-{id}"),CourseAction::Observe{preset},goal)?;
        }
        let mut observation=lesson.lab.clone().unwrap();observation.read_gb=35.0;
        view=act(&mut store,&view,"trial-explore",CourseAction::Explore{state:observation},goal)?;
        view=act(&mut store,&view,"trial-explain",CourseAction::Advance,goal)?;
        write(&output,"original-course.json",&json!({"model":model,"source":lesson.summary.source,"source_excerpt":source_excerpt,
            "course_artifact":"docs/examples/inquiry-course/ai-infra-resource-bounds.course.json","view":view,
            "scope":"One scripted learner, first reflection round; original prediction choice retained without prose, so prediction Coach is not covered; actual public source is read-only"}));
        let started=control(&mut state,"start",json!({"course_run_id":view.progress.run_id,"target":"整课目标不应覆盖当前复盘题"}));
        write(&output,"practice-start.json",&started);
        let first=replay(&mut state,&view,1,"请按当前这一段的复盘目标开始闭卷练习，把必要的实验条件和数值放进题面，先让我回答。")?;
        if first["decision"]["decision"]["action"]!="question" {return Err("The first delivered turn did not establish the course question".into());}
        let help=replay(&mut state,&view,2,"我不明白为什么两项必要时间不是相加，请直接把这一步讲明白。")?;
        if help["practice"]["assisted"]!=true {return Err("Requested teaching was not retained as assistance".into());}
        let retry=replay(&mut state,&view,3,"换一个同目标的新问题让我独立试。保留原配置作为对照，再加入读取量从70GB减至28GB、其他条件不变这个新改动；让我解释它和只升级硬件的区别，并回看自己的预测。请只给题面，不给解法。")?;
        if retry["decision"]["decision"]["action"]!="retry" || retry["practice"]["assisted"]!=false {return Err("A new independent attempt was not delivered".into());}
        let done=replay(&mut state,&view,4,"原配置的计算下限是140/989.4≈0.1415ms，读取下限是70/3.35≈20.90ms。两项都是总耗时必须满足的下限，容量足够且充分重叠时取较大值，不相加。只把算力翻倍，计算变为约0.0707ms，读取仍约20.90ms，所以下界不改善；带宽翻倍或读取量减半，读取变为约10.45ms，仍大于9ms。新改动把读取量降至28GB，读取下限约8.36ms，计算仍约0.1415ms，所以下界约8.36ms，低于9ms但不保证实测达标，还可能有调度和传输开销。减少读取量改变工作量，增加带宽改变硬件能力，虽然都能降低读取下限，前者还要核对质量和转换成本。我原来选了算力翻倍会使整个任务下界减半；现在保留算力会缩短计算那一项的判断，修正了它必然缩短总下界的预测，要先比较哪个资源约束更大。")?;
        let done=finish_course_trial_dialogue(done,|index,previous| {
            let answer=course_trial_followup_answer(&output,index,previous)?;
            replay(&mut state,&view,index,&answer)
        })?;
        write(&output,"closed-book-completion.json",&done);
        let result_ref=done["practice"]["result_ref"].as_str().ok_or("No delivered completion reference")?.to_owned();
        view=act(&mut store,&view,"trial-use-result",CourseAction::UseClosedBook{result_ref:result_ref.clone()},goal)?;
        let expression=view.expressions.last().ok_or("Reuse did not create a course expression")?.clone();
        let job=store.coach_job(&expression.job_ref).map_err(|error|error.message)?.ok_or("Original-course Coach job is absent")?;
        // Select the real queued job through the existing inline dependency
        // driver. It performs freeze/material/execute/evidence commit unchanged.
        let turn=AgentTurnRef{session_id:job.input.binding.chat_session_id.clone(),turn_id:job.input.binding.turn_id.clone(),user_turn_ordinal:1};
        store.ensure_coach_turn_dependency(&turn.session_id,&turn.turn_id,Some(&job.input.job_ref),"trial").map_err(|error|error.message)?;
        let premature=act(&mut store,&view,"trial-before-review",CourseAction::Advance,goal);
        write(&output,"course-reuse-submitted.json",&json!({"result_ref":result_ref,"expression":expression,"job":job,
            "premature_advance_rejected":premature.is_err(),"dispatch":"Existing drive_inline dependency selects the UseClosedBook-created job; original source hydration, model validation and evidence commit remain enabled"}));
        if premature.is_ok(){return Err("Course advanced before its original Coach accepted the reflection".into());}
        let calls_before=adapter.usage.lock().unwrap().calls.len();let coach_started=Instant::now();
        let executed=bounded(|cancel|reader_service::tests::host_calls::teaching_coach::drive_inline(&reader_service::tests::host_calls::borrowed_app_port(&mut state),&adapter,None,&turn,false,&cancel));
        let reviewed=store.coach_job(&expression.job_ref).map_err(|error|error.message)?.unwrap();
        write(&output,"original-course-coach.json",&json!({"elapsed_ms":coach_started.elapsed().as_millis(),"job":reviewed,
            "error":executed.as_ref().err(),"usage_calls":adapter.usage.lock().unwrap().calls[calls_before..]}));
        executed.map_err(|error|error.message)?;
        if reviewed.status!=memory::tutor_coach::CoachJobStatus::Ready {return Err("Original-course Coach did not reach Ready; inspect retained error and captures".into());}
        view=act(&mut store,&view,"trial-read-feedback",CourseAction::ReadFeedback{job_ref:expression.job_ref},goal)?;
        let displayed=store.course_view(&view.progress.run_id).map_err(|error|error.message)?;
        write(&output,"feedback-read.json",&json!(displayed));
        let interpretations=store.teaching_expression_interpretations(&expression.fact_ref).map_err(|error|error.message)?;
        if interpretations.iter().any(|e| !e.assistance_refs.is_empty() || !e.support_delivery_refs.is_empty()) {
            return Err("The imported independent attempt was relabeled with prior course help; inspect its understanding evidence".into());
        }
        if store.course_run(&view.progress.run_id).map_err(|error|error.message)?.reflection.is_none() {
            return Err("Original-course Coach did not accept the imported reflection; its concrete gap is retained".into());
        }
        view=act(&mut store,&view,"trial-next-round",CourseAction::Advance,goal)?;
        if view.round_index!=1 || view.progress.stage!=CourseStage::Prediction {return Err("Accepted reflection did not return to the next original course round".into());}
        write(&output,"next-course-step.json",&json!(view));
        let before_reopen=store.course_view(&view.progress.run_id).map_err(|error|error.message)?;
        drop(store);
        let restored=state.user.learning_store().map_err(|error|error.message)?
            .course_view(&view.progress.run_id).map_err(|error|error.message)?;
        write(&output,"resumed-course.json",&json!({"chat_session_id":state.workspace.selected_chat,
            "before_reopen":before_reopen,"view":restored}));
        if serde_json::to_value(&restored).unwrap()!=serde_json::to_value(&before_reopen).unwrap() {
            return Err("Reopening private storage did not restore the same course step".into());
        }
        Ok(json!({"result_ref":result_ref,"next_round":view.round_index,"next_stage":view.progress.stage,"course_complete":false}))
    })();
    let usage=adapter.usage.lock().unwrap().clone();
    write(&output,"trial-result.json",&json!({"model":model,"elapsed_ms":began.elapsed().as_millis(),"passed":attempt.is_ok(),
        "result":attempt.as_ref().ok(),"failure":attempt.as_ref().err(),"usage":usage,"cost":cost(&usage,official_flash),
        "sample_count":1,"max_learner_turns":4+COURSE_TRIAL_MAX_FOLLOWUPS,"step_timeout_seconds":180,
        "planned_scope":"Scripted learner followed by at most three operator answers to actual follow-ups, through real Resident deliveries, ordinary after-delivery Coaches, persisted completion reference, original course Coach, ReadFeedback, Advance and reopened private storage. A failed attempt stops at the recorded failure; missing later artifacts mean those steps were not executed. No synthetic teacher proposals or judgments; no rerun to obtain a passing sample.",
        "not_covered":["Human first-use learning experience","Decision/transfer semantic live sample","Prediction Coach"]}));
    assert!(attempt.is_ok(),"{}",attempt.unwrap_err());
}


#[test]
#[ignore = "paid single-model protocol trial; three logical calls; requires explicit request and CNY budgets"]
fn model_provider_live_tool_and_structured_trial() {
    use runtime::{AgentRequestPlan, CompletionRequest, Message, Role, ToolSpec};
    use runtime::provider_stream::{ModelDelta, ModelObserver};
    use std::{fs::OpenOptions, io::Write, path::{Path,PathBuf}, sync::Arc, time::{Duration,Instant}};

    const SOURCE_PATH:&str="examples/quickstart/book.md";
    const SECTION:&str="从问题回到原文";
    const SOURCE_ID:&str="quickstart:source-evidence";
    const OUTPUT_LIMIT:u32=4096;

    fn write_new(directory:&Path,name:&str,value:&Value) {
        let mut file=OpenOptions::new().write(true).create_new(true).open(directory.join(name))
            .expect("Choose a fresh evidence directory; earlier trial artifacts must remain unchanged");
        file.write_all(&serde_json::to_vec_pretty(value).unwrap()).unwrap();
        file.flush().unwrap();
    }
    struct Trace {
        file:std::fs::File,
        started:Instant,
        first_text_ms:Option<u128>,
        text:String,
        usage:Option<runtime::provider_stream::ModelUsage>,
        finish:Option<String>,
        request_events:u32,
    }
    impl Trace {
        fn open(directory:&Path,stage:&str)->Self {
            Self {file:OpenOptions::new().write(true).create_new(true).open(directory.join(format!("{stage}-events.jsonl"))).unwrap(),
                started:Instant::now(),first_text_ms:None,text:String::new(),usage:None,finish:None,request_events:0}
        }
        fn summary(&self)->Value {
            json!({"elapsed_ms":self.started.elapsed().as_millis(),"first_text_ms":self.first_text_ms,
                "observed_text":self.text,"usage":self.usage,"finish":self.finish,"request_events":self.request_events,
                "actual_send_ledger":"provider-sends.jsonl"})
        }
    }
    impl ModelObserver for Trace {
        fn observe(&mut self,delta:ModelDelta) {
            let event=match delta {
                ModelDelta::Request(value)=>{self.request_events+=1;json!({"kind":"request","body":value.as_ref()})},
                ModelDelta::Response(value)=>json!({"kind":"response","body":value.as_ref()}),
                ModelDelta::RetryReset=>{self.text.clear();json!({"kind":"retry_reset"})},
                ModelDelta::Text(text)=>{
                    if !text.is_empty() && self.first_text_ms.is_none(){self.first_text_ms=Some(self.started.elapsed().as_millis());}
                    self.text.push_str(&text);json!({"kind":"text","text":text})
                },
                ModelDelta::ToolArguments{index,id,name,arguments}=>json!({"kind":"tool_arguments","index":index,"id":id,"name":name,"arguments":arguments}),
                ModelDelta::Usage(usage)=>{self.usage=Some(usage.clone());json!({"kind":"usage","usage":usage})},
                ModelDelta::Finish(reason)=>{self.finish=Some(reason.clone());json!({"kind":"finish","reason":reason})},
            };
            writeln!(self.file,"{}",json!({"elapsed_ms":self.started.elapsed().as_millis(),"event":event})).unwrap();
            self.file.flush().unwrap();
        }
    }

    let max_requests=std::env::var("MODEL_TRIAL_MAX_REQUESTS").expect("Set an explicit request budget (3 to 6)")
        .parse::<u64>().expect("MODEL_TRIAL_MAX_REQUESTS must be an integer");
    assert!((3..=6).contains(&max_requests),"This trial permits at most six actual sends, including transport retries");
    let max_cost=std::env::var("MODEL_TRIAL_MAX_COST_MICRO_CNY").expect("Set the authorized per-sample CNY budget")
        .parse::<u64>().expect("MODEL_TRIAL_MAX_COST_MICRO_CNY must be an integer");
    assert!((1..=200_000).contains(&max_cost),"A protocol sample must not reserve more than CNY 0.20");
    std::env::var("MODEL_TRIAL_RATES_FILE").expect("MODEL_TRIAL_RATES_FILE supplies the verified rate snapshot");
    let output=PathBuf::from(std::env::var("MODEL_PROTOCOL_TRIAL_OUTPUT").expect("MODEL_PROTOCOL_TRIAL_OUTPUT"));
    std::fs::create_dir_all(&output).unwrap();
    let config=runtime::ProviderConfig::from_env().expect("Select exactly one configured protocol model");
    let model=config.model.clone();
    let protocol=config.options.api_protocol;
    let origin=url::Url::parse(&config.base_url).unwrap().origin().ascii_serialization();
    write_new(&output,"trial-start.json",&json!({"model":model,"api_protocol":protocol,"binding":config.options.binding,
        "provider_origin":origin,"source_path":SOURCE_PATH,"section":SECTION,"source_id":SOURCE_ID,
        "logical_calls":3,"max_actual_sends":max_requests,"max_cost_micro_cny":max_cost,"output_token_limit":OUTPUT_LIMIT,
        "semantic_evaluation":"pending_review"}));
    eprintln!("Protocol trial: source={SOURCE_PATH}, model={model}, protocol={}, origin={origin}",protocol.as_str());
    let provider=runtime::ProviderRegistry::adapter_from_config_with_timeout(config,Duration::from_secs(180));
    provider.set_spend_context(runtime::model_spend::ChargeScope::OperatorTask {task_ref:format!("protocol-trial-{}",uuid::Uuid::now_v7())},
        Some(Arc::new(model_trial_budget::TrialBudget::open(&output))));
    let began=Instant::now();
    let attempt=(||->Result<Value,String> {
        let source_file=Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(SOURCE_PATH);
        let tools=vec![ToolSpec {name:"source_read".into(),description:"Read the named section from the original local source file.".into(),
            parameters:json!({"type":"object","properties":{"path":{"type":"string","enum":[SOURCE_PATH]},
                "section":{"type":"string","enum":[SECTION]}},"required":["path","section"],"additionalProperties":false})}];
        let question=format!("知识图谱中的关系边与支撑回答的事实证据是什么关系？先调用 source_read 读取 {SOURCE_PATH} 中的「{SECTION}」小节，收到原文后用一两句话回答，并用 [{SOURCE_ID}] 标注来源。");
        let mut messages=vec![Message::system("你是阅读助手。回答必须依据工具返回的原文。尚未读取原文时，只请求 source_read，不要凭已有知识作答。完成一次读取后直接回答，不重复读取。"),Message::user(question)];
        let mut request=AgentRequestPlan::for_ad_hoc(provider.model_runtime_profile(),&messages,&tools);
        request.output_token_limit=Some(OUTPUT_LIMIT);
        request.parallel_tool_calls=false;
        provider.set_model_purpose("protocol_trial_tool");
        let mut first_trace=Trace::open(&output,"01-tool");
        let first=provider.chat_observed(&request,&mut first_trace);
        write_new(&output,"01-tool-result.json",&json!({"trace":first_trace.summary(),"response":first.as_ref().ok(),
            "error":first.as_ref().err().map(|error|&error.message)}));
        let first=first.map_err(|error|error.message)?;
        if first.tool_calls.len()!=1 {return Err(format!("Expected exactly one source_read call; received {}",first.tool_calls.len()));}
        let call=&first.tool_calls[0];
        if call.name!="source_read" || call.id.is_empty() {return Err("The model returned the wrong tool or an empty tool call ID".into());}
        let arguments:Value=serde_json::from_str(&call.arguments).map_err(|error|format!("Incomplete tool arguments: {error}"))?;
        if arguments!=json!({"path":SOURCE_PATH,"section":SECTION}) {return Err("The tool arguments do not select the requested source section".into());}

        // The file is actually read only after the model requests the checked tool.
        let source=std::fs::read_to_string(&source_file).map_err(|error|error.to_string())?;
        let marker=format!("## {SECTION}");
        let section=source.split_once(&marker).ok_or("The requested source heading is missing")?.1
            .split("\n## ").next().unwrap().trim().to_owned();
        if section.is_empty(){return Err("The original source section is empty".into());}
        let tool_result=json!({"source_id":SOURCE_ID,"path":SOURCE_PATH,"section":SECTION,"text":section});
        write_new(&output,"source-tool-result.json",&tool_result);
        messages.push(Message {provider_continuation:first.provider_continuation.clone(),role:Role::Assistant,
            content:first.text.clone(),tool_calls:first.tool_calls.clone(),tool_call_id:None});
        messages.push(Message {provider_continuation:None,role:Role::Tool,content:Some(tool_result.to_string()),
            tool_calls:vec![],tool_call_id:Some(call.id.clone())});
        write_new(&output,"retained-tool-history.json",&json!(messages));
        // Reopen the serialized history so the second call exercises private-state persistence too.
        let restored:Vec<Message>=serde_json::from_slice(&std::fs::read(output.join("retained-tool-history.json")).map_err(|error|error.to_string())?)
            .map_err(|error|error.to_string())?;
        if restored!=messages{return Err("Tool history changed during serialization".into());}
        let mut answer_request=AgentRequestPlan::for_ad_hoc(provider.model_runtime_profile(),&restored,&tools);
        answer_request.output_token_limit=Some(OUTPUT_LIMIT);
        answer_request.parallel_tool_calls=false;
        answer_request.tool_choice=runtime::model_runtime::ToolChoice::None;
        provider.set_model_purpose("protocol_trial_answer");
        let mut answer_trace=Trace::open(&output,"02-answer");
        let answer=provider.chat_observed(&answer_request,&mut answer_trace);
        write_new(&output,"02-answer-result.json",&json!({"trace":answer_trace.summary(),"response":answer.as_ref().ok(),
            "error":answer.as_ref().err().map(|error|&error.message)}));
        let answer=answer.map_err(|error|error.message)?;
        let text=answer.text.as_deref().filter(|text|!text.trim().is_empty()).ok_or("The source-grounded answer is empty")?;
        if !answer.tool_calls.is_empty() || !text.contains(&format!("[{SOURCE_ID}]")) {
            return Err("The second call must deliver an answer with the original source label and no further tool calls".into());
        }

        provider.set_model_purpose("protocol_trial_structured");
        let mut structured_trace=Trace::open(&output,"03-structured");
        let structured=provider.complete_structured_observed(CompletionRequest {
            system:"你是原文信息提取器。只返回 JSON 对象，字段为 source_id、claim、supporting_quote。claim 用一句话概括原文对知识图谱与事实证据关系的判断；supporting_quote 必须是原文中直接支持该判断的一段逐字引用。source_id 原样保留输入标识。不要使用外部资料。".into(),
            user:tool_result.to_string(),images:vec![],followup_user:None,output_token_limit:Some(OUTPUT_LIMIT),reasoning_effort:None,
        },&mut structured_trace);
        write_new(&output,"03-structured-result.json",&json!({"trace":structured_trace.summary(),"output":structured.as_ref().ok(),
            "error":structured.as_ref().err().map(|error|&error.message)}));
        let structured=structured.map_err(|error|error.message)?;
        if !structured.is_object() || structured["source_id"]!=SOURCE_ID {
            return Err("Structured output does not preserve the source identity".into());
        }
        let claim=structured["claim"].as_str().filter(|text|!text.trim().is_empty()).ok_or("Structured output has no claim")?;
        let quote=structured["supporting_quote"].as_str().filter(|text|!text.trim().is_empty()).ok_or("Structured output has no supporting quote")?;
        if !section.contains(quote) {return Err("The supporting quote is not verbatim original source text".into());}
        Ok(json!({"tool_id":call.id,"tool_name":call.name,"source_id":SOURCE_ID,"answer":text,"structured_claim":claim,
            "supporting_quote":quote,"tool_history_reloaded":true,"logical_calls_completed":3,
            "semantic_evaluation":"pending_review","program_assertions":["tool_name_and_arguments","tool_id_round_trip",
                "private_state_serialization","answer_source_label","structured_source_identity","verbatim_supporting_quote"]}))
    })();
    write_new(&output,"trial-result.json",&json!({"model":model,"api_protocol":protocol,"source_path":SOURCE_PATH,
        "elapsed_ms":began.elapsed().as_millis(),"execution_passed":attempt.is_ok(),"result":attempt.as_ref().ok(),
        "failure":attempt.as_ref().err(),"semantic_evaluation":"pending_review","actual_send_ledger":"provider-sends.jsonl"}));
    eprintln!("Protocol trial result: model={model}, protocol={}, execution_passed={}, source={SOURCE_PATH}",protocol.as_str(),attempt.is_ok());
    assert!(attempt.is_ok(),"{}",attempt.unwrap_err());
}
