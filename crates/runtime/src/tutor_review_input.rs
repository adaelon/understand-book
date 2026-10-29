//! Separate factual evidence from the learner-visible basis for explanation.
//!
//! Preview reports repeat the same whole-page text after each action and in
//! each viewport. Keep the first exact text and point later copies to it; the
//! original packet remains available to the caller for evidence validation.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

const MIN_REUSED_TEXT_CHARS: usize = 128;

const EVIDENCE_READING_GUIDE: &str = "author_claims 是作者说明，不是页面实际效果。按 author_preview_observations 中的环境与 observations 操作顺序阅读实际证据，结合 step、action、scroll、viewport、layout、scene 和截图说明确认位置及状态。dom.text 与 reading.text 是对应状态或 selector 的实际文字，不代表整段同时出现在屏幕；dom.visible_text 才描述该视口的可见文字。dom.semantic_text 仅作 DOM 语义证据，不作为逐字交付引文。same_text_as 是本页内直接指向完全相同原文的 JSON Pointer，读取该处原文即可，证据性质仍由当前字段决定；复用文字不复用视口、操作或可见范围，也不说明学习者看过这些内容。";

/// Choose the evidence surface, not a predicted question difficulty. An
/// explicit working target is required; media and page deliveries keep their
/// full observation-aware review contract.
pub fn uses_text_review(packet: &Value) -> bool {
    if packet["proposed_board"].is_object()
        || packet["proposed_presentations"].as_array().is_some_and(|pages| !pages.is_empty())
        || !packet["frozen_input"]["video_question"].is_null()
        || !packet["frozen_input"]["music_question"].is_null()
        || packet["music_interactions"].as_array().is_some_and(|items| !items.is_empty())
        || packet["presented_source_references"].as_array().into_iter().flatten()
            .any(|source| !source["media"].is_null() || !source["music"].is_null()) {
        return false;
    }
    packet["proposed_delivery"]["moves"].as_array().is_some_and(|moves| {
        !moves.is_empty() && moves.iter().all(|movement| {
            movement["target"].is_object() && movement["presentation"].is_null()
                && movement["assessment"].is_null()
                && ["media_bindings", "music_bindings"].into_iter().all(|field|
                    movement["target"][field].as_array().is_none_or(Vec::is_empty))
        })
    })
}

/// Give the explanation reader only what the learner can use to understand
/// this answer. Source retrieval, the author's plan and other reviewers' claims
/// cannot fill a missing step in the delivered explanation.
pub fn project_explanation(packet: &Value) -> Value {
    let frozen = &packet["frozen_input"];
    let action = &frozen["current_action"];
    let grounding = &packet["learner_grounding"];
    let expressions = grounding["expressions"].as_array().map(Vec::as_slice).unwrap_or_default();
    // UI requests can have an application-generated accepted_request. The
    // supplement is the learner's actual wording; never label the UI label as
    // evidence that the learner has understood something.
    let current_words = action["user_supplement"].as_str().filter(|text| !text.trim().is_empty())
        .or_else(|| {
            if matches!(action["action"]["origin"].as_str(), Some("ui_entry" | "ui_choice")) { None }
            else { packet["original_request"].as_str().filter(|text| !text.trim().is_empty()) }
        });
    // Grounding is oldest-first. Repeated identical questions still have
    // different input-time help: only the latest matching expression is current.
    let current_expression = current_words.and_then(|words| expressions.iter().rposition(|expression|
        expression["text"].as_str() == Some(words)));
    let mut support_refs = Vec::new();
    if let Some(index) = current_expression {
        for field in ["support_delivery_refs", "assistance_refs"] {
            support_refs.extend(expressions[index][field].as_array().into_iter().flatten().filter_map(Value::as_str));
        }
    }
    if let Some(reference) = action["action"]["teaching_choice"]["delivery_ref"].as_str() {
        support_refs.push(reference);
    }
    let delivered_support: Vec<_> = grounding["delivered_support"].as_array().into_iter().flatten()
        .filter(|support| support["delivery_ref"].as_str().is_some_and(|reference| support_refs.contains(&reference)))
        .map(|support| explanation_fields(support, &["delivery_ref", "kind", "occurred_at", "answer", "text"]))
        .collect();
    let reader_expressions: Vec<_> = expressions.iter().enumerate().filter(|(_, expression)| expression["text"].is_string())
        .map(|(index, expression)| {
            let mut words = explanation_fields(expression, &["text", "fact_ref", "evidence_ref", "kind", "source_turn_id"]);
            words["scope"] = json!(if Some(index) == current_expression { "current" } else { "past" });
            words
        }).collect();
    let mut request_context = explanation_fields(action, &["action", "accepted_request", "user_supplement", "answer_ref"]);
    // A passage explicitly selected in the UI is part of the question. It is
    // not the retrieved chapter and must not stand in for the answer's own
    // explanation. Keep it distinct from previously delivered assistant help.
    if let Some(selected) = action.get("source_excerpt").filter(|value| !value.is_null()) {
        request_context["selected_text"] = selected.clone();
    }
    let committed_deliveries: Vec<_> = packet["committed_deliveries"].as_array().into_iter().flatten()
        .map(|delivery| explanation_fields(delivery, &["delivery_id", "text", "answer_parts"]))
        .collect();
    let mut projected = json!({
        "current_request":packet["original_request"],
        "delivery_scope":packet["delivery_scope"],
        "committed_deliveries":committed_deliveries,
        "current_request_context":request_context,
        "preceding_assistant_response":action["answer_text"],
        "delivered_support":delivered_support,
        "reader_expressions":reader_expressions,
        "confirmed_background":packet["learner_view"]["confirmed_background"],
        "confirmed_preferences":frozen["confirmed_preferences"],
        "current_delivery":packet["proposed_delivery"]["text"],
        "open_issues":crate::teaching_review_issues::branch_issues(&packet["open_issues"], "explanation")
    });
    // This is the host's first unaccepted draft, never an author summary or
    // source packet. It exposes lost explanatory work without witnessing the
    // current delivery or establishing the old draft's factual correctness.
    if let Some(text) = packet["revision_reference"]["text"].as_str().filter(|text| !text.trim().is_empty()) {
        projected["revision_reference"] = json!({"text":text,
            "purpose":"Preserve the relevant explanatory work of the first unaccepted draft while correcting it. This is neither verified source evidence nor delivered help; it cannot supply current answer_quotes.",
            "changes_from_reference":crate::orchestrator::teaching_review_changes(
                &json!({"text":text}), &json!({"text":packet["proposed_delivery"]["text"]}))});
    }
    projected
}

