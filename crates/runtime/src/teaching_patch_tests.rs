fn patch_factual_revision(question: &str, location: &str) -> Result<serde_json::Value, AdapterError> {
    Ok(serde_json::json!({"decision":"revise","revision":"Correct the unsupported catalog claim using the inspected source.",
        "request_resolution":{"status":"answered","question_quote":question,"reason":"The requested comparison is present; one factual claim still needs correction."},
        "findings":[{"rule_id":"R2","location":location,"evidence":"The inspected evidence does not yet establish this catalog claim.",
            "required_change":"Correct or establish this specific catalog claim."}]}))
}

fn patch_review_update(status: &str) -> Result<serde_json::Value, AdapterError> {
    let mut review=accepted_review().unwrap();
    review["issue_updates"]=serde_json::json!([{"issue_ref":"factual:1:0","status":status,
        "reason":if status=="resolved" {"The current complete text and inspected source now establish the catalog claim."}
            else {"The changed wording still lacks the source evidence required for this catalog claim."}}]);
    Ok(review)
}

fn active_teaching_revision_draft(messages: &[Message]) -> serde_json::Value {
    let snapshots:Vec<_>=messages.iter().filter_map(|message|message.content.as_deref())
        .filter(|text|text.starts_with("runtime_state_snapshot.v1\n")).collect();
    let manifest=snapshots.last().unwrap().lines().find_map(|line|line.strip_prefix("active_keys=")).unwrap();
    let active:Vec<String>=serde_json::from_str(manifest).unwrap();
    assert!(active.iter().any(|key|key=="agent.teaching_revision"),"the latest request must still activate the repair draft");
    let published=snapshots.iter().rev().find_map(|text|text.split_once("\nkey=agent.teaching_revision\n").map(|(_,body)|body)).unwrap();
    let body=published.split("\nkey=").next().unwrap();
    serde_json::from_str(body.split_once("This draft has NOT passed review: ").unwrap().1.trim()).unwrap()
}

