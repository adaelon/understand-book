use super::*;

// Persist the same proposal/delivery facts produced by the Resident, rather
// than injecting a course response or claiming that a completion is a grade.
fn closed_book_result(store: &mut LearningStore, view: &CourseLessonView, id: &str, response: &str) -> String {
    use crate::closed_book::{ClosedBookCourseOrigin, ClosedBookPractice};
    let run=store.course_run(&view.progress.run_id).unwrap();
    let origin=ClosedBookCourseOrigin::from_run(&run);
    let mut practice=ClosedBookPractice { id:id.into(),owner_id:view.progress.run_id.clone(),course:true,
        chat_session_id:format!("chat:{id}"),source_id:run.lesson.summary.source.publication.book_id.clone(),
        source_revision:run.lesson.summary.source.source_revision.clone(),goal_revision:Some(run.progress.goal_revision),
        course_origin:Some(origin.clone()),answer_origin:None,result_ref:None,revision:0,status:"active".into(),can_resume:true,
        target:origin.question.clone(),criteria:"解释原任务要求的关系与条件".into(),attempt:1,
        assisted:false,needs_retry:false,visible_turn_ids:vec![],draft:String::new(),completion_reason:String::new(),learner_quote:String::new() };
    practice=store.save_closed_book(practice,&format!("start:{id}"),"now").unwrap();
    for (suffix,learner,kind,text) in [("question","开始练习","question",origin.question.as_str()),("answer",response,"complete","你独立说明了当前练习要求的关系。")] {
        let turn=format!("{id}:{suffix}");
        store.append_teaching(&TeachingEvent{event_id:format!("closed-book-proposal:{turn}"),binding:practice.binding(&turn),
            kind:TeachingFact::ClosedBookProposed,causal_refs:vec![],payload:json!({"practice_id":practice.id,"revision":practice.revision,
                "response":learner,"decision":{"action":kind,"text":text,"provides_help":false}}),occurred_at:"now".into()}).unwrap();
        practice.visible_turn_ids.push(turn.clone());
        if kind=="complete" { practice.status="complete".into();practice.completion_reason="本次独立说明了闭卷问题".into();
            practice.learner_quote=response.into();practice.result_ref=Some(format!("closed-book:delivery:{turn}")); }
        practice=store.save_closed_book(practice,&format!("delivery:{turn}"),"now").unwrap();
    }
    practice.result_ref.unwrap()
}

#[test]
fn closed_book_course_result_uses_original_decision_coach_and_still_requires_transfer() {
    let root=tempfile::tempdir().unwrap();let file=root.path().join("learning.db");
    let mut store=LearningStore::open(&file,false).unwrap();
    let mut v=decision_start(&mut store);
    let previous_answer=v.answer.clone();
    let response="总时间同时不小于读取与计算下限，先比较较大的那一项；这只是理想下界，还需要实测。";
    let result=closed_book_result(&mut store,&v,"decision-attempt",response);
    cannot_advance(&mut store,&v);
    let mutation=CourseMutation{operation_id:"reuse-decision".into(),run_id:Some(v.progress.run_id.clone()),expected_revision:v.progress.revision,
        release_version:1,action:CourseAction::UseClosedBook{result_ref:result.clone()}};
    v=store.mutate_course(&mutation,None,1,"now",None).unwrap();
    assert_eq!(v.answer,previous_answer,"a dialogue never invents a multiple-choice response");
    let expression=v.expressions.last().unwrap();
    assert_eq!(expression.response,response);assert_eq!(expression.closed_book_result_ref.as_ref(),Some(&result));
    let job=store.coach_job(&expression.job_ref).unwrap().unwrap();
    assert_eq!(job.input.context["course_response"]["question"],decision_sample().task.question);
    assert!(job.input.context["course_response"]["choice"].is_null());
    assert_eq!(job.input.context["course_response"]["closed_book_result"]["exchanges"].as_array().unwrap().len(),2);
    assert!(job.input.context["observation_facts"][0]["prior_help"].as_array().unwrap().is_empty());
    cannot_advance(&mut store,&v);
    finish_decision(&mut store,&v,true);cannot_advance(&mut store,&v);
    let job=v.decision.as_ref().unwrap().latest_job.clone().unwrap();
    v=act(&mut store,&v,"read-reuse",CourseAction::ReadFeedback{job_ref:job});
    v=act(&mut store,&v,"transfer-after-reuse",CourseAction::Advance);
    assert_eq!(v.decision.as_ref().unwrap().phase,CourseDecisionPhase::Transfer);
    assert_eq!(v.progress.status,CourseStatus::Active);cannot_advance(&mut store,&v);
    let count=v.expressions.len();
    v=act(&mut store,&v,"reuse-result-again",CourseAction::UseClosedBook{result_ref:result});
    assert_eq!(v.expressions.len(),count);assert!(v.decision.as_ref().unwrap().latest_job.is_none());
    // A lost HTTP receipt still replays, without replacing the newer transfer.
    assert!(store.mutate_course(&mutation,None,1,"now",None).is_ok());
    drop(store);let store=LearningStore::open(&file,false).unwrap();
    assert_eq!(store.course_view(&v.progress.run_id).unwrap().decision.unwrap().phase,CourseDecisionPhase::Transfer);
}

#[test]
fn closed_book_reflection_rejection_can_continue_until_original_task_is_accepted() {
    let root=tempfile::tempdir().unwrap();let file=root.path().join("learning.db");
    let mut store=LearningStore::open(&file,false).unwrap();
    let course:InquiryCourse=serde_json::from_str(include_str!("../../../docs/examples/inquiry-course/ai-infra-resource-bounds.course.json")).unwrap();
    let lesson=&course.lessons[0];
    let mut v=store.mutate_course(&CourseMutation{operation_id:"start".into(),run_id:None,expected_revision:0,release_version:1,
        action:CourseAction::Start{lesson_id:lesson.summary.id.clone()}},Some(("release",1,lesson)),1,"now",None).unwrap();
    v=act(&mut store,&v,"begin",CourseAction::Advance);
    v=act(&mut store,&v,"predict",CourseAction::Predict{response:CourseResponse{choice:lesson.prediction.options[0].id.clone(),reason:String::new()}});
    for (id,preset) in [("base",CoursePreset::Baseline),("compute",CoursePreset::DoubleCompute),("bandwidth",CoursePreset::DoubleBandwidth)] {v=act(&mut store,&v,id,CourseAction::Observe{preset});}
    v=act(&mut store,&v,"explore",CourseAction::Explore{state:lesson.lab.clone().unwrap()});
    v=act(&mut store,&v,"explain",CourseAction::Advance);
    let result=closed_book_result(&mut store,&v,"reflection-attempt","总时间不能小于任一必要时间。");
    v=act(&mut store,&v,"reuse",CourseAction::UseClosedBook{result_ref:result.clone()});
    let imported=store.teaching_event(&v.expressions.last().unwrap().fact_ref).unwrap();
    assert!(store.prior_chat_help_for_expression(&imported).unwrap().is_empty(),
        "the old course explanation is not help during the imported independent attempt");
    let first=v.expressions.last().unwrap().job_ref.clone();
    let finish=|store:&mut LearningStore,id:&str,accepted:bool| {
        let job=store.claim_coach_job(id,"now").unwrap().unwrap();
        store.finish_coach_job(id,job.attempt,json!({"course_feedback":{"response_quote":job.input.context["course_response"]["response"],
            "acknowledgement":"已经说明时间下限。","guidance":"把这个关系与保存的实验比较起来。","follow_up":null,
            "course_reuse":{"accepted":accepted,"reason":if accepted{"已解释实验怎样修正预测"}else{"还需说明实验怎样支持或修正预测"}}},
            "next_step":{"participation":{"choice":if accepted{"conclude"}else{"support"}}}}),"now").unwrap();
    };
    finish(&mut store,&first,false);v=act(&mut store,&v,"read-rejection",CourseAction::ReadFeedback{job_ref:first.clone()});
    assert!(store.course_run(&v.progress.run_id).unwrap().reflection.is_none());cannot_advance(&mut store,&v);
    v=act(&mut store,&v,"ask-missing",CourseAction::FeedbackReply{job_ref:first,intent:CourseReplyIntent::Explain,response:"请解释怎样比较我的预测和实验".into()});
    let help=v.expressions.last().unwrap().job_ref.clone();
    assert_eq!(v.expressions.last().unwrap().closed_book_result_ref.as_ref(),Some(&result));
    finish(&mut store,&help,false);v=act(&mut store,&v,"read-help",CourseAction::ReadFeedback{job_ref:help.clone()});
    v=act(&mut store,&v,"answer-original",CourseAction::FeedbackReply{job_ref:help,intent:CourseReplyIntent::Answer,
        response:"加快计算之后读取仍较慢，下界不再降低。这修正了我以为算力总能降低下界的预测。".into()});
    assert!(store.course_run(&v.progress.run_id).unwrap().reflection.is_none());cannot_advance(&mut store,&v);
    let last=v.expressions.last().unwrap().job_ref.clone();
    let job=store.coach_job(&last).unwrap().unwrap();
    assert!(job.input.context["course_response"]["closed_book_result"].is_object());
    assert!(!job.input.context["observation_facts"][0]["prior_help"].as_array().unwrap().is_empty());
    let reply=store.teaching_event(&v.expressions.last().unwrap().fact_ref).unwrap();
    assert!(!store.prior_chat_help_for_expression(&reply).unwrap().is_empty(),
        "a later reply with the same result reference still retains newly received course help");
    finish(&mut store,&last,true);cannot_advance(&mut store,&v);
    drop(store);let mut store=LearningStore::open(&file,false).unwrap();
    v=act(&mut store,&v,"read-accepted",CourseAction::ReadFeedback{job_ref:last});
    assert!(store.course_run(&v.progress.run_id).unwrap().reflection.unwrap().contains("修正"));
    v=act(&mut store,&v,"next-round",CourseAction::Advance);assert_eq!(v.round_index,1);
}

