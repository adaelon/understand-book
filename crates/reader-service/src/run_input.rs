//! Persisted Reader turn input, independent of account admission and charging.
use crate::*;
use base_schema::PublishedBookRef;
fn invalid() -> ToolError { crate::user_storage_paths::error("RUN_INPUT_INVALID", "validation", "Run request could not be completed") }
fn invalid_sprint() -> ToolError { crate::user_storage_paths::error("EXAM_SPRINT_INVALID", "validation", "请填写有效考试日期、复习范围和每天可用时间。") }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderBinding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<runtime::model_settings::ModelBinding>,
    #[serde(default)]
    pub api_protocol: runtime::model_settings::ApiProtocol,
    pub mode: String,
    pub endpoint: String,
    pub model: String,
    pub profile: runtime::ModelRuntimeProfile,
}
impl ProviderBinding {
    pub fn from_config(config: &runtime::ProviderConfig) -> Self {
        Self {
            configuration: config.options.binding.clone(), api_protocol: config.options.api_protocol,
            mode: config.mode.as_str().into(), endpoint: config.base_url.clone(), model: config.model.clone(),
            profile: runtime::ProviderRegistry::adapter_from_config(config.clone()).model_runtime_profile(),
        }
    }
}
/// Only the necessary request/scene/provider metadata; credentials and temporary candidates are absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrozenTurn<M = Vec<Message>> {
    #[serde(default)]
    pub board_context: Option<crate::board_api::FrozenBoardFollowUp>,
    #[serde(default)]
    pub exam_sprint: Option<crate::run_input::SprintInput>,
    pub version: u32,
    #[serde(default)]
    pub learning_workspace_id: Option<String>,
    #[serde(default)]
    pub learning_workspace_goal: Option<base_schema::WorkspaceGoal>,
    #[serde(default)]
    pub learning_workspace_members: Option<Vec<PublishedBookRef>>,
    pub owner: String,
    pub workspace: String,
    pub generation: u64,
    pub effect_revision: u64,
    pub chat: String,
    pub turn: String,
    pub ordinal: u64,
    pub head: Option<String>,
    pub publication: Option<PublishedBookRef>,
    pub normalized: Value,
    pub reader: Option<runtime::run_context::ReaderInputSnapshot>,
    #[serde(default)]
    pub music_question: Option<base_schema::MusicQuestionContext>,
    pub confirmation: Option<String>,
    pub provider: ProviderBinding,
    #[serde(default)]
    pub teaching_provider: Option<ProviderBinding>,
    pub message: String,
    pub agent_message: String,
    pub messages: M,
    pub quote: Option<AskQuote>,
    pub presentation: Option<runtime::presentation::PresentationFollowUp>,
    pub teaching: teaching::FrozenTeachingPreparation,
    pub goal: Option<runtime::goal::ResidentGoal>,
    pub now: String,
    pub preparation_complete: bool,
}
impl<M> FrozenTurn<M> {
    pub fn coach_provider_binding(&self) -> Value { json!(self.teaching_provider.as_ref().unwrap_or(&self.provider)) }
    pub fn map_messages<N>(self, map: impl FnOnce(M) -> N) -> FrozenTurn<N> {
        FrozenTurn {
            board_context: self.board_context,
            exam_sprint: self.exam_sprint,
            learning_workspace_id: self.learning_workspace_id,
            learning_workspace_goal: self.learning_workspace_goal,
            learning_workspace_members: self.learning_workspace_members,
            version: self.version, owner: self.owner, workspace: self.workspace,
            generation: self.generation, effect_revision: self.effect_revision, chat: self.chat,
            turn: self.turn, ordinal: self.ordinal, head: self.head, publication: self.publication,
            normalized: self.normalized, reader: self.reader, confirmation: self.confirmation,
            music_question: self.music_question,
            provider: self.provider, teaching_provider: self.teaching_provider, message: self.message, agent_message: self.agent_message,
            messages: map(self.messages), quote: self.quote, presentation: self.presentation,
            teaching: self.teaching, goal: self.goal, now: self.now,
            preparation_complete: self.preparation_complete,
        }
    }
    pub fn set_prepared(&mut self, teaching: teaching::FrozenTeachingPreparation) {
        self.teaching = teaching;
        self.preparation_complete = true;
    }
}
impl FrozenTurn {
    pub fn turn_ref(&self) -> AgentTurnRef {
        AgentTurnRef {
            session_id: self.chat.clone(),
            turn_id: self.turn.clone(),
            user_turn_ordinal: self.ordinal,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SprintInput {
    pub exam_date: String,
    #[serde(default)]
    pub syllabus: String,
    pub daily_minutes: Option<u32>,
    /// Calendar boundaries are explicitly the student's local offset, in minutes.
    pub utc_offset_minutes: i32,
    pub entry_mode: String,
}

impl SprintInput {
    fn date(&self) -> Result<time::Date, ToolError> {
        let format = time::format_description::parse("[year]-[month]-[day]").map_err(|_| invalid_sprint())?;
        time::Date::parse(&self.exam_date, &format).map_err(|_| invalid_sprint())
    }
    pub fn validate(&self) -> Result<(), ToolError> {
        self.date()?;
        if !matches!(self.entry_mode.as_str(), "diagnostic" | "question")
            || self.daily_minutes.is_some_and(|minutes| minutes == 0 || minutes > 1440)
            || !(-720..=840).contains(&self.utc_offset_minutes) { return Err(invalid_sprint()); }
        Ok(())
    }
    pub fn constraints(&self) -> Vec<String> {
        let mut constraints = vec![format!("考试日期：{}（UTC{:+} 分钟）；按当前日期安排复习。", self.exam_date, self.utc_offset_minutes)];
        constraints.push(if self.syllabus.trim().is_empty() { "考试范围暂未提供；先依据课程注明的适用范围，不猜测学校考纲。".into() }
            else { format!("已确认考试范围：{}", self.syllabus.trim()) });
        if let Some(minutes) = self.daily_minutes { constraints.push(format!("每天可用复习时间：{minutes} 分钟。")); }
        constraints.push(if self.entry_mode == "diagnostic" { "开始方式：先做短诊断，根据实际作答调整；观看视频不代表已掌握。".into() }
            else { "开始方式：直接处理我提出的问题，无需先做诊断；根据实际作答调整。".into() });
        constraints
    }
    pub fn remaining_days(&self, now: i64) -> Result<i64, ToolError> {
        let offset = time::UtcOffset::from_whole_seconds(self.utc_offset_minutes * 60).map_err(|_| invalid_sprint())?;
        let today = time::OffsetDateTime::from_unix_timestamp(now).map_err(|_| invalid_sprint())?.to_offset(offset).date();
        Ok((self.date()? - today).whole_days())
    }
    pub fn calendar(&self, now: i64, goal_revision: u32) -> Result<Value, ToolError> {
        let offset = time::UtcOffset::from_whole_seconds(self.utc_offset_minutes * 60).map_err(|_| invalid_sprint())?;
        let today = time::OffsetDateTime::from_unix_timestamp(now).map_err(|_| invalid_sprint())?.to_offset(offset).date();
        Ok(json!({"exam_date":self.exam_date,"current_local_date":today.to_string(),
            "utc_offset_minutes":self.utc_offset_minutes,"remaining_days":self.remaining_days(now)?,
            "workspace_goal_revision":goal_revision,"as_of":now}))
    }
}

pub fn normalize(workspace: &str, input: &Value) -> Result<Value, ToolError> {
    crate::interaction_input::InteractionInput::parse(input)?;
    runtime::response_language::ResponseLanguage::from_request(input)?;
    presentation_requested(input)?;
    let message = input["message"]
        .as_str()
        .ok_or_else(invalid)?;
    let mut v = json!({"workspace_id":workspace,"message":message,"display_user":input["display_user"].as_str().unwrap_or(message)});
    if input["entry_action"] == "explain_back" {
        for key in ["client_request_id", "operation_id", "expression_id"] { v[key] = input[key].clone(); }
        if !input["teach_back_reply"].is_null() { v["teach_back_reply"] = input["teach_back_reply"].clone(); }
        if input["course_context"].is_object() { v["course_context"] = input["course_context"].clone(); }
    }
    for key in [
        "session_id",
        "generation",
        "expected_revision",
        "attachment_id",
        "published_book_ref",
        "expected_chat_head",
        "question_anchor_lid",
        "question_quote",
        "presentation_follow_up",
        "note_mem_id",
        "teaching_ref",
        "goal_id",
        "goal_action",
        "retry_of",
        "video_window_id",
        "video_draft_id",
    ] {
        v[key] = input[key].clone();
    }
    // Keep ordinary requests byte-for-byte equivalent to the pre-TC1 normalized
    // value, so retrying a previously accepted key does not become a conflict.
    for key in ["scope", "personal_chat_ref", "note_refs", "note_reply_to", "understanding_ref", "board_follow_up", "response_language", "presentation_requested", "music_question", "entry_action", "learning_purpose", "note_ref", "answer_ref", "teaching_choice", "source_id", "source_revision", "chapter_lid", "teaching_response_origin"] {
        if let Some(value) = input.get(key).filter(|v| !v.is_null()) {
            v[key] = value.clone();
        }
    }
    Ok(v)
}