#[test]
fn teaching_patch_revision_comparison_repairs_review_format_then_requires_the_lost_connection() {
    let b=book();
    let mut store=MemoryStore::open(tmp("teaching-patch-comparison-repair")).unwrap();
    let mut reader=Reader::new(&b,DEFAULT_RADIUS);
    let snapshot=default_profile_snapshot(&b,&store,"t0");
    let question="Explain the retained catalog and selected route.";
    let retained="The catalog retains B when this route omits it. A later route can select the same retained B without recreating it.";
    let original=format!("{retained}\n\nPRIVATE_UNSUPPORTED_DETAIL");
    let shortened="The catalog and selected route are different.";
    let mut selection=selected_body("comparison-repair",&original);
    selection["finish"]=serde_json::json!(true);
    let shorten=serde_json::json!({"operation":"revise","draft_ref":"teaching-draft:1","edits":[
        {"field":"delivery_text","paragraph":1,"new_text":shortened},
        {"field":"delivery_text","paragraph":2,"new_text":""}]});
    let restore=serde_json::json!({"operation":"revise","draft_ref":"teaching-draft:2","edits":[
        {"field":"delivery_text","paragraph":1,"new_text":retained}]});
    let adapter=ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("original","tutor.step",&selection.to_string())]),
        turn_calls(vec![call("shortened","tutor.step",&shorten.to_string())]),
        turn_calls(vec![call("restored","tutor.step",&restore.to_string())]),
    ],vec![patch_factual_revision(question,"PRIVATE_UNSUPPORTED_DETAIL"),patch_review_update("resolved"),accepted_review()]);
    let comparison=serde_json::json!({"reference_quotes":[retained],"answer_quotes":[shortened],
        "lost_relation":"The reusable card is no longer connected to later route selection.",
        "reason":"Restore how retaining B makes it available to a later route without recreating it; merely naming two different objects loses that explanation."});
    let mut invalid=accepted_review().unwrap();
    invalid["revision_comparison"]=comparison.clone();
    invalid["revision_comparison"]["answer_quotes"]=serde_json::json!([retained]);
    let mut loss=accepted_review().unwrap();
    loss["revision_comparison"]=comparison;
    let mut preserved=accepted_review().unwrap();
    preserved["revision_comparison"]=serde_json::json!({"reference_quotes":[retained],"answer_quotes":[retained],
        "lost_relation":null,"reason":"The current answer again connects retention to later reuse; the unrelated unsupported detail is absent."});
    adapter.explanation_reviews.borrow_mut().extend([Ok(invalid),Ok(loss),Ok(preserved)]);
    let mut context=RunContext::new(new_session(),OuterConfig {max_turns:Some(16),..Default::default()},adapter.model_runtime_profile());
    context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:vec![]};
    let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),question,"t0").unwrap();
    assert!(!outcome.incomplete,"{outcome:?}");
    assert_eq!(outcome.answer.as_deref(),Some(format!("{retained}\n\n").as_str()),"deleting a paragraph preserves the surrounding separators");
    assert_eq!(adapter.inner.0.seen_plans.borrow().len(),3,"review format repair must not ask the author to rewrite");
    assert_eq!(port.reviews.len(),3);
    let rejected=&port.reviews[1]["independent_reviews"]["explanation"];
    assert!(rejected["format_repair"]["validation_error"].as_str().unwrap().contains("answer_quotes"));
    assert_eq!(rejected["raw"]["decision"],"accept");
    assert_eq!(rejected["review"]["decision"],"revise");
    assert!(rejected["review"]["revision"].as_str().unwrap().contains("reusable card"));
    assert_eq!(port.reviews[1]["review"]["open_issues"][0]["branch"],"explanation");
    assert_eq!(port.reviews[2]["review"]["open_issues"],serde_json::json!([]));
    let explanations=adapter.seen_explanation_reviews.borrow();
    assert_eq!(explanations.len(),3);
    assert_eq!(explanations[0]["current_delivery"],format!("{shortened}\n\n"));
    assert_eq!(explanations[1]["current_delivery"],format!("{shortened}\n\n"));
    for packet in explanations.iter() { assert_eq!(packet["revision_reference"]["text"],original); }
    assert_eq!(explanations[0]["revision_reference"],explanations[1]["revision_reference"],"format repair keeps the exact comparison input");
    let requests=adapter.seen_review_requests.borrow();
    let explanation_requests:Vec<_>=requests.iter().filter(|request|request.system.contains("Teaching explanation acceptance review.")).collect();
    assert_eq!(explanation_requests[0].user,explanation_requests[1].user);
    assert!(explanation_requests[1].followup_user.as_deref().unwrap().starts_with("review_format_repair:\n"));
}

