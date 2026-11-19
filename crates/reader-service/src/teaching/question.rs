//! A question keeps its own discussion and immutable submissions. Neither typing nor asking is an attempt.
use super::*;
use runtime::{CompletionRequest, ModelAdapter};

pub(super) fn steps(facts: &[TeachingEvent]) -> Vec<Value> {
    let replies: std::collections::HashMap<_, _> = facts.iter()
        .filter(|e| e.kind == TeachingFact::HelpDelivered && e.payload["action"] == "ask_step")
        .filter_map(|e| Some((e.payload["step_ref"].as_str()?, e))).collect();
    facts.iter().filter(|e| e.kind == TeachingFact::LearnerAction && e.payload["action"] == "ask_step")
        .map(|e| {
            let reply = replies.get(e.event_id.as_str());
            json!({"step_ref":e.event_id,"text":e.payload["response"],"selected":e.payload["selected"],
                "reply":reply.map(|r| &r.payload["text"]), "help_ref":reply.map(|r| &r.event_id)})
        }).collect()
}

pub(super) fn attempts(facts: &[TeachingEvent]) -> Vec<Value> {
    facts.iter().filter(|e| e.kind == TeachingFact::LearnerAction
        && matches!(e.payload["action"].as_str(), Some("submit" | "revise")))
        .map(|e| json!({"action_ref":e.event_id,"number":e.payload["attempt"],"answer":e.payload["response"],
            "reasoning":e.payload["reasoning"]})).collect()
}

pub(super) fn validate_answer(input: &Value, response: &str) -> Result<(), ToolError> {
    let Some(options) = input["options"].as_array() else { return Ok(()) };
    let ids = serde_json::from_str::<Vec<String>>(response).unwrap_or_else(|_| vec![response.to_owned()]);
    if ids.is_empty() || (input["kind"] == "single_choice" && ids.len() != 1)
        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
        || ids.iter().any(|id| !options.iter().any(|o| o["id"].as_str() == Some(id))) {
        return Err(invalid("请选择题目提供的选项"));
    }
    Ok(())
}

/// The evaluator selects exact lines from the frozen book and answer. It need not
/// transcribe LaTeX, and the same assessor still checks every selected citation.
fn worked_packet(packet: &Value, locale: &Value) -> Value {
    let mut passages = vec![];
    for (lid, source) in packet["sources"].as_object().into_iter().flatten() {
        for quote in source.as_str().unwrap_or_default().lines().map(str::trim)
            .filter(|s| !s.is_empty() && *s != "$$" && !s.starts_with("<a ") && !s.starts_with('#')) {
            passages.push(json!({"index":passages.len(),"lid":lid,"quote":quote}));
        }
    }
    let lines: Vec<_> = packet["response"].as_str().unwrap_or_default().lines().filter(|l| !l.trim().is_empty())
        .enumerate().map(|(index,text)| json!({"index":index,"text":text})).collect();
    json!({"language":locale,"question":packet["prompt"],"criteria":packet["contract"]["rule"]["items"],
        "answer_lines":lines,"book_passages":passages})
}

fn worked_items(packet: &Value, result: &Value) -> Result<Value, ToolError> {
    let items = result["items"].as_array().ok_or_else(|| invalid("本次批改未完成，请重试"))?;
    let at = |list: &str, id: &Value| id.as_u64().and_then(|i| packet[list].get(i as usize))
        .ok_or_else(|| invalid("批改引用了未提供的行，请重试"));
    let mut evaluated = Vec::with_capacity(items.len());
    for item in items {
        let response = if item["answer_line"].is_null() { "" }
            else { at("answer_lines", &item["answer_line"])?["text"].as_str().unwrap_or("") };
        let mut sources = vec![];
        for id in item["passages"].as_array().ok_or_else(|| invalid("批改缺少教材引用"))? {
            let passage = at("book_passages", id)?;
            sources.push(json!({"lid":passage["lid"],"quote":passage["quote"]}));
        }
        evaluated.push(json!({"id":item["id"],"verdict":item["verdict"],"reason":item["reason"],"response_quote":response,"sources":sources}));
    }
    Ok(json!(evaluated))
}

pub(crate) enum QuestionWork {
    Cached(Value),
    Step { request: CompletionRequest, action: TeachingEvent },
    Grade { packet: Value, action_id: String },
}

#[derive(Deserialize)]
struct StepRequest {
    operation_id: String,
    delivery_ref: String,
    text: String,
    #[serde(default)] selected: Vec<String>,
    locale: runtime::response_language::ResponseLanguage,
}

