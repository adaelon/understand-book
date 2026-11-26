use super::super::tutor_loop_tests::{finish, outcome};
use super::explain_back_admission::{explanation, original_answer};
use super::*;
use memory::learning_evidence::EvidenceNature;

fn later_submission(origin: &AgentTurnRef, head: &AgentTurnRef, key: &str) -> Value {
    let mut input = explanation(origin);
    input["expected_chat_head"] = json!(head.turn_id);
    input["client_request_id"] = json!(key);
    input["expression_id"] = json!(key);
    input["operation_id"] = json!(key);
    input
}

fn comparison_fixture(round: &str, binding: &memory::teaching::TeachingBinding, turn: &str, response: &Value) -> Value {
    json!({"round_id":round,"source":{"source_id":binding.source_id,"source_revision":binding.source_revision},
        "book_graph":{"nodes":[{"id":"observation","label":"Clock still running","evidence":[]},
            {"id":"clock","label":"Elapsed time","condition":"during the stop","evidence":[]},
            {"id":"total","label":"Total time","evidence":[]}],
            "edges":[{"id":"observed","from":"observation","to":"clock","relation_type":"supports","evidence":[{"lid":"1.1","quote":scenario()["quote"]}]},
                {"id":"b","from":"clock","to":"total","relation_type":"defines","evidence":[{"lid":"1.1","quote":scenario()["quote"]}]}]},
        "student_graph":{"nodes":[{"id":"s","label":"Count the stop","quotes":[{"turn_id":turn,"text":response}]}],"edges":[]},
        "rows":[{"row_id":"r","book_edge_ids":["b","observed"],"book_node_ids":[],"student_edge_ids":[],"student_node_ids":["s"],
            "status":"consistent","summary":"Both count the stop."}],"side_notes":[]})
}

fn legacy_comparison_fixture(round: &str, binding: &memory::teaching::TeachingBinding, turn: &str, response: &Value) -> Value {
    json!({"round_id":round,"source":{"source_id":binding.source_id,"source_revision":binding.source_revision},
        "book_relations":[{"id":"b","condition":"during the stop","relation_type":"defines","conclusion":"time still counts",
            "evidence":[{"lid":"1.1","quote":scenario()["quote"]}]}],
        "student_relations":[{"id":"s","relation_type":"concludes","conclusion":"Count the stop",
            "quotes":[{"turn_id":turn,"text":response}]}],
        "rows":[{"row_id":"r","book_relation_ids":["b"],"student_relation_ids":["s"],"status":"consistent","summary":"Both count the stop."}],
        "side_notes":[]})
}

