//! Teaching remains owned by the Reader when an ACP Agent writes the answer.
use crate::agent_run::{AppStatePort, PreparedAgentChat};
use read_tools::ToolError;
use serde_json::{json, Value};

pub(crate) fn closed_book(prepared: &PreparedAgentChat) -> bool {
    prepared
        .tutor
        .as_ref()
        .is_some_and(|tutor| tutor["closed_book"]["status"] == "active")
}

/// Additional teacher instructions. The caller supplies the frozen user
/// message, which already includes the prepared course's visible lesson.
pub(crate) fn prompt(
    port: &impl AppStatePort,
    prepared: &PreparedAgentChat,
) -> Result<String, ToolError> {
    let Some(tutor) = &prepared.tutor else {
        return Ok(String::new());
    };
    if closed_book(prepared) {
        let practice = &tutor["closed_book"];
        let transport = "For the final response return exactly one JSON object with action, text, provides_help (boolean), reason, and the applicable target, criteria, learner_quote, new_case and followup_focus fields. The Reader checks this decision before delivering text. Omit Markdown code fences.";
        let rules = runtime::tutor::closed_book_instructions(practice, transport);
        let dialogue = port.with_app(|state| {
            prepared.scope.check_owner(state)?;
            let chat = state.user.agent_history.sessions.iter()
                .find(|s| s.id == prepared.turn_ref.session_id)
                .ok_or_else(|| invalid("原练习的对话已不可用。"))?;
            Ok::<_, ToolError>(practice["visible_turn_ids"].as_array().into_iter().flatten()
                .enumerate().filter_map(|(index, id)| {
                    let turn = chat.turns.iter().find(|t| Some(t.turn_id.as_str()) == id.as_str())?;
                    let text = turn.outcome.as_ref()?.answer.as_deref()?;
                    // A retry turn's incoming user reply belongs to the previous attempt.
                    Some(json!({"learner": (index > 0).then(|| turn.user_authored_text()), "teacher":text}))
                }).collect::<Vec<_>>())
        })?;
        let teaching_scope = json!({
            "target": practice["target"],
            "criteria": practice["criteria"],
            "course_origin": practice["course_origin"],
            "answer_origin": practice["answer_origin"],
            "delivered_dialogue": dialogue
        });
        let retry = if practice["assisted"] == true || practice["needs_retry"] == true {
            "\nFor a retry, compare the new question with the answer already disclosed below. Supply concrete details that require a new decision the learner must justify. For an evidence-reading target, you might give two short excerpts and ask which one supports a specific claim and why. Changing a name or a count while asking the same already-answered yes/no question leaves that judgment assisted."
        } else { "" };
        return Ok(format!(
            "\n{rules}{retry}\nFrozen teaching context, for teacher reference only:\n{teaching_scope}"
        ));
    }
    if tutor["status"] == "course" {
        return Ok("\nTeach the current prepared course step. Explain directly or give the requested hint, then let the learner respond. Reader records delivered assistance and the learner's own submission in the original course.".into());
    }
    if tutor["status"] == "practice" {
        let reference = &tutor["learner_context"]["responding_to_practice"]["delivery_ref"];
        let exercise = if let Some(id) = reference.as_str() {
            port.with_app(|state| {
                prepared.scope.check_owner(state)?;
                let store = state.user.learning_store_for_workspace(prepared.scope.learning_workspace_id.as_deref())?;
                let original = store.teaching_event(id)?;
                let facts = store.teaching_activity_events(id)?;
                let shown: Vec<_> = facts.iter().filter(|e| e.kind == memory::teaching::TeachingFact::HelpDisplayed)
                    .map(|e| &e.payload["text"]).collect();
                let attempts: Vec<_> = facts.iter().filter(|e| e.kind == memory::teaching::TeachingFact::LearnerAction
                    && matches!(e.payload["action"].as_str(), Some("submit" | "revise")))
                    .map(|e| json!({"response": e.payload["response"], "reasoning":e.payload["reasoning"]})).collect();
                Ok::<_, ToolError>(json!({"question":original.payload["move"]["prompt"],"displayed_help":shown,"submissions":attempts,
                    "action":tutor["learner_context"]["responding_to_practice"]["action"],"practice_history":tutor["learner_context"]["practice_history"],
                    "submitted_work":tutor["learner_context"]["responding_to_practice"]["submitted_work"]}))
            })?
        } else {
            Value::Null
        };
        let application = if tutor["application_requested"] == true {
            " Invent a new concrete situation with changed givens and require the learner to apply the previous relationship. Compare the problem with the prior explanation; if copying or reordering the original passage answers it, replace the problem. Write original prose in the reader's selected language and script, preserving source quotations."
        } else { "" };
        return Ok(format!("\nThe learner is practicing. Respond to the actual attempt and any help already given. A skip is a choice to move on; do not infer a difficulty or claim a streak from it. When referring to earlier answers use their actual outcomes and help in this conversation. If submitted_work contains a question in the learner's reasoning, address that question using their selected answer and the original problem. If asked for a new variant, give one self-contained question and wait for the learner's response.{application} Reader owns grading and completion. Original exercise (when supplied): {exercise}"));
    }
    Ok(String::new())
}