#[test]
fn closed_book_result_preserves_original_choice_task_and_rejects_wrong_owner_or_proposal() {
    let root=tempfile::tempdir().unwrap();let mut store=LearningStore::open(&root.path().join("learning.db"),false).unwrap();
    let course=sample();let lesson=&course.lessons[0];
    let mut v=store.mutate_course(&CourseMutation{operation_id:"ordinary".into(),run_id:None,expected_revision:0,release_version:1,
        action:CourseAction::Start{lesson_id:lesson.summary.id.clone()}},Some(("release",1,lesson)),1,"now",None).unwrap();
    v=act(&mut store,&v,"begin",CourseAction::Advance);
    let result=closed_book_result(&mut store,&v,"choice-attempt","需要比较两项时间。");
    v=act(&mut store,&v,"reuse-choice",CourseAction::UseClosedBook{result_ref:result.clone()});
    assert_eq!(v.progress.stage,CourseStage::Prediction);assert!(v.prediction_response.is_none());assert!(v.expressions.is_empty());
    let attempt=|v:&CourseLessonView,result:&str,id:&str|CourseMutation{operation_id:id.into(),run_id:Some(v.progress.run_id.clone()),expected_revision:v.progress.revision,
        release_version:1,action:CourseAction::UseClosedBook{result_ref:result.into()}};
    assert!(store.mutate_course(&attempt(&v,"closed-book-proposal:choice-attempt:answer","proposal"),None,1,"now",None).is_err());
    let other=decision_start(&mut store);
    assert!(store.mutate_course(&attempt(&other,&result,"wrong-owner"),None,1,"now",None).is_err());
}

#[test]
fn mathematics_lessons_persist_observations_and_keep_independent_answers_separate() {
    let artifact: Value = serde_json::from_str(include_str!("../../../docs/examples/inquiry-course/optimization-foundations.course.json")).unwrap();
    let course: InquiryCourse = serde_json::from_value(artifact["course"].clone()).unwrap();
    course.validate().unwrap();
    assert_eq!(course.lessons.len(), 3);
    for lesson in &course.lessons {
        let root=tempfile::tempdir().unwrap();let file=root.path().join("learning.db");
        let mut store=LearningStore::open(&file,false).unwrap();
        let mut view=store.mutate_course(&CourseMutation{operation_id:"start".into(),run_id:None,expected_revision:0,release_version:1,
            action:CourseAction::Start{lesson_id:lesson.summary.id.clone()}},Some(("local",1,lesson)),1,"now",None).unwrap();
        assert!(view.solution.is_none() && view.task.is_none() && view.experiment.is_none());
        view=act(&mut store,&view,"begin",CourseAction::Advance);
        for round in 0..2 {
            view=act(&mut store,&view,&format!("predict-{round}"),CourseAction::Predict{response:CourseResponse{choice:"".into(),reason:"我先作出一个判断，并在图中验证。".into()}});
            let state=match view.experiment.clone().unwrap(){CourseExperiment::Inquiry{visualization:Some(state),..}=>state,_=>panic!("missing mathematics experiment")};
            assert!(store.mutate_course(&CourseMutation{operation_id:format!("missing-state-{round}"),run_id:Some(view.progress.run_id.clone()),expected_revision:view.progress.revision,release_version:1,
                action:CourseAction::Investigate{response:"观察".into(),visualization:None}},None,1,"now",None).is_err());
            view=act(&mut store,&view,&format!("observe-{round}"),CourseAction::Investigate{response:"这份参数和我的观察一同保存。".into(),visualization:Some(state.clone())});
            view=act(&mut store,&view,&format!("pause-{round}"),CourseAction::Pause);
            drop(store);store=LearningStore::open(&file,false).unwrap();
            view=store.course_view(&view.progress.run_id).unwrap();assert_eq!(view.math_exploration,Some(state.clone()));
            view=act(&mut store,&view,&format!("resume-{round}"),CourseAction::Resume);
            view=act(&mut store,&view,&format!("explain-{round}"),CourseAction::Advance);
            view=act(&mut store,&view,&format!("reflect-{round}"),CourseAction::Reflect{response:"我用公式解释观察并核对适用条件。".into()});
            let job=store.coach_job(&view.expressions.last().unwrap().job_ref).unwrap().unwrap();
            assert_eq!(job.input.context["course_response"]["math_exploration"],json!(state));
            assert!(job.input.context["course_response"]["task_criteria"].is_null());
            view=act(&mut store,&view,&format!("advance-{round}"),CourseAction::Advance);
            assert_eq!(view.round_history[round].math_exploration,Some(state));
            if round==0 { assert!(view.math_exploration.is_none()); }
        }
        assert_eq!(view.progress.stage,CourseStage::Task);assert!(view.solution.is_none());
        view=act(&mut store,&view,"answer",CourseAction::Answer{response:CourseResponse{choice:"".into(),reason:"这是独立首答，包含我的计算与证明。".into()}});
        let job=store.coach_job(&view.expressions.last().unwrap().job_ref).unwrap().unwrap();
        assert_eq!(job.input.context["course_response"]["task_criteria"].as_array().unwrap().len(),4);
        assert!(view.solution.is_some());
        assert_eq!(view.answer.as_ref().unwrap().reason,"这是独立首答，包含我的计算与证明。");
        assert!(store.mutate_course(&CourseMutation{operation_id:"edit-lab-during-test".into(),run_id:Some(view.progress.run_id.clone()),expected_revision:view.progress.revision,release_version:1,
            action:CourseAction::Investigate{response:"改写实验".into(),visualization:view.math_exploration.clone()}},None,1,"now",None).is_err());
    }
}

