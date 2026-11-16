use super::*;

fn control(state: &mut AppState, action: &str, extra: Value) -> Value {
    let chat = state
        .workspace
        .selected_chat
        .clone()
        .expect("the fixture opens a chat before practice");
    let saved = state
        .user
        .learning_store()
        .unwrap()
        .closed_book(&chat, &state.workspace.book.base.book_id)
        .unwrap();
    let mut body = json!({"action":action,"chat_session_id":chat,"revision":saved.as_ref().map_or(0,|v|v.revision),"operation_id":uuid::Uuid::now_v7().to_string()});
    body.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    let reply = post(state, "/tutor/closed-book", &body.to_string());
    assert_eq!(reply.status, 200, "{}", reply.body);
    serde_json::from_str(&reply.body).unwrap()
}

fn exercise(state: &mut AppState, message: &str, decision: Value) -> Value {
    let run = prepare_agent_chat(state, &json!({"message":message}).to_string(), "now")
        .unwrap_or_else(|e| panic!("{}", e.body));
    if decision["action"] != "start" {
        assert_eq!(
            run.tutor.as_ref().unwrap()["closed_book"]["status"],
            "active"
        );
    }
    let adapter = ChatStubAdapter::scripted(vec![call(
        "tutor.step",
        json!({"operation":"closed_book","closed_book":decision}),
    )]);
    let result = agent_run::execute_prepared(
        &agent_run::BorrowedAppPort(RefCell::new(state)),
        &adapter,
        run,
        Default::default(),
    );
    assert_eq!(result.reply.status, 200, "{}", result.reply.body);
    control(state, "state", json!({}))
}

#[test]
fn closed_book_selected_explanation_is_frozen_even_after_a_newer_answer() {
    let (_root,mut state)=fixture_with_assets(false);
    post(&mut state,"/agent/new","{}");
    let mut turn_ids=Vec::new();
    for (question,text) in [("停车为什么影响全程平均速度？","全程总时间包含停车，距离不变时总时间增加会降低平均速度。"),
        ("怎样比较资源下界？","比较计算与读取两项时间下限。")] {
        let run=prepare_agent_chat(&mut state,&json!({"message":question}).to_string(),"now").unwrap_or_else(|reply|panic!("{}",reply.body));
        turn_ids.push(run.turn_ref.turn_id.clone());
        let adapter=ChatStubAdapter::scripted(vec![answer(text)]);
        let result=agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut state)),&adapter,run,Default::default());
        assert_eq!(result.reply.status,200,"{}",result.reply.body);
    }
    let started=control(&mut state,"start",json!({"answer_turn_id":turn_ids[0],"target":"解释刚才内容"}));
    assert_eq!(started["target"],"停车为什么影响全程平均速度？");
    assert_eq!(started["answer_origin"]["turn_id"],turn_ids[0]);
    assert!(started["answer_origin"]["answer"].as_str().unwrap().contains("包含停车"));
    assert!(!started["answer_origin"]["answer"].as_str().unwrap().contains("资源"));
    let run=prepare_agent_chat(&mut state,&json!({"message":"开始练习"}).to_string(),"now").unwrap_or_else(|reply|panic!("{}",reply.body));
    assert_eq!(run.tutor.as_ref().unwrap()["closed_book"]["answer_origin"],started["answer_origin"]);
}

