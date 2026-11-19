use super::*;

pub fn control(state: &mut AppState, action: &str, extra: Value) -> Value {
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

pub fn exercise(state: &mut AppState, message: &str, decision: Value) -> Value {
    exercise_with_review(state, message, decision, None)
}

fn exercise_with_review(state: &mut AppState, message: &str, decision: Value, review: Option<Value>) -> Value {
    let run = prepare_agent_chat(state, &json!({"message":message}).to_string(), "now")
        .unwrap_or_else(|e| panic!("{}", e.body));
    if decision["action"] != "start" {
        assert_eq!(
            run.tutor.as_ref().unwrap()["closed_book"]["status"],
            "active"
        );
    }
    let mut samples = vec![call(
        "tutor.step",
        json!({"operation":"closed_book","closed_book":decision}),
    )];
    if let Some(v) = review {
        for step in v.as_array().map(Vec::as_slice).unwrap_or(std::slice::from_ref(&v)) {
            samples.push(answer(&step.to_string()));
        }
    }
    let adapter = ChatStubAdapter::scripted(samples);
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
fn closed_book_overview_without_authored_text_starts_and_requires_a_new_application() {
    use crate::tests::teaching_design_tests::{chapter_fixture, purpose_input};
    let (_root, mut state) = chapter_fixture();
    let owner = state.user.learning_store().unwrap().state().unwrap().control;
    let end = json!({"operation_id":"end-before-overview","expected_revision":owner.revision,
        "action":{"kind":"end","session_id":owner.current_tutor_session_id.unwrap()}});
    assert_eq!(post(&mut state, "/tutor/mutate", &end.to_string()).status, 200);
    post(&mut state, "/agent/new", "{}");
    let input = purpose_input(&state, "discover", "1", "overview");
    let run = prepare_agent_chat(&mut state, &input.to_string(), "now").unwrap_or_else(|e| panic!("{}", e.body));
    let turn = run.turn_ref.clone();
    let prior_answer = "列子得靠风，风不吹就飞不了。";
    let adapter = ChatStubAdapter::scripted(vec![answer(prior_answer)]);
    let result = agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut state)), &adapter, run, Default::default());
    assert_eq!(result.reply.status, 200, "{}", result.reply.body);
    let saved = state.user.agent_history.sessions.iter().find(|s| s.id == turn.session_id).unwrap().turns.last().unwrap();
    assert_eq!(saved.user_authored_text(), "");
    let requested = saved.domain.interaction.as_ref().unwrap().accepted_request.clone();
    let started = control(&mut state, "start", json!({"answer_turn_id":turn.turn_id}));
    assert_eq!(started["status"], "active");
    assert_eq!(started["target"], requested);
    assert_eq!(started["answer_origin"]["answer"], prior_answer);

    let propose = |state: &AppState, turn: &AgentTurnRef, decision: Value| {
        teaching::step(&state.private_context(), turn, json!({"operation":"closed_book","closed_book":decision}), &[], &[])
    };
    let deliver = |state: &mut AppState, turn: &AgentTurnRef, text: &str| {
        let mut delivered = outcome(None);
        delivered.answer = Some(text.into());
        delivered.answer_view.as_mut().unwrap().parts = vec![AgentAnswerPart::Markdown { text: text.into() }];
        finish(state, turn, &delivered);
    };
    let ask = prepare_agent_chat(&mut state, r#"{"message":"我来解释一下"}"#, "now").unwrap_or_else(|e| panic!("{}", e.body));
    let mut question = json!({"action":"question","target":"判断对外部条件的依赖","criteria":"用情境中的依赖关系说明判断",
        "new_case":"列子得靠风","text":"风不吹会怎样？","provides_help":false,"reason":"检验依赖关系"});
    assert!(propose(&state, &ask.turn_ref, question.clone()).is_err());
    let changed = "船在无风时改用双桨。";
    let transfer_question = format!("{changed}\n\n它还依赖什么条件？");
    question["new_case"] = json!(changed);
    question["text"] = json!("它还依赖什么条件？");
    question["criteria"] = json!("须说列子靠风");
    let offered = propose(&state, &ask.turn_ref, question).unwrap();
    assert_eq!(offered["delivery_draft"]["text"], transfer_question);
    deliver(&mut state, &ask.turn_ref, &transfer_question);
    let active = control(&mut state, "state", json!({}));
    assert_eq!(active["transfer_condition"], changed);
    assert_eq!(active["criteria"], "它还依赖什么条件？");

    let echo = prepare_agent_chat(&mut state, &json!({"message":prior_answer,"response_language":"en"}).to_string(), "now").unwrap_or_else(|e| panic!("{}", e.body));
    let mut completion = json!({"action":"complete","text":"已完成","learner_quote":prior_answer,
        "provides_help":false,"reason":"答对了"});
    assert!(propose(&state, &echo.turn_ref, completion.clone()).is_err());
    {
        let port = agent_run::BorrowedAppPort(RefCell::new(&mut state));
        let acp_prompt = crate::acp_teaching::prompt(&port, &echo).unwrap();
        assert!(acp_prompt.contains(&format!("transfer_condition=\"{changed}\"")));
        assert!(acp_prompt.contains(prior_answer));
        assert!(crate::acp_teaching::answer(&port, &echo, &completion.to_string()).is_err());
    }
    let neutral = propose(&state, &echo.turn_ref, json!({"action":"followup","text":"要靠人力划船。","followup_focus":"apply_case",
        "provides_help":false,"reason":"还只讲了列子"})).unwrap();
    let prompt = neutral["delivery_draft"]["text"].as_str().unwrap();
    assert!(prompt.starts_with("What do you conclude"));
    assert!(!prompt.contains("人力"));
    deliver(&mut state, &echo.turn_ref, prompt);
    assert_eq!(control(&mut state, "state", json!({}))["assisted"], false);
    let still_old = "没有气流，人就无法飞行。";
    let blind = json!({"uses_original_relationship":true,"prior_answer_suffices":false,"prior_quote":"列子得靠风","case_quote":"改用双桨",
        "reason":"新交通工具由双桨提供动力，需要说明划桨人的作用。"});
    completion["learner_quote"] = json!(still_old);
    let needs_application = exercise_with_review(&mut state, still_old, completion.clone(), Some(json!([blind, {
        "verdict":"missing_application","case_quote":"无风时改用双桨","learner_quote":"没有气流",
        "reason":"这仍在说明旧例飞行的条件，尚未解释划船。"
    }])));
    assert_eq!(needs_application["status"], "active");
    assert_eq!(needs_application["assisted"], false);
    assert!(needs_application["result_ref"].is_null());
    let independent = "不用风了，但要有人划船才会前进。";
    completion["learner_quote"] = json!(independent);
    completion["reason"] = json!("指出新的依赖由风变成了人的划动。");
    let pending = exercise_with_review(&mut state, independent, completion.clone(), Some(json!([blind, {
        "verdict":"independent","case_quote":"引文不存在","learner_quote":independent,"reason":"推理充分"
    }])));
    assert_eq!(pending["status"], "active");
    assert_eq!(pending["assisted"], false);
    assert!(pending["result_ref"].is_null());
    let latest = state.user.agent_history.sessions.iter().find(|s| Some(&s.id) == state.workspace.selected_chat.as_ref())
        .and_then(|s| s.turns.last()).and_then(|t| t.outcome.as_ref()).and_then(|o| o.answer.as_deref());
    assert!(latest.is_some_and(|text| text.contains("还没能确认独立完成")));
    let done = exercise_with_review(&mut state, independent, completion, Some(json!([blind, {
        "verdict":"independent","case_quote":"改用双桨","learner_quote":independent,
        "reason":"学习者由双桨推导出人的划动，而非复述旧题中的借风。"
    }])));
    assert_eq!(done["status"], "complete");
    assert_eq!(done["learner_quote"], independent);
}