#[test]
fn course_feedback_counts_its_side_chat_as_help_without_importing_another_course() {
    let root=tempfile::tempdir().unwrap();
    let mut store=LearningStore::open(&root.path().join("learning.db"),false).unwrap().with_workspace(Some("workspace"));
    let course=sample(); let lesson=&course.lessons[0];
    let v=store.mutate_course(&CourseMutation{operation_id:"shared-tutor".into(),run_id:None,expected_revision:0,release_version:1,action:CourseAction::Start{lesson_id:lesson.summary.id.clone()}},Some(("release",1,lesson)),1,"now",None).unwrap();
    // Saved ordinary-observation conversations remain valid help records too.
    for (turn,session,run,text) in [
        ("course-chat",v.progress.run_id.clone(),v.progress.run_id.clone(),"先分别比较两项时间"),
        ("saved-chat",format!("observation:{}",lesson.summary.source.publication.book_id),v.progress.run_id.clone(),"旧对话中已经给过的例子"),
        ("other-chat","course:other".into(),"course:other".into(),"另一课的帮助"),
    ] {
        let binding=TeachingBinding{tutor_session_id:session.clone(),session_revision:0,control_revision:0,
            source_id:lesson.summary.source.publication.book_id.clone(),source_revision:lesson.summary.source.source_revision.clone(),map_revision:None,
            chat_session_id:"chat".into(),turn_id:turn.into()};
        store.append_teaching(&TeachingEvent{event_id:format!("binding:{turn}"),binding:binding.clone(),kind:TeachingFact::TurnBound,causal_refs:vec![],
            payload:json!({"inquiry_course":{"lesson":{"progress":{"run_id":run}}}}),occurred_at:"now".into()}).unwrap();
        store.append_teaching(&TeachingEvent{event_id:format!("usage:{session}:assistant:{turn}"),binding,kind:TeachingFact::UsageObserved,causal_refs:vec![],
            payload:json!({"usage":{"kind":"assistant_delivery"},"answer":text}),occurred_at:"now".into()}).unwrap();
    }
    let v=act(&mut store,&v,"after-chat",CourseAction::AskQuestion{response:"为什么要分别比较？".into()});
    let job=store.coach_job(&v.expressions.last().unwrap().job_ref).unwrap().unwrap();
    let help=job.input.context["observation_facts"][0]["prior_help"].to_string();
    assert!(help.contains("先分别比较两项时间") && help.contains("旧对话中已经给过的例子"));
    assert!(!help.contains("另一课的帮助"));
    assert_eq!(job.input.context["teaching_context"]["status"],"course");
    assert_eq!(job.input.binding.tutor_session_id,v.progress.run_id);
    assert_eq!(v.progress.stage,CourseStage::Introduction);
}

#[test]
fn short_support_returns_to_the_same_question_and_only_opened_help_is_attributed() {
    let root=tempfile::tempdir().unwrap();let path=root.path().join("learning.db");
    let mut store=LearningStore::open(&path,false).unwrap();let course=sample();let lesson=&course.lessons[0];
    let mut v=store.mutate_course(&CourseMutation{operation_id:"support-start".into(),run_id:None,expected_revision:0,release_version:1,action:CourseAction::Start{lesson_id:lesson.summary.id.clone()}},Some(("release",1,lesson)),1,"now",None).unwrap();
    v=act(&mut store,&v,"question",CourseAction::AskQuestion{response:"为什么同时开始还要等？".into()});
    assert_eq!(v.progress.stage,CourseStage::Introduction);
    let question=v.expressions.last().unwrap().clone();
    let job=store.claim_coach_job(&question.job_ref,"now").unwrap().unwrap();
    store.finish_coach_job(&question.job_ref,job.attempt,json!({"course_feedback":{"response_quote":question.response,"acknowledgement":"先分开开始与完成。","guidance":"可以先补看同时进行的含义。","follow_up":"两个任务都结束需要多久？","support_branch":{"focus":"同时开始与全部完成","explanation":"同时开始不等于同时完成。","example":"洗衣需 8 分钟，烧水需 5 分钟，全部完成要 8 分钟。","return_prompt":"回到原题，读取和计算都结束需要多久？"},"question_guide":{"suggested_question":"两个任务同时开始，全部完成的时间由谁决定？","why":"明确了要比较的完成事件。","try_next":"画出两条从零开始、长度不同的时间线。"}},"next_step":{"participation":{"choice":"conclude"}}}),"now").unwrap();
    v=act(&mut store,&v,"read",CourseAction::ReadFeedback{job_ref:question.job_ref.clone()});
    assert_eq!(store.course_view(&v.progress.run_id).unwrap().reviews[0].feedback.as_ref().unwrap().acknowledgement,"");
    v=act(&mut store,&v,"other-question",CourseAction::AskQuestion{response:"我该比较哪个完成时刻？".into()});
    let other=v.expressions.last().unwrap();
    let before=store.coach_job(&other.job_ref).unwrap().unwrap();
    let help=before.input.context["observation_facts"][0]["prior_help"].to_string();
    assert!(!help.contains("洗衣"),"unopened branch must not count as help");
    assert!(!help.contains("先分开开始与完成"),"omitted question acknowledgement must not count as delivered help");
    assert!(!help.contains("participation"),"private next-step planning is not delivered teaching content");
    v=act(&mut store,&v,"open-support",CourseAction::OpenSupport{job_ref:question.job_ref.clone()});
    let visit=v.support_visit.clone().unwrap();
    assert_eq!(visit.original_response,question.response);assert_eq!(visit.job_ref,question.job_ref);
    cannot_advance(&mut store,&v);
    v=act(&mut store,&v,"pause-support",CourseAction::Pause);
    drop(store);let mut store=LearningStore::open(&path,false).unwrap();
    v=store.course_view(&v.progress.run_id).unwrap();assert_eq!(v.support_visit,Some(visit));
    v=act(&mut store,&v,"resume-support",CourseAction::Resume);
    v=act(&mut store,&v,"return",CourseAction::ReturnFromSupport);
    assert!(v.support_visit.is_none());assert_eq!(v.progress.stage,CourseStage::Introduction);
    v=act(&mut store,&v,"answer-original",CourseAction::FeedbackReply{job_ref:question.job_ref.clone(),response:"全部完成要等较慢的一项".into(),intent:CourseReplyIntent::Answer});
    let reply=v.expressions.last().unwrap();assert_eq!(reply.reply_to,Some(question.job_ref));
    let after=store.coach_job(&reply.job_ref).unwrap().unwrap();
    assert!(after.input.context["observation_facts"][0]["prior_help"].to_string().contains("洗衣"));
    assert_eq!(after.input.context["course_response"]["reply_to"]["feedback"]["acknowledgement"],"");
    assert!(after.input.context["course_response"]["reply_to"]["feedback"].get("support_branch").is_none());
    assert_eq!(v.expressions[0].response,"为什么同时开始还要等？");
    v=act(&mut store,&v,"resolve",CourseAction::ResolveQuestion{fact_ref:question.fact_ref.clone(),resolved:true});
    assert_eq!(store.course_view(&v.progress.run_id).unwrap().resolved_questions,vec![question.fact_ref]);
    assert!(store.course_run(&v.progress.run_id).unwrap().assessment.is_none());
}