#[test]
fn closed_book_controls_replay_committed_operations_before_revision_checks() {
    let (_root, mut state) = fixture_with_assets(false);
    post(&mut state, "/agent/new", "{}");
    let chat = state.workspace.selected_chat.clone().unwrap();
    let start = json!({"action":"start","chat_session_id":chat,"revision":0,
        "operation_id":"lost-start-response","target":"全程平均速度"});
    let first = post(&mut state, "/tutor/closed-book", &start.to_string());
    assert_eq!(first.status, 200, "{}", first.body);
    let started: Value = serde_json::from_str(&first.body).unwrap();
    let replay = post(&mut state, "/tutor/closed-book", &start.to_string());
    assert_eq!(replay.status, 200, "{}", replay.body);
    assert_eq!(serde_json::from_str::<Value>(&replay.body).unwrap(), started);

    let pause = json!({"action":"pause","chat_session_id":chat,"revision":started["revision"],
        "operation_id":"lost-pause-response","draft":"全程时间包含停车"});
    let first = post(&mut state, "/tutor/closed-book", &pause.to_string());
    assert_eq!(first.status, 200, "{}", first.body);
    let paused: Value = serde_json::from_str(&first.body).unwrap();
    let resumed = control(&mut state, "resume", json!({"draft":"全程时间从出发到抵达"}));
    let replay = post(&mut state, "/tutor/closed-book", &pause.to_string());
    assert_eq!(replay.status, 200, "{}", replay.body);
    assert_eq!(serde_json::from_str::<Value>(&replay.body).unwrap(), paused);
    assert_eq!(control(&mut state, "state", json!({})), resumed,
        "replaying an older receipt must not overwrite newer progress or drafts");

    let mut stale = pause;
    stale["operation_id"] = json!("new-operation-with-stale-revision");
    let rejected = post(&mut state, "/tutor/closed-book", &stale.to_string());
    assert_eq!(rejected.status, 409, "{}", rejected.body);
    let failure: Value = serde_json::from_str(&rejected.body).unwrap();
    assert_eq!(failure["error_code"], "CLOSED_BOOK_REVISION_CONFLICT");
    assert_eq!(control(&mut state, "state", json!({})), resumed);

    post(&mut state, "/agent/new", "{}");
    let mut other_chat = start;
    other_chat["chat_session_id"] = json!(state.workspace.selected_chat);
    let rejected = post(&mut state, "/tutor/closed-book", &other_chat.to_string());
    assert_eq!(rejected.status, 400, "{}", rejected.body);
    assert!(control(&mut state, "state", json!({})).is_null());
}

#[test]
fn closed_book_spoken_start_and_stop_use_the_same_durable_resident_path() {
    let (_root, mut state) = fixture_with_assets(false);
    post(&mut state, "/agent/new", "{}");
    let started = exercise(
        &mut state,
        "开启闭卷练到会，练习刚讲过的全程平均速度。",
        json!({
        "action":"start","learner_quote":"开启闭卷练到会","target":"解释全程平均速度","criteria":"说明总时间的范围",
        "text":"出发与到达之间有一次停留，你会怎样确定全程平均速度所用的时间？","provides_help":false,"reason":"用户明确开启"}),
    );
    assert_eq!(started["status"], "active");
    let paused = exercise(
        &mut state,
        "暂停一下",
        json!({"action":"pause","text":"已经暂停，回来可以接着回答。","provides_help":false,"reason":"用户要求暂停"}),
    );
    assert_eq!(paused["status"], "paused");
    assert_eq!(paused["needs_retry"], false);
    control(&mut state, "resume", json!({}));
    let ended = exercise(
        &mut state,
        "先不练了，回去看看原文",
        json!({"action":"exit","text":"已保留当前进度，可以回去看资料了。","provides_help":false,"reason":"用户要求结束并查阅"}),
    );
    assert_eq!(ended["status"], "ended");
    assert_eq!(ended["needs_retry"], true);
    assert_eq!(ended["completion_reason"], "");
}

