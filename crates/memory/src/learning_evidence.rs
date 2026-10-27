//! Append-only interpretations and deterministic, disposable learner projections.
use crate::{
    assessment::AssessmentStatus,
    learning::{storage, LearningStore},
    teaching::{invalid, TeachingBinding, TeachingEvent, TeachingFact},
};
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const ESTIMATOR_VERSION: &str = "learner.conditions.v3";

// Interpretations may be written by a later background job. Their position in
// learning history comes from the last actual expression they interpret, never
// from the time at which that job happened to finish.
const ORIGIN_JOIN_TEMPLATE: &str = "LEFT JOIN teaching_trace origin ON origin.learning_workspace_id=e.learning_workspace_id AND origin.seq=(
    SELECT t.seq FROM teaching_trace t WHERE t.learning_workspace_id=e.learning_workspace_id
    AND json_extract(t.event,'$.binding.source_id')=e.source_id
    AND json_extract(t.event,'$.binding.source_revision')=json_extract(e.evidence,'$.source_revision')
    AND t.session_id=json_extract(e.evidence,'$.session_id')
    AND (t.event_id=json_extract(e.evidence,'$.action_ref') OR t.event_id IN (SELECT value FROM json_each(e.evidence,'$.fact_refs')))
    AND (json_extract(t.event,'$.kind')='learner_action' OR (json_extract(t.event,'$.kind')='usage_observed' AND json_extract(t.event,'$.payload.usage.kind') IN ('user_message','reading_action')))
    AND coalesce(json_extract(t.event,'$.payload.response_origin.kind'),'')!='interface_request'
    AND NOT (coalesce(json_extract(t.event,'$.payload.response_origin.kind'),'')='presentation_prefill' AND json_extract(t.event,'$.payload.response_origin.prefill_text')=json_extract(t.event,'$.payload.response'))
    AND length(trim(coalesce(CASE WHEN json_extract(t.event,'$.payload.usage.kind')='reading_action' AND json_type(t.event,'$.payload.interaction')='object' THEN json_extract(t.event,'$.payload.interaction.user_supplement') ELSE json_extract(t.event,'$.payload.response') END,'')))>0
    ORDER BY EXPRESSION_TIME DESC,t.seq DESC LIMIT 1)";

const ORIGIN_ORDER: &str = "CASE
    WHEN trim(coalesce(json_extract(origin.event,'$.occurred_at'),''))!='' AND json_extract(origin.event,'$.occurred_at') NOT GLOB '*[^0-9.]*'
    THEN CAST(json_extract(origin.event,'$.occurred_at') AS REAL) / CASE WHEN CAST(json_extract(origin.event,'$.occurred_at') AS REAL)>100000000000 THEN 1000 ELSE 1 END
    WHEN json_extract(origin.event,'$.occurred_at') GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]*'
    THEN (julianday(json_extract(origin.event,'$.occurred_at'))-2440587.5)*86400 END";