fn explanation_fields(value: &Value, fields: &[&str]) -> Value {
    Value::Object(fields.iter().filter_map(|field| value.get(*field).filter(|value| !value.is_null())
        .map(|value| ((*field).to_owned(), value.clone()))).collect())
}

/// Check factual claims against the complete request, actual learner words and
/// help, and every actually read source. The independent explanation reader
/// owns pedagogical sufficiency; private teaching plans cannot narrow either
/// review to the author's preferred target.
/// The caller retains the unprojected packet for validation and durable audit.
pub fn project_text(mut packet: Value) -> Value {
    packet["open_issues"] = crate::teaching_review_issues::branch_issues(&packet["open_issues"], "factual");
    if let Some(context) = packet.get_mut("review_context").and_then(Value::as_object_mut) {
        context.remove("open_issues");
    }
    if let Some(fields) = packet.as_object_mut() {
        for field in ["current_design", "local_design", "coach_context", "agent_working_state", "revision_reference"] {
            fields.remove(field);
        }
    }
    for movement in packet["proposed_delivery"]["moves"].as_array_mut().into_iter().flatten() {
        if let Some(fields) = movement.as_object_mut() {
            // The joined final text above is the exact content under review.
            for field in ["delivery_text", "explanation", "hint", "coach_ref", "coach_adjustment", "participation"] {
                fields.remove(field);
            }
        }
        if let Some(target) = movement.get_mut("target").and_then(Value::as_object_mut) {
            target.remove("expected_performance");
            target.remove("learning_focus");
        }
    }
    if let Some(grounding) = packet.get_mut("learner_grounding").and_then(Value::as_object_mut) {
        grounding.remove("interpretations");
        for support in grounding.get_mut("delivered_support").and_then(Value::as_array_mut).into_iter().flatten() {
            if let Some(fields) = support.as_object_mut() { fields.remove("move"); }
        }
    }
    if let Some(prior) = packet["review_context"]["prior_reviews"].as_array_mut() {
        for review in prior {
            if let Some(fields) = review.as_object_mut() {
                fields.remove("reading_path");
                fields.remove("improvements");
                fields.remove("changes_since_previous_review");
                fields.remove("open_issues");
            }
        }
    }
    packet
}