#[test]
fn teach_back_receipt_freezes_original_and_only_counts_displayed_same_answer() {
    let (root, mut state) = fixture(false);
    let origin = original_answer(&mut state);
    let first = prepare(&mut state, explanation(&origin)).turn_ref;
    let round = format!("tb:{}:r1", first.turn_id);
    let store = state.user.learning_store().unwrap();
    let binding = store
        .teaching_event(&format!("binding:{}", first.turn_id))
        .unwrap()
        .binding;
    let prior = format!(
        "usage:{}:assistant:{}",
        binding.tutor_session_id, origin.turn_id
    );
    let input = store
        .teaching_event(&format!(
            "usage:{}:user:{}",
            binding.tutor_session_id, first.turn_id
        ))
        .unwrap();
    assert_eq!(input.payload["support_delivery_refs"], json!([prior]));
    assert!(input.learner_expression().is_none());
    assert!(store.teaching_event_sequence(&round).unwrap().is_none());
    assert_eq!(
        store.teaching_event(&prior).unwrap().binding.turn_id,
        origin.turn_id
    );

    let card = legacy_comparison_fixture(&round, &binding, &first.turn_id, &input.payload["response"]);
    let mut result = outcome(None);
    result.answer_view.as_mut().unwrap().parts = vec![AgentAnswerPart::TeachBackComparison {
        comparison: serde_json::from_value(card.clone()).unwrap(),
    }];
    result.answer = None;
    finish(&mut state, &first, &result);
    let delivered = store.teaching_event(&round).unwrap();
    assert_eq!(delivered.kind, TeachingFact::TeachBackDelivered);
    assert!(delivered.payload["comparison"]["book_graph"].is_object());
    // A previous release committed the old wire format to the immutable trace.
    let sql = rusqlite::Connection::open(root.path().join("learning.db")).unwrap();
    assert_eq!(sql.execute("UPDATE teaching_trace SET event=json_set(event,'$.payload.comparison',json(?)) WHERE event_id=?",
        rusqlite::params![card.to_string(), round]).unwrap(), 1);
    crate::teaching::record_delivery(&state.private_context(), &first, "replay").unwrap();
    let evidence_id = format!("interpretation:{round}");
    let evidence = store.evidence(&evidence_id).unwrap();
    assert_eq!(evidence.nature, EvidenceNature::Hypothesis);
    assert_eq!(evidence.delivery_ref.as_ref(), Some(&round));
    assert_eq!(evidence.learner_quote, input.payload["response"]);
    assert_eq!(evidence.support_delivery_refs, vec![prior.clone()]);
    assert!(store.teaching_activity_events(&round).unwrap().is_empty());

    let before = prepare(
        &mut state,
        later_submission(&origin, &first, "before-display"),
    )
    .turn_ref;
    let before_id = format!("usage:{}:user:{}", binding.tutor_session_id, before.turn_id);
    assert_eq!(
        store.teaching_event(&before_id).unwrap().payload["support_delivery_refs"],
        json!([prior])
    );
    finish(&mut state, &before, &outcome(None));
    // The comparison write can succeed before its derived evidence is committed.
    // Reopening the card reconciles both without another model request.
    assert_eq!(
        sql.execute(
            "DELETE FROM learning_evidence WHERE evidence_id=?",
            [&evidence_id]
        )
        .unwrap(),
        1
    );
    let display =
        json!({"chat_session_id":first.session_id,"turn_id":first.turn_id,"round_id":round});
    let receipt = post_ok(&mut state, "/tutor/display", display.clone());
    assert_eq!(post_ok(&mut state, "/tutor/display", display), receipt);
    assert_eq!(store.evidence(&evidence_id).unwrap(), evidence);
    assert_eq!(store.teaching_event(&round).unwrap().payload["comparison"], card);
    assert_eq!(store.teaching_activity_events(&round).unwrap().len(), 1);
    let mut updated_source = binding.clone();
    updated_source.source_revision = "changed edition".into();
    assert!(store.displayed_teach_backs(&updated_source, &origin.turn_id).unwrap().is_empty());
    assert_eq!(
        store.teaching_event(&before_id).unwrap().payload["support_delivery_refs"],
        json!([prior])
    );
    let unrelated = prepare(&mut state, json!({"message":"Where is the next chapter?"})).turn_ref;
    let unrelated_input = store
        .teaching_event(&format!(
            "usage:{}:user:{}",
            binding.tutor_session_id, unrelated.turn_id
        ))
        .unwrap();
    assert!(!unrelated_input.payload["support_delivery_refs"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|id| id == &round));
    finish(&mut state, &unrelated, &outcome(None));
    let after = prepare(
        &mut state,
        later_submission(&origin, &unrelated, "after-display"),
    )
    .turn_ref;
    let after_input = store
        .teaching_event(&format!(
            "usage:{}:user:{}",
            binding.tutor_session_id, after.turn_id
        ))
        .unwrap();
    assert_eq!(
        after_input.payload["support_delivery_refs"],
        json!([prior, round])
    );
    let mut next_card = card;
    let next_round = format!("tb:{}:r1", after.turn_id);
    next_card["round_id"] = json!(next_round);
    next_card["student_relations"][0]["quotes"][0]["turn_id"] = json!(after.turn_id);
    result.answer_view.as_mut().unwrap().parts = vec![AgentAnswerPart::TeachBackComparison {
        comparison: serde_json::from_value(next_card).unwrap(),
    }];
    finish(&mut state, &after, &result);
    let view = post_ok(&mut state, "/tutor/evidence", json!({"evidence_ref":format!("interpretation:{next_round}")}));
    assert_eq!(view["prompt"], "Why include stopped time?");
    let seen = view["assistance"].as_array().unwrap().iter().find(|help| help["fact_ref"] == round).unwrap();
    assert_eq!(seen["fact_sequence"], json!(store.teaching_event_sequence(receipt["event_id"].as_str().unwrap()).unwrap()));
    // This saved card predates card-level support metadata. Reopening it must
    // recover both the original explanation and the earlier displayed comparison.
    let legacy_display = json!({"chat_session_id":after.session_id,"turn_id":after.turn_id,"round_id":next_round});
    let legacy = post_ok(&mut state, "/tutor/display", legacy_display.clone());
    assert_eq!(legacy["support_delivery_refs"], json!([prior, round]));
    assert_eq!(post_ok(&mut state, "/tutor/display", legacy_display), legacy);
}

