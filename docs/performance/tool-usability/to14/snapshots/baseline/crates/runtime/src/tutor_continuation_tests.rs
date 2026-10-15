use crate::tutor_continuation::*;
use serde_json::{json, Value};

fn action_context(action: &str) -> Value {
    json!({"status":"active","current_action":{
        "action":{"origin":"ui_entry","entry_action":action},"user_supplement":"",
        "accepted_request":"UI request","answer_ref":null,"answer_text":null,
        "source_excerpt":null,"source_bindings":[],"target":null,
        "expected_chat_head":null,"presentation_follow_up":null}})
}

#[test]
fn tutor_projection_exposes_only_current_operations() {
    for (status, operations) in [("observing",json!(["trace","understanding","evidence"])), ("reference",json!(["trace"]))] {
        let context = json!({"status":status});
        let registry = crate::orchestrator::resident_tool_registry().with_tutor_context(Some(&context));
        let registration = registry.registration("tutor.step").unwrap();
        let schema = &registration.spec.parameters;
        assert_eq!(schema["properties"]["operation"]["enum"], operations);
        assert!(schema["properties"].get("move").is_none());
        assert!(registration.validate_arguments(r#"{"operation":"trace"}"#).is_ok());
        assert!(registration.validate_arguments(&json!({"operation":"select","move":movement()}).to_string()).is_err());
        assert!(registration.validate_arguments(r#"{"operation":"evidence","action_ref":"a","object_id":"o","operation_id":"e","response_quote":"q","sources":[],"interpretation":"i"}"#).is_err());
        if status == "observing" {
            assert!(registration.validate_arguments(&json!({"operation":"evidence","nature":"hypothesis","operation_id":"h",
                "target":{"learning_focus":"time","expected_performance":"explain time","capability":"explanation",
                    "source_bindings":[{"source_id":"book","source_revision":"r","start_lid":"1.1","end_lid":"1.1"}],"object_refs":[]},
                "fact_refs":["user-question"],"interpretation":"Question concerns time.","teaching_implication":"Explain elapsed time."}).to_string()).is_ok());
        }
        assert_eq!(registry.visible_specs().into_iter().find(|s| s.name == "tutor.step").unwrap().parameters, *schema);
    }
    for status in ["paused","inactive"] {
        assert!(crate::orchestrator::resident_tool_registry().with_tutor_context(Some(&json!({"status":status}))).registration("tutor.step").is_none());
    }
}

#[test]
fn tutor_action_intent_preserves_request_default_and_preference_precedence() {
    use crate::tutor::action_intent;
    let mut context = action_context("explain_selection");
    context["default_teaching_intent"] = json!("guided_inquiry");
    assert_eq!(action_intent(&context), Some(("direct_explanation","current_action")));
    context["current_action"]["action"]["entry_action"] = json!("continue_answer");
    assert_eq!(action_intent(&context), Some(("guided_inquiry","session_default")));
    context["default_teaching_intent"] = json!("direct_explanation");
    assert_eq!(action_intent(&context), Some(("direct_explanation","session_default")));
    context["default_teaching_intent"] = Value::Null;
    assert_eq!(action_intent(&context), Some(("neutral","current_action")));
    context["confirmed_preferences"] = json!([{"fact_id":"p","preference":"Ask me to compare first."}]);
    assert!(action_intent(&context).is_none());
    let registry = crate::orchestrator::resident_tool_registry().with_tutor_context(Some(&context));
    assert!(registry.registration("tutor.step").unwrap().spec.parameters["properties"]["move"]["properties"].get("preference_ref").is_some());
    context["default_teaching_intent"] = json!("guided_inquiry");
    context["current_action"]["user_supplement"] = json!("Explain directly this time.");
    assert!(action_intent(&context).is_none());
    for context in [context,json!({"status":"active","current_request":"Explain directly."})] {
        let registry = crate::orchestrator::resident_tool_registry().with_tutor_context(Some(&context));
        let registration = registry.registration("tutor.step").unwrap();
        let fields = &registration.spec.parameters["properties"]["move"]["properties"];
        assert!(fields.get("intent_quote").is_some());
        assert!(fields.get("continuation").is_some());
        let mut value = movement();
        if context["current_action"].is_object() {
            value.as_object_mut().unwrap().remove("capability");
            value["target"] = json!({"learning_focus":"time","expected_performance":"explain time","capability":"explanation",
                "source_bindings":[{"source_id":"book","source_revision":"r","start_lid":"1.1","end_lid":"1.1"}],"object_refs":[]});
        }
        assert!(registration.validate_arguments(&json!({"operation":"select","move":value}).to_string()).is_ok());
    }
}

#[test]
fn tutor_projection_pure_click_owns_intent_and_accepts_optional_continuation() {
    for action in ["explain_selection", "continue_answer"] {
        let context = action_context(action);
        let registry = crate::orchestrator::resident_tool_registry().with_tutor_context(Some(&context));
        let registration = registry.registration("tutor.step").unwrap();
        let mut value = movement();
        value["target"] = json!({"learning_focus":"time","expected_performance":"explain time","capability":"explanation",
            "source_bindings":[{"source_id":"book","source_revision":"r","start_lid":"1.1","end_lid":"1.1"}],"object_refs":[]});
        for key in ["interaction_intent","intent_origin","intent_quote","preference_ref","capability"] {
            assert!(registration.spec.parameters["properties"]["move"]["properties"].get(key).is_none(), "{key}");
            value.as_object_mut().unwrap().remove(key);
        }
        assert!(registration.validate_arguments(&json!({"operation":"select","move":value}).to_string()).is_ok());
        let mut no_target = value.clone(); no_target.as_object_mut().unwrap().remove("target");
        assert!(registration.validate_arguments(&json!({"operation":"select","move":no_target}).to_string()).is_err());
        value["continuation"] = continuation();
        assert!(registration.validate_arguments(&json!({"operation":"select","move":value}).to_string()).is_ok());
    }
}

fn continuation() -> Value {
    json!({"recommended_option_id":"example","options":[
        {"option_id":"example","label":"算一次总路程除以总时间","purpose":"看清分母的含义","move_kind":"explain"}]})
}
fn movement() -> Value {
    json!({"move_id":"explain","object_ids":[],"capability":"explanation","kind":"explain",
        "interaction_intent":"neutral","intent_origin":"neutral","prompt":"平均速度"})
}
fn accepts(value: &Value) -> bool {
    crate::orchestrator::resident_tool_registry().registration("tutor.step").unwrap()
        .validate_arguments(&json!({"operation":"select","move":value}).to_string()).is_ok()
}

#[test]
fn continuation_contract_runtime_and_schema_reject_invalid_options() {
    let mut good = movement();
    good["continuation"] = continuation();
    let validate = |v: &Value| serde_json::from_value::<TeachingContinuation>(v.clone())
        .map(|c| c.validate().is_ok()).unwrap_or(false);
    assert!(accepts(&good));
    assert!(validate(&good["continuation"]));
    let mut invalids = vec![];
    let mut c = continuation(); c["options"] = json!([]); invalids.push(c);
    let mut c = continuation(); c["options"] = json!([c["options"][0],c["options"][0]]); invalids.push(c);
    let mut c = continuation(); c["recommended_option_id"] = json!("missing"); invalids.push(c);
    let mut c = continuation(); c["options"][0]["purpose"] = json!("  "); invalids.push(c);
    let mut c = continuation(); c["options"][0]["move_kind"] = json!("reader_write"); invalids.push(c);
    let mut c = continuation(); c["options"][0]["target"] = json!({"source_id":"another-book"}); invalids.push(c);
    let mut c = continuation(); c["options"][0]["tool_call"] = json!({"name":"reader.note"}); invalids.push(c);
    let mut c = continuation(); c["options"][0]["assessment"] = json!({"answer":"secret"}); invalids.push(c);
    let mut c = continuation(); c["options"][0]["label"] = json!("文".repeat(CONTINUATION_TEXT_MAX+1)); invalids.push(c);
    let mut c = continuation(); c["options"][0]["purpose"] = json!("文".repeat(CONTINUATION_TEXT_MAX+1)); invalids.push(c);
    let mut c = continuation(); c["options"][0]["option_id"] = json!("a".repeat(CONTINUATION_ID_MAX+1)); invalids.push(c);
    let mut c = continuation(); c["options"] = json!([
        {"option_id":"example","label":"a","purpose":"a","move_kind":"explain"},
        {"option_id":"b","label":"b","purpose":"b","move_kind":"compare"},
        {"option_id":"c","label":"c","purpose":"c","move_kind":"observe"}]); invalids.push(c);
    for c in invalids {
        assert!(!validate(&c), "serde/domain accepted {c}");
        good["continuation"] = c;
        assert!(!accepts(&good), "registry accepted {good}");
    }
    let mut c = continuation();
    c["options"][0]["label"] = json!("文".repeat(CONTINUATION_TEXT_MAX));
    c["options"][0]["purpose"] = json!("文".repeat(CONTINUATION_TEXT_MAX));
    c["options"][0]["option_id"] = json!("a".repeat(CONTINUATION_ID_MAX));
    c["recommended_option_id"] = c["options"][0]["option_id"].clone();
    c["options"].as_array_mut().unwrap().push(json!({"option_id":"b","label":"b","purpose":"b","move_kind":"observe"}));
    assert!(validate(&c));
    good["continuation"] = c;
    assert!(accepts(&good));
    let parsed: crate::tutor::TeachingMove = serde_json::from_value(good.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap()["continuation"], good["continuation"]);
}

#[test]
fn continuation_contract_legacy_move_and_public_allowlist() {
    let old = movement();
    let parsed: crate::tutor::TeachingMove = serde_json::from_value(old).unwrap();
    assert!(parsed.continuation.is_none());
    assert!(serde_json::to_value(parsed).unwrap().get("continuation").is_none());
    let mut input = movement(); input["continuation"] = continuation();
    input["assessment"] = json!({"private":"secret"}); input["hint"] = json!("secret help");
    input["explanation"] = json!("secret answer");
    let public: Value = serde_json::from_str(&crate::tutor::public_arguments(&json!({"move":input}).to_string())).unwrap();
    assert!(public["move"].get("continuation").is_none(), "candidate must not appear in public tool traces");
    for malformed in [json!({"operation":"select","continuation":continuation()}),
        json!({"operation":"select","move":input.to_string()})] {
        let public = crate::tutor::public_arguments(&malformed.to_string());
        assert!(!public.contains("continuation"), "malformed candidates remain private");
    }
    for key in ["assessment","hint","explanation"] { assert!(public["move"].get(key).is_none()); }
    let view = json!({"delivery_ref":"delivery","origin_turn_id":"turn","target_summary":"速度",
        "continuation":continuation(),"expected_chat_head":"turn","availability":{"status":"eligible"}});
    assert!(serde_json::from_value::<TutorContinuationView>(view.clone()).is_ok());
    for key in ["assessment","hint","explanation","tool_call"] {
        let mut invalid = view.clone(); invalid[key] = json!("secret");
        assert!(serde_json::from_value::<TutorContinuationView>(invalid).is_err());
    }
    let registry = crate::orchestrator::resident_tool_registry();
    let registration = registry.registration("tutor.step").unwrap();
    assert!(!registration.capabilities.contains(&crate::tool_registry::LegacyToolCapability::ReaderWrite));
}

#[test]
fn continuation_contract_action_origins_and_direction_are_distinct() {
    use ts_rs::TS;
    assert!(InteractionAction::decl().contains("\"origin\": \"ui_entry\""));
    assert!(InteractionResolution::decl().contains("\"kind\": \"run_input\""));
    assert!(ContinuationAvailability::decl().contains("\"status\": \"consumed\""));
    let mut value = movement(); value["intent_origin"] = json!("current_action");
    assert!(accepts(&value));
    assert!(serde_json::from_value::<EntryAction>(json!("arbitrary_tool")).is_err());
    let snapshot: ResolvedInteraction = serde_json::from_value(json!({
        "action":{"origin":"ui_entry","entry_action":"clarify_answer"},"user_supplement":"",
        "accepted_request":"这一步没跟上","answer_ref":{"turn_id":"old","excerpt":"平均"},
        "answer_text":"平均速度","source_excerpt":null,"source_bindings":[],"target":null,
        "expected_chat_head":"current","presentation_follow_up":null})).unwrap();
    assert!(snapshot.validates_current_action("direct_explanation","explain"));
    assert!(!snapshot.validates_current_action("guided_inquiry","question"));
    let mut continuation = snapshot.clone();
    continuation.action = InteractionAction::UiEntry { entry_action:EntryAction::ContinueAnswer };
    assert!(continuation.validates_current_action("neutral","explain"));
    assert!(!continuation.validates_current_action("direct_explanation","explain"));
    for (kind, intent, name) in [(ContinuationMoveKind::Explain,"direct_explanation","explain"),
        (ContinuationMoveKind::Compare,"neutral","compare"), (ContinuationMoveKind::Observe,"neutral","observe")] {
        let mut choice = snapshot.clone();
        choice.action = InteractionAction::UiChoice { teaching_choice:TeachingChoice { delivery_ref:"original".into(),option_id:name.into() }, move_kind:kind };
        assert!(choice.validates_current_action(intent,name));
        assert!(!choice.validates_current_action("guided_inquiry","question"));
        assert_eq!(crate::tutor::action_intent(&json!({"current_action":choice})),Some((intent,"current_action")));
        choice.user_supplement = "Use my revised request".into();
        assert_eq!(crate::tutor::action_intent(&json!({"current_action":choice})),None);
    }
    let encoded = serde_json::to_value(InteractionResolution::RunInput { interaction:snapshot.clone() }).unwrap();
    assert_eq!(encoded["kind"], "run_input");
    assert_eq!(encoded["interaction"]["answer_ref"]["turn_id"], "old");
    assert_eq!(encoded["interaction"]["expected_chat_head"], "current");
    assert_eq!(serde_json::from_value::<InteractionResolution>(encoded).unwrap(), InteractionResolution::RunInput { interaction:snapshot });
    assert_eq!(serde_json::to_value(InteractionResolution::ActivityHelp { delivery_refs: vec!["d".into()] }).unwrap(),
        json!({"kind":"activity_help","delivery_refs":["d"]}));
}