#[test]
fn closed_book_transfer_review_keeps_the_rewrapped_original_passage_assisted() {
    let (_root, mut state) = fixture_with_assets(false);
    post(&mut state, "/agent/new", "{}");
    let source = prepare_agent_chat(&mut state, r#"{"message":"我跳过了这个活动，请调整接下来的讲法。"}"#, "now")
        .unwrap_or_else(|e| panic!("{}", e.body));
    let origin = source.turn_ref.turn_id.clone();
    let adapter = ChatStubAdapter::scripted(vec![answer("他飞起来靠的是风（御风而行），风在他身体外面，风不吹就飞不了。所以省掉走路不等于不用依靠。")]);
    let result = agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut state)), &adapter, source, Default::default());
    assert_eq!(result.reply.status, 200);
    control(&mut state, "start", json!({"answer_turn_id":origin}));
    let case = "《庄子·逍遥游》里有这样一句：夫列子御风而行，泠然善也，旬有五日而后反。彼于致福者，未数数然也。此虽免乎行，犹有所待者也。（大意：列子驾着风飞行，轻妙美好，过了十五天才返回。他对求福的事，并不急切追求。这样做虽然免去了步行，却还是有所依靠。）";
    let question = "列子这样算不算真正无所待？给结论，并用他自己和风的关系说明理由。";
    exercise(&mut state, "出题", json!({"action":"question","new_case":case,"text":question,"provides_help":false,"reason":"检查理解"}));
    let learner = "他不用走路了，但还得靠风，风不来他就飞不了。句子里说御风而行，所以还是有所待。";
    let decision = json!({"action":"complete","learner_quote":"风不来他就飞不了","text":"你可以独立完成。","reason":"说出了依赖","provides_help":false});
    let review = json!({"uses_original_relationship":true,"prior_answer_suffices":true,"prior_quote":"他飞起来靠的是风","case_quote":"用他自己和风的关系说明理由",
        "reason":"原讲解与题面同问列子免于步行后是否仍依赖风。"});
    let saved = exercise_with_review(&mut state, learner, decision, Some(review));
    assert_eq!(saved["status"], "active");
    assert_eq!(saved["assisted"], true);
    assert!(saved["result_ref"].is_null());
    let fresh = exercise(&mut state, "再来一题", json!({"action":"retry","new_case":"另一段写道「汤之问棘也是已」。假设它是寓言性叙述。","text":"商汤是否真的问过棘？请说明判断依据。","provides_help":false,"reason":"换新材料"}));
    assert_eq!(fresh["attempt"], 2);
    assert_eq!(fresh["assisted"], false);
    assert!(fresh["result_ref"].is_null());
    let reply = "从寓言的写法无法考证商汤的历史行迹。";
    let unrelated = exercise_with_review(&mut state, reply, json!({"action":"complete","learner_quote":reply,
        "text":"这点已经能够独立完成。","reason":"解释了历史考据","provides_help":false}), Some(json!({
        "uses_original_relationship":false,"prior_answer_suffices":false,"prior_quote":"他飞起来靠的是风","case_quote":"假设它是寓言性叙述",
        "reason":"判断叙述的史料价值无法证明读者理解飞行的依赖条件。"
    })));
    assert_eq!(unrelated["status"], "active");
    assert_eq!(unrelated["needs_retry"], true);
    assert_eq!(unrelated["assisted"], false);
    assert!(unrelated["result_ref"].is_null());
}