#[test]
fn closed_book_does_not_count_prepared_or_interrupted_delivery() {
    let (_root, mut state) = fixture_with_assets(false);
    post(&mut state, "/agent/new", "{}");
    control(&mut state, "start", json!({"target":"全程平均速度"}));
    for interrupted in [true, false] {
        let run = prepare_agent_chat(&mut state, r#"{"message":"开始提问"}"#, "now")
            .unwrap_or_else(|e| panic!("{}", e.body));
        teaching::step(&state.private_context(),&run.turn_ref,json!({"operation":"closed_book","closed_book":{
            "action":"question","criteria":"完整时间范围","text":"总时间包括什么？","provides_help":false,"reason":"初次邀请"}}),&[],&[]).unwrap();
        assert_eq!(
            control(&mut state, "state", json!({}))["visible_turn_ids"],
            json!([])
        );
        if !interrupted {
            control(&mut state, "pause", json!({}));
        }
        let mut delivered = outcome(None);
        delivered.incomplete = interrupted;
        finish(&mut state, &run.turn_ref, &delivered);
        let saved = control(&mut state, "state", json!({}));
        assert_eq!(saved["visible_turn_ids"], json!([]));
        assert_eq!(saved["criteria"], "");
    }
}

#[test]
fn closed_book_course_keeps_the_original_owner_and_progress() {
    let (_root, mut state) = fixture_with_assets(false);
    post(&mut state, "/agent/new", "{}");
    let mut course: memory::course::InquiryCourse = serde_json::from_str(include_str!(
        "../../../../docs/examples/inquiry-course/ai-infra-resource-bounds.v1.course.json"
    ))
    .unwrap();
    let lesson = &mut course.lessons[0];
    lesson.summary.source.publication.book_id = state.workspace.book.base.book_id.clone();
    lesson.summary.source.publication.publication_id = "local".into();
    lesson.summary.source.source_revision = state.workspace.book.source_fingerprint().into();
    let mut store = state.user.learning_store().unwrap();
    let before = store.state().unwrap();
    let opened=store.mutate_course(&serde_json::from_value(json!({"operation_id":"closed-course","run_id":null,
        "expected_revision":0,"release_version":1,"action":{"kind":"start","lesson_id":lesson.summary.id}})).unwrap(),
        Some(("fixture",1,lesson)),before.workspace_goal_revision.unwrap_or(0),"now",None).unwrap();
    let started = control(
        &mut state,
        "start",
        json!({"course_run_id":opened.progress.run_id}),
    );
    assert_eq!(started["owner_id"], opened.progress.run_id);
    assert_eq!(started["course"], true);
    assert_eq!(started["course_origin"]["stage"],"introduction");
    assert_eq!(started["course_origin"]["round_index"],0);
    assert_eq!(started["target"],lesson.summary.objective);
    assert!(started["result_ref"].is_null());
    exercise(
        &mut state,
        "开始",
        json!({"action":"question","criteria":"比较时间约束","text":"怎样比较两个资源耗时？","provides_help":false,"reason":"观察当前课程理解"}),
    );
    let completed=exercise(
        &mut state,
        "需要同时满足两项约束，较慢的一项决定下界。",
        json!({"action":"complete",
        "text":"你独立说明了两项约束。","learner_quote":"需要同时满足两项约束，较慢的一项决定下界","provides_help":false,"reason":"本次能解释下界，不替代原课程任务"}),
    );
    let turn=completed["visible_turn_ids"].as_array().unwrap().last().unwrap().as_str().unwrap();
    assert_eq!(completed["result_ref"],format!("closed-book:delivery:{turn}"));
    assert_eq!(
        store.course_view(&opened.progress.run_id).unwrap().progress,
        opened.progress
    );
    assert_eq!(store.state().unwrap().sessions.len(), before.sessions.len());
}

#[test]
#[ignore = "configured-provider semantic trial against a local published source; writes isolated evidence"]
fn closed_book_live_model_trial() {
    let directory = std::env::var("CLOSED_BOOK_TRIAL_SOURCE").expect("CLOSED_BOOK_TRIAL_SOURCE");
    let output = std::path::PathBuf::from(
        std::env::var("CLOSED_BOOK_TRIAL_OUTPUT").expect("CLOSED_BOOK_TRIAL_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let source =
        std::fs::read_to_string(std::path::Path::new(&directory).join("source.txt")).unwrap();
    let book = Book::load(&directory).unwrap();
    let (_root, mut state) = fixture_with_book(false, book, &source);
    post(&mut state, "/agent/new", "{}");
    let config = runtime::ProviderConfig::from_env().unwrap();
    let model = config.model.clone();
    let adapter = runtime::ProviderRegistry::adapter_from_config(config);
    let replay = |state: &mut AppState, index: usize, message: &str| -> Value {
        let run = prepare_agent_chat(state, &json!({"message":message}).to_string(), "now")
            .unwrap_or_else(|e| panic!("{}", e.body));
        let turn = run.turn_ref.clone();
        let began = std::time::Instant::now();
        let result = agent_run::execute_prepared(
            &agent_run::BorrowedAppPort(RefCell::new(state)),
            adapter.as_ref(),
            run,
            Default::default(),
        );
        let store = state.user.learning_store().unwrap();
        let proposal = store
            .teaching_event(&format!("closed-book-proposal:{}", turn.turn_id))
            .ok();
        let saved = store
            .closed_book(&turn.session_id, &state.workspace.book.base.book_id)
            .unwrap();
        let response = state
            .user
            .agent_history
            .sessions
            .iter()
            .find(|s| s.id == turn.session_id)
            .unwrap()
            .turns
            .last()
            .unwrap();
        let report = json!({"model":model,"source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint(),
            "message":message,"elapsed_ms":began.elapsed().as_millis(),"reply_status":result.reply.status,
            "answer":response.outcome.as_ref().and_then(|o|o.answer.as_ref()),"decision":proposal.map(|p|p.payload),"state":saved,
            "tokens_spent":response.outcome.as_ref().map(|outcome|outcome.tokens_spent),"scope":"scripted learner responses through the real Resident and configured model; isolated private state"});
        std::fs::write(
            output.join(format!("turn-{index}.json")),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        assert_eq!(result.reply.status, 200, "{}", result.reply.body);
        eprintln!(
            "Closed-book trial turn {index}: {}",
            report["decision"]["decision"]["action"]
        );
        report
    };
    replay(&mut state,0,"请依据《深入理解 AI Infra》的原文，简短讲解：容量足够且计算和读写充分重叠时，为什么耗时下界取两项必要时间的最大值，以及为什么它不等于实际耗时。现在只讲解，不出题。");
    control(
        &mut state,
        "start",
        json!({"target":"解释充分重叠时两项时间约束及下界含义，并独立判断算力改变后的下界"}),
    );
    let first = replay(
        &mut state,
        1,
        "开启闭卷练到会，请围绕刚讲过的重点先让我自己解释，等我回答。",
    );
    assert_eq!(first["decision"]["decision"]["action"], "question");
    let vague = replay(&mut state, 2, "懂了。");
    assert_ne!(vague["state"]["status"], "complete");
    let help = replay(
        &mut state,
        3,
        "我还是不知道为什么，请直接把缺的这一步讲明白。",
    );
    assert_eq!(help["state"]["assisted"], true);
    let retry=replay(&mut state,4,"换一个同目标的新问题让我独立试。请用计算时间下界 8ms、读写时间下界 3ms、容量充足且充分重叠、只将算力翻倍这个新条件出题，先不要给答案。");
    assert_eq!(retry["decision"]["decision"]["action"], "retry");
    assert_eq!(retry["state"]["assisted"], false);
    let done=replay(&mut state,5,"算力翻倍后，计算所需时间下界从 8ms 变为 4ms，读写仍需 3ms。任务要同时满足两项资源的时间约束，所以在容量充足、计算和读写充分重叠的条件下，总时间下界是 max(4,3)=4ms，而不是相加为 7ms。4ms 只是理想下界，不保证实际耗时正好是 4ms，实际还有调度等开销。");
    assert_eq!(
        done["state"]["status"], "complete",
        "The frozen target is now satisfied by an independent explanation and application"
    );
}

#[test]
fn closed_book_real_resident_delivery_help_retry_and_completion() {
    let (_root, mut state) = fixture_with_assets(false);
    post(&mut state, "/agent/new", "{}");
    let started = control(
        &mut state,
        "start",
        json!({"target":"Explain whole-trip average speed"}),
    );
    assert_eq!(started["status"], "active");
    let first = exercise(
        &mut state,
        "Start",
        json!({"action":"question","target":"Explain whole-trip average speed",
        "criteria":"Include stopped time and explain its effect","text":"Why does a stop change the average speed of a whole journey?","provides_help":false,"reason":"Observe the denominator"}),
    );
    assert_eq!(first["visible_turn_ids"].as_array().unwrap().len(), 1);
    control(
        &mut state,
        "draft",
        json!({"draft":"My unfinished explanation"}),
    );
    let helped = exercise(
        &mut state,
        "Please explain",
        json!({"action":"help","text":"Total elapsed time includes the stop; the distance does not increase during it.","provides_help":true,"reason":"Supply the missing relationship"}),
    );
    assert_eq!(helped["assisted"], true);
    assert_eq!(
        helped["draft"], "My unfinished explanation",
        "receiving help must not consume an unsent draft"
    );
    let attempt = exercise(
        &mut state,
        "Try a new problem",
        json!({"action":"retry","text":"Two journeys cover the same distance. A takes 12 minutes including stops; B takes 15. Which has greater whole-trip average speed, and why?","provides_help":false,"reason":"Independent comparison"}),
    );
    assert_eq!(attempt["attempt"], 2);
    assert_eq!(attempt["assisted"], false);
    assert_eq!(attempt["visible_turn_ids"].as_array().unwrap().len(), 1);
    assert_ne!(attempt["visible_turn_ids"][0], first["visible_turn_ids"][0]);
    let done = exercise(
        &mut state,
        "A: the same distance divided by less total elapsed time gives a higher speed.",
        json!({"action":"complete",
        "text":"You used total elapsed time to compare both journeys independently.","provides_help":false,
        "learner_quote":"the same distance divided by less total elapsed time gives a higher speed","reason":"Correct relationship and comparison, with no help in this attempt"}),
    );
    assert_eq!(done["status"], "complete");
    assert!(
        state
            .user
            .agent_history
            .sessions
            .last()
            .unwrap()
            .turns
            .len()
            >= 4
    );
}

#[test]
#[ignore = "paid single-sample course reuse trial; retains failed evidence and requires an isolated output directory"]
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
    let provider=runtime::ProviderRegistry::adapter_from_config_with_timeout(config,Duration::from_secs(180));
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
        let result=bounded(|cancel|agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut *state)),&adapter,run,cancel));
        let store=state.user.learning_store().map_err(|error|error.message)?;
        let proposal=store.teaching_event(&format!("closed-book-proposal:{}",turn.turn_id)).ok();
        let practice=store.closed_book(&turn.session_id,&state.workspace.book.base.book_id).map_err(|error|error.message)?;
        let ordinary_coach=store.coach_job_for_source(&view.progress.run_id,memory::tutor_coach::CoachJobOrigin::AfterTurn,&turn.turn_id)
            .map_err(|error|error.message)?;
        let response=state.user.agent_history.sessions.iter().find(|session|session.id==turn.session_id).unwrap()
            .turns.iter().find(|saved|saved.turn_id==turn.turn_id).unwrap();
        let calls=adapter.usage.lock().unwrap().calls[calls_before..].to_vec();
        let report=json!({"turn_ref":turn.turn_id,"message":message,"reply_status":result.reply.status,
            "elapsed_ms":step_started.elapsed().as_millis(),"frozen_course":frozen_course,
            "answer":response.outcome.as_ref().and_then(|outcome|outcome.answer.as_ref()),"error":response.error,
            "decision":proposal.map(|event|event.payload),"practice":practice,"ordinary_after_turn_coach":ordinary_coach,
            "usage_calls":calls,"usage_scope":"Resident and its ordinary after-delivery Coach; the original-course reuse Coach runs separately"});
        write(&output,&format!("turn-{index}.json"),&report);
        eprintln!("Course reuse trial turn {index}: {}",report["decision"]["decision"]["action"]);
        if result.reply.status!=200 { return Err(format!("Resident turn {index} failed with HTTP {}",result.reply.status)); }
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
        write(&output,"closed-book-completion.json",&done);
        if done["practice"]["status"]!="complete" || done["practice"]["assisted"]!=false {return Err("The real closed-book teacher did not accept this independent answer; no replay is attempted".into());}
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
        let executed=bounded(|cancel|crate::teaching_coach::drive_inline(&agent_run::BorrowedAppPort(RefCell::new(&mut state)),&adapter,None,&turn,false,&cancel));
        let reviewed=store.coach_job(&expression.job_ref).map_err(|error|error.message)?.unwrap();
        write(&output,"original-course-coach.json",&json!({"elapsed_ms":coach_started.elapsed().as_millis(),"job":reviewed,
            "error":executed.as_ref().err(),"usage_calls":adapter.usage.lock().unwrap().calls[calls_before..]}));
        executed.map_err(|error|error.message)?;
        if reviewed.status!=memory::tutor_coach::CoachJobStatus::Ready {return Err("Original-course Coach did not reach Ready; inspect retained error and captures".into());}
        view=act(&mut store,&view,"trial-read-feedback",CourseAction::ReadFeedback{job_ref:expression.job_ref},goal)?;
        let displayed=store.course_view(&view.progress.run_id).map_err(|error|error.message)?;
        write(&output,"feedback-read.json",&json!(displayed));
        if store.course_run(&view.progress.run_id).map_err(|error|error.message)?.reflection.is_none() {
            return Err("Original-course Coach did not accept the imported reflection; its concrete gap is retained".into());
        }
        view=act(&mut store,&view,"trial-next-round",CourseAction::Advance,goal)?;
        if view.round_index!=1 || view.progress.stage!=CourseStage::Prediction {return Err("Accepted reflection did not return to the next original course round".into());}
        write(&output,"next-course-step.json",&json!(view));
        Ok(json!({"result_ref":result_ref,"next_round":view.round_index,"next_stage":view.progress.stage,"course_complete":false}))
    })();
    let usage=adapter.usage.lock().unwrap().clone();
    write(&output,"trial-result.json",&json!({"model":model,"elapsed_ms":began.elapsed().as_millis(),"passed":attempt.is_ok(),
        "result":attempt.as_ref().ok(),"failure":attempt.as_ref().err(),"usage":usage,"cost":cost(&usage,official_flash),
        "sample_count":1,"planned_logical_calls":9,"step_timeout_seconds":180,
        "planned_scope":"Scripted learner through real Resident deliveries, ordinary after-delivery Coaches, actual persisted completion reference, original course Coach, ReadFeedback and Advance. A failed attempt stops at the recorded failure; missing later artifacts mean those steps were not executed. No synthetic model proposals or judgments; no rerun to obtain a passing sample.",
        "not_covered":["Human first-use learning experience","Decision/transfer semantic live sample","Prediction Coach"]}));
    assert!(attempt.is_ok(),"{}",attempt.unwrap_err());
}

