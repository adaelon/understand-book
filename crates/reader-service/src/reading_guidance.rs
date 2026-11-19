//! One-shot input suggestions. No agent run, teaching event or chat turn is created.
use read_tools::{Book, ToolError};
use runtime::{CompletionRequest, ModelAdapter};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose { Welcome, Understand, Explore, Selection, Followup }

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope { Selection, Chapter, Section, Material, Media }

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuidanceRequest {
    pub purpose: Purpose,
    pub scope: Scope,
    pub lid: Option<String>,
    #[serde(default)] pub quote: String,
    #[serde(default)] pub material_title: String,
    #[serde(default)] pub scope_label: String,
    #[serde(default)] pub known_intent: String,
    #[serde(default)] pub medium: String,
    #[serde(default)] pub recent_turns: Vec<GuidanceTurn>,
    pub locale: runtime::response_language::ResponseLanguage,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuidanceTurn {
    pub turn_id: String,
    pub user: String,
    pub assistant: String,
    #[serde(default)] pub source_quote: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Guidance {
    pub question: String,
    pub draft: String,
}

fn bounded(text: &str, limit: usize) -> String { text.chars().take(limit).collect() }
fn conversation_excerpt(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit { return text.into(); }
    // Keep the end: an answer can already contain the next question or choice.
    let tail: String = text.chars().rev().take(limit / 3).collect::<Vec<_>>().into_iter().rev().collect();
    format!("{}\n[…]\n{}", bounded(text, limit - limit / 3 - 5), tail)
}
fn invalid(message: &str) -> ToolError {
    crate::user_storage_paths::error("INVALID_READING_GUIDANCE", "validation", message)
}

pub fn prepare(book: &Book, mut request: GuidanceRequest) -> Result<CompletionRequest, ToolError> {
    request.material_title = bounded(&request.material_title, 200);
    request.scope_label = bounded(&request.scope_label, 200);
    request.known_intent = bounded(&request.known_intent, 600);
    request.quote = bounded(&request.quote, 2000);
    request.medium = bounded(&request.medium, 20);
    request.recent_turns = request.recent_turns.into_iter().rev().take(2).collect::<Vec<_>>().into_iter().rev().map(|mut turn| {
        turn.turn_id = bounded(&turn.turn_id, 100);
        turn.user = conversation_excerpt(&turn.user, 1000);
        turn.assistant = conversation_excerpt(&turn.assistant, 2400);
        turn.source_quote = bounded(&turn.source_quote, 1000);
        turn
    }).collect();
    // Read only a bounded page, even when the selected LID covers an entire book.
    let lid = request.lid.as_deref().or_else(|| book.base.lid_nodes.first().map(|node| node.lid.as_str()));
    let excerpt = if matches!(request.purpose, Purpose::Followup) {
        // A reader may have moved since this answer. Do not substitute a new chapter.
        request.recent_turns.last().map(|turn| turn.source_quote.clone()).unwrap_or_default()
    } else if matches!(request.scope, Scope::Selection) && !request.quote.trim().is_empty() {
        request.quote.clone()
    } else if let Some(lid) = lid { bounded(&book.text_page(lid, None, None)?.text, 2000) }
    else { String::new() };
    Ok(CompletionRequest {
        followup_user: None, images: Vec::new(), output_token_limit: Some(1000), reasoning_effort: Some("low".into()),
        system: include_str!("reading_guidance_prompt.md").into(),
        user: json!({"state":request,"source_excerpt":excerpt}).to_string(),
    })
}

pub fn execute(adapter: &dyn ModelAdapter, request: CompletionRequest) -> Result<Option<Guidance>, ToolError> {
    adapter.set_model_purpose("reading_guidance");
    // Exactly one completion; malformed output falls back to the existing UI copy.
    let value = adapter.complete_structured(request).map_err(|error| error.spend_stop
        .map(|stop| stop.tool_error()).unwrap_or_else(|| crate::user_storage_paths::error(
            "READING_GUIDANCE_PROVIDER_FAILED", "provider", &error.message)))?;
    parse(value)
}

fn parse(value: Value) -> Result<Option<Guidance>, ToolError> {
    if value.get("show").and_then(Value::as_bool) == Some(false) { return Ok(None); }
    let mut guidance: Guidance = serde_json::from_value(value).map_err(|_| invalid("Expected a question and a draft"))?;
    guidance.question = guidance.question.trim().into();
    guidance.draft = guidance.draft.trim().into();
    if guidance.question.is_empty() || guidance.draft.is_empty()
        || guidance.question.chars().count() > 240 || guidance.draft.chars().count() > 400 {
        return Err(invalid("Reading guidance must be a short question and an editable draft"));
    }
    Ok(Some(guidance))
}

enum GuidanceTask {
    Reading(CompletionRequest),
    Question(crate::teaching::question::QuestionWork),
}

pub struct GuidanceWork(GuidanceTask);

impl GuidanceWork {
    pub fn prepare(state: &crate::PrivateBookContext<'_>, action: &str, input: &Value, now: &str) -> Result<Self, ToolError> {
        if action == "reader/guidance" {
            let reader = serde_json::from_value(input.clone()).map_err(|_| invalid("Invalid reading guidance request"))?;
            Ok(Self(GuidanceTask::Reading(prepare(state.book, reader)?)))
        } else {
            Ok(Self(GuidanceTask::Question(crate::teaching::question::QuestionWork::prepare(state, action, input, now)?)))
        }
    }

    pub fn requires_finish(&self) -> bool {
        matches!(self.0, GuidanceTask::Question(_))
    }

    pub fn execute(&self, adapter: &dyn ModelAdapter) -> Result<Value, ToolError> {
        match &self.0 {
            GuidanceTask::Reading(request) => Ok(json!(execute(adapter, request.clone())?)),
            GuidanceTask::Question(work) => work.execute(adapter),
        }
    }

    pub fn finish(&self, state: &crate::PrivateBookContext<'_>, value: Value, now: &str) -> Result<Value, ToolError> {
        match &self.0 { GuidanceTask::Reading(_) => Ok(value), GuidanceTask::Question(work) => work.finish(state, value, now) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn book() -> Book {
        let source = "趋势策略预测方向。套利策略研究相对价格。".repeat(200);
        let mut base = base_schema::sample_base();
        for node in &mut base.lid_nodes { node.span.end = source.encode_utf16().count(); }
        Book::new(base, &source)
    }
    fn input(purpose: &str) -> GuidanceRequest {
        serde_json::from_value(json!({"purpose":purpose,"scope":"section","lid":"1.1",
            "material_title":"趋势与套利","scope_label":"当前小节","locale":"zh-CN"})).unwrap()
    }
    struct Probe { calls: Mutex<Vec<CompletionRequest>>, output: Value }
    impl ModelAdapter for Probe {
        fn complete(&self, _: CompletionRequest) -> Result<runtime::ParsedResponse, runtime::AdapterError> {
            panic!("guidance must use a single structured completion")
        }
        fn chat(&self, _: &runtime::AgentRequestPlan) -> Result<runtime::AssistantTurn, runtime::AdapterError> {
            panic!("guidance must not enter an agent run")
        }
        fn complete_structured(&self, request: CompletionRequest) -> Result<Value, runtime::AdapterError> {
            self.calls.lock().unwrap().push(request); Ok(self.output.clone())
        }
    }
    #[test]
    fn reading_guidance_uses_one_completion_with_bounded_source_and_known_state() {
        for purpose in ["welcome", "understand", "explore", "selection"] {
            let mut input = input(purpose); input.known_intent = "想理解适用条件".into();
            let work = prepare(&book(), input).unwrap();
            // The native provider's json_object mode requires JSON in the prompt.
            assert!(work.system.to_lowercase().contains("json"));
            let data: Value = serde_json::from_str(&work.user).unwrap();
            assert_eq!(data["state"]["purpose"], purpose);
            assert_eq!(data["state"]["known_intent"], "想理解适用条件");
            assert_eq!(data["source_excerpt"].as_str().unwrap().chars().count(), 2000);
            let adapter = Probe { calls: Mutex::new(vec![]), output: json!({"question":"哪一点让你犹豫？","draft":"我想弄清两种策略的区别。"}) };
            let result = execute(&adapter, work).unwrap().unwrap();
            assert_eq!(result.draft, "我想弄清两种策略的区别。");
            assert_eq!(adapter.calls.lock().unwrap().len(), 1);
        }
    }
    #[test]
    fn reading_guidance_keeps_selection_and_language_and_rejects_missing_scope() {
        let mut input = input("explore"); input.scope = Scope::Selection;
        input.quote = "我选中的一句原文".into(); input.locale = runtime::response_language::ResponseLanguage::English;
        let data: Value = serde_json::from_str(&prepare(&book(), input).unwrap().user).unwrap();
        assert_eq!(data["source_excerpt"], "我选中的一句原文"); assert_eq!(data["state"]["locale"], "en");
        let mut input = self::input("understand"); input.lid = Some("missing".into());
        assert!(prepare(&book(), input).is_err());
    }
    #[test]
    fn reading_guidance_does_not_retry_invalid_generated_text() {
        for output in [json!({"draft":""}), json!({"question":"问题","draft":"  "}),
            json!({"question":"问题","draft":"字".repeat(401)})] {
            let adapter = Probe { calls: Mutex::new(vec![]), output };
            assert!(execute(&adapter, prepare(&book(), input("welcome")).unwrap()).is_err());
            assert_eq!(adapter.calls.lock().unwrap().len(), 1);
        }
    }
    #[test]
    fn reading_guidance_followup_uses_delivered_dialogue_not_a_new_reading_position() {
        let mut input = input("followup");
        input.lid = Some("new-reading-position".into());
        input.recent_turns = (0..3).map(|i| GuidanceTurn {
            turn_id: format!("turn-{i}"), user: format!("我认为{}因为……最后的理由", "字".repeat(1200)),
            assistant: format!("解释{}最后已经向用户提了一个问题？", "字".repeat(3000)), source_quote: format!("第{i}轮的原文"),
        }).collect();
        let request = prepare(&book(), input).unwrap();
        let state: Value = serde_json::from_str(&request.user).unwrap();
        let turns = state["state"]["recent_turns"].as_array().unwrap();
        assert_eq!(turns.len(), 2); assert_eq!(turns[0]["turn_id"], "turn-1");
        assert_eq!(state["source_excerpt"], "第2轮的原文");
        assert!(turns[1]["user"].as_str().unwrap().ends_with("最后的理由"));
        assert!(turns[1]["user"].as_str().unwrap().chars().count() <= 1000);
        assert!(turns[1]["assistant"].as_str().unwrap().ends_with("最后已经向用户提了一个问题？"));
        assert!(turns[1]["assistant"].as_str().unwrap().chars().count() <= 2400);
        let adapter = Probe { calls: Mutex::new(vec![]), output: json!({"show":false}) };
        assert!(execute(&adapter, request).unwrap().is_none());
        assert_eq!(adapter.calls.lock().unwrap().len(), 1);
    }
}
