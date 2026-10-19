//! Delivery and response boundaries for the ordinary teaching loop.
use super::*;
use memory::teaching::{TeachingEvent, TeachingFact};

pub(super) fn fixture() -> (tempfile::TempDir, AppState) {
    let source="Average speed is total distance divided by total time. Equal distances need not take equal times. The total time must be positive. ".repeat(3);
    super::tutor_loop_tests::fixture_with_book(false,Book::new(sample_base(),&source),&source)
}

pub(super) fn range() -> EvidenceRange { EvidenceRange { start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![] } }

pub(super) fn target() -> Value {
    json!({"learning_focus":"Whole-trip average speed","expected_performance":"Distinguish moving time from total elapsed time","capability":"explanation",
        "source_bindings":[{"start_lid":"1.1","end_lid":"1.1"}],"object_refs":[]})
}

pub(super) fn design_request(operation_id:&str, previous:Option<&str>) -> Value {
    json!({"operation":"design","operation_id":operation_id,"design":{
        "target":target(),"organizing_model":"A trip is one total distance and all elapsed time; moving speed alone cannot tell us its average.",
        "representation":{"kind":"text","reason":"The current relationship can be established with the short trip example."},
        "blocks":[{"block_id":"total-time","relation":"Whole-trip time includes time spent stopped.","purpose":"Compare journeys even when one moves faster but stops.",
            "source_bindings":[{"start_lid":"1.1","end_lid":"1.1"}]}],"connections":[],"focus_block_refs":["total-time"],
        "basis_refs":[],"open_question_refs":[],"revision_reason":"Make the time denominator visible before comparing speeds.","previous_ref":previous}})
}

pub(super) fn select_request(id:&str, design_ref:&str, invite:bool) -> Value {
    let mut movement=json!({"move_id":id,"target":target(),"object_ids":[],"capability":"explanation","kind":"compare","interaction_intent":"neutral","intent_origin":"neutral",
        "design_ref":design_ref,"block_refs":["total-time"],"prompt":"Compare a trip with a stop against the same distance without a stop.","actions":[]});
    if invite { movement["response_expectation"]=json!({"public_prompt":"A moves for 10 minutes and stops for 5; B takes 12 minutes without a stop over the same distance. Which trip has the higher average speed, and why?","block_refs":["total-time"]}); }
    super::tutor_loop_tests::fixture_participation(&mut movement);
    json!({"operation":"select","move":movement})
}

fn call(state:&AppState,turn:&AgentTurnRef,input:Value)->Value {
    teaching::step(&state.private_context(),turn,input,&[],&[range()]).unwrap()
}

fn complete(state:&mut AppState,turn:&AgentTurnRef,selected:&Value,incomplete:bool) {
    let mut output=super::tutor_loop_tests::outcome(None);
    output.answer=Some(selected["delivery_draft"]["text"].as_str().unwrap().to_owned());
    if let Some(view)=output.answer_view.as_mut() {
        view.parts=vec![AgentAnswerPart::Markdown {text:output.answer.clone().unwrap()}];
    }
    output.teaching_delivery_move_refs=serde_json::from_value(selected["delivery_move_refs"].clone()).unwrap();
    output.incomplete=incomplete;
    super::tutor_loop_tests::finish(state,turn,&output);
}

