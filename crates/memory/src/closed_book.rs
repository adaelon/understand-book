//! Closed-book practice is a projection of the existing private teaching log.
use crate::{
    learning::{storage, LearningStore},
    teaching::{invalid, TeachingBinding, TeachingEvent, TeachingFact},
};
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct ClosedBookCourseOrigin {
    pub round_index: u32,
    pub stage: crate::course::CourseStage,
    pub decision_phase: Option<crate::course::CourseDecisionPhase>,
    /// The already-revealed task, retained when a later attempt uses a variant.
    pub question: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct ClosedBookAnswerOrigin {
    pub turn_id: String,
    pub question: String,
    pub answer: String,
}

impl ClosedBookCourseOrigin {
    pub fn from_run(run: &crate::course::CourseRun) -> Self {
        use crate::course::{CourseDecisionPhase, CourseStage};
        let content = run.lesson.round_content(run.round_index as usize);
        let question = match run.progress.stage {
            CourseStage::Explanation => content.reflection_prompt.clone()
                .unwrap_or_else(|| run.lesson.summary.objective.clone()),
            CourseStage::Task | CourseStage::Feedback => run.decision.as_ref()
                .filter(|d| d.phase == CourseDecisionPhase::Transfer)
                .and(run.lesson.decision_teaching.as_ref())
                .map(|t| t.transfer_question.clone()).unwrap_or_else(|| run.lesson.task.question.clone()),
            CourseStage::Prediction => content.prediction.question.clone(),
            _ => run.lesson.summary.objective.clone(),
        };
        Self { round_index: run.round_index, stage: run.progress.stage,
            decision_phase: run.decision.as_ref().map(|d| d.phase), question }
    }

    pub fn matches_run(&self, run: &crate::course::CourseRun) -> bool {
        self.round_index == run.round_index && self.stage == run.progress.stage
            && self.decision_phase == run.decision.as_ref().map(|d| d.phase)
    }
}

pub fn revision_conflict() -> ToolError {
    ToolError {
        error_code: "CLOSED_BOOK_REVISION_CONFLICT".into(),
        category: "conflict".into(),
        message: "练习状态已更新，请刷新后重试".into(),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct ClosedBookPractice {
    pub id: String,
    pub owner_id: String,
    pub course: bool,
    pub chat_session_id: String,
    pub source_id: String,
    pub source_revision: String,
    pub goal_revision: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub course_origin: Option<ClosedBookCourseOrigin>,
    /// The exact delivered explanation selected by the learner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub answer_origin: Option<ClosedBookAnswerOrigin>,
    /// Set only when the completion reply has actually been delivered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub result_ref: Option<String>,
    pub revision: u32,
    pub status: String,
    /// Projected against the current owner and source before returning to the UI.
    #[serde(default)]
    pub can_resume: bool,
    pub target: String,
    pub criteria: String,
    pub attempt: u32,
    pub assisted: bool,
    pub needs_retry: bool,
    pub visible_turn_ids: Vec<String>,
    pub draft: String,
    pub completion_reason: String,
    pub learner_quote: String,
}

impl ClosedBookPractice {
    pub fn binding(&self, turn: &str) -> TeachingBinding {
        TeachingBinding {
            tutor_session_id: self.owner_id.clone(),
            session_revision: 0,
            control_revision: 0,
            source_id: self.source_id.clone(),
            source_revision: self.source_revision.clone(),
            map_revision: None,
            chat_session_id: self.chat_session_id.clone(),
            turn_id: turn.into(),
        }
    }
}

impl LearningStore {
    pub fn closed_book_operation(
        &self,
        operation: &str,
    ) -> Result<Option<ClosedBookPractice>, ToolError> {
        let raw: Option<String> = self.connection.query_row(
            "SELECT json_extract(event,'$.payload') FROM teaching_trace WHERE learning_workspace_id=?
             AND event_id=? AND json_extract(event,'$.kind')='closed_book_changed'",
            params![self.workspace_scope, format!("closed-book:{operation}")], |r| r.get(0))
            .optional().map_err(storage)?;
        raw.map(|r| serde_json::from_str(&r).map_err(storage)).transpose()
    }

    pub fn closed_book(
        &self,
        chat: &str,
        source: &str,
    ) -> Result<Option<ClosedBookPractice>, ToolError> {
        let raw: Option<String> = self.connection.query_row(
            "SELECT json_extract(event,'$.payload') FROM teaching_trace WHERE learning_workspace_id=?
             AND json_extract(event,'$.kind')='closed_book_changed'
             AND json_extract(event,'$.binding.chat_session_id')=? AND json_extract(event,'$.binding.source_id')=? ORDER BY seq DESC LIMIT 1",
            params![self.workspace_scope,chat,source], |r|r.get(0)).optional().map_err(storage)?;
        raw.map(|r| serde_json::from_str(&r).map_err(storage))
            .transpose()
    }

    pub fn save_closed_book(
        &mut self,
        mut value: ClosedBookPractice,
        operation: &str,
        now: &str,
    ) -> Result<ClosedBookPractice, ToolError> {
        let id = format!("closed-book:{operation}");
        if let Some(previous) = self.closed_book_operation(operation)? {
            if previous.id != value.id || previous.chat_session_id != value.chat_session_id
                || previous.source_id != value.source_id {
                return Err(invalid("练习操作已用于另一目标"));
            }
            return Ok(previous);
        }
        let current = self.closed_book(&value.chat_session_id, &value.source_id)?;
        if current.as_ref().map_or(0, |v| v.revision) != value.revision {
            return Err(revision_conflict());
        }
        value.revision += 1;
        self.append_teaching(&TeachingEvent {
            event_id: id,
            binding: value.binding(operation),
            kind: TeachingFact::ClosedBookChanged,
            causal_refs: vec![],
            payload: serde_json::to_value(&value).map_err(storage)?,
            occurred_at: now.into(),
        })?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_book_receipts_replay_without_rewriting_newer_progress() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("learning.db");
        let mut store = LearningStore::open(&path, false).unwrap().with_workspace(Some("workspace"));
        let practice = ClosedBookPractice {
            id: "practice".into(), owner_id: "stage".into(), course: false,
            chat_session_id: "chat".into(), source_id: "book".into(), source_revision: "v1".into(),
            goal_revision: None, course_origin: None, answer_origin: None, result_ref: None,
            revision: 0, status: "active".into(), can_resume: true,
            target: "解释全程时间".into(), criteria: String::new(), attempt: 1,
            assisted: false, needs_retry: false, visible_turn_ids: vec![], draft: String::new(),
            completion_reason: String::new(), learner_quote: String::new(),
        };
        let started = store.save_closed_book(practice.clone(), "start", "t0").unwrap();
        let mut next = started.clone();
        next.draft = "包含途中停车时间".into();
        let drafted = store.save_closed_book(next, "draft", "t1").unwrap();
        assert_eq!(store.save_closed_book(practice, "start", "t2").unwrap(), started);
        assert_eq!(store.closed_book("chat", "book").unwrap(), Some(drafted));
        assert_eq!(store.save_closed_book(started.clone(), "stale-draft", "t3").unwrap_err().error_code,
            "CLOSED_BOOK_REVISION_CONFLICT");
        let mut other = started;
        other.chat_session_id = "another-chat".into();
        assert_eq!(store.save_closed_book(other, "start", "t4").unwrap_err().error_code, "TEACHING_INVALID");
        let other = LearningStore::open(&path, false).unwrap().with_workspace(Some("another-workspace"));
        assert!(other.closed_book_operation("start").unwrap().is_none());
    }
}