#[test]
fn closed_book_unassisted_answer_cannot_trigger_an_unrequested_retry() {
    let (_root, mut state) = fixture_with_assets(false);
    post(&mut state, "/agent/new", "{}");
    control(&mut state, "start", json!({"target":"时间范围"}));
    exercise(
        &mut state,
        "开始",
        json!({"action":"question","criteria":"说明整个旅程的时间","text":"旅程包含一次停留，应计入哪段时间？","provides_help":false,"reason":"当前目标"}),
    );
    let run = prepare_agent_chat(
        &mut state,
        r#"{"message":"应计入从出发到抵达的全部时间，因为停留也占用了旅程的时间。"}"#,
        "now",
    )
    .unwrap_or_else(|e| panic!("{}", e.body));
    assert_eq!(
        run.tutor.as_ref().unwrap()["closed_book"]["current_question"],
        "旅程包含一次停留，应计入哪段时间？"
    );
    let error = teaching::step(
        &state.private_context(),
        &run.turn_ref,
        json!({"operation":"closed_book","closed_book":{
        "action":"retry","text":"再换一道题","provides_help":false,"reason":"之前得到过帮助"}}),
        &[],
        &[],
    )
    .unwrap_err();
    assert!(error.message.contains("本次尝试未受助"));
    teaching::step(&state.private_context(),&run.turn_ref,json!({"operation":"closed_book","closed_book":{
        "action":"complete","text":"你独立解释了时间范围。","provides_help":false,"learner_quote":"停留也占用了旅程的时间","reason":"当前未受助回答说明了完整时间范围"}}),&[],&[]).unwrap();
    let mut delivered = outcome(None);
    delivered.answer = Some("你独立解释了时间范围。".into());
    finish(&mut state, &run.turn_ref, &delivered);
    assert_eq!(
        control(&mut state, "state", json!({}))["status"],
        "complete"
    );
}

