//! Explicit references to selected old chat turns. Original facts and judgments
//! remain in their source scope; these records never become mastery projections.
use crate::{
    assessment::ResponseAssessment,
    learning::{storage, LearningStore},
    learning_evidence::LearningEvidence,
    teaching::{invalid, TeachingEvent, TeachingFact},
};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LearningRecordSelection {
    pub source_learning_workspace_id: Option<String>,
    pub chat_session_id: String,
    pub through_seq: u64,
    pub turn_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedLearningRecordSelection {
    pub selection: LearningRecordSelection,
    pub linked_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkedLearningRecords {
    pub selection: LearningRecordSelection,
    pub linked_at: String,
    pub facts: Vec<TeachingEvent>,
    pub evidence: Vec<LearningEvidence>,
    pub assessments: Vec<ResponseAssessment>,
}

impl LearningStore {
    /// The caller verifies chat ownership and obtains the turn IDs from the
    /// selected committed chat prefix. Only this reference is written here.
    pub fn link_records(
        &mut self,
        selection: &LearningRecordSelection,
        now: &str,
    ) -> Result<LinkedLearningRecordSelection, ToolError> {
        let source = selection
            .source_learning_workspace_id
            .as_deref()
            .unwrap_or_default();
        if self.workspace_scope.is_empty()
            || source == self.workspace_scope
            || selection.source_learning_workspace_id.as_deref() == Some("")
            || selection.chat_session_id.trim().is_empty()
            || selection.through_seq == 0
            || selection.turn_ids.is_empty()
            || selection.turn_ids.iter().any(|id| id.trim().is_empty())
        {
            return Err(invalid("请选择其他范围内已有的聊天记录，再带入学习工作区"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let existing: Option<(String, String)> = tx.query_row(
            "SELECT selection,linked_at FROM linked_learning_records WHERE learning_workspace_id=? AND source_learning_workspace_id=? AND chat_session_id=?",
            params![self.workspace_scope, source, selection.chat_session_id], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(storage)?;
        let mut accepted = selection.clone();
        let linked_at = if let Some((json, linked_at)) = existing.as_ref() {
            let old: LearningRecordSelection = serde_json::from_str(json).map_err(storage)?;
            accepted.through_seq = accepted.through_seq.max(old.through_seq);
            accepted.turn_ids.extend(old.turn_ids);
            linked_at.clone()
        } else {
            now.to_owned()
        };
        accepted.turn_ids.sort();
        accepted.turn_ids.dedup();
        let json = serde_json::to_string(&accepted).map_err(storage)?;
        if existing.as_ref().is_none_or(|(old, _)| old != &json) {
            tx.execute("INSERT INTO linked_learning_records(learning_workspace_id,source_learning_workspace_id,chat_session_id,selection,linked_at) VALUES (?,?,?,?,?) ON CONFLICT(learning_workspace_id,source_learning_workspace_id,chat_session_id) DO UPDATE SET selection=excluded.selection",
                params![self.workspace_scope, source, selection.chat_session_id, json, linked_at]).map_err(storage)?;
        }
        tx.commit().map_err(storage)?;
        Ok(LinkedLearningRecordSelection {
            selection: accepted,
            linked_at,
        })
    }

    pub fn linked_record_selections(
        &self,
    ) -> Result<Vec<LinkedLearningRecordSelection>, ToolError> {
        let mut q = self.connection.prepare("SELECT selection,linked_at FROM linked_learning_records WHERE learning_workspace_id=? ORDER BY linked_at,source_learning_workspace_id,chat_session_id").map_err(storage)?;
        let rows = q
            .query_map([&self.workspace_scope], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(storage)?;
        rows.map(|r| {
            let (selection, linked_at) = r.map_err(storage)?;
            Ok(LinkedLearningRecordSelection {
                selection: serde_json::from_str(&selection).map_err(storage)?,
                linked_at,
            })
        })
        .collect()
    }

    pub fn linked_record_bundles(&self) -> Result<Vec<LinkedLearningRecords>, ToolError> {
        self.linked_record_selections()?
            .into_iter()
            .map(|link| record_bundle(&self.connection, link.selection, link.linked_at))
            .collect()
    }

    /// The source scope and chat select an existing association. The caller's
    /// turn IDs and sequence cannot widen the saved selection.
    pub fn linked_records(
        &self,
        selection: &LearningRecordSelection,
    ) -> Result<LinkedLearningRecords, ToolError> {
        let saved: Option<(String, String)> = self.connection.query_row(
            "SELECT selection,linked_at FROM linked_learning_records WHERE learning_workspace_id=? AND source_learning_workspace_id=? AND chat_session_id=?",
            params![self.workspace_scope, selection.source_learning_workspace_id.as_deref().unwrap_or_default(), selection.chat_session_id],
            |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(storage)?;
        let (json, linked_at) = saved.ok_or_else(|| invalid("这份聊天记录尚未带入当前工作区"))?;
        record_bundle(
            &self.connection,
            serde_json::from_str(&json).map_err(storage)?,
            linked_at,
        )
    }

    /// Read a selected original chat prefix without creating an association.
    /// The server supplies through_seq separately for its chat projection.
    pub fn chat_record_bundle(
        &self,
        chat: &str,
        turn_ids: &[String],
    ) -> Result<LinkedLearningRecords, ToolError> {
        record_bundle(
            &self.connection,
            LearningRecordSelection {
                source_learning_workspace_id: self.learning_workspace_id().map(str::to_owned),
                chat_session_id: chat.to_owned(),
                through_seq: 0,
                turn_ids: turn_ids.to_vec(),
            },
            String::new(),
        )
    }
}

fn record_bundle(
    connection: &Connection,
    selection: LearningRecordSelection,
    linked_at: String,
) -> Result<LinkedLearningRecords, ToolError> {
    let scope = selection
        .source_learning_workspace_id
        .as_deref()
        .unwrap_or_default();
    let roots: Vec<(u64, TeachingEvent)> = {
        let mut q = connection.prepare("SELECT seq,event FROM teaching_trace WHERE learning_workspace_id=? AND json_extract(event,'$.binding.chat_session_id')=? AND json_extract(event,'$.binding.turn_id') IN (SELECT value FROM json_each(?)) ORDER BY seq").map_err(storage)?;
        let rows = q
            .query_map(
                params![
                    scope,
                    selection.chat_session_id,
                    serde_json::to_string(&selection.turn_ids).map_err(storage)?
                ],
                |r| Ok((r.get::<_, u64>(0)?, r.get::<_, String>(1)?)),
            )
            .map_err(storage)?;
        rows.map(|r| {
            let (seq, json) = r.map_err(storage)?;
            Ok((seq, serde_json::from_str(&json).map_err(storage)?))
        })
        .collect::<Result<_, ToolError>>()?
    };
    // Only the selected learner's actual expressions seed interpretation lookup.
    // A shared assistant delivery does not adopt another chat's responses.
    let expressions: Vec<_> = roots
        .iter()
        .filter(|(_, fact)| {
            fact.learner_expression().is_some() || fact.kind == TeachingFact::LearnerAction
        })
        .map(|(_, fact)| fact.event_id.as_str())
        .collect();
    let mut pending_evidence = BTreeSet::new();
    {
        let mut q = connection.prepare("SELECT evidence_id FROM learning_evidence WHERE learning_workspace_id=?1 AND (json_extract(evidence,'$.action_ref') IN (SELECT value FROM json_each(?2)) OR EXISTS(SELECT 1 FROM json_each(evidence,'$.fact_refs') f JOIN json_each(?2) selected ON selected.value=f.value))").map_err(storage)?;
        for id in q
            .query_map(
                params![scope, serde_json::to_string(&expressions).map_err(storage)?],
                |r| r.get::<_, String>(0),
            )
            .map_err(storage)?
        {
            pending_evidence.insert(id.map_err(storage)?);
        }
    }
    let mut pending_facts: BTreeSet<_> = roots
        .iter()
        .map(|(_, fact)| fact.event_id.clone())
        .collect();
    let mut pending_assessments = BTreeSet::new();
    let mut visited_facts = BTreeSet::new();
    let mut visited_evidence = BTreeSet::new();
    let mut visited_assessments = BTreeSet::new();
    let mut facts = BTreeMap::new();
    let mut evidence = BTreeMap::new();
    let mut assessments = BTreeMap::new();
    while !pending_facts.is_empty()
        || !pending_evidence.is_empty()
        || !pending_assessments.is_empty()
    {
        while let Some(id) = pending_facts.pop_first() {
            if !visited_facts.insert(id.clone()) {
                continue;
            }
            let row: Option<(u64, String)> = connection.query_row("SELECT seq,event FROM teaching_trace WHERE learning_workspace_id=? AND event_id=?", params![scope,id], |r| Ok((r.get(0)?,r.get(1)?))).optional().map_err(storage)?;
            let Some((seq, json)) = row else {
                continue;
            };
            let event: TeachingEvent = serde_json::from_str(&json).map_err(storage)?;
            pending_facts.extend(event.causal_refs.iter().cloned());
            event_references(&event, &mut pending_facts, &mut pending_evidence);
            pending_assessments.insert(id);
            facts.insert(seq, event);
        }
        while let Some(id) = pending_evidence.pop_first() {
            if !visited_evidence.insert(id.clone()) {
                continue;
            }
            let row: Option<(u64, String)> = connection.query_row("SELECT seq,evidence FROM learning_evidence WHERE learning_workspace_id=? AND evidence_id=?", params![scope,id], |r| Ok((r.get(0)?,r.get(1)?))).optional().map_err(storage)?;
            let Some((seq, json)) = row else {
                continue;
            };
            let item: LearningEvidence = serde_json::from_str(&json).map_err(storage)?;
            pending_facts.insert(item.action_ref.clone());
            pending_facts.extend(item.delivery_ref.iter().cloned());
            pending_facts.extend(item.design_ref.iter().cloned());
            pending_facts.extend(item.fact_refs.iter().cloned());
            pending_facts.extend(item.assistance_refs.iter().cloned());
            pending_assessments.extend(item.assessment_ref.iter().cloned());
            pending_evidence.extend(item.supersedes.iter().cloned());
            let mut q = connection.prepare("SELECT evidence_id FROM learning_evidence WHERE learning_workspace_id=? AND json_extract(evidence,'$.supersedes')=?").map_err(storage)?;
            for next in q
                .query_map(params![scope, id], |r| r.get::<_, String>(0))
                .map_err(storage)?
            {
                pending_evidence.insert(next.map_err(storage)?);
            }
            evidence.insert(seq, item);
        }
        while let Some(id) = pending_assessments.pop_first() {
            if !visited_assessments.insert(id.clone()) {
                continue;
            }
            let row: Option<String> = connection.query_row("SELECT assessment FROM teaching_assessments WHERE learning_workspace_id=? AND action_ref=?", params![scope,id], |r| r.get(0)).optional().map_err(storage)?;
            if let Some(json) = row {
                let item: ResponseAssessment = serde_json::from_str(&json).map_err(storage)?;
                pending_facts.insert(item.contract_ref.clone());
                assessments.insert(id, item);
            }
        }
    }
    Ok(LinkedLearningRecords {
        selection,
        linked_at,
        facts: facts.into_values().collect(),
        evidence: evidence.into_values().collect(),
        assessments: assessments.into_values().collect(),
    })
}

fn add_string(value: &Value, refs: &mut BTreeSet<String>) {
    if let Some(id) = value.as_str().filter(|s| !s.is_empty()) {
        refs.insert(id.to_owned());
    }
}
fn add_array(value: &Value, refs: &mut BTreeSet<String>) {
    for item in value.as_array().into_iter().flatten() {
        add_string(item, refs);
    }
}

fn event_references(
    event: &TeachingEvent,
    facts: &mut BTreeSet<String>,
    evidence: &mut BTreeSet<String>,
) {
    let p = &event.payload;
    // These are actual causal links, not arbitrary IDs embedded in a frozen
    // context snapshot. In particular TurnBound.learner_context is not traversed.
    for key in ["delivery_ref", "invitation_context_ref", "assistance_for"] {
        add_string(&p[key], facts);
    }
    for key in ["assistance_refs", "support_delivery_refs"] {
        add_array(&p[key], facts);
    }
    let movement = &p["move"];
    add_string(&movement["design_ref"], facts);
    add_string(&movement["responds_to"]["invitation_ref"], facts);
    add_array(&movement["responds_to"]["fact_refs"], facts);
    add_array(&movement["interpretation_refs"], evidence);
    let design = &p["design"];
    add_string(&design["previous_ref"], facts);
    add_string(&design["return_to"]["invitation_ref"], facts);
    add_string(&design["return_to"]["scene_ref"], facts);
    add_array(&design["open_question_refs"], facts);
    // Design basis_refs may refer to either an actual fact or an interpretation.
    add_array(&design["basis_refs"], facts);
    add_array(&design["basis_refs"], evidence);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assessment::AssessmentStatus,
        learning_evidence::EvidenceNature,
        teaching::{TeachingBinding, TeachingSourceBinding, TeachingTarget},
    };
    use serde_json::json;

    fn observation(
        store: &mut LearningStore,
        chat: &str,
        turn: &str,
        id: &str,
        source: &str,
        causes: &[&str],
    ) -> LearningEvidence {
        let binding = TeachingBinding {
            tutor_session_id: format!("observation:{source}"),
            session_revision: 0,
            control_revision: 0,
            source_id: source.into(),
            source_revision: "original-source-v1".into(),
            map_revision: None,
            chat_session_id: chat.into(),
            turn_id: turn.into(),
        };
        store.append_teaching(&TeachingEvent { event_id: id.into(), binding,
            kind: TeachingFact::UsageObserved, causal_refs: causes.iter().map(|s| (*s).into()).collect(),
            payload: json!({"usage":{"kind":"user_message"},"response":"为什么梯度指向增长方向？",
                "published_book_ref":{"book_id":source,"publication_id":"original-publication"}}), occurred_at: "then".into() }).unwrap();
        let evidence = LearningEvidence {
            evidence_id: format!("interpretation:{id}"),
            action_ref: id.into(),
            delivery_ref: None,
            session_id: format!("observation:{source}"),
            source_id: source.into(),
            source_revision: "original-source-v1".into(),
            map_revision: None,
            object_id: None,
            object_revision: None,
            nature: EvidenceNature::Hypothesis,
            target: Some(TeachingTarget {
                media_bindings: None,
                learning_focus: "梯度方向".into(),
                expected_performance: "解释方向".into(),
                capability: "explanation".into(),
                source_bindings: vec![TeachingSourceBinding {
                    source_id: source.into(),
                    source_revision: "original-source-v1".into(),
                    start_lid: "1.1".into(),
                    end_lid: "1.2".into(),
                }],
                object_refs: vec![],
            }),
            design_ref: None,
            block_refs: vec![],
            fact_refs: vec![id.into()],
            teaching_implication: "结合方向导数解释".into(),
            label: "梯度方向".into(),
            capability: "explanation".into(),
            assistance_refs: vec![],
            attempt: 0,
            assessment_ref: None,
            status: AssessmentStatus::Unassessed,
            interpretation: "用户询问梯度方向".into(),
            learner_quote: "为什么梯度指向增长方向？".into(),
            interpreter_version: "test".into(),
            supersedes: None,
            correction: None,
            source_quotes: json!([{"publication_id":"original-publication","lid":"1.1"}]),
        };
        store.append_evidence(&evidence).unwrap()
    }

    fn original_rows(store: &LearningStore, scope: &str) -> Vec<(String, String)> {
        let mut q = store.connection.prepare("SELECT 'fact:'||event_id,event FROM teaching_trace WHERE learning_workspace_id=?1 UNION ALL SELECT 'evidence:'||evidence_id,evidence FROM learning_evidence WHERE learning_workspace_id=?1 UNION ALL SELECT 'assessment:'||action_ref,assessment FROM teaching_assessments WHERE learning_workspace_id=?1 ORDER BY 1").unwrap();
        let rows = q
            .query_map([scope], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        rows.map(Result::unwrap).collect()
    }

    #[test]
    fn w6_selected_turns_keep_reference_sources_revision_chain_and_idempotent_links() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("learning.db");
        let mut old = LearningStore::open(&path, false).unwrap();
        let unrelated = observation(
            &mut old,
            "other-chat",
            "other-turn",
            "unselected",
            "book",
            &[],
        );
        let mut support = old.teaching_event("unselected").unwrap();
        support.event_id = "shared-support".into();
        support.kind = TeachingFact::MessageDelivered;
        support.payload = json!({"answer":"原来的帮助"});
        old.append_teaching(&support).unwrap();
        let selected = observation(
            &mut old,
            "selected-chat",
            "selected-turn",
            "selected",
            "book",
            &["shared-support"],
        );
        let reference = observation(
            &mut old,
            "selected-chat",
            "selected-turn",
            "reference",
            "reference-book",
            &[],
        );
        let correction = old
            .correct_evidence(&selected.evidence_id, "before-link", "我想问的是变化率")
            .unwrap();
        let future = observation(
            &mut old,
            "selected-chat",
            "future-turn",
            "future",
            "book",
            &[],
        );
        let mut next_to_support = unrelated.clone();
        next_to_support.evidence_id = "unselected-with-old-help".into();
        next_to_support.fact_refs.push("shared-support".into());
        // Historical records could cite delivered help as a fact. Seed the old
        // row directly: new hypotheses now require actual learner expressions.
        old.connection.execute("INSERT INTO learning_evidence(learning_workspace_id,evidence_id,source_id,evidence) VALUES (?,?,?,?)",
            params![old.workspace_scope,next_to_support.evidence_id,next_to_support.source_id,serde_json::to_string(&next_to_support).unwrap()]).unwrap();
        let mut target = LearningStore::open(&path, false)
            .unwrap()
            .with_workspace(Some("target"));
        assert!(target.linked_record_bundles().unwrap().is_empty());
        let before = original_rows(&old, "");
        let choice = LearningRecordSelection {
            source_learning_workspace_id: None,
            chat_session_id: "selected-chat".into(),
            through_seq: 12,
            turn_ids: vec!["selected-turn".into()],
        };
        let linked = target.link_records(&choice, "first-link").unwrap();
        assert_eq!(target.link_records(&choice, "retry").unwrap(), linked);
        assert_eq!(target.linked_record_selections().unwrap(), vec![linked]);
        assert_eq!(
            original_rows(&old, ""),
            before,
            "linking cannot change source bytes or append facts"
        );
        assert!(original_rows(&target, "target").is_empty());
        let bundle = target.linked_record_bundles().unwrap().pop().unwrap();
        assert_eq!(bundle.selection, choice);
        assert_eq!(
            bundle
                .facts
                .iter()
                .map(|f| f.event_id.as_str())
                .collect::<Vec<_>>(),
            ["shared-support", "selected", "reference"]
        );
        assert_eq!(
            bundle
                .evidence
                .iter()
                .map(|e| e.evidence_id.as_str())
                .collect::<Vec<_>>(),
            [
                selected.evidence_id.as_str(),
                reference.evidence_id.as_str(),
                correction.evidence_id.as_str()
            ]
        );
        assert_eq!(bundle.evidence[1].source_quotes, reference.source_quotes);
        assert!(target
            .current_interpretations("book", "original-source-v1", 0, 20)
            .unwrap()
            .is_empty());
        assert!(target
            .rebuild_learner_projection("book")
            .unwrap()
            .rows
            .is_empty());
        assert!(target.teaching_event("selected").is_err());
        assert!(target
            .correct_evidence(&selected.evidence_id, "wrong-scope", "不能改源记录")
            .is_err());
        let latest = old
            .correct_evidence(&correction.evidence_id, "after-link", "确认原表达的意思")
            .unwrap();
        let bundle = target.linked_record_bundles().unwrap().pop().unwrap();
        assert!(bundle.evidence.iter().any(|e| e == &latest));
        assert!(!bundle
            .evidence
            .iter()
            .any(|e| e.evidence_id == future.evidence_id));
        let mut unaccepted = choice.clone();
        unaccepted.turn_ids.push("future-turn".into());
        unaccepted.through_seq = 999;
        assert_eq!(
            target.linked_records(&unaccepted).unwrap().selection,
            choice
        );
        unaccepted.source_learning_workspace_id = Some("unlinked-origin".into());
        assert!(target.linked_records(&unaccepted).is_err());
        let before_local = original_rows(&old, "");
        let local = observation(
            &mut target,
            "target-chat",
            "target-turn",
            "selected",
            "book",
            &[],
        );
        target
            .correct_evidence(&local.evidence_id, "local", "本工作区自己的解释")
            .unwrap();
        assert_eq!(original_rows(&old, ""), before_local);
        let mut expanded = choice.clone();
        expanded.through_seq = 18;
        expanded.turn_ids.push("future-turn".into());
        let accepted = target
            .link_records(&expanded, "explicit-second-choice")
            .unwrap();
        assert_eq!(accepted.linked_at, "first-link");
        assert_eq!(accepted.selection.through_seq, 18);
        assert_eq!(
            accepted.selection.turn_ids,
            ["future-turn", "selected-turn"]
        );
        assert!(target.linked_record_bundles().unwrap()[0]
            .evidence
            .iter()
            .any(|e| e == &future));
        drop(target);
        let reopened = LearningStore::open(&path, false)
            .unwrap()
            .with_workspace(Some("target"));
        assert_eq!(reopened.linked_record_selections().unwrap(), [accepted]);
        assert!(LearningStore::open(&path, false)
            .unwrap()
            .with_workspace(Some("other-target"))
            .linked_record_bundles()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn w6_assessment_and_design_causes_are_read_in_origin_without_inheriting_performance() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("learning.db");
        let mut origin = LearningStore::open(&path, false)
            .unwrap()
            .with_workspace(Some("origin"));
        let selected = observation(&mut origin, "chat", "turn", "basis", "book", &[]);
        let binding = origin.teaching_event("basis").unwrap().binding;
        origin.append_teaching(&TeachingEvent { event_id:"delivery".into(), binding:binding.clone(),
            kind:TeachingFact::MessageDelivered, causal_refs:vec!["basis".into()], occurred_at:"then".into(),
            payload:json!({"move":{"capability":"explanation","interpretation_refs":[selected.evidence_id]},
                "object_versions":[{"ref":{"object_id":"gradient"},"object_revision":1}]}) }).unwrap();
        origin.append_teaching(&TeachingEvent { event_id:"action".into(), binding,
            kind:TeachingFact::LearnerAction, causal_refs:vec!["delivery".into()], occurred_at:"then".into(),
            payload:json!({"delivery_ref":"delivery","action":"submit","response":"沿梯度方向变化最大","attempt":1}) }).unwrap();
        let assessment = ResponseAssessment {
            action_ref: "action".into(),
            contract_ref: "delivery".into(),
            evaluator_version: "test".into(),
            status: AssessmentStatus::Correct,
            items: vec![],
        };
        origin.save_assessment(&assessment).unwrap();
        origin.derive_assessed_evidence("observation:book").unwrap();
        let snapshot = original_rows(&origin, "origin");
        let mut target = LearningStore::open(&path, false)
            .unwrap()
            .with_workspace(Some("target"));
        target
            .link_records(
                &LearningRecordSelection {
                    source_learning_workspace_id: Some("origin".into()),
                    chat_session_id: "chat".into(),
                    through_seq: 10,
                    turn_ids: vec!["turn".into()],
                },
                "now",
            )
            .unwrap();
        let bundle = target.linked_record_bundles().unwrap().pop().unwrap();
        assert_eq!(bundle.assessments, [assessment]);
        assert_eq!(bundle.evidence.len(), 2);
        assert!(bundle
            .evidence
            .iter()
            .any(|e| e.status == AssessmentStatus::Correct));
        assert!(target
            .rebuild_learner_projection("book")
            .unwrap()
            .rows
            .is_empty());
        assert!(target.assessment("action").unwrap().is_none());
        assert_eq!(original_rows(&origin, "origin"), snapshot);
        assert_eq!(
            origin
                .chat_record_bundle("chat", &["turn".into()])
                .unwrap()
                .evidence,
            bundle.evidence
        );
    }

    #[test]
    fn w6_version_two_upgrade_adds_only_empty_reference_storage() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("learning.db");
        let mut original = LearningStore::open(&path, false).unwrap();
        observation(&mut original, "chat", "turn", "old", "book", &[]);
        let before = original_rows(&original, "");
        original
            .connection
            .execute_batch("DROP TABLE linked_learning_records; PRAGMA user_version=2;")
            .unwrap();
        drop(original);
        let upgraded = LearningStore::open(&path, false).unwrap();
        assert_eq!(original_rows(&upgraded, ""), before);
        assert!(upgraded.linked_record_selections().unwrap().is_empty());
        assert_eq!(
            upgraded
                .connection
                .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            3
        );
    }
}