#[test]
fn teaching_patch_long_feedback_survives_receipt_budget_and_failed_review_retry() {
    let b=book_leaves(2);
    let mut store=MemoryStore::open(tmp("teaching-patch-long-feedback-retry")).unwrap();
    let mut reader=Reader::new(&b,DEFAULT_RADIUS);
    let snapshot=default_profile_snapshot(&b,&store,"t0");
    let question="Read 1.1 and 1.2 and explain the retained catalog and selected route.";
    let answer="The catalog retains B when the current route omits it, allowing a later route to select the same card.";
    let mut selection=selected_body("long-feedback",answer);
    selection["finish"]=serde_json::json!(true);
    let required_changes:Vec<_>=["retention and selection", "later reuse", "ordering of route changes", "removal from a route versus the catalog"]
        .into_iter().enumerate().map(|(index, relation)|format!(
            "REPAIR_SCOPE_{index}: Establish {relation} from the complete relevant passage before resubmitting. {}",
            "Read the stated object, action, condition and result together; preserve the explanation when those source conditions support it, and correct only the unsupported dependency. ".repeat(9)))
        .collect();
    let findings:Vec<_>=required_changes.iter().enumerate().map(|(index, change)|serde_json::json!({
        "rule_id":"R3","location":format!("Source-dependent relation {index} in the current explanation."),
        "evidence":format!("The current read establishes only part of relation {index}; its remaining source conditions have not been read."),
        "required_change":change})).collect();
    let rejection=serde_json::json!({"decision":"revise","revision":"Read all four complete source relationships before resubmitting.","learner_claims":[],
        "request_resolution":{"status":"answered","question_quote":question,
            "reason":"The explanation addresses the request; its four source gaps still require the specified reading."},
        "findings":findings});
    assert!(rejection.to_string().len()>4096,"the old complete tool feedback would exceed its 4 KiB receipt budget");
    let retry=r#"{"operation":"revise","draft_ref":"teaching-draft:1","edits":[]}"#;
    let adapter=ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("first-source","book.text",r#"{"lid":"1.1"}"#)]),
        turn_calls(vec![call("long-rejection","tutor.step",&selection.to_string())]),
        turn_calls(vec![call("complete-source","book.text",r#"{"lid":"1.2"}"#)]),
        turn_calls(vec![call("review-unavailable","tutor.step",retry)]),
        turn_calls(vec![call("retry-review","tutor.step",retry)]),
    ],vec![Ok(rejection),Err(AdapterError {spend_stop:None,message:"Review provider temporarily unavailable".into()}),accepted_review()]);
    let mut context=RunContext::new(new_session(),OuterConfig {max_turns:Some(16),..Default::default()},adapter.model_runtime_profile());
    context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:vec![]};
    let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),question,"t0").unwrap();
    assert!(!outcome.incomplete,"{outcome:?}");
    assert_eq!(outcome.answer.as_deref(),Some(answer));
    assert_eq!(port.accepted,["select"]);
    assert_eq!(port.reviews.len(),2,"a failed provider call cannot replace the last real review");
    assert_eq!(port.reviews[1]["review"]["open_issues"],serde_json::json!([]));
    let plans=adapter.inner.0.seen_plans.borrow();
    assert_eq!(plans.len(),5);
    for index in [2,3,4] {
        let messages=plans[index].ordered_messages();
        let private=active_teaching_revision_draft(&messages);
        assert_eq!(private["draft_ref"],"teaching-draft:1");
        let feedback=&private["review_feedback"];
        assert!(feedback.get("request_resolution").is_none() && feedback.get("revision").is_none(),
            "current issues carry complete feedback without duplicate aggregate instructions");
        assert_eq!(feedback["guidance"],TEACHING_REVISION_GUIDANCE);
        assert!(feedback.get("findings").is_none(),"the feedback must not duplicate open_issues as another findings array");
        let issues=feedback["open_issues"].as_array().unwrap();
        assert_eq!(issues.len(),4);
        for (issue,change) in required_changes.iter().enumerate() {
            let reference=format!("factual:1:{issue}");
            let matching:Vec<_>=issues.iter().filter(|item|item["issue_ref"]==reference).collect();
            assert_eq!(matching.len(),1,"request {index} must retain each issue identity once");
            assert_eq!(matching[0]["finding"]["required_change"],*change,"request {index} must retain the complete original required_change");
            assert_eq!(matching[0]["finding"],port.reviews[0]["review"]["findings"][issue]);
            assert_eq!(feedback.to_string().matches(&format!("REPAIR_SCOPE_{issue}:")).count(),1,
                "each complete repair direction appears only once in current feedback");
        }
        let receipt=tool_result(&messages,"long-rejection");
        assert!(!receipt.contains("semantic_budget_exceeded"));
        assert!(receipt.contains("teaching-draft:1") && receipt.contains("review_feedback"));
        assert!(!receipt.contains("REPAIR_SCOPE_"),"full opinions belong in the private fragment, not a duplicate tool receipt");
        if index==4 {
            assert!(tool_result(&messages,"review-unavailable").contains("TEACHING_REVIEW_FAILED"));
        }
    }
    let reviews=adapter.seen_reviews.borrow();
    assert_eq!(reviews.len(),3);
    assert_eq!(reviews[1]["proposed_delivery"],reviews[2]["proposed_delivery"]);
    for review in &reviews[1..] {
        assert_eq!(review["open_issues"].as_array().unwrap().len(),4);
        assert!(review["current_source_evidence"].as_array().unwrap().iter().any(|source|
            source["segments"].as_array().unwrap().iter().any(|segment|segment["lid"]=="1.2")));
    }
}

