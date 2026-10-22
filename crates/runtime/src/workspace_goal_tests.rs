use super::*;
use crate::workspace_goal::{WorkspaceGoalContext, WorkspaceGoalUpdate};
use base_schema::WorkspaceGoal;
use serde_json::json;

struct WorkspaceState<'a> {
    state: BorrowedResidentState<'a>,
    current_goal: WorkspaceGoal,
    updates: Vec<WorkspaceGoal>,
    plans: Vec<crate::goal::ResidentGoal>,
    fail_goal_record: bool,
}
impl ResidentStatePort for WorkspaceState<'_> {
    fn update_workspace_goal(&mut self, update: &WorkspaceGoalUpdate, user: &str) -> Result<WorkspaceGoal, ToolError> {
        update.validate(&self.current_goal, user).map_err(|message| ToolError {
            error_code:"WORKSPACE_GOAL_UPDATE_INVALID".into(),category:"validation".into(),message,
        })?;
        let goal = WorkspaceGoal {revision:self.current_goal.revision + 1,text:update.goal.text.clone(),
            constraints:update.goal.constraints.clone(),created_at:1};
        self.current_goal = goal.clone();
        self.updates.push(goal.clone());
        if self.fail_goal_record {
            return Err(ToolError {error_code:"WORKSPACE_GOAL_COMMIT_INCOMPLETE".into(),category:"unavailable".into(),
                message:"The accepted goal was saved but its turn record could not be saved".into()});
        }
        Ok(goal)
    }
    fn persist_goal(&mut self, goal: &crate::goal::ResidentGoal) -> Result<(), ToolError> {
        self.plans.push(goal.clone()); Ok(())
    }
    fn submit_private<R>(&mut self, operation: impl FnOnce(&mut MemoryStore) -> R) -> Result<R,ToolError> { self.state.submit_private(operation) }
    fn read_live_reader<R>(&mut self, operation: impl FnOnce(&Reader) -> R) -> Result<R,ToolError> { self.state.read_live_reader(operation) }
    fn apply_reader<R>(&mut self, operation: impl FnOnce(&mut MemoryStore,&mut Reader) -> R) -> Result<R,ToolError> { self.state.apply_reader(operation) }
    fn reader_input(&mut self, book:&Book, question:&str)->crate::run_context::ReaderInputSnapshot { self.state.reader_input(book,question) }
}

fn replay(name:&str, goal:WorkspaceGoal, user:&str, turns:Vec<AssistantTurn>)
    -> (OuterOutcome, RunContext, Vec<AgentRequestPlan>, Vec<WorkspaceGoal>, Vec<crate::goal::ResidentGoal>) {
    replay_with_tutor(name, goal, user, turns, json!({"status":"observing","learner_context":{}}))
}

fn replay_with_tutor(name:&str, goal:WorkspaceGoal, user:&str, turns:Vec<AssistantTurn>, tutor:serde_json::Value)
    -> (OuterOutcome, RunContext, Vec<AgentRequestPlan>, Vec<WorkspaceGoal>, Vec<crate::goal::ResidentGoal>) {
    let book = book();
    let mut store = MemoryStore::open(tmp(name)).unwrap();
    let mut reader = Reader::new(&book, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&book,&store,"t0");
    let adapter = RequestPlanRecordingAdapter::new(turns,vec![]);
    let mut context = RunContext::new(new_session(),OuterConfig {max_turns:Some(6),..Default::default()},adapter.model_runtime_profile());
    context.current_user_message = Some(user.into());
    context.goal = Some(crate::goal::ResidentGoal::new("delivery:1".into(),"turn:1".into(),user.into()));
    context.workspace_goal = Some(WorkspaceGoalContext {exam_calendar:None,learning_workspace_id:"workspace:A".into(),goal:goal.clone()});
    // Ordinary questions carry the parent direction without turning teaching on.
    context.tutor = Some(tutor);
    let mut state = WorkspaceState {state:BorrowedResidentState {store:&mut store,reader:&mut reader},
        current_goal:goal,updates:vec![],plans:vec![],fail_goal_record:false};
    let result = run_context(&book,&mut state,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),user,"t0").unwrap();
    let plans = adapter.seen_plans.into_inner();
    (result,context,plans,state.updates,state.plans)
}