#[test]
fn inquiry_activities_keep_written_evidence_and_open_responses_without_resource_parameters() {
    for kind in [CourseInquiryKind::Reading, CourseInquiryKind::Reasoning, CourseInquiryKind::Practice] {
        let mut course=sample();let lesson=&mut course.lessons[0];
        lesson.experiment=CourseExperiment::Inquiry{activity:kind.clone(),instruction:"核对材料中的条件".into(),material:"对照定义与两个案例。".into(),response_prompt:"写出依据和推理。".into(),visualization:None};
        lesson.lab=None;lesson.prediction.options.clear();lesson.task.options.clear();
        lesson.assessment.rule=ScoringRule::Open{items:vec![crate::assessment::RubricItem{id:"reason".into(),criterion:"引用必要条件并解释结论".into(),retry_prompt:None,target:None,note_refs:vec![],source_lids:lesson.assessment.source_lids.clone(),source_ref_ids:vec![]}]};
        course.validate().unwrap();let lesson=&course.lessons[0];
        let root=tempfile::tempdir().unwrap();let path=root.path().join("learning.db");
        let mut store=LearningStore::open(&path,false).unwrap();
        let mut v=store.mutate_course(&CourseMutation{operation_id:"start".into(),run_id:None,expected_revision:0,release_version:1,action:CourseAction::Start{lesson_id:lesson.summary.id.clone()}},Some(("release",1,lesson)),1,"now",None).unwrap();
        assert_eq!(v.inquiry_kind,Some(kind));assert!(v.experiment.is_none());
        v=act(&mut store,&v,"begin",CourseAction::Advance);
        v=act(&mut store,&v,"predict",CourseAction::Predict{response:CourseResponse{choice:String::new(),reason:"我猜条件会影响结论".into()}});
        cannot_advance(&mut store,&v);
        v=act(&mut store,&v,"write",CourseAction::Investigate{response:"案例二不满足定义中的条件，所以不能直接使用结论。".into(),visualization:None});
        let job=v.expressions.last().unwrap();assert_eq!(job.activity,"investigation");
        assert!(store.coach_job(&job.job_ref).unwrap().is_some());
        drop(store);let mut store=LearningStore::open(&path,false).unwrap();
        v=store.course_view(&v.progress.run_id).unwrap();assert!(v.activity_response.as_ref().unwrap().contains("案例二"));
        v=act(&mut store,&v,"explain",CourseAction::Advance);v=act(&mut store,&v,"task",CourseAction::Advance);
        assert!(v.round_history[0].activity_response.is_some());
        v=act(&mut store,&v,"answer",CourseAction::Answer{response:CourseResponse{choice:String::new(),reason:"先核对条件，再依据定义解释。".into()}});
        assert_eq!(v.assessment_status.as_deref(),Some("unassessed"));assert!(v.solution.is_some());
        v=act(&mut store,&v,"finish",CourseAction::Advance);assert_eq!(v.progress.status,CourseStatus::Completed);
    }
}

#[test]
fn batch_course_preserves_shared_work_and_requires_its_own_observations() {
    let course:InquiryCourse=serde_json::from_str(include_str!("../../../docs/examples/inquiry-course/ai-infra-resource-objects.unit.course.json")).unwrap();
    course.validate().unwrap();assert_eq!(course.lessons.len(),2);
    let lesson=&course.lessons[1];
    let root=tempfile::tempdir().unwrap();let path=root.path().join("learning.db");
    let mut store=LearningStore::open(&path,false).unwrap().with_workspace(Some("batch"));
    let mut v=store.mutate_course(&CourseMutation{operation_id:"batch-start".into(),run_id:None,expected_revision:0,release_version:1,action:CourseAction::Start{lesson_id:lesson.summary.id.clone()}},Some(("unit",1,lesson)),1,"now",None).unwrap();
    assert!(v.experiment.is_none());
    v=act(&mut store,&v,"batch-next",CourseAction::Advance);
    for round in 0..2 {
        v=act(&mut store,&v,&format!("predict-{round}"),CourseAction::Predict{response:CourseResponse{choice:v.prediction.options[0].id.clone(),reason:String::new()}});
        assert!(store.mutate_course(&CourseMutation{operation_id:format!("wrong-presets-{round}"),run_id:Some(v.progress.run_id.clone()),expected_revision:v.progress.revision,release_version:1,action:CourseAction::Observe{preset:CoursePreset::DoubleCompute}},None,1,"now",None).is_err());
        for (i,preset) in [CoursePreset::BatchOne,CoursePreset::BatchFour,CoursePreset::BatchEight].into_iter().enumerate(){v=act(&mut store,&v,&format!("batch-{round}-{i}"),CourseAction::Observe{preset});}
        let mut state=v.lab.clone().unwrap();state.work_gflops=state.work_gflops/state.batch_size.unwrap() as f64*8.0;state.batch_size=Some(8);
        v=act(&mut store,&v,&format!("save-{round}"),CourseAction::Explore{state:state.clone()});
        drop(store);store=LearningStore::open(&path,false).unwrap().with_workspace(Some("batch"));
        v=store.course_view(&v.progress.run_id).unwrap();assert_eq!(v.exploration,Some(state));
        v=act(&mut store,&v,&format!("explain-{round}"),CourseAction::Advance);
        v=act(&mut store,&v,&format!("reflect-{round}"),CourseAction::Reflect{response:"权重读取在本批共享，计算量随请求数变化；分摊量不能当成请求等待。".into()});
        v=act(&mut store,&v,&format!("advance-{round}"),CourseAction::Advance);
    }
    assert_eq!(v.progress.stage,CourseStage::Task);assert_eq!(v.lab.as_ref().unwrap().batch_size,Some(4));
    assert_eq!(v.experiment.as_ref().unwrap().batch_size(),Some(4));
    assert_eq!(v.exploration.as_ref().unwrap().work_gflops,256.0);
    let mut invalid=v.lab.clone().unwrap();invalid.batch_size=Some(0);assert!(invalid.validate().is_err());
    assert!(serde_json::to_value(course.lessons[0].lab.as_ref().unwrap()).unwrap().get("batch_size").is_none());
}