fn invalid(message: &str) -> ToolError {
    crate::user_storage_paths::error("ACP_TEACHING_INVALID", "validation", message)
}

/// A closed-book decision becomes a proposal for the *original* turn. The
/// normal successful answer finalizer commits the practice transition. A
/// cancelled/failed answer therefore never counts as delivered help or proof.
pub(crate) fn answer(
    port: &impl AppStatePort,
    prepared: &PreparedAgentChat,
    raw: &str,
) -> Result<String, ToolError> {
    reviewed_answer(port, prepared, raw, None)
}

pub(crate) fn reviewed_answer(
    port: &impl AppStatePort,
    prepared: &PreparedAgentChat,
    raw: &str,
    review: Option<Value>,
) -> Result<String, ToolError> {
    if !closed_book(prepared) {
        return Ok(raw.into());
    }
    use crate::teaching::invalid_closed_book_decision;
    let mut decision: Value = serde_json::from_str(raw.trim())
        .map_err(|_| invalid_closed_book_decision("请直接返回一个闭卷决定 JSON 对象，勿添加代码围栏或其他正文。"))?;
    if let Some(kind) = decision.get("type") {
        let Some(kind @ ("clarify" | "question" | "followup" | "help" | "retry" | "complete" | "pause" | "exit")) = kind.as_str() else {
            return Err(invalid_closed_book_decision("请在 action 中选择一个受支持的闭卷动作。"));
        };
        if decision.get("action").is_some_and(|action| action != kind) {
            return Err(invalid_closed_book_decision("action 与 type 给出了不同的闭卷动作，请只返回一个 action。"));
        }
        let kind = kind.to_owned();
        let fields = decision.as_object_mut().unwrap();
        fields.remove("type");
        fields.insert("action".into(), json!(kind));
    }
    if decision["provides_help"].as_bool().is_none()
        || decision["reason"]
            .as_str()
            .is_none_or(|reason| reason.trim().is_empty())
        || decision["action"] == "start"
        || !decision["action"].is_string()
    {
        return Err(invalid_closed_book_decision("闭卷回复需要合法的 action、布尔值 provides_help 和非空 reason。"));
    }
    if ["new_case", "text"].iter().any(|field| {
        decision[*field].as_str().is_some_and(|value| value.contains("[[source:"))
    })
    {
        return Err(invalid_closed_book_decision("闭卷题面或反馈必须能在当前对话中独立阅读。"));
    }
    let mut request = json!({"operation":"closed_book","closed_book":decision});
    if let Some(review) = review { request["transfer_review"] = review; }
    let proposal = port.with_app(|state| {
        prepared.scope.check_owner(state)?;
        crate::teaching::step(
            &prepared.scope.private_context(state),
            &prepared.turn_ref,
            request,
            &[],
            &[],
        )
    })?;
    proposal
        .pointer("/delivery_draft/text")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid("未找到已接纳的闭卷题面或反馈。"))
}