#[test]
fn teaching_design_grounding_unknown_allows_explanation_and_rejects_delivery_as_understanding() {
    let (_root,mut state)=fixture();
    let user_text="Please explain whole-trip average speed. I do not understand why stops count.";
    let run=prepare_agent_chat(&mut state,&json!({"message":user_text}).to_string(),"first").unwrap_or_else(|e|panic!("{}",e.body));
    let initial=&run.tutor.as_ref().unwrap()["learner_context"]["learner_grounding"];
    assert_eq!(initial["expressions"][0]["text"],user_text);
    assert!(initial["interpretations"].as_array().unwrap().is_empty());
    let accepted=call(&state,&run.turn_ref,design_request("explain-first",None));
    let mut support=select_request("plain-explanation",accepted["design_ref"].as_str().unwrap(),false);
    support["move"]["delivery_text"]=json!("The trip begins at departure and ends at arrival. A stop lies inside that interval, so average speed divides the distance by both moving and stopped time.");
    support["learner_grounding"]=json!({"expressions":[{"fact_ref":"invented","text":"I already understand."}]});
    let normalized=teaching::preflight_step(&state.private_context(),&run.turn_ref,&support,&[],&[range()]).unwrap();
    assert_eq!(normalized["learner_grounding"]["expressions"][0]["text"],user_text,"the host replaces model-supplied understanding context");
    assert!(normalized["move"]["response_expectation"].is_null(),"unknown understanding does not require a quiz before explanation");
    assert!(normalized["learner_grounding"]["interpretations"].as_array().unwrap().is_empty());
    let selected=call(&state,&run.turn_ref,support);
    complete(&mut state,&run.turn_ref,&selected,false);
    let delivery=state.user.learning_store().unwrap().teaching_deliveries(&run.turn_ref.session_id,&run.turn_ref.turn_id).unwrap().remove(0);
    let second=prepare_agent_chat(&mut state,r#"{"message":"So elapsed time includes the stop, right?"}"#,"second").unwrap_or_else(|e|panic!("{}",e.body));
    let mut revision=design_request("delivery-is-not-learning",accepted["design_ref"].as_str());
    revision["design"]["basis_refs"]=json!([delivery.event_id]);
    assert!(teaching::step(&state.private_context(),&second.turn_ref,revision,&[],&[range()]).is_err(),
        "delivering an explanation is not a basis for a learner-understanding claim");
    let fact_ref=format!("usage:{}:user:{}",delivery.binding.tutor_session_id,second.turn_ref.turn_id);
    let hypothesis=json!({"operation":"evidence","nature":"hypothesis","operation_id":"mixed-facts",
        "target":target(),"fact_refs":[fact_ref,delivery.event_id],
        "interpretation":"The learner asks whether stops are included.","teaching_implication":"Confirm that relationship."});
    assert!(teaching::step(&state.private_context(),&second.turn_ref,hypothesis,&[],&[range()]).is_err(),
        "even a mixed set of real words plus a delivery cannot turn help into learner evidence");
}

#[test]
fn teaching_design_grounding_carries_original_words_and_full_input_time_help_into_review() {
    let (_root,mut state)=fixture();
    let original=format!("I cannot see why waiting belongs in the denominator. {} END-OF-LEARNER-WORDS", "I am asking about the same entire journey. ".repeat(35));
    let first=prepare_agent_chat(&mut state,&json!({"message":original}).to_string(),"first").unwrap_or_else(|e|panic!("{}",e.body));
    let tutor_session=first.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap();
    let original_ref=format!("usage:{tutor_session}:user:{}",first.turn_ref.turn_id);
    let accepted=call(&state,&first.turn_ref,design_request("grounded-help",None));
    let design_ref=accepted["design_ref"].as_str().unwrap();
    let answer=format!("{} The stopped interval belongs to elapsed time. END-OF-DELIVERED-HELP", "This explanation tracks one journey from departure to arrival. ".repeat(40));
    let mut support=select_request("long-help",design_ref,false);
    support["move"]["delivery_text"]=json!(answer);
    support["move"]["responds_to"]=json!({"fact_refs":[original_ref],"disposition":"addressed"});
    let selected=call(&state,&first.turn_ref,support);
    complete(&mut state,&first.turn_ref,&selected,false);
    let delivery=state.user.learning_store().unwrap().teaching_deliveries(&first.turn_ref.session_id,&first.turn_ref.turn_id).unwrap().remove(0);
    let second=prepare_agent_chat(&mut state,r#"{"message":"Then the waiting interval is part of the total time, right?"}"#,"second").unwrap_or_else(|e|panic!("{}",e.body));
    assert!(second.tutor.as_ref().unwrap()["learner_context"]["learner_grounding"]["expressions"].as_array().unwrap()
        .iter().any(|fact|fact["fact_ref"]==original_ref && fact["text"]==original),
        "the words that prompted prior support remain available for a before/help/after comparison");
    let response_ref=format!("usage:{}:user:{}",delivery.binding.tutor_session_id,second.turn_ref.turn_id);
    let interpretation=call(&state,&second.turn_ref,json!({"operation":"evidence","nature":"hypothesis","operation_id":"grounded-confirmation",
        "target":target(),"design_ref":design_ref,"block_refs":["total-time"],"fact_refs":[response_ref],
        "interpretation":"The learner explicitly connects waiting with total elapsed time and asks for confirmation.",
        "teaching_implication":"Confirm the expressed connection without inferring another misconception."}));
    let mut revised=design_request("use-confirmation",Some(design_ref));
    revised["design"]["basis_refs"]=json!([interpretation["evidence_ref"]]);
    let revision=call(&state,&second.turn_ref,revised);
    let author_grounding=&revision["learner_context_update"]["learner_grounding"];
    assert!(author_grounding["expressions"].as_array().unwrap().iter().any(|fact|fact["fact_ref"]==original_ref && fact["text"]==original));
    let later_help=TeachingEvent {event_id:"help-after-confirmation".into(),binding:delivery.binding.clone(),kind:TeachingFact::HelpDisplayed,
        causal_refs:vec![delivery.event_id.clone()],payload:json!({"delivery_ref":delivery.event_id,"text":"This help arrived after the learner's words."}),occurred_at:"later".into()};
    state.user.learning_store().unwrap().append_teaching(&later_help).unwrap();
    let mut confirmation=select_request("confirm",revision["design_ref"].as_str().unwrap(),false);
    confirmation["move"]["responds_to"]=json!({"fact_refs":[response_ref],"disposition":"addressed"});
    confirmation["move"]["interpretation_refs"]=json!([interpretation["evidence_ref"]]);
    let normalized=teaching::preflight_step(&state.private_context(),&second.turn_ref,&confirmation,&[],&[range()]).unwrap();
    let grounding=&normalized["learner_grounding"];
    assert_eq!(grounding["interpretations"][0]["fact_refs"],json!([response_ref]));
    assert!(grounding["expressions"].as_array().unwrap().iter().any(|fact|fact["fact_ref"]==original_ref && fact["text"]==original));
    assert!(grounding["delivered_support"].as_array().unwrap().iter().any(|help|help["delivery_ref"]==delivery.event_id && help["answer"]==answer),
        "review receives the complete prior answer, including its final qualification");
    assert!(!grounding["delivered_support"].as_array().unwrap().iter().any(|help|help["delivery_ref"]==later_help.event_id),
        "later help cannot rewrite the support available when the learner expressed the connection");
}

#[test]
fn teaching_design_grounding_keeps_original_words_for_the_explicit_retained_moves() {
    let (_root,mut state)=fixture();
    let original="I am unsure whether a stop belongs in the journey time.";
    let first=prepare_agent_chat(&mut state,&json!({"message":original}).to_string(),"first").unwrap_or_else(|e|panic!("{}",e.body));
    let tutor_session=first.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap();
    let original_ref=format!("usage:{tutor_session}:user:{}",first.turn_ref.turn_id);
    let accepted=call(&state,&first.turn_ref,design_request("retained-grounding",None));
    let design_ref=accepted["design_ref"].as_str().unwrap();
    let first_support=call(&state,&first.turn_ref,select_request("earlier-help",design_ref,false));
    complete(&mut state,&first.turn_ref,&first_support,false);
    let second=prepare_agent_chat(&mut state,r#"{"message":"Please make the whole-trip comparison clearer."}"#,"second").unwrap_or_else(|e|panic!("{}",e.body));
    let mut historical=select_request("answer-earlier-question",design_ref,false);
    historical["move"]["responds_to"]=json!({"fact_refs":[original_ref],"disposition":"addressed"});
    let saved=call(&state,&second.turn_ref,historical);
    let mut next=select_request("connect-comparison",design_ref,false);
    let alone=teaching::preflight_step(&state.private_context(),&second.turn_ref,&next,&[],&[range()]).unwrap();
    assert!(!alone["learner_grounding"]["expressions"].as_array().unwrap().iter().any(|fact|fact["fact_ref"]==original_ref),
        "an earlier unretained selection does not silently add its learner basis");
    next["delivery_move_refs"]=json!([saved["selected"]]);
    let together=teaching::preflight_step(&state.private_context(),&second.turn_ref,&next,&[],&[range()]).unwrap();
    assert!(together["learner_grounding"]["expressions"].as_array().unwrap().iter().any(|fact|fact["fact_ref"]==original_ref && fact["text"]==original),
        "review of the retained body must carry the same original words as its first selection");
    let selected=call(&state,&second.turn_ref,next);
    assert_eq!(selected["delivery_draft"]["moves"].as_array().unwrap().len(),2);
}

#[test]
fn teaching_design_grounding_rejects_superseded_understanding_in_a_retained_move() {
    let (_root,mut state)=fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"Does whole-trip time include the stop?"}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let tutor_session=run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap();
    let fact_ref=format!("usage:{tutor_session}:user:{}",run.turn_ref.turn_id);
    let accepted=call(&state,&run.turn_ref,design_request("correct-retained-understanding",None));
    let design_ref=accepted["design_ref"].as_str().unwrap();
    let mut hypothesis=json!({"operation":"evidence","nature":"hypothesis","operation_id":"before-correction",
        "target":target(),"design_ref":design_ref,"block_refs":["total-time"],"fact_refs":[fact_ref],
        "interpretation":"The learner excludes stopped time.","teaching_implication":"Correct that denominator."});
    let prior=call(&state,&run.turn_ref,hypothesis.clone());
    let mut first=select_request("uses-prior-understanding",design_ref,false);
    first["move"]["interpretation_refs"]=json!([prior["evidence_ref"]]);
    let saved=call(&state,&run.turn_ref,first.clone());
    hypothesis["operation_id"]=json!("after-correction");
    hypothesis["supersedes"]=prior["evidence_ref"].clone();
    hypothesis["interpretation"]=json!("The learner asks for confirmation and has not asserted that stopped time is excluded.");
    hypothesis["teaching_implication"]=json!("Explain why the stopped interval is included, without inventing a misconception.");
    let corrected=call(&state,&run.turn_ref,hypothesis);
    let mut next=select_request("finish-explanation",design_ref,false);
    next["delivery_move_refs"]=json!([saved["selected"]]);
    assert!(teaching::preflight_step(&state.private_context(),&run.turn_ref,&next,&[],&[range()]).is_err(),
        "retaining a selected body must not bypass the current understanding check");
    assert!(teaching::step(&state.private_context(),&run.turn_ref,next.clone(),&[],&[range()]).is_err(),
        "the eventual append enforces the same rule as preflight");
    first["move"]["move_id"]=json!("uses-corrected-understanding");
    first["move"]["interpretation_refs"]=json!([corrected["evidence_ref"]]);
    let repaired=call(&state,&run.turn_ref,first);
    next["delivery_move_refs"]=json!([repaired["selected"]]);
    let normalized=teaching::preflight_step(&state.private_context(),&run.turn_ref,&next,&[],&[range()]).unwrap();
    assert!(normalized["learner_grounding"]["interpretations"].as_array().unwrap().iter().any(|item|item["evidence_ref"]==corrected["evidence_ref"]));
    assert!(!normalized["learner_grounding"]["interpretations"].as_array().unwrap().iter().any(|item|item["evidence_ref"]==prior["evidence_ref"]));
}

#[test]
fn teaching_design_grounding_legacy_unsupported_basis_stays_readable_until_revised() {
    let (_root,mut state)=fixture();
    let first=prepare_agent_chat(&mut state,r#"{"message":"Please explain average speed."}"#,"first").unwrap_or_else(|e|panic!("{}",e.body));
    let accepted=call(&state,&first.turn_ref,design_request("original",None));
    let selected=call(&state,&first.turn_ref,select_request("explain",accepted["design_ref"].as_str().unwrap(),false));
    complete(&mut state,&first.turn_ref,&selected,false);
    let delivery=state.user.learning_store().unwrap().teaching_deliveries(&first.turn_ref.session_id,&first.turn_ref.turn_id).unwrap().remove(0);
    let mut legacy=state.user.learning_store().unwrap().teaching_event(accepted["design_ref"].as_str().unwrap()).unwrap();
    legacy.event_id="legacy-design-with-delivery-basis".into();
    legacy.payload["design"]["previous_ref"]=accepted["design_ref"].clone();
    legacy.payload["design"]["basis_refs"]=json!([delivery.event_id]);
    state.user.learning_store().unwrap().append_teaching(&legacy).unwrap();
    let next=prepare_agent_chat(&mut state,r#"{"message":"I am still unsure why waiting counts."}"#,"next").unwrap_or_else(|e|panic!("{}",e.body));
    let grounding=&next.tutor.as_ref().unwrap()["learner_context"]["learner_grounding"];
    assert_eq!(grounding["unavailable_basis_refs"],json!([delivery.event_id]));
    assert!(grounding["interpretations"].as_array().unwrap().is_empty());
    let invalid=select_request("cannot-adopt",&legacy.event_id,false);
    assert!(teaching::preflight_step(&state.private_context(),&next.turn_ref,&invalid,&[],&[range()]).is_err());
    let revised=call(&state,&next.turn_ref,design_request("remove-unsupported-premise",Some(&legacy.event_id)));
    let explain=select_request("explain-the-question",revised["design_ref"].as_str().unwrap(),false);
    let normalized=teaching::preflight_step(&state.private_context(),&next.turn_ref,&explain,&[],&[range()]).unwrap();
    assert!(normalized["learner_grounding"]["unavailable_basis_refs"].as_array().unwrap().is_empty());
    assert!(normalized["move"]["response_expectation"].is_null());
}

#[test]
fn teaching_design_participation_has_real_consequences_without_mandatory_questions() {
    let (_root,mut state)=fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"Explain whole-trip speed."}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let accepted=call(&state,&run.turn_ref,design_request("participation",None));
    let design_ref=accepted["design_ref"].as_str().unwrap();
    let mut request=select_request("support",design_ref,false);
    request["move"].as_object_mut().unwrap().remove("participation");
    assert!(teaching::step(&state.private_context(),&run.turn_ref,request.clone(),&[],&[range()]).unwrap_err().message.contains("participation"));
    request["move"]["participation"]=json!({"choice":"invite","reason":"The comparison is ready.","learner_inference":"Choose the denominator.","support":"The two total times are visible."});
    assert!(teaching::step(&state.private_context(),&run.turn_ref,request.clone(),&[],&[range()]).unwrap_err().message.contains("response_expectation"));
    request["move"]["participation"]=json!({"choice":"support","reason":"The user still needs to distinguish moving time from total time.","support":"The stop occurs before this journey ends."});
    let support=call(&state,&run.turn_ref,request.clone());
    assert!(support["selected"].is_string(),"support is accepted without asking a question");
    request["move"]["participation"]["reason"]=json!(" ");
    assert!(teaching::step(&state.private_context(),&run.turn_ref,request,&[],&[range()]).is_err());
    let mut invite=select_request("invite",design_ref,true);
    invite["move"]["participation"]["support"]=json!("");
    assert!(teaching::step(&state.private_context(),&run.turn_ref,invite.clone(),&[],&[range()]).is_err());
    invite=select_request("invite",design_ref,true);
    invite["move"]["participation"]=json!({"choice":"support","reason":"More help first.","support":"The timeline includes the stop."});
    assert!(teaching::step(&state.private_context(),&run.turn_ref,invite.clone(),&[],&[range()]).unwrap_err().message.contains("新的作答任务"));
    invite=select_request("invite",design_ref,true);
    invite["move"]["kind"]=json!("explain");
    invite["move"]["interaction_intent"]=json!("direct_explanation");
    invite["move"]["intent_origin"]=json!("current_request");
    invite["move"]["intent_quote"]=json!("Explain whole-trip speed.");
    assert!(teaching::step(&state.private_context(),&run.turn_ref,invite,&[],&[range()]).unwrap_err().message.contains("直接讲解"));
    complete(&mut state,&run.turn_ref,&support,false);
    let store=state.user.learning_store().unwrap();
    let saved=store.teaching_event(support["selected"].as_str().unwrap()).unwrap();
    assert_eq!(saved.payload["move"]["participation"]["choice"],"support");
    let delivered=store.teaching_deliveries(&run.turn_ref.session_id,&run.turn_ref.turn_id).unwrap();
    assert!(delivered[0].payload["move"].get("participation").is_none(),"private teaching rationale must not appear in public delivery");
    assert!(delivered[0].payload["move"].get("delivery_text").is_none(),"the accepted draft is carried by the actual answer, not a second public move body");
    assert!(store.pending_response_invitation(&saved.binding.tutor_session_id,&saved.binding.source_id,&saved.binding.source_revision).unwrap().is_none());
}

#[test]
fn teaching_design_delivery_draft_uses_only_the_explicit_accepted_set() {
    let (_root,mut state)=fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"I cannot follow the time comparison."}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let accepted=call(&state,&run.turn_ref,design_request("draft",None));
    let design_ref=accepted["design_ref"].as_str().unwrap();
    let mut first=select_request("support",design_ref,false);
    first["move"]["delivery_text"]=json!("The journey continues during the stop.");
    first["move"]["participation"]["learner_inference"]=json!("Decide which elapsed time to use for this journey.");
    let mut missing=first.clone();missing["move"].as_object_mut().unwrap().remove("delivery_text");
    assert!(teaching::step(&state.private_context(),&run.turn_ref,missing,&[],&[range()]).unwrap_err().message.contains("delivery_text"));
    let mut no_support=first.clone();no_support["move"]["participation"]["support"]=json!("");
    assert!(teaching::step(&state.private_context(),&run.turn_ref,no_support,&[],&[range()]).unwrap_err().message.contains("补给"));
    let saved_first=call(&state,&run.turn_ref,first.clone());
    assert_eq!(saved_first["delivery_draft"]["text"],"The journey continues during the stop.");
    first["move"]["delivery_text"]=json!("An unaccepted replacement must not become the draft.");
    assert!(teaching::step(&state.private_context(),&run.turn_ref,first,&[],&[range()]).is_err());
    let mut second=select_request("invitation",design_ref,true);
    let prompt=second["move"]["response_expectation"]["public_prompt"].as_str().unwrap().to_owned();
    second["move"]["delivery_text"]=json!("Use the elapsed time for the entire journey.");
    let alone=call(&state,&run.turn_ref,second.clone());
    assert_eq!(alone["delivery_draft"]["delivery_move_refs"],json!([alone["selected"]]));
    assert_eq!(alone["delivery_draft"]["text"],format!("Use the elapsed time for the entire journey.\n\n{prompt}"));
    second["move"]["move_id"]=json!("retained-invitation");
    second["move"]["delivery_text"]=json!(format!("Now apply that connection. {prompt}"));
    second["delivery_move_refs"]=json!([saved_first["selected"],saved_first["selected"]]);
    let together=call(&state,&run.turn_ref,second);
    assert_eq!(together["delivery_draft"]["delivery_move_refs"],json!([saved_first["selected"],together["selected"]]));
    assert_eq!(together["delivery_draft"]["moves"].as_array().unwrap().len(),2);
    assert_eq!(together["delivery_draft"]["moves"][0]["participation"]["learner_inference"],"Decide which elapsed time to use for this journey.");
    let text=together["delivery_draft"]["text"].as_str().unwrap();
    assert_eq!(text,format!("The journey continues during the stop.\n\nNow apply that connection. {prompt}"));
    assert_eq!(text.matches(prompt.as_str()).count(),1,"the ordinary invitation appears once in its accepted body");
    complete(&mut state,&run.turn_ref,&together,false);
    let deliveries=state.user.learning_store().unwrap().teaching_deliveries(&run.turn_ref.session_id,&run.turn_ref.turn_id).unwrap();
    assert_eq!(deliveries.len(),2,"the abandoned invitation never becomes a delivery");
    for delivery in &deliveries {
        assert_eq!(delivery.payload["answer"],text);
        assert!(delivery.payload["move"].get("participation").is_none());
        assert!(delivery.payload["move"].get("delivery_text").is_none());
    }
}

#[test]
fn teaching_design_ordinary_support_and_remaining_inference_survive_a_focus_change() {
    let (root,mut state)=fixture();
    let before_quote=format!("I cannot tell whether a stop belongs to this journey. {} End of original explanation.","I think only moving minutes count. ".repeat(300));
    let first=prepare_agent_chat(&mut state,&json!({"message":before_quote}).to_string(),"first").unwrap_or_else(|e|panic!("{}",e.body));
    let tutor_session=first.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap();
    let before_ref=format!("usage:{tutor_session}:user:{}",first.turn_ref.turn_id);
    let before=state.user.learning_store().unwrap().teaching_event(&before_ref).unwrap();
    let click=TeachingEvent {event_id:"click-without-expression".into(),binding:before.binding.clone(),kind:TeachingFact::UsageObserved,
        causal_refs:vec![],payload:json!({"usage":{"kind":"reading_action"},"response":"Continue","interaction":{"user_supplement":""},"response_origin":{"kind":"interface_request"}}),occurred_at:"first".into()};
    state.user.learning_store().unwrap().append_teaching(&click).unwrap();
    let accepted=call(&state,&first.turn_ref,design_request("support-before-attempt",None));
    let design_ref=accepted["design_ref"].as_str().unwrap();
    let mut support=select_request("show-interval",design_ref,false);
    support["move"]["delivery_text"]=json!("Departure is before the stop and arrival is after it; both belong to this same journey.");
    support["move"]["participation"]["support"]=json!("Locate the stopped interval inside this journey.");
    support["move"]["participation"]["learner_inference"]=json!("Use the stopped interval when choosing the total elapsed time.");
    support["move"]["responds_to"]=json!({"fact_refs":[before_ref,click.event_id],"disposition":"addressed"});
    assert!(teaching::step(&state.private_context(),&first.turn_ref,support.clone(),&[],&[range()]).is_err(),
        "a UI click cannot supply an expression that a teaching move responds to");
    support["move"]["responds_to"]["fact_refs"]=json!([before_ref]);
    call(&state,&first.turn_ref,select_request("never-delivered",design_ref,false));
    let selected=call(&state,&first.turn_ref,support);
    complete(&mut state,&first.turn_ref,&selected,false);
    let delivery=state.user.learning_store().unwrap().teaching_deliveries(&first.turn_ref.session_id,&first.turn_ref.turn_id).unwrap().remove(0);
    let shown=TeachingEvent {event_id:"interval-hint".into(),binding:delivery.binding.clone(),kind:TeachingFact::HelpDisplayed,
        causal_refs:vec![delivery.event_id.clone()],payload:json!({"delivery_ref":delivery.event_id,"text":"The stopped interval lies between departure and arrival."}),occurred_at:"before-second".into()};
    state.user.learning_store().unwrap().append_teaching(&shown).unwrap();
    let second=prepare_agent_chat(&mut state,r#"{"message":"The clock is still running, I think, but I cannot connect it to the speed."}"#,"second").unwrap_or_else(|e|panic!("{}",e.body));
    let fact_ref=format!("usage:{}:user:{}",delivery.binding.tutor_session_id,second.turn_ref.turn_id);
    let fact=state.user.learning_store().unwrap().teaching_event(&fact_ref).unwrap();
    assert_eq!(fact.payload["support_delivery_refs"],json!([delivery.event_id]));
    assert_eq!(fact.payload["assistance_refs"],json!([shown.event_id]));
    assert!(fact.payload["invitation_context_ref"].is_null(),"receiving help does not manufacture an invitation or attempt");
    let mut revision=design_request("denominator",Some(design_ref));
    revision["design"]["blocks"][0]["block_id"]=json!("distance-time-relation");
    revision["design"]["blocks"][0]["relation"]=json!("Whole-trip speed compares distance to all elapsed time.");
    revision["design"]["focus_block_refs"]=json!(["distance-time-relation"]);
    let revised=call(&state,&second.turn_ref,revision);
    let old=&revised["learner_context_update"]["learning_opportunity"]["delivered_support"][0];
    assert_eq!(old["delivery_ref"],delivery.event_id);
    assert_eq!(old["block_refs"],json!(["total-time"]));
    assert_eq!(old["participation"]["learner_inference"],"Use the stopped interval when choosing the total elapsed time.");
    assert_eq!(old["responding_to_expressions"].as_array().unwrap().len(),1,"only the actual authored expression is adopted by responds_to");
    let initial=&old["responding_to_expressions"][0];
    assert_eq!(initial["fact_ref"],before_ref);
    assert_eq!(initial["fact_refs"],json!([before_ref]));
    assert_eq!(initial["expression_preview"]["truncated"],true);
    assert_eq!(initial["support_delivery_refs"],before.payload["support_delivery_refs"]);
    assert_eq!(initial["assistance_refs"],before.payload["assistance_refs"]);
    assert_eq!(old["learner_expressions"][0]["fact_ref"],fact_ref);
    assert_eq!(old["learner_expressions"][0]["expression_preview"]["text"],"The clock is still running, I think, but I cannot connect it to the speed.");
    let evidence=call(&state,&second.turn_ref,json!({"operation":"evidence","nature":"hypothesis","operation_id":"clock-foothold",
        "target":target(),"design_ref":revised["design_ref"],"block_refs":["distance-time-relation"],"fact_refs":[fact_ref],
        "interpretation":"The learner tentatively connects the stop to elapsed time after seeing the interval; applying that time to speed is still unexpressed.",
        "teaching_implication":"Supply the distance over elapsed time relationship, then decide whether a small remaining judgment is useful."}));
    assert_eq!(evidence["learner_context_update"]["learning_opportunity"]["delivered_support"][0]["learner_expressions"][0]["interpretations"][0]["evidence_ref"],evidence["evidence_ref"]);
    let mut later_hint=shown.clone();
    later_hint.event_id="later-interval-hint".into();
    later_hint.occurred_at="after-second".into();
    state.user.learning_store().unwrap().append_teaching(&later_hint).unwrap();
    let mut later=select_request("later-help",revised["design_ref"].as_str().unwrap(),false);
    later["move"]["block_refs"]=json!(["distance-time-relation"]);
    later["move"]["responds_to"]=json!({"fact_refs":[fact_ref],"disposition":"addressed"});
    let selected=call(&state,&second.turn_ref,later);
    complete(&mut state,&second.turn_ref,&selected,false);
    assert_eq!(state.user.learning_store().unwrap().teaching_event(&fact_ref).unwrap().payload["support_delivery_refs"],json!([delivery.event_id]),
        "later help cannot be counted among the supports the earlier expression had received");
    state.user.store=MemoryStore::open(root.path().join("memory.json")).unwrap();
    let reopened=memory::learning::LearningStore::open(&root.path().join("learning.db"),false).unwrap();
    assert_eq!(reopened.teaching_event(&before_ref).unwrap().learner_expression(),Some(before_quote.as_str()));
    assert_eq!(reopened.teaching_event(&fact_ref).unwrap().payload["support_delivery_refs"],json!([delivery.event_id]));
    drop(reopened);
    let third=prepare_agent_chat(&mut state,r#"{"message":"I can now see the denominator, but only in that example."}"#,"third").unwrap_or_else(|e|panic!("{}",e.body));
    let rows=third.tutor.as_ref().unwrap()["learner_context"]["learning_opportunity"]["delivered_support"].as_array().unwrap();
    assert_eq!(rows.len(),2,"a merely selected move never supplies support");
    assert_eq!(rows[1]["delivery_ref"],delivery.event_id,"the old reasoning opportunity survives later help and a new focus");
    assert_eq!(rows[1]["responding_to_expressions"],old["responding_to_expressions"],"the actual expression that prompted help survives a focus revision and database reopen");
    assert_eq!(rows[1]["learner_expressions"][0]["interpretations"][0]["evidence_ref"],evidence["evidence_ref"]);
    assert_eq!(rows[1]["learner_expressions"][0]["support_delivery_refs"],json!([delivery.event_id]));
    let response_to_later=&rows[0]["responding_to_expressions"][0];
    assert_eq!(response_to_later["fact_ref"],fact_ref);
    assert_eq!(response_to_later["support_delivery_refs"],json!([delivery.event_id]));
    assert_eq!(response_to_later["assistance_refs"],json!([shown.event_id]),"the later hint is not a help condition of this earlier expression");
    assert_eq!(response_to_later["interpretations"][0]["evidence_ref"],evidence["evidence_ref"]);
    let mut full=String::new();
    let mut offset=0;
    loop {
        let page=call(&state,&third.turn_ref,json!({"operation":"trace","event_id":before_ref,"after":offset}));
        full.push_str(page["chunk"].as_str().unwrap());
        if let Some(next)=page["next"].as_u64() { offset=next; } else { break; }
    }
    let original:Value=serde_json::from_str(&full).unwrap();
    assert_eq!(original["content"]["response"],before_quote,"a truncated working preview must retain a readable original fact");
}

#[test]
fn teaching_design_support_continues_one_invitation_and_conclusion_closes_it() {
    let (_root,mut state)=fixture();
    let first=prepare_agent_chat(&mut state,r#"{"message":"Explain whole-trip speed."}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let accepted=call(&state,&first.turn_ref,design_request("invitation",None));
    let design_ref=accepted["design_ref"].as_str().unwrap();
    let selected=call(&state,&first.turn_ref,select_request("invite",design_ref,true));
    complete(&mut state,&first.turn_ref,&selected,false);
    let invitation=state.user.learning_store().unwrap().teaching_deliveries(&first.turn_ref.session_id,&first.turn_ref.turn_id).unwrap().remove(0);
    let next=prepare_agent_chat(&mut state,r#"{"message":"I cannot tell which time counts."}"#,"next").unwrap_or_else(|e|panic!("{}",e.body));
    let mut help=select_request("help",design_ref,false);
    help["move"]["responds_to"]=json!({"fact_refs":[],"invitation_ref":invitation.event_id,"disposition":"continue"});
    let selected=call(&state,&next.turn_ref,help.clone());
    complete(&mut state,&next.turn_ref,&selected,false);
    let help_delivery=state.user.learning_store().unwrap().teaching_deliveries(&next.turn_ref.session_id,&next.turn_ref.turn_id).unwrap().remove(0);
    assert_eq!(state.user.learning_store().unwrap().pending_response_invitation(&invitation.binding.tutor_session_id,&invitation.binding.source_id,&invitation.binding.source_revision).unwrap().unwrap().event_id,invitation.event_id);
    let response=prepare_agent_chat(&mut state,r#"{"message":"I think the stopped time is still part of the trip, but I am unsure."}"#,"response").unwrap_or_else(|e|panic!("{}",e.body));
    let response_ref=format!("usage:{}:user:{}",invitation.binding.tutor_session_id,response.turn_ref.turn_id);
    let frozen_support=json!([invitation.event_id,help_delivery.event_id]);
    assert_eq!(state.user.learning_store().unwrap().teaching_event(&response_ref).unwrap().payload["support_delivery_refs"],frozen_support,
        "the learner's reply must preserve both the original invitation and the help actually delivered before it");
    help["move"]["move_id"]=json!("later-help");
    let selected=call(&state,&response.turn_ref,help.clone());
    complete(&mut state,&response.turn_ref,&selected,false);
    assert_eq!(state.user.learning_store().unwrap().teaching_event(&response_ref).unwrap().payload["support_delivery_refs"],frozen_support,
        "later help cannot rewrite the support conditions frozen with the earlier reply");
    let last=prepare_agent_chat(&mut state,r#"{"message":"The stopped minutes count too. That is enough for now."}"#,"last").unwrap_or_else(|e|panic!("{}",e.body));
    help["move"]["move_id"]=json!("conclude");
    help["move"]["participation"]=json!({"choice":"conclude","reason":"The user has resolved this comparison and wants to stop."});
    assert!(teaching::step(&state.private_context(),&last.turn_ref,help.clone(),&[],&[range()]).unwrap_err().message.contains("收束"));
    help["move"]["responds_to"]["disposition"]=json!("addressed");
    let selected=call(&state,&last.turn_ref,help);
    assert!(state.user.learning_store().unwrap().pending_response_invitation(&invitation.binding.tutor_session_id,&invitation.binding.source_id,&invitation.binding.source_revision).unwrap().is_some(),"selection alone cannot close the invitation");
    complete(&mut state,&last.turn_ref,&selected,false);
    assert!(state.user.learning_store().unwrap().pending_response_invitation(&invitation.binding.tutor_session_id,&invitation.binding.source_id,&invitation.binding.source_revision).unwrap().is_none());
}

#[test]
fn teaching_design_learning_opportunity_uses_delivered_support_and_exact_relationships() {
    let (_root,mut state)=fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"I think moving speed decides the whole-trip speed."}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let mut request=design_request("two-relationships",None);
    let mut distance=request["design"]["blocks"][0].clone();
    distance["block_id"]=json!("distance"); distance["relation"]=json!("The numerator is whole-trip distance.");
    request["design"]["blocks"].as_array_mut().unwrap().push(distance);
    let accepted=call(&state,&run.turn_ref,request.clone());
    let design_ref=accepted["design_ref"].as_str().unwrap();
    call(&state,&run.turn_ref,select_request("abandoned",design_ref,false));
    let selected=call(&state,&run.turn_ref,select_request("delivered",design_ref,false));
    let before=call(&state,&run.turn_ref,request.clone());
    assert_eq!(before["learner_context_update"]["learning_opportunity"]["delivered_support"],json!([]));
    let fact_ref=format!("usage:{}:user:{}",run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap(),run.turn_ref.turn_id);
    let mut interpretation=json!({"operation":"evidence","nature":"hypothesis","operation_id":"time",
        "target":target(),"design_ref":design_ref,"block_refs":["total-time"],"fact_refs":[fact_ref],
        "interpretation":"The learner uses moving speed as sufficient.","teaching_implication":"Separate moving and total time."});
    let time=call(&state,&run.turn_ref,interpretation.clone());
    interpretation["operation_id"]=json!("distance");interpretation["block_refs"]=json!(["distance"]);
    let other=call(&state,&run.turn_ref,interpretation);
    assert_eq!(other["learner_context_update"]["learning_opportunity"]["interpretation_refs"],json!([time["evidence_ref"]]));
    complete(&mut state,&run.turn_ref,&selected,false);
    let delivery=state.user.learning_store().unwrap().teaching_deliveries(&run.turn_ref.session_id,&run.turn_ref.turn_id).unwrap().remove(0);
    // Unrelated recent activity must not hide the actual earlier support.
    for index in 0..12 {
        state.user.learning_store().unwrap().append_teaching(&TeachingEvent {event_id:format!("unrelated-usage-{index}"),binding:delivery.binding.clone(),
            kind:TeachingFact::UsageObserved,causal_refs:vec![],payload:json!({"usage":{"kind":"reading_action"}}),occurred_at:"later".into()}).unwrap();
    }
    let next=prepare_agent_chat(&mut state,r#"{"message":"I still mix up those two times."}"#,"next").unwrap_or_else(|e|panic!("{}",e.body));
    let opportunity=&next.tutor.as_ref().unwrap()["learner_context"]["learning_opportunity"];
    assert_eq!(opportunity["delivered_support"].as_array().unwrap().len(),1);
    assert_eq!(opportunity["delivered_support"][0]["delivery_ref"],delivery.event_id);
    assert_eq!(opportunity["interpretation_refs"],json!([time["evidence_ref"]]));
    request["operation_id"]=json!("distance-focus");request["design"]["previous_ref"]=json!(design_ref);
    request["design"]["focus_block_refs"]=json!(["distance"]);
    let revised=call(&state,&next.turn_ref,request.clone());
    let opportunity=&revised["learner_context_update"]["learning_opportunity"];
    assert_eq!(opportunity["delivered_support"][0]["delivery_ref"],delivery.event_id,"a new focus must not erase previously delivered support");
    assert_eq!(opportunity["delivered_support"][0]["block_refs"],json!(["total-time"]),"historical support retains its original relationship instead of being relabeled as the new focus");
    assert_eq!(opportunity["interpretation_refs"],json!([other["evidence_ref"]]));
    request["operation_id"]=json!("return-time");request["design"]["previous_ref"]=revised["design_ref"].clone();request["design"]["focus_block_refs"]=json!(["total-time"]);
    let returned=call(&state,&next.turn_ref,request);
    assert_eq!(returned["learner_context_update"]["learning_opportunity"]["delivered_support"][0]["delivery_ref"],delivery.event_id,"same relationship support survives design revisions");
}

#[test]
fn teaching_design_current_target_keeps_the_delivered_local_step_until_design_revision() {
    let (_root,mut state)=fixture();
    let first=prepare_agent_chat(&mut state,r#"{"message":"I cannot tell which minutes belong to the trip."}"#,"first").unwrap_or_else(|e|panic!("{}",e.body));
    let mut first_design=design_request("whole-trip",None);
    let mut distance=first_design["design"]["blocks"][0].clone();
    distance["block_id"]=json!("distance");distance["relation"]=json!("Whole-trip distance is the numerator.");
    first_design["design"]["blocks"].as_array_mut().unwrap().push(distance);
    first_design["design"]["focus_block_refs"]=json!(["total-time","distance"]);
    let accepted=call(&state,&first.turn_ref,first_design);
    let design_ref=accepted["design_ref"].as_str().unwrap();
    let mut local=select_request("local-step",design_ref,false);
    local["move"]["target"]["learning_focus"]=json!("Count the stopped minutes in this trip");
    local["move"]["target"]["expected_performance"]=json!("Identify the interval between departure and arrival");
    let selected=call(&state,&first.turn_ref,local);
    complete(&mut state,&first.turn_ref,&selected,false);
    let delivery=state.user.learning_store().unwrap().teaching_deliveries(&first.turn_ref.session_id,&first.turn_ref.turn_id).unwrap().remove(0);
    let local_target=delivery.payload["move"]["target"].clone();
    for index in 0..12 {
        state.user.learning_store().unwrap().append_teaching(&TeachingEvent {event_id:format!("local-step-unrelated-{index}"),binding:delivery.binding.clone(),
            kind:TeachingFact::UsageObserved,causal_refs:vec![],payload:json!({"usage":{"kind":"reading_action"}}),occurred_at:"later".into()}).unwrap();
    }
    let second=prepare_agent_chat(&mut state,r#"{"message":"I only see that the clock kept going."}"#,"second").unwrap_or_else(|e|panic!("{}",e.body));
    assert_eq!(second.tutor.as_ref().unwrap()["learner_context"]["current_target"],local_target,
        "the local delivered step must survive unrelated recent events and the broader design target");
    let mut implicit=select_request("inherit-local-step",design_ref,false);
    implicit["move"].as_object_mut().unwrap().remove("target");
    let inherited=call(&state,&second.turn_ref,implicit.clone());
    assert_eq!(state.user.learning_store().unwrap().teaching_event(inherited["selected"].as_str().unwrap()).unwrap().payload["move"]["target"],local_target,
        "omitting target while continuing the delivered relationship must preserve the smaller working step");
    implicit["move"]["move_id"]=json!("another-relationship");implicit["move"]["block_refs"]=json!(["distance"]);
    let changed=call(&state,&second.turn_ref,implicit);
    assert_eq!(state.user.learning_store().unwrap().teaching_event(changed["selected"].as_str().unwrap()).unwrap().payload["move"]["target"],accepted["design"]["target"],
        "another relationship must not inherit the previous local step just because its design is unchanged");
    let unconsumed=call(&state,&second.turn_ref,select_request("unconsumed-broadening",design_ref,false));
    assert_eq!(state.user.learning_store().unwrap().teaching_event(unconsumed["selected"].as_str().unwrap()).unwrap().payload["move"]["target"],accepted["design"]["target"],
        "an explicitly supplied target still takes precedence");
    complete(&mut state,&second.turn_ref,&unconsumed,true);
    let third=prepare_agent_chat(&mut state,r#"{"message":"I still cannot connect that to average speed."}"#,"third").unwrap_or_else(|e|panic!("{}",e.body));
    assert_eq!(third.tutor.as_ref().unwrap()["learner_context"]["current_target"],local_target,
        "an incomplete answer must not turn its selected target into the working step");
    let mut revision=design_request("new-target",Some(design_ref));
    revision["design"]["target"]["learning_focus"]=json!("Compare two complete trips after counting all elapsed time");
    let revised=call(&state,&third.turn_ref,revision);
    assert_eq!(revised["learner_context_update"]["current_target"],revised["design"]["target"],
        "a new design has its own target until that revision delivers a smaller step");
    let mut next=select_request("new-design-support",revised["design_ref"].as_str().unwrap(),false);
    next["move"].as_object_mut().unwrap().remove("target");
    let selected=call(&state,&third.turn_ref,next);
    assert_eq!(state.user.learning_store().unwrap().teaching_event(selected["selected"].as_str().unwrap()).unwrap().payload["move"]["target"],revised["design"]["target"],
        "an omitted target cannot adopt a smaller step from an older design revision");
    complete(&mut state,&third.turn_ref,&selected,false);
    let referenced=prepare_agent_chat(&mut state,&json!({"message":"I meant this earlier step.","teaching_ref":delivery.event_id}).to_string(),"referenced").unwrap_or_else(|e|panic!("{}",e.body));
    assert_eq!(referenced.tutor.as_ref().unwrap()["learner_context"]["current_target"],local_target,
        "an explicit reference retains its original target even after a new design is delivered");
}

#[test]
fn teaching_design_focus_bounds_moves_and_their_invitation_until_explicit_revision() {
    let (_root,mut state)=fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"Explain whole-trip speed."}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let mut request=design_request("focused",None);
    let mut distance=request["design"]["blocks"][0].clone();
    distance["block_id"]=json!("distance");distance["relation"]=json!("Whole-trip distance is the numerator.");
    request["design"]["blocks"].as_array_mut().unwrap().push(distance);
    let accepted=call(&state,&run.turn_ref,request.clone());
    let design_ref=accepted["design_ref"].as_str().unwrap();
    let mut movement=select_request("outside-focus",design_ref,false);
    movement["move"]["block_refs"]=json!(["distance"]);
    assert!(teaching::step(&state.private_context(),&run.turn_ref,movement.clone(),&[],&[range()]).unwrap_err().message.contains("当前教学焦点"));
    let mut invitation=select_request("outside-move",design_ref,true);
    invitation["move"]["response_expectation"]["block_refs"]=json!(["distance"]);
    assert!(teaching::step(&state.private_context(),&run.turn_ref,invitation,&[],&[range()]).unwrap_err().message.contains("本次教学动作"));
    request["operation_id"]=json!("change-focus");request["design"]["previous_ref"]=json!(design_ref);
    request["design"]["focus_block_refs"]=json!(["distance"]);
    let revised=call(&state,&run.turn_ref,request.clone());
    movement["move"]["design_ref"]=revised["design_ref"].clone();
    assert!(call(&state,&run.turn_ref,movement)["selected"].is_string(),"an explicit design revision may change the working relationship");
    request["operation_id"]=json!("full-explanation");request["design"]["previous_ref"]=revised["design_ref"].clone();
    request["design"]["focus_block_refs"]=json!(["total-time","distance"]);
    let wide=call(&state,&run.turn_ref,request);
    let mut full=select_request("full-explanation",wide["design_ref"].as_str().unwrap(),false);
    full["move"]["block_refs"]=json!(["total-time","distance"]);
    full["move"]["kind"]=json!("explain");full["move"]["interaction_intent"]=json!("direct_explanation");
    full["move"]["intent_origin"]=json!("current_request");full["move"]["intent_quote"]=json!("Explain whole-trip speed.");
    assert!(call(&state,&run.turn_ref,full)["selected"].is_string(),"the working focus can remain broad when a complete explanation calls for it");
    let typed:memory::teaching_design::TeachingDesign=serde_json::from_value(wide["design"].clone()).unwrap();
    assert!(typed.validate_focus(&[]).is_ok(),"callers retain responsibility for their existing nonempty constraints");
    assert!(typed.validate_focus(&["unknown".into()]).is_err(),"focus checking also validates block identity");
}

#[test]
fn teaching_design_return_context_keeps_original_question_and_help_separate_from_current_support() {
    let (_root,mut state)=fixture();
    let first=prepare_agent_chat(&mut state,r#"{"message":"How does stopping change the whole-trip average?"}"#,"first").unwrap_or_else(|e|panic!("{}",e.body));
    let mut design=design_request("original",None);
    let mut clock=design["design"]["blocks"][0].clone();
    clock["block_id"]=json!("clock-interval");clock["relation"]=json!("Elapsed time runs from departure to arrival.");
    design["design"]["blocks"].as_array_mut().unwrap().push(clock);
    let accepted=call(&state,&first.turn_ref,design.clone());
    let design_ref=accepted["design_ref"].as_str().unwrap();
    assert!(accepted["learner_context_update"]["learning_opportunity"]["return_context"].is_null(),"no return state is invented without saved links");
    let selected=call(&state,&first.turn_ref,select_request("original-invitation",design_ref,true));
    complete(&mut state,&first.turn_ref,&selected,false);
    let invitation=state.user.learning_store().unwrap().teaching_deliveries(&first.turn_ref.session_id,&first.turn_ref.turn_id).unwrap().remove(0);
    let second=prepare_agent_chat(&mut state,r#"{"message":"Stopped means no movement, so I do not see why that time is in it."}"#,"second").unwrap_or_else(|e|panic!("{}",e.body));
    let mut help=select_request("original-help",design_ref,false);
    help["move"]["responds_to"]=json!({"fact_refs":[],"invitation_ref":invitation.event_id,"disposition":"continue"});
    let selected=call(&state,&second.turn_ref,help);
    complete(&mut state,&second.turn_ref,&selected,false);
    let earlier_help=state.user.learning_store().unwrap().teaching_deliveries(&second.turn_ref.session_id,&second.turn_ref.turn_id).unwrap().remove(0);
    let third=prepare_agent_chat(&mut state,r#"{"message":"The clock keeps going, I think, but I still cannot use that to compare the trips."}"#,"third").unwrap_or_else(|e|panic!("{}",e.body));
    let question_ref=format!("usage:{}:user:{}",invitation.binding.tutor_session_id,third.turn_ref.turn_id);
    design["operation_id"]=json!("smaller-step");design["design"]["previous_ref"]=json!(design_ref);
    design["design"]["focus_block_refs"]=json!(["clock-interval"]);
    design["design"]["open_question_refs"]=json!([question_ref]);
    design["design"]["return_to"]=json!({"block_refs":["total-time"],"invitation_ref":invitation.event_id});
    let revised=call(&state,&third.turn_ref,design);
    let return_context=&revised["learner_context_update"]["learning_opportunity"]["return_context"];
    assert_eq!(return_context["block_refs"],json!(["total-time"]));
    assert_eq!(return_context["open_questions"][0]["fact_ref"],question_ref);
    assert_eq!(return_context["open_questions"][0]["expression_preview"]["text"],"The clock keeps going, I think, but I still cannot use that to compare the trips.");
    assert_eq!(return_context["invitations"][0]["delivery_ref"],invitation.event_id);
    assert_eq!(return_context["support"][0]["event_id"],earlier_help.event_id);
    let mut local=select_request("clock-help",revised["design_ref"].as_str().unwrap(),false);
    local["move"]["block_refs"]=json!(["clock-interval"]);
    local["move"]["responds_to"]=json!({"fact_refs":[question_ref],"invitation_ref":invitation.event_id,"disposition":"deferred"});
    let selected=call(&state,&third.turn_ref,local);
    complete(&mut state,&third.turn_ref,&selected,false);
    let current_help=state.user.learning_store().unwrap().teaching_deliveries(&third.turn_ref.session_id,&third.turn_ref.turn_id).unwrap().remove(0);
    let fourth=prepare_agent_chat(&mut state,r#"{"message":"I can see those start and finish times now."}"#,"fourth").unwrap_or_else(|e|panic!("{}",e.body));
    let opportunity=&fourth.tutor.as_ref().unwrap()["learner_context"]["learning_opportunity"];
    assert_eq!(opportunity["delivered_support"][0]["delivery_ref"],current_help.event_id);
    assert_eq!(opportunity["return_context"],*return_context,
        "later remedial support must not rewrite the help frozen with the original unresolved expression");
    assert!(opportunity["pending_response_invitation"].is_null(),"deferring the old invitation does not erase its explicit return context");
}

#[test]
fn teaching_design_only_final_selection_opens_an_invitation_and_recovers() {
    let (root,mut state)=fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"Explain whole-trip speed."}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let accepted=call(&state,&run.turn_ref,design_request("first",None));
    let design_ref=accepted["design_ref"].as_str().unwrap();
    let old=call(&state,&run.turn_ref,select_request("abandoned",design_ref,true));
    let store=state.user.learning_store().unwrap();
    let session=run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap();
    assert!(store.pending_response_invitation(session,&state.workspace.book.base.book_id,state.workspace.book.source_fingerprint()).unwrap().is_none(),"selection alone must not open an invitation");
    drop(store);
    let final_selection=call(&state,&run.turn_ref,select_request("actual",design_ref,true));
    assert_ne!(old["selected"],final_selection["selected"]);
    complete(&mut state,&run.turn_ref,&final_selection,false);
    state.user.store=MemoryStore::open(root.path().join("memory.json")).unwrap();
    let store=state.user.learning_store().unwrap();
    let deliveries=store.teaching_deliveries(&run.turn_ref.session_id,&run.turn_ref.turn_id).unwrap();
    assert_eq!(deliveries.len(),1);
    assert_eq!(deliveries[0].payload["move"]["move_id"],"actual");
    let invitation=store.pending_response_invitation(session,&state.workspace.book.base.book_id,state.workspace.book.source_fingerprint()).unwrap().unwrap();
    assert_eq!(invitation.event_id,deliveries[0].event_id);
    assert_eq!(store.current_teaching_design(session,&state.workspace.book.base.book_id,state.workspace.book.source_fingerprint()).unwrap().unwrap().event_id,design_ref);
}

#[test]
fn teaching_design_incomplete_answer_does_not_open_an_invitation() {
    let (_root,mut state)=fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"Explain whole-trip speed."}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let accepted=call(&state,&run.turn_ref,design_request("first",None));
    let selected=call(&state,&run.turn_ref,select_request("unfinished",accepted["design_ref"].as_str().unwrap(),true));
    complete(&mut state,&run.turn_ref,&selected,true);
    assert!(state.user.learning_store().unwrap().teaching_deliveries(&run.turn_ref.session_id,&run.turn_ref.turn_id).unwrap().is_empty());
}

#[test]
fn teaching_design_selection_reuses_accepted_target_when_omitted() {
    let (_root,mut state)=fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"Explain whole-trip speed."}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let accepted=call(&state,&run.turn_ref,design_request("target",None));
    let retried=call(&state,&run.turn_ref,design_request("target",None));
    for response in [&accepted,&retried] {
        assert!(response["learner_context_update"].get("recent_facts").is_none());
        assert!(response["learner_context_update"].get("current_request").is_none());
        assert_eq!(response["learner_context_update"]["current_design"]["design_ref"],accepted["design_ref"]);
    }
    let mut request=select_request("reuse-target",accepted["design_ref"].as_str().unwrap(),true);
    request["move"].as_object_mut().unwrap().remove("target");
    let selected=call(&state,&run.turn_ref,request);
    let saved=state.user.learning_store().unwrap().teaching_event(selected["selected"].as_str().unwrap()).unwrap();
    assert_eq!(saved.payload["move"]["target"],accepted["design"]["target"]);
}

#[test]
fn teaching_design_preflight_reports_missing_reads_without_accepting_the_design() {
    let (_root,mut state)=fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"Explain whole-trip speed."}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let request=design_request("source-repair",None);
    let binding:memory::teaching::TeachingBinding=serde_json::from_value(run.tutor.as_ref().unwrap()["binding"].clone()).unwrap();
    let store=state.user.learning_store().unwrap();
    let before=store.teaching_turn_events(&binding.tutor_session_id,&run.turn_ref.turn_id).unwrap().len();
    let missing=teaching::preflight_step(&state.private_context(),&run.turn_ref,&request,&[],&[]).unwrap_err();
    assert_eq!(missing.error.error_code,"TEACHING_INVALID");
    assert_eq!(missing.missing_source_read,Some(range()));
    assert_eq!(store.teaching_turn_events(&binding.tutor_session_id,&run.turn_ref.turn_id).unwrap().len(),before);
    let normalized=teaching::preflight_step(&state.private_context(),&run.turn_ref,&request,&[],&[range()]).unwrap();
    assert_eq!(store.teaching_turn_events(&binding.tutor_session_id,&run.turn_ref.turn_id).unwrap().len(),before,
        "successful source validation must not persist a design before the actual step");
    let accepted=call(&state,&run.turn_ref,normalized.clone());
    assert_eq!(accepted["design"],normalized["design"]);
    // An exact retry retains the stored design and does not demand a new read.
    let retry=teaching::preflight_step(&state.private_context(),&run.turn_ref,&request,&[],&[]).unwrap();
    assert_eq!(retry,normalized);
    assert_eq!(store.teaching_turn_events(&binding.tutor_session_id,&run.turn_ref.turn_id).unwrap().len(),before+1);
}

#[test]
fn teaching_select_preflight_rejects_conflicts_and_reports_missing_reads_without_events() {
    let (_root,mut state)=fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"Explain whole-trip speed."}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let accepted=call(&state,&run.turn_ref,design_request("preflight",None));
    let design_ref=accepted["design_ref"].as_str().unwrap();
    let store=state.user.learning_store().unwrap();
    let binding=store.teaching_event(design_ref).unwrap().binding;
    let before=store.teaching_turn_events(&binding.tutor_session_id,&run.turn_ref.turn_id).unwrap();
    let mut invite=select_request("invite",design_ref,true);
    invite["move"]["kind"]=json!("explain");
    invite["move"]["interaction_intent"]=json!("direct_explanation");
    invite["move"]["intent_origin"]=json!("current_request");
    invite["move"]["intent_quote"]=json!("Explain whole-trip speed.");
    let conflict=teaching::preflight_step(&state.private_context(),&run.turn_ref,&invite,&[],&[range()]).unwrap_err();
    assert!(conflict.error.message.contains("直接讲解"));
    assert!(conflict.missing_source_read.is_none());
    let support=select_request("support",design_ref,false);
    let missing=teaching::preflight_step(&state.private_context(),&run.turn_ref,&support,&[],&[]).unwrap_err();
    assert!(missing.error.message.contains("本轮尚未读取目标"));
    assert_eq!(missing.missing_source_read,Some(range()));
    assert_eq!(store.teaching_turn_events(&binding.tutor_session_id,&run.turn_ref.turn_id).unwrap().len(),before.len(),
        "failed preflights must not append teaching events");
    let normalized=teaching::preflight_step(&state.private_context(),&run.turn_ref,&support,&[],&[range()]).unwrap();
    assert_eq!(store.teaching_turn_events(&binding.tutor_session_id,&run.turn_ref.turn_id).unwrap().len(),before.len(),
        "successful preflight must not select the move");
    let selected=call(&state,&run.turn_ref,normalized.clone());
    let saved=store.teaching_event(selected["selected"].as_str().unwrap()).unwrap();
    assert_eq!(saved.payload["move"],normalized["move"]);
    let after=store.teaching_turn_events(&binding.tutor_session_id,&run.turn_ref.turn_id).unwrap();
    assert_eq!(after.len(),before.len()+1);
    assert_eq!(after.iter().filter(|event|event.kind==TeachingFact::MoveSelected).count(),1);
}

#[test]
fn teaching_select_validation_errors_identify_the_missing_field_and_invalid_source() {
    let (_root,mut state)=fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"Explain whole-trip speed."}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let accepted=call(&state,&run.turn_ref,design_request("body-source",None));
    let mut request=select_request("body-source",accepted["design_ref"].as_str().unwrap(),false);
    request["move"]["kind"]=json!("explain");
    request["move"]["interaction_intent"]=json!("direct_explanation");
    request["move"]["intent_origin"]=json!("current_request");
    let missing_quote=teaching::preflight_step(&state.private_context(),&run.turn_ref,&request,&[],&[range()]).unwrap_err();
    assert_eq!(missing_quote.error.error_code,"TEACHING_INVALID");
    assert!(missing_quote.error.message.contains("move.intent_quote"),"{}",missing_quote.error.message);
    request["move"]["intent_quote"]=json!("Explain whole-trip speed.");
    let body="The stop belongs to the elapsed time. [[source:source_ref_missing]]";
    request["move"]["delivery_text"]=json!(body);
    let failure=teaching::preflight_step(&state.private_context(),&run.turn_ref,&request,&[],&[range()]).unwrap_err();
    assert_eq!(failure.error.error_code,"TEACHING_INVALID");
    assert!(failure.missing_source_read.is_none(),"an invalid citation is not a missing book read");
    assert!(failure.error.message.contains("move.delivery_text"),"{}",failure.error.message);
    let (_,diagnostics)=failure.error.message.split_once('\n').expect("keep the source compiler diagnostics");
    let issues:Vec<runtime::orchestrator::AnswerDeliveryIssue>=serde_json::from_str(diagnostics).unwrap();
    assert_eq!(issues.len(),1);
    assert_eq!(issues[0].error_code,"UNKNOWN_SOURCE_REF");
    assert_eq!(issues[0].trigger_value.as_deref(),Some("source_ref_missing"));
    assert_eq!(issues[0].start,body.find("[[source:"));
    assert_eq!(issues[0].end,Some(body.len()));
    let step_error=teaching::step(&state.private_context(),&run.turn_ref,request.clone(),&[],&[range()]).unwrap_err();
    assert_eq!(step_error.message,failure.error.message,"preflight and selection must expose the same repair information");
    request["move"]["delivery_text"]=json!("The stop belongs to the elapsed time.");
    let selected=call(&state,&run.turn_ref,request);
    assert!(selected["selected"].is_string(),"correcting the invalid body permits selection");
}

#[test]
fn teaching_select_preflight_inherits_the_same_target_as_selection() {
    let (_root,mut state)=fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"Explain whole-trip speed."}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let accepted=call(&state,&run.turn_ref,design_request("preflight-target",None));
    let mut request=select_request("inherit-target",accepted["design_ref"].as_str().unwrap(),false);
    request["move"].as_object_mut().unwrap().remove("target");
    let normalized=teaching::preflight_step(&state.private_context(),&run.turn_ref,&request,&[],&[range()]).unwrap();
    assert_eq!(normalized["move"]["target"],accepted["design"]["target"]);
    let selected=call(&state,&run.turn_ref,request);
    let saved=state.user.learning_store().unwrap().teaching_event(selected["selected"].as_str().unwrap()).unwrap();
    assert_eq!(saved.payload["move"],normalized["move"]);
}

#[test]
fn teaching_design_free_text_freezes_help_and_feedback_resolves_only_on_delivery() {
    let (_root,mut state)=fixture();
    let first=prepare_agent_chat(&mut state,r#"{"message":"Explain whole-trip speed."}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let accepted=call(&state,&first.turn_ref,design_request("first",None));
    let design_ref=accepted["design_ref"].as_str().unwrap();
    let selected=call(&state,&first.turn_ref,select_request("question",design_ref,true));
    complete(&mut state,&first.turn_ref,&selected,false);
    let invitation=state.user.learning_store().unwrap().teaching_deliveries(&first.turn_ref.session_id,&first.turn_ref.turn_id).unwrap().remove(0);
    let help=TeachingEvent {event_id:"shown-help".into(),binding:invitation.binding.clone(),kind:TeachingFact::HelpDisplayed,
        causal_refs:vec![invitation.event_id.clone()],payload:json!({"delivery_ref":invitation.event_id,"text":"Count the stopped minutes too."}),occurred_at:"before-input".into()};
    state.user.learning_store().unwrap().append_teaching(&help).unwrap();
    let response=prepare_agent_chat(&mut state,r#"{"message":"A moved faster, so A must have the greater average."}"#,"input").unwrap_or_else(|e|panic!("{}",e.body));
    let context=response.tutor.as_ref().unwrap();
    assert_eq!(context["learner_context"]["pending_response_invitation"]["delivery_ref"],invitation.event_id);
    let fact_ref=format!("usage:{}:user:{}",invitation.binding.tutor_session_id,response.turn_ref.turn_id);
    let fact=state.user.learning_store().unwrap().teaching_event(&fact_ref).unwrap();
    assert_eq!(fact.payload["assistance_refs"],json!(["shown-help"]));
    let mut later_help=help.clone();later_help.event_id="later-help".into();later_help.occurred_at="after-input".into();
    state.user.learning_store().unwrap().append_teaching(&later_help).unwrap();
    let evidence=call(&state,&response.turn_ref,json!({"operation":"evidence","nature":"hypothesis","operation_id":"reasoning",
        "target":target(),"design_ref":design_ref,"block_refs":["total-time"],"fact_refs":[fact_ref],
        "interpretation":"The response uses moving speed as sufficient for the whole-trip comparison; whether stopped time is included remains unresolved.",
        "teaching_implication":"Keep the same journeys and make A's stopped time part of the denominator."}));
    let stored=state.user.learning_store().unwrap().evidence(evidence["evidence_ref"].as_str().unwrap()).unwrap();
    assert_eq!(stored.assistance_refs,vec!["shown-help"]);
    let refreshed=&evidence["learner_context_update"];
    assert_eq!(refreshed["interpretations"][0]["evidence_ref"],evidence["evidence_ref"]);
    assert_eq!(refreshed["interpretations"][0]["assistance_refs"],json!(["shown-help"]));
    assert!(refreshed.get("recent_facts").is_none(),"fresh understanding must not replace the original input/help facts");
    assert!(refreshed.get("current_request").is_none());
    let mut feedback=select_request("feedback",design_ref,false);
    feedback["move"]["responds_to"]=json!({"fact_refs":[fact_ref],"invitation_ref":invitation.event_id,"disposition":"addressed"});
    feedback["move"]["interpretation_refs"]=json!([evidence["evidence_ref"]]);
    let selected=call(&state,&response.turn_ref,feedback);
    assert!(state.user.learning_store().unwrap().pending_response_invitation(&invitation.binding.tutor_session_id,&invitation.binding.source_id,&invitation.binding.source_revision).unwrap().is_some());
    complete(&mut state,&response.turn_ref,&selected,false);
    assert!(state.user.learning_store().unwrap().pending_response_invitation(&invitation.binding.tutor_session_id,&invitation.binding.source_id,&invitation.binding.source_revision).unwrap().is_none());
}

#[test]
fn teaching_design_evidence_revision_refreshes_the_current_view_without_rebinding_input() {
    let (_root,mut state)=fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"Moving faster means the whole trip is faster, I think."}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let accepted=call(&state,&run.turn_ref,design_request("before-evidence",None));
    assert_eq!(accepted["learner_context_update"]["interpretations"],json!([]));
    let binding=&run.tutor.as_ref().unwrap()["binding"];
    let fact_ref=format!("usage:{}:user:{}",binding["tutor_session_id"].as_str().unwrap(),run.turn_ref.turn_id);
    assert!(accepted["learner_context_update"]["uninterpreted_fact_refs"].as_array().unwrap().contains(&json!(fact_ref)));
    let mut request=json!({"operation":"evidence","nature":"hypothesis","operation_id":"initial-interpretation",
        "target":target(),"design_ref":accepted["design_ref"],"block_refs":["total-time"],"fact_refs":[fact_ref],
        "interpretation":"The learner equates moving speed with whole-trip speed.",
        "teaching_implication":"Show how stopped time changes the denominator."});
    let original=call(&state,&run.turn_ref,request.clone());
    assert_eq!(original["learner_context_update"]["interpretations"][0]["evidence_ref"],original["evidence_ref"]);
    assert!(!original["learner_context_update"]["uninterpreted_fact_refs"].as_array().unwrap().contains(&json!(fact_ref)));
    request["operation_id"]=json!("revised-interpretation");
    request["supersedes"]=original["evidence_ref"].clone();
    request["interpretation"]=json!("The learner tentatively equates the two; their words do not yet establish whether stopped time is excluded.");
    let revised=call(&state,&run.turn_ref,request);
    assert_eq!(revised["learner_context_update"]["interpretations"].as_array().unwrap().len(),1);
    assert_eq!(revised["learner_context_update"]["interpretations"][0]["evidence_ref"],revised["evidence_ref"]);
    assert!(revised["learner_context_update"]["evidence_watermark"].as_u64().unwrap()>original["learner_context_update"]["evidence_watermark"].as_u64().unwrap());
    assert_eq!(run.tutor.as_ref().unwrap()["learner_context"]["interpretations"],json!([]),"the original turn snapshot is immutable");
    assert!(run.tutor.as_ref().unwrap()["learner_context"]["uninterpreted_fact_refs"].as_array().unwrap().contains(&json!(fact_ref)),"only the dynamic view changes; the saved input snapshot stays unchanged");
    assert_eq!(state.user.learning_store().unwrap().teaching_event(&fact_ref).unwrap().learner_expression(),Some("Moving faster means the whole trip is faster, I think."));
}

#[test]
fn teaching_design_page_help_requires_visible_scene_and_follows_the_response() {
    use runtime::presentation::*;
    for invite in [true,false] {
    let (_root,mut state)=fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"Show the whole-trip comparison."}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let mut initial=design_request("page",None);
    initial["design"]["representation"]=json!({"kind":"presentation","reason":"Compare stopped and moving intervals in one scene.","observation":"Changing the stop changes elapsed time while moving time stays fixed."});
    let accepted=call(&state,&run.turn_ref,initial);
    let design_ref=accepted["design_ref"].as_str().unwrap().to_owned();
    let candidate=state.private_context().create_presentation_candidate(&run.turn_ref.session_id,&run.turn_ref.turn_id,None,
        PresentationContent {teaching_binding:Some(PresentationTeachingBinding {design_ref:design_ref.clone(),block_refs:vec!["total-time".into()],
                regions:vec![PresentationTeachingRegion {region_id:"compare".into(),block_refs:vec!["total-time".into()],state_fields:vec!["stop".into()],observation:"Stopped minutes are inside the journey.".into()}]}),method_refs:None,method_observations:Default::default(),animation_assets:Default::default(),
            title:"Two journeys".into(),content_files:BTreeMap::from([("index.html".into(),"<p>Count stopped minutes too.</p>".into())]),entrypoint:"index.html".into(),
            readable_content:"Compare elapsed time".into(),source_bindings:vec![],assumptions:vec![],state_contract:json!({}),initial_state:json!({"stop":5})}).unwrap();
    let reference=state.private_context().persist_presentation_candidate(&run.turn_ref.session_id,&run.turn_ref.turn_id,&candidate.candidate_id).unwrap();
    let mut request=select_request("page-support",accepted["design_ref"].as_str().unwrap(),invite);
    request["move"]["presentation"]=json!(reference);
    let selected=call(&state,&run.turn_ref,request);
    let mut outcome=super::tutor_loop_tests::outcome(Some(&reference));
    outcome.teaching_delivery_move_refs=serde_json::from_value(selected["delivery_move_refs"].clone()).unwrap();
    super::tutor_loop_tests::finish(&mut state,&run.turn_ref,&outcome);
    let delivery=state.user.learning_store().unwrap().teaching_deliveries(&run.turn_ref.session_id,&run.turn_ref.turn_id).unwrap().remove(0);
    let scene=state.private_context().save_presentation_state(&run.turn_ref.session_id,&run.turn_ref.turn_id,&reference,
        PresentationState {values:json!({"stop":5}),visible_step:Some("compare".into()),observed_result:"Count stopped minutes too.".into(),source_ref_ids:vec![]}).unwrap();
    let mut help=json!({"delivery_ref":delivery.event_id,"presentation_follow_up":scene,"kind":"hint","text":"This text was never visible."});
    let rejected=post(&mut state,"/tutor/page-help",&help.to_string());
    assert_ne!(rejected.status,200);
    help["text"]=json!("Count stopped minutes too.");
    let accepted=post(&mut state,"/tutor/page-help",&help.to_string());
    assert_eq!(accepted.status,200,"{}",accepted.body);
    let help_ref:Value=serde_json::from_str(&accepted.body).unwrap();
    let reply=prepare_agent_chat(&mut state,&json!({"message":"I included the stop: A takes 15 minutes, so B is faster overall.","presentation_follow_up":scene}).to_string(),"next").unwrap_or_else(|e|panic!("{}",e.body));
    let fact_ref=format!("usage:{}:user:{}",delivery.binding.tutor_session_id,reply.turn_ref.turn_id);
    let fact=state.user.learning_store().unwrap().teaching_event(&fact_ref).unwrap();
    assert_eq!(fact.payload["assistance_refs"],json!([help_ref["event_id"]]));
    assert_eq!(fact.payload["support_delivery_refs"],json!([delivery.event_id]));
    assert_eq!(fact.payload["invitation_context_ref"].is_string(),invite,"support-only help must not invent an invitation");
    // New browser state and a newer page revision exist before the focus changes.
    // The earlier expression must retain the exact scene it actually carried.
    let later_scene=state.private_context().save_presentation_state(&run.turn_ref.session_id,&run.turn_ref.turn_id,&reference,
        PresentationState {values:json!({"stop":9}),visible_step:Some("later".into()),observed_result:"Later help: try nine stopped minutes.".into(),source_ref_ids:vec![]}).unwrap();
    let later_help=post(&mut state,"/tutor/page-help",&json!({"delivery_ref":delivery.event_id,"presentation_follow_up":later_scene,
        "kind":"hint","text":"Later help: try nine stopped minutes."}).to_string());
    assert_eq!(later_help.status,200,"{}",later_help.body);
    let mut new_content=candidate.content.clone();
    new_content.title="A later version".into();
    new_content.readable_content="A different comparison".into();
    let newer=state.private_context().create_presentation_candidate(&reply.turn_ref.session_id,&reply.turn_ref.turn_id,Some(reference.clone()),new_content).unwrap();
    let newer_ref=state.private_context().persist_presentation_candidate(&reply.turn_ref.session_id,&reply.turn_ref.turn_id,&newer.candidate_id).unwrap();
    assert!(newer_ref.revision>reference.revision);
    let mut revision=design_request("return-scene",Some(&design_ref));
    revision["design"]["blocks"].as_array_mut().unwrap().push(json!({"block_id":"distance-time-relation","relation":"Whole-trip speed compares distance with all elapsed time.",
        "purpose":"Connect the observed denominator to speed.","source_bindings":[{"start_lid":"1.1","end_lid":"1.1"}]}));
    revision["design"]["focus_block_refs"]=json!(["distance-time-relation"]);
    revision["design"]["open_question_refs"]=json!([fact_ref]);
    for wrong in [&scene.saved_state_ref,&delivery.event_id] {
        revision["design"]["return_to"]=json!({"block_refs":["total-time"],"scene_ref":wrong});
        let error=teaching::step(&state.private_context(),&reply.turn_ref,revision.clone(),&[],&[range()]).unwrap_err();
        assert!(error.message.contains("return_to.scene_ref"));
    }
    revision["design"]["return_to"]["scene_ref"]=json!(fact_ref);
    let returned=call(&state,&reply.turn_ref,revision);
    assert_eq!(returned["design"]["return_to"]["scene_ref"],fact_ref);
    let return_context=&returned["learner_context_update"]["learning_opportunity"]["return_context"];
    assert_eq!(return_context["scene"]["fact_ref"],fact_ref);
    assert_eq!(return_context["scene"]["presentation_follow_up"],json!(scene));
    assert_eq!(return_context["scene"]["scene"]["receipt"],json!(scene));
    assert_eq!(return_context["scene"]["scene"]["presentation"]["reference"],json!(reference));
    assert_eq!(return_context["scene"]["scene"]["presentation"]["teaching_binding"]["design_ref"],design_ref);
    assert_eq!(return_context["scene"]["scene"]["values_preview"]["text"],r#"{"stop":5}"#);
    assert_eq!(return_context["scene"]["scene"]["visible_step"],"compare");
    assert_eq!(return_context["scene"]["scene"]["observed_result_preview"]["text"],"Count stopped minutes too.");
    if invite { assert_eq!(return_context["invitations"][0]["delivery_ref"],delivery.event_id); }
    else { assert_eq!(return_context["invitations"],json!([])); }
    assert!(return_context["support_refs"].as_array().unwrap().contains(&help_ref["event_id"]));
    let support=&returned["learner_context_update"]["learning_opportunity"]["delivered_support"][0];
    assert_eq!(support["presentation"],json!(reference));
    assert_eq!(support["presentation_support"]["teaching_binding"]["block_refs"],json!(["total-time"]));
    assert_eq!(support["learner_expressions"][0]["scene"]["receipt"],json!(scene));
    assert_eq!(support["learner_expressions"][0]["scene"]["presentation"]["reference"],json!(reference));
    assert_eq!(support["learner_expressions"][0]["assistance_refs"],json!([help_ref["event_id"]]),"later displayed help must not retroactively support this expression");
    let frozen=state.user.learning_store().unwrap().teaching_event(&fact_ref).unwrap();
    assert_eq!(frozen.payload["assistance_refs"],json!([help_ref["event_id"]]));
    let trace=call(&state,&reply.turn_ref,json!({"operation":"trace"}));
    assert!(trace["events"].as_array().unwrap().iter().any(|row|row["event"]["event_id"]==fact_ref && row["event"]["presentation_follow_up"]==json!(scene)),"trace must locate the original immutable page observation");
    }
}


fn chapter_fixture() -> (tempfile::TempDir, AppState) {
    use base_schema::{LidNode, NodeKind, Span};
    let source = "# First chapter\nAlpha relation.\n# Second chapter\nBeta relation.\n";
    let split = source.find("# Second").unwrap();
    let mut base = sample_base();
    base.lid_nodes = [("1", vec![1], NodeKind::Chapter, 0, split, vec!["1.1".into()]),
        ("1.1", vec![1,1], NodeKind::Paragraph, 16, split, vec![]),
        ("2", vec![2], NodeKind::Chapter, split, source.len(), vec!["2.1".into()]),
        ("2.1", vec![2,1], NodeKind::Paragraph, split+17, source.len(), vec![])]
        .into_iter().map(|(lid,path,kind,start,end,children)| LidNode { lid:lid.into(),path,kind,span:Span {start,end},children }).collect();
    let (root,mut state)=super::tutor_loop_tests::fixture_with_book(false,Book::new(base,source),source);
    // Each fixture chapter has one leaf; keep the visible window to one leaf
    // so goto can actually move its viewport anchor into the second chapter.
    state.workspace.reader=Reader::new(&state.workspace.book,0);
    (root,state)
}

#[test]
fn chapter_entry_resolves_real_heading_without_claiming_source_evidence() {
    use crate::interaction_input::InteractionInput;
    let (_root,state) = chapter_fixture();
    let input = json!({"message":"I know the terminology.","entry_action":"guide_chapter","chapter_lid":"1",
        "source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint()});
    let action = InteractionInput::parse(&input).unwrap();
    let Some(runtime::tutor_continuation::InteractionResolution::RunInput { interaction }) =
        action.resolve(&state.user,&state.workspace,&input,None).unwrap() else { panic!("expected run"); };
    assert_eq!(interaction.accepted_request,"带我读《First chapter》这一章");
    assert!(interaction.source_excerpt.is_none() && interaction.target.is_none());
    assert_eq!(interaction.source_bindings[0].start_lid,"1");
    let mut changed=input.clone(); changed["chapter_lid"]=json!("1.1");
    assert!(InteractionInput::parse(&changed).unwrap().resolve(&state.user,&state.workspace,&changed,None).is_err());
    changed=input.clone(); changed["source_revision"]=json!("old");
    assert_eq!(action.resolve(&state.user,&state.workspace,&changed,None).unwrap_err().error_code,"INTERACTION_STALE");
    let mut changed=input.clone();changed["chapter_lid"]=json!("2");
    assert_ne!(run_admission::normalize("local",&input).unwrap(),run_admission::normalize("local",&changed).unwrap(),
        "retrying an accepted identity with another chapter must conflict");
}

#[test]
fn chapter_scope_is_frozen_before_binding_and_survives_navigation_and_reopen() {
    let (root,mut state)=chapter_fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"带我读这一章。我知道术语，但不知道用途。"}"#,"now")
        .unwrap_or_else(|e|panic!("{}",e.body));
    let context=run.tutor.as_ref().unwrap();
    assert_eq!(context["chapter_scope"]["chapter_lid"],"1");
    let first=context["binding"]["tutor_session_id"].clone();
    assert_eq!(context["material_scope"][0]["scope_refs"],json!(["1"]));
    super::tutor_loop_tests::finish(&mut state,&run.turn_ref,&super::tutor_loop_tests::outcome(None));
    state.workspace.reader.goto_lid(&state.workspace.book,&mut state.user.store,"2.1","later").unwrap();
    assert_eq!(state.workspace.reader.state().viewport.anchor_lid,"2.1");
    state.user.store=MemoryStore::open(root.path().join("memory.json")).unwrap();
    {
        // A fresh SQLite connection rehydrates persisted TutorSession scope.
        let reopened=memory::learning::LearningStore::open(&root.path().join("learning.db"),false).unwrap();
        let persisted=reopened.state().unwrap();
        assert_eq!(persisted.sessions[first.as_str().unwrap()].material_scope[0].scope_refs,vec!["1"]);
    }
    let continued=prepare_agent_chat(&mut state,r#"{"message":"继续"}"#,"later")
        .unwrap_or_else(|e|panic!("{}",e.body));
    assert_eq!(continued.tutor.as_ref().unwrap()["chapter_scope"]["chapter_lid"],"1");
    assert_eq!(continued.tutor.as_ref().unwrap()["binding"]["tutor_session_id"],first);
    let chat=state.user.agent_history.sessions.iter().find(|s| s.id == continued.turn_ref.session_id).unwrap();
    assert_eq!(chat.goals.last().unwrap().related_goal_id.as_deref(),chat.turns[0].goal_ref.as_ref().map(|r|r.id.as_str()));
    super::tutor_loop_tests::finish(&mut state,&continued.turn_ref,&super::tutor_loop_tests::outcome(None));
    let changed=prepare_agent_chat(&mut state,r#"{"message":"带我读这一章"}"#,"later")
        .unwrap_or_else(|e|panic!("{}",e.body));
    assert_eq!(changed.tutor.as_ref().unwrap()["chapter_scope"]["chapter_lid"],"2");
    let second=changed.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].clone();
    assert_ne!(second,first);
    super::tutor_loop_tests::finish(&mut state,&changed.turn_ref,&super::tutor_loop_tests::outcome(None));
    let head=state.user.agent_history.sessions.iter().find(|s|Some(s.id.as_str())==state.workspace.selected_chat.as_deref())
        .and_then(|s|s.turns.last()).map(|t|&t.turn_id);
    let ui=json!({"message":"","entry_action":"guide_chapter","chapter_lid":"2","session_id":state.workspace.selected_chat,
        "client_request_id":"same-chapter","expected_chat_head":head,
        "source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint()});
    let same=prepare_agent_chat(&mut state,&ui.to_string(),"later").unwrap_or_else(|e|panic!("{}",e.body));
    assert_eq!(same.tutor.as_ref().unwrap()["binding"]["tutor_session_id"],second);
    assert_eq!(same.tutor.as_ref().unwrap()["current_action"]["accepted_request"],"带我读《Second chapter》这一章");
    assert!(same.agent_message.contains("带我读《Second chapter》这一章"));
}

#[test]
fn chapter_natural_request_while_tutor_off_preserves_ordinary_reading() {
    for paused in [false,true] {
        let (_root,mut state)=chapter_fixture();
        let before=state.user.learning_store().unwrap().state().unwrap();
        let action=memory::learning::TutorMutation { operation_id:"chapter-inactive".into(),
            expected_revision:before.control.revision, action:if paused {
                memory::learning::TutorAction::Pause { session_id:before.control.current_tutor_session_id.clone().unwrap() }
            } else { memory::learning::TutorAction::SetEnabled { enabled:false } } };
        state.user.learning_store().unwrap().mutate(&action,"now").unwrap();
        let run=prepare_agent_chat(&mut state,r#"{"message":"带我读这一章"}"#,"later")
            .unwrap_or_else(|e|panic!("{}",e.body));
        assert_eq!(run.tutor.as_ref().unwrap()["status"],"observing");
        let after=state.user.learning_store().unwrap().state().unwrap();
        assert_eq!(after.control.enabled,paused);
        assert_eq!(after.sessions.len(),before.sessions.len());
        assert_eq!(after.control.current_tutor_session_id,before.control.current_tutor_session_id);
        let session_id=before.control.current_tutor_session_id.as_ref().unwrap();
        assert_eq!(after.sessions[session_id].material_scope,before.sessions[session_id].material_scope);
        assert_eq!(after.sessions[session_id].status,memory::learning::TutorSessionStatus::Paused);
    }
}

#[test]
fn chapter_continuation_does_not_reopen_an_ordinary_completed_goal() {
    let (_root,mut state)=chapter_fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"Explain average speed."}"#,"now")
        .unwrap_or_else(|e|panic!("{}",e.body));
    assert!(run.tutor.as_ref().unwrap()["chapter_scope"].is_null());
    super::tutor_loop_tests::finish(&mut state,&run.turn_ref,&super::tutor_loop_tests::outcome(None));
    let reply=prepare_agent_chat(&mut state,r#"{"message":"继续"}"#,"later").err().expect("ordinary completed Goal stays completed");
    let error:Value=serde_json::from_str(&reply.body).unwrap();
    assert_eq!(error["error_code"],"GOAL_NOT_FOUND");
    // Starting a chapter does not make an earlier ordinary turn part of it.
    let mut learning = state.user.learning_store().unwrap();
    let view = learning.state().unwrap();
    learning.mutate(&memory::learning::TutorMutation { operation_id:"new-chapter-after-answer".into(),
        expected_revision:view.control.revision, action:memory::learning::TutorAction::Start {
            user_intent:"Read first chapter".into(),explicit_constraints:vec![],default_teaching_intent:None,
            material_scope:vec![memory::learning::TutorMaterial { source_id:state.workspace.book.base.book_id.clone(),
                scope_refs:vec!["1".into()],role:memory::learning::TutorMaterialRole::Primary }] } },"later").unwrap();
    let reply=prepare_agent_chat(&mut state,r#"{"message":"继续"}"#,"later").err().expect("previous turn has no binding to this chapter");
    let error:Value=serde_json::from_str(&reply.body).unwrap();
    assert_eq!(error["error_code"],"GOAL_NOT_FOUND");
}

#[test]
fn chapter_natural_request_uses_admitted_anchor_before_window_center() {
    for input_anchor in [None,Some("2.1")] {
        let (_root,mut state)=chapter_fixture();
        state.workspace.reader=Reader::new(&state.workspace.book,DEFAULT_RADIUS);
        state.workspace.reader.goto_lid(&state.workspace.book,&mut state.user.store,"2.1","now").unwrap();
        assert_eq!(state.workspace.reader.state().viewport.anchor_lid,"1.1");
        let mut input=json!({"message":"带我读这一章"});
        if let Some(anchor)=input_anchor { input["question_anchor_lid"]=json!(anchor); }
        let run=prepare_agent_chat(&mut state,&input.to_string(),"later")
            .unwrap_or_else(|e|panic!("{}",e.body));
        assert_eq!(run.tutor.as_ref().unwrap()["chapter_scope"]["chapter_lid"],if input_anchor.is_some() { "2" } else { "1" });
    }
}

#[test]
fn agent_work_review_reopens_into_next_turn_separate_from_learner_context() {
    let (root,mut state)=fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"Explain whole-trip average speed."}"#,"first")
        .unwrap_or_else(|e|panic!("{}",e.body));
    let design=call(&state,&run.turn_ref,design_request("agent-work",None));
    let rejected=json!({"round":1,"original_request":"Explain whole-trip average speed.",
        "current_design":design,"proposed_delivery":{"text":"Only count moving time.","moves":[]},
        "source_evidence":[{"lid":"1.1","text":"Average speed is total distance divided by total time."}],
        "review":{"decision":"revise","revision":"The denominator must include stopped time.",
            "agent_work":{"understanding":[{"statement":"Stopped time belongs in total elapsed time.","sources":[{"lid":"1.1","quote":"total time"}]}],"open_questions":[]}},
        "changes_since_previous_review":[]});
    let first=teaching::record_agent_review(&state.private_context(),&run.turn_ref,rejected.clone()).unwrap();
    let rejected_ref=first["current_work"]["review_ref"].as_str().unwrap().to_owned();
    assert_eq!(first["corrections"][0]["review_status"],"revision_requested");
    let mut accepted=rejected.clone();accepted["round"]=json!(2);
    accepted["review"]["decision"]=json!("accept");accepted["review"]["revision"]=Value::Null;
    accepted["proposed_delivery"]["text"]=json!("Count all elapsed time, including stops.");
    accepted["changes_since_previous_review"]=json!([{"path":"delivery.text","before":"Only count moving time.","after":"Count all elapsed time, including stops."}]);
    teaching::record_agent_review(&state.private_context(),&run.turn_ref,accepted).unwrap();
    let mut selection=select_request("corrected",design["design_ref"].as_str().unwrap(),false);
    selection["move"]["delivery_text"]=json!("Count all elapsed time, including stops.");
    let selected=call(&state,&run.turn_ref,selection);
    complete(&mut state,&run.turn_ref,&selected,false);
    state.user.store=MemoryStore::open(root.path().join("memory.json")).unwrap();
    let next=prepare_agent_chat(&mut state,r#"{"message":"So the stop counts too?"}"#,"second")
        .unwrap_or_else(|e|panic!("{}",e.body));
    let context=next.tutor.as_ref().unwrap();
    assert!(context["learner_context"].get("agent_working_state").is_none());
    assert_eq!(context["agent_working_state"]["current_work"]["review_status"],"accepted_by_review");
    assert_eq!(context["agent_working_state"]["corrections"][0]["review_ref"],rejected_ref);
    assert_eq!(context["agent_working_state"]["current_work"]["delivered_refs"].as_array().unwrap().len(),1);
    assert!(context["learner_context"]["interpretations"].as_array().unwrap().is_empty());
    assert!(!context["learner_context"]["uninterpreted_fact_refs"].as_array().unwrap().contains(&json!(rejected_ref)));
    let trace=call(&state,&next.turn_ref,json!({"operation":"trace","event_id":rejected_ref}));
    let saved:Value=serde_json::from_str(trace["chunk"].as_str().unwrap()).unwrap();
    assert_eq!(saved["content"]["source_evidence"],rejected["source_evidence"]);
    assert_eq!(saved["content"]["proposed_delivery"],rejected["proposed_delivery"]);
}