fn decision_sample() -> CourseLesson {
    let mut course:InquiryCourse=serde_json::from_str(include_str!("../../../docs/examples/inquiry-course/ai-infra-resource-bounds.decision.course.json")).unwrap();
    course.validate().unwrap();
    let mut lesson=course.lessons.remove(0);lesson.follow_up_rounds.clear();lesson
}
fn decision_start(store:&mut LearningStore) -> CourseLessonView {
    let lesson=decision_sample();
    let mut v=store.mutate_course(&CourseMutation{operation_id:"start-decision".into(),run_id:None,expected_revision:0,release_version:1,action:CourseAction::Start{lesson_id:lesson.summary.id.clone()}},Some(("decision",1,&lesson)),1,"now",None).unwrap();
    v=act(store,&v,"predict-next",CourseAction::Advance);
    v=act(store,&v,"predict",CourseAction::Predict{response:CourseResponse{choice:lesson.prediction.options[0].id.clone(),reason:String::new()}});
    for (id,preset) in [("a",CoursePreset::Baseline),("b",CoursePreset::DoubleCompute),("c",CoursePreset::DoubleBandwidth)] { v=act(store,&v,id,CourseAction::Observe{preset}); }
    v=act(store,&v,"explore",CourseAction::Explore{state:lesson.lab.unwrap()});
    v=act(store,&v,"explain",CourseAction::Advance);v=act(store,&v,"task",CourseAction::Advance);
    assert!(v.decision.is_some());assert_eq!(v.lab.as_ref().unwrap().work_gflops,1200.0);
    let mut observed=v.lab.clone().unwrap(); observed.bandwidth_tb_s=10.0;
    v=act(store,&v,"task-experiment",CourseAction::Explore{state:observed.clone()});
    v=act(store,&v,"answer",CourseAction::Answer{response:CourseResponse{choice:lesson.task.options[0].id.clone(),reason:"还是要实测啊".into()}});
    assert_eq!(v.exploration,Some(observed));v
}
fn finish_decision(store:&mut LearningStore, v:&CourseLessonView, ready:bool) {
    let id=v.decision.as_ref().unwrap().latest_job.as_ref().unwrap();
    let job=store.claim_coach_job(id,"now").unwrap().unwrap();
    let facts=job.input.context["course_teaching"]["evidence_facts"].as_array().unwrap();
    let latest=facts.last().unwrap();
    let teaching=decision_sample().decision_teaching.unwrap();
    let goals:Vec<_>=teaching.goals.iter().map(|g|json!({"goal_id":g.id,"status":if ready{"demonstrated"}else{"unconfirmed"},"fact_ref":if ready{latest[0].clone()}else{Value::Null},"quote":if ready{latest[1].clone()}else{Value::Null},"reason":"本回合用于测试推进，真实语义由运行时模型核验"})).collect();
    let guidance=json!({"course_feedback":{"response_quote":latest[1],"acknowledgement":"保留了你的认识","guidance":"一起比较两项时间。","follow_up":if ready{Value::Null}else{json!("只改变读取时，下界会怎样？")},"decision":{"goals":goals,"focus_goal":if ready{Value::Null}else{json!("resource_choice")},"ready":ready}},"next_step":{"participation":{"choice":if ready{"conclude"}else{"support"}}}});
    store.finish_coach_job(id,job.attempt,guidance,"now").unwrap();
}
#[test]
fn decision_reply_restores_the_delivered_question_purpose_after_reopening() {
    let root=tempfile::tempdir().unwrap();let path=root.path().join("learning.db");
    let mut store=LearningStore::open(&path,false).unwrap().with_workspace(Some("decision-purpose"));
    let mut v=decision_start(&mut store);
    let first=v.decision.as_ref().unwrap().latest_job.clone().unwrap();
    let job=store.claim_coach_job(&first,"now").unwrap().unwrap();
    let question="继续只加算力还会降低下界吗？";
    let goals:Vec<_>=decision_sample().decision_teaching.unwrap().goals.iter().map(|g|json!({"goal_id":g.id,"status":"unconfirmed","fact_ref":null,"quote":null,"reason":"尚无独立应用表达"})).collect();
    store.finish_coach_job(&first,job.attempt,json!({"next_step":invitation_step(question),
        "course_feedback":{"response_quote":"还是要实测啊","acknowledgement":"你保留了实测条件。","guidance":"比较两项必要时间下限。","follow_up":question,
            "decision":{"ready":false,"focus_goal":"resource_choice","goals":goals}}}),"now").unwrap();
    v=act(&mut store,&v,"read",CourseAction::ReadFeedback{job_ref:first.clone()});
    drop(store);
    let mut store=LearningStore::open(&path,false).unwrap().with_workspace(Some("decision-purpose"));
    v=act(&mut store,&v,"reply",CourseAction::DecisionReply{job_ref:Some(first.clone()),response:"请先解释为什么取较大值".into(),intent:CourseReplyIntent::Explain});
    let next=store.coach_job(v.decision.as_ref().unwrap().latest_job.as_ref().unwrap()).unwrap().unwrap();
    let original=&next.input.context["course_response"]["reply_to"];
    assert_eq!(original["job_ref"],first);assert_eq!(original["question"],question);
    assert_eq!(original["next_step"],invitation_step(question));
    assert_eq!(original["expression"]["response"],"还是要实测啊");
    assert!(!next.input.context["observation_facts"][0]["prior_help"].as_array().unwrap().is_empty());
    assert_eq!(next.input.context["course_teaching"]["intent"],"explain");
}

fn cannot_advance(store:&mut LearningStore,v:&CourseLessonView) {
    assert!(store.mutate_course(&CourseMutation{operation_id:format!("blocked-{}",v.progress.revision),run_id:Some(v.progress.run_id.clone()),expected_revision:v.progress.revision,release_version:1,action:CourseAction::Advance},None,1,"now",None).is_err());
}
#[test]
fn decision_teaching_needs_current_delivered_evidence_then_a_fresh_application() {
    let root=tempfile::tempdir().unwrap();let path=root.path().join("learning.db");
    let mut store=LearningStore::open(&path,false).unwrap().with_workspace(Some("decision"));
    let mut v=decision_start(&mut store);
    assert!(v.solution.is_none());cannot_advance(&mut store,&v);
    let first=v.decision.as_ref().unwrap().latest_job.clone().unwrap();
    finish_decision(&mut store,&v,false);
    v=act(&mut store,&v,"read-first",CourseAction::ReadFeedback{job_ref:first.clone()});
    let view=store.course_view(&v.progress.run_id).unwrap();assert!(!view.decision.as_ref().unwrap().can_continue);assert!(view.decision.as_ref().unwrap().support.is_some());
    cannot_advance(&mut store,&v);
    v=act(&mut store,&v,"reply",CourseAction::DecisionReply{job_ref:Some(first.clone()),response:"原配置12与8，升算力后6与8，变工作量后两项12，需要同时满足约束并实测。".into(),intent:CourseReplyIntent::Answer});
    let current=v.decision.as_ref().unwrap().latest_job.clone().unwrap();
    assert_ne!(first,current);cannot_advance(&mut store,&v);
    assert!(store.mutate_course(&CourseMutation{operation_id:"stale-reply".into(),run_id:Some(v.progress.run_id.clone()),expected_revision:v.progress.revision,release_version:1,action:CourseAction::DecisionReply{job_ref:Some(first),response:"旧问题的回答".into(),intent:CourseReplyIntent::Answer}},None,1,"now",None).is_err());
    finish_decision(&mut store,&v,true);cannot_advance(&mut store,&v);
    v=act(&mut store,&v,"read-current",CourseAction::ReadFeedback{job_ref:current});
    assert!(store.course_view(&v.progress.run_id).unwrap().decision.unwrap().can_continue);
    v=act(&mut store,&v,"transfer",CourseAction::Advance);
    assert_eq!(v.progress.status,CourseStatus::Active);assert_eq!(v.decision.as_ref().unwrap().phase,CourseDecisionPhase::Transfer);cannot_advance(&mut store,&v);
    drop(store);let mut store=LearningStore::open(&path,false).unwrap().with_workspace(Some("decision"));
    v=store.course_view(&v.progress.run_id).unwrap();assert!(v.decision.as_ref().unwrap().question.contains("900"));assert_eq!(v.lab.as_ref().unwrap().read_gb,54.0);
    v=act(&mut store,&v,"transfer-answer",CourseAction::DecisionReply{job_ref:None,response:"6与9，先升带宽变成6与4.5；变化后12与8，只需提升算力到至少225，满足下界后仍需同条件实测。".into(),intent:CourseReplyIntent::Answer});
    let job=v.decision.as_ref().unwrap().latest_job.clone().unwrap();
    assert_eq!(store.coach_job(&job).unwrap().unwrap().input.context["course_teaching"]["evidence_facts"].as_array().unwrap().len(),1);
    finish_decision(&mut store,&v,true);v=act(&mut store,&v,"read-transfer",CourseAction::ReadFeedback{job_ref:job});
    v=act(&mut store,&v,"complete",CourseAction::Advance);assert_eq!(v.progress.status,CourseStatus::Completed);
}
#[test]
fn decision_help_and_pause_never_count_as_successful_application() {
    let root=tempfile::tempdir().unwrap();let mut store=LearningStore::open(&root.path().join("learning.db"),false).unwrap().with_workspace(Some("decision"));
    let mut v=decision_start(&mut store);finish_decision(&mut store,&v,false);
    let id=v.decision.as_ref().unwrap().latest_job.clone().unwrap();v=act(&mut store,&v,"read",CourseAction::ReadFeedback{job_ref:id.clone()});
    v=act(&mut store,&v,"help",CourseAction::DecisionReply{job_ref:Some(id),response:"max是什么意思，请直接讲清楚".into(),intent:CourseReplyIntent::Explain});
    finish_decision(&mut store,&v,true);let id=v.decision.as_ref().unwrap().latest_job.clone().unwrap();
    v=act(&mut store,&v,"read-help",CourseAction::ReadFeedback{job_ref:id});cannot_advance(&mut store,&v);
    v=act(&mut store,&v,"pause",CourseAction::Pause);assert_eq!(v.progress.status,CourseStatus::Paused);
    v=act(&mut store,&v,"resume",CourseAction::Resume);cannot_advance(&mut store,&v);
}