/// Reduce repeated preview text without summarizing it or discarding states.
/// References are confined to one saved presentation (including its revision).
pub fn project(mut packet: Value) -> Value {
    let Some(pages) = packet
        .get_mut("proposed_presentations")
        .and_then(Value::as_array_mut)
    else {
        return packet;
    };

    for (page_index, page) in pages.iter_mut().enumerate() {
        let Some(page) = page.as_object_mut() else {
            continue;
        };
        let mut claims = Map::new();
        for field in ["readable_content", "initial_state", "state_contract"] {
            if let Some(value) = page.remove(field) {
                claims.insert(field.to_owned(), value);
            }
        }
        if !claims.is_empty() {
            page.insert("author_claims".to_owned(), Value::Object(claims));
        }

        let Some(previews) = page
            .get_mut("author_preview_observations")
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        if previews.is_empty() {
            continue;
        }

        // Direct string comparison is sufficient for the small number of
        // distinct page states. Never reuse author claims or another page.
        let mut seen_text = BTreeMap::<String, String>::new();
        for (preview_index, preview) in previews.iter_mut().enumerate() {
            let prefix = format!(
                "#/proposed_presentations/{page_index}/author_preview_observations/{preview_index}"
            );
            if let Some(observations) = preview
                .get_mut("observations")
                .and_then(Value::as_array_mut)
            {
                for (observation_index, observation) in observations.iter_mut().enumerate() {
                    if let Some(dom) = observation.get_mut("dom").and_then(Value::as_object_mut) {
                        for field in ["text", "semantic_text", "visible_text"] {
                            if let Some(text) = dom.get_mut(field) {
                                reuse_exact_text(
                                    text,
                                    format!(
                                        "{prefix}/observations/{observation_index}/dom/{field}"
                                    ),
                                    &mut seen_text,
                                );
                            }
                        }
                    }
                }
            }
            if let Some(text) = preview
                .get_mut("reading")
                .and_then(|reading| reading.get_mut("text"))
            {
                reuse_exact_text(text, format!("{prefix}/reading/text"), &mut seen_text);
            }
        }
        page.insert(
            "evidence_reading_guide".to_owned(),
            json!(EVIDENCE_READING_GUIDE),
        );
    }
    packet
}

