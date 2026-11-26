//! Compact recovery input for a text application card after an authoring failure.
//! The full select still passes the ordinary tool, source and practice preflights.
use super::*;

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ApplicationCard {
    /// A short new case. Make every stated condition and outcome mutually consistent.
    situation: String,
    /// Ask for one decision that depends on a detail of this case. Do not restate the previous explanation or give the rule.
    question: String,
    /// Two to six plausible choices, in display order, without A/B prefixes or marks. Exactly one must fit the complete case.
    options: Vec<String>,
    /// One-based position of the correct choice. Private.
    correct_option: usize,
    /// Private reasoning showing which new detail makes this choice hold and rules out the closest competing option.
    solution: String,
    /// One private hint directing attention to an already visible detail. Leave choosing its meaning to the learner.
    hint: String,
}

pub(super) fn supported(context: &Value) -> bool {
    let target = &context["current_design"]["design"]["target"];
    context["application_requested"] == true && context["status"] == "practice"
        && context["current_action"]["user_supplement"].as_str().unwrap_or_default().is_empty()
        && target["source_bindings"].as_array().is_some_and(|sources| !sources.is_empty())
        && !target["media_bindings"].is_array() && !target["music_bindings"].is_array()
}

pub(super) fn tool() -> crate::ToolSpec {
    crate::ToolSpec { name:"tutor.step".into(),
        description:format!("Publish one new application question using the accepted design. Supply only the situation, question, choices, private key, solution and hint. {}", crate::practice::APPLICATION_SELF_CHECK),
        parameters:json!({"type":"object","additionalProperties":false,"required":["operation","application"],
            "properties":{"operation":{"type":"string","enum":["select"]},
                "application":book_tool_contracts::schema_value::<ApplicationCard>()}}) }
}

pub(super) fn expand(context: &Value, request: &Value) -> Result<Value,String> {
    if !supported(context) || request["operation"] != "select" || request.as_object().is_none_or(|fields| fields.len() != 2) {
        return Err("Use operation=select and application only for a requested application card with an accepted text design.".into());
    }
    let card: ApplicationCard = serde_json::from_value(request["application"].clone()).map_err(|e| e.to_string())?;
    if !(2..=6).contains(&card.options.len()) || !(1..=card.options.len()).contains(&card.correct_option)
        || [&card.situation,&card.question,&card.solution,&card.hint].iter().any(|s| s.trim().is_empty()) {
        return Err("Supply the whole new case, one question, two to six choices, a valid one-based correct_option, solution and hint.".into());
    }
    let design = &context["current_design"]["design"];
    let target = &design["target"];
    let options: Vec<_> = card.options.iter().enumerate().map(|(i,text)|
        json!({"id":((b'A' + i as u8) as char).to_string(),"text":text})).collect();
    let ids: Vec<_> = options.iter().map(|option| &option["id"]).collect();
    let sources: std::collections::BTreeSet<_> = target["source_bindings"].as_array().into_iter().flatten()
        .flat_map(|source| [source["start_lid"].as_str(),source["end_lid"].as_str()]).flatten().collect();
    let english = context["response_language"] == "en";
    let mut movement = json!({
        "move_id":"application","kind":"question","design_ref":context["current_design"]["design_ref"],
        "block_refs":design["focus_block_refs"],"target":target,
        "object_ids":target["object_refs"].as_array().into_iter().flatten().map(|r| &r["object_id"]).collect::<Vec<_>>(),
        "prompt":format!("{}\n\n{}",card.situation.trim(),card.question.trim()),
        "delivery_text":if english {"Choose an answer on the practice card."} else {"请在练习卡中选择你的答案。"},
        "participation":{"choice":"invite","reason":if english {"The reader requested a new application."} else {"读者要求用新情境检验刚讲的关系。"},
            "support":card.situation,"learner_inference":card.question},
        "actions":["submit","revise","request_hint","reveal","skip"],"answer_input":{"kind":"single_choice","options":options},
        "assessment":{"rule":{"kind":"choice_set","choices":ids,"correct":[ids[card.correct_option-1]]},"source_lids":sources,
            "source_sufficient":true,"feedback":"on_reveal"},
        "explanation":card.solution,"practice":{"entry":"reading_margin","mode":"discriminate","distance":"adjacent",
            "application_situation":card.situation,"hints":[{"kind":"in_problem","text":card.hint}]}
    });
    if !context["current_action"].is_object() { movement["capability"] = target["capability"].clone(); }
    if action_intent(context).is_none() {
        movement["interaction_intent"] = json!("neutral");
        movement["intent_origin"] = json!("neutral");
    }
    Ok(json!({"operation":"select","move":movement,"finish":true}))
}