fn origin_join() -> String {
    ORIGIN_JOIN_TEMPLATE.replace("EXPRESSION_TIME", &ORIGIN_ORDER.replace("origin.event","t.event"))
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceNature {
    Hypothesis,
    #[default]
    Performance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearningEvidence {
    pub evidence_id: String,
    pub action_ref: String,
    pub delivery_ref: Option<String>,
    pub session_id: String,
    pub source_id: String,
    pub source_revision: String,
    pub map_revision: Option<String>,
    pub object_id: Option<String>,
    pub object_revision: Option<u64>,
    #[serde(default)]
    pub nature: EvidenceNature,
    #[serde(default)]
    pub target: Option<crate::teaching::TeachingTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub block_refs: Vec<String>,
    #[serde(default)]
    pub fact_refs: Vec<String>,
    #[serde(default)]
    pub teaching_implication: String,
    pub label: String,
    pub capability: String,
    pub assistance_refs: Vec<String>,
    /// Explanations already delivered before the expression, separate from
    /// activity help (hints/reveals) and never the answer to this expression.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub support_delivery_refs: Vec<String>,
    pub attempt: u64,
    pub assessment_ref: Option<String>,
    pub status: AssessmentStatus,
    pub interpretation: String,
    pub learner_quote: String,
    pub interpreter_version: String,
    pub supersedes: Option<String>,
    pub correction: Option<String>,
    pub source_quotes: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeRow {
    pub object_id: String,
    pub object_revision: u64,
    pub source_revision: String,
    pub map_revision: Option<String>,
    pub label: String,
    pub capability: String,
    pub independent_support: u32,
    pub assisted_support: u32,
    pub revised_support: u32,
    pub partial: u32,
    pub difficulty: u32,
    pub uncertain: u32,
    pub state: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearnerProjection {
    pub estimator_version: String,
    pub evidence_watermark: u64,
    pub teaching_map_revision_refs: Vec<String>,
    pub rows: Vec<KnowledgeRow>,
    pub paths: BTreeMap<String, Value>,
}

impl LearningStore {
    pub fn fact_has_interpretation(&self, id: &str) -> Result<bool, ToolError> {
        self.connection.query_row("SELECT EXISTS(SELECT 1 FROM learning_evidence e, json_each(e.evidence,'$.fact_refs') f WHERE e.learning_workspace_id=? AND f.value=?)", params![self.workspace_scope,id], |r|r.get(0)).map_err(storage)
    }
    pub fn evidence_is_superseded(&self, id: &str) -> Result<bool, ToolError> {
        self.connection.query_row("SELECT EXISTS(SELECT 1 FROM learning_evidence WHERE learning_workspace_id=? AND json_extract(evidence,'$.supersedes')=?)", params![self.workspace_scope,id], |r|r.get(0)).map_err(storage)
    }
    pub fn evidence(&self, id: &str) -> Result<LearningEvidence, ToolError> {
        let row: Option<String> = self
            .connection
            .query_row(
                "SELECT evidence FROM learning_evidence WHERE learning_workspace_id=? AND evidence_id=?",
                params![self.workspace_scope,id],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        serde_json::from_str(&row.ok_or_else(|| invalid("学习证据不存在"))?).map_err(storage)
    }

    /// Resolve a current interpretation to the learner's original expressions.
    /// Understanding may carry across sessions for the same source version, but
    /// its original facts must still belong to the interpretation's own session.
    pub fn understanding_basis(
        &self,
        binding: &TeachingBinding,
        evidence_id: &str,
    ) -> Result<(LearningEvidence, Vec<TeachingEvent>), ToolError> {
        let evidence = self.evidence(evidence_id)?;
        if evidence.source_id != binding.source_id
            || evidence.source_revision != binding.source_revision
            || self.evidence_is_superseded(evidence_id)?
        {
            return Err(invalid("理解记录不属于当前来源版本或已被更新"));
        }
        let mut origin = binding.clone();
        origin.tutor_session_id = evidence.session_id.clone();
        let facts = if evidence.nature == EvidenceNature::Performance {
            // Existing performance records also list delivered support in fact_refs.
            if evidence.correction.as_ref().is_some_and(|text| !text.trim().is_empty()) {
                let action = self.teaching_event(&evidence.action_ref)?;
                if action.binding.source_id != origin.source_id
                    || action.binding.source_revision != origin.source_revision
                    || action.binding.tutor_session_id != origin.tutor_session_id
                {
                    return Err(invalid("理解依据不属于原记录的来源、版本或会话"));
                }
                // The later user correction is an expression in its own right.
                // Do not turn the original interface response into learner words.
                action.learner_expression().is_some().then_some(action).into_iter().collect()
            } else {
                vec![self.learner_expression_fact(&origin, &evidence.action_ref)?]
            }
        } else {
            let mut expressions = Vec::new();
            for id in &evidence.fact_refs {
                let fact = self.teaching_event(id)?;
                // Historic hypotheses can mix support with expressions. Keep only
                // actual expressions in the basis supplied to design and review.
                if fact.learner_expression().is_some() {
                    expressions.push(self.learner_expression_fact(&origin, id)?);
                }
            }
            if expressions.is_empty() {
                return Err(invalid("理解记录没有真实用户表达依据；保持理解未知并继续讲清当前问题"));
            }
            expressions
        };
        Ok((evidence, facts))
    }

    pub fn append_evidence(
        &mut self,
        evidence: &LearningEvidence,
    ) -> Result<LearningEvidence, ToolError> {
        let existing: Option<String> = self
            .connection
            .query_row(
                "SELECT evidence FROM learning_evidence WHERE learning_workspace_id=? AND evidence_id=?",
                params![self.workspace_scope,evidence.evidence_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        if let Some(old) = existing {
            let old: LearningEvidence = serde_json::from_str(&old).map_err(storage)?;
            if old != *evidence {
                return Err(invalid("同一证据身份不能改写解释"));
            }
            return Ok(old);
        }
        let action = self.teaching_event(&evidence.action_ref)?;
        if let Some(id) = &evidence.design_ref {
            let event = self.teaching_event(id)?;
            if event.kind != TeachingFact::DesignRevised || event.binding.tutor_session_id != evidence.session_id
                || event.binding.source_id != evidence.source_id || event.binding.source_revision != evidence.source_revision {
                return Err(invalid("理解关系不属于当前设计来源和会话"));
            }
            let design: crate::teaching_design::TeachingDesign = serde_json::from_value(event.payload["design"].clone()).map_err(storage)?;
            design.validate_blocks(&evidence.block_refs)?;
        } else if !evidence.block_refs.is_empty() { return Err(invalid("关系理解需要设计版本")); }
        if evidence.nature == EvidenceNature::Hypothesis {
            if evidence.fact_refs.is_empty() || !evidence.fact_refs.contains(&evidence.action_ref)
                || evidence.interpretation.trim().is_empty() || evidence.teaching_implication.trim().is_empty()
                || evidence.assessment_ref.is_some() || evidence.status != AssessmentStatus::Unassessed
                || evidence.delivery_ref.is_some() || evidence.object_id.is_some() || evidence.object_revision.is_some()
            { return Err(invalid("理解假设需要事实与教学影响，不附加评分或对象能力")); }
            let mut origin = action.binding.clone();
            origin.tutor_session_id = evidence.session_id.clone();
            origin.source_id = evidence.source_id.clone();
            origin.source_revision = evidence.source_revision.clone();
            let mut quote_is_original = false;
            for id in &evidence.fact_refs {
                let fact = self.learner_expression_fact(&origin, id)?;
                quote_is_original |= !evidence.learner_quote.trim().is_empty()
                    && fact.learner_expression().is_some_and(|text| text.contains(&evidence.learner_quote));
            }
            if !quote_is_original {
                return Err(invalid("理解记录的用户引文必须来自所引用的真实用户表达"));
            }
            let target = evidence.target.as_ref().ok_or_else(||invalid("理解假设缺少适用目标"))?;
            target.validate(&action.binding)?;
        } else {
        let delivery = self.teaching_event(evidence.delivery_ref.as_deref().ok_or_else(||invalid("表现证据缺少交付"))?)?;
        if action.kind != TeachingFact::LearnerAction
            || action.payload["delivery_ref"] != delivery.event_id
            || delivery.binding.source_id != evidence.source_id
            || delivery.binding.source_revision != evidence.source_revision
            || delivery.binding.map_revision != evidence.map_revision
            || delivery.binding.tutor_session_id != evidence.session_id
            || delivery.payload["move"]["capability"] != evidence.capability
            || evidence.object_id.is_some() != evidence.object_revision.is_some()
            || (evidence.object_id.is_some() && !delivery.payload["object_versions"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|o| {
                    o["ref"]["object_id"].as_str() == evidence.object_id.as_deref()
                        && o["object_revision"].as_u64() == evidence.object_revision
                }))
            || (evidence.object_id.is_none() && (evidence.target.is_none()
                || serde_json::to_value(&evidence.target).map_err(storage)? != delivery.payload["move"]["target"]))
        {
            return Err(invalid("证据与实际行为或原对象版本不符"));
        }
        }
        if let Some(id) = &evidence.supersedes {
            let previous = self.evidence(id)?;
            if previous.nature != evidence.nature || previous.source_id != evidence.source_id
                || previous.source_revision != evidence.source_revision
                || (evidence.nature == EvidenceNature::Performance && (previous.session_id != evidence.session_id || previous.action_ref != evidence.action_ref
                    || previous.object_id != evidence.object_id))
            {
                return Err(invalid("替代解释必须属于同一次对象表现"));
            }
            let replaced:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM learning_evidence WHERE learning_workspace_id=? AND json_extract(evidence,'$.supersedes')=?)",params![self.workspace_scope,id],|r|r.get(0)).map_err(storage)?;
            if replaced {
                return Err(invalid("解释已经更新，请刷新后纠正"));
            }
        }
        self.connection
            .execute(
                "INSERT INTO learning_evidence(learning_workspace_id,evidence_id,source_id,evidence) VALUES (?,?,?,?)",
                params![
                    self.workspace_scope,
                    evidence.evidence_id,
                    evidence.source_id,
                    serde_json::to_string(evidence).map_err(storage)?
                ],
            )
            .map_err(storage)?;
        Ok(evidence.clone())
    }

    /// Safe after a crash between assessment and evidence: stable identity skips accepted work.
    pub fn derive_assessed_evidence(&mut self, session: &str) -> Result<(), ToolError> {
        let mut after = 0;
        loop {
            let page = self.teaching_events(session, after, 100)?;
            if page.is_empty() {
                break;
            }
            for (seq, action) in page {
                after = seq;
                if action.kind != TeachingFact::LearnerAction
                    || !matches!(
                        action.payload["action"].as_str(),
                        Some("submit" | "revise" | "self_report")
                    )
                {
                    continue;
                }
                let assessment = self.assessment(&action.event_id)?;
                let personal = action.payload["action"] == "self_report";
                if assessment.is_none() && !personal {
                    continue;
                }
                let status = assessment
                    .as_ref()
                    .map(|a| a.status.clone())
                    .unwrap_or(AssessmentStatus::Uncertain);
                let delivery = self.teaching_event(
                    action.payload["delivery_ref"]
                        .as_str()
                        .ok_or_else(|| invalid("缺少交付引用"))?,
                )?;
                let mut objects = delivery.payload["object_versions"].as_array().cloned().unwrap_or_default();
                if objects.is_empty() && !delivery.payload["move"]["target"].is_null() { objects.push(Value::Null); }
                for object in objects {
                    let id = object["ref"]["object_id"]
                        .as_str();
                    let evidence_id = format!("assessment:{}:{}", action.event_id, id.unwrap_or("source"));
                    // Historic accepted evidence keeps its original serialized contract.
                    // Derivation resumes missing work, it does not rewrite prior interpretations.
                    match self.evidence(&evidence_id) {
                        Ok(_) => continue,
                        Err(e) if e.error_code == "TEACHING_INVALID" => {},
                        Err(e) => return Err(e),
                    }
                    let evidence = LearningEvidence {
                        evidence_id,
                        action_ref: action.event_id.clone(),
                        delivery_ref: Some(delivery.event_id.clone()),
                        session_id: session.into(),
                        source_id: delivery.binding.source_id.clone(),
                        source_revision: delivery.binding.source_revision.clone(),
                        map_revision: delivery.binding.map_revision.clone(),
                        object_id: id.map(str::to_owned),
                        object_revision: object["object_revision"].as_u64(),
                        nature: EvidenceNature::Performance,
                        target: serde_json::from_value(delivery.payload["move"].get("target").cloned().unwrap_or(Value::Null)).map_err(storage)?,
                        design_ref: delivery.payload["move"]["design_ref"].as_str().map(str::to_owned),
                        block_refs: serde_json::from_value(delivery.payload["move"].get("block_refs").cloned().unwrap_or(json!([]))).map_err(storage)?,
                        fact_refs: vec![delivery.event_id.clone(), action.event_id.clone()],
                        teaching_implication: "结合本次表现及帮助条件调整下一步；不足时保持未知".into(),
                        label: object["meaning"].as_str().or_else(||delivery.payload["move"]["target"]["learning_focus"].as_str()).unwrap_or("学习表现").into(),
                        capability: delivery.payload["move"]["capability"]
                            .as_str()
                            .unwrap_or_default()
                            .into(),
                        assistance_refs: serde_json::from_value(
                            action.payload["assistance_refs"].clone(),
                        )
                        .unwrap_or_default(),
                        support_delivery_refs: serde_json::from_value(action.payload.get("support_delivery_refs").cloned().unwrap_or(json!([]))).unwrap_or_default(),
                        attempt: action.payload["attempt"].as_u64().unwrap_or(1),
                        assessment_ref: assessment.as_ref().map(|_| action.event_id.clone()),
                        status: status.clone(),
                        interpretation: if personal {
                            "用户自述，未作客观能力判断"
                        } else {
                            match status {
                                AssessmentStatus::Correct => "本次回答满足活动标准",
                                AssessmentStatus::Partial => "本次回答满足部分标准",
                                AssessmentStatus::Incorrect => "本次回答未满足活动标准",
                                _ => "目前依据不足以判断",
                            }
                        }
                        .into(),
                        learner_quote: action.payload["response"]
                            .as_str()
                            .unwrap_or_default()
                            .into(),
                        interpreter_version: if personal {
                            "personal.statement.v1"
                        } else {
                            "assessment.conditions.v1"
                        }
                        .into(),
                        supersedes: None,
                        correction: None,
                        source_quotes: json!(assessment.as_ref().map(|a| &a.items)),
                    };
                    self.append_evidence(&evidence)?;
                }
            }
        }
        Ok(())
    }

    pub fn correct_evidence(
        &mut self,
        id: &str,
        operation: &str,
        text: &str,
    ) -> Result<LearningEvidence, ToolError> {
        if operation.trim().is_empty() || text.trim().is_empty() {
            return Err(invalid("请填写纠正说明"));
        }
        let mut next = self.evidence(id)?;
        if next.nature == EvidenceNature::Hypothesis {
            // A user can correct an older interpretation whose fact_refs also
            // contained help. The new record keeps only its actual expressions.
            let facts = next.fact_refs.iter().map(|id| self.teaching_event(id))
                .collect::<Result<Vec<_>, _>>()?;
            let expressions = facts.iter().filter_map(|fact| fact.learner_expression().map(|text| (fact, text)))
                .collect::<Vec<_>>();
            let (first, quote) = expressions.first().ok_or_else(|| invalid("理解记录没有真实用户表达依据；保持理解未知并继续讲清当前问题"))?;
            next.fact_refs = expressions.iter().map(|(fact, _)| fact.event_id.clone()).collect();
            if !next.fact_refs.contains(&next.action_ref) { next.action_ref = first.event_id.clone(); }
            if next.learner_quote.trim().is_empty() || !expressions.iter().any(|(_, text)| text.contains(&next.learner_quote)) {
                next.learner_quote = (*quote).into();
            }
        }
        next.evidence_id = format!("correction:{operation}");
        next.supersedes = Some(id.into());
        next.correction = Some(text.into());
        next.status = if next.nature == EvidenceNature::Hypothesis { AssessmentStatus::Unassessed } else { AssessmentStatus::Uncertain };
        next.interpretation = "原解释已被用户纠正，等待新的表现依据".into();
        next.teaching_implication = format!("采用用户纠正重新确定讲解起点：{text}");
        next.interpreter_version = "learner.correction.v1".into();
        self.append_evidence(&next)
    }

    /// Current source-bound interpretations, including source-only performances.
    /// Replacement is resolved before the page limit, so superseded claims never reappear.
    pub fn current_interpretations(&self, source: &str, revision: &str, after: u64, limit: u32) -> Result<Vec<(u64, LearningEvidence)>, ToolError> {
        let origin_join=origin_join();
        let mut q = self.connection.prepare(&format!("SELECT e.seq,e.evidence FROM learning_evidence e {origin_join} WHERE e.learning_workspace_id=? AND e.source_id=? AND json_extract(e.evidence,'$.source_revision')=? AND NOT EXISTS (SELECT 1 FROM learning_evidence n WHERE n.learning_workspace_id=e.learning_workspace_id AND json_extract(n.evidence,'$.supersedes')=e.evidence_id) ORDER BY {ORIGIN_ORDER} DESC,origin.seq DESC,e.seq DESC LIMIT ? OFFSET ?")).map_err(storage)?;
        let rows = q.query_map(params![self.workspace_scope,source, revision, limit.min(20), after], |r|Ok((r.get::<_,u64>(0)?,r.get::<_,String>(1)?))).map_err(storage)?;
        rows.map(|r| { let (seq,text)=r.map_err(storage)?; Ok((seq,serde_json::from_str(&text).map_err(storage)?)) }).collect()
    }

    /// Stable relationship IDs carry across revisions of the same private design only.
    /// Relevant older understanding is fetched before the recency limit is applied.
    pub fn current_design_interpretations(&self, session: &str, source: &str, revision: &str, design_ref: Option<&str>, blocks: &[String], limit: u32) -> Result<Vec<(u64, LearningEvidence)>, ToolError> {
        let origin_join=origin_join();
        let mut q=self.connection.prepare(&format!("WITH RECURSIVE lineage(id) AS (
            SELECT ?1 WHERE ?1 IS NOT NULL UNION ALL
            SELECT json_extract(t.event,'$.payload.design.previous_ref') FROM teaching_trace t JOIN lineage l ON t.event_id=l.id WHERE t.learning_workspace_id=?7 AND json_extract(t.event,'$.payload.design.previous_ref') IS NOT NULL)
            SELECT e.seq,e.evidence FROM learning_evidence e {origin_join} WHERE e.learning_workspace_id=?7 AND e.source_id=?2 AND json_extract(e.evidence,'$.source_revision')=?3
            AND NOT EXISTS (SELECT 1 FROM learning_evidence n WHERE n.learning_workspace_id=e.learning_workspace_id AND json_extract(n.evidence,'$.supersedes')=e.evidence_id)
            ORDER BY CASE WHEN json_extract(e.evidence,'$.session_id')=?4
              AND json_extract(e.evidence,'$.design_ref') IN (SELECT id FROM lineage)
              AND EXISTS (SELECT 1 FROM json_each(e.evidence,'$.block_refs') b JOIN json_each(?5) f ON b.value=f.value)
              THEN 0 ELSE 1 END, {ORIGIN_ORDER} DESC,origin.seq DESC,e.seq DESC LIMIT ?6")).map_err(storage)?;
        let rows=q.query_map(params![design_ref,source,revision,session,serde_json::to_string(blocks).map_err(storage)?,limit.min(20),self.workspace_scope],|r|Ok((r.get::<_,u64>(0)?,r.get::<_,String>(1)?))).map_err(storage)?;
        rows.map(|r| {let (seq,text)=r.map_err(storage)?; Ok((seq,serde_json::from_str(&text).map_err(storage)?))}).collect()
    }

    /// Presentation metadata only: absent or invalid legacy timestamps stay unknown.
    pub fn evidence_chronology(&self, id: &str) -> Result<Value, ToolError> {
        let origin_join=origin_join();
        self.connection.query_row(&format!("SELECT origin.seq,
            CASE WHEN {ORIGIN_ORDER} IS NOT NULL THEN json_extract(origin.event,'$.occurred_at') END,
            json_extract(origin.event,'$.binding.chat_session_id'),json_extract(origin.event,'$.binding.turn_id')
            FROM learning_evidence e {origin_join} WHERE e.learning_workspace_id=? AND e.evidence_id=?"),
            params![self.workspace_scope,id], |r| Ok(json!({
                "fact_sequence":r.get::<_,Option<u64>>(0)?,"occurred_at":r.get::<_,Option<String>>(1)?,
                "source_session_id":r.get::<_,Option<String>>(2)?,"source_turn_id":r.get::<_,Option<String>>(3)?
            }))).map_err(storage)
    }

    pub fn evidence_expressions(&self, evidence: &LearningEvidence) -> Result<Vec<(u64, TeachingEvent)>, ToolError> {
        let mut refs=evidence.fact_refs.clone();
        refs.push(evidence.action_ref.clone());
        let expression_order=ORIGIN_ORDER.replace("origin.event","event");
        let mut q=self.connection.prepare(&format!("SELECT seq,event FROM teaching_trace WHERE learning_workspace_id=? AND session_id=?
            AND json_extract(event,'$.binding.source_id')=? AND json_extract(event,'$.binding.source_revision')=?
            AND event_id IN (SELECT value FROM json_each(?)) ORDER BY {expression_order},seq")).map_err(storage)?;
        let rows=q.query_map(params![self.workspace_scope,evidence.session_id,evidence.source_id,evidence.source_revision,serde_json::to_string(&refs).map_err(storage)?],
            |r| Ok((r.get::<_,u64>(0)?,r.get::<_,String>(1)?))).map_err(storage)?;
        let mut expressions=Vec::new();
        for row in rows {
            let (seq,text)=row.map_err(storage)?;
            let fact:TeachingEvent=serde_json::from_str(&text).map_err(storage)?;
            if fact.learner_expression().is_some() { expressions.push((seq,fact)); }
        }
        Ok(expressions)
    }

    /// Same relation across learning observations, including corrections as history.
    /// A matching title or source paragraph alone is not a relationship identity.
    pub fn understanding_timeline(&self, id: &str, after: u64, limit: u32) -> Result<Vec<(u64, LearningEvidence)>, ToolError> {
        let seed=self.evidence(id)?;
        let mut objects:Vec<Value>=seed.target.as_ref().into_iter().flat_map(|target| &target.object_refs)
            .map(|object|json!({"object_id":object.object_id,"object_revision":object.object_revision})).collect();
        if let (Some(id),Some(revision))=(&seed.object_id,seed.object_revision) { objects.push(json!({"object_id":id,"object_revision":revision})); }
        let origin_join=origin_join();
        let sql=format!("WITH RECURSIVE
            ancestors(id) AS (SELECT ?1 WHERE ?1 IS NOT NULL UNION
                SELECT json_extract(t.event,'$.payload.design.previous_ref') FROM teaching_trace t JOIN ancestors a ON t.event_id=a.id
                WHERE t.learning_workspace_id=?2 AND json_extract(t.event,'$.payload.design.previous_ref') IS NOT NULL),
            family(id) AS (SELECT id FROM ancestors UNION SELECT t.event_id FROM teaching_trace t JOIN family f ON json_extract(t.event,'$.payload.design.previous_ref')=f.id
                WHERE t.learning_workspace_id=?2 AND t.session_id=?3 AND json_extract(t.event,'$.binding.source_id')=?4 AND json_extract(t.event,'$.binding.source_revision')=?5),
            correction_edges(a,b) AS (SELECT evidence_id,json_extract(evidence,'$.supersedes') FROM learning_evidence
                WHERE learning_workspace_id=?2 AND source_id=?4 AND json_extract(evidence,'$.source_revision')=?5 AND json_extract(evidence,'$.supersedes') IS NOT NULL),
            corrections(id) AS (SELECT ?6 UNION SELECT CASE WHEN c.id=edge.a THEN edge.b ELSE edge.a END FROM correction_edges edge JOIN corrections c ON c.id=edge.a OR c.id=edge.b)
            SELECT e.seq,e.evidence FROM learning_evidence e {origin_join}
            WHERE e.learning_workspace_id=?2 AND e.source_id=?4 AND json_extract(e.evidence,'$.source_revision')=?5 AND (
              e.evidence_id IN (SELECT id FROM corrections)
              OR (json_extract(e.evidence,'$.session_id')=?3 AND json_extract(e.evidence,'$.design_ref') IN (SELECT id FROM family)
                  AND EXISTS (SELECT 1 FROM json_each(e.evidence,'$.block_refs') b JOIN json_each(?7) focus ON b.value=focus.value))
              OR (json_extract(e.evidence,'$.capability')=?8 AND EXISTS (SELECT 1 FROM json_each(?9) o
                  WHERE (json_extract(o.value,'$.object_id')=json_extract(e.evidence,'$.object_id') AND json_extract(o.value,'$.object_revision')=json_extract(e.evidence,'$.object_revision'))
                  OR EXISTS (SELECT 1 FROM json_each(e.evidence,'$.target.object_refs') target WHERE json_extract(o.value,'$.object_id')=json_extract(target.value,'$.object_id') AND json_extract(o.value,'$.object_revision')=json_extract(target.value,'$.object_revision')))))
            ORDER BY {ORIGIN_ORDER},origin.seq,e.seq LIMIT ?10 OFFSET ?11");
        let mut q=self.connection.prepare(&sql).map_err(storage)?;
        let rows=q.query_map(params![seed.design_ref,self.workspace_scope,seed.session_id,seed.source_id,seed.source_revision,seed.evidence_id,
            serde_json::to_string(&seed.block_refs).map_err(storage)?,seed.capability,serde_json::to_string(&objects).map_err(storage)?,limit.min(20),after],
            |r| Ok((r.get::<_,u64>(0)?,r.get::<_,String>(1)?))).map_err(storage)?;
        rows.map(|row| { let (seq,text)=row.map_err(storage)?; Ok((seq,serde_json::from_str(&text).map_err(storage)?)) }).collect()
    }

    pub fn evidence_page(
        &self,
        source: &str,
        after: u64,
        limit: u32,
    ) -> Result<Vec<(u64, LearningEvidence)>, ToolError> {
        let mut query=self.connection.prepare("SELECT seq,evidence FROM learning_evidence WHERE learning_workspace_id=? AND source_id=? AND seq>? ORDER BY seq LIMIT ?").map_err(storage)?;
        let rows = query
            .query_map(params![self.workspace_scope,source, after, limit.min(100)], |r| {
                Ok((r.get::<_, u64>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(storage)?;
        rows.map(|r| {
            let (seq, text) = r.map_err(storage)?;
            Ok((seq, serde_json::from_str(&text).map_err(storage)?))
        })
        .collect()
    }

    pub fn object_evidence_refs(
        &self,
        source: &str,
        object: &str,
        revision: u64,
        capability: &str,
        before: Option<u64>,
    ) -> Result<Vec<(u64, String)>, ToolError> {
        let mut q=self.connection.prepare("SELECT seq,evidence_id FROM learning_evidence WHERE learning_workspace_id=? AND source_id=? AND json_extract(evidence,'$.object_id')=? AND json_extract(evidence,'$.object_revision')=? AND json_extract(evidence,'$.capability')=? AND seq<? ORDER BY seq DESC LIMIT 10").map_err(storage)?;
        let rows = q
            .query_map(
                params![
                    self.workspace_scope,
                    source,
                    object,
                    revision,
                    capability,
                    before.unwrap_or(i64::MAX as u64)
                ],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(storage)?;
        rows.map(|r| r.map_err(storage)).collect()
    }

    pub fn learner_projection(
        &self,
        source: &str,
    ) -> Result<(Option<LearnerProjection>, u64), ToolError> {
        let text: Option<String> = self
            .connection
            .query_row(
                "SELECT projection FROM learner_projection WHERE learning_workspace_id=? AND source_id=?",
                params![self.workspace_scope,source],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        let watermark = self
            .connection
            .query_row(
                "SELECT coalesce(max(seq),0) FROM learning_evidence WHERE learning_workspace_id=? AND source_id=?",
                params![self.workspace_scope,source],
                |r| r.get(0),
            )
            .map_err(storage)?;
        Ok((
            text.map(|s| serde_json::from_str(&s).map_err(storage))
                .transpose()?,
            watermark,
        ))
    }

    pub fn rebuild_learner_projection(
        &mut self,
        source: &str,
    ) -> Result<LearnerProjection, ToolError> {
        // Read and replace in one transaction: interruption leaves either the old or complete view.
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let all: Vec<(u64, LearningEvidence)> = {
            let origin_join=origin_join();
            let mut q = tx
                .prepare(
                    &format!("SELECT e.seq,e.evidence FROM learning_evidence e {origin_join} WHERE e.learning_workspace_id=? AND e.source_id=? ORDER BY {ORIGIN_ORDER},origin.seq,e.seq"),
                )
                .map_err(storage)?;
            let rows = q
                .query_map(params![self.workspace_scope,source], |r| {
                    Ok((r.get::<_, u64>(0)?, r.get::<_, String>(1)?))
                })
                .map_err(storage)?;
            rows.map(|r| {
                let (s, v) = r.map_err(storage)?;
                Ok((s, serde_json::from_str(&v).map_err(storage)?))
            })
            .collect::<Result<_, ToolError>>()?
        };
        let superseded: BTreeSet<_> = all
            .iter()
            .filter_map(|(_, e)| e.supersedes.as_ref())
            .collect();
        let mut rows = BTreeMap::<(String, u64, String, String), KnowledgeRow>::new();
        let mut paths = BTreeMap::<String, Value>::new();
        for (_, e) in all
            .iter()
            .filter(|(_, e)| !superseded.contains(&e.evidence_id))
        {
            if e.nature == EvidenceNature::Hypothesis { continue; }
            let (Some(object_id), Some(object_revision)) = (&e.object_id, e.object_revision) else { continue; };
            let row = rows
                .entry((
                    object_id.clone(),
                    object_revision,
                    e.source_revision.clone(),
                    e.capability.clone(),
                ))
                .or_insert_with(|| KnowledgeRow {
                    object_id: object_id.clone(),
                    object_revision,
                    source_revision: e.source_revision.clone(),
                    map_revision: e.map_revision.clone(),
                    label: e.label.clone(),
                    capability: e.capability.clone(),
                    independent_support: 0,
                    assisted_support: 0,
                    revised_support: 0,
                    partial: 0,
                    difficulty: 0,
                    uncertain: 0,
                    state: "unknown".into(),
                    evidence_refs: vec![],
                });
            match e.status {
                AssessmentStatus::Correct if !e.assistance_refs.is_empty() || !e.support_delivery_refs.is_empty() => {
                    row.assisted_support += 1
                }
                AssessmentStatus::Correct if e.attempt > 1 => row.revised_support += 1,
                AssessmentStatus::Correct => row.independent_support += 1,
                AssessmentStatus::Partial => row.partial += 1,
                AssessmentStatus::Incorrect => row.difficulty += 1,
                _ => row.uncertain += 1,
            }
            row.state = if row.independent_support
                + row.assisted_support
                + row.revised_support
                + row.partial
                + row.difficulty
                > 0
            {
                "observed"
            } else {
                "unknown"
            }
            .into();
            row.evidence_refs.push(e.evidence_id.clone());
            let key = format!(
                "{}:{}:{}:{}",
                e.session_id, object_id, object_revision, e.capability
            );
            let action_sequence: u64 = tx
                .query_row(
                    "SELECT seq FROM teaching_trace WHERE learning_workspace_id=? AND event_id=?",
                    params![self.workspace_scope,e.action_ref],
                    |r| r.get(0),
                )
                .map_err(storage)?;
            paths.insert(key,json!({"session_id":e.session_id,"object_id":e.object_id,"object_revision":e.object_revision,"source_revision":e.source_revision,"capability":e.capability,"last_status":e.status,"assistance_refs":e.assistance_refs,"support_delivery_refs":e.support_delivery_refs,"attempt":e.attempt,"evidence_ref":e.evidence_id,"action_sequence":action_sequence}));
        }
        let projection = LearnerProjection {
            estimator_version: ESTIMATOR_VERSION.into(),
            evidence_watermark: all.iter().map(|(seq, _)| *seq).max().unwrap_or(0),
            teaching_map_revision_refs: all
                .iter()
                .filter_map(|(_, e)| e.map_revision.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            rows: rows.into_values().collect(),
            paths,
        };
        tx.execute("INSERT INTO learner_projection(learning_workspace_id,source_id,projection) VALUES (?,?,?) ON CONFLICT(learning_workspace_id,source_id) DO UPDATE SET projection=excluded.projection",params![self.workspace_scope,source,serde_json::to_string(&projection).map_err(storage)?]).map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(projection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assessment::ResponseAssessment,
        teaching::{TeachingBinding, TeachingEvent},
    };
    fn observation(id: &str, kind: TeachingFact, payload: Value) -> TeachingEvent {
        TeachingEvent {
            event_id: id.into(),
            binding: TeachingBinding {
                tutor_session_id: "session".into(), session_revision: 1, control_revision: 0,
                source_id: "book".into(), source_revision: "source-v1".into(), map_revision: None,
                chat_session_id: "chat".into(), turn_id: "turn".into(),
            },
            kind, causal_refs: vec![], payload, occurred_at: "now".into(),
        }
    }

    fn hypothesis(fact: &TeachingEvent) -> LearningEvidence {
        serde_json::from_value(json!({
            "evidence_id":format!("hypothesis:{}",fact.event_id),"action_ref":fact.event_id,
            "session_id":fact.binding.tutor_session_id,"source_id":fact.binding.source_id,
            "source_revision":fact.binding.source_revision,"nature":"hypothesis","fact_refs":[fact.event_id],
            "teaching_implication":"explain the requested relation","label":"embedding","capability":"explanation",
            "assistance_refs":[],"attempt":0,"status":"unassessed","interpretation":"asks how embedding is used",
            "learner_quote":fact.learner_expression().unwrap_or_default(),"interpreter_version":"test","source_quotes":[],
            "target":{"learning_focus":"embedding","expected_performance":"explain the input",
                "capability":"explanation","source_bindings":[{"source_id":fact.binding.source_id,
                    "source_revision":fact.binding.source_revision,"start_lid":"1.1","end_lid":"1.2"}],"object_refs":[]}
        })).unwrap()
    }

    #[test]
    fn understanding_timeline_orders_original_expressions_before_late_background_writes() {
        let directory=tempfile::tempdir().unwrap();
        let mut store=LearningStore::open(&directory.path().join("learning.db"),false).unwrap();
        let mut earlier=observation("earlier",TeachingFact::UsageObserved,json!({"usage":{"kind":"user_message"},"response":"prefill 和 decode 怎么区分？"}));
        earlier.occurred_at="2026-10-24T03:00:00Z".into();
        earlier.binding.turn_id="first-turn".into();
        let mut later=observation("later",TeachingFact::UsageObserved,json!({"usage":{"kind":"user_message"},"response":"prefill 处理输入，decode 逐个生成，对吗？"}));
        later.occurred_at="2026-10-24T11:01:00+08:00".into();
        later.binding.turn_id="second-turn".into();
        store.append_teaching(&earlier).unwrap();
        store.append_teaching(&later).unwrap();
        let later=hypothesis(&later);
        let earlier=hypothesis(&earlier);
        store.append_evidence(&later).unwrap();
        store.append_evidence(&earlier).unwrap();
        let current=store.current_interpretations("book","source-v1",0,20).unwrap();
        assert_eq!(current.iter().map(|(_,e)|e.evidence_id.as_str()).collect::<Vec<_>>(),vec![later.evidence_id.as_str(),earlier.evidence_id.as_str()]);
        assert_eq!(store.current_interpretations("book","source-v1",1,1).unwrap()[0].1.evidence_id,earlier.evidence_id);
        let chronology=store.evidence_chronology(&later.evidence_id).unwrap();
        assert_eq!(chronology["occurred_at"],"2026-10-24T11:01:00+08:00");
        assert_eq!(chronology["source_turn_id"],"second-turn");
        assert_eq!(chronology["fact_sequence"],2);
        assert_eq!(store.current_design_interpretations("session","book","source-v1",None,&[],20).unwrap()[0].1.evidence_id,later.evidence_id);
        // Equal labels and paragraphs alone never manufacture a learning trajectory.
        assert_eq!(store.understanding_timeline(&later.evidence_id,0,20).unwrap().len(),1);
    }

    fn relation_design(store: &mut LearningStore, id: &str, previous: Option<&str>, fact: &TeachingEvent) {
        let mut event=fact.clone();
        event.event_id=id.into();
        event.kind=TeachingFact::DesignRevised;
        let target=hypothesis(fact).target.unwrap();
        event.payload=json!({"design":{"target":target,"organizing_model":"input then generation",
            "blocks":[{"block_id":"phase","relation":"prefill versus decode","purpose":"distinguish stages","source_bindings":target.source_bindings}],
            "focus_block_refs":["phase"],"representation":{"kind":"text","reason":"compare two stages"},"revision_reason":"clarify phases","previous_ref":previous}});
        store.append_teaching(&event).unwrap();
    }

    #[test]
    fn understanding_timeline_keeps_hypothesis_evolution_and_corrections_in_design_lineage() {
        let directory=tempfile::tempdir().unwrap();
        let mut store=LearningStore::open(&directory.path().join("learning.db"),false).unwrap();
        let mut earlier=observation("before",TeachingFact::UsageObserved,json!({"usage":{"kind":"user_message"},"response":"两个阶段是同时做吗？"}));
        earlier.occurred_at="2026-10-24T03:00:00Z".into();
        let mut later=observation("after",TeachingFact::UsageObserved,json!({"usage":{"kind":"user_message"},"response":"输入先处理完，再持续生成。"}));
        later.occurred_at="2026-10-24T03:05:00Z".into();
        store.append_teaching(&earlier).unwrap();store.append_teaching(&later).unwrap();
        relation_design(&mut store,"design:one",None,&earlier);
        relation_design(&mut store,"design:two",Some("design:one"),&later);
        relation_design(&mut store,"design:unrelated",None,&later);
        let mut first=hypothesis(&earlier);first.design_ref=Some("design:one".into());first.block_refs=vec!["phase".into()];
        let mut second=hypothesis(&later);second.design_ref=Some("design:two".into());second.block_refs=vec!["phase".into()];
        store.append_evidence(&second).unwrap();store.append_evidence(&first).unwrap();
        let corrected=store.correct_evidence(&first.evidence_id,"fix-earlier","我是在问流程顺序，不是在断言同时发生").unwrap();
        let mut unrelated=second.clone();unrelated.evidence_id="other-design".into();unrelated.design_ref=Some("design:unrelated".into());
        store.append_evidence(&unrelated).unwrap();
        let rows=store.understanding_timeline(&second.evidence_id,0,20).unwrap();
        assert_eq!(rows.iter().map(|(_,e)|e.evidence_id.as_str()).collect::<Vec<_>>(),vec![first.evidence_id.as_str(),corrected.evidence_id.as_str(),second.evidence_id.as_str()]);
        assert!(store.evidence_is_superseded(&first.evidence_id).unwrap());
        assert!(!store.evidence_is_superseded(&second.evidence_id).unwrap());
        assert_eq!(store.evidence(&first.evidence_id).unwrap(),first,"later learning and correction both retain the original interpretation");
        assert_eq!(store.understanding_timeline(&first.evidence_id,2,1).unwrap()[0].1.evidence_id,second.evidence_id);
        let current=store.current_interpretations("book","source-v1",0,20).unwrap();
        assert!(!current.iter().any(|(_,e)|e.evidence_id==first.evidence_id));
    }

    #[test]
    fn understanding_timeline_formal_relations_do_not_cross_sources_versions_or_workspaces() {
        let directory=tempfile::tempdir().unwrap();
        let path=directory.path().join("learning.db");
        let mut store=LearningStore::open(&path,false).unwrap().with_workspace(Some("main"));
        let mut seeds=Vec::new();
        for (id,source,revision,object_revision) in [("a","book","source-v1",1),("b","book","source-v1",1),("other-source","other","source-v1",1),("new-source-version","book","source-v2",1),("new-object-version","book","source-v1",2)] {
            let mut fact=observation(id,TeachingFact::UsageObserved,json!({"usage":{"kind":"user_message"},"response":"我能区分两个阶段"}));
            fact.binding.source_id=source.into();fact.binding.source_revision=revision.into();
            store.append_teaching(&fact).unwrap();
            let mut evidence=hypothesis(&fact);
            evidence.target.as_mut().unwrap().object_refs=vec![crate::teaching::TeachingObjectRevision {source_id:source.into(),object_id:"phase".into(),object_revision}];
            store.append_evidence(&evidence).unwrap();seeds.push(evidence);
        }
        let mut other=LearningStore::open(&path,false).unwrap().with_workspace(Some("other"));
        let fact=store.teaching_event("a").unwrap();other.append_teaching(&fact).unwrap();other.append_evidence(&seeds[0]).unwrap();
        assert_eq!(store.understanding_timeline(&seeds[0].evidence_id,0,20).unwrap().iter().map(|(_,e)|e.evidence_id.as_str()).collect::<Vec<_>>(),vec!["hypothesis:a","hypothesis:b"]);
        assert_eq!(other.understanding_timeline(&seeds[0].evidence_id,0,20).unwrap().len(),1);
        assert!(store.evidence_chronology(&seeds[0].evidence_id).unwrap()["occurred_at"].is_null(),"legacy unknown time is not replaced with background completion time");
    }

    #[test]
    fn understanding_timeline_combined_observation_uses_latest_actual_expression_not_help() {
        let directory=tempfile::tempdir().unwrap();
        let mut store=LearningStore::open(&directory.path().join("learning.db"),false).unwrap();
        let mut before=observation("before",TeachingFact::UsageObserved,json!({"usage":{"kind":"user_message"},"response":"不懂"}));
        before.occurred_at="2026-10-24T03:00:00Z".into();
        let mut after=before.clone();after.event_id="after".into();after.occurred_at="2026-10-24T03:01:00Z".into();after.payload["response"]=json!("我现在能说出区别");
        let help=observation("help",TeachingFact::MessageDelivered,json!({"answer":"解释"}));
        for event in [&before,&after,&help] { store.append_teaching(event).unwrap(); }
        let mut evidence=hypothesis(&before);evidence.fact_refs.push(after.event_id.clone());
        store.append_evidence(&evidence).unwrap();
        assert_eq!(store.evidence_chronology(&evidence.evidence_id).unwrap()["occurred_at"],after.occurred_at);
        assert_eq!(store.evidence_expressions(&evidence).unwrap().iter().map(|(_,f)|f.event_id.as_str()).collect::<Vec<_>>(),vec!["before","after"]);
        let projection=store.rebuild_learner_projection("book").unwrap();
        assert_eq!(projection.evidence_watermark,store.learner_projection("book").unwrap().1);
    }

    #[test]
    fn understanding_timeline_compares_real_epoch_seconds_milliseconds_and_rfc3339() {
        let directory=tempfile::tempdir().unwrap();
        let mut store=LearningStore::open(&directory.path().join("learning.db"),false).unwrap();
        for (id,at) in [("latest","1720000001000"),("oldest","2024-07-03T09:46:39Z"),("middle","1720000000")] {
            let mut fact=observation(id,TeachingFact::UsageObserved,json!({"usage":{"kind":"user_message"},"response":"我在复述关系"}));
            fact.occurred_at=at.into();store.append_teaching(&fact).unwrap();store.append_evidence(&hypothesis(&fact)).unwrap();
            assert_eq!(store.evidence_chronology(&format!("hypothesis:{id}")).unwrap()["occurred_at"],at);
        }
        assert_eq!(store.current_interpretations("book","source-v1",0,20).unwrap().iter().map(|(_,e)|e.evidence_id.as_str()).collect::<Vec<_>>(),vec!["hypothesis:latest","hypothesis:middle","hypothesis:oldest"]);
    }

    #[test]
    fn hypothesis_requires_every_basis_to_be_an_actual_expression() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = LearningStore::open(&directory.path().join("learning.db"), false).unwrap();
        let question = observation("question", TeachingFact::UsageObserved,
            json!({"usage":{"kind":"user_message"},"response":"进入 embedding 的是中间产物，对吗？"}));
        store.append_teaching(&question).unwrap();
        for (id, kind, payload) in [
            ("delivered", TeachingFact::MessageDelivered, json!({"answer":"向量只负责排入口"})),
            ("displayed", TeachingFact::Displayed, json!({"response":"向量只负责排入口"})),
            ("help", TeachingFact::HelpDisplayed, json!({"response":"向量只负责排入口"})),
            ("click", TeachingFact::UsageObserved, json!({"usage":{"kind":"reading_action"},
                "interaction":{"user_supplement":""},"response":"解释这段"})),
            ("interface", TeachingFact::LearnerAction, json!({"action":"chat_response","response":"继续",
                "response_origin":{"kind":"interface_request"}})),
            ("prefill", TeachingFact::UsageObserved, json!({"usage":{"kind":"user_message"},
                "response":"向量只负责排入口","response_origin":{"kind":"presentation_prefill","prefill_text":"向量只负责排入口"}})),
        ] {
            store.append_teaching(&observation(id, kind, payload)).unwrap();
            let mut evidence = hypothesis(&question);
            evidence.fact_refs.push(id.into());
            let error = store.append_evidence(&evidence).unwrap_err();
            assert!(error.message.contains("真实用户表达"), "{id}: {}", error.message);
            assert!(store.learner_expression_fact(&question.binding, id).is_err());
        }
        store.append_evidence(&hypothesis(&question)).unwrap();
        assert_eq!(store.understanding_basis(&question.binding, "hypothesis:question").unwrap().1, vec![question]);
    }

    #[test]
    fn hypothesis_quotes_remain_original_for_questions_self_reports_and_answers() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = LearningStore::open(&directory.path().join("learning.db"), false).unwrap();
        for (id, kind, payload) in [
            ("question", TeachingFact::UsageObserved, json!({"usage":{"kind":"user_message"},"response":"是根据 lid 树切分的吗？"})),
            ("self-report", TeachingFact::LearnerAction, json!({"action":"self_report","response":"我还不明白输入怎样变成向量"})),
            ("answer", TeachingFact::LearnerAction, json!({"action":"submit","response":"输入是预构建的中间产物"})),
            ("supplement", TeachingFact::UsageObserved, json!({"usage":{"kind":"reading_action"},
                "interaction":{"user_supplement":"这里的输入是什么？"},"response":"解释这段"})),
            ("edited-prefill", TeachingFact::UsageObserved, json!({"usage":{"kind":"user_message"},
                "response":"我不理解向量如何比较","response_origin":{"kind":"presentation_prefill","prefill_text":"继续"}})),
        ] {
            let fact = observation(id, kind, payload);
            store.append_teaching(&fact).unwrap();
            let mut evidence = hypothesis(&fact);
            evidence.learner_quote = "用户已经理解向量只排入口".into();
            assert!(store.append_evidence(&evidence).unwrap_err().message.contains("用户引文"));
            store.append_evidence(&hypothesis(&fact)).unwrap();
            assert_eq!(store.learner_expression_fact(&fact.binding, id).unwrap(), fact);
        }
        let question = store.teaching_event("question").unwrap();
        let mut evidence = hypothesis(&question);
        evidence.evidence_id = "multiple-expressions".into();
        evidence.fact_refs.push("answer".into());
        evidence.learner_quote = "预构建的中间产物".into();
        store.append_evidence(&evidence).unwrap();
        assert_eq!(store.understanding_basis(&question.binding, &evidence.evidence_id).unwrap().1.len(), 2);
    }

    #[test]
    fn understanding_basis_uses_original_session_and_legacy_performance_action() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = LearningStore::open(&directory.path().join("learning.db"), false).unwrap();
        performance(&mut store, "answer", AssessmentStatus::Correct);
        let action = store.teaching_event("answer").unwrap();
        let mut current = action.binding.clone();
        current.tutor_session_id = "later-session".into();
        current.chat_session_id = "later-chat".into();
        let id = "assessment:answer:gradient";
        let (evidence, facts) = store.understanding_basis(&current, id).unwrap();
        assert!(evidence.fact_refs.contains(&"delivery".into()));
        assert_eq!(facts, vec![action.clone()]);
        assert!(store.learner_expression_fact(&current, "answer").is_err());
        current.source_revision = "source-v2".into();
        assert!(store.understanding_basis(&current, id).is_err());
        let correction = store.correct_evidence(id, "correction", "我仍然不理解方向").unwrap();
        assert!(store.understanding_basis(&action.binding, id).is_err());
        assert_eq!(store.understanding_basis(&action.binding, &correction.evidence_id).unwrap().1, vec![action]);
    }

    #[test]
    fn understanding_basis_filters_legacy_help_but_requires_a_real_expression() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = LearningStore::open(&directory.path().join("learning.db"), false).unwrap();
        let fact = observation("question", TeachingFact::UsageObserved,
            json!({"usage":{"kind":"user_message"},"response":"向量怎样参与检索？"}));
        store.append_teaching(&fact).unwrap();
        store.append_teaching(&observation("help", TeachingFact::HelpDisplayed, json!({"text":"解释"}))).unwrap();
        let mut legacy = hypothesis(&fact);
        legacy.fact_refs.push("help".into());
        for (id, refs) in [("mixed", legacy.fact_refs.clone()), ("only-help", vec!["help".into()])] {
            legacy.evidence_id = id.into();
            legacy.fact_refs = refs;
            store.connection.execute("INSERT INTO learning_evidence(learning_workspace_id,evidence_id,source_id,evidence) VALUES (?,?,?,?)",
                params![store.workspace_scope,legacy.evidence_id,legacy.source_id,serde_json::to_string(&legacy).unwrap()]).unwrap();
        }
        assert_eq!(store.understanding_basis(&fact.binding, "mixed").unwrap().1, vec![fact.clone()]);
        assert!(store.understanding_basis(&fact.binding, "only-help").is_err());
        let correction = store.correct_evidence("mixed", "correct-mixed", "我只是在确认输入").unwrap();
        assert_eq!(correction.fact_refs, vec![fact.event_id]);
        assert_eq!(store.correct_evidence("mixed", "correct-mixed", "我只是在确认输入").unwrap(), correction);
    }

    #[test]
    fn understanding_basis_keeps_user_correction_when_original_response_was_prefilled() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = LearningStore::open(&directory.path().join("learning.db"), false).unwrap();
        performance(&mut store, "independent", AssessmentStatus::Correct);
        let original = store.teaching_event("independent").unwrap();
        let mut prefilled = original.clone();
        prefilled.event_id = "prefilled".into();
        prefilled.payload["response_origin"] = json!({"kind":"presentation_prefill","prefill_text":"我的推导"});
        store.append_teaching(&prefilled).unwrap();
        store.save_assessment(&ResponseAssessment { action_ref:"prefilled".into(), contract_ref:"delivery".into(),
            evaluator_version:"test".into(), status:AssessmentStatus::Correct, items:vec![] }).unwrap();
        store.derive_assessed_evidence("session").unwrap();
        let id = "assessment:prefilled:gradient";
        assert!(store.understanding_basis(&original.binding, id).is_err());
        let correction = store.correct_evidence(id, "prefill-correction", "这段是预填的，我还不理解").unwrap();
        let (basis, expressions) = store.understanding_basis(&original.binding, &correction.evidence_id).unwrap();
        assert_eq!(basis.correction.as_deref(), Some("这段是预填的，我还不理解"));
        assert!(expressions.is_empty());
        assert_eq!(store.teaching_event("prefilled").unwrap(), prefilled);
        let mut wrong_source = original.binding.clone();
        wrong_source.source_revision = "other-version".into();
        assert!(store.understanding_basis(&wrong_source, &correction.evidence_id).is_err());
        let mut wrong_session = correction.clone();
        wrong_session.evidence_id = "wrong-session".into();
        wrong_session.session_id = "other-session".into();
        store.connection.execute("INSERT INTO learning_evidence(learning_workspace_id,evidence_id,source_id,evidence) VALUES (?,?,?,?)",
            params![store.workspace_scope,wrong_session.evidence_id,wrong_session.source_id,serde_json::to_string(&wrong_session).unwrap()]).unwrap();
        assert!(store.understanding_basis(&original.binding, &wrong_session.evidence_id).is_err());
    }

    fn performance(store: &mut LearningStore, id: &str, status: AssessmentStatus) {
        let binding = TeachingBinding {
            tutor_session_id: "session".into(), session_revision: 1, control_revision: 0,
            source_id: "book".into(), source_revision: "source-v1".into(), map_revision: None,
            chat_session_id: "chat".into(), turn_id: "turn".into(),
        };
        store.append_teaching(&TeachingEvent {
            event_id: "delivery".into(), binding: binding.clone(), kind: TeachingFact::MessageDelivered,
            causal_refs: vec![], occurred_at: "now".into(),
            payload: json!({"move":{"capability":"explanation"},"object_versions":[{"ref":{"object_id":"gradient"},"object_revision":1,"meaning":"Gradient"}]}),
        }).unwrap();
        store.append_teaching(&TeachingEvent {
            event_id: id.into(), binding, kind: TeachingFact::LearnerAction,
            causal_refs: vec!["delivery".into()], occurred_at: "now".into(),
            payload: json!({"delivery_ref":"delivery","action":"submit","response":"我的推导","assistance_refs":[],"attempt":1}),
        }).unwrap();
        store.save_assessment(&ResponseAssessment { action_ref: id.into(), contract_ref: "delivery".into(),
            evaluator_version: "test".into(), status, items: vec![] }).unwrap();
        store.derive_assessed_evidence("session").unwrap();
    }

    #[test]
    fn w3_same_book_evidence_corrections_pages_and_projection_stay_in_workspace() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("learning.db");
        let mut a = LearningStore::open(&path, false).unwrap().with_workspace(Some("A"));
        let mut b = LearningStore::open(&path, false).unwrap().with_workspace(Some("B"));
        let mut legacy = LearningStore::open(&path, false).unwrap();
        performance(&mut legacy, "legacy", AssessmentStatus::Partial);
        performance(&mut a, "same-id", AssessmentStatus::Incorrect);
        performance(&mut b, "same-id", AssessmentStatus::Correct);
        performance(&mut a, "only-a", AssessmentStatus::Incorrect);
        assert!(b.teaching_event("only-a").is_err());
        assert!(b.assessment("only-a").unwrap().is_none());
        assert!(b.correct_evidence("assessment:only-a:gradient", "cross", "不属于此工作区").is_err());
        let correction = a.correct_evidence("assessment:same-id:gradient", "correct", "我理解了梯度，但计算出错").unwrap();
        assert_eq!(correction.supersedes.as_deref(), Some("assessment:same-id:gradient"));
        assert!(!b.evidence_is_superseded("assessment:same-id:gradient").unwrap());
        assert_eq!(b.current_interpretations("book", "source-v1", 0, 20).unwrap()[0].1.status, AssessmentStatus::Correct);
        assert_eq!(a.current_interpretations("book", "source-v1", 0, 1).unwrap()[0].1.evidence_id, "assessment:only-a:gradient");
        assert_eq!(a.current_interpretations("book", "source-v1", 1, 1).unwrap()[0].1.evidence_id, correction.evidence_id,"correcting an old interpretation does not make its original expression newer");
        let first = a.evidence_page("book", 0, 1).unwrap();
        let second = a.evidence_page("book", first[0].0, 1).unwrap();
        assert_eq!(first[0].1.status, AssessmentStatus::Incorrect);
        assert_eq!(second[0].1.evidence_id, "assessment:only-a:gradient");
        assert_eq!(b.object_evidence_refs("book", "gradient", 1, "explanation", None).unwrap().len(), 1);
        let a_projection = a.rebuild_learner_projection("book").unwrap();
        let b_projection = b.rebuild_learner_projection("book").unwrap();
        assert_eq!((a_projection.rows[0].difficulty, a_projection.rows[0].uncertain), (1, 1));
        assert_eq!(b_projection.rows[0].independent_support, 1);
        assert_eq!(legacy.rebuild_learner_projection("book").unwrap().rows[0].partial, 1);
        assert_eq!(b.teaching_events("session", 0, 100).unwrap().len(), 2);
        assert_eq!(b.teaching_activity_events("delivery").unwrap().len(), 1);
        assert_eq!(b.teaching_expression_interpretations("same-id").unwrap()[0].status, AssessmentStatus::Correct);
        assert_eq!(b.current_design_interpretations("session", "book", "source-v1", None, &[], 20).unwrap().len(), 1);
        assert_eq!(b.teaching_learning_opportunity("session", "book", "source-v1", None, &[]).unwrap(), (vec![], vec![]));
        assert!(b.teaching_lineage_support("session", "book", "source-v1", None).unwrap().is_empty());
        a.connection.execute("DELETE FROM learner_projection WHERE learning_workspace_id='A'", []).unwrap();
        assert_eq!(a.rebuild_learner_projection("book").unwrap(), a_projection);
        assert_eq!(b.learner_projection("book").unwrap().0.unwrap(), b_projection);
        drop(a);
        let mut reopened = LearningStore::open(&path, false).unwrap().with_workspace(Some("A"));
        assert_eq!(reopened.rebuild_learner_projection("book").unwrap(), a_projection);
        assert!(reopened.evidence("assessment:legacy:gradient").is_err());
    }

    #[test]
    fn w3_ordinary_understanding_is_recorded_while_teaching_is_disabled() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("learning.db");
        let mut a = LearningStore::open(&path, false).unwrap().with_workspace(Some("A"));
        let b = LearningStore::open(&path, false).unwrap().with_workspace(Some("B"));
        assert!(!a.state().unwrap().control.enabled);
        let binding = TeachingBinding { tutor_session_id:"ordinary".into(), session_revision:0, control_revision:0,
            source_id:"book".into(), source_revision:"source-v1".into(), map_revision:None,
            chat_session_id:"chat".into(), turn_id:"turn".into() };
        a.append_teaching(&TeachingEvent { event_id:"question".into(), binding, kind:TeachingFact::UsageObserved,
            causal_refs:vec![], payload:json!({"usage":{"kind":"user_message"},"response":"我不理解梯度为什么指向增长方向"}), occurred_at:"now".into() }).unwrap();
        let evidence = LearningEvidence {
            evidence_id:"question-understanding".into(), action_ref:"question".into(), delivery_ref:None,
            session_id:"ordinary".into(), source_id:"book".into(), source_revision:"source-v1".into(), map_revision:None,
            object_id:None, object_revision:None, nature:EvidenceNature::Hypothesis,
            target:Some(crate::teaching::TeachingTarget { media_bindings:None, music_bindings:None, learning_focus:"梯度方向".into(), expected_performance:"解释方向与变化率".into(),
                capability:"explanation".into(), source_bindings:vec![crate::teaching::TeachingSourceBinding {source_id:"book".into(), source_revision:"source-v1".into(),start_lid:"L1".into(),end_lid:"L2".into()}],object_refs:vec![] }),
            design_ref:None, block_refs:vec![], fact_refs:vec!["question".into()], teaching_implication:"结合方向导数解释".into(),
            label:"梯度方向".into(),capability:"explanation".into(),assistance_refs:vec![],support_delivery_refs:vec![],attempt:1,assessment_ref:None,
            status:AssessmentStatus::Unassessed,interpretation:"用户对梯度方向有疑问".into(),learner_quote:"我不理解梯度为什么指向增长方向".into(),
            interpreter_version:"test".into(),supersedes:None,correction:None,source_quotes:json!([]),
        };
        a.append_evidence(&evidence).unwrap();
        assert!(a.fact_has_interpretation("question").unwrap());
        assert!(!b.fact_has_interpretation("question").unwrap());
        assert_eq!(a.current_interpretations("book","source-v1",0,20).unwrap()[0].1,evidence);
        assert!(a.rebuild_learner_projection("book").unwrap().rows.is_empty());
        assert!(!a.state().unwrap().control.enabled);
    }
    #[test]
    fn learning_evidence_replays_conditions_corrections_and_interrupted_projection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("learning.db");
        let mut store = LearningStore::open(&path, false).unwrap();
        let binding = TeachingBinding {
            tutor_session_id: "session".into(),
            session_revision: 1,
            control_revision: 1,
            source_id: "book".into(),
            source_revision: "source-v1".into(),
            map_revision: Some("map-v1".into()),
            chat_session_id: "chat".into(),
            turn_id: "turn".into(),
        };
        let delivery = TeachingEvent {
            event_id: "delivery".into(),
            binding: binding.clone(),
            kind: TeachingFact::MessageDelivered,
            causal_refs: vec![],
            payload: json!({"move":{"capability":"explanation"},"object_versions":[{"ref":{"object_id":"speed"},"object_revision":1,"meaning":"Average speed"}]}),
            occurred_at: "now".into(),
        };
        store.append_teaching(&delivery).unwrap();
        for (id, help, attempt, status) in [
            ("independent", vec![], 1, Some(AssessmentStatus::Correct)),
            ("helped", vec!["help"], 1, Some(AssessmentStatus::Correct)),
            ("retry", vec![], 2, Some(AssessmentStatus::Correct)),
            ("failed", vec![], 1, None),
        ] {
            store.append_teaching(&TeachingEvent { event_id:id.into(),binding:binding.clone(),kind:TeachingFact::LearnerAction,causal_refs:vec!["delivery".into()],payload:json!({"delivery_ref":"delivery","action":"submit","response":"distance / time","assistance_refs":help,"attempt":attempt}),occurred_at:"now".into() }).unwrap();
            if let Some(status) = status {
                store
                    .save_assessment(&ResponseAssessment {
                        action_ref: id.into(),
                        contract_ref: "delivery".into(),
                        evaluator_version: "test".into(),
                        status,
                        items: vec![],
                    })
                    .unwrap();
            }
        }
        store.derive_assessed_evidence("session").unwrap();
        let original = store.rebuild_learner_projection("book").unwrap();
        assert_eq!(original.rows.len(), 1);
        assert_eq!(
            (
                original.rows[0].independent_support,
                original.rows[0].assisted_support,
                original.rows[0].revised_support
            ),
            (1, 1, 1)
        );
        assert_eq!(store.evidence_page("book", 0, 100).unwrap().len(), 3);
        store
            .connection
            .execute("DELETE FROM learner_projection", [])
            .unwrap();
        assert_eq!(store.rebuild_learner_projection("book").unwrap(), original);
        let correction = store
            .correct_evidence(
                "assessment:independent:speed",
                "correct",
                "I used a worked example",
            )
            .unwrap();
        assert_eq!(
            store
                .correct_evidence(
                    "assessment:independent:speed",
                    "correct",
                    "I used a worked example"
                )
                .unwrap(),
            correction
        );
        assert_ne!(
            store
                .learner_projection("book")
                .unwrap()
                .0
                .unwrap()
                .evidence_watermark,
            store.learner_projection("book").unwrap().1
        );
        store.connection.execute_batch("CREATE TRIGGER projection_fail BEFORE UPDATE ON learner_projection BEGIN SELECT RAISE(ABORT,'write failed'); END;").unwrap();
        assert!(store.rebuild_learner_projection("book").is_err());
        assert_eq!(
            store.learner_projection("book").unwrap().0.unwrap(),
            original
        );
        store
            .connection
            .execute_batch("DROP TRIGGER projection_fail")
            .unwrap();
        let corrected = store.rebuild_learner_projection("book").unwrap();
        assert_eq!(corrected.rows[0].independent_support, 0);
        assert_eq!(corrected.rows[0].uncertain, 1);
        assert_eq!(
            corrected.paths.values().next().unwrap()["evidence_ref"],
            "assessment:retry:speed"
        );
        store.derive_assessed_evidence("session").unwrap();
        assert_eq!(store.rebuild_learner_projection("book").unwrap(), corrected);
        drop(store);
        let mut reopened = LearningStore::open(&path, false).unwrap();
        assert_eq!(
            reopened.rebuild_learner_projection("book").unwrap(),
            corrected
        );
        assert_eq!(reopened.teaching_event("delivery").unwrap(), delivery);
    }
}
