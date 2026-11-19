//! Append-only teaching facts. Payloads are exact deliveries/observations, never mastery.
use crate::learning::{storage, LearningStore};
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeachingBinding {
    pub tutor_session_id: String,
    pub session_revision: u32,
    pub control_revision: u32,
    pub source_id: String,
    pub source_revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_revision: Option<String>,
    pub chat_session_id: String,
    pub turn_id: String,
}

/// A source-grounded focus; formal object identity is optional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct TeachingTarget {
    /// Private relationship identity. Reuse an offered candidate only for the same relationship.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub focus_ref: Option<String>,
    pub learning_focus: String,
    pub expected_performance: String,
    pub capability: String,
    pub source_bindings: Vec<TeachingSourceBinding>,
    // Runtime-bound observations from source references, not model-authored input.
    #[schemars(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub media_bindings: Option<Vec<TeachingMediaBinding>>,
    // Read-time source snapshots, bound by the host; never model-authored targets.
    #[schemars(skip)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub music_bindings: Option<Vec<TeachingMusicBinding>>,
    #[serde(default)]
    pub object_refs: Vec<TeachingObjectRevision>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct TeachingSourceBinding {
    pub source_id: String,
    pub source_revision: String,
    pub start_lid: String,
    pub end_lid: String,
}

/// Exact observed media, independent of transcript availability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct TeachingMediaBinding {
    pub source_id: String,
    pub source_revision: String,
    pub target: base_schema::EvidenceTarget,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct TeachingMusicBinding {
    pub source_id: String,
    pub source_revision: String,
    pub target: base_schema::MusicSourceTarget,
}