fn direction(text:&str, constraints:&[&str])->WorkspaceGoal {
    WorkspaceGoal {revision:3,text:text.into(),constraints:constraints.iter().map(|text|(*text).into()).collect(),created_at:0}
}
fn plan(focus:&str,next:&str)->AssistantTurn {
    turn_calls(vec![call("plan","goal.update",&json!({"operation":"working","focus":focus,"next_move":next}).to_string())])
}
fn current_workspace(plan:&AgentRequestPlan)->String {
    plan.input.iter().rev().filter_map(|message|message.content.as_deref())
        .find(|text|text.contains("key=agent.workspace_goal\n")).expect("workspace direction reaches the actual provider request").into()
}
fn assert_wire_goal(plan:&AgentRequestPlan,goal:&str) {
    let (native,_) = crate::native_chat_request_projection("w4-controlled",plan);
    let react = crate::react_chat_request_projection("w4-controlled",plan);
    for wire in [native,react] {
        let text = wire["messages"].as_array().unwrap().iter().filter_map(|message|message["content"].as_str()).collect::<Vec<_>>().join("\n");
        assert!(text.contains(goal));
        assert!(text.contains("Workspace learning direction"));
    }
}

#[test]
fn w4_deep_learning_prerequisite_plan_returns_to_original_goal() {
    let goal = direction("学习深度学习，能够实现基本模型",&["每天一小时"]);
    let (out,context,requests,updates,plans) = replay("w4-chain-rule",goal.clone(),"反向传播的链式法则没弄懂，先帮我补这里。",vec![
        plan("补齐复合函数梯度的链式法则连接","回到反向传播，沿计算图应用各局部导数"),
        turn_final("先把复合函数中各局部变化怎样相乘讲清楚，再回到反向传播沿计算图传递梯度。"),
    ]);
    assert!(!out.incomplete);
    assert!(updates.is_empty());
    assert_eq!(context.workspace_goal.unwrap().goal,goal);
    assert_eq!(plans[0].working.next_move,"回到反向传播，沿计算图应用各局部导数");
    assert!(current_workspace(&requests[1]).contains("每天一小时"));
    assert!(requests[1].input.iter().filter_map(|m|m.content.as_deref()).any(|text|text.contains(&plans[0].working.next_move)));
    assert_wire_goal(&requests[1],&goal.text);
}

#[test]
fn w4_three_day_exam_constraint_reaches_plan_and_continuation() {
    let goal = direction("系统学习机器学习，包括线性模型、反向传播和无监督学习",
        &["考试只剩三天","考纲只包括线性模型和反向传播，不考无监督学习"]);
    let (out,context,requests,updates,plans) = replay("w4-three-days",goal.clone(),"按我的考纲和时间安排下一步。",vec![
        plan("优先处理考纲内尚未掌握的反向传播","用一道考纲内的计算题核对薄弱连接，再安排余下两天复习"),
        turn_final("三天内先处理反向传播的薄弱连接，再用一道考纲内计算题决定后续复习深度。"),
    ]);
    assert!(!out.incomplete);
    assert!(updates.is_empty());
    assert_eq!(context.workspace_goal.unwrap().goal,goal);
    assert!(plans[0].working.next_move.contains("考纲内"));
    let latest = current_workspace(&requests[1]);
    for constraint in &goal.constraints { assert!(latest.contains(constraint)); }
    assert!(latest.contains("\"active_teaching\":false"));
    assert_wire_goal(&requests[1],&goal.text);
    for request in &requests {
        let (native,_) = crate::native_chat_request_projection("w4-narrow-exam-scope",request);
        let react = crate::react_chat_request_projection("w4-narrow-exam-scope",request);
        for wire in [native,react] {
            let prompt = wire["messages"].as_array().unwrap().iter().filter_map(|message|message["content"].as_str()).collect::<Vec<_>>().join("\n");
            assert!(prompt.contains(&goal.text),"the broad goal must be retained, not rewritten to hide the scope distinction");
            assert!(prompt.contains(&goal.constraints[1]),"the current narrower syllabus must accompany the broad goal");
            let policy = request.input.iter().filter_map(|message|message.content.as_deref())
                .find(|text|text.contains("key=agent.workspace_goal_policy\n")).unwrap();
            for priority_rule in ["latest accepted exam scope","takes precedence","exam requirements","out-of-scope background",
                "confirmed grading criteria","marks or deductions"] {
                assert!(policy.contains(priority_rule) && prompt.contains(priority_rule),
                    "the scope priority guidance must reach the actual provider: {priority_rule}");
            }
        }
    }
}

#[test]
fn w4_temporary_question_cannot_replace_workspace_direction() {
    let goal = direction("学习深度学习，能够实现基本模型",&[]);
    let user = "临时插问一下咖啡为什么会苦，工作区目标不变。";
    let (out,context,requests,updates,plans) = replay("w4-side-question",goal.clone(),user,vec![
        plan("回答咖啡苦味这个临时问题","下次继续深度学习的原阶段"),
        turn_final("咖啡的苦味来自多种苦味物质及烘焙过程的变化。这个插问不改变原来的深度学习目标。"),
    ]);
    assert!(!out.incomplete);
    assert!(updates.is_empty());
    assert_eq!(context.workspace_goal.unwrap().goal,goal);
    assert!(out.trace.iter().all(|step| step.tool!="workspace.goal.update"));
    assert_eq!(plans[0].working.next_move,"下次继续深度学习的原阶段");
    assert_wire_goal(requests.last().unwrap(),&goal.text);
}