fn reuse_exact_text(value: &mut Value, path: String, seen: &mut BTreeMap<String, String>) {
    let Some(text) = value.as_str() else {
        return;
    };
    // Short labels and changing numeric values remain inline. A reference must
    // replace a meaningfully larger copy, not make a small report harder to read.
    if text.chars().take(MIN_REUSED_TEXT_CHARS).count() < MIN_REUSED_TEXT_CHARS {
        return;
    }
    if let Some(first_path) = seen.get(text) {
        *value = json!({"same_text_as": first_path});
    } else {
        seen.insert(text.to_owned(), path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wb4_explanation_projection_keeps_full_request_and_actual_committed_parts() {
        let packet = json!({
            "original_request":"Explain how to compute the interval, then use it to calculate average speed.",
            "delivery_scope":{"kind":"board_part","relation":"How the two times determine the interval.",
                "remaining_request":"Use the interval to calculate average speed."},
            "committed_deliveries":[{"delivery_id":"delivery-clock", "text":"Measure both times using the same clock.",
                "answer_parts":[{"kind":"text","text":"Measure both times using the same clock."}],
                "source_bindings":[{"preview_snapshot":"PRIVATE_SOURCE_TEXT"}],
                "teaching_delivery_move_refs":["PRIVATE_MOVEMENT_REF"], "author_claims":"PRIVATE_AUTHOR_SUMMARY"}],
            "proposed_delivery":{"text":"Subtract the start time from the end time.",
                "moves":[{"explanation":"PRIVATE_NOT_DELIVERED"}]}
        });
        let projected = project_explanation(&packet);
        assert_eq!(projected["current_request"], packet["original_request"]);
        assert_ne!(projected["current_request"], packet["delivery_scope"]["relation"]);
        assert_eq!(projected["delivery_scope"], packet["delivery_scope"]);
        assert_eq!(projected["committed_deliveries"][0]["delivery_id"], "delivery-clock");
        assert_eq!(projected["committed_deliveries"][0]["text"], packet["committed_deliveries"][0]["text"]);
        assert_eq!(projected["committed_deliveries"][0]["answer_parts"], packet["committed_deliveries"][0]["answer_parts"]);
        assert_eq!(projected["current_delivery"], packet["proposed_delivery"]["text"]);
        assert!(!projected.to_string().contains("PRIVATE_"));
        let factual = project_text(packet.clone());
        assert_eq!(factual["original_request"], packet["original_request"]);
        assert_eq!(factual["delivery_scope"], packet["delivery_scope"]);
        assert_eq!(factual["committed_deliveries"], packet["committed_deliveries"]);

        let mut final_packet = packet;
        final_packet.as_object_mut().unwrap().remove("delivery_scope");
        final_packet["proposed_delivery"]["text"] = json!("Together the saved steps establish the calculation.");
        let final_projection = project_explanation(&final_packet);
        assert!(final_projection["delivery_scope"].is_null());
        assert_eq!(final_projection["current_request"], final_packet["original_request"]);
        assert_eq!(final_projection["committed_deliveries"], projected["committed_deliveries"]);
    }

    #[test]
    fn explanation_projection_cannot_borrow_the_authors_explanation_or_review_verdict() {
        let packet = json!({
            "original_request":"Why does B remain?",
            "frozen_input":{"current_request":"Why does B remain?", "user_intent":"PRIVATE_OLD_GOAL",
                "confirmed_preferences":["Show the operation."]},
            "learner_view":{"confirmed_background":["New to partial updates."],
                "pending_activity":{"answer":"PRIVATE_PENDING_ANSWER"}},
            "learner_grounding":{"expressions":[{"fact_ref":"current", "text":"Why does B remain?",
                "support_delivery_refs":["help"]}],
                "delivered_support":[{"delivery_ref":"help", "answer":"The update changes A; B remains.",
                    "move":{"explanation":"PRIVATE_PREVIOUS_PLAN"}}],
                "interpretations":[{"interpretation":"PRIVATE_LEARNER_DIAGNOSIS"}]},
            "current_design":{"target":{"expected_performance":"PRIVATE_EXPECTED_PERFORMANCE"}},
            "current_source_evidence":[{"text":"PRIVATE_SOURCE_EXPLANATION"}],
            "agent_working_state":{"understanding":"PRIVATE_AUTHOR_MEMORY"},
            "coach_context":{"guidance":"PRIVATE_COACH_GUIDANCE"},
            "review_context":{"prior_reviews":[{"decision":"accept", "reason":"PRIVATE_PRIOR_VERDICT"}]},
            "proposed_delivery":{"text":"B remains.", "moves":[{"explanation":"PRIVATE_AUTHOR_EXPLANATION"}]}
        });
        let projected = project_explanation(&packet);
        assert_eq!(projected["current_request"], "Why does B remain?");
        assert_eq!(projected["current_delivery"], "B remains.");
        assert_eq!(projected["delivered_support"][0]["answer"], "The update changes A; B remains.");
        assert_eq!(projected["confirmed_background"], packet["learner_view"]["confirmed_background"]);
        assert_eq!(projected["confirmed_preferences"], packet["frozen_input"]["confirmed_preferences"]);
        assert!(!projected.to_string().contains("PRIVATE_"));
    }

    #[test]
    fn unresolved_issues_remain_with_their_own_review_branch() {
        let factual = json!({"issue_ref":"factual:1:0","branch":"factual","kind":"finding",
            "finding":{"evidence":"FACTUAL_SOURCE_GAP"}});
        let explanation = json!({"issue_ref":"explanation:2:request","branch":"explanation","kind":"request",
            "request_resolution":{"reason":"EXPLANATION_CONNECTION_GAP"}});
        let packet = json!({"original_request":"Explain the delay.","proposed_delivery":{"text":"The delay matters."},
            "open_issues":[factual.clone(),explanation.clone()],
            "review_context":{"open_issues":[factual.clone(),explanation.clone()],
                "prior_reviews":[{"open_issues":[factual.clone(),explanation.clone()]}]},
            "current_source_evidence":[{"text":"ACTUAL_SOURCE"}]});
        let text = project_text(packet.clone());
        assert_eq!(text["open_issues"], json!([factual]));
        assert!(text["review_context"].get("open_issues").is_none());
        assert!(text["review_context"]["prior_reviews"][0].get("open_issues").is_none());
        assert!(!text.to_string().contains("EXPLANATION_CONNECTION_GAP"));
        let independent = project_explanation(&packet);
        assert_eq!(independent["open_issues"], json!([explanation]));
        assert!(!independent.to_string().contains("FACTUAL_SOURCE_GAP"));
        assert!(!independent.to_string().contains("ACTUAL_SOURCE"));
    }

    #[test]
    fn revision_reference_preserves_only_the_actual_draft_without_becoming_delivery_evidence() {
        let mut input = json!({"original_request":"Explain why B remains.",
            "proposed_delivery":{"text":"B remains."},"open_issues":[]});
        assert!(project_explanation(&input).get("revision_reference").is_none());
        input["revision_reference"] = json!({
            "text":"The update replaces only A. B is not named, so B retains its earlier value.",
            "purpose":"PRIVATE_AUTHOR_INSTRUCTION","source_evidence":"PRIVATE_SOURCE",
            "review":"PRIVATE_FACTUAL_REVIEW","changes_from_reference":"PRIVATE_AUTHOR_DIFF"});
        let projected = project_explanation(&input);
        assert_eq!(projected["revision_reference"]["text"], input["revision_reference"]["text"]);
        assert!(projected["revision_reference"]["purpose"].as_str().unwrap().contains("first unaccepted draft"));
        assert_eq!(projected["revision_reference"]["changes_from_reference"], json!(
            crate::orchestrator::teaching_review_changes(
                &json!({"text":input["revision_reference"]["text"]}), &input["proposed_delivery"])));
        assert!(projected["revision_reference"]["changes_from_reference"].to_string().contains("The update replaces only A."));
        assert!(!projected.to_string().contains("PRIVATE_"));
        assert!(project_text(input.clone()).get("revision_reference").is_none());

        let mut review = json!({"decision":"accept","revision":null,"findings":[],"learner_claims":[],
            "request_resolution":{"status":"answered","question_quote":"Explain why B remains.",
                "requested_relation":"Why B is retained.","delivered_relation":"The answer states B remains.",
                "answer_quotes":["B remains."],"reason":"Current explanation judgment."}});
        crate::tutor::normalize_delivery_review(&mut review, &input).unwrap();
        review["request_resolution"]["answer_quotes"] = json!(["The update replaces only A."]);
        assert!(crate::tutor::normalize_delivery_review(&mut review, &input).is_err(),
            "the original draft must not fill a connection missing from the current delivery");
    }

    #[test]
    fn explanation_projection_uses_current_words_and_their_input_time_help() {
        let packet = json!({
            "original_request":"Please explain directly.",
            "frozen_input":{"current_request":"Old copied request", "learner_request":"Quiz me."},
            "learner_grounding":{"expression_order":"oldest_first", "expressions":[
                {"fact_ref":"first", "text":"Please explain directly.", "support_delivery_refs":["old-help"]},
                {"fact_ref":"second", "text":"Quiz me.", "support_delivery_refs":["quiz-help"]},
                {"fact_ref":"current", "text":"Please explain directly.", "support_delivery_refs":["current-help"]}],
                "delivered_support":[
                    {"delivery_ref":"old-help", "answer":"Old answer."},
                    {"delivery_ref":"current-help", "answer":"The exact most recent answer."},
                    {"delivery_ref":"quiz-help", "answer":"Quiz answer."}]},
            "proposed_delivery":{"text":"Here is the explanation."}
        });
        let projected = project_explanation(&packet);
        assert_eq!(projected["current_request"], "Please explain directly.");
        assert_eq!(projected["reader_expressions"][0]["scope"], "past");
        assert_eq!(projected["reader_expressions"][1]["scope"], "past");
        assert_eq!(projected["reader_expressions"][2]["scope"], "current");
        assert_eq!(projected["delivered_support"].as_array().unwrap().len(), 1);
        assert_eq!(projected["delivered_support"][0]["answer"], "The exact most recent answer.");
    }

    #[test]
    fn explanation_projection_preserves_ui_question_context_without_source_or_target_expansion() {
        let packet = json!({
            "original_request":"Explain the selected sentence.",
            "frozen_input":{"current_action":{
                "action":{"origin":"ui_entry", "entry_action":"explain_selection"},
                "accepted_request":"Explain the selected sentence.", "user_supplement":"Please use words, not matrices.",
                "source_excerpt":"The state stores a sum of outer products.",
                "source_bindings":[{"preview_snapshot":"PRIVATE_WHOLE_CHAPTER"}],
                "target":{"expected_performance":"PRIVATE_AUTHOR_TARGET"},
                "answer_ref":{"turn_id":"previous", "excerpt":"The state has fixed dimensions."},
                "answer_text":"The state has fixed dimensions."}},
            "learner_grounding":{"expressions":[{"text":"Please use words, not matrices.",
                "support_delivery_refs":["help"]}],
                "delivered_support":[{"delivery_ref":"help", "text":"Previous public help."}]},
            "proposed_delivery":{"text":"Each new item changes the values inside the same slots."}
        });
        let projected = project_explanation(&packet);
        assert_eq!(projected["current_request_context"]["user_supplement"], "Please use words, not matrices.");
        assert_eq!(projected["current_request_context"]["selected_text"], "The state stores a sum of outer products.");
        assert_eq!(projected["preceding_assistant_response"], "The state has fixed dimensions.");
        assert_eq!(projected["reader_expressions"][0]["scope"], "current");
        assert_eq!(projected["delivered_support"][0]["text"], "Previous public help.");
        assert!(!projected.to_string().contains("PRIVATE_"));
    }

    #[test]
    fn explanation_projection_does_not_turn_a_ui_click_into_current_learner_words() {
        let packet = json!({
            "original_request":"Continue.",
            "frozen_input":{"current_action":{"action":{"origin":"ui_choice",
                "teaching_choice":{"delivery_ref":"offering-help", "option_id":"continue"},"move_kind":"explain"},
                "accepted_request":"Continue.", "user_supplement":""}},
            "learner_grounding":{"expressions":[{"text":"Continue.", "support_delivery_refs":["old-help"]}],
                "delivered_support":[{"delivery_ref":"old-help", "answer":"Unrelated old help."},
                    {"delivery_ref":"offering-help", "answer":"Actual offering answer."}]},
            "proposed_delivery":{"text":"The next step."}
        });
        let projected = project_explanation(&packet);
        assert_eq!(projected["reader_expressions"][0]["scope"], "past");
        assert_eq!(projected["delivered_support"].as_array().unwrap().len(), 1);
        assert_eq!(projected["delivered_support"][0]["answer"], "Actual offering answer.");
    }

    #[test]
    fn text_projection_preserves_factual_evidence_and_actual_help_without_private_teaching_plans() {
        let packet = json!({
            "original_request":"Explain all required steps, then confirm my interpretation.",
            "frozen_input":{"current_request":"Explain all required steps, then confirm my interpretation.",
                "learning_purpose":"Understand how the catalog update works, not memorize the final entries.",
                "constraints":["Explain the reason"]},
            "learner_grounding":{"expressions":[{"fact_ref":"current","text":"B remains when omitted, right?","support_delivery_refs":["help"]}],
                "delivered_support":[{"delivery_ref":"help","occurred_at":"12345","answer":"Original help, retained verbatim.",
                    "move":{"explanation":"PRIVATE_HISTORICAL_PLAN"}}],
                "interpretations":[{"interpretation":"PRIVATE_READER_DIAGNOSIS"}]},
            "current_design":{"design_ref":"design:one","design":{
                "target":{"learning_focus":"How a partial update preserves unmentioned entries.",
                    "expected_performance":"Trace an update of A and explain why B remains."},
                "organizing_model":"Follow the same catalog before, during and after the update.",
                "basis_refs":["current","help"],
                "revision_reason":"The previous answer named the result but did not show the update operation.",
                "representation":{"kind":"text","reason":"A small before-and-after trace can show each operation."},
                "blocks":[{"block_id":"local","relation":"Not selected does not imply deleted."},
                    {"block_id":"later","relation":"LATER_RELATION".repeat(500)}]}},
            "proposed_delivery":{"text":"B remains in the catalog.","moves":[{"target":{
                "source_bindings":[{"start_lid":"1.1","end_lid":"1.1"}],
                "learning_focus":"AUTHOR_FOCUS", "expected_performance":"AUTHOR_EXPECTED_PERFORMANCE"},
                "block_refs":["local"],"delivery_text":"B remains in the catalog.",
                "explanation":"Trace the effect of changing A while B is absent from the update.","hint":"Only entries named by the update are changed.",
                "coach_ref":"coach:previous","coach_adjustment":{"reason":"The reader asks for the steps directly.","fact_refs":["current"]},
                "participation":{"choice":"conclude","reason":"Confirm the requested relationship."}}]},
            "proposed_presentations":[],"coach_context":{"status":"ready","coach_ref":"coach:previous","source_turn_ref":"previous",
                "guidance":{"next_step":{"focus":"Show how a partial update affects the catalog.",
                    "participation":{"choice":"invite","reason":"Let the reader predict which entries remain.","support":"Start with A and B.","learner_inference":"The effect of omitting B."}},
                    "reason":"The previous explanation gave the result without the process."}},
            "agent_working_state":{"current_work":{"review_ref":"review:previous","request":{"text":"What changes in a partial update?","truncated":false},
                    "agent_work":{"understanding":[{"statement":"An omitted entry is retained.","sources":[{"lid":"1.1","quote":"unmentioned entries remain"}]}],
                        "open_questions":["The reader has not yet seen why B remains."]},
                    "proposed_text":{"text":"REPEATED_PRIVATE_DRAFT".repeat(100),"truncated":false}},
                "previous_work":[{"review_ref":"review:earlier","agent_work":{"understanding":[],"open_questions":["Explain the update operation."]},
                    "proposed_text":{"text":"OLDER_PRIVATE_DRAFT".repeat(100),"truncated":false}}],
                "corrections":[{"review_ref":"review:correction","revision":{"text":"Omission is not deletion; show the operation.","truncated":false},
                    "review_status":"accepted_by_review","follow_up_changes":[{"changes":[{"before":"OLD_CHANGE_TEXT".repeat(100),"after":"Repaired text."}]}]}],
                "meaning":"Teacher work is not learner understanding or proof of delivery."},
            "current_source_evidence":[{"source_id":"book","segments":[{"lid":"1.1","text":"FULL_ORIGINAL_SOURCE".repeat(100)}]}],
            "current_web_evidence":[{"url":"https://example.org/reference","text":"Full observed external reference."}],
            "review_context":{"round":2,"prior_reviews":[{"decision":"revise","revision":"Preserve the necessary condition.","improvements":[{"suggested_change":"OPTIONAL_POLISH"}]}]}
        });
        assert!(uses_text_review(&packet), "selection depends on the actual evidence surface, not short wording");
        let projected = project_text(packet.clone());
        for key in ["original_request", "frozen_input", "current_source_evidence", "current_web_evidence"] {
            assert_eq!(projected[key], packet[key], "{key}");
        }
        assert_eq!(projected["learner_grounding"]["expressions"], packet["learner_grounding"]["expressions"]);
        for key in ["delivery_ref", "occurred_at", "answer"] {
            assert_eq!(projected["learner_grounding"]["delivered_support"][0][key],
                packet["learner_grounding"]["delivered_support"][0][key]);
        }
        assert_eq!(projected["proposed_delivery"]["text"], packet["proposed_delivery"]["text"]);
        assert_eq!(projected["proposed_delivery"]["moves"][0]["target"]["source_bindings"],
            packet["proposed_delivery"]["moves"][0]["target"]["source_bindings"]);
        for key in ["participation", "explanation", "hint", "coach_ref", "coach_adjustment"] {
            assert!(projected["proposed_delivery"]["moves"][0][key].is_null(), "{key}");
        }
        for key in ["current_design", "local_design", "coach_context", "agent_working_state"] {
            assert!(projected[key].is_null(), "{key}");
        }
        assert_eq!(projected["review_context"]["prior_reviews"][0]["revision"], "Preserve the necessary condition.");
        for absent in ["LATER_RELATION", "REPEATED_PRIVATE_DRAFT", "OLDER_PRIVATE_DRAFT", "OLD_CHANGE_TEXT", "OPTIONAL_POLISH",
            "AUTHOR_FOCUS", "AUTHOR_EXPECTED_PERFORMANCE", "PRIVATE_HISTORICAL_PLAN", "PRIVATE_READER_DIAGNOSIS"] {
            assert!(!projected.to_string().contains(absent), "{absent}");
        }
        assert!(projected.to_string().len() < packet.to_string().len() / 2, "projection must materially remove repeated unrelated tasks");
    }

    #[test]
    fn local_followup_new_media_keep_full_review_and_prior_help_stays_verbatim() {
        let text = json!({"proposed_delivery":{"text":"A relation","moves":[{"target":{"source_bindings":[]}}]},"proposed_presentations":[]});
        assert!(uses_text_review(&text));
        for field in ["scene", "presentation"] {
            let mut scene = text.clone();
            scene["learner_grounding"] = json!({"expressions":[{}]});
            scene["learner_grounding"]["expressions"][0][field] = json!({"id":"original-scene"});
            assert!(uses_text_review(&scene));
            assert_eq!(project_text(scene.clone())["learner_grounding"], scene["learner_grounding"]);
        }
        let mut page = text.clone();
        page["proposed_presentations"] = json!([{"reference":{"presentation_id":"p"}}]);
        assert!(!uses_text_review(&page));
        let mut board = text.clone();
        board["proposed_board"] = json!({"status":"inspected_candidate","items":[{"reading_text":"A drawn relation"}]});
        assert!(!uses_text_review(&board), "a native diagram requires the visual review that includes its observations");
        let mut media = text;
        media["proposed_delivery"]["moves"][0]["target"]["media_bindings"] = json!([{"target":"frame"}]);
        assert!(!uses_text_review(&media));
    }

    fn restore_text(value: &Value, packet: &Value) -> Value {
        match value {
            Value::Object(object) if object.len() == 1 && object.contains_key("same_text_as") => {
                let pointer = object["same_text_as"]
                    .as_str()
                    .unwrap()
                    .strip_prefix('#')
                    .unwrap();
                let original = packet
                    .pointer(pointer)
                    .expect("every reference must resolve");
                assert!(
                    original.is_string(),
                    "references must point directly to text"
                );
                original.clone()
            }
            Value::Object(object) => Value::Object(
                object
                    .iter()
                    .map(|(key, value)| (key.clone(), restore_text(value, packet)))
                    .collect(),
            ),
            Value::Array(values) => Value::Array(
                values
                    .iter()
                    .map(|value| restore_text(value, packet))
                    .collect(),
            ),
            _ => value.clone(),
        }
    }

    #[test]
    fn review_projection_reconstructs_all_text_and_preserves_observed_states() {
        let text = "公开全文，包括下面需要滚动才能看到的推导。".repeat(10);
        let changed_text = format!("{text}当前数值已从 60 秒变成 30 秒。");
        let semantic = format!("{text}折叠区的语义内容。");
        let previews = json!([
            {"candidate_id":"candidate-1", "environment_name":"desktop",
                "environment":{"width":960,"height":720,"input":"mouse"},
                "observations":[
                    {"step":0,"action":null,"scroll":{"x":0,"y":0,"max_y":2400},
                        "viewport":{"width":960,"height":720},"scene":{"id":"start"},
                        "layout":{"elements":[{"x":12,"y":730,"width":400,"height":100}]},
                        "dom":{"text":text,"semantic_text":semantic,"visible_text":"开头", "controls":[{"value":1}]}},
                    {"step":1,"action":{"type":"scroll","y":720},"scroll":{"x":0,"y":720,"max_y":2400},
                        "dom":{"text":text,"semantic_text":semantic,"visible_text":"推导", "controls":[{"value":1}]}},
                    {"step":2,"action":{"type":"input","value":3},"scroll":{"x":0,"y":720,"max_y":2400},
                        "dom":{"text":changed_text,"semantic_text":semantic,"visible_text":"30 秒", "controls":[{"value":3}]}}
                ],"reading":{"selector":"main","action_step":2,"text":changed_text,"page_state":{"s":3}}},
            {"environment_name":"mobile","environment":{"width":390,"height":844,"input":"touch"},
                "observations":[{"step":0,"action":null,"scroll":{"x":0,"y":0,"max_y":4800},
                    "dom":{"text":text,"semantic_text":semantic,"visible_text":"移动端开头"}}],
                "reading":{"selector":"#details","text":"选择器只读到这里"}}
        ]);
        let original = json!({
            "original_request":"请保留完整推导", "frozen_input":{"quote":"我不懂这里"},
            "learner_grounding":{"expressions":[{"text":"原话不能换成摘要"}]},
            "current_source_evidence":[{"content":"来源原文"}],
            "current_coach":{"intent":"解释移位相减"},
            "proposed_presentations":[{"reference":{"presentation_id":"p","revision":2},
                "author_preview_observations":previews}]
        });
        let projected = project(original.clone());
        let page = &projected["proposed_presentations"][0];
        assert_eq!(
            restore_text(&page["author_preview_observations"], &projected),
            previews
        );
        assert!(
            page["author_preview_observations"][0]["observations"][0]["dom"]["text"].is_string()
        );
        assert!(
            page["author_preview_observations"][0]["observations"][1]["dom"]["text"]
                ["same_text_as"]
                .is_string()
        );
        assert_eq!(
            page["author_preview_observations"][0]["observations"][2]["dom"]["text"],
            changed_text
        );
        for key in [
            "original_request",
            "frozen_input",
            "learner_grounding",
            "current_source_evidence",
            "current_coach",
        ] {
            assert_eq!(projected[key], original[key]);
        }
        assert_eq!(
            original["proposed_presentations"][0]["author_preview_observations"],
            previews
        );
    }

    #[test]
    fn review_projection_does_not_reuse_text_across_page_revisions() {
        let text = "同一句话也不能让两个版本的观察混在一起。".repeat(10);
        let original = json!({"proposed_presentations":[
            {"reference":{"presentation_id":"p","revision":1},"author_preview_observations":[
                {"observations":[{"dom":{"text":text}},{"dom":{"text":text}}]}]},
            {"reference":{"presentation_id":"p","revision":2},"author_preview_observations":[
                {"observations":[{"dom":{"text":text}},{"dom":{"text":text}}]}]}
        ]});
        let projected = project(original.clone());
        for index in 0..2 {
            let previews =
                &projected["proposed_presentations"][index]["author_preview_observations"];
            assert_eq!(previews[0]["observations"][0]["dom"]["text"], text);
            assert_eq!(previews[0]["observations"][1]["dom"]["text"]["same_text_as"],
                format!("#/proposed_presentations/{index}/author_preview_observations/0/observations/0/dom/text"));
            assert_eq!(
                restore_text(previews, &projected),
                original["proposed_presentations"][index]["author_preview_observations"]
            );
        }
    }

    #[test]
    fn review_projection_separates_author_claims_without_using_them_as_evidence() {
        let text = "作者声称有效不能代替实际观察。".repeat(12);
        let original = json!({"proposed_presentations":[{
            "reference":{"presentation_id":"p","revision":1},
            "readable_content":text,"initial_state":{"s":3},"state_contract":{"claim":"可操作"},
            "author_preview_observations":[{"reading":{"selector":"main","text":text}}]
        }]});
        let projected = project(original.clone());
        let page = &projected["proposed_presentations"][0];
        for field in ["readable_content", "initial_state", "state_contract"] {
            assert!(page.get(field).is_none());
            assert_eq!(
                page["author_claims"][field],
                original["proposed_presentations"][0][field]
            );
        }
        assert_eq!(
            page["author_preview_observations"][0]["reading"]["text"],
            text
        );
        assert_eq!(project(projected.clone()), projected);
    }

    #[test]
    fn review_projection_leaves_packets_without_pages_unchanged() {
        for packet in [
            json!({"original_request":"只回答文字","proposed_delivery":{"text":"回答"}}),
            json!({"proposed_presentations":[],"learner_grounding":{"expressions":["原话"]}}),
            json!({"proposed_presentations":null}),
        ] {
            assert_eq!(project(packet.clone()), packet);
        }
    }
}
