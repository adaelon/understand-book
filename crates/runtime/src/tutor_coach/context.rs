//! A model view of a frozen coach job. The durable job keeps the complete audit
//! trail; this view removes copies of facts already supplied in canonical fields.
use serde_json::Value;
use std::collections::BTreeSet;

pub fn model_input(input: &Value) -> Value {
    let input = if super::is_course_input(input) { input.clone() } else { observation_input(input) };
    let mut view = input.clone();
    let has_delivery = input["actual_delivery"].is_object();
    let has_observations = input["observation_facts"].is_array();
    // A newly revised design wins; otherwise carry the accepted design once.
    if view["current_design"].is_null() {
        if let Some(design) = input.pointer("/teaching_context/current_design").filter(|v| !v.is_null()) {
            view["current_design"] = design.clone();
        }
    }
    let has_design = !view["current_design"].is_null();
    let grounding = &input["teaching_context"]["learner_context"]["learner_grounding"];
    let mut supplied_facts = BTreeSet::<String>::new();
    for (items, field) in [(&input["turn_facts"], "event_id"), (&grounding["expressions"], "fact_ref"),
        (&grounding["delivered_support"], "delivery_ref")] {
        supplied_facts.extend(items.as_array().into_iter().flatten().filter_map(|v|v[field].as_str()).map(str::to_owned));
    }
    if let Some(context) = view.get_mut("teaching_context").and_then(Value::as_object_mut) {
        // Keep any older fact not supplied in the canonical views. Grounding
        // follows evidence links; it is not necessarily the entire recent past.
        retain_unsupplied(context.get_mut("recent_facts"), &mut supplied_facts);
        if has_design { context.remove("current_design"); }
        if let Some(learner) = context.get_mut("learner_context").and_then(Value::as_object_mut) {
            if has_design { learner.remove("current_design"); }
            if input["known_focuses"].is_array() { learner.remove("known_focuses"); }
            retain_unsupplied(learner.get_mut("recent_facts"), &mut supplied_facts);
            if let Some(support) = learner.get_mut("learning_opportunity")
                .and_then(|v|v.get_mut("delivered_support")).and_then(Value::as_array_mut) {
                for delivery in support {
                    if let Some(full) = grounding["delivered_support"].as_array().into_iter().flatten()
                        .find(|v| !delivery["delivery_ref"].is_null() && v["delivery_ref"] == delivery["delivery_ref"]) {
                        if let Some(fields) = delivery.as_object_mut() {
                            if full["answer"].is_string() { fields.remove("answer_preview"); }
                            for field in ["target", "participation", "response_expectation"] {
                                if fields.get(field) == full["move"].get(field) { fields.remove(field); }
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(delivery) = view.get_mut("actual_delivery").and_then(Value::as_object_mut) {
        if let Some(answer_view) = delivery.remove("answer_view") {
            // Rich media/activities can add facts absent from the plain answer.
            let parts: Vec<_> = answer_view["parts"].as_array().into_iter().flatten()
                .filter(|part| !matches!(part["kind"].as_str(), Some("markdown" | "text")))
                .cloned().collect();
            if !parts.is_empty() { delivery.insert("parts".into(), Value::Array(parts)); }
        }
    }
    if let Some(facts) = view.get_mut("turn_facts").and_then(Value::as_array_mut) {
        for fact in facts {
            let kind = fact["kind"].as_str().unwrap_or("").to_owned();
            if let Some(payload) = fact.get_mut("payload").and_then(Value::as_object_mut) {
                if kind == "turn_bound" && input["teaching_context"].is_object() {
                    payload.clear();
                    payload.insert("context_ref".into(), Value::String("teaching_context".into()));
                    continue;
                }
                if kind == "design_revised" && has_design {
                    payload.clear();
                    payload.insert("context_ref".into(), Value::String("current_design".into()));
                    continue;
                }
                if has_delivery && (kind == "message_delivered" || payload.get("usage").is_some_and(|v| v["kind"] == "assistant_delivery")) {
                    for field in ["answer", "answer_view", "source_bindings"] { payload.remove(field); }
                    payload.insert("delivery_ref".into(), Value::String("actual_delivery".into()));
                }
                if has_observations && payload.get("usage").is_some_and(|v| v["kind"] == "learner_expression") {
                    payload.remove("response");
                }
            }
        }
    }
    // The verifier needs source bindings/receipts, while the model needs the
    // actual read_sources, which already include their exact ranges and IDs.
    if input["read_sources"].is_array() {
        if let Some(fields) = view.as_object_mut() {
            fields.remove("source_bindings");
            fields.remove("verified_read_ranges");
        }
        deduplicate_sources(&mut view);
    }
    // Provider identity and publication admission are host bookkeeping, not
    // learner evidence. Candidate references retain their publication identity.
    if let Some(fields) = view.as_object_mut() {
        fields.remove("provider_binding");
    }
    reuse_delivered_answers(&mut view);
    view
}

/// Ordinary observations judge the learner at submission. The host still keeps
/// the completed answer, but it becomes help only for a later expression.
fn observation_input(input: &Value) -> Value {
    let mut view = serde_json::Map::new();
    for field in ["trigger", "source_turn_ref", "current_request", "user_request", "teaching_context", "observation_facts",
        "dialogue", "known_focuses", "personal_notes", "read_sources", "knowledge_candidates",
        "music_question", "video_question", "workspace_goal", "publication"] {
        if let Some(value) = input.get(field) { view.insert(field.into(), value.clone()); }
    }
    // teaching_context is frozen when the learner submits. turn_facts,
    // actual_delivery and current_design also contain work done afterwards;
    // observation_facts already supplies the original learner facts and help.
    if let Some(dialogue) = view.get_mut("dialogue").and_then(Value::as_array_mut) {
        let current = input["source_turn_ref"].as_str();
        let expressions: BTreeSet<_> = input["observation_facts"].as_array().into_iter().flatten()
            .filter_map(|fact| fact["binding"]["turn_id"].as_str()).collect();
        dialogue.retain(|turn| turn["turn_ref"].as_str().is_some_and(|id|
            Some(id) != current && !expressions.contains(id)));
    }
    Value::Object(view)
}

fn reuse_delivered_answers(view: &mut Value) {
    let mut answers: Vec<(String, String)> = vec![];
    if let Some(answer) = view["actual_delivery"]["answer"].as_str().filter(|s| !s.is_empty()) {
        answers.push((answer.into(), "actual_delivery".into()));
    }
    if let Some(dialogue) = view.get_mut("dialogue").and_then(Value::as_array_mut) {
        for turn in dialogue {
            if replace_answer(turn, &answers) { continue; }
            if let (Some(answer), Some(id)) = (turn["answer"].as_str(), turn["turn_ref"].as_str()) {
                if !answer.is_empty() { answers.push((answer.into(), format!("dialogue:{id}"))); }
            }
        }
    }
    if let Some(facts) = view.get_mut("observation_facts").and_then(Value::as_array_mut) {
        for fact in facts {
            for help in fact.get_mut("prior_help").and_then(Value::as_array_mut).into_iter().flatten() {
                replace_answer(help, &answers);
            }
        }
    }
    if let Some(support) = view.pointer_mut("/teaching_context/learner_context/learner_grounding/delivered_support").and_then(Value::as_array_mut) {
        for help in support { replace_answer(help, &answers); }
    }
}

fn replace_answer(value: &mut Value, answers: &[(String, String)]) -> bool {
    let Some(answer) = value["answer"].as_str() else { return false; };
    let Some((_, reference)) = answers.iter().find(|(text, _)|text == answer) else { return false; };
    let fields = value.as_object_mut().unwrap();
    fields.remove("answer");
    fields.insert("answer_ref".into(), Value::String(reference.clone()));
    true
}

fn retain_unsupplied(facts: Option<&mut Value>, supplied: &mut BTreeSet<String>) {
    if let Some(facts) = facts.and_then(Value::as_array_mut) {
        facts.retain(|fact| fact["event_id"].as_str().is_none_or(|id| supplied.insert(id.into())));
    }
}

fn deduplicate_sources(input: &mut Value) {
    let Some(sources) = input.get_mut("read_sources").and_then(Value::as_array_mut) else { return; };
    let original = std::mem::take(sources);
    for (i, source) in original.iter().enumerate() {
        let redundant = original.iter().enumerate().any(|(j, other)| {
            if i == j || source.get("source_ref_id").is_some() { return false; }
            if source == other { return j < i; }
            // A full parent passage also supplies its exact child excerpt.
            // Never apply this to a partial parent or identical text elsewhere.
            let (Some(parent), Some(start), Some(end), Some(text), Some(full)) = (
                other["range"]["start_lid"].as_str(), source["range"]["start_lid"].as_str(),
                source["range"]["end_lid"].as_str(), source["text"].as_str(), other["text"].as_str(),
            ) else { return false; };
            let within = |lid: &str| lid == parent || lid.strip_prefix(parent).is_some_and(|tail| tail.starts_with('.'));
            other["range"]["end_lid"].as_str() == Some(parent)
                && other["range"].get("ranges").is_none_or(|v| v.as_array().is_some_and(Vec::is_empty))
                && within(start) && within(end) && !text.is_empty() && full.len() > text.len() && full.contains(text)
        });
        if !redundant { sources.push(source.clone()); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn course_projection_keeps_expression_help_and_nontext_delivery() {
        let input = json!({"course_response":{"activity":"reflection"},"observation_facts":[{"fact_ref":"f","user_expression":"我觉得还要算停车时间",
            "prior_help":[{"event_id":"prior","answer":"先前的解释"}],"already_interpreted":false}],
            "actual_delivery":{"relative_to_expression":"after","answer":"刚刚给出的答案",
                "answer_view":{"parts":[{"kind":"markdown","text":"刚刚给出的答案"},{"kind":"presentation","id":"slide"}]}},
            "teaching_context":{"current_design":{"design_ref":"d"},"recent_facts":["duplicate"],
                "learner_context":{"current_design":{"design_ref":"d"},"recent_facts":["duplicate"],
                    "learner_grounding":{"expressions":["earlier"],"delivered_support":["prior help"]},
                    "learning_opportunity":{"delivered_support":["copy"],"pending_response_invitation":"question"}}},
            "turn_facts":[{"kind":"turn_bound","payload":{"huge":"duplicate context"}},
                {"kind":"message_delivered","payload":{"answer":"duplicate","move":{"response_expectation":"invitation"}}}]});
        let view = model_input(&input);
        assert_eq!(view["observation_facts"], input["observation_facts"]);
        assert_eq!(view["actual_delivery"]["relative_to_expression"], "after");
        assert_eq!(view["actual_delivery"]["parts"], json!([{"kind":"presentation","id":"slide"}]));
        assert_eq!(view["turn_facts"][1]["payload"]["move"]["response_expectation"], "invitation");
        assert_eq!(view["teaching_context"]["learner_context"]["learner_grounding"], input["teaching_context"]["learner_context"]["learner_grounding"]);
        assert_eq!(view["current_design"]["design_ref"], "d");
        assert_eq!(view["teaching_context"]["learner_context"]["learning_opportunity"]["pending_response_invitation"], "question");
        assert_eq!(input["turn_facts"][0]["payload"]["huge"], "duplicate context", "frozen job stays unchanged");
    }

    #[test]
    fn coach_source_dedup_does_not_expand_partial_or_unrelated_reads() {
        let parent = json!({"range":{"start_lid":"1.2","end_lid":"1.2"},"text":"abc def"});
        let child = json!({"range":{"start_lid":"1.2.1","end_lid":"1.2.1"},"text":"abc"});
        let elsewhere = json!({"range":{"start_lid":"1.20.1","end_lid":"1.20.1"},"text":"abc"});
        let input = json!({"read_sources":[parent.clone(),child.clone(),elsewhere.clone(),parent.clone()]});
        assert_eq!(model_input(&input)["read_sources"], json!([parent,elsewhere]));
        let mut partial = input.clone();
        partial["read_sources"][0]["range"]["ranges"] = json!([{"lid":"1.2.1","range":{"start":0,"end":3}}]);
        partial["read_sources"].as_array_mut().unwrap().pop();
        assert_eq!(model_input(&partial)["read_sources"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn coach_projection_preserves_unlinked_history_and_resolves_exact_help_copies() {
        let input = json!({"source_turn_ref":"now","actual_delivery":{"answer":"new answer"},"dialogue":[
            {"turn_ref":"old","user":"earlier question","answer":"old help"},
            {"turn_ref":"now","answer":"new answer"}],
            "observation_facts":[{"fact_ref":"f","user_expression":"my understanding","prior_help":[
                {"event_id":"help","answer":"old help"},{"event_id":"different","answer":"help outside this dialogue"}]}],
            "teaching_context":{"recent_facts":[{"event_id":"not-in-grounding","response_preview":"important correction"}],
                "learner_context":{"recent_facts":[{"event_id":"not-in-grounding","response_preview":"important correction"}],
                    "learner_grounding":{"expressions":[],"delivered_support":[]},
                    "learning_opportunity":{"delivered_support":[{"delivery_ref":"other","answer_preview":"older teaching"}]}}}});
        let view = model_input(&input);
        assert_eq!(view["teaching_context"]["recent_facts"][0]["response_preview"], "important correction");
        assert_eq!(view["teaching_context"]["learner_context"]["recent_facts"], json!([]));
        assert_eq!(view["teaching_context"]["learner_context"]["learning_opportunity"]["delivered_support"], input["teaching_context"]["learner_context"]["learning_opportunity"]["delivered_support"]);
        assert_eq!(view["observation_facts"][0]["prior_help"][0]["answer_ref"], "dialogue:old");
        assert_eq!(view["observation_facts"][0]["prior_help"][1]["answer"], "help outside this dialogue");
        assert_eq!(view["dialogue"][0]["answer"], "old help");
        assert_eq!(view["dialogue"].as_array().unwrap().len(), 1);
        assert!(view.get("actual_delivery").is_none());
    }

    #[test]
    fn ordinary_observation_is_independent_of_the_current_teacher_answer() {
        let input = json!({"trigger":"after_turn","source_turn_ref":"now",
            "observation_facts":[{"fact_ref":"f","binding":{"turn_id":"now"},
                "user_expression":"计算一直是11.2 ms，被压下去的是读取，计算成为较大项。",
                "prior_help":[{"event_id":"help","answer":"原来6 ms的计算包含在里面。"}]}],
            "dialogue":[{"turn_ref":"old","user":"先解释瓶颈","answer":"原来6 ms的计算包含在里面。"},
                {"turn_ref":"now","user":"计算一直是11.2 ms","answer":"学生过度归因，只错一处"}],
            "actual_delivery":{"relative_to_expression":"after","answer":"学生过度归因，只错一处",
                "answer_view":{"parts":[{"kind":"presentation","text":"本轮之后的图"}]}},
            "current_design":{"text":"本轮之后的新教案"},
            "consumed_guidance":{"text":"不应当充当当前理解的指令"},
            "turn_facts":[{"kind":"message_delivered","payload":{"answer":"学生过度归因，只错一处"}},
                {"kind":"design_revised","payload":{"text":"本轮之后的新教案"}}],
            "teaching_context":{"current_design":{"design_ref":"prior-design"},
                "socratic_dialogue":{"question":"计算和读取分别是多少？","purpose":{"focus":"两项比较"}},
                "learner_context":{"learner_grounding":{"delivered_support":[{"delivery_ref":"old","answer":"原来6 ms的计算包含在里面。"}]}}},
            "read_sources":[{"range":{"start_lid":"1.1","end_lid":"1.1"},"text":"下界取两项较大值。"}]});
        let view = model_input(&input);
        let mut corrected = input.clone();
        corrected["actual_delivery"] = json!({"answer":"你纠正得对，我两处都说错了"});
        corrected["dialogue"][1]["answer"] = json!("你纠正得对，我两处都说错了");
        corrected["current_design"] = json!({"text":"另一份本轮之后的教案"});
        corrected["turn_facts"] = json!([]);
        assert_eq!(view, model_input(&corrected), "changing later teacher work cannot change the learner-observation input");
        assert!(!view.to_string().contains("学生过度归因"));
        assert!(!view.to_string().contains("本轮之后"));
        assert_eq!(view["dialogue"][0]["answer"], "原来6 ms的计算包含在里面。", "keep the disputed prior statement for independent source checking");
        assert_eq!(view["observation_facts"][0]["prior_help"][0]["answer_ref"], "dialogue:old");
        assert_eq!(view["current_design"]["design_ref"], "prior-design");
        assert_eq!(view["teaching_context"]["socratic_dialogue"], input["teaching_context"]["socratic_dialogue"]);
        assert_eq!(view["read_sources"], input["read_sources"]);
        assert_eq!(input["actual_delivery"]["answer"], "学生过度归因，只错一处", "the durable delivery is not rewritten");
    }
}