fn media_identity_matches(target: &base_schema::EvidenceTarget, source: &str) -> bool {
    use base_schema::EvidenceTarget;
    match target {
        EvidenceTarget::Text { publication, .. } | EvidenceTarget::Visual { publication, .. }
            | EvidenceTarget::Audio { publication, .. } => publication.book_id == source && !publication.publication_id.trim().is_empty(),
        EvidenceTarget::Joint { members } => !members.is_empty() && members.iter().all(|member|media_identity_matches(&member.target,source)),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct TeachingObjectRevision {
    pub source_id: String,
    pub object_id: String,
    #[ts(type = "number")]
    pub object_revision: u64,
}

impl TeachingTarget {
    /// Storage validates identity. The turn owner checks these ranges against actual reads.
    pub fn validate(&self, binding: &TeachingBinding) -> Result<(), ToolError> {
        if [&self.learning_focus, &self.expected_performance, &self.capability]
            .iter().any(|s| s.trim().is_empty()) || (self.source_bindings.is_empty() && self.media_bindings.as_ref().is_none_or(Vec::is_empty)
                && self.music_bindings.as_ref().is_none_or(Vec::is_empty))
            || self.source_bindings.iter().any(|source| {
                source.source_id.trim().is_empty() || source.source_revision.trim().is_empty()
                    || source.source_id != binding.source_id || source.source_revision != binding.source_revision
                    || source.start_lid.trim().is_empty() || source.end_lid.trim().is_empty()
            })
            || self.media_bindings.iter().flatten().any(|source| source.source_id != binding.source_id
                || source.source_revision != binding.source_revision || source.source_revision.trim().is_empty()
                || !media_identity_matches(&source.target, &binding.source_id))
            || self.music_bindings.iter().flatten().any(|source| source.source_id != binding.source_id
                || source.source_revision != binding.source_revision || source.source_revision.trim().is_empty()
                || source.target.publication.book_id != binding.source_id || source.target.publication.publication_id.trim().is_empty()
                || source.target.audio.recording.recording_id.trim().is_empty() || source.target.audio.recording.revision.trim().is_empty()
                || !source.target.audio.start_ms.is_finite() || !source.target.audio.end_ms.is_finite()
                || source.target.audio.start_ms < 0.0 || source.target.audio.end_ms <= source.target.audio.start_ms
                || source.target.score.as_ref().is_some_and(|score| score.score.score_id.trim().is_empty() || score.score.revision.trim().is_empty()
                    || score.source_ids.is_empty() || score.source_ids.iter().any(|id|id.trim().is_empty())))
            || self.object_refs.iter().any(|object| object.source_id != binding.source_id
                || object.object_id.trim().is_empty() || object.object_revision == 0)
        {
            return Err(invalid("教学目标缺少焦点、期望表现或确切来源/对象版本"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeachingFact {
    TurnBound,
    /// The system accepted an interpretation in this run; never a new learner expression.
    EvidenceInterpreted,
    /// The teacher's reviewed work and corrections, never learner evidence.
    AgentWorkReviewed,
    /// A sourced correction of the teacher's own statement; never learner evidence.
    TeacherCorrectionRecorded,
    /// The main teacher's applicability decision for a historical correction.
    TeacherCorrectionUsed,
    /// A coach's future guidance, never a delivered activity or learner evidence.
    CoachGuided,
    DesignRevised,
    MoveSelected,
    MessageDelivered,
    Displayed,
    LearnerAction,
    HelpDelivered,
    HelpDisplayed,
    UsageObserved,
    ContinuationDisplayed,
    ContinuationChosen,
    /// An after-delivery soundness verdict on a practice problem; never learner evidence.
    PracticeChecked,
    ClosedBookChanged,
    ClosedBookProposed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct ContinuationDisplayed {
    pub delivery_ref: String,
    /// Actual visible order, not all options in the offered set.
    pub option_ids: Vec<String>,
    pub recommended_option_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct ContinuationChosen {
    pub delivery_ref: String,
    pub option_id: String,
    pub accepted_turn_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeachingEvent {
    pub event_id: String,
    pub binding: TeachingBinding,
    pub kind: TeachingFact,
    pub causal_refs: Vec<String>,
    pub payload: Value,
    pub occurred_at: String,
}

impl TeachingEvent {
    /// A retry may have a new attempt time, but must preserve the saved fact.
    pub fn validate_unchanged_retry(&self, previous: &Self) -> Result<(), ToolError> {
        let mut retry = self.clone();
        retry.occurred_at = previous.occurred_at.clone();
        if *previous != retry {
            return Err(invalid("同一教学操作不能改写已保存事实"));
        }
        Ok(())
    }

    /// Actual learner expression; UI labels and assistant quotations remain context.
    pub fn learner_expression(&self) -> Option<&str> {
        let text = match self.kind {
            TeachingFact::UsageObserved if self.payload["usage"]["kind"] == "reading_action" =>
                self.payload.get("interaction").filter(|v|v.is_object())
                    .map(|v| &v["user_supplement"]).unwrap_or(&self.payload["response"]).as_str(),
            TeachingFact::UsageObserved if self.payload["usage"]["kind"] == "user_message" => self.payload["response"].as_str(),
            TeachingFact::LearnerAction => self.payload["response"].as_str(),
            _ => None,
        };
        text.filter(|s| !s.trim().is_empty()
            && self.payload["response_origin"]["kind"] != "interface_request"
            && !(self.payload["response_origin"]["kind"] == "presentation_prefill"
                && self.payload["response_origin"]["prefill_text"].as_str() == Some(*s)))
    }
}

pub fn invalid(message: impl Into<String>) -> ToolError {
    ToolError {
        error_code: "TEACHING_INVALID".into(),
        category: "validation".into(),
        message: message.into(),
    }
}

impl LearningStore {
    /// A learner claim must resolve to an expression from this source and session.
    /// Delivered help and interface choices remain context, even when they carry text.
    pub fn learner_expression_fact(
        &self,
        binding: &TeachingBinding,
        id: &str,
    ) -> Result<TeachingEvent, ToolError> {
        let fact = self.teaching_event(id)?;
        if fact.binding.source_id != binding.source_id
            || fact.binding.source_revision != binding.source_revision
            || fact.binding.tutor_session_id != binding.tutor_session_id
        {
            return Err(invalid("理解依据不属于当前来源、版本或会话"));
        }
        if fact.learner_expression().is_none() {
            return Err(invalid("理解依据需要真实用户表达；交付、展示、点击或未修改的预填内容仅是帮助背景。理解未知时继续讲清当前问题，无需创建理解假设"));
        }
        Ok(fact)
    }

    pub fn current_teaching_design(&self, session: &str, source: &str, revision: &str) -> Result<Option<TeachingEvent>, ToolError> {
        let event: Option<String> = self.connection.query_row(
            "SELECT event FROM teaching_trace WHERE learning_workspace_id=? AND session_id=? AND json_extract(event,'$.kind')='design_revised' AND json_extract(event,'$.binding.source_id')=? AND json_extract(event,'$.binding.source_revision')=? ORDER BY seq DESC LIMIT 1",
            params![self.workspace_scope, session,source,revision], |r|r.get(0)).optional().map_err(storage)?;
        event.map(|event|serde_json::from_str(&event).map_err(storage)).transpose()
    }

    /// Only delivered moves resolve or replace an invitation. Candidate selections do not.
    pub fn pending_response_invitation(&self, session: &str, source: &str, revision: &str) -> Result<Option<TeachingEvent>, ToolError> {
        self.find_response_invitation(session, source, revision, None)
    }

    /// A Socratic question belongs to the conversation that delivered it.
    /// Existing stage-wide invitations retain their original scope.
    pub fn pending_response_invitation_in_chat(&self, binding: &TeachingBinding) -> Result<Option<TeachingEvent>, ToolError> {
        self.find_response_invitation(&binding.tutor_session_id, &binding.source_id, &binding.source_revision, Some(&binding.chat_session_id))
    }

    fn find_response_invitation(&self, session: &str, source: &str, revision: &str, chat: Option<&str>) -> Result<Option<TeachingEvent>, ToolError> {
        let mut query = self.connection.prepare("SELECT event FROM teaching_trace WHERE learning_workspace_id=? AND session_id=? AND json_extract(event,'$.kind')='message_delivered' AND json_extract(event,'$.binding.source_id')=? AND json_extract(event,'$.binding.source_revision')=? ORDER BY seq DESC").map_err(storage)?;
        let rows = query.query_map(params![self.workspace_scope, session,source,revision], |r|r.get::<_,String>(0)).map_err(storage)?;
        let mut closed = std::collections::BTreeSet::new();
        for row in rows {
            let event: TeachingEvent = serde_json::from_str(&row.map_err(storage)?).map_err(storage)?;
            if let Some(id) = event.payload["move"]["responds_to"]["invitation_ref"].as_str() {
                if matches!(event.payload["move"]["responds_to"]["disposition"].as_str(), Some("addressed" | "deferred")) { closed.insert(id.to_owned()); }
            }
            let expectation = &event.payload["move"]["response_expectation"];
            if expectation.is_object() && !closed.contains(&event.event_id)
                && (expectation["socratic"] != true || chat.is_none_or(|id| id == event.binding.chat_session_id)) {
                return Ok(Some(event));
            }
        }
        Ok(None)
    }

    pub fn latest_teaching_focus(&self, session: &str, source: &str) -> Result<Option<TeachingEvent>, ToolError> {
        let event: Option<String> = self.connection.query_row(
            "SELECT event FROM teaching_trace WHERE learning_workspace_id=? AND session_id=? AND json_extract(event,'$.kind')='message_delivered' AND json_extract(event,'$.binding.source_id')=? AND json_extract(event,'$.payload.assistance_for') IS NULL ORDER BY seq DESC LIMIT 1",
            params![self.workspace_scope, session,source], |r|r.get(0)).optional().map_err(storage)?;
        event.map(|event|serde_json::from_str(&event).map_err(storage)).transpose()
    }

    /// An unfinished question belongs to its chat, independent of the legacy
    /// account-wide teaching control or the currently displayed learning goal.
    pub fn latest_chat_teaching_activity(&self, chat: &str, source: &str) -> Result<Option<TeachingEvent>, ToolError> {
        let event: Option<String> = self.connection.query_row(
            "SELECT event FROM teaching_trace WHERE learning_workspace_id=? AND json_extract(event,'$.kind')='message_delivered' AND json_extract(event,'$.binding.chat_session_id')=? AND json_extract(event,'$.binding.source_id')=? AND json_extract(event,'$.payload.assessment_contract_ref') IS NOT NULL AND json_extract(event,'$.payload.assistance_for') IS NULL ORDER BY seq DESC LIMIT 1",
            params![self.workspace_scope,chat,source], |r|r.get(0)).optional().map_err(storage)?;
        event.map(|event|serde_json::from_str(&event).map_err(storage)).transpose()
    }

    /// Evidence for deciding whether to offer support or invite an attempt.
    /// Restrict both sides to this design lineage and the current relationships;
    /// selected or abandoned moves never count as something the reader saw.
    pub fn teaching_learning_opportunity(&self, session: &str, source: &str, revision: &str, design_ref: Option<&str>, blocks: &[String]) -> Result<(Vec<TeachingEvent>, Vec<String>), ToolError> {
        let blocks=serde_json::to_string(blocks).map_err(storage)?;
        let lineage="WITH RECURSIVE lineage(id) AS (
            SELECT ?1 WHERE ?1 IS NOT NULL UNION ALL
            SELECT json_extract(t.event,'$.payload.design.previous_ref') FROM teaching_trace t JOIN lineage l ON t.event_id=l.id WHERE t.learning_workspace_id=?6 AND json_extract(t.event,'$.payload.design.previous_ref') IS NOT NULL)";
        let mut delivered=self.connection.prepare(&format!("{lineage}
            SELECT t.event FROM teaching_trace t WHERE t.learning_workspace_id=?6 AND t.session_id=?2
            AND json_extract(t.event,'$.kind')='message_delivered'
            AND json_extract(t.event,'$.binding.source_id')=?3 AND json_extract(t.event,'$.binding.source_revision')=?4
            AND json_extract(t.event,'$.payload.move.design_ref') IN (SELECT id FROM lineage)
            AND EXISTS (SELECT 1 FROM json_each(t.event,'$.payload.move.block_refs') b JOIN json_each(?5) f ON b.value=f.value)
            ORDER BY t.seq DESC LIMIT 3")).map_err(storage)?;
        let deliveries=delivered.query_map(params![design_ref,session,source,revision,blocks,self.workspace_scope],|r|r.get::<_,String>(0)).map_err(storage)?
            .map(|r|serde_json::from_str(&r.map_err(storage)?).map_err(storage)).collect::<Result<Vec<_>,_>>()?;
        let mut interpreted=self.connection.prepare(&format!("{lineage}
            SELECT e.evidence_id FROM learning_evidence e WHERE e.learning_workspace_id=?6 AND json_extract(e.evidence,'$.session_id')=?2
            AND e.source_id=?3 AND json_extract(e.evidence,'$.source_revision')=?4
            AND json_extract(e.evidence,'$.design_ref') IN (SELECT id FROM lineage)
            AND EXISTS (SELECT 1 FROM json_each(e.evidence,'$.block_refs') b JOIN json_each(?5) f ON b.value=f.value)
            AND NOT EXISTS (SELECT 1 FROM learning_evidence n WHERE n.learning_workspace_id=e.learning_workspace_id AND json_extract(n.evidence,'$.supersedes')=e.evidence_id)
            ORDER BY e.seq DESC LIMIT 12")).map_err(storage)?;
        let interpretations=interpreted.query_map(params![design_ref,session,source,revision,blocks,self.workspace_scope],|r|r.get::<_,String>(0)).map_err(storage)?
            .map(|r|r.map_err(storage)).collect::<Result<Vec<_>,_>>()?;
        Ok((deliveries,interpretations))
    }

    /// A focus revision does not erase the support already provided within this
    /// design. Keep its original relationship identity when presenting history.
    pub fn teaching_lineage_support(&self, session: &str, source: &str, revision: &str, design_ref: Option<&str>) -> Result<Vec<TeachingEvent>, ToolError> {
        let mut query=self.connection.prepare("WITH RECURSIVE lineage(id) AS (
            SELECT ?1 WHERE ?1 IS NOT NULL UNION ALL
            SELECT json_extract(t.event,'$.payload.design.previous_ref') FROM teaching_trace t JOIN lineage l ON t.event_id=l.id WHERE t.learning_workspace_id=?5 AND json_extract(t.event,'$.payload.design.previous_ref') IS NOT NULL)
            SELECT event FROM teaching_trace WHERE learning_workspace_id=?5 AND session_id=?2 AND json_extract(event,'$.kind')='message_delivered'
            AND json_extract(event,'$.binding.source_id')=?3 AND json_extract(event,'$.binding.source_revision')=?4
            AND json_extract(event,'$.payload.move.design_ref') IN (SELECT id FROM lineage)
            ORDER BY seq DESC LIMIT 6").map_err(storage)?;
        let rows=query.query_map(params![design_ref,session,source,revision,self.workspace_scope],|row|row.get::<_,String>(0)).map_err(storage)?;
        rows.map(|row|serde_json::from_str(&row.map_err(storage)?).map_err(storage)).collect()
    }

    /// Freeze the last actual answer in this chat before accepting the next
    /// input. Selections and answers from another chat are not received support.
    pub fn latest_chat_teaching_support(&self, session: &str, source: &str, revision: &str, chat: &str) -> Result<Vec<TeachingEvent>, ToolError> {
        let turn:Option<String>=self.connection.query_row("SELECT json_extract(event,'$.binding.turn_id') FROM teaching_trace
            WHERE learning_workspace_id=?5 AND session_id=?1 AND json_extract(event,'$.kind')='message_delivered'
            AND json_extract(event,'$.binding.source_id')=?2 AND json_extract(event,'$.binding.source_revision')=?3
            AND json_extract(event,'$.binding.chat_session_id')=?4 ORDER BY seq DESC LIMIT 1",
            params![session,source,revision,chat,self.workspace_scope],|row|row.get(0)).optional().map_err(storage)?;
        let Some(turn)=turn else { return Ok(vec![]); };
        Ok(self.teaching_deliveries(chat,&turn)?.into_iter().filter(|event|
            event.binding.tutor_session_id==session && event.binding.source_id==source && event.binding.source_revision==revision).collect())
    }

    /// Expressions retain the support frozen at input acceptance, even after
    /// later help or a different design focus. This does not classify attempts.
    pub fn teaching_supported_expressions(&self, delivery: &str) -> Result<Vec<TeachingEvent>, ToolError> {
        let mut query=self.connection.prepare("SELECT event FROM teaching_trace t
            WHERE t.learning_workspace_id=?2 AND EXISTS(SELECT 1 FROM json_each(t.event,'$.payload.support_delivery_refs') s WHERE s.value=?1)
            AND json_extract(event,'$.kind') IN ('usage_observed','learner_action')
            ORDER BY seq DESC LIMIT 4").map_err(storage)?;
        let rows=query.query_map(params![delivery,self.workspace_scope],|row|row.get::<_,String>(0)).map_err(storage)?;
        let events=rows.map(|row|serde_json::from_str::<TeachingEvent>(&row.map_err(storage)?).map_err(storage)).collect::<Result<Vec<_>,_>>()?;
        Ok(events.into_iter().filter(|event|event.learner_expression().is_some()).collect())
    }

    pub fn teaching_expression_interpretations(&self, fact: &str) -> Result<Vec<crate::learning_evidence::LearningEvidence>, ToolError> {
        let mut query=self.connection.prepare("SELECT evidence FROM learning_evidence e
            WHERE e.learning_workspace_id=?2 AND (json_extract(e.evidence,'$.action_ref')=?1
                OR EXISTS(SELECT 1 FROM json_each(e.evidence,'$.fact_refs') f WHERE f.value=?1))
            AND NOT EXISTS(SELECT 1 FROM learning_evidence n WHERE n.learning_workspace_id=e.learning_workspace_id AND json_extract(n.evidence,'$.supersedes')=e.evidence_id)
            ORDER BY seq DESC LIMIT 4").map_err(storage)?;
        let rows=query.query_map(params![fact,self.workspace_scope],|row|row.get::<_,String>(0)).map_err(storage)?;
        rows.map(|row|serde_json::from_str(&row.map_err(storage)?).map_err(storage)).collect()
    }

    /// Find work before applying the preview limit. Unrelated recent deliveries
    /// must not push an earlier response to the current relationship out of view.
    pub fn teaching_continuity_facts(
        &self, binding: &TeachingBinding, design_ref: Option<&str>, blocks: &[String],
        fact_refs: &[String],
    ) -> Result<Vec<TeachingEvent>, ToolError> {
        let mut query = self.connection.prepare("WITH RECURSIVE lineage(id) AS (
            SELECT ?1 WHERE ?1 IS NOT NULL UNION ALL
            SELECT json_extract(t.event,'$.payload.design.previous_ref') FROM teaching_trace t
            JOIN lineage l ON t.event_id=l.id WHERE t.learning_workspace_id=?6
                AND json_extract(t.event,'$.payload.design.previous_ref') IS NOT NULL),
            support(id) AS (
                SELECT event_id FROM teaching_trace WHERE learning_workspace_id=?6 AND session_id=?2
                AND json_extract(event,'$.kind')='message_delivered'
                AND json_extract(event,'$.payload.move.design_ref') IN (SELECT id FROM lineage)
                AND EXISTS(SELECT 1 FROM json_each(event,'$.payload.move.block_refs') b
                    JOIN json_each(?5) f ON b.value=f.value))
            SELECT event FROM teaching_trace t WHERE learning_workspace_id=?6 AND session_id=?2
                AND json_extract(event,'$.binding.source_id')=?3
                AND json_extract(event,'$.binding.source_revision')=?4
                AND (json_extract(event,'$.kind')='learner_action'
                    AND json_extract(event,'$.payload.action') IN ('submit','revise','chat_response')
                    OR json_extract(event,'$.kind')='usage_observed'
                    AND json_extract(event,'$.payload.usage.kind') IN ('user_message','reading_action'))
                AND (?1 IS NULL OR json_extract(event,'$.binding.turn_id')=?8
                    OR event_id IN (SELECT value FROM json_each(?7))
                    OR json_extract(event,'$.payload.delivery_ref') IN (SELECT id FROM support)
                    OR json_extract(event,'$.payload.invitation_context_ref') IN (SELECT id FROM support)
                    OR EXISTS(SELECT 1 FROM json_each(event,'$.payload.support_delivery_refs') s JOIN support ON s.value=support.id)
                    OR EXISTS(SELECT 1 FROM learning_evidence e WHERE e.learning_workspace_id=?6
                        AND json_extract(e.evidence,'$.design_ref') IN (SELECT id FROM lineage)
                        AND EXISTS(SELECT 1 FROM json_each(e.evidence,'$.block_refs') b JOIN json_each(?5) f ON b.value=f.value)
                        AND NOT EXISTS(SELECT 1 FROM learning_evidence n WHERE n.learning_workspace_id=?6 AND json_extract(n.evidence,'$.supersedes')=e.evidence_id)
                        AND (json_extract(e.evidence,'$.action_ref')=t.event_id
                            OR EXISTS(SELECT 1 FROM json_each(e.evidence,'$.fact_refs') f WHERE f.value=t.event_id))))
            ORDER BY seq DESC LIMIT 13").map_err(storage)?;
        let rows = query.query_map(params![design_ref, binding.tutor_session_id, binding.source_id,
            binding.source_revision, serde_json::to_string(blocks).map_err(storage)?,
            self.workspace_scope, serde_json::to_string(fact_refs).map_err(storage)?, binding.turn_id],
            |r| r.get::<_, String>(0)).map_err(storage)?;
        rows.map(|row| serde_json::from_str(&row.map_err(storage)?).map_err(storage)).collect()
    }
    pub fn teaching_activity_events(
        &self,
        delivery: &str,
    ) -> Result<Vec<TeachingEvent>, ToolError> {
        // Activity responses/help carry delivery_ref; delivered conversational
        // support uses the existing ordinary-invitation or activity-help link.
        let mut query = self.connection.prepare("SELECT event FROM teaching_trace WHERE learning_workspace_id=?2 AND (json_extract(event,'$.payload.delivery_ref')=?1
            OR (json_extract(event,'$.kind')='message_delivered' AND
                (json_extract(event,'$.payload.move.responds_to.invitation_ref')=?1 OR json_extract(event,'$.payload.assistance_for')=?1)))
            ORDER BY seq").map_err(storage)?;
        let rows = query
            .query_map(params![delivery,self.workspace_scope], |r| r.get::<_, String>(0))
            .map_err(storage)?;
        rows.map(|row| serde_json::from_str(&row.map_err(storage)?).map_err(storage))
            .collect()
    }
    pub fn teaching_recent(
        &self,
        session: &str,
        limit: u32,
    ) -> Result<Vec<TeachingEvent>, ToolError> {
        let mut query = self.connection.prepare("SELECT event FROM teaching_trace WHERE learning_workspace_id=? AND session_id=? AND json_extract(event,'$.kind') IN ('message_delivered','learner_action','help_displayed','usage_observed','continuation_displayed','continuation_chosen') ORDER BY seq DESC LIMIT ?").map_err(storage)?;
        let rows = query
            .query_map(params![self.workspace_scope, session, limit.min(20)], |r| r.get::<_, String>(0))
            .map_err(storage)?;
        rows.map(|row| serde_json::from_str(&row.map_err(storage)?).map_err(storage))
            .collect()
    }
    pub fn teaching_deliveries(
        &self,
        chat: &str,
        turn: &str,
    ) -> Result<Vec<TeachingEvent>, ToolError> {
        let mut query = self.connection.prepare("SELECT event FROM teaching_trace WHERE learning_workspace_id=? AND json_extract(event,'$.binding.chat_session_id')=? AND json_extract(event,'$.binding.turn_id')=? AND json_extract(event,'$.kind')='message_delivered' ORDER BY seq").map_err(storage)?;
        let rows = query
            .query_map(params![self.workspace_scope, chat, turn], |r| r.get::<_, String>(0))
            .map_err(storage)?;
        rows.map(|row| serde_json::from_str(&row.map_err(storage)?).map_err(storage))
            .collect()
    }
    /// Delivered practice problems still without a final soundness verdict, oldest
    /// first. Failed attempts are their own facts; `max_failures` bounds retries.
    pub fn unchecked_practice_deliveries(&self, max_failures: u32, limit: u32) -> Result<Vec<TeachingEvent>, ToolError> {
        let mut query = self.connection.prepare("SELECT d.event FROM teaching_trace d WHERE d.learning_workspace_id=?1
            AND json_extract(d.event,'$.kind')='message_delivered' AND json_type(d.event,'$.payload.move.practice')='object'
            AND NOT EXISTS (SELECT 1 FROM teaching_trace c WHERE c.learning_workspace_id=?1 AND c.event_id='practice-check:'||d.event_id)
            AND (SELECT count(*) FROM teaching_trace f WHERE f.learning_workspace_id=?1 AND json_extract(f.event,'$.kind')='practice_checked'
                AND json_extract(f.event,'$.payload.delivery_ref')=d.event_id AND json_extract(f.event,'$.payload.verdict')='failed')<?2
            ORDER BY d.seq LIMIT ?3").map_err(storage)?;
        let rows = query
            .query_map(params![self.workspace_scope, max_failures, limit], |r| r.get::<_, String>(0))
            .map_err(storage)?;
        rows.map(|row| serde_json::from_str(&row.map_err(storage)?).map_err(storage))
            .collect()
    }

    /// Delivered practice problems on one source, newest first.
    pub fn practice_deliveries(&self, source: &str, limit: u32) -> Result<Vec<TeachingEvent>, ToolError> {
        let mut query = self.connection.prepare("SELECT event FROM teaching_trace WHERE learning_workspace_id=? AND json_extract(event,'$.kind')='message_delivered'
            AND json_type(event,'$.payload.move.practice')='object' AND json_extract(event,'$.binding.source_id')=? ORDER BY seq DESC LIMIT ?").map_err(storage)?;
        let rows = query
            .query_map(params![self.workspace_scope, source, limit], |r| r.get::<_, String>(0))
            .map_err(storage)?;
        rows.map(|row| serde_json::from_str(&row.map_err(storage)?).map_err(storage))
            .collect()
    }

    /// Failed check attempts already recorded for a delivered practice problem.
    pub fn practice_check_failures(&self, delivery: &str) -> Result<u32, ToolError> {
        self.connection.query_row("SELECT count(*) FROM teaching_trace WHERE learning_workspace_id=? AND json_extract(event,'$.kind')='practice_checked' AND json_extract(event,'$.payload.delivery_ref')=? AND json_extract(event,'$.payload.verdict')='failed'",
            params![self.workspace_scope, delivery], |r| r.get(0)).map_err(storage)
    }

    /// The final verdict on a delivered practice problem: `sound` or `questionable`.
    pub fn practice_check_verdict(&self, delivery: &str) -> Result<Option<String>, ToolError> {
        self.connection.query_row("SELECT json_extract(event,'$.payload.verdict') FROM teaching_trace WHERE learning_workspace_id=? AND event_id='practice-check:'||?",
            params![self.workspace_scope, delivery], |r| r.get(0)).optional().map_err(storage)
    }

    pub fn teaching_event(&self, id: &str) -> Result<TeachingEvent, ToolError> {
        let json: Option<String> = self
            .connection
            .query_row(
                "SELECT event FROM teaching_trace WHERE learning_workspace_id=? AND event_id=?",
                params![self.workspace_scope,id],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        serde_json::from_str(&json.ok_or_else(|| invalid("教学引用不存在"))?).map_err(storage)
    }

    pub fn teaching_event_sequence(&self, id: &str) -> Result<Option<u64>, ToolError> {
        self.connection.query_row("SELECT seq FROM teaching_trace WHERE learning_workspace_id=? AND event_id=?",
            params![self.workspace_scope,id], |row|row.get(0)).optional().map_err(storage)
    }

    /// Oldest-first page with a stable cursor; callers never need to load all private history.
    pub fn teaching_events(
        &self,
        session: &str,
        after: u64,
        limit: u32,
    ) -> Result<Vec<(u64, TeachingEvent)>, ToolError> {
        let mut query = self.connection.prepare(
            "SELECT seq,event FROM teaching_trace WHERE learning_workspace_id=? AND session_id=? AND seq>? ORDER BY seq LIMIT ?"
        ).map_err(storage)?;
        let rows = query
            .query_map(params![self.workspace_scope, session, after, limit.min(100)], |r| {
                Ok((r.get::<_, u64>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(storage)?;
        rows.map(|row| {
            let (seq, json) = row.map_err(storage)?;
            Ok((seq, serde_json::from_str(&json).map_err(storage)?))
        })
        .collect()
    }

    pub fn teaching_turn_events(
        &self,
        session: &str,
        turn: &str,
    ) -> Result<Vec<TeachingEvent>, ToolError> {
        let mut query = self.connection.prepare("SELECT event FROM teaching_trace WHERE learning_workspace_id=? AND session_id=? AND json_extract(event,'$.binding.turn_id')=? ORDER BY seq").map_err(storage)?;
        let rows = query
            .query_map(params![self.workspace_scope, session, turn], |r| r.get::<_, String>(0))
            .map_err(storage)?;
        rows.map(|row| serde_json::from_str(&row.map_err(storage)?).map_err(storage))
            .collect()
    }

    pub fn append_teaching(&mut self, event: &TeachingEvent) -> Result<TeachingEvent, ToolError> {
        let tx = self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
        let result = append_in(&tx, &self.workspace_scope, event)?;
        tx.commit().map_err(storage)?;
        Ok(result)
    }
}

// Shared by course progression so facts and the course receipt commit together.
pub(crate) fn append_in(db: &rusqlite::Connection, scope: &str, event: &TeachingEvent) -> Result<TeachingEvent, ToolError> {
        if event.event_id.trim().is_empty() {
            return Err(invalid("缺少教学事件身份"));
        }
        if event.binding.map_revision.as_ref().is_some_and(|s| s.trim().is_empty()) {
            return Err(invalid("缺少地图应省略引用，不能保存空地图版本"));
        }
        if event.kind == TeachingFact::DesignRevised {
            let design: crate::teaching_design::TeachingDesign = serde_json::from_value(event.payload["design"].clone()).map_err(|e|invalid(e.to_string()))?;
            design.validate(&event.binding)?;
        }
        if let Some(target) = event.payload.get("move").and_then(|m| m.get("target")).filter(|t| !t.is_null()) {
            let target: TeachingTarget = serde_json::from_value(target.clone()).map_err(|e| invalid(e.to_string()))?;
            target.validate(&event.binding)?;
            if event.payload["move"]["capability"] != target.capability {
                return Err(invalid("教学动作与目标能力不一致"));
            }
        }
        let previous: Option<String> = db
            .query_row(
                "SELECT event FROM teaching_trace WHERE learning_workspace_id=? AND event_id=?",
                params![scope,event.event_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        if let Some(json) = previous {
            let old: TeachingEvent = serde_json::from_str(&json).map_err(storage)?;
            event.validate_unchanged_retry(&old)?;
            return Ok(old);
        }
        // Causal edges always stay inside the same private teaching session.
        for id in &event.causal_refs {
            let owner: Option<String> = db
                .query_row(
                    "SELECT session_id FROM teaching_trace WHERE learning_workspace_id=? AND event_id=?",
                    params![scope,id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(storage)?;
            if owner.as_deref() != Some(&event.binding.tutor_session_id) {
                return Err(invalid("教学因果引用不属于此会话"));
            }
        }
        db
            .execute(
                "INSERT INTO teaching_trace(learning_workspace_id,event_id,session_id,event) VALUES (?,?,?,?)",
                params![
                    scope,
                    event.event_id,
                    event.binding.tutor_session_id,
                    serde_json::to_string(event).map_err(storage)?
                ],
            )
            .map_err(storage)?;
        Ok(event.clone())

}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn om2_continuity_queries_keep_workspace_source_and_stage_after_reopen() {
        let root=tempfile::tempdir().unwrap();
        let path=root.path().join("learning.db");
        let binding=TeachingBinding {tutor_session_id:"stage".into(),session_revision:1,control_revision:1,
            source_id:"book".into(),source_revision:"v1".into(),map_revision:None,chat_session_id:"chat".into(),turn_id:"old".into()};
        let fact=TeachingEvent {event_id:"work".into(),binding:binding.clone(),kind:TeachingFact::UsageObserved,
            causal_refs:vec![],payload:serde_json::json!({"usage":{"kind":"user_message"},"response":"I can explain this example."}),occurred_at:"then".into()};
        for scope in ["selected","other"] {
            let mut store=LearningStore::open(&path,false).unwrap().with_workspace(Some(scope));
            store.append_teaching(&fact).unwrap();
            for (id,stage,revision) in [("other-stage","old-stage","v1"),("other-version","stage","v2")] {
                let mut other=fact.clone(); other.event_id=id.into();
                other.binding.tutor_session_id=stage.into(); other.binding.source_revision=revision.into();
                store.append_teaching(&other).unwrap();
            }
        }
        let store=LearningStore::open(&path,false).unwrap().with_workspace(Some("selected"));
        assert_eq!(store.teaching_continuity_facts(&binding,None,&[],&[]).unwrap(),[fact]);
        let empty=LearningStore::open(&path,false).unwrap().with_workspace(Some("unselected"));
        assert!(empty.teaching_continuity_facts(&binding,None,&[],&[]).unwrap().is_empty());
    }
    #[test]
    fn media_target_preserves_exact_visual_audio_identity_without_text_positions() {
        let binding=TeachingBinding {tutor_session_id:"s".into(),session_revision:0,control_revision:0,source_id:"video".into(),source_revision:"original".into(),map_revision:None,chat_session_id:"chat".into(),turn_id:"turn".into()};
        let visual=serde_json::json!({"kind":"visual","publication":{"book_id":"video","publication_id":"v1"},"frames":[{"video":{"media_id":"clip","media_revision":"1"},"actual_pts_ms":1000.0,"width":1280,"height":720}],"regions":[]});
        let audio=serde_json::json!({"kind":"audio","publication":{"book_id":"video","publication_id":"v1"},"location":{"video":{"media_id":"clip","media_revision":"1"},"start_ms":0.0,"end_ms":2000.0},"review_result_ref":"original-review"});
        for media in [visual,audio] {
            let mut target:TeachingTarget=serde_json::from_value(serde_json::json!({"learning_focus":"画面或原声","expected_performance":"解释原资料","capability":"explanation","source_bindings":[],"media_bindings":[{"source_id":"video","source_revision":"original","target":media}]})).unwrap();
            target.validate(&binding).unwrap();assert!(target.source_bindings.is_empty());
            assert_eq!(serde_json::to_value(&target.media_bindings.as_ref().unwrap()[0].target).unwrap(),media);
            target.media_bindings.as_mut().unwrap()[0].source_revision="other".into();assert!(target.validate(&binding).is_err());
            target.media_bindings=None;assert!(target.validate(&binding).is_err());
        }
    }
    #[test]
    fn music_target_retains_prepared_source_without_claiming_audio_observation() {
        let binding=TeachingBinding {tutor_session_id:"s".into(),session_revision:0,control_revision:0,source_id:"music".into(),source_revision:"book-revision".into(),map_revision:None,chat_session_id:"chat".into(),turn_id:"turn".into()};
        let raw=serde_json::json!({"learning_focus":"旋律与伴奏","expected_performance":"指出两种角色的区别","capability":"listening_comparison","source_bindings":[],
            "music_bindings":[{"source_id":"music","source_revision":"book-revision","target":{"publication":{"book_id":"music","publication_id":"pub1"},
                "audio":{"recording":{"recording_id":"recording","revision":"r1"},"start_ms":1000,"end_ms":2000},
                "score":{"score":{"score_id":"score","revision":"s1"},"source_ids":["note1"]},"occurrence_ids":["repeat2"],"event_ids":["theme"],"provenance_ids":["notation"]}}],"object_refs":[]});
        let mut target:TeachingTarget=serde_json::from_value(raw).unwrap();target.validate(&binding).unwrap();
        assert!(target.media_bindings.is_none());assert!(target.source_bindings.is_empty());
        let stored=serde_json::to_string(&target).unwrap();assert_eq!(serde_json::from_str::<TeachingTarget>(&stored).unwrap(),target);
        target.music_bindings.as_mut().unwrap()[0].target.publication.book_id="other".into();assert!(target.validate(&binding).is_err());
        target.music_bindings=None;assert!(target.validate(&binding).is_err());
    }
    #[test]
    fn continuation_events_roundtrip_without_becoming_performance() {
        use ts_rs::TS;
        assert!(TeachingObjectRevision::decl().contains("object_revision: number"));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("learning.db");
        let mut store = LearningStore::open(&path, false).unwrap();
        let old = serde_json::json!({"event_id":"old","binding":{"tutor_session_id":"session",
            "session_revision":1,"control_revision":1,"source_id":"book","source_revision":"rev",
            "chat_session_id":"chat","turn_id":"old-turn"},"kind":"message_delivered","causal_refs":[],
            "payload":{"answer":"original"},"occurred_at":"now"});
        let original: TeachingEvent = serde_json::from_value(old.clone()).unwrap();
        store.append_teaching(&original).unwrap();
        for (id, kind, payload) in [
            ("displayed",TeachingFact::ContinuationDisplayed,serde_json::to_value(ContinuationDisplayed {
                delivery_ref:"old".into(),option_ids:vec!["a".into()],recommended_option_id:"a".into() }).unwrap()),
            ("chosen",TeachingFact::ContinuationChosen,serde_json::to_value(ContinuationChosen {
                delivery_ref:"old".into(),option_id:"a".into(),accepted_turn_id:"new-turn".into() }).unwrap()),
            ("reading-action",TeachingFact::UsageObserved,serde_json::json!({"usage":{"kind":"reading_action"},"response":""}))] {
            let mut binding=original.binding.clone();
            if kind == TeachingFact::ContinuationChosen { binding.turn_id="new-turn".into(); }
            let event = TeachingEvent { event_id:id.into(),kind,binding,causal_refs:vec!["old".into()],payload,occurred_at:"now".into() };
            store.append_teaching(&event).unwrap();
            assert_eq!(store.append_teaching(&event).unwrap(),event);
        }
        drop(store);
        let mut store=LearningStore::open(&path,false).unwrap();
        assert_eq!(serde_json::to_value(store.teaching_event("old").unwrap()).unwrap(),old);
        assert_eq!(store.teaching_event("displayed").unwrap().kind,TeachingFact::ContinuationDisplayed);
        assert_eq!(store.teaching_event("chosen").unwrap().kind,TeachingFact::ContinuationChosen);
        store.derive_assessed_evidence("session").unwrap();
        let count:i64=store.connection.query_row("SELECT COUNT(*) FROM learning_evidence",[],|r|r.get(0)).unwrap();
        assert_eq!(count,0);
        for fact in ["displayed", "chosen", "reading-action"] {
            let evidence: crate::learning_evidence::LearningEvidence = serde_json::from_value(serde_json::json!({
                "evidence_id":format!("hypothesis:{fact}"),"action_ref":fact,"session_id":"session",
                "source_id":"book","source_revision":"rev","nature":"hypothesis","fact_refs":[fact],
                "teaching_implication":"use an example","label":"speed","capability":"explanation",
                "assistance_refs":[],"attempt":0,"status":"unassessed","interpretation":"needs examples",
                "learner_quote":"","interpreter_version":"test","source_quotes":[],
                "target":{"learning_focus":"speed","expected_performance":"explain time","capability":"explanation",
                    "source_bindings":[{"source_id":"book","source_revision":"rev","start_lid":"1.1","end_lid":"1.1"}],"object_refs":[]}
            })).unwrap();
            let error = store.append_evidence(&evidence).unwrap_err();
            assert_eq!(error.error_code,"TEACHING_INVALID");
            assert!(error.message.contains("真实用户表达"));
            assert!(error.message.contains("无需创建理解假设"));
        }
        let raw:String=store.connection.query_row("SELECT event FROM teaching_trace WHERE event_id='old'",[],|r|r.get(0)).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&raw).unwrap(),old);
    }
    #[test]
    fn teaching_facts_are_immutable_retryable_and_survive_projection_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("learning.db");
        let mut store = LearningStore::open(&path, false).unwrap();
        let mut event = TeachingEvent {
            event_id: "delivery".into(),
            binding: TeachingBinding {
                tutor_session_id: "tutor".into(),
                session_revision: 1,
                control_revision: 1,
                source_id: "book".into(),
                source_revision: "old".into(),
                map_revision: Some("v1".into()),
                chat_session_id: "chat1".into(),
                turn_id: "turn1".into(),
            },
            kind: TeachingFact::MessageDelivered,
            causal_refs: vec![],
            payload: serde_json::json!({"text":"original"}),
            occurred_at: "now".into(),
        };
        let first = store.append_teaching(&event).unwrap();
        event.occurred_at = "later".into();
        assert_eq!(store.append_teaching(&event).unwrap(), first);
        event.payload = serde_json::json!({"text":"changed"});
        assert!(store.append_teaching(&event).is_err());
        event.event_id = "next".into();
        event.causal_refs = vec!["delivery".into()];
        store.connection.execute_batch("CREATE TRIGGER teaching_fail BEFORE INSERT ON teaching_trace BEGIN SELECT RAISE(ABORT,'write failed'); END;").unwrap();
        assert!(store.append_teaching(&event).is_err());
        assert!(store.teaching_event("next").is_err());
        store
            .connection
            .execute_batch("DROP TRIGGER teaching_fail")
            .unwrap();
        store.rebuild().unwrap();
        drop(store);
        let store = LearningStore::open(&path, false).unwrap();
        assert_eq!(store.teaching_event("delivery").unwrap(), first);
        assert_eq!(store.teaching_events("tutor", 0, 100).unwrap().len(), 1);
    }
}

#[cfg(test)]
mod t15_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn teaching_t15_targets_and_optional_maps_survive_reopen_and_preserve_legacy_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("learning.db");
        let mut store = LearningStore::open(&path, false).unwrap();
        let old = json!({"event_id":"old","binding":{"tutor_session_id":"session","session_revision":1,
            "control_revision":1,"source_id":"book","source_revision":"source-v1","map_revision":"map-v1",
            "chat_session_id":"chat","turn_id":"turn"},"kind":"message_delivered","causal_refs":[],
            "payload":{"move":{"object_ids":["speed"],"capability":"explanation"},"object_versions":[{"ref":{"source_id":"book","object_id":"speed"},"object_revision":3}]},"occurred_at":"now"});
        // Insert the pre-T15 JSON itself, so reopening tests real historical bytes.
        store.connection.execute("INSERT INTO teaching_trace(event_id,session_id,event) VALUES ('old','session',?)", [old.to_string()]).unwrap();
        let legacy = store.teaching_event("old").unwrap();
        assert_eq!(serde_json::to_value(&legacy).unwrap(), old);
        let target = json!({"learning_focus":"理解平均速度","expected_performance":"解释总路程与总时间的关系",
            "capability":"explanation","source_bindings":[{"source_id":"book","source_revision":"source-v1","start_lid":"1.1","end_lid":"1.2"}],"object_refs":[]});
        for (id, map) in [("without-map", None), ("with-map", Some("map-v2".to_string()))] {
            let mut event = legacy.clone();
            event.event_id = id.into();
            event.binding.map_revision = map;
            event.payload = json!({"move":{"capability":"explanation","target":target}});
            if id == "with-map" {
                event.payload["move"]["target"]["object_refs"] = json!([{"source_id":"book","object_id":"speed","object_revision":4}]);
            }
            store.append_teaching(&event).unwrap();
        }
        drop(store);
        let mut store = LearningStore::open(&path, false).unwrap();
        let no_map = store.teaching_event("without-map").unwrap();
        assert_eq!(no_map.binding.map_revision, None);
        assert!(serde_json::to_value(&no_map.binding).unwrap().get("map_revision").is_none());
        assert_eq!(no_map.payload["move"]["target"], target);
        let mapped = store.teaching_event("with-map").unwrap();
        assert_eq!(mapped.binding.map_revision.as_deref(), Some("map-v2"));
        assert_eq!(mapped.payload["move"]["target"]["object_refs"][0]["object_revision"], 4);
        assert_eq!(serde_json::to_value(store.teaching_event("old").unwrap()).unwrap(), old);
        let raw: String = store.connection.query_row("SELECT event FROM teaching_trace WHERE event_id='old'", [], |r| r.get(0)).unwrap();
        assert_eq!(raw, old.to_string());
        let mut invalid_event = no_map.clone();
        invalid_event.event_id = "invalid".into();
        invalid_event.binding.map_revision = Some("".into());
        assert!(store.append_teaching(&invalid_event).is_err());
        invalid_event.binding.map_revision = None;
        invalid_event.payload["move"]["target"]["source_bindings"] = json!([]);
        assert!(store.append_teaching(&invalid_event).is_err());
        invalid_event.payload["move"]["target"] = target;
        invalid_event.payload["move"]["target"]["source_bindings"][0]["source_revision"] = json!("old-source");
        assert!(store.append_teaching(&invalid_event).is_err());
        invalid_event.payload["move"]["target"] = no_map.payload["move"]["target"].clone();
        invalid_event.payload["move"]["capability"] = json!("prediction");
        assert!(store.append_teaching(&invalid_event).is_err());
    }
}