#[test]
fn teach_back_help_only_turn_has_no_learner_evidence_or_displayable_card() {
    let (_root, mut state) = fixture(false);
    let origin = original_answer(&mut state);
    let mut request = explanation(&origin);
    request["message"] = json!("Could you explain elapsed time first?");
    let turn = prepare(&mut state, request).turn_ref;
    finish(&mut state, &turn, &outcome(None));
    let store = state.user.learning_store().unwrap();
    let binding = store
        .teaching_event(&format!("binding:{}", turn.turn_id))
        .unwrap()
        .binding;
    let facts = store
        .teaching_turn_events(&binding.tutor_session_id, &turn.turn_id)
        .unwrap();
    assert!(facts.iter().all(|f| f.learner_expression().is_none()));
    assert!(facts
        .iter()
        .all(|f| f.kind != TeachingFact::TeachBackDelivered));
    assert!(store
        .current_interpretations(&binding.source_id, &binding.source_revision, 0, 10)
        .unwrap()
        .is_empty());
    let display = json!({"chat_session_id":turn.session_id,"turn_id":turn.turn_id,"round_id":format!("tb:{}:r1",turn.turn_id)});
    assert_ne!(
        post(&mut state, "/tutor/display", &display.to_string()).status,
        200
    );
}

#[test]
fn closed_book_only_uses_a_displayed_comparison_and_hides_history_until_exit() {
    use super::super::tutor_loop_tests::closed_book_tests::control;
    let (_root, mut state) = fixture(false);
    let origin = original_answer(&mut state);
    let entry = explanation(&origin);
    let turn = prepare(&mut state, entry.clone()).turn_ref;
    let round = format!("tb:{}:r1", turn.turn_id);
    let store = state.user.learning_store().unwrap();
    let binding = store.teaching_event(&format!("binding:{}", turn.turn_id)).unwrap().binding;
    let card = comparison_fixture(&round, &binding, &turn.turn_id, &entry["message"]);
    let mut outcome = outcome(None);
    outcome.answer_view.as_mut().unwrap().parts = vec![AgentAnswerPart::TeachBackComparison {
        comparison: serde_json::from_value(card).unwrap(),
    }];
    finish(&mut state, &turn, &outcome);

    let practice = control(&mut state, "start", json!({"answer_turn_id":origin.turn_id}));
    assert!(practice["support_delivery_refs"].is_null());
    control(&mut state, "exit", json!({}));
    let display = json!({"chat_session_id":turn.session_id,"turn_id":turn.turn_id,"round_id":round});
    post_ok(&mut state, "/tutor/display", display.clone());
    let practice = control(&mut state, "resume", json!({}));
    assert_eq!(practice["support_delivery_refs"], json!([round]));
    assert!(!practice.to_string().contains("Both count the stop."));
    assert_eq!(practice["assisted"], false);
    let run = prepare(&mut state, json!({"message":"Give me an independent problem."}));
    let teacher = run.tutor.as_ref().unwrap();
    assert!(runtime::tutor::control_instructions(teacher).contains("Both count the stop."));
    assert!(runtime::tutor::control_instructions(teacher).contains("Elapsed time —defines→ Total time"));
    assert!(crate::acp_teaching::prompt(&crate::agent_run::BorrowedAppPort(std::cell::RefCell::new(&mut state)), &run)
        .unwrap().contains("Both count the stop."));
    let propose = |state: &AppState, case: &str| teaching::step(&state.private_context(), &run.turn_ref,
        json!({"operation":"closed_book","closed_book":{"action":"question","new_case":case,"text":"Which interval counts?",
            "target":"elapsed time","provides_help":false,"reason":"Identify the measured interval."}}), &[], &[]);
    assert!(propose(&state, "during the stop").is_err());
    let variant = "A second timer is started only after the stop has ended.";
    let question = propose(&state, variant).unwrap()["delivery_draft"]["text"].as_str().unwrap().to_owned();
    assert_eq!(question, format!("{variant}\n\nWhich interval counts?"));
    outcome.answer_view.as_mut().unwrap().parts = vec![AgentAnswerPart::Markdown {text: question.clone()}];
    outcome.answer = Some(question);
    finish(&mut state, &run.turn_ref, &outcome);
    let url = format!("/agent/history/recap?session_id={}", turn.session_id);
    for action in ["state", "pause", "resume"] {
        let saved = control(&mut state, action, json!({}));
        assert_eq!(saved["transfer_condition"], variant);
        assert_eq!(saved["visible_turn_ids"], json!([run.turn_ref.turn_id]));
        let history = get(&mut state, "/agent/history");
        assert_eq!(history.status, 200);
        assert!(!history.body.contains("Both count the stop."));
        let view: Value = serde_json::from_str(&history.body).unwrap();
        assert_eq!(view["current"]["turns"].as_array().unwrap().len(), 1);
        assert_eq!(view["current"]["turns"][0]["turn_id"], run.turn_ref.turn_id);
        assert!(get(&mut state, &url).body.contains("CLOSED_BOOK_HISTORY_COVERED"));
        assert_ne!(post(&mut state, "/tutor/display", &display.to_string()).status, 200);
    }
    control(&mut state, "exit", json!({}));
    assert_eq!(get(&mut state, &url).status, 200);
    assert!(get(&mut state, "/agent/history").body.contains("Both count the stop."));
    assert_eq!(post_ok(&mut state, "/tutor/display", display)["event_id"], format!("display:{round}"));
}