fn sample() -> InquiryCourse {
    serde_json::from_str(include_str!(
        "../../../docs/examples/inquiry-course/ai-infra-resource-bounds.v1.course.json"
    ))
    .unwrap()
}

fn invitation_step(question:&str) -> Value {
    json!({"focus":"比较两项时间","participation":{"choice":"invite","reason":"已比较原配置，尝试只改变一项","support":"读取仍是 8 ms，计算是 5 ms","learner_inference":"只加快非瓶颈不能降低当前下界"},"activity":{"kind":"predict","learner_task":question,"response_mode":"conversation","material_use":"当前轮的两项资源下限","evidence_to_notice":"能指出读取仍较慢；在已给数值帮助下判断","adjustment":"仍不清楚就比较两项约束，不换目标"}})
}

#[test]
fn course_activity_reply_keeps_the_round_and_the_delivered_feedback_context() {
    let root=tempfile::tempdir().unwrap();
    let mut store=LearningStore::open(&root.path().join("learning.db"),false).unwrap().with_workspace(Some("round-feedback"));
    let course:InquiryCourse=serde_json::from_str(include_str!("../../../docs/examples/inquiry-course/ai-infra-resource-bounds.course.json")).unwrap();
    let lesson=&course.lessons[0];
    let mut v=store.mutate_course(&CourseMutation{operation_id:"start".into(),run_id:None,expected_revision:0,release_version:1,action:CourseAction::Start{lesson_id:lesson.summary.id.clone()}},Some(("release",1,lesson)),1,"now",None).unwrap();
    v=act(&mut store,&v,"begin",CourseAction::Advance);
    v=act(&mut store,&v,"predict",CourseAction::Predict{response:CourseResponse{choice:lesson.prediction.options[0].id.clone(),reason:String::new()}});
    for (id,preset) in [("base",CoursePreset::Baseline),("compute",CoursePreset::DoubleCompute),("bandwidth",CoursePreset::DoubleBandwidth)] {v=act(&mut store,&v,id,CourseAction::Observe{preset});}
    v=act(&mut store,&v,"explore",CourseAction::Explore{state:lesson.lab.clone().unwrap()});
    v=act(&mut store,&v,"explain",CourseAction::Advance);
    v=act(&mut store,&v,"reflection",CourseAction::Reflect{response:"这个部分没懂".into()});
    let first=v.expressions.last().unwrap().job_ref.clone();
    let finish=|store:&mut LearningStore,id:&str|{
        let job=store.claim_coach_job(id,"now").unwrap().unwrap();
        store.finish_coach_job(id,job.attempt,json!({"course_feedback":{"response_quote":job.input.context["course_response"]["response"],"acknowledgement":"先把两项时间放在一起。","guidance":"读取仍需要 8 ms，计算缩短到 5 ms 时仍需等读取。","follow_up":"继续只加算力还会降低下界吗？"},"next_step":invitation_step("继续只加算力还会降低下界吗？")}),"now").unwrap();
    };
    finish(&mut store,&first);
    let request=|v:&CourseLessonView,id:&str,job:&str,intent,response:&str|CourseMutation{operation_id:id.into(),run_id:Some(v.progress.run_id.clone()),expected_revision:v.progress.revision,release_version:1,action:CourseAction::FeedbackReply{job_ref:job.into(),intent,response:response.into()}};
    assert!(store.mutate_course(&request(&v,"hidden",&first,CourseReplyIntent::Explain,"先讲给我听"),None,1,"now",None).is_err());
    v=act(&mut store,&v,"read",CourseAction::ReadFeedback{job_ref:first.clone()});
    let help=request(&v,"help",&first,CourseReplyIntent::Explain,"先讲给我听");
    v=store.mutate_course(&help,None,1,"now",None).unwrap();
    assert_eq!(store.mutate_course(&help,None,1,"now",None).unwrap(),v);
    assert_eq!(v.progress.stage,CourseStage::Explanation);assert_eq!(v.round_index,0);
    assert_eq!(store.course_run(&v.progress.run_id).unwrap().reflection.as_deref(),Some("这个部分没懂"));
    let expression=v.expressions.last().unwrap();
    assert_eq!(expression.intent,Some(CourseReplyIntent::Explain));assert_eq!(expression.reply_to.as_deref(),Some(first.as_str()));
    let job=store.coach_job(&expression.job_ref).unwrap().unwrap();
    assert_eq!(job.input.context["course_response"]["reply_to"]["feedback"]["follow_up"],"继续只加算力还会降低下界吗？");
    let original=&job.input.context["course_response"]["reply_to"];
    assert_eq!(original["question"],"继续只加算力还会降低下界吗？");
    assert_eq!(original["next_step"],invitation_step("继续只加算力还会降低下界吗？"));
    assert_eq!(job.input.context["observation_only"],false);
    assert_eq!(job.input.context["course_response"]["intent"],"explain");
    assert!(job.input.context["observation_facts"][0]["prior_help"].as_array().unwrap().iter().any(|h|
        h["text"].as_str().and_then(|text|serde_json::from_str::<Value>(text).ok()).is_some_and(|text|text["job_ref"]==first)));
    assert!(store.mutate_course(&request(&v,"old",&first,CourseReplyIntent::Answer,"旧反馈的回答"),None,1,"now",None).is_err());
    let latest=expression.job_ref.clone();finish(&mut store,&latest);
    v=act(&mut store,&v,"read-help",CourseAction::ReadFeedback{job_ref:latest.clone()});
    v=store.mutate_course(&request(&v,"answer-help",&latest,CourseReplyIntent::Answer,"读取仍是较慢项，只加算力不会再降低下界。"),None,1,"now",None).unwrap();
    assert_eq!(store.course_run(&v.progress.run_id).unwrap().reflection.as_deref(),Some("读取仍是较慢项，只加算力不会再降低下界。"));
    let old=v.expressions.last().unwrap().job_ref.clone();
    finish(&mut store,&old);v=act(&mut store,&v,"read-answer",CourseAction::ReadFeedback{job_ref:old.clone()});
    v=act(&mut store,&v,"next-round",CourseAction::Advance);
    assert_eq!(v.round_index,1);
    let next_question=v.prediction.clone();
    v=store.mutate_course(&request(&v,"earlier-round",&old,CourseReplyIntent::Question,"为什么读取那项还是 8？"),None,1,"now",None).unwrap();
    assert_eq!(v.round_index,1);assert_eq!(v.progress.stage,CourseStage::Prediction);assert_eq!(v.prediction,next_question);
    let response=v.expressions.last().unwrap();assert_eq!(response.round_index,Some(0));
    let job=store.coach_job(&response.job_ref).unwrap().unwrap();
    assert_eq!(job.input.context["course_response"]["round_index"],0);
    assert_eq!(job.input.context["course_response"]["exploration"],json!(v.round_history[0].exploration));
}