impl QuestionWork {
    pub(crate) fn prepare(state: &PrivateBookContext<'_>, route: &str, input: &Value, now: &str) -> Result<Self, ToolError> {
        if route == "tutor/grade" {
            let id = input["action_ref"].as_str().ok_or_else(|| invalid("缺少已提交的答案"))?;
            let saved = referenced_delivery(state, id)?;
            let action = event(state, id)?;
            if saved.payload["move"]["answer_input"]["kind"] != "worked_solution"
                || action.kind != TeachingFact::LearnerAction || !matches!(action.payload["action"].as_str(), Some("submit" | "revise"))
                || activity_status(state, &saved)? != "active" {
                return Err(invalid("只支持已提交的计算题"));
            }
            let packet = assessment_packet(state, &action)?;
            if !packet["existing"].is_null() { return Ok(Self::Cached(save_assessment_items(state,id,&packet,Value::Null)?)); }
            return Ok(Self::Grade { packet: worked_packet(&packet,&input["locale"]), action_id: id.to_owned() });
        }
        let input: StepRequest = serde_json::from_value(input.clone()).map_err(|e| invalid(e.to_string()))?;
        let saved = delivery(state, &input.delivery_ref)?;
        let movement = &saved.payload["move"];
        let prior = state.user.learning_store_for_workspace(state.learning_workspace_id)?.teaching_activity_events(&saved.event_id)?;
        if input.text.trim().is_empty() || input.operation_id.is_empty()
            || !movement["actions"].as_array().is_some_and(|a| a.iter().any(|s| s == "submit"))
            || !prior.iter().any(|e| e.kind == TeachingFact::Displayed) || activity_status(state, &saved)? != "active" {
            return Err(invalid("请在当前练习题里写下要问的那一步"));
        }
        if !input.selected.is_empty() {
            validate_answer(&movement["answer_input"], &json!(input.selected).to_string())?;
        }
        let action = append(state, &saved.binding, format!("step:{}", input.operation_id), TeachingFact::LearnerAction,
            vec![saved.event_id.clone()], json!({"delivery_ref":saved.event_id,"action":"ask_step","response":input.text.trim(),
                "selected":input.selected}), now)?;
        if let Some(reply) = prior.iter().find(|e| e.kind == TeachingFact::HelpDelivered && e.payload["step_ref"] == action.event_id) {
            return Ok(Self::Cached(json!({"help_ref":reply.event_id,"text":reply.payload["text"]})));
        }
        let shown: BTreeSet<_> = prior.iter().filter(|e| e.kind == TeachingFact::HelpDisplayed)
            .filter_map(|e| e.payload["step_ref"].as_str()).collect();
        let history: Vec<_> = steps(&prior).into_iter().filter(|s| s["step_ref"].as_str().is_some_and(|id| shown.contains(id)))
            .rev().take(3).collect();
        let other_help: Vec<_> = prior.iter().rev().filter(|e| e.kind == TeachingFact::HelpDisplayed && e.payload["action"] != "ask_step")
            .filter_map(|e| e.payload["text"].as_str()).take(3).collect();
        let source = frozen_contract(state, &saved)?.and_then(|(selected, contract)| {
            let lid = contract.source_lids.first()?;
            selected.payload["scoring_sources"][lid].as_str().map(|s| s.chars().take(1800).collect::<String>())
        });
        let request = CompletionRequest { system: include_str!("question_step_prompt.md").into(),
            user: json!({"language":input.locale,"question":movement["prompt"],"input":movement["answer_input"],
                "choices_selected":input.selected,"student_step":input.text.trim(),"other_displayed_help":other_help,
                "previous_exchanges":history.into_iter().rev().collect::<Vec<_>>(),"book_excerpt":source}).to_string(),
            followup_user: None, images: vec![], output_token_limit: Some(1200), reasoning_effort: Some("low".into()) };
        Ok(Self::Step { request, action })
    }

    pub(crate) fn execute(&self, adapter: &dyn ModelAdapter) -> Result<Value, ToolError> {
        let provider_error = |e: runtime::AdapterError| e.spend_stop.map(|s| s.tool_error()).unwrap_or_else(||
            crate::user_storage_paths::error("QUESTION_MODEL_UNAVAILABLE", "provider", &e.message));
        match self {
            Self::Cached(value) => Ok(value.clone()),
            Self::Step { request, .. } => {
                adapter.set_model_purpose("question_step");
                let reply = adapter.complete_structured(request.clone()).map_err(provider_error)?;
                if !matches!(reply["hint_kind"].as_str(), Some("in_problem" | "structure" | "worked_step"))
                    || reply["text"].as_str().is_none_or(|s| s.trim().is_empty()) {
                    return Err(invalid("这一步的引导未生成，请重试"));
                }
                Ok(json!({"text":reply["text"],"hint_kind":reply["hint_kind"]}))
            }
            Self::Grade { packet, .. } => {
                adapter.set_model_purpose("tutor_assessment");
                let result = adapter.complete_structured(CompletionRequest { system: include_str!("worked_grading_prompt.md").into(),
                    user: packet.to_string(), followup_user:None, images:vec![], output_token_limit:Some(2400), reasoning_effort:Some("low".into()) })
                    .map_err(provider_error)?;
                worked_items(packet,&result)
            }
        }
    }

    pub(crate) fn finish(&self, state: &PrivateBookContext<'_>, answer: Value, now: &str) -> Result<Value, ToolError> {
        match self {
            Self::Cached(value) => Ok(value.clone()),
            Self::Grade { action_id, .. } => {
                let action = event(state, action_id)?;
                let packet = assessment_packet(state, &action)?;
                save_assessment_items(state, action_id, &packet, answer)
            }
            Self::Step { action, .. } => {
                let original = event(state, &action.event_id)?;
                if original.binding != action.binding || original.binding.source_id != state.book.base.book_id {
                    return Err(invalid("请回到原题查看引导"));
                }
                let id = format!("help:{}", action.event_id);
                let store = state.user.learning_store_for_workspace(state.learning_workspace_id)?;
                if let Some(help) = store.teaching_activity_events(action.payload["delivery_ref"].as_str().unwrap_or(""))?
                    .into_iter().find(|e| e.event_id == id) {
                    return Ok(json!({"help_ref":help.event_id,"text":help.payload["text"]}));
                }
                let help = append(state, &action.binding, id, TeachingFact::HelpDelivered,
                    vec![action.event_id.clone()], json!({"action":"ask_step","delivery_ref":action.payload["delivery_ref"],
                        "step_ref":action.event_id,"text":answer["text"],"hint_kind":answer["hint_kind"]}), now)?;
                Ok(json!({"help_ref":help.event_id,"text":help.payload["text"]}))
            }
        }
    }
}