#[test]
fn teaching_patch_preserves_complete_long_delivery_participation_and_finish() {
    let b=book();
    let mut store=MemoryStore::open(tmp("teaching-patch-long-delivery")).unwrap();
    let mut reader=Reader::new(&b,DEFAULT_RADIUS);
    let snapshot=default_profile_snapshot(&b,&store,"t0");
    let question="Explain how the retained catalog differs from the selected route.";
    let unchanged="The catalog preserves the same card so a later route can select it. ".repeat(35);
    let original=format!("{unchanged}\n\nPRIVATE_WRONG_CATALOG_CLAIM\n\nUNCHANGED_TAIL_AFTER_LONG_EXPLANATION");
    let replacement="The route omits B while the catalog continues to retain B.";
    let mut selection=selected_body("patched-long",&original);
    selection["finish"]=serde_json::json!(true);
    selection["move"]["participation"]["reason"]=serde_json::json!("PRIVATE_ORIGINAL_PARTICIPATION_MUST_SURVIVE");
    let revision=serde_json::json!({"operation":"revise","draft_ref":"teaching-draft:1","edits":[
        {"field":"delivery_text","paragraph":2,"new_text":replacement}]});
    let adapter=ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("initial-draft","tutor.step",&selection.to_string())]),
        turn_calls(vec![call("patch-draft","tutor.step",&revision.to_string())]),
    ],vec![patch_factual_revision(question,"the catalog claim"),patch_review_update("resolved")]);
    let mut context=RunContext::new(new_session(),OuterConfig {max_turns:Some(12),..Default::default()},adapter.model_runtime_profile());
    context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let sink=std::sync::Arc::new(TeachingStreamSink::default());
    context.events=crate::run_events::RunEvents::new(Some(sink.clone()));
    let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:vec![]};
    let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),question,"t0").unwrap();
    let expected=original.replace("PRIVATE_WRONG_CATALOG_CLAIM",replacement);
    assert!(!outcome.incomplete,"{outcome:?}");
    assert_eq!(outcome.answer.as_deref(),Some(expected.as_str()));
    assert_eq!(port.accepted,["select"]);
    let selected=&port.selections["selected:patched-long"];
    for (key,value) in selection["move"].as_object().unwrap() {
        if key!="delivery_text" { assert_eq!(&selected[key],value,"the patch must preserve move.{key}"); }
    }
    assert_eq!(selected["delivery_text"],expected);
    assert_eq!(adapter.inner.0.seen_plans.borrow().len(),2,"finish=true must remain effective without another author turn");
    let facts=adapter.seen_reviews.borrow();
    assert_eq!(facts.len(),2);
    assert_eq!(facts[1]["proposed_delivery"]["text"],expected,"review must receive the complete patched text, not only its changed span");
    assert_eq!(facts[1]["open_issues"][0]["issue_ref"],"factual:1:0");
    let explanations=adapter.seen_explanation_reviews.borrow();
    assert_eq!(explanations.len(),1);
    assert_eq!(explanations[0]["revision_reference"]["text"],original,"the explanation reviewer needs the actual first draft to judge retained depth");
    assert_eq!(explanations[0]["current_delivery"],expected,"the complete current delivery remains the quotation witness");
    let plans=adapter.inner.0.seen_plans.borrow();
    let retry=plans[1].ordered_messages();
    let private=active_teaching_revision_draft(&retry);
    assert_eq!(private["draft_ref"],"teaching-draft:1");
    let preserved=&private["preserved_select_fields"];
    assert_eq!(preserved["move"]["participation"]["reason"],"PRIVATE_ORIGINAL_PARTICIPATION_MUST_SURVIVE");
    assert!(preserved["move"].get("delivery_text").is_none());
    let editable=private["editable_paragraphs"].as_array().unwrap();
    assert_eq!(editable.len(),3);
    for (paragraph,text) in [(1,unchanged.as_str()),(2,"PRIVATE_WRONG_CATALOG_CLAIM"),(3,"UNCHANGED_TAIL_AFTER_LONG_EXPLANATION")] {
        assert_eq!(editable[paragraph-1],serde_json::json!({"field":"delivery_text","paragraph":paragraph,"text":text}),
            "the author receives paragraph {paragraph} with its host-assigned number and exact text");
        assert!(!preserved.to_string().contains(text),"editable paragraph {paragraph} must not also repeat inside preserved metadata");
    }
    let initial_call=context.messages.iter().flat_map(|message|&message.tool_calls).find(|call|call.id=="initial-draft").unwrap();
    let initial_arguments:serde_json::Value=serde_json::from_str(&initial_call.arguments).unwrap();
    assert!(initial_arguments["move"].get("delivery_text").is_none());
    assert!(initial_arguments["move"].get("participation").is_none());
    let patch_call=context.messages.iter().flat_map(|message|&message.tool_calls).find(|call|call.id=="patch-draft").unwrap();
    let arguments:serde_json::Value=serde_json::from_str(&patch_call.arguments).unwrap();
    assert_eq!(arguments["edits"],serde_json::json!([{"field":"delivery_text","paragraph":2}]),"patch edit text belongs only to the private draft path");
    assert!(!serde_json::to_string(&*sink.0.lock().unwrap()).unwrap().contains("PRIVATE_WRONG_CATALOG_CLAIM"));
}