#[test]
fn closed_book_clarifying_a_topic_does_not_complete_the_learning_goal() {
    let (_root, mut state) = fixture_with_assets(false);
    post(&mut state, "/agent/new", "{}");
    control(&mut state, "start", json!({}));
    let clarification = exercise(
        &mut state,
        "我想练到会",
        json!({"action":"clarify","text":"你想练哪个概念或哪道题？","provides_help":false,"reason":"先确定对象"}),
    );
    assert_eq!(clarification["criteria"], "");
    assert_eq!(clarification["status"], "active");
    let question = exercise(
        &mut state,
        "平均速度",
        json!({"action":"question","target":"解释平均速度的时间范围","criteria":"说明旅程的全部时间",
        "text":"旅程中停留的一段时间该怎样处理？为什么？","provides_help":false,"reason":"现在可以观察理解"}),
    );
    assert_eq!(question["visible_turn_ids"].as_array().unwrap().len(), 1);
    assert_ne!(
        question["visible_turn_ids"][0],
        clarification["visible_turn_ids"][0]
    );
    assert_eq!(question["status"], "active");
}

#[test]
fn closed_book_cannot_complete_assisted_or_unanswered_and_preserves_resume_conditions() {
    let (_root, mut state) = fixture_with_assets(false);
    post(&mut state, "/agent/new", "{}");
    control(&mut state, "start", json!({"target":"Total time"}));
    exercise(
        &mut state,
        "Start",
        json!({"action":"question","criteria":"Explain time","text":"What time belongs in the denominator?","provides_help":false,"reason":"Observe explanation"}),
    );
    control(&mut state, "help", json!({}));
    let run = prepare_agent_chat(&mut state, r#"{"message":"All elapsed time"}"#, "now")
        .unwrap_or_else(|e| panic!("{}", e.body));
    let result = teaching::step(
        &state.private_context(),
        &run.turn_ref,
        json!({"operation":"closed_book","closed_book":{
        "action":"complete","text":"Done","learner_quote":"All elapsed time","provides_help":false,"reason":"Correct"}}),
        &[],
        &[],
    );
    assert!(result.is_err());
    let paused = control(&mut state, "pause", json!({"draft":"Unfinished thought"}));
    assert_eq!(paused["draft"], "Unfinished thought");
    let restored = control(&mut state, "resume", json!({}));
    assert_eq!(restored["assisted"], true);
    assert_eq!(
        restored["needs_retry"], false,
        "pausing keeps sources covered and preserves the attempt"
    );
    control(&mut state, "exit", json!({}));
    assert_eq!(control(&mut state, "state", json!({}))["needs_retry"], true);
    assert!(teaching::step(
        &state.private_context(),
        &run.turn_ref,
        json!({"operation":"closed_book","closed_book":{
        "action":"retry","text":"Another question","provides_help":false}}),
        &[],
        &[]
    )
    .is_err());
    post(&mut state, "/agent/new", "{}");
    assert!(control(&mut state, "state", json!({})).is_null());
}