#[test]
fn course_revisiting_experiment_keeps_the_current_task_and_answer() {
    let root = tempfile::tempdir().unwrap();
    let mut store = LearningStore::open(&root.path().join("learning.db"), false).unwrap().with_workspace(Some("workspace"));
    let course: InquiryCourse = serde_json::from_str(include_str!("../../../docs/examples/inquiry-course/ai-infra-resource-bounds.v2.course.json")).unwrap();
    let lesson = &course.lessons[0];
    let mut v = store.mutate_course(&CourseMutation { operation_id:"start".into(),run_id:None,expected_revision:0,release_version:1,
        action:CourseAction::Start { lesson_id:lesson.summary.id.clone() } },Some(("release",1,lesson)),1,"now",None).unwrap();
    v = act(&mut store,&v,"next",CourseAction::Advance);
    v = act(&mut store,&v,"predict",CourseAction::Predict { response:CourseResponse { choice:lesson.prediction.options[0].id.clone(),reason:String::new() } });
    for (id,preset) in [("base",CoursePreset::Baseline),("compute",CoursePreset::DoubleCompute),("bandwidth",CoursePreset::DoubleBandwidth)] {
        v = act(&mut store,&v,id,CourseAction::Observe { preset });
    }
    let mut state = lesson.lab.clone().unwrap();
    v = act(&mut store,&v,"save",CourseAction::Explore { state:state.clone() });
    v = act(&mut store,&v,"explain",CourseAction::Advance);
    v = act(&mut store,&v,"task",CourseAction::Advance);
    let task = v.task.clone();
    state.compute_tflops *= 2.0;
    v = act(&mut store,&v,"revisit",CourseAction::Explore { state:state.clone() });
    assert_eq!(v.progress.stage,CourseStage::Task);
    assert_eq!(v.task,task);
    assert!(v.solution.is_none() && v.answer.is_none());
    v = act(&mut store,&v,"answer",CourseAction::Answer { response:CourseResponse { choice:lesson.task.options[0].id.clone(),reason:"比较两项下界再实测".into() } });
    let answer = v.answer.clone();
    let assessment = v.assessment_status.clone();
    v = act(&mut store,&v,"revisit-feedback",CourseAction::Observe { preset:CoursePreset::Baseline });
    assert_eq!(v.progress.stage,CourseStage::Feedback);
    assert_eq!(v.answer,answer);
    assert_eq!(v.assessment_status,assessment);
    assert_eq!(store.course_view(&v.progress.run_id).unwrap().answer,answer);
}
fn act(
    store: &mut LearningStore,
    view: &CourseLessonView,
    id: &str,
    action: CourseAction,
) -> CourseLessonView {
    store
        .mutate_course(
            &CourseMutation {
                operation_id: id.into(),
                run_id: Some(view.progress.run_id.clone()),
                expected_revision: view.progress.revision,
                release_version: view.progress.release_version,
                action,
            },
            None,
            1,
            "now",
            None,
        )
        .unwrap()
}