pub(crate) fn revision_prompt(original: &str, draft: &str, error: &ToolError) -> String {
    format!(r#"{original}

Reader rejected the preceding private draft before delivery. Revise it once for this same learner turn using the frozen context above.
Rejected draft: {}
Reader validation: {}
Return one corrected JSON object. Base shape: {{"action":"<choose a permitted action>","text":"<learner-facing text>","provides_help":false,"reason":"<actual reason>"}}. For question or retry with an answer_origin, add new_case with the entire changed situation. text then contains only the task to solve in that situation. Give the reader new facts that require a fresh judgment. For other actions follow their fields in the original instructions."#, json!(draft), error.message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_run::BorrowedAppPort;
    use crate::tests::{
        post,
        tutor_loop_tests::{
            assessed_activity, closed_book_tests::control, finish, fixture_with_assets, json_post,
            outcome,
        },
    };
    use crate::{prepare_agent_chat, AgentAnswerPart, AppState};
    use std::cell::RefCell;

    fn prepare(state: &mut AppState, input: Value) -> PreparedAgentChat {
        prepare_agent_chat(state, &input.to_string(), "now")
            .unwrap_or_else(|failure| panic!("{}", failure.body))
    }

    fn deliver(state: &mut AppState, run: &PreparedAgentChat, raw: &str) {
        let real_answer = answer(&BorrowedAppPort(RefCell::new(state)), run, raw).unwrap();
        let mut result = outcome(None);
        result.answer = Some(real_answer.clone());
        result.answer_view.as_mut().unwrap().parts =
            vec![AgentAnswerPart::Markdown { text: real_answer }];
        finish(state, &run.turn_ref, &result);
    }

    #[test]
    fn acp_question_help_is_attributed_on_display_and_the_reader_grades_the_actual_submission() {
        let (_root, mut state) = fixture_with_assets(true);
        let (_, id, _) = assessed_activity(
            &mut state,
            json!({"kind":"choice_set","choices":["A","B","C"],"correct":["A","B"]}),
            "on_reveal",
        );
        let request = prepare(
            &mut state,
            json!({"message":"Can you give me one hint?", "teaching_ref":id}),
        );
        let instructions = prompt(&BorrowedAppPort(RefCell::new(&mut state)), &request).unwrap();
        assert!(instructions.contains("Choose the necessary quantities"));
        let hint = "Think about the distance and the total time.";
        deliver(&mut state, &request, hint);
        let store = state.user.learning_store().unwrap();
        let turn = &request.turn_ref.turn_id;
        let follow_up = format!("followup:{turn}");
        assert_eq!(
            store.teaching_event(&follow_up).unwrap().payload["answer"],
            hint
        );
        let help_receipt = format!("help-seen:{follow_up}");
        assert!(store.teaching_event(&help_receipt).is_err());
        json_post(
            &mut state,
            "/tutor/display",
            json!({"delivery_ref":follow_up}),
        );
        let submitted = json_post(
            &mut state,
            "/tutor/action",
            json!({"operation_id":"actual-attempt",
            "delivery_ref":id,"action":"submit","response":"A"}),
        );
        assert_eq!(submitted["assessment"], "partial");
        let actual = store.teaching_event("action:actual-attempt").unwrap();
        assert_eq!(actual.payload["response"], "A");
        assert_eq!(actual.payload["assistance_refs"], json!([help_receipt]));
        let feedback = prepare(
            &mut state,
            json!({"message":"What did I miss?","teaching_ref":submitted["event_id"]}),
        );
        let instructions = prompt(&BorrowedAppPort(RefCell::new(&mut state)), &feedback).unwrap();
        assert!(instructions.contains(hint));
        assert!(instructions.contains(r#""response":"A""#));
    }

    #[test]
    fn acp_course_hint_returns_to_the_original_step_and_attaches_to_the_real_learner_response() {
        let (_root, mut state) = fixture_with_assets(false);
        let mut course: memory::course::InquiryCourse = serde_json::from_str(include_str!(
            "../../../docs/examples/inquiry-course/ai-infra-resource-bounds.v1.course.json"
        ))
        .unwrap();
        let lesson = &mut course.lessons[0];
        lesson.summary.source.publication.book_id = state.workspace.book.base.book_id.clone();
        lesson.summary.source.publication.publication_id = "local".into();
        lesson.summary.source.source_revision = state.workspace.book.source_fingerprint().into();
        let mut store = state.user.learning_store().unwrap();
        let goal = store.state().unwrap().workspace_goal_revision.unwrap_or(0);
        let initial = store
            .mutate_course(
                &serde_json::from_value(json!({
                    "operation_id":"start","run_id":null,"expected_revision":0,"release_version":1,
                    "action":{"kind":"start","lesson_id":lesson.summary.id}
                }))
                .unwrap(),
                Some(("fixture", 1, lesson)),
                goal,
                "now",
                None,
            )
            .unwrap();
        let run = &initial.progress.run_id;
        let learner_question = "两个资源约束怎样比较？";
        let hint = "分别算出读取和计算所需的时间，再判断它们对同一段执行时间的约束。";
        let request = prepare(
            &mut state,
            json!({"message":learner_question,
            "course_context":{"run_id":run,"revision":initial.progress.revision,"surface":"course"}}),
        );
        assert_eq!(request.tutor.as_ref().unwrap()["status"], "course");
        let turn = &request.turn_ref.turn_id;
        let instructions = prompt(&BorrowedAppPort(RefCell::new(&mut state)), &request).unwrap();
        assert!(instructions.contains("current prepared course step"));
        deliver(&mut state, &request, hint);
        let help = format!("usage:{run}:assistant:{turn}");
        assert_eq!(store.teaching_event(&help).unwrap().payload["answer"], hint);
        assert_eq!(store.course_view(run).unwrap(), initial);
        let response = "我想比较读取与计算各自的耗时。";
        store
            .mutate_course(
                &serde_json::from_value(json!({"operation_id":"ask","run_id":run,
            "expected_revision":initial.progress.revision,"release_version":1,
            "action":{"kind":"ask_question","response":response}}))
                .unwrap(),
                None,
                goal,
                "now",
                None,
            )
            .unwrap();
        let submitted = store
            .teaching_event(&format!("{run}:ask:question"))
            .unwrap();
        assert!(submitted.payload["support_delivery_refs"]
            .as_array()
            .unwrap()
            .contains(&json!(help)));
        assert_eq!(submitted.payload["response"], response);
    }

    #[test]
    fn acp_closed_book_revises_only_a_rejected_draft_once_before_delivery() {
        use crate::agent_run::RunCoordinator;
        use std::{path::Path, sync::{atomic::AtomicBool, Arc, Mutex}};
        let (root, mut state) = fixture_with_assets(false);
        post(&mut state, "/agent/new", "{}");
        let source = prepare(&mut state, json!({"message":"列子为何仍有所待？"}));
        deliver(&mut state, &source, "列子需要靠风，风不吹他就不能飞。");
        control(&mut state, "start", json!({"answer_turn_id":source.turn_ref.turn_id}));
        let clarification = prepare(&mut state, json!({"message":"我来解释一下"}));
        let mut draft = json!({"type":"clarify","action":"complete","text":"想练哪个问题？",
            "reason":"先确认所选目标","provides_help":false});
        assert!(answer(&BorrowedAppPort(RefCell::new(&mut state)), &clarification, &draft.to_string()).is_err());
        draft.as_object_mut().unwrap().remove("action");
        deliver(&mut state, &clarification, &draft.to_string());
        let started = control(&mut state, "state", json!({}));
        assert_eq!(started["criteria"], "");
        assert_eq!(started["visible_turn_ids"].as_array().unwrap().len(), 1);
        let state = Arc::new(Mutex::new(state));
        let mut coordinator = RunCoordinator::new(state.clone(), Arc::new(AtomicBool::new(false)),
            crate::observability::ObservabilityRuntime::disabled(None));
        coordinator.local_profile = Some(root.path().to_owned());
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tests/fixtures/acp_teaching.cjs");
        let next = root.path().join("teacher.json");
        let record = root.path().join("samples.txt");
        crate::local_acp::update(root.path(), &json!({"action":"save","agent":{
            "id":"teacher","name":"teacher","command":"node","args":[script,next]}})).unwrap();
        let rejected = "列子凭什么飞行？";
        let original = json!({"action":"question","text":rejected,"reason":"判断依赖条件","provides_help":false});
        let case = "飞艇可在无风时升空，气囊漏气时却会降落。";
        let public_task = "它的飞行受什么条件限制？请说明理由。";
        let revised = json!({"action":"question","new_case":case,"text":public_task,
            "reason":"解释新的依赖关系","provides_help":false});
        for (stop, correction, expected_samples, passes) in [
            ("max_tokens", &original, "initial\n", false),
            ("end_turn", &original, "initial\nrevision\n", false),
            ("end_turn", &revised, "initial\nrevision\n", true),
        ] {
            std::fs::write(&record, "").unwrap();
            std::fs::write(&next, serde_json::to_vec(&json!({"decision":original,"revision":correction,
                "context":"风不吹他就不能飞","rejected":rejected,"validation":"请在 new_case 单独给出",
                "stop":stop,"record":record})).unwrap()).unwrap();
            let result = coordinator.run(r#"{"local_agent_id":"teacher","message":"我来解释一下"}"#, "now",
                &crate::tests::ChatStubAdapter::scripted(vec![]), || {}, || {});
            assert_eq!(std::fs::read_to_string(&record).unwrap(), expected_samples);
            assert_eq!(result.status == 200, passes, "{}", result.body);
            let saved = control(&mut state.lock().unwrap(), "state", json!({}));
            if passes {
                assert_eq!(saved["transfer_condition"], case);
                let text = format!("{case}\n\n{public_task}");
                let delivered = state.with_app(|s| {
                    let session = s.user.agent_history.sessions.iter()
                        .find(|session| Some(&session.id) == s.workspace.selected_chat.as_ref())?;
                    session.turns.last()?.outcome.as_ref()?.answer.clone()
                });
                assert_eq!(delivered, Some(text));
                assert_eq!(saved["visible_turn_ids"].as_array().unwrap().len(), 1);
                assert!(!result.body.contains(rejected));
            } else {
                assert_eq!(saved["revision"], started["revision"]);
            }
        }
        let learner = "没风也能飞；但浮力不足时就会落下。";
        std::fs::write(&record, "").unwrap();
        let complete = json!({"action":"complete","learner_quote":"浮力不足时就会落下","text":"你说明了气囊漏气为何影响飞行。",
            "reason":"PRIVATE_AUTHOR_VERDICT","provides_help":false});
        std::fs::write(&next, serde_json::to_vec(&json!({"context":case,"decision":complete,"record":record,"learner":learner,
            "comparison":{"uses_original_relationship":true,"prior_answer_suffices":false,"prior_quote":"列子需要靠风","case_quote":"气囊漏气",
                "reason":"飞艇有新的浮力条件，原来的御风理由不足以解释后果。"},
            "review":{"verdict":"independent","case_quote":"气囊漏气","learner_quote":"浮力不足时就会落下",
            "reason":"学习者由气囊漏气推断浮力不足会造成降落。"}})).unwrap()).unwrap();
        let result = coordinator.run(&json!({"local_agent_id":"teacher","message":learner}).to_string(),
            "now", &crate::tests::ChatStubAdapter::scripted(vec![]), || {}, || {});
        assert_eq!(result.status, 200, "{}", result.body);
        assert_eq!(std::fs::read_to_string(&record).unwrap(), "initial\ncomparison\nreview\n");
        let saved = control(&mut state.lock().unwrap(), "state", json!({}));
        assert_eq!(saved["status"], "complete");
        assert_eq!(saved["completion_reason"], "学习者由气囊漏气推断浮力不足会造成降落。");
    }

    #[test]
    fn acp_closed_book_preserves_the_assisted_attempt_and_only_accepts_a_new_independent_reply() {
        use crate::agent_run::RunCoordinator;
        use std::{
            path::Path,
            sync::{atomic::AtomicBool, Arc, Mutex},
            time::Duration,
        };
        let (root, mut state) = fixture_with_assets(false);
        post(&mut state, "/agent/new", "{}");
        control(&mut state, "start", json!({"target":"全程平均速度"}));
        let state = Arc::new(Mutex::new(state));
        let mut coordinator = RunCoordinator::new(
            state.clone(),
            Arc::new(AtomicBool::new(false)),
            crate::observability::ObservabilityRuntime::disabled(None),
        );
        coordinator.local_profile = Some(root.path().to_owned());
        let script =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tests/fixtures/acp_teaching.cjs");
        let next = root.path().join("next-turn.json");
        for id in ["teacher-a", "teacher-b"] {
            crate::local_acp::update(
                root.path(),
                &json!({"action":"save","agent":{"id":id,"name":id,
                "command":"node","args":[script,next]}}),
            )
            .unwrap();
        }
        let send = |agent: &str, learner: &str, context: &str, decision: Value, success: bool| {
            std::fs::write(
                &next,
                serde_json::to_vec(&json!({"context":context,"decision":decision})).unwrap(),
            )
            .unwrap();
            let mut live = None;
            let response = coordinator.run(
                &json!({"local_agent_id":agent,"message":learner}).to_string(),
                "now",
                &crate::tests::ChatStubAdapter::scripted(vec![]),
                || { live = state.with_app(|s| s.workspace.active_agent_stream.as_ref()?.upgrade()); },
                || {},
            );
            assert_eq!(response.status == 200, success, "{}", response.body);
            assert!(!response.body.contains("PRIVATE_TEACHER_REFERENCE"));
            let turn = state.with_app(|s| {
                s.user
                    .agent_history
                    .sessions
                    .iter()
                    .find(|session| Some(&session.id) == s.workspace.selected_chat.as_ref())
                    .unwrap()
                    .turns
                    .last()
                    .unwrap()
                    .clone()
            });
            assert_eq!(turn.user_authored_text(), learner);
            if success {
                assert_eq!(
                    turn.outcome.unwrap().answer.as_deref(),
                    decision["text"].as_str()
                );
            }
            let (events, terminal) = live.unwrap().read_after(Some(0), Duration::ZERO);
            assert!(terminal);
            assert!(!events.is_empty());
            for event in events {
                let text = event.payload.to_string();
                assert!(!text.contains("PRIVATE_TEACHER_REFERENCE"));
                assert!(!text.contains("provides_help"));
            }
        };
        let initial_question = "停车时间算入平均速度吗？";
        let hint = "全程时间从出发算到到达，包含停车。";
        let variant = "相同距离，一辆车多等了一盏红灯，哪辆平均速度较高？";
        send(
            "teacher-a",
            "开始练习",
            "全程平均速度",
            json!("我觉得你已经会了"),
            false,
        );
        send(
            "teacher-a",
            "请出题",
            "全程平均速度",
            json!({"action":"question","criteria":"解释总时间",
            "text":initial_question,"provides_help":false,"reason":"独立判断"}),
            true,
        );
        send(
            "teacher-a",
            "给一个提示",
            initial_question,
            json!({"action":"help","text":hint,
            "provides_help":true,"reason":"提供关键关系"}),
            true,
        );
        send(
            "teacher-b",
            "总时间包含停车。",
            hint,
            json!({"action":"complete","text":"回答正确。",
            "learner_quote":"总时间包含停车","provides_help":false,"reason":"给出关系"}),
            false,
        );
        send(
            "teacher-b",
            "换一道题。",
            hint,
            json!({"action":"retry","text":variant,
            "provides_help":false,"reason":"换条件验证应用"}),
            true,
        );
        let independent = "没多等的车总时间短，距离相同，所以平均速度高。";
        send(
            "teacher-b",
            independent,
            variant,
            json!({"action":"complete","text":"你说明了总时间对平均速度的影响。",
            "learner_quote":independent,"provides_help":false,"reason":"独立解释了两个量与结论"}),
            true,
        );
        let saved = control(&mut state.lock().unwrap(), "state", json!({}));
        assert_eq!(saved["status"], "complete");
        assert_eq!(saved["attempt"], 2);
        assert_eq!(saved["assisted"], false);
        assert_eq!(saved["learner_quote"], independent);
    }
}
