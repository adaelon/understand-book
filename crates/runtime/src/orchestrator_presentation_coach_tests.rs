// Included inside orchestrator::tests. These tests exercise the actual prepare
// boundary; the coach is not supplied as a fixture on RunContext.
mod presentation_coach_prepare {
    use super::*;
    use crate::presentation::{PresentationMethodObservations, PresentationMethodRead};
    use crate::presentation_author::{AuthorRequest, AuthorResult};
    use crate::run_context::{CancellationToken, ReaderInputSnapshot};
    use serde_json::{json, Value};

    const USER_REQUEST: &str =
        "请做一个可以完整阅读的演示，解释一次更新如何改变同一对象，保留推导。";
    const AUTHOR_PROPOSAL: &str = "AUTHOR_PROPOSAL_NOT_USER: 我准备只给一张摘要卡";
    const GOAL: &str = "COACH_GOAL: 分清当前状态、更新量与更新后的状态";
    const ADAPTATION: &str =
        "COACH_ADAPTATION: 保持对象身份，读取条件后把一次更新与完整推导连接起来";

    fn selected_method() -> Value {
        crate::presentation_methods::catalog()["methods"][0].clone()
    }

    fn selected_plan() -> Value {
        let method = selected_method();
        json!({
            "purpose": "交互理解与完整推导",
            "learning_goal": GOAL,
            "reading_path": ["先辨认当前状态和更新量", "操作一次并对应变化", "保留完整推导供连续阅读"],
            "methods": [{"method_id": method["id"], "revision": method["revision"], "adaptation": ADAPTATION}],
            "needs_read": [{"scope": "当前材料中的更新规则", "purpose": "核实变化条件，作者设想不构成来源证据"}],
            "reason": "当前用户要完整理解更新关系，摘要卡的作者设想不足以完成这个目标"
        })
    }

    enum CoachResult {
        Ready,
        WithPlan(Value),
        ProviderFailure,
        SpendStop,
        Cancel(CancellationToken),
    }

    struct Adapter {
        chats: RefCell<VecDeque<AssistantTurn>>,
        plans: RefCell<Vec<AgentRequestPlan>>,
        coach_requests: RefCell<Vec<CompletionRequest>>,
        coach_result: CoachResult,
        output_limits_at: Vec<usize>,
        cancel_at_limit: Option<CancellationToken>,
    }

    impl Adapter {
        fn new(chats: Vec<AssistantTurn>, coach_result: CoachResult) -> Self {
            Self {
                chats: RefCell::new(chats.into()),
                plans: RefCell::new(Vec::new()),
                coach_requests: RefCell::new(Vec::new()),
                coach_result,
                output_limits_at: Vec::new(),
                cancel_at_limit: None,
            }
        }
    }

    impl ModelAdapter for Adapter {
        fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
            panic!("the presentation coach must make an independent structured call")
        }

        fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
            self.coach_requests.borrow_mut().push(request);
            match &self.coach_result {
                CoachResult::Ready => Ok(selected_plan()),
                CoachResult::WithPlan(plan) => Ok(plan.clone()),
                CoachResult::ProviderFailure => Err(AdapterError {
                    spend_stop: None,
                    message: "scripted provider unavailable".into(),
                }),
                CoachResult::SpendStop => Err(AdapterError {
                    spend_stop: Some(crate::model_spend::SpendStop::InsufficientAllowance),
                    message: "scripted spend stop".into(),
                }),
                CoachResult::Cancel(token) => {
                    token.cancel();
                    Err(AdapterError {
                        spend_stop: None,
                        message: "scripted cancellation".into(),
                    })
                }
            }
        }

        fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
            self.plans.borrow_mut().push(request.clone());
            self.chats
                .borrow_mut()
                .pop_front()
                .ok_or_else(|| AdapterError {
                    spend_stop: None,
                    message: "presentation coach test chat exhausted".into(),
                })
        }

        fn chat_observed(&self, request: &AgentRequestPlan, observer: &mut dyn crate::provider_stream::ModelObserver) -> Result<AssistantTurn, AdapterError> {
            let ordinal = self.plans.borrow().len() + 1;
            if !self.output_limits_at.contains(&ordinal) { return self.chat(request); }
            self.plans.borrow_mut().push(request.clone());
            observer.observe(crate::provider_stream::ModelDelta::ToolArguments {
                index:0, id:"discarded-write".into(), name:"presentation.author".into(),
                arguments:r#"{"operation":"write","title":"never execute this partial response"}"#.into(),
            });
            observer.observe(crate::provider_stream::ModelDelta::OutputLimitReached { total_tokens:Some(11) });
            if let Some(token) = &self.cancel_at_limit { token.cancel(); }
            Err(AdapterError { spend_stop:None, message:"limited fixture".into() })
        }
    }

    struct Port {
        store: MemoryStore,
        reader: Reader,
        observations: PresentationMethodObservations,
        reads_before_sampling: Vec<Vec<PresentationMethodRead>>,
    }

    impl Port {
        fn new(book: &Book, label: &str) -> Self {
            Self {
                store: MemoryStore::open(tmp(label)).unwrap(),
                reader: Reader::new(book, 1),
                observations: PresentationMethodObservations::default(),
                reads_before_sampling: Vec::new(),
            }
        }
    }

    impl ResidentStatePort for Port {
        fn submit_private<R>(
            &mut self,
            f: impl FnOnce(&mut MemoryStore) -> R,
        ) -> Result<R, ToolError> {
            Ok(f(&mut self.store))
        }
        fn read_live_reader<R>(&mut self, f: impl FnOnce(&Reader) -> R) -> Result<R, ToolError> {
            Ok(f(&self.reader))
        }
        fn apply_reader<R>(
            &mut self,
            f: impl FnOnce(&mut MemoryStore, &mut Reader) -> R,
        ) -> Result<R, ToolError> {
            Ok(f(&mut self.store, &mut self.reader))
        }
        fn reader_input(&mut self, book: &Book, question: &str) -> ReaderInputSnapshot {
            ReaderInputSnapshot::capture(book, &self.reader, question)
        }
        fn record_presentation_methods(
            &mut self,
            observations: &PresentationMethodObservations,
        ) -> Result<(), ToolError> {
            self.observations.merge(observations);
            Ok(())
        }
        fn author_presentation(
            &mut self,
            request: AuthorRequest,
            _: &[SourceBinding],
            _: &[Message],
            _: &CancellationToken,
        ) -> Result<AuthorResult, ToolError> {
            let AuthorRequest::ReadMethod {
                method_id,
                revision,
                file,
                offset,
                length,
            } = request
            else {
                panic!("prepare is run-local; this regression only sends a method read to the host")
            };
            // Merely issuing read_method is not evidence that its body has
            // reached the next author sampling.
            self.reads_before_sampling
                .push(self.observations.reads.clone());
            let file = file.as_deref().unwrap_or("card.md");
            let (method, source) = crate::presentation_methods::source(&method_id, revision, file)?;
            let total = source.chars().count();
            let text: String = source
                .chars()
                .skip(offset)
                .take(length.unwrap_or(total))
                .collect();
            let end = offset + text.chars().count();
            let mut body = crate::presentation_methods::read_metadata(method, file);
            body["offset"] = json!(offset);
            body["text"] = json!(text);
            body["end_offset"] = json!(end);
            body["total_characters"] = json!(total);
            Ok(AuthorResult {
                body,
                images: Vec::new(),
                previewed_candidate: None,
                delivered: None,
            })
        }
    }

    fn discover() -> AssistantTurn {
        turn_calls(vec![call(
            "coach-discover",
            "tool.search",
            &json!({
                "task":"prepare a complete explanatory presentation",
                "required_capabilities":["presentation_authoring"], "scope":"passage",
                "operation":"explain", "effect_mode":"read_only", "max_results":1
            })
            .to_string(),
        )])
    }

    fn prepare(id: &str) -> AssistantTurn {
        turn_calls(vec![call(
            id,
            "presentation.author",
            &json!({
                "operation":"prepare", "phase":"local", "framework":AUTHOR_PROPOSAL,
                "focus":"用一张摘要卡结束", "needs":[]
            })
            .to_string(),
        )])
    }

    fn context(adapter: &Adapter) -> RunContext {
        let mut context = RunContext::new(
            new_session(),
            OuterConfig {
                max_turns: Some(8),
                token_budget: 1_000_000,
            },
            adapter.model_runtime_profile(),
        );
        context.tutor = Some(json!({"status":"observing", "learner_context":{}}));
        context
    }

    fn execute(
        port: &mut Port,
        book: &Book,
        adapter: &Adapter,
        context: &mut RunContext,
    ) -> Result<OuterOutcome, ToolError> {
        let snapshot = default_profile_snapshot(book, &port.store, "t0");
        run_context(
            book,
            port,
            adapter,
            context,
            &snapshot,
            &ResidentTurnResources::default(),
            None,
            &mut EphemeralCompactionCheckpointSink::default(),
            USER_REQUEST,
            "t0",
        )
    }

    fn sampled_body(plan: &AgentRequestPlan, call_id: &str) -> Value {
        let messages = plan.ordered_messages();
        let value: Value = serde_json::from_str(tool_result(&messages, call_id)).unwrap();
        value["model_body"].clone()
    }

    fn sampled_handoff(plan: &AgentRequestPlan) -> Value {
        let messages = plan.ordered_messages();
        let handoffs = messages
            .iter()
            .filter_map(|message| {
                let content = message.content.as_deref()?;
                let (_, handoff) = content.split_once("\npresentation_coach.v1\n")?;
                assert!(content.starts_with("sampling_guidance.v1\n"),
                    "a mid-turn plan must arrive at the sampling boundary, not before an already-sent user message");
                Some(handoff.trim())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            handoffs.len(),
            1,
            "the complete private plan must be supplied once, not duplicated into tool receipts"
        );
        serde_json::from_str(handoffs[0]).unwrap()
    }

    #[test]
    fn page_output_limit_recovers_once_after_prepare_and_preserves_the_sampled_plan() {
        let book = book();
        let mut port = Port::new(&book, "page-output-limit");
        let method = selected_method();
        let read = turn_calls(vec![call("accepted-read", "presentation.author", &json!({
            "operation":"read_method","method_id":method["id"],"revision":method["revision"],"file":"card.md"
        }).to_string())]);
        let mut adapter = Adapter::new(vec![discover(),prepare("accepted-prepare"),read],CoachResult::Ready);
        adapter.output_limits_at = vec![3,5];
        let mut context = context(&adapter);
        let outcome = execute(&mut port,&book,&adapter,&mut context).unwrap();
        assert!(outcome.incomplete && outcome.warning.as_deref() == Some(MODEL_OUTPUT_LIMIT));
        let plans = adapter.plans.borrow();
        let limited = &plans[2];
        let resumed = &plans[3];
        assert_eq!(limited.input, resumed.input[..limited.input.len()]);
        assert_eq!(limited.instructions, resumed.instructions);
        assert_eq!(resumed.input.len(), limited.input.len()+1);
        let recovered = |plan: &AgentRequestPlan| plan.input.last().and_then(|m| m.content.as_deref())
            .is_some_and(|text| text.contains("presentation_output_recovery.v1"));
        assert!(recovered(resumed));
        assert_eq!(plans.iter().filter(|p| recovered(p)).count(),1);
        assert!(resumed.tools.iter().any(|t| t.name=="presentation.author"));
        assert!(!serde_json::to_string(&resumed.input).unwrap().contains("discarded-write"));
        assert_eq!(port.reads_before_sampling.len(),1);
    }

    #[test]
    fn page_output_limit_without_preparation_after_cancellation_or_at_the_turn_limit_never_resamples() {
        let book = book();
        for scenario in ["unprepared","cancelled","turn-limit"] {
            let mut port = Port::new(&book, &format!("page-limit-{scenario}"));
            let turns = if scenario == "unprepared" { vec![discover()] } else {vec![discover(),prepare("accepted")]};
            let mut adapter = Adapter::new(turns, CoachResult::Ready);
            adapter.output_limits_at = vec![if scenario == "unprepared" {2} else {3}];
            let mut context = context(&adapter);
            if scenario == "cancelled" { adapter.cancel_at_limit=Some(context.cancellation.clone()); }
            // Discovery, prepare and the expression coach use the first three slots.
            // The truncated third outer sampling must occupy the final fourth slot.
            if scenario == "turn-limit" { context.config.max_turns=Some(4); }
            let outcome = execute(&mut port,&book,&adapter,&mut context);
            if scenario == "cancelled" {
                assert!(outcome.is_err());
            } else {
                assert_eq!(outcome.unwrap().warning.as_deref(), Some(MODEL_OUTPUT_LIMIT), "{scenario}");
            }
            assert!(adapter.plans.borrow().iter().all(|p| !p.input.iter().any(|m|
                m.content.as_deref().is_some_and(|text| text.contains("presentation_output_recovery.v1")))));
            assert!(port.reads_before_sampling.is_empty());
        }
    }

    #[test]
    fn presentation_coach_spatial_choice_supplies_three_to_a_local_author() {
        let catalog = crate::presentation_methods::catalog();
        let Some(method) = catalog["methods"].as_array().unwrap().iter().find(|m|
            m["scenes"].is_array() && m["needs"].as_array().is_some_and(|needs| needs.contains(&json!("three")))
        ) else { return }; // Isolated candidate catalogs can omit spatial methods.
        let mut plan = selected_plan();
        plan["methods"] = json!([{"method_id": method["id"], "revision": method["revision"],
            "adaptation": "用实际对象重做装配，在第二视角检查内部零件"}]);
        let book = book();
        let mut port = Port::new(&book, "spatial-scene-guidance");
        let adapter = Adapter::new(vec![discover(), prepare("local-without-three"), turn_final("已取得立体制作依据。")], CoachResult::WithPlan(plan));
        let mut context = context(&adapter);
        execute(&mut port, &book, &adapter, &mut context).unwrap();
        let plans = adapter.plans.borrow();
        assert_eq!(sampled_body(&plans[2], "local-without-three")["needs"], json!([]));
        let three = include_str!("../../../skills/presentation/references/three.md").trim();
        let includes_three = |plan: &AgentRequestPlan| plan.ordered_messages().iter()
            .any(|m| m.content.as_deref().is_some_and(|body| body.contains(three)));
        assert!(!includes_three(&plans[1]));
        assert!(includes_three(&plans[2]));
    }

    #[test]
    fn learning_quality_old_presentation_does_not_reopen_authoring_for_a_question() {
        let book=book();
        let mut port=Port::new(&book,"learning-quality-followup");
        let adapter=Adapter::new(vec![turn_final("先核对之前那句话的依据。")],CoachResult::Ready);
        let mut context=context(&adapter);
        context.tutor=Some(json!({"status":"practice","learner_context":{},
            "current_design":{"design_ref":"design:previous-turn","design":{
                "representation":{"kind":"presentation","observation":"此前的杠杆演示"},
                "focus_block_refs":["leverage"],"blocks":[{"block_id":"leverage"}]}}}));
        // Only the first sample is under inspection; the scripted final answer
        // intentionally does not complete the active tutor's delivery protocol.
        let _=execute(&mut port,&book,&adapter,&mut context);
        let plans=adapter.plans.borrow();
        let first=plans.first().expect("ordinary follow-up was sampled");
        assert!(!first.tools.iter().any(|tool|tool.name=="presentation.author"));
        assert!(!first.instruction_assets.iter().any(|asset|asset.asset_id.contains("presentation")));
        assert!(first.tools.iter().any(|tool|tool.name=="tool.search"),"authoring can still be discovered when needed");
    }

    #[test]
    fn presentation_coach_observing_prepare_reuses_plan_and_samples_selected_method() {
        let book = book();
        let mut port = Port::new(&book, "presentation-coach-observing");
        let method = selected_method();
        let adapter = Adapter::new(vec![
            discover(), prepare("prepare-first"),
            turn_calls(vec![call("selected-card", "presentation.author", &json!({
                "operation":"read_method", "method_id":method["id"], "revision":method["revision"], "file":"card.md"
            }).to_string())]),
            prepare("prepare-again"), turn_final("已取得方法，后续按完整阅读路径制作。"),
        ], CoachResult::Ready);
        let mut context = context(&adapter);
        let outcome = execute(&mut port, &book, &adapter, &mut context).unwrap();
        assert!(!outcome.incomplete, "{outcome:?}");
        let calls = adapter.coach_requests.borrow();
        assert_eq!(
            calls.len(),
            1,
            "repeated prepare must reuse this turn's coach result"
        );
        let packet: Value = serde_json::from_str(&calls[0].user).unwrap();
        let input = &packet["current_request_context"];
        assert_eq!(input["current_user_request"], USER_REQUEST);
        assert_eq!(input["author_proposal"]["framework"], AUTHOR_PROPOSAL);
        assert_eq!(input["tutor_context"]["status"], "observing");
        assert!(input.get("current_source_evidence").is_some());
        assert!(
            !input["current_source_evidence"]
                .to_string()
                .contains(AUTHOR_PROPOSAL),
            "the author proposal must not become actual source evidence"
        );
        assert!(packet["method_catalog"]["methods"]
            .as_array()
            .is_some_and(|methods| !methods.is_empty()));
        let plans = adapter.plans.borrow();
        assert_eq!(plans.len(), 5);
        let first = sampled_body(&plans[2], "prepare-first");
        let repeated = sampled_body(&plans[4], "prepare-again");
        assert_eq!(first["presentation_coach"]["status"], "ready");
        assert!(
            first["presentation_coach"].get("plan").is_none(),
            "the tool receipt should not duplicate the private handoff"
        );
        assert_eq!(first["presentation_coach"], repeated["presentation_coach"]);
        let handoff = sampled_handoff(&plans[2]);
        assert_eq!(handoff["plan"]["learning_goal"], GOAL);
        assert_eq!(handoff["plan"]["methods"][0]["adaptation"], ADAPTATION);
        assert_eq!(handoff, sampled_handoff(&plans[4]));
        assert_eq!(context.presentation_coach.as_ref(), Some(&handoff));
        assert_eq!(port.reads_before_sampling.len(), 1);
        assert!(
            port.reads_before_sampling[0].is_empty(),
            "selection or a tool call alone must not register a read"
        );
        let card = sampled_body(&plans[3], "selected-card");
        assert_eq!(card["status"], "method_read");
        assert!(
            card["text"].as_str().is_some_and(|text| !text.is_empty()),
            "actual method text must reach the author"
        );
        let observed = &port.observations;
        assert_eq!(observed.reads.len(), 1);
        assert_eq!(observed.reads[0].tool_call_id, "selected-card");
        assert_eq!(observed.reads[0].method_id, method["id"].as_str().unwrap());
        assert_eq!(
            observed.reads[0].characters,
            card["text"].as_str().unwrap().chars().count()
        );
    }

    #[test]
    fn presentation_coach_provider_failure_is_reused_and_author_continues() {
        let book = book();
        let mut port = Port::new(&book, "presentation-coach-unavailable");
        let adapter = Adapter::new(
            vec![
                discover(),
                prepare("prepare-first"),
                // A real technical preparation change must reuse the failed
                // coach attempt. An identical consecutive call is correctly
                // rejected by the existing no-progress guard before prepare.
                turn_calls(vec![call(
                    "prepare-again",
                    "presentation.author",
                    &json!({
                        "operation":"prepare", "phase":"local", "framework":AUTHOR_PROPOSAL,
                        "focus":"用一张摘要卡结束", "needs":["state"]
                    })
                    .to_string(),
                )]),
                turn_final("继续按当前请求提供完整讲解。"),
            ],
            CoachResult::ProviderFailure,
        );
        let mut context = context(&adapter);
        let outcome = execute(&mut port, &book, &adapter, &mut context).unwrap();
        assert_eq!(
            outcome.answer.as_deref(),
            Some("继续按当前请求提供完整讲解。")
        );
        assert!(!outcome.incomplete);
        assert_eq!(
            adapter.coach_requests.borrow().len(),
            1,
            "a second prepare must not retry a failed provider"
        );
        let plans = adapter.plans.borrow();
        assert_eq!(plans.len(), 4);
        let first = sampled_body(&plans[2], "prepare-first");
        let repeated = sampled_body(&plans[3], "prepare-again");
        assert_eq!(first["status"], "authoring_context_prepared");
        assert_eq!(
            repeated["status"], "authoring_context_prepared",
            "{repeated}"
        );
        assert_eq!(first["presentation_coach"]["status"], "unavailable");
        assert_eq!(first["presentation_coach"], repeated["presentation_coach"]);
        let handoff = sampled_handoff(&plans[2]);
        assert_eq!(handoff["status"], "unavailable");
        assert_eq!(handoff, sampled_handoff(&plans[3]));
        assert_eq!(context.presentation_coach.as_ref(), Some(&handoff));
    }

    #[test]
    fn presentation_coach_keeps_current_ui_action_when_user_supplement_is_empty() {
        use crate::tutor_continuation::{
            ContinuationMoveKind, InteractionAction, ResolvedInteraction, TeachingChoice,
        };
        let book = book();
        let mut port = Port::new(&book, "presentation-coach-ui-action");
        let adapter = Adapter::new(
            vec![
                discover(),
                prepare("prepare-click"),
                turn_final("已按所选比较安排演示。"),
            ],
            CoachResult::Ready,
        );
        let binding = memory::teaching::TeachingSourceBinding {
            source_id: book.base.book_id.clone(),
            source_revision: book.source_fingerprint().to_owned(),
            start_lid: "1.1".into(),
            end_lid: "1.1".into(),
        };
        let resolved = ResolvedInteraction {
            action: InteractionAction::UiChoice {
                teaching_choice: TeachingChoice {
                    delivery_ref: "delivery:offered-comparison".into(),
                    option_id: "compare-update".into(),
                },
                move_kind: ContinuationMoveKind::Compare,
            },
            learning_purpose: None,
            user_supplement: String::new(),
            accepted_request: "把更新前后的同一对象并排比较，并保留推导".into(),
            answer_ref: None,
            answer_text: None,
            source_excerpt: None,
            source_bindings: vec![binding.clone()],
            target: Some(memory::teaching::TeachingTarget { focus_ref:None,
                learning_focus: "更新前后同一对象的对应关系".into(),
                expected_performance: "分清对象状态与更新量".into(),
                capability: "comparison".into(),
                source_bindings: vec![binding],
                media_bindings: None,
                music_bindings: None,
                object_refs: vec![],
            }),
            expected_chat_head: Some("turn:offered-comparison".into()),
            presentation_follow_up: None,
            note_ref: None,
        };
        let action = serde_json::to_value(&resolved).unwrap();
        let prior_design = json!({"design_ref":"design:prior", "design":{
            "purpose":"OLD_DESIGN: 只查看符号定义", "representation":{"kind":"text"}
        }});
        let evidence =
            json!([{"fact_ref":"usage:learner-expression", "expression":"想对应更新前后的对象"}]);
        let mut context = context(&adapter);
        context.current_user_message = Some(String::new());
        context.tutor = Some(json!({
            "status":"observing", "current_action":action, "current_design":prior_design,
            "learner_context":{
                "current_action":action, "current_request":"OLD_REQUEST: 只查符号",
                "current_design":prior_design, "learner_expressions":evidence
            }
        }));
        let outcome = execute(&mut port, &book, &adapter, &mut context).unwrap();
        assert!(!outcome.incomplete, "{outcome:?}");
        let requests = adapter.coach_requests.borrow();
        assert_eq!(requests.len(), 1);
        let packet: Value = serde_json::from_str(&requests[0].user).unwrap();
        let input = &packet["current_request_context"];
        assert_eq!(
            input["current_user_request"], "",
            "a button click must not acquire typed words from a wrapper or prior design"
        );
        assert_eq!(
            input["tutor_context"]["current_action"], action,
            "the host-resolved request, target and source scope must survive deduplication"
        );
        assert_eq!(
            input["tutor_context"]["current_action"]["accepted_request"],
            resolved.accepted_request
        );
        assert_eq!(
            input["tutor_context"]["current_design"], prior_design,
            "the old design remains separately labelled context"
        );
        let learner = &input["tutor_context"]["learner_context"];
        for duplicate in ["current_request", "current_action", "current_design"] {
            assert!(
                learner.get(duplicate).is_none(),
                "duplicate {duplicate} must not compete with its authoritative owner"
            );
        }
        assert_eq!(
            learner["learner_expressions"], evidence,
            "deduplication must retain actual learner evidence"
        );
    }

    #[test]
    fn presentation_coach_spend_stop_terminates_before_another_author_sampling() {
        let book = book();
        let mut port = Port::new(&book, "presentation-coach-spend-stop");
        let adapter = Adapter::new(
            vec![
                discover(),
                prepare("prepare-stop"),
                turn_final("must not run"),
            ],
            CoachResult::SpendStop,
        );
        let mut context = context(&adapter);
        let error = execute(&mut port, &book, &adapter, &mut context).unwrap_err();
        assert_eq!(
            error.error_code,
            crate::model_spend::SpendStop::InsufficientAllowance.code()
        );
        assert_eq!(adapter.coach_requests.borrow().len(), 1);
        assert_eq!(
            adapter.plans.borrow().len(),
            2,
            "spend exhaustion must not degrade into continued author work"
        );
        assert!(
            context.presentation_coach.is_none(),
            "a spend stop is not an unavailable coach plan"
        );
    }

    #[test]
    fn presentation_coach_cancellation_terminates_without_unavailable_fallback() {
        let book = book();
        let mut port = Port::new(&book, "presentation-coach-cancel");
        let token = CancellationToken::default();
        let adapter = Adapter::new(
            vec![
                discover(),
                prepare("prepare-stop"),
                turn_final("must not run"),
            ],
            CoachResult::Cancel(token.clone()),
        );
        let mut context = context(&adapter);
        context.cancellation = token;
        let error = execute(&mut port, &book, &adapter, &mut context).unwrap_err();
        assert_eq!(error.error_code, "AGENT_RUN_CANCELLED");
        assert_eq!(adapter.coach_requests.borrow().len(), 1);
        assert_eq!(adapter.plans.borrow().len(), 2);
        assert!(
            context.presentation_coach.is_none(),
            "cancellation must not produce an unavailable coach handoff"
        );
        assert!(context.presentation_authoring.is_none());
    }
}