#[test]
fn course_teach_back_uses_one_reflection_and_keeps_display_and_advance_separate() {
    use memory::course::{CourseMutation, CourseStage, InquiryCourse};
    let (root, mut state) = fixture(false);
    let mut course: InquiryCourse = serde_json::from_str(include_str!(
        "../../../../docs/examples/inquiry-course/ai-infra-resource-bounds.v1.course.json")).unwrap();
    let lesson = &mut course.lessons[0];
    let source = &mut lesson.summary.source;
    source.publication.book_id = state.workspace.book.base.book_id.clone();
    source.publication.publication_id = "local".into();
    source.source_revision = state.workspace.book.source_fingerprint().into();
    source.start_lid = "1.1".into();
    source.end_lid = "1.2".into();
    lesson.assessment.source_lids = vec!["1.1".into()];
    let mutation = |name: &str, run: Option<&str>, revision: u32, action:Value| -> CourseMutation {
        serde_json::from_value(json!({"operation_id":name,"run_id":run,"release_version":1,
            "expected_revision":revision,"action":action})).unwrap()
    };
    let mut store = state.user.learning_store().unwrap();
    let mut view = store.mutate_course(&mutation("start",None,0,json!({"kind":"start","lesson_id":lesson.summary.id})),
        Some(("fixture",1,lesson)),1,"now",None).unwrap();
    let run = view.progress.run_id.clone();
    for (id, action) in [
        ("predict",json!({"kind":"advance"})),
        ("submit-prediction",json!({"kind":"predict","response":{"choice":"compute","reason":"My prediction"}})),
        ("baseline",json!({"kind":"observe","preset":"baseline"})),
        ("compute",json!({"kind":"observe","preset":"double_compute"})),
        ("bandwidth",json!({"kind":"observe","preset":"double_bandwidth"})),
        ("explanation",json!({"kind":"advance"})),
    ] {
        view = store.mutate_course(&mutation(id,Some(&run),view.progress.revision,action),None,1,"now",None).unwrap();
    }
    let context = json!({"run_id":run,"revision":view.progress.revision,"surface":"course"});
    let question = prepare(&mut state,json!({"message":"What do you mean by total time?","course_context":context}));
    finish(&mut state,&question.turn_ref,&outcome(None));
    let earlier_answer = format!("usage:{run}:assistant:{}", question.turn_ref.turn_id);
    let before_delivery = store.course_run(&run).unwrap();
    let submitted = "Count stopped time.";
    let submit = json!({"message":submitted,"entry_action":"explain_back","expression_id":"course-expression",
        "operation_id":"first","client_request_id":"course-expression","session_id":state.workspace.selected_chat,"expected_chat_head":question.turn_ref.turn_id,
        "course_context":context});
    let first = prepare(&mut state,submit.clone()).turn_ref;
    let chat = state.user.agent_history.sessions.iter().find(|s| s.id == first.session_id).unwrap();
    let saved = chat.turns.last().unwrap();
    assert_eq!(crate::turn_view(&state.workspace.book, saved).domain.course_retry_context.as_ref(), Some(&context));
    let round = format!("tb:{}:r1",first.turn_id);
    let binding = store.teaching_event(&format!("binding:{}",first.turn_id)).unwrap().binding;
    assert_eq!(binding.tutor_session_id,run);
    let card=comparison_fixture(&round,&binding,&first.turn_id,&json!(submitted));
    let mut result=outcome(None);
    result.answer_view.as_mut().unwrap().parts=vec![AgentAnswerPart::TeachBackComparison {
        comparison:serde_json::from_value(card.clone()).unwrap(),
    }];
    finish(&mut state,&first,&result);
    assert!(crate::interaction_input::local_repeat(&mut state,&submit.to_string()).unwrap().is_some());
    // Emulate shutdown after the chat and evidence committed but before the course row update.
    let db = rusqlite::Connection::open(root.path().join("learning.db")).unwrap();
    db.execute("UPDATE course_runs SET state=? WHERE run_id=?",
        rusqlite::params![serde_json::to_string(&before_delivery).unwrap(),run]).unwrap();
    crate::teaching_coach::recover_user(&state.user).unwrap();
    assert_eq!(store.course_run(&run).unwrap().reflection.as_deref(),Some(submitted));
    crate::teaching::record_delivery(&state.private_context(),&first,"again").unwrap();
    let view=store.course_view(&run).unwrap();
    assert_eq!(view.progress.stage,CourseStage::Explanation);
    let reviews: Vec<_>=view.reviews.iter().filter(|r|r.expression.teach_back.is_some()).collect();
    assert_eq!(reviews.len(),1);
    assert_eq!(reviews[0].expression.response,submitted);
    let delivered_card = json!(serde_json::from_value::<runtime::teach_back::TeachBackComparison>(card).unwrap());
    assert_eq!(reviews[0].comparison,Some(delivered_card.clone()));
    assert!(!reviews[0].displayed);
    assert!(store.coach_job(&round).unwrap().is_none());
    let evidence=store.evidence(&format!("interpretation:{round}")).unwrap();
    assert_eq!(evidence.target.as_ref().unwrap().expected_performance, "Total time");
    assert!(evidence.support_delivery_refs.contains(&format!("{run}:explanation:delivery")));
    assert!(evidence.support_delivery_refs.contains(&earlier_answer));
    let delivery=store.teaching_event(&round).unwrap();
    assert_eq!(delivery.payload["course"]["lesson_id"],lesson.summary.id);
    assert_eq!(delivery.payload["course"]["round_index"],0);
    let display=json!({"chat_session_id":first.session_id,"turn_id":first.turn_id,"round_id":round});
    post_ok(&mut state,"/tutor/display",display.clone());
    assert!(store.course_view(&run).unwrap().reviews.last().unwrap().displayed);
    let practice = post_ok(&mut state,"/tutor/closed-book",json!({"chat_session_id":first.session_id,"action":"start",
        "operation_id":"begin-variant","revision":0,"course_run_id":run}));
    assert_eq!(practice["support_delivery_refs"],json!([round]));
    assert!(store.course_view(&run).unwrap().reviews.last().unwrap().comparison.is_none());
    post_ok(&mut state,"/tutor/closed-book",json!({"chat_session_id":first.session_id,"action":"exit",
        "operation_id":"leave-variant","revision":practice["revision"]}));
    assert_eq!(store.course_view(&run).unwrap().reviews.last().unwrap().comparison.as_ref(),Some(&delivered_card));
    let advance=mutation("continue",Some(&run),view.progress.revision,json!({"kind":"advance"}));
    let next=store.mutate_course(&advance,None,1,"now",None).unwrap();
    assert_eq!(next.round_history[0].reflection.as_deref(),Some(submitted));
}
