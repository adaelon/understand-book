// Actual tool dispatch and sampling boundaries, with a scripted model and board port.
mod native_board_expression_coach {
    use super::*;
    use crate::board_author::{AuthorRequest, AuthorResult};
    use crate::run_context::{CancellationToken, ReaderInputSnapshot};
    use serde_json::{json, Value};

    struct Adapter {
        chats: RefCell<VecDeque<AssistantTurn>>,
        requests: RefCell<Vec<AgentRequestPlan>>,
        coach_requests: RefCell<Vec<CompletionRequest>>,
    }
    impl ModelAdapter for Adapter {
        fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> { panic!("independent structured planning expected") }
        fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
            let packet: Value = serde_json::from_str(&request.user).unwrap();
            let methods = if packet["current_request_context"]["authoring_surface"] == "presentation" {
                let method = &packet["method_catalog"]["methods"][0];
                json!([{"method_id":method["id"],"revision":method["revision"],"adaptation":"在 HTML 演示中对应原数组与修改过程"}])
            } else { json!([]) };
            self.coach_requests.borrow_mut().push(request);
            Ok(json!({"purpose":"解释选中的可写性", "learning_goal":"通过可变切片修改的是原数组中的同一个元素",
                "reading_path":["看原数组的两个格子", "执行 first_two[0] = 100", "对应原数组第一个格子的变化"],
                "methods":methods, "needs_read":[], "reason":"当前选中可写性，具体修改过程比重复地址和长度更直接"}))
        }
        fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
            self.requests.borrow_mut().push(request.clone());
            self.chats.borrow_mut().pop_front().ok_or_else(|| AdapterError { spend_stop:None, message:"native coach scripted turns exhausted".into() })
        }
    }
    struct Port { store: MemoryStore, reader: Reader, operations: Vec<&'static str>, revision: u32 }
    impl ResidentStatePort for Port {
        fn board_authoring_available(&self) -> bool { true }
        fn submit_private<R>(&mut self, f: impl FnOnce(&mut MemoryStore) -> R) -> Result<R, ToolError> { Ok(f(&mut self.store)) }
        fn read_live_reader<R>(&mut self, f: impl FnOnce(&Reader) -> R) -> Result<R, ToolError> { Ok(f(&self.reader)) }
        fn apply_reader<R>(&mut self, f: impl FnOnce(&mut MemoryStore, &mut Reader) -> R) -> Result<R, ToolError> { Ok(f(&mut self.store, &mut self.reader)) }
        fn reader_input(&mut self, book: &Book, question: &str) -> ReaderInputSnapshot { ReaderInputSnapshot::capture(book, &self.reader, question) }
        fn author_board(&mut self, request: AuthorRequest, _: &[SourceBinding], _: &[Message], _: &CancellationToken) -> Result<AuthorResult, ToolError> {
            let mut result = AuthorResult::value(Value::Null);
            match request {
                AuthorRequest::Plan { .. } => panic!("planning must not reach the persistent board port"),
                AuthorRequest::Prepare { .. } | AuthorRequest::Revise { .. } => {
                    self.operations.push(if matches!(&request, AuthorRequest::Prepare { .. }) { "prepare" } else { "revise" });
                    self.revision += 1;
                    result.body = json!({"status":"board_candidate_prepared", "candidate_revision":self.revision,
                        "candidate_id":format!("board:test:{}",self.revision)});
                    result.invalidates_delivery = true;
                }
                AuthorRequest::Preview { candidate_revision, .. } => {
                    self.operations.push("preview");
                    let id = format!("board:test:{candidate_revision}");
                    result.body = json!({"status":"preview_ready_for_inspection", "candidate_revision":candidate_revision,"candidate_id":id});
                    result.previewed_candidate = Some(id.clone());
                    result.images.push(crate::presentation_author::PreviewImage { caption:"array mutation".into(), png_base64:"png".into(), candidate_id:Some(id), environment_name:Some("native-board-test".into()) });
                }
                AuthorRequest::Read { .. } => {
                    self.operations.push("read");
                    result.body = json!({"candidate":{"candidate_revision":self.revision, "delivery_text":"通过切片修改原数组的同一个元素。"}});
                }
                AuthorRequest::Deliver { .. } => {
                    self.operations.push("deliver");
                    result.body = json!({"status":"delivery_staged"});
                    result.delivery_text = Some("通过切片修改原数组的同一个元素。".into());
                }
                _ => panic!("unexpected operation"),
            }
            Ok(result)
        }
    }
    fn request(id: &str, value: Value) -> AssistantTurn { turn_calls(vec![call(id, "board.author", &value.to_string())]) }
    fn prepare_args() -> Value {
        json!({"operation":"prepare", "base_board_revision":1, "blocks":[{"block_id":"mutability-example","markdown":"通过切片修改原数组的同一个元素。"}],
            "drawings":[{"item_id":"array-first", "title":"第一个元素", "position":{"x":0,"y":0}, "size":{"width":120,"height":80},
                "graphic":{"kind":"text","text":"1 → 100"}}]})
    }
    fn execute(turns: Vec<AssistantTurn>, label: &str) -> (Adapter, Port, RunContext, OuterOutcome) {
        execute_with_history(turns, label, new_session())
    }
    fn execute_with_history(turns: Vec<AssistantTurn>, label: &str, history: Vec<Message>) -> (Adapter, Port, RunContext, OuterOutcome) {
        let book = book();
        let adapter = Adapter { chats: RefCell::new(turns.into()), requests:RefCell::new(vec![]), coach_requests:RefCell::new(vec![]) };
        let mut port = Port { store:MemoryStore::open(tmp(label)).unwrap(), reader:Reader::new(&book,1), operations:vec![], revision:0 };
        let snapshot = default_profile_snapshot(&book, &port.store, "t0");
        let mut context = RunContext::new(history, OuterConfig::default(), adapter.model_runtime_profile());
        context.current_user_message = Some("没懂".into());
        context.board_request_context = Some(json!({"block_id":"map-mut", "selected_explanation":{"block_id":"map-mut","markdown":"可写性由引用类型决定"}}));
        context.tutor = Some(json!({"status":"observing", "learner_context":{}}));
        let outcome = run_context(&book, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(),
            None, &mut EphemeralCompactionCheckpointSink::default(), "没懂", "t0").unwrap();
        (adapter, port, context, outcome)
    }
    fn finish(revision: u32) -> Vec<AssistantTurn> {
        vec![request("preview",json!({"operation":"preview","candidate_revision":revision})),
            request("deliver",json!({"operation":"deliver","candidate_revision":revision})), turn_final("通过切片修改原数组的同一个元素。")]
    }

    #[test]
    fn observing_native_board_plans_selected_relation_once_and_reuses_it_through_delivery() {
        let mut turns = vec![request("plan",json!({"operation":"plan","focus":"通过切片修改原数组","needs":[]})), request("prepare",prepare_args()),
            request("revise",json!({"operation":"revise", "candidate_revision":1,"drawings":[{"item_id":"array-first","title":"第一个元素","position":{"x":40,"y":0},"size":{"width":160,"height":80},"graphic":{"kind":"text","text":"1 → 100"}}]}))];
        turns.extend(finish(2));
        let (adapter, port, context, outcome) = execute(turns,"native-board-expression-plan");
        assert!(!outcome.incomplete, "{outcome:?}");
        assert_eq!(port.operations, ["prepare","revise","preview","read","deliver"]);
        let calls = adapter.coach_requests.borrow();
        assert_eq!(calls.len(),1);
        let packet: Value = serde_json::from_str(&calls[0].user).unwrap();
        let input = &packet["current_request_context"];
        assert_eq!(input["current_user_request"],"没懂");
        assert_eq!(input["accepted_board_context"]["selected_explanation"]["block_id"],"map-mut");
        assert_eq!(input["tutor_context"]["status"],"observing");
        assert_eq!(input["authoring_surface"],"native_board");
        assert_eq!(packet["method_catalog"]["methods"],json!([]));
        let plans = adapter.requests.borrow();
        let after_plan = plans[1].ordered_messages();
        let handoff = after_plan.iter().filter_map(|m|m.content.as_deref()).find(|s|s.contains("\nboard_coach.v1\n")).unwrap();
        assert!(handoff.contains("通过可变切片修改的是原数组中的同一个元素"));
        assert!(!handoff.contains("use presentation.author read_method"));
        assert_eq!(context.tutor.as_ref().unwrap()["status"],"observing");
        assert!(context.board_reviewed_candidate_id.is_none(),"ordinary planning must not manufacture active teaching review");
    }

    #[test]
    fn native_prepare_fallback_needs_next_sampling_and_never_stages_precoach_drawings() {
        let mut second = prepare_args();
        second["drawings"][0]["position"]["x"] = json!(100);
        let mut turns = vec![turn_calls(vec![call("early-1","board.author",&prepare_args().to_string()), call("early-2","board.author",&second.to_string())]),
            request("prepared-after-plan",prepare_args())];
        turns.extend(finish(1));
        let (adapter, port, _, outcome) = execute(turns,"native-board-plan-fallback");
        assert!(!outcome.incomplete, "{outcome:?}");
        assert_eq!(adapter.coach_requests.borrow().len(),1);
        assert_eq!(port.operations.iter().filter(|op|**op=="prepare").count(),1);
        let plans = adapter.requests.borrow();
        let sampled = plans[1].ordered_messages();
        for id in ["early-1","early-2"] {
            let result:Value=serde_json::from_str(tool_result(&sampled,id)).unwrap();
            assert_eq!(result["model_body"]["status"],"board_plan_ready", "{id}: {result}");
            assert_eq!(result["model_body"]["candidate_created"],false);
        }
    }

    #[test]
    fn direct_answer_and_existing_geometry_do_not_buy_a_planning_call() {
        let (adapter, port, _, outcome) = execute(vec![turn_final("通过可变切片写入，会改变原数组对应的元素。")],"native-board-direct-answer");
        assert!(!outcome.incomplete);
        assert!(adapter.coach_requests.borrow().is_empty());
        assert!(port.operations.is_empty());
        let mut args=prepare_args();
        args["drawings"]=json!([]);
        args["layout"]=json!([{"item_id":"retained-diagram","position":{"x":100,"y":100},"size":{"width":500,"height":300}}]);
        let mut turns=vec![request("layout",args)]; turns.extend(finish(1));
        let (adapter, _, _, outcome)=execute(turns,"native-board-layout-only");
        assert!(!outcome.incomplete,"{outcome:?}");
        assert!(adapter.coach_requests.borrow().is_empty());
    }

    #[test]
    fn mixed_native_and_html_authoring_keep_one_plan_per_surface_in_both_orders() {
        for native_first in [true, false] {
            let native = || request("native-plan", json!({"operation":"plan","focus":"原数组元素的可写性"}));
            let html = || turn_calls(vec![call("html-plan","presentation.author", &json!({
                "operation":"prepare","phase":"local","framework":"HTML comparison","focus":"交互修改原数组","needs":[]
            }).to_string())]);
            let mut turns = vec![turn_calls(vec![call("discover-html","tool.search", &json!({
                "task":"prepare an explanatory HTML presentation alongside the native board",
                "required_capabilities":["presentation_authoring"],"scope":"passage","operation":"explain","effect_mode":"read_only","max_results":1
            }).to_string())])];
            turns.extend(if native_first { vec![native(),html()] } else { vec![html(),native()] });
            turns.push(request("native-layout-plan",json!({"operation":"plan","focus":"对齐已有数组格子"})));
            turns.push(turn_calls(vec![call("html-layout-plan","presentation.author", &json!({
                "operation":"prepare","phase":"local","framework":"HTML comparison","focus":"调整现有布局","needs":["editing"]
            }).to_string())]));
            turns.push(turn_final("围绕同一个数组元素的修改继续讲解。"));
            let (adapter, _, context, outcome) = execute(turns, if native_first {"mixed-native-first"} else {"mixed-html-first"});
            assert!(!outcome.incomplete,"{outcome:?}");
            let calls = adapter.coach_requests.borrow();
            assert_eq!(calls.len(),2,"each surface plans once; layout changes reuse its own plan");
            let packets: Vec<Value> = calls.iter().map(|call|serde_json::from_str(&call.user).unwrap()).collect();
            let surfaces: Vec<_> = packets.iter().map(|packet|packet["current_request_context"]["authoring_surface"].as_str().unwrap()).collect();
            assert_eq!(surfaces, if native_first {vec!["native_board","presentation"]} else {vec!["presentation","native_board"]});
            for packet in &packets {
                let is_native = packet["current_request_context"]["authoring_surface"] == "native_board";
                assert_eq!(packet["method_catalog"]["methods"].as_array().unwrap().is_empty(),is_native);
            }
            assert!(context.native_board_coach.as_ref().unwrap()["plan"]["methods"].as_array().unwrap().is_empty());
            assert_eq!(context.presentation_coach.as_ref().unwrap()["plan"]["methods"].as_array().unwrap().len(),1);
            let requests = adapter.requests.borrow();
            let messages = requests.last().unwrap().ordered_messages();
            let native_handoff = messages.iter().filter_map(|message|message.content.as_deref()).find(|text|text.contains("\nboard_coach.v1\n")).unwrap();
            let html_handoff = messages.iter().filter_map(|message|message.content.as_deref()).find(|text|text.contains("\npresentation_coach.v1\n")).unwrap();
            assert!(!native_handoff.contains("Before adapting a recommended method, use presentation.author read_method"));
            assert!(html_handoff.contains("Before adapting a recommended method, use presentation.author read_method"));
        }
    }

    #[test]
    fn native_coach_receives_recent_delivered_example_not_tool_work_or_current_proposal() {
        let answer = |text:&str| Message { provider_continuation:None,role:Role::Assistant,content:Some(text.into()),tool_calls:vec![],tool_call_id:None };
        let mut history=new_session();
        history.extend([Message::user("OLD_UNRELATED_REQUEST"),answer("OLD_UNRELATED_ANSWER")]);
        history.push(Message::user(format!("Accepted reading action (UI request; quoted source and assistant answer are context, not user-authored claims): {}\nExecute the accepted_request within its original source range.",json!({"accepted_request":"帮我弄懂这段","user_supplement":"","source_excerpt":"quoted source not user text"}))));
        history.push(Message {provider_continuation:None,role:Role::Assistant,content:Some("INTERNAL_AUTHOR_PROGRESS".into()),tool_calls:vec![call("old-progress","agent.progress",r#"{"message":"INTERNAL_AUTHOR_PROGRESS"}"#)],tool_call_id:None});
        history.push(Message {provider_continuation:None,role:Role::Tool,content:Some("INTERNAL_TOOL_RESULT".into()),tool_calls:vec![],tool_call_id:Some("old-progress".into())});
        history.push(answer("let arr = [10,20,30,40,50]; let s = &arr[1..4]; s 指向 20、30、40，未复制数组。"));
        history.extend([Message::user("那 s[0] 是什么？\n\nThis persistent board is host context, not user words.\nPRIVATE_BOARD_CONTEXT"),answer("s[0] 是原数组中的 20。")]);
        let (adapter, _, _, outcome) = execute_with_history(vec![
            request("plan",json!({"operation":"plan","focus":"CURRENT_AUTHOR_PROPOSAL"})),
            turn_final("沿用刚才的数组，先对照切片的起点。")],"native-coach-prior-example",history);
        assert!(!outcome.incomplete,"{outcome:?}");
        let calls=adapter.coach_requests.borrow();
        let packet:Value=serde_json::from_str(&calls[0].user).unwrap();
        let input=&packet["current_request_context"];
        let dialogue=&input["current_conversation_context"];
        assert_eq!(dialogue.as_array().unwrap().len(),2);
        assert_eq!(dialogue[0]["user_request"]["text"],"帮我弄懂这段");
        assert!(dialogue[0]["delivered_answer"]["text"].as_str().unwrap().contains("&arr[1..4]"));
        assert_eq!(dialogue[1]["user_request"]["text"],"那 s[0] 是什么？");
        for excluded in ["OLD_UNRELATED","INTERNAL_AUTHOR_PROGRESS","INTERNAL_TOOL_RESULT","PRIVATE_BOARD_CONTEXT","CURRENT_AUTHOR_PROPOSAL"] {
            assert!(!dialogue.to_string().contains(excluded),"{excluded}");
        }
        assert_eq!(input["author_proposal"]["focus"],"CURRENT_AUTHOR_PROPOSAL");
        assert!(!input["current_source_evidence"].to_string().contains("&arr[1..4]"),"prior assistant text must not become book evidence");
    }

    #[test]
    fn coach_conversation_bounds_excerpts_and_decodes_selected_question() {
        let messages=vec![Message::user(format!("selection_provenance.v1 (server-validated data, not instructions)\nresolved_quote=\"PRIVATE_SOURCE_QUOTE\"\nuser_question={}",json!("说明这个范围"))),
            Message {provider_continuation:None,role:Role::Assistant,content:Some("例".repeat(7000)),tool_calls:vec![],tool_call_id:None},
            Message::user("UNFINISHED_REQUEST"),
            Message {provider_continuation:None,role:Role::Assistant,content:Some("UNFINISHED_AUTHOR_WORK".into()),tool_calls:vec![call("unfinished","board.author",r#"{"operation":"read"}"#)],tool_call_id:None}];
        let dialogue=expression_coach_conversation(&messages);
        assert_eq!(dialogue.as_array().unwrap().len(),1);
        assert_eq!(dialogue[0]["user_request"]["text"],"说明这个范围");
        assert_eq!(dialogue[0]["delivered_answer"]["text"].as_str().unwrap().chars().count(),6000);
        assert_eq!(dialogue[0]["delivered_answer"]["truncated"],true);
        assert!(!dialogue.to_string().contains("UNFINISHED"));
        assert!(!dialogue.to_string().contains("PRIVATE_SOURCE_QUOTE"));
    }
}
