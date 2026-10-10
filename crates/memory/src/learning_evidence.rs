//! Append-only interpretations and deterministic, disposable learner projections.
use crate::{
    assessment::AssessmentStatus,
    learning::{storage, LearningStore},
    teaching::{invalid, TeachingFact},
};
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const ESTIMATOR_VERSION: &str = "learner.conditions.v2";

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
    #[serde(default)]
    pub fact_refs: Vec<String>,
    #[serde(default)]
    pub teaching_implication: String,
    pub label: String,
    pub capability: String,
    pub assistance_refs: Vec<String>,
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
        self.connection.query_row("SELECT EXISTS(SELECT 1 FROM learning_evidence e, json_each(e.evidence,'$.fact_refs') f WHERE f.value=?)", [id], |r|r.get(0)).map_err(storage)
    }
    pub fn evidence_is_superseded(&self, id: &str) -> Result<bool, ToolError> {
        self.connection.query_row("SELECT EXISTS(SELECT 1 FROM learning_evidence WHERE json_extract(evidence,'$.supersedes')=?)", [id], |r|r.get(0)).map_err(storage)
    }
    pub fn evidence(&self, id: &str) -> Result<LearningEvidence, ToolError> {
        let row: Option<String> = self
            .connection
            .query_row(
                "SELECT evidence FROM learning_evidence WHERE evidence_id=?",
                [id],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        serde_json::from_str(&row.ok_or_else(|| invalid("学习证据不存在"))?).map_err(storage)
    }

    pub fn append_evidence(
        &mut self,
        evidence: &LearningEvidence,
    ) -> Result<LearningEvidence, ToolError> {
        let existing: Option<String> = self
            .connection
            .query_row(
                "SELECT evidence FROM learning_evidence WHERE evidence_id=?",
                [&evidence.evidence_id],
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
        if evidence.nature == EvidenceNature::Hypothesis {
            if evidence.fact_refs.is_empty() || !evidence.fact_refs.contains(&evidence.action_ref)
                || evidence.interpretation.trim().is_empty() || evidence.teaching_implication.trim().is_empty()
                || evidence.assessment_ref.is_some() || evidence.status != AssessmentStatus::Unassessed
                || evidence.delivery_ref.is_some() || evidence.object_id.is_some() || evidence.object_revision.is_some()
            { return Err(invalid("理解假设需要事实与教学影响，不附加评分或对象能力")); }
            for id in &evidence.fact_refs {
                let fact = self.teaching_event(id)?;
                if fact.binding.source_id != evidence.source_id || fact.binding.source_revision != evidence.source_revision
                    || fact.binding.tutor_session_id != evidence.session_id
                    || matches!(fact.kind, TeachingFact::TurnBound | TeachingFact::MoveSelected | TeachingFact::HelpDelivered)
                { return Err(invalid("理解依据不属于当前来源、会话或实际使用事实")); }
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
            let replaced:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM learning_evidence WHERE json_extract(evidence,'$.supersedes')=?)",[id],|r|r.get(0)).map_err(storage)?;
            if replaced {
                return Err(invalid("解释已经更新，请刷新后纠正"));
            }
        }
        self.connection
            .execute(
                "INSERT INTO learning_evidence(evidence_id,source_id,evidence) VALUES (?,?,?)",
                params![
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
    pub fn current_interpretations(&self, source: &str, revision: &str, before: Option<u64>, limit: u32) -> Result<Vec<(u64, LearningEvidence)>, ToolError> {
        let mut q = self.connection.prepare("SELECT e.seq,e.evidence FROM learning_evidence e WHERE e.source_id=? AND json_extract(e.evidence,'$.source_revision')=? AND e.seq<? AND NOT EXISTS (SELECT 1 FROM learning_evidence n WHERE json_extract(n.evidence,'$.supersedes')=e.evidence_id) ORDER BY e.seq DESC LIMIT ?").map_err(storage)?;
        let rows = q.query_map(params![source, revision, before.unwrap_or(i64::MAX as u64), limit.min(20)], |r|Ok((r.get::<_,u64>(0)?,r.get::<_,String>(1)?))).map_err(storage)?;
        rows.map(|r| { let (seq,text)=r.map_err(storage)?; Ok((seq,serde_json::from_str(&text).map_err(storage)?)) }).collect()
    }

    pub fn evidence_page(
        &self,
        source: &str,
        after: u64,
        limit: u32,
    ) -> Result<Vec<(u64, LearningEvidence)>, ToolError> {
        let mut query=self.connection.prepare("SELECT seq,evidence FROM learning_evidence WHERE source_id=? AND seq>? ORDER BY seq LIMIT ?").map_err(storage)?;
        let rows = query
            .query_map(params![source, after, limit.min(100)], |r| {
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
        let mut q=self.connection.prepare("SELECT seq,evidence_id FROM learning_evidence WHERE source_id=? AND json_extract(evidence,'$.object_id')=? AND json_extract(evidence,'$.object_revision')=? AND json_extract(evidence,'$.capability')=? AND seq<? ORDER BY seq DESC LIMIT 10").map_err(storage)?;
        let rows = q
            .query_map(
                params![
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
                "SELECT projection FROM learner_projection WHERE source_id=?",
                [source],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        let watermark = self
            .connection
            .query_row(
                "SELECT coalesce(max(seq),0) FROM learning_evidence WHERE source_id=?",
                [source],
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
            let mut q = tx
                .prepare(
                    "SELECT seq,evidence FROM learning_evidence WHERE source_id=? ORDER BY seq",
                )
                .map_err(storage)?;
            let rows = q
                .query_map([source], |r| {
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
                AssessmentStatus::Correct if !e.assistance_refs.is_empty() => {
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
                    "SELECT seq FROM teaching_trace WHERE event_id=?",
                    [&e.action_ref],
                    |r| r.get(0),
                )
                .map_err(storage)?;
            if paths
                .get(&key)
                .is_none_or(|p| p["action_sequence"].as_u64().unwrap_or(0) <= action_sequence)
            {
                paths.insert(key,json!({"session_id":e.session_id,"object_id":e.object_id,"object_revision":e.object_revision,"source_revision":e.source_revision,"capability":e.capability,"last_status":e.status,"assistance_refs":e.assistance_refs,"attempt":e.attempt,"evidence_ref":e.evidence_id,"action_sequence":action_sequence}));
            }
        }
        let projection = LearnerProjection {
            estimator_version: ESTIMATOR_VERSION.into(),
            evidence_watermark: all.last().map_or(0, |(seq, _)| *seq),
            teaching_map_revision_refs: all
                .iter()
                .filter_map(|(_, e)| e.map_revision.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            rows: rows.into_values().collect(),
            paths,
        };
        tx.execute("INSERT INTO learner_projection(source_id,projection) VALUES (?,?) ON CONFLICT(source_id) DO UPDATE SET projection=excluded.projection",params![source,serde_json::to_string(&projection).map_err(storage)?]).map_err(storage)?;
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