#[test]
fn w4_explicit_redirection_updates_effective_goal_but_keeps_run_provenance() {
    let goal = direction("学习深度学习，能够实现基本模型",&[]);
    let user = "深度学习先不学了，接下来只准备三天后的期末考试。";
    let (out,context,requests,updates,plans) = replay_with_tutor("w4-redirection",goal.clone(),user,vec![
        turn_calls(vec![call("redirect","workspace.goal.update",&json!({"basis_quote":user,"expected_revision":3,
            "goal":{"text":"只准备三天后的期末考试","constraints":["三天后"]}}).to_string()),
            call("old-stage","tutor.step",r#"{"operation":"outside"}"#)]),
        plan("按新目标准备期末冲刺","先确认考纲和实际薄弱点，再在三天内选择阶段"),
        turn_final("接下来只准备三天后的期末考试。旧阶段已停止，下一步按考纲和薄弱点安排。"),
    ],json!({"status":"active","workspace_goal_revision":3,"binding":{"tutor_session_id":"stage:old"},
        "user_intent":"逐章学习深度学习","learner_context":{},"current_design":null}));
    assert!(!out.incomplete);
    assert_eq!(updates.len(),1);
    assert_eq!(updates[0].revision,4);
    assert_eq!(context.workspace_goal.unwrap().goal,goal);
    assert_eq!(context.accepted_workspace_goal.as_ref(),Some(&updates[0]));
    assert_eq!(context.tutor.unwrap()["status"],"paused");
    assert!(out.trace.iter().any(|step|step.tool=="tutor.step" && step.result_digest.contains("TUTOR_SESSION_INACTIVE")));
    assert!(requests[1].tools.iter().all(|tool|tool.name!="tutor.step"));
    assert!(plans[0].working.next_move.contains("三天"));
    let latest = current_workspace(&requests[1]);
    assert!(latest.contains("\"run_goal_revision\":3"));
    assert!(latest.contains("\"revision\":4"));
    assert!(latest.contains("只准备三天后的期末考试"));
    assert!(!latest.contains("学习深度学习"));
    assert_wire_goal(&requests[1],"只准备三天后的期末考试");
}

#[test]
fn w4_committed_goal_with_failed_turn_record_stops_before_next_model_send() {
    let book = book();
    let mut store = MemoryStore::open(tmp("w4-partial-goal-commit")).unwrap();
    let mut reader = Reader::new(&book, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&book,&store,"t0");
    let goal = direction("学习深度学习",&[]);
    let user = "深度学习先不学了，接下来只准备三天后的期末考试。";
    let adapter = RequestPlanRecordingAdapter::new(vec![turn_calls(vec![call("redirect","workspace.goal.update",
        &json!({"basis_quote":user,"expected_revision":3,"goal":{"text":"只准备三天后的期末考试","constraints":[]}}).to_string())])],vec![]);
    let mut context = RunContext::new(new_session(),OuterConfig::default(),adapter.model_runtime_profile());
    context.current_user_message = Some(user.into());
    context.workspace_goal = Some(WorkspaceGoalContext {exam_calendar:None,learning_workspace_id:"workspace:A".into(),goal:goal.clone()});
    let mut state = WorkspaceState {state:BorrowedResidentState {store:&mut store,reader:&mut reader},
        current_goal:goal,updates:vec![],plans:vec![],fail_goal_record:true};
    let error = run_context(&book,&mut state,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),user,"t0").unwrap_err();
    assert_eq!(error.error_code,"WORKSPACE_GOAL_COMMIT_INCOMPLETE");
    assert_eq!(adapter.seen_plans.borrow().len(),1,"partial persistence must not trigger another plan or fallback model send");
    assert_eq!(state.current_goal.revision,4,"the accepted workspace goal already exists");
    assert_eq!(context.model_turns,1,"the completed model call remains accounted for");
}

#[test]
fn exam_calendar_is_visible_only_for_the_effective_goal_revision() {
    let goal = direction("准备期末考试", &["只复习前三章"]);
    let context = WorkspaceGoalContext {learning_workspace_id:"exam".into(),goal:goal.clone(),
        exam_calendar:Some(json!({"workspace_goal_revision":3,"current_local_date":"2026-10-21","remaining_days":2}))};
    let projection = crate::workspace_goal::projection(&context,None,None);
    assert!(projection.contains("2026-10-21"));
    assert!(projection.contains("remaining_days"));
    let changed = WorkspaceGoal { revision:4, ..goal };
    let projection = crate::workspace_goal::projection(&context,Some(&changed),None);
    assert!(!projection.contains("2026-10-21"));
}

#[test]
fn workspace_material_directory_and_cross_material_tools_reach_first_sampling() {
    struct LibraryState<'a> {
        state: BorrowedResidentState<'a>,
        available: bool,
        directory_reads: usize,
    }
    impl ResidentStatePort for LibraryState<'_> {
        fn library_available(&self) -> bool { self.available }
        fn library_read(&mut self, name: &str, arguments: serde_json::Value) -> Result<crate::library::LibraryReadResult, ToolError> {
            assert_eq!(name, "library.list");
            assert_eq!(arguments, json!({"limit":6}));
            self.directory_reads += 1;
            Ok(crate::library::LibraryReadResult { source:None, body:json!({
                "items":[
                    {"name":"课程讲义","published_book_ref":{"book_id":"text","publication_id":"text-v1"},"available_operations":["structure","text"]},
                    {"name":"配套视频","published_book_ref":{"book_id":"video","publication_id":"video-v2"},"available_operations":["search_text","text"],
                     "video":{"duration_ms":60000,"video":{"media_id":"original-video","media_revision":"1"}}}
                ],
                "complete":false,
                "next_calls":[{"name":"library.list","arguments":{"after":{"book_id":"video","publication_id":"video-v2"},"limit":6}}]
            }) })
        }
        fn submit_private<R>(&mut self, operation: impl FnOnce(&mut MemoryStore) -> R) -> Result<R,ToolError> { self.state.submit_private(operation) }
        fn read_live_reader<R>(&mut self, operation: impl FnOnce(&Reader) -> R) -> Result<R,ToolError> { self.state.read_live_reader(operation) }
        fn apply_reader<R>(&mut self, operation: impl FnOnce(&mut MemoryStore,&mut Reader) -> R) -> Result<R,ToolError> { self.state.apply_reader(operation) }
        fn reader_input(&mut self, book:&Book, question:&str)->crate::run_context::ReaderInputSnapshot { self.state.reader_input(book,question) }
    }
    for (workspace, available) in [(true,true),(true,false),(false,true)] {
        let book = book();
        let mut store = MemoryStore::open(tmp(&format!("workspace-material-directory-{workspace}-{available}"))).unwrap();
        let mut reader = Reader::new(&book, DEFAULT_RADIUS);
        let snapshot = default_profile_snapshot(&book,&store,"t0");
        let adapter = RequestPlanRecordingAdapter::new(vec![turn_final("请从你正在做的题开始。")],vec![]);
        let mut context = RunContext::new(new_session(),OuterConfig::default(),adapter.model_runtime_profile());
        if workspace {
            context.workspace_goal = Some(WorkspaceGoalContext { exam_calendar:None,learning_workspace_id:"workspace:course".into(),
                goal:direction("期末复习",&[]) });
        }
        let mut state = LibraryState {state:BorrowedResidentState {store:&mut store,reader:&mut reader},available,directory_reads:0};
        run_context(&book,&mut state,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"从这道题开始","t0").unwrap();
        let supplied = workspace && available;
        assert_eq!(state.directory_reads,usize::from(supplied));
        let plans = adapter.seen_plans.borrow();
        let first = &plans[0];
        for name in ["library.list","library.read","library.learning","video.search"] {
            assert_eq!(first.tools.iter().any(|tool|tool.name==name),supplied,"{name}");
        }
        assert!(!first.tools.iter().any(|tool|tool.name=="video.read_frames"));
        let (native,_) = crate::native_chat_request_projection("workspace-materials",first);
        let react = crate::react_chat_request_projection("workspace-materials",first);
        for wire in [native,react] {
            let prompt = wire["messages"].as_array().unwrap().iter().filter_map(|message|message["content"].as_str()).collect::<Vec<_>>().join("\n");
            assert_eq!(prompt.contains("workspace_materials.v1"),supplied);
            if supplied {
                assert!(prompt.contains("配套视频"));
                assert!(prompt.contains("video-v2"));
                assert!(prompt.contains("next_calls"));
                assert!(prompt.contains("metadata, not observed source evidence"));
                assert!(prompt.contains("book.* reads only the currently open material"));
            }
        }
        assert!(context.evidence_ledger.references.is_empty(),"directory metadata cannot count as a reference read");
        assert!(context.evidence_ledger.bindings().is_empty());
        assert!(!context.messages.iter().filter_map(|message|message.content.as_deref()).any(|content|content.contains("workspace_materials.v1")),
            "the fresh directory must not become stale saved conversation history");
    }
}