#[test]
fn course_gates_content_preserves_support_and_reopens_frozen_progress() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("learning.db");
    let mut store = LearningStore::open(&path, false)
        .unwrap()
        .with_workspace(Some("workspace"));
    let course = sample();
    course.validate().unwrap();
    let lesson = &course.lessons[0];
    let start = CourseMutation {
        operation_id: "start".into(),
        run_id: None,
        expected_revision: 0,
        release_version: 1,
        action: CourseAction::Start {
            lesson_id: lesson.summary.id.clone(),
        },
    };
    let mut v = store
        .mutate_course(&start, Some(("release", 1, lesson)), 1, "now", None)
        .unwrap();
    assert!(v.experiment.is_none() && v.task.is_none() && v.solution.is_none());
    assert!(!serde_json::to_string(&v).unwrap().contains("read-bound"));
    assert_eq!(
        v,
        store
            .mutate_course(&start, Some(("release", 2, lesson)), 1, "later", None)
            .unwrap()
    );
    v = act(&mut store, &v, "predict-page", CourseAction::Advance);
    v = act(
        &mut store,
        &v,
        "prediction",
        CourseAction::Predict {
            response: CourseResponse {
                choice: "compute".into(),
                reason: "我以为算得快就结束得快".into(),
            },
        },
    );
    assert!(v.experiment.is_some() && v.explanation.is_none() && v.assessment_status.is_none());
    let early = CourseMutation {
        operation_id: "early".into(),
        run_id: Some(v.progress.run_id.clone()),
        expected_revision: v.progress.revision,
        release_version: 1,
        action: CourseAction::Advance,
    };
    assert!(store.mutate_course(&early, None, 1, "now", None).is_err());
    for (id, preset) in [
        ("base", CoursePreset::Baseline),
        ("compute", CoursePreset::DoubleCompute),
        ("bandwidth", CoursePreset::DoubleBandwidth),
    ] {
        v = act(&mut store, &v, id, CourseAction::Observe { preset });
    }
    v = act(&mut store, &v, "pause", CourseAction::Pause);
    drop(store);
    let mut store = LearningStore::open(&path, false)
        .unwrap()
        .with_workspace(Some("workspace"));
    assert_eq!(store.course_run(&v.progress.run_id).unwrap().view(), v);
    assert!(LearningStore::open(&path, false)
        .unwrap()
        .with_workspace(Some("other"))
        .course_run(&v.progress.run_id)
        .is_err());
    v = act(&mut store, &v, "resume", CourseAction::Resume);
    v = act(&mut store, &v, "explanation", CourseAction::Advance);
    v = act(&mut store, &v, "task", CourseAction::Advance);
    assert!(v.task.is_some() && v.solution.is_none() && v.hint.is_none());
    v = act(&mut store, &v, "hint", CourseAction::Hint);
    assert!(v.hint.is_some());
    v = act(
        &mut store,
        &v,
        "answer",
        CourseAction::Answer {
            response: CourseResponse {
                choice: "read-bound".into(),
                reason: "等待读取".into(),
            },
        },
    );
    assert_eq!(v.assessment_status.as_deref(), Some("correct"));
    assert_eq!(v.progress.status, CourseStatus::Active);
    let run = store.course_run(&v.progress.run_id).unwrap();
    let assessment = run.assessment.unwrap();
    let response = store.teaching_event(&assessment.action_ref).unwrap();
    assert_eq!(response.payload["response"], "等待读取");
    assert_eq!(response.payload["reason_assessed"], false);
    assert_eq!(response.payload["selected_option"]["text"], lesson.task.options.iter().find(|o|o.id=="read-bound").unwrap().text);
    assert!(response.payload["support_delivery_refs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r.as_str().unwrap().ends_with("hint:delivery")));
    assert_eq!(
        store.assessment(&assessment.action_ref).unwrap(),
        Some(assessment)
    );
    assert!(v.solution.is_some());
    v = act(&mut store, &v, "finish", CourseAction::Advance);
    assert_eq!(v.progress.status, CourseStatus::Completed);
    assert_eq!(v.progress.release_version, 1);
    assert_eq!(store.course_progress().unwrap().len(), 1);
}

#[test]
fn course_rejects_stale_edits_and_changed_goal_without_turning_skip_into_success() {
    let root = tempfile::tempdir().unwrap();
    let mut store = LearningStore::open(&root.path().join("learning.db"), false).unwrap();
    let course = sample();
    let lesson = &course.lessons[0];
    let mut request = CourseMutation {
        operation_id: "start".into(),
        run_id: None,
        expected_revision: 0,
        release_version: 1,
        action: CourseAction::Start {
            lesson_id: lesson.summary.id.clone(),
        },
    };
    let v = store
        .mutate_course(&request, Some(("release", 1, lesson)), 1, "now", None)
        .unwrap();
    request.run_id = Some(v.progress.run_id.clone());
    request.operation_id = "next".into();
    request.action = CourseAction::Advance;
    assert_eq!(
        store
            .mutate_course(&request, None, 1, "now", None)
            .unwrap_err()
            .error_code,
        "COURSE_REVISION_CONFLICT"
    );
    request.expected_revision = v.progress.revision;
    assert!(store.mutate_course(&request, None, 2, "now", None).is_err());
    request.action = CourseAction::Skip;
    let skipped = store.mutate_course(&request, None, 2, "now", None).unwrap();
    assert_eq!(skipped.progress.status, CourseStatus::Skipped);
    assert!(skipped.assessment_status.is_none());
    assert_eq!(skipped.progress.stage, CourseStage::Introduction);
}

#[test]
fn course_rejects_mismatched_task_criteria_and_unusable_experiments() {
    let mut course = sample();
    course.lessons[0].task.options.remove(0);
    assert!(course.validate().is_err());
    let mut course = sample();
    let CourseExperiment::ResourceBounds { bandwidth_tb_s, .. } = &mut course.lessons[0].experiment else { panic!("resource fixture") };
    *bandwidth_tb_s = 0.0;
    assert!(course.validate().is_err());
    let mut course = sample();
    course.routes[0].lesson_ids.push("absent".into());
    assert!(course.validate().is_err());
}

#[test]
fn course_multiple_investigations_gate_preserve_and_resume_each_round() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("learning.db");
    let mut store = LearningStore::open(&path, false).unwrap().with_workspace(Some("workspace"));
    let course: InquiryCourse = serde_json::from_str(include_str!("../../../docs/examples/inquiry-course/ai-infra-resource-bounds.course.json")).unwrap();
    course.validate().unwrap();
    let lesson = &course.lessons[0];
    assert_eq!(lesson.summary.estimated_minutes, 27);
    let mut v = store.mutate_course(&CourseMutation { operation_id:"start-rounds".into(),run_id:None,expected_revision:0,release_version:1,
        action:CourseAction::Start { lesson_id:lesson.summary.id.clone() } },Some(("release",1,lesson)),1,"now",None).unwrap();
    assert_eq!(v.round_titles.len(),3);
    assert!(v.round_history.is_empty());
    assert!(!serde_json::to_string(&v).unwrap().contains(&lesson.follow_up_rounds[0].prediction.question));
    v = act(&mut store,&v,"begin",CourseAction::Advance);
    for index in 0..3 {
        let round = lesson.round_content(index);
        assert_eq!(v.round_index,index as u32);
        assert_eq!(v.progress.stage,CourseStage::Prediction);
        assert_eq!(v.prediction,round.prediction);
        assert!(v.experiment.is_none() && v.explanation.is_none() && v.task.is_none());
        assert!(v.prediction_response.is_none() && v.exploration.is_none() && v.observed.is_empty());
        let before = v.round_history.clone();
        v = act(&mut store,&v,&format!("predict-{index}"),CourseAction::Predict { response:CourseResponse { choice:round.prediction.options[1].id.clone(),reason:format!("第 {index} 轮的预测") } });
        assert_eq!(v.experiment.as_ref(),Some(&round.experiment));
        for (name,preset) in [("baseline",CoursePreset::Baseline),("compute",CoursePreset::DoubleCompute),("bandwidth",CoursePreset::DoubleBandwidth)] {
            v = act(&mut store,&v,&format!("{name}-{index}"),CourseAction::Observe { preset });
        }
        v = act(&mut store,&v,&format!("save-{index}"),CourseAction::Explore { state:round.lab.unwrap() });
        v = act(&mut store,&v,&format!("explain-{index}"),CourseAction::Advance);
        assert_eq!(v.round_history,before,"new experiments must not rewrite earlier conditions");
        let early = CourseMutation {operation_id:format!("early-{index}"),run_id:Some(v.progress.run_id.clone()),expected_revision:v.progress.revision,release_version:1,action:CourseAction::Advance};
        assert!(store.mutate_course(&early,None,1,"now",None).is_err(),"a new round needs the learner's explanation");
        v = act(&mut store,&v,&format!("reflect-{index}"),CourseAction::Reflect { response:format!("第 {index} 轮解释：比较较慢项，说明模型条件。") });
        let expression = v.expressions.last().unwrap();
        assert_eq!(expression.round_index,Some(index as u32));
        assert_eq!(expression.round_title.as_ref(),Some(&v.round_titles[index]));
        let job = store.coach_job(&expression.job_ref).unwrap().unwrap();
        assert_eq!(job.input.context["course_response"]["lesson"]["prediction"]["question"],round.prediction.question);
        v = act(&mut store,&v,&format!("next-{index}"),CourseAction::Advance);
        assert_eq!(v.round_history.len(),index+1);
        assert!(v.round_history[index].reflection.as_ref().unwrap().contains(&format!("第 {index} 轮解释")));
        if index == 0 {
            v = act(&mut store,&v,"pause-next-round",CourseAction::Pause);
            drop(store);
            store = LearningStore::open(&path, false).unwrap().with_workspace(Some("workspace"));
            assert_eq!(store.course_run(&v.progress.run_id).unwrap().view(),v);
            v = act(&mut store,&v,"resume-next-round",CourseAction::Resume);
        }
    }
    assert_eq!(v.progress.stage,CourseStage::Task);
    assert_eq!(v.task,Some(lesson.task.clone()));
    assert_eq!(v.expressions.len(),6);
    assert!(v.solution.is_none());
    v = act(&mut store,&v,"final-answer",CourseAction::Answer { response:CourseResponse { choice:"read-bound".into(),reason:"1200/200=6，40/5=8；下界 8 ms，仍须实测。".into() } });
    assert_eq!(v.assessment_status.as_deref(),Some("correct"));
    v = act(&mut store,&v,"finish-rounds",CourseAction::Advance);
    assert_eq!(v.progress.status,CourseStatus::Completed);
    assert_eq!(v.round_history.len(),3);
    assert_eq!(v.expressions.len(),7);
}

#[test]
fn course_validates_follow_up_content_and_resource_arithmetic() {
    let course: InquiryCourse = serde_json::from_str(include_str!("../../../docs/examples/inquiry-course/ai-infra-resource-bounds.course.json")).unwrap();
    let mut broken = course.clone();
    broken.lessons[0].follow_up_rounds[1].lab.as_mut().unwrap().bandwidth_tb_s = 0.0;
    assert!(broken.validate().is_err());
    let mut broken = course.clone();
    broken.lessons[0].follow_up_rounds[0].prediction.options[1].id = broken.lessons[0].follow_up_rounds[0].prediction.options[0].id.clone();
    assert!(broken.validate().is_err());
    let mut broken = course.clone();
    broken.lessons[0].summary.estimated_minutes = 31;
    assert!(broken.validate().is_err());
    let lower = |f:f64,r:f64,c:f64,w:f64| (f/c).max(r/w);
    assert_eq!([lower(600.,16.,60.,2.),lower(600.,16.,75.,2.),lower(600.,16.,120.,2.),lower(600.,16.,240.,2.)],[10.,8.,8.,8.]);
    assert_eq!(lower(600.,16.,120.,4.),5.);
    assert_eq!([lower(1440.,48.,240.,4.),lower(1440.,48.,120.,8.)],[12.,12.]);
    assert_eq!(lower(1440.,48.,160.,5.4),9.);
    assert_eq!(lower(1440.,48.,200.,6.),8.);
    assert_eq!(lower(1440.,48.,240.,8.),6.);
    assert_eq!([lower(1200.,40.,200.,5.),lower(1200.,40.,100.,10.),lower(2400.,60.,200.,5.)],[8.,12.,12.]);
}