#[test]
fn closed_book_echoing_an_authored_balloon_correction_cannot_complete() {
    let (_root, mut state) = fixture_with_assets(false);
    post(&mut state, "/agent/new", "{}");
    let source = prepare_agent_chat(&mut state, r#"{"message":"列子为什么仍是有所待？"}"#, "now").unwrap_or_else(|e| panic!("{}", e.body));
    let source_turn = source.turn_ref.clone();
    let adapter = ChatStubAdapter::scripted(vec![answer("列子御风而行，不用走路但仍然要靠风。")]);
    let result = agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut state)), &adapter, source, Default::default());
    assert_eq!(result.reply.status, 200, "{}", result.reply.body);
    control(&mut state, "start", json!({"answer_turn_id":source_turn.turn_id}));

    let situation = "有人造了一艘飞艇，出门不用走路，全靠艇身外的气囊浮力带着走；气囊一旦漏气，就哪也去不了。";
    let task = "能不能因此说他“无所待”？请用这个例子说说你的判断和理由。";
    let first = exercise(&mut state, "请出题", json!({"action":"question","new_case":situation,"text":task,
        "criteria":"列子仍需依靠风","target":"判断对外部条件的依赖","provides_help":false,"reason":"换情境"}));
    assert_eq!(first["transfer_condition"], situation);
    assert_eq!(first["criteria"], task);
    let previous_answer = "他不用走路了，但还得靠风，风不来他就飞不了。";
    let correction = "不过这次的例子里，飞艇靠的是艇身外的气囊浮力，并不是风。换成这个例子再看：气囊一旦漏气，飞艇就动不了，这算不算“有所待”？";
    let corrected = exercise(&mut state, previous_answer, json!({"action":"followup","text":correction,
        "provides_help":false,"reason":"请把回答应用到飞艇"}));
    assert_eq!(corrected["assisted"], true);
    let repeated = "这还是有所待。飞艇靠的是艇身外的气囊浮力，气囊一旦漏气，飞艇就动不了。";
    let echo = prepare_agent_chat(&mut state, &json!({"message":repeated}).to_string(), "now").unwrap_or_else(|e| panic!("{}", e.body));
    let text = crate::acp_teaching::answer(&agent_run::BorrowedAppPort(RefCell::new(&mut state)), &echo,
        &json!({"action":"complete","learner_quote":repeated,"text":"你能独立完成。","provides_help":false,"reason":"正确指出新的依赖"}).to_string()).unwrap();
    assert!(text.contains("刚才获得的帮助"));
    assert!(!text.contains("你能独立完成"));
    let mut acknowledged = outcome(None);
    acknowledged.answer = Some(text.clone());
    acknowledged.answer_view.as_mut().unwrap().parts = vec![AgentAnswerPart::Markdown { text }];
    finish(&mut state, &echo.turn_ref, &acknowledged);
    let saved = control(&mut state, "state", json!({}));
    assert_eq!(saved["status"], "active");
    assert_eq!(saved["assisted"], true);
    assert!(saved["result_ref"].is_null());
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
fn follow_up_course_preview_delivery_and_return_keep_original_step() {
    let (_root, mut state) = fixture_with_assets(false);
    (state.user.agent_history, state.user.session_store) =
        crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    json_post(&mut state, "/agent/new", json!({}));
    let parent = state.workspace.selected_chat.clone().unwrap();
    let mut course: memory::course::InquiryCourse = serde_json::from_str(include_str!(
        "../../../../docs/examples/inquiry-course/ai-infra-resource-bounds.v1.course.json"
    )).unwrap();
    let lesson = &mut course.lessons[0];
    lesson.summary.source.publication.book_id = state.workspace.book.base.book_id.clone();
    lesson.summary.source.publication.publication_id = "local".into();
    lesson.summary.source.source_revision = state.workspace.book.source_fingerprint().into();
    let mut store = state.user.learning_store().unwrap();
    let goal = store.state().unwrap().workspace_goal_revision.unwrap_or(0);
    let initial = store.mutate_course(&serde_json::from_value(json!({
        "operation_id":"open","run_id":null,"expected_revision":0,"release_version":1,
        "action":{"kind":"start","lesson_id":lesson.summary.id}
    })).unwrap(), Some(("fixture",1,lesson)), goal, "now", None).unwrap();
    let run_id = &initial.progress.run_id;
    let context = json!({"run_id":run_id,"revision":initial.progress.revision,"surface":"course"});
    let question = "这两个时间如何比较？";
    let parent_run = prepare_agent_chat(&mut state,
        &json!({"message":question,"course_context":context}).to_string(), "now").unwrap_or_else(|e| panic!("{}",e.body));
    let parent_turn = parent_run.turn_ref;
    let mut delivered = outcome(None);
    let answer = "比较两个时间下界。";
    delivered.answer = Some(answer.into());
    delivered.answer_view = Some(serde_json::from_value(json!({
        "parts":[{"kind":"markdown","text":answer}],"sources":[]
    })).unwrap());
    finish(&mut state, &parent_turn, &delivered);

    let initial_events = store.teaching_events(run_id,0,100).unwrap().len();
    let path = format!("/agent/follow-ups?session_id={parent}");
    assert_eq!(get(&mut state, &path).status, 200);
    let request_id = uuid::Uuid::now_v7().to_string();
    let first_question = "时间下界是什么意思？";
    let selected = json!({"parent_session_id":parent,"parent_turn_id":parent_turn.turn_id,
        "anchor":{"start_part":0,"end_part":0,"start_offset":4,"end_offset":8,"excerpt":"时间下界"},
        "question":first_question,"client_request_id":request_id});
    let branch = json_post(&mut state, "/agent/follow-ups", selected.clone());
    let child = branch["session_id"].as_str().unwrap();
    assert_eq!(state.workspace.selected_chat.as_deref(), Some(parent.as_str()));
    assert_eq!(store.teaching_events(run_id,0,100).unwrap().len(), initial_events);
    json_post(&mut state, "/agent/history/select", json!({"session_id":child}));

    // A valid course under a newer copy of the same local book is not the
    // original material. Opening it cannot attribute this branch to that course.
    let original_book = state.workspace.book.clone();
    state.workspace.book = std::sync::Arc::new(Book::new(original_book.base.clone(), "Different edition. ".repeat(30).as_str()));
    let mut revised_lesson = lesson.clone();
    revised_lesson.summary.source.source_revision = state.workspace.book.source_fingerprint().into();
    let revised = store.mutate_course(&serde_json::from_value(json!({
        "operation_id":"open-revised","run_id":null,"expected_revision":0,"release_version":1,
        "action":{"kind":"start","lesson_id":revised_lesson.summary.id}
    })).unwrap(),Some(("revised",1,&revised_lesson)),goal,"now",None).unwrap();
    let other_course = json!({"run_id":revised.progress.run_id,"revision":revised.progress.revision,"surface":"course"});
    assert!(crate::course_context::freeze_chat(&state.user,&state.workspace,&other_course).unwrap().is_none());
    state.workspace.book = original_book;

    let child_run = prepare_agent_chat(&mut state,
        &json!({"message":first_question,"session_id":child,"client_request_id":request_id,
            "expected_chat_head":null,"course_context":context}).to_string(),
        "now").unwrap_or_else(|e| panic!("{}",e.body));
    let child_turn = child_run.turn_ref;
    let binding = store.teaching_event(&format!("binding:{}",child_turn.turn_id)).unwrap();
    assert_eq!(binding.binding.tutor_session_id,*run_id);
    assert_eq!(binding.binding.chat_session_id,child);
    assert_eq!(binding.binding.source_id,lesson.summary.source.publication.book_id);
    assert_eq!(binding.binding.session_revision,initial.progress.revision);
    assert_eq!(binding.payload["inquiry_course"]["lesson"]["progress"]["stage"],"introduction");
    finish(&mut state, &child_turn, &outcome(None));
    teaching::record_delivery(&state.private_context(), &child_turn, "now").unwrap();
    json_post(&mut state, "/agent/history/select", json!({"session_id":parent}));
    assert_eq!(store.course_view(run_id).unwrap(), initial);
    let help = format!("usage:{run_id}:assistant:{}",child_turn.turn_id);
    let events = store.teaching_events(run_id,0,100).unwrap();
    assert_eq!(events.iter().filter(|(_,e)|e.event_id==help).count(),1);

    // The next actual course response carries the child's delivered help exactly once.
    store.mutate_course(&serde_json::from_value(json!({"operation_id":"question",
        "run_id":run_id,"expected_revision":initial.progress.revision,"release_version":1,
        "action":{"kind":"ask_question","response":"我想比较它们的大小。"}
    })).unwrap(),None,goal,"now",None).unwrap();
    let fact = store.teaching_event(&format!("{run_id}:question:question")).unwrap();
    assert_eq!(fact.payload["support_delivery_refs"].as_array().unwrap().iter()
        .filter(|r|r.as_str()==Some(help.as_str())).count(),1);

    control(&mut state,"start",json!({"course_run_id":run_id}));
    for status in ["active","paused"] {
        if status == "paused" { control(&mut state,"pause",json!({})); }
        for rejected in [get(&mut state, &path), post(&mut state, "/agent/follow-ups", &selected.to_string())] {
            assert_eq!(rejected.status, 409, "{}", rejected.body);
            let failure: Value = serde_json::from_str(&rejected.body).unwrap();
            assert_eq!(failure["error_code"], "FOLLOW_UP_CLOSED_BOOK");
        }
    }
    control(&mut state,"exit",json!({}));
    assert_eq!(get(&mut state,&path).status,200);
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

pub const COURSE_TRIAL_MAX_FOLLOWUPS: usize = 3;

pub fn finish_course_trial_dialogue(
    mut report: Value,
    mut reply: impl FnMut(usize, &Value) -> Result<Value, String>,
) -> Result<Value, String> {
    for additional in 0..=COURSE_TRIAL_MAX_FOLLOWUPS {
        let practice = &report["practice"];
        if practice["assisted"] != false || practice["needs_retry"] == true {
            return Err("The current answer is assisted or requires a fresh attempt; it cannot prove independent completion".into());
        }
        if practice["status"] == "complete" {
            if practice["result_ref"].as_str().is_none_or(str::is_empty) {
                return Err("No delivered completion reference".into());
            }
            return Ok(report);
        }
        if practice["status"] != "active"
            || report["decision"]["decision"]["action"] != "followup"
        {
            return Err("Expected independent completion or a delivered follow-up; inspect the saved turn".into());
        }
        if additional == COURSE_TRIAL_MAX_FOLLOWUPS {
            return Err("The three-follow-up trial limit was reached; retain the unanswered question for review".into());
        }
        report = reply(5 + additional, &report)?;
    }
    unreachable!()
}

pub fn course_trial_followup_answer(
    output: &std::path::Path,
    index: usize,
    report: &Value,
) -> Result<String, String> {
    let answer_path = output.join(format!("learner-answer-{index}.txt"));
    let request_path = output.join(format!("follow-up-{index}.json"));
    let request = json!({
        "teacher_answer": report["answer"],
        "previous_learner_message": report["message"],
        "answer_path": answer_path,
        "instructions": "Read the actual displayed question, then write the learner's answer as UTF-8 text. Answer only the task; do not instruct the teacher to complete, alter its criteria, or fabricate a completion result.",
        "wait_seconds": 180
    });
    std::fs::write(&request_path, serde_json::to_vec_pretty(&request).unwrap())
        .map_err(|error| error.to_string())?;
    eprintln!("Waiting for learner answer: {} (question: {})", answer_path.display(), request_path.display());
    let began = std::time::Instant::now();
    while began.elapsed() < std::time::Duration::from_secs(180) {
        match std::fs::read_to_string(&answer_path) {
            Ok(answer) if !answer.trim().is_empty() => return Ok(answer.trim().to_owned()),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err(format!("No answer received for {}; the question and isolated state remain available", request_path.display()))
}

#[test]
fn closed_book_course_trial_accepts_a_real_followup_before_completion() {
    let (_root, mut state) = fixture_with_assets(false);
    post(&mut state, "/agent/new", "{}");
    control(&mut state, "start", json!({"target":"Explain whole-trip average speed"}));
    exercise(&mut state, "Start", json!({"action":"question","criteria":"Include stopped time and explain its effect",
        "text":"How does a stop affect whole-trip average speed?","provides_help":false,"reason":"Observe the time range"}));
    let practice = exercise(&mut state, "Use the same whole-trip distance.", json!({"action":"followup",
        "text":"The distance is unchanged. What time belongs in the denominator, and what happens to the quotient?",
        "provides_help":false,"reason":"The time relationship is still missing"}));
    let report = json!({"practice":practice,"decision":{"decision":{"action":"followup"}}});
    let completed = finish_course_trial_dialogue(report, |index, _| {
        assert_eq!(index, 5);
        let practice = exercise(&mut state, "The total elapsed time includes the stop; with the same distance, more time means a lower average speed.",
            json!({"action":"complete","text":"You explained both the time range and its effect.","provides_help":false,
                "learner_quote":"with the same distance, more time means a lower average speed","reason":"Independent explanation of the frozen relationship"}));
        Ok(json!({"practice":practice}))
    }).unwrap();
    assert_eq!(completed["practice"]["status"], "complete");
    assert!(completed["practice"]["result_ref"].as_str().unwrap().starts_with("closed-book:delivery:"));
}

#[test]
fn closed_book_course_trial_stops_on_assistance_and_limits_followups() {
    let followup = json!({"practice":{"status":"active","assisted":false,"needs_retry":false},
        "decision":{"decision":{"action":"followup"}}});
    let mut calls = 0;
    let failure = finish_course_trial_dialogue(followup.clone(), |_, _| {
        calls += 1;
        Ok(followup.clone())
    }).unwrap_err();
    assert_eq!(calls, COURSE_TRIAL_MAX_FOLLOWUPS);
    assert!(failure.contains("trial limit"));
    let failure = finish_course_trial_dialogue(json!({"practice":{"status":"complete","assisted":true,"result_ref":"delivered"}}),
        |_, _| panic!("an assisted answer must not be retried until it passes")).unwrap_err();
    assert!(failure.contains("assisted"));
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