#[test]
fn teaching_patch_empty_edits_rechecks_complete_evidence_before_closing_issue() {
    let b=book_leaves(2);
    let mut store=MemoryStore::open(tmp("teaching-patch-evidence-only")).unwrap();
    let mut reader=Reader::new(&b,DEFAULT_RADIUS);
    let snapshot=default_profile_snapshot(&b,&store,"t0");
    let question="Read 1.1 and 1.2 and explain why the retained card remains available.";
    let selection=selected_body("evidence-only","The retained catalog contains the card omitted from this route.");
    let adapter=ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("first-source","book.text",r#"{"lid":"1.1"}"#)]),
        turn_calls(vec![call("before-evidence","tutor.step",&selection.to_string())]),
        turn_calls(vec![call("missing-source","book.text",r#"{"lid":"1.2"}"#)]),
        turn_calls(vec![call("evidence-only","tutor.step",r#"{"operation":"revise","draft_ref":"teaching-draft:1","edits":[]}"#)]),
        turn_final("Deliver the supported text."),
    ],vec![patch_factual_revision(question,"the retained catalog claim"),patch_review_update("resolved")]);
    let mut context=RunContext::new(new_session(),OuterConfig {max_turns:Some(12),..Default::default()},adapter.model_runtime_profile());
    context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:vec![]};
    let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),question,"t0").unwrap();
    assert!(!outcome.incomplete,"{outcome:?}");
    assert_eq!(port.accepted,["select"]);
    assert_eq!(outcome.answer.as_deref(),selection["move"]["delivery_text"].as_str());
    assert!(!tool_result(&context.messages,"missing-source").contains("LID_PROVENANCE_REQUIRED"),
        "the explicitly requested second passage must be read before evidence-only resubmission");
    let facts=adapter.seen_reviews.borrow();
    assert_eq!(facts.len(),2);
    assert_eq!(facts[0]["proposed_delivery"],facts[1]["proposed_delivery"],"new evidence need not rewrite an already supported explanation");
    assert_eq!(facts[1]["open_issues"].as_array().unwrap().len(),1);
    assert_eq!(facts[1]["open_issues"][0]["issue_ref"],"factual:1:0","reading does not close the reviewer's issue");
    let segments=facts[1]["current_source_evidence"].as_array().unwrap().iter()
        .flat_map(|range|range["segments"].as_array().unwrap().iter()).collect::<Vec<_>>();
    assert!(segments.iter().any(|segment|segment["lid"]=="1.1"));
    assert!(segments.iter().any(|segment|segment["lid"]=="1.2"));
    let explanations=adapter.seen_explanation_reviews.borrow();
    assert_eq!(explanations.len(),1,"only the factually accepted resubmission reaches the independent explanation review");
    assert_eq!(explanations[0]["current_delivery"],selection["move"]["delivery_text"]);
    assert_eq!(explanations[0]["revision_reference"]["text"],selection["move"]["delivery_text"],"evidence-only resubmission preserves the original complete reference text");
    assert_eq!(port.reviews[1]["independent_reviews"]["factual"]["review"]["issue_updates"][0]["status"],"resolved");
    assert_eq!(port.reviews[1]["independent_reviews"]["explanation"]["review"]["decision"],"accept");
    let plans=adapter.inner.0.seen_plans.borrow();
    let current_snapshot=|index:usize| plans[index].ordered_messages().into_iter().filter_map(|message|message.content)
        .rfind(|text|text.starts_with("runtime_state_snapshot.v1")).unwrap();
    for index in [2,3] {
        let messages=plans[index].ordered_messages();
        let private=active_teaching_revision_draft(&messages);
        assert_eq!(private["draft_ref"],"teaching-draft:1");
        assert_eq!(private["editable_paragraphs"][0]["text"],selection["move"]["delivery_text"]);
        assert_eq!(private["review_feedback"]["open_issues"][0]["issue_ref"],"factual:1:0");
        assert_eq!(private["review_feedback"]["open_issues"][0]["finding"],port.reviews[0]["review"]["findings"][0]);
        let receipt=tool_result(&messages,"before-evidence");
        assert!(receipt.contains("review_feedback") && receipt.contains("teaching-draft:1"),"the brief receipt points to the complete private repair context");
    }
    assert!(!current_snapshot(4).contains("agent.teaching_revision"),"successful selection expires the private repair draft");
}

#[test]
fn teaching_patch_unresolved_fact_keeps_identity_and_rejects_stale_draft() {
    let b=book();
    let mut store=MemoryStore::open(tmp("teaching-patch-unresolved-fact")).unwrap();
    let mut reader=Reader::new(&b,DEFAULT_RADIUS);
    let snapshot=default_profile_snapshot(&b,&store,"t0");
    let question="Explain the actual retained catalog.";
    let mut selection=selected_body("stable-issue","PRIVATE_ORIGINAL_WRONG_CLAIM");
    selection["finish"]=serde_json::json!(true);
    let patch=|reference:&str,new:&str|serde_json::json!({"operation":"revise","draft_ref":reference,
        "edits":[{"field":"delivery_text","paragraph":1,"new_text":new}]}).to_string();
    let final_text="The retained catalog keeps B available even when this route omits B.";
    let adapter=ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("original","tutor.step",&selection.to_string())]),
        turn_calls(vec![call("still-unresolved","tutor.step",&patch("teaching-draft:1","PRIVATE_INTERMEDIATE_WRONG_CLAIM"))]),
        turn_calls(vec![call("stale","tutor.step",&patch("teaching-draft:1",final_text))]),
        turn_calls(vec![call("supported","tutor.step",&patch("teaching-draft:2",final_text))]),
    ],vec![patch_factual_revision(question,"the catalog claim"),patch_review_update("unresolved"),patch_review_update("resolved")]);
    let mut context=RunContext::new(new_session(),OuterConfig {max_turns:Some(16),..Default::default()},adapter.model_runtime_profile());
    context.tutor=Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let sink=std::sync::Arc::new(TeachingStreamSink::default());
    context.events=crate::run_events::RunEvents::new(Some(sink.clone()));
    let mut port=TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},accepted:vec![],selections:Default::default(),reviews:vec![]};
    let outcome=run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),question,"t0").unwrap();
    assert!(!outcome.incomplete,"{outcome:?}");
    assert_eq!(outcome.answer.as_deref(),Some(final_text));
    assert_eq!(port.accepted,["select"]);
    assert_eq!(port.reviews.len(),3,"a stale reference must not start another review");
    assert_eq!(port.reviews[1]["review"]["decision"],"revise","an unresolved factual issue overrides a reported accept");
    let explanations=adapter.seen_explanation_reviews.borrow();
    assert_eq!(explanations.len(),1,"unresolved facts must return to the author before explanation review");
    assert_eq!(explanations[0]["revision_reference"]["text"],selection["move"]["delivery_text"],"successive patches retain the first complete draft as the depth reference");
    assert_eq!(explanations[0]["current_delivery"],final_text,"quotes must still be checked against the actual complete final draft");
    let facts=adapter.seen_reviews.borrow();
    for index in [1,2] {
        assert_eq!(facts[index]["open_issues"].as_array().unwrap().len(),1);
        assert_eq!(facts[index]["open_issues"][0]["issue_ref"],"factual:1:0","carried findings keep their original identity");
    }
    assert!(tool_result(&context.messages,"stale").contains("draft_ref"));
    let plans=adapter.inner.0.seen_plans.borrow();
    for index in [2,3] {
        let messages=plans[index].ordered_messages();
        let private=active_teaching_revision_draft(&messages);
        assert_eq!(private["draft_ref"],"teaching-draft:2","a failed stale edit leaves the latest draft reference unchanged");
        assert_eq!(private["editable_paragraphs"][0]["text"],"PRIVATE_INTERMEDIATE_WRONG_CLAIM");
        assert_eq!(private["review_feedback"]["open_issues"][0]["issue_ref"],"factual:1:0");
        let current=&private["review_feedback"]["open_issues"][0]["finding"];
        assert_eq!(current["location"],port.reviews[0]["review"]["findings"][0]["location"]);
        assert_eq!(current["required_change"],port.reviews[1]["review"]["findings"][0]["required_change"]);
        assert!(current["required_change"].as_str().unwrap().contains("still lacks the source evidence"));
        assert!(private["review_feedback"]["open_issues"][0].get("last_reason").is_none());
        let receipt=tool_result(&messages,"still-unresolved");
        assert!(receipt.contains("review_feedback") && receipt.contains("teaching-draft:2"),"the brief receipt points to the latest draft and its unchanged issue identity");
    }
    let public_calls=serde_json::to_string(&context.messages.iter().flat_map(|message|&message.tool_calls).collect::<Vec<_>>()).unwrap();
    let stream=serde_json::to_string(&*sink.0.lock().unwrap()).unwrap();
    for private in ["PRIVATE_ORIGINAL_WRONG_CLAIM","PRIVATE_INTERMEDIATE_WRONG_CLAIM"] {
        assert!(!public_calls.contains(private),"unaccepted patch arguments must stay private: {private}");
        assert!(!stream.contains(private),"unaccepted patch content must not stream: {private}");
    }
    for call in context.messages.iter().flat_map(|message|&message.tool_calls).filter(|call|matches!(call.id.as_str(),"still-unresolved"|"stale"|"supported")) {
        let arguments:serde_json::Value=serde_json::from_str(&call.arguments).unwrap();
        assert_eq!(arguments["edits"],serde_json::json!([{"field":"delivery_text","paragraph":1}]),"private edits must not survive in the public tool history");
    }
}
