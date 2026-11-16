//! Current, correctable interpretations over retained evidence. No model runs on reads.
use crate::{
    assessment::AssessmentStatus,
    learning::{storage, LearningStore},
    learning_evidence::{EvidenceNature, LearningEvidence},
    teaching::{invalid, TeachingBinding, TeachingTarget},
    NoteRef,
};
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(crate) const SCHEMA: &str = "
CREATE TABLE understanding_focus (learning_workspace_id TEXT NOT NULL, focus_ref TEXT NOT NULL,
    source_id TEXT NOT NULL, source_revision TEXT NOT NULL, target TEXT NOT NULL,
    PRIMARY KEY(learning_workspace_id,focus_ref));
CREATE INDEX understanding_focus_source ON understanding_focus(learning_workspace_id,source_id,source_revision);
CREATE INDEX evidence_focus ON learning_evidence(learning_workspace_id,source_id,json_extract(evidence,'$.target.focus_ref'));
CREATE INDEX evidence_supersedes ON learning_evidence(learning_workspace_id,json_extract(evidence,'$.supersedes'));
CREATE INDEX teaching_expression_turn ON teaching_trace(learning_workspace_id,json_extract(event,'$.binding.chat_session_id'),json_extract(event,'$.binding.turn_id'));
CREATE TABLE understanding_note_evidence (note_id TEXT NOT NULL, revision INTEGER NOT NULL, learning_workspace_id TEXT NOT NULL, evidence_id TEXT NOT NULL,
    PRIMARY KEY(note_id,revision,learning_workspace_id,evidence_id));
";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnderstandingObservation {
    /// Learner-facing account of what changed, separate from the teacher's plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learner_summary: Option<LearnerUnderstandingSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub knowledge_links: Vec<read_tools::knowledge::KnowledgeLink>,
    #[serde(default)]
    pub demonstrated: Vec<String>,
    #[serde(default)]
    pub difficulties: Vec<String>,
    #[serde(default)]
    pub unconfirmed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(deny_unknown_fields)]
pub struct LearnerUnderstandingSummary {
    /// One short title naming the most important change in understanding or specific question, without listing several conclusions.
    pub headline: String,
    /// One or two sentences summarizing the relationship actually expressed and the help received. Keep detailed values in the evidence and do not infer unexpressed views.
    pub explanation: String,
    /// An optional action related to the current question; null for ordinary course continuation. Do not write instructions for the teacher.
    pub next_step: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviousUnderstanding {
    pub evidence_ref: String,
    pub learner_quote: String,
    pub assistance: String,
}

fn assistance_label(e: &LearningEvidence) -> String {
    if !e.assistance_refs.is_empty() || !e.support_delivery_refs.is_empty() {
        "在已提供帮助下"
    } else if e.attempt > 1 {
        "修改回答后"
    } else {
        "本次未记录提示或先前讲解"
    }.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assessment::{AssessmentStatus, ResponseAssessment};
    use crate::teaching::{TeachingEvent, TeachingFact};
    fn binding(turn: &str) -> TeachingBinding {
        TeachingBinding {
            tutor_session_id: "stage".into(),
            session_revision: 1,
            control_revision: 1,
            source_id: "book".into(),
            source_revision: "v1".into(),
            map_revision: None,
            chat_session_id: "chat".into(),
            turn_id: turn.into(),
        }
    }
    fn target(focus: &str, capability: &str) -> TeachingTarget {
        serde_json::from_value(json!({"learning_focus":focus,"expected_performance":format!("解释{focus}"),"capability":capability,"source_bindings":[{"source_id":"book","source_revision":"v1","start_lid":"1.1","end_lid":"1.1"}]})).unwrap()
    }
    fn hypothesis(
        store: &mut LearningStore,
        id: &str,
        time: &str,
        words: &str,
        understanding: Value,
    ) -> LearningEvidence {
        let b = binding(id);
        store
            .append_teaching(&TeachingEvent {
                event_id: id.into(),
                binding: b.clone(),
                kind: TeachingFact::UsageObserved,
                causal_refs: vec![],
                payload: json!({"usage":{"kind":"user_message"},"response":words}),
                occurred_at: time.into(),
            })
            .unwrap();
        let mut t = target("全程时间包含停车时间", "explanation");
        store
            .bind_focus(&mut t, &b, &format!("focus:{id}"))
            .unwrap();
        let e:LearningEvidence=serde_json::from_value(json!({"evidence_id":format!("e:{id}"),"action_ref":id,"session_id":"stage","source_id":"book","source_revision":"v1","nature":"hypothesis","target":t,"fact_refs":[id],"teaching_implication":"比较含停车的一次行程","label":t.learning_focus,"capability":t.capability,"assistance_refs":[],"attempt":0,"status":"unassessed","interpretation":"按实际表达保留观察","learner_quote":words,"interpreter_version":"test","source_quotes":[],"understanding":understanding,"note_refs":[{"note_id":"note-a","revision":1}]})).unwrap();
        store.append_evidence(&e).unwrap()
    }
    #[test]
    fn learner_summary_keeps_the_change_and_never_exposes_the_teacher_plan() {
        let root=tempfile::tempdir().unwrap();
        let mut store=LearningStore::open(&root.path().join("learning.db"),false).unwrap();
        let first=hypothesis(&mut store,"first","2026-11-12T10:00:00Z","停车不计入时间",json!({"difficulties":["忽略停车时间"]}));
        assert!(store.understanding_summaries("book","v1").unwrap()[0].next_step.is_empty());
        hypothesis(&mut store,"later","2026-11-12T10:01:00Z","停车也算在全程时间里",json!({
            "demonstrated":["说明停车时间属于全程时间"],"unconfirmed":["尚未在新情况中独立应用"],
            "learner_summary":{"headline":"你把停车时间补进了原来的解释。","explanation":"现在你能说明全程时间包含停车。","next_step":"换一次行程，试着自己判断。"}
        }));
        let summaries=store.understanding_summaries("book","v1").unwrap();
        let current=&summaries[0];
        assert_eq!(current.learner_summary.as_ref().unwrap().headline,"你把停车时间补进了原来的解释。");
        assert_eq!(current.previous.as_ref().unwrap().evidence_ref,first.evidence_id);
        assert_eq!(current.previous.as_ref().unwrap().learner_quote,"停车不计入时间");
        assert_eq!(current.next_step,"换一次行程，试着自己判断。");
        assert!(current.difficulties.is_empty());
        assert_eq!(current.demonstrated.len(),1);
    }

    #[test]
    fn course_response_freezes_understanding_from_an_earlier_conversation() {
        use crate::course::{InquiryCourse,CourseMutation,CourseAction,CourseResponse};
        let root=tempfile::tempdir().unwrap();
        let mut store=LearningStore::open(&root.path().join("learning.db"),false).unwrap().with_workspace(Some("learner"));
        let old=hypothesis(&mut store,"previous-chat","2026-11-12T09:00:00Z","停车也算在全程时间里",json!({"demonstrated":["解释总时间"],"unconfirmed":["尚未独立应用"]}));
        let mut course:InquiryCourse=serde_json::from_str(include_str!("../../../docs/examples/inquiry-course/ai-infra-resource-bounds.course.json")).unwrap();
        let lesson=&mut course.lessons[0];
        lesson.summary.source.publication.book_id="book".into();lesson.summary.source.source_revision="v1".into();
        let mut view=store.mutate_course(&CourseMutation{operation_id:"start".into(),run_id:None,expected_revision:0,release_version:1,action:CourseAction::Start{lesson_id:lesson.summary.id.clone()}},Some(("release",1,lesson)),1,"2026-11-12T10:00:00Z",None).unwrap();
        for (id,action) in [("advance",CourseAction::Advance),("predict",CourseAction::Predict{response:CourseResponse{choice:lesson.prediction.options[0].id.clone(),reason:"先比较两个时间".into()}})] {
            view=store.mutate_course(&CourseMutation{operation_id:id.into(),run_id:Some(view.progress.run_id.clone()),expected_revision:view.progress.revision,release_version:1,action},None,1,"2026-11-12T10:01:00Z",None).unwrap();
        }
        let job_ref=&view.expressions[0].job_ref;
        let job=store.coach_job(job_ref).unwrap().unwrap();
        let prior=&job.input.context["teaching_context"]["learner_context"]["current_understanding"];
        assert_eq!(prior[0]["evidence_ref"],old.evidence_id);
        assert_eq!(prior[0]["unconfirmed"][0],"尚未独立应用");
        hypothesis(&mut store,"future-chat","2026-11-12T11:00:00Z","这是后来的表达",json!({"demonstrated":["不能回填旧任务"]}));
        let saved=store.coach_job(job_ref).unwrap().unwrap();
        assert_eq!(&saved.input.context["teaching_context"]["learner_context"]["current_understanding"],prior);
    }

    #[test]
    fn a_new_expression_changes_the_current_view_without_moving_the_old_branch_quote() {
        let root=tempfile::tempdir().unwrap();
        let mut store=LearningStore::open(&root.path().join("learning.db"),false).unwrap();
        let observation=|artifact:&str,entry:&str,quote:&str|json!({"knowledge_links":[{"reference":{"source_id":"book","source_revision":"v1","publication_id":null,"artifact":artifact,"entry_id":entry},"quote":quote}]});
        let first=hypothesis(&mut store,"before","2026-11-05T10:00:00Z","停车不算时间",observation("discourse","1.1","停车不算时间"));
        let later=hypothesis(&mut store,"after","2026-11-05T10:01:00Z","停车也算总时间",observation("pass1","concept:time","停车也算总时间"));
        let summaries=store.understanding_summaries("book","v1").unwrap();
        let old=summaries.iter().find(|s|s.evidence_ref==first.evidence_id).unwrap();
        let current=summaries.iter().find(|s|s.evidence_ref==later.evidence_id).unwrap();
        assert_eq!(old.focus_ref,current.focus_ref);
        assert_eq!(old.later_evidence_ref.as_deref(),Some(later.evidence_id.as_str()));
        assert!(current.later_evidence_ref.is_none());
        assert_eq!(old.learner_quote,"停车不算时间");
        assert_eq!(old.knowledge_links[0].reference.artifact,"discourse");
        assert_eq!(current.learner_quote,"停车也算总时间");
        assert_eq!(current.knowledge_links[0].reference.artifact,"pass1");
        assert_eq!(summaries.iter().filter(|s|s.later_evidence_ref.is_none()).count(),1);
    }

    #[test]
    fn knowledge_links_keep_each_branches_original_expression() {
        let root=tempfile::tempdir().unwrap();let mut store=LearningStore::open(&root.path().join("learning.db"),false).unwrap();
        let observation=|entry:&str,quote:&str|json!({"knowledge_links":[{"reference":{"source_id":"book","source_revision":"v1","publication_id":null,"artifact":"pass1","entry_id":entry},"quote":quote}]});
        let first=hypothesis(&mut store,"a","2026-11-01T10:00:00+08:00","停车也算时间",observation("claim:time","停车也算时间"));
        let second=hypothesis(&mut store,"b","2026-11-01T11:00:00+08:00","回程也算路程",observation("claim:distance","回程也算路程"));
        hypothesis(&mut store,"c","2026-11-01T12:00:00+08:00","我还想继续看看",json!({"unconfirmed":["尚未表达进一步理解"]}));
        let summaries=store.understanding_summaries("book","v1").unwrap();
        assert_eq!(summaries.len(),3);
        assert_eq!(summaries.iter().find(|s|s.evidence_ref==first.evidence_id).unwrap().learner_quote,"停车也算时间");
        assert_eq!(summaries.iter().find(|s|s.evidence_ref==second.evidence_id).unwrap().knowledge_links[0].reference.entry_id,"claim:distance");
        assert_eq!(store.understanding_timeline(&second.evidence_id,0,20).unwrap().len(),3);
    }
    #[test]
    fn current_understanding_uses_expression_order_keeps_history_and_scopes_corrections() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("learning.db");
        let mut store = LearningStore::open(&path, false)
            .unwrap()
            .with_workspace(Some("a"));
        let newer = hypothesis(
            &mut store,
            "new",
            "2026-11-01T10:00:00+08:00",
            "停车仍在行程内，计入总时间",
            json!({"demonstrated":["能解释停车为何计入总时间"],"unconfirmed":["新的情境尚未尝试"]}),
        );
        let older = hypothesis(
            &mut store,
            "old",
            "1793458800",
            "停车不算时间吧？",
            json!({"unconfirmed":["用户在求证停车是否计时，尚不能认定误解"]}),
        );
        assert_eq!(
            newer.target.as_ref().unwrap().focus_ref,
            older.target.as_ref().unwrap().focus_ref
        );
        let summaries = store.understanding_summaries("book", "v1").unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].evidence_ref, newer.evidence_id);
        assert!(summaries[0].difficulties.is_empty());
        assert_eq!(
            store
                .understanding_timeline(&newer.evidence_id, 0, 20)
                .unwrap()
                .len(),
            2
        );
        let correction = store
            .correct_evidence(&newer.evidence_id, "fix", "我参考了笔记才答出来")
            .unwrap();
        assert!(correction.corrected_at.is_some());
        assert_eq!(
            store
                .correct_evidence(&newer.evidence_id, "fix", "我参考了笔记才答出来")
                .unwrap(),
            correction
        );
        let current = store.understanding_summaries("book", "v1").unwrap();
        assert!(current[0].demonstrated.is_empty());
        assert_eq!(
            current[0].correction.as_deref(),
            Some("我参考了笔记才答出来")
        );
        assert_eq!(store.note_evidence("note-a").unwrap().len(), 3);
        assert!(LearningStore::open(&path, false)
            .unwrap()
            .with_workspace(Some("b"))
            .understanding_summaries("book", "v1")
            .unwrap()
            .is_empty());
        assert!(store
            .understanding_summaries("book", "v2")
            .unwrap()
            .is_empty());
        let mut changed = target("速度等于距离除以运动时间", "explanation");
        changed.focus_ref = newer.target.unwrap().focus_ref;
        assert!(store
            .bind_focus(&mut changed, &binding("changed"), "focus:changed")
            .is_err());
    }
    #[test]
    fn rubric_attribution_does_not_turn_a_calculation_error_into_a_concept_error() {
        let root = tempfile::tempdir().unwrap();
        let mut store = LearningStore::open(&root.path().join("learning.db"), false).unwrap();
        let b = binding("practice");
        let mut concept = target("全程时间包含停车时间", "explanation");
        store.bind_focus(&mut concept, &b, "focus:time").unwrap();
        let mut calculation = target("速度比值计算", "calculation");
        store
            .bind_focus(&mut calculation, &b, "focus:calculate")
            .unwrap();
        let note = json!({"note_id":"idea","revision":2});
        let selected = TeachingEvent {
            event_id: "selected".into(),
            binding: b.clone(),
            kind: TeachingFact::MoveSelected,
            causal_refs: vec![],
            occurred_at: "1793498400".into(),
            payload: json!({"move":{"target":concept,"capability":"explanation","note_refs":[note],"assessment":{"rule":{"kind":"open","items":[
            {"id":"time","criterion":"包括停车","target":concept,"source_lids":["1.1"],"note_refs":[note]},
            {"id":"number","criterion":"计算比值","target":calculation,"source_lids":["1.1"]},
            {"id":"unit","criterion":"写出单位","source_lids":["1.1"]}]},"source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"}}}),
        };
        store.append_teaching(&selected).unwrap();
        let mut delivery = selected.clone();
        delivery.event_id = "delivery".into();
        delivery.kind = TeachingFact::MessageDelivered;
        delivery.payload["assessment_contract_ref"] = json!("selected");
        delivery.payload["object_versions"] = json!([]);
        store.append_teaching(&delivery).unwrap();
        let action = TeachingEvent {
            event_id: "answer".into(),
            binding: b.clone(),
            kind: TeachingFact::LearnerAction,
            causal_refs: vec!["delivery".into()],
            occurred_at: "1793498460".into(),
            payload: json!({"action":"submit","delivery_ref":"delivery","response":"总时间含停车是 15 分钟，但 10 除以 0.25 等于 25","assistance_refs":[],"attempt":1}),
        };
        store.append_teaching(&action).unwrap();
        store.save_assessment(&serde_json::from_value::<ResponseAssessment>(json!({"action_ref":"answer","contract_ref":"selected","evaluator_version":"test","status":"partial","items":[
            {"id":"time","verdict":"supported","response_quote":"总时间含停车是 15 分钟","sources":[],"reason":"时间范围正确"},
            {"id":"number","verdict":"unsupported","response_quote":"10 除以 0.25 等于 25","sources":[],"reason":"比值应为 40"},
            {"id":"unit","verdict":"unsupported","response_quote":"等于 25","sources":[],"reason":"未写出速度单位"}]})).unwrap()).unwrap();
        store.derive_assessed_evidence("stage").unwrap();
        assert_eq!(
            store
                .evidence("assessment:answer:criterion:time")
                .unwrap()
                .status,
            AssessmentStatus::Correct
        );
        assert_eq!(
            store
                .evidence("assessment:answer:criterion:number")
                .unwrap()
                .status,
            AssessmentStatus::Incorrect
        );
        let summaries = store.understanding_summaries("book", "v1").unwrap();
        assert_eq!(summaries.len(), 2);
        assert!(summaries
            .iter()
            .find(|s| s.capability == "explanation")
            .unwrap()
            .difficulties
            .is_empty());
        // The after-turn observer can also interpret this same practice response.
        // A questionable task must withdraw both forms of dependent judgment.
        let mut observed = store.evidence("assessment:answer:criterion:time").unwrap();
        observed.evidence_id = "observed-answer".into();
        observed.nature = EvidenceNature::Hypothesis;
        observed.delivery_ref = None;
        observed.assessment_ref = None;
        observed.criterion_refs.clear();
        observed.status = AssessmentStatus::Unassessed;
        observed.fact_refs = vec![action.event_id.clone()];
        observed.understanding.demonstrated = vec!["本次表达把停车纳入总时间".into()];
        store.append_evidence(&observed).unwrap();
        store
            .append_teaching(&TeachingEvent {
                event_id: "check".into(),
                binding: b,
                kind: TeachingFact::PracticeChecked,
                causal_refs: vec!["delivery".into()],
                occurred_at: "1793498500".into(),
                payload: json!({"delivery_ref":"delivery","verdict":"questionable"}),
            })
            .unwrap();
        assert!(store
            .understanding_summaries("book", "v1")
            .unwrap()
            .is_empty());
        assert_eq!(store.note_evidence("idea").unwrap().len(), 3);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoteEvidenceLink {
    pub reference: NoteRef,
    /// Exact part of this saved version whose relationship is interpreted.
    pub quote: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnderstandingSummary {
    pub learner_summary: Option<LearnerUnderstandingSummary>,
    pub previous: Option<PreviousUnderstanding>,
    /// A newer expression about the same relationship and capability exists.
    /// This branch keeps its original quote, without presenting it as current.
    pub later_evidence_ref: Option<String>,
    #[serde(default)]
    pub knowledge_links: Vec<read_tools::knowledge::KnowledgeLink>,
    pub focus_ref: String,
    pub source_id: String,
    pub source_revision: String,
    pub workspace_id: Option<String>,
    pub label: String,
    pub capability: String,
    pub target: Option<TeachingTarget>,
    pub demonstrated: Vec<String>,
    pub difficulties: Vec<String>,
    pub unconfirmed: Vec<String>,
    pub evidence_ref: String,
    pub evidence_refs: Vec<String>,
    pub note_refs: Vec<NoteRef>,
    pub next_step: String,
    pub occurred_at: Option<String>,
    pub source_session_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub source_session_title: Option<String>,
    pub assistance: String,
    pub nature: EvidenceNature,
    pub learner_quote: String,
    pub correction: Option<String>,
    pub interpreter_version: String,
}

pub fn relationship_key(e: &LearningEvidence) -> String {
    if let (Some(id), Some(version)) = (&e.object_id, e.object_revision) {
        return format!("object:{id}:{version}");
    }
    if let Some(target) = &e.target {
        if target.object_refs.len() == 1 {
            let object = &target.object_refs[0];
            return format!("object:{}:{}", object.object_id, object.object_revision);
        }
        if let Some(id) = &target.focus_ref {
            return id.clone();
        }
    }
    // Old records without an accepted relationship stay individually attributable.
    e.supersedes
        .clone()
        .unwrap_or_else(|| e.evidence_id.clone())
}

pub(crate) struct AssessedGroup {
    pub suffix: String,
    pub target: Option<TeachingTarget>,
    pub object_id: Option<String>,
    pub object_revision: Option<u64>,
    pub label: String,
    pub capability: String,
    pub criteria: Vec<String>,
    pub notes: Vec<NoteRef>,
    pub status: AssessmentStatus,
    pub items: Value,
}

pub(crate) fn assessed_groups(
    store: &LearningStore,
    delivery: &crate::teaching::TeachingEvent,
    assessment: Option<&crate::assessment::ResponseAssessment>,
) -> Result<Vec<AssessedGroup>, ToolError> {
    use crate::assessment::{AssessmentContract, ItemVerdict, ScoringRule};
    let movement = &delivery.payload["move"];
    let target: Option<TeachingTarget> =
        serde_json::from_value(movement.get("target").cloned().unwrap_or(Value::Null))
            .map_err(storage)?;
    let notes: Vec<NoteRef> =
        serde_json::from_value(movement.get("note_refs").cloned().unwrap_or(json!([])))
            .map_err(storage)?;
    let contract = delivery.payload["assessment_contract_ref"]
        .as_str()
        .map(|id| store.teaching_event(id))
        .transpose()?
        .map(|event| {
            serde_json::from_value::<AssessmentContract>(
                event.payload["move"]["assessment"].clone(),
            )
        })
        .transpose()
        .map_err(storage)?;
    let mut groups: BTreeMap<String, AssessedGroup> = BTreeMap::new();
    if let (
        Some(AssessmentContract {
            rule: ScoringRule::Open { items },
            ..
        }),
        Some(assessment),
    ) = (contract, assessment)
    {
        for criterion in items {
            let Some(target) = criterion.target else {
                continue;
            };
            let object = target
                .object_refs
                .first()
                .filter(|_| target.object_refs.len() == 1)
                .cloned();
            let key = format!(
                "{}:{}",
                target.focus_ref.as_deref().unwrap_or(&criterion.id),
                target.capability
            );
            let group = groups.entry(key).or_insert_with(|| AssessedGroup {
                suffix: format!("criterion:{}", criterion.id),
                object_id: object.as_ref().map(|o| o.object_id.clone()),
                object_revision: object.map(|o| o.object_revision),
                label: target.learning_focus.clone(),
                capability: target.capability.clone(),
                target: Some(target),
                criteria: vec![],
                notes: vec![],
                status: AssessmentStatus::Uncertain,
                items: json!([]),
            });
            group.criteria.push(criterion.id.clone());
            let attributed = if criterion.note_refs.is_empty() && notes.len() == 1 {
                notes.clone()
            } else {
                criterion.note_refs
            };
            for note in attributed {
                if !group.notes.contains(&note) {
                    group.notes.push(note)
                }
            }
            if let Some(item) = assessment.items.iter().find(|item| item.id == criterion.id) {
                group.items.as_array_mut().unwrap().push(json!(item));
            }
        }
        for group in groups.values_mut() {
            let verdicts: Vec<_> = group
                .items
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|item| {
                    serde_json::from_value::<ItemVerdict>(item["verdict"].clone()).ok()
                })
                .collect();
            group.status = if assessment.status == AssessmentStatus::Uncertain
                || verdicts.len() != group.criteria.len()
                || verdicts.contains(&ItemVerdict::Uncertain)
            {
                AssessmentStatus::Uncertain
            } else if verdicts.iter().all(|v| *v == ItemVerdict::Supported) {
                AssessmentStatus::Correct
            } else if verdicts.iter().all(|v| *v == ItemVerdict::Unsupported) {
                AssessmentStatus::Incorrect
            } else {
                AssessmentStatus::Partial
            };
        }
    }
    // Whole-task results stay in ResponseAssessment. Explicitly attributed
    // criteria must not be overwritten by a broader aggregate about this task.
    if groups.is_empty() {
        let objects = delivery.payload["object_versions"].as_array();
        let object = objects
            .filter(|objects| objects.len() == 1 && groups.is_empty())
            .and_then(|o| o.first());
        let object_id = object
            .and_then(|o| o["ref"]["object_id"].as_str())
            .map(str::to_owned);
        groups.insert(
            "aggregate".into(),
            AssessedGroup {
                suffix: object_id.clone().unwrap_or_else(|| "source".into()),
                object_id,
                object_revision: object.and_then(|o| o["object_revision"].as_u64()),
                label: object
                    .and_then(|o| o["meaning"].as_str())
                    .or_else(|| target.as_ref().map(|t| t.learning_focus.as_str()))
                    .unwrap_or("综合活动表现")
                    .into(),
                capability: movement["capability"].as_str().unwrap_or_default().into(),
                target,
                criteria: vec![],
                notes: if notes.len() == 1 { notes } else { vec![] },
                status: assessment
                    .map(|a| a.status.clone())
                    .unwrap_or(AssessmentStatus::Uncertain),
                items: json!(assessment.map(|a| &a.items)),
            },
        );
    }
    Ok(groups.into_values().collect())
}

impl LearningStore {
    pub fn note_evidence(
        &self,
        note_id: &str,
    ) -> Result<Vec<(Option<String>, LearningEvidence)>, ToolError> {
        let mut q=self.connection.prepare("SELECT DISTINCT e.learning_workspace_id,e.evidence FROM understanding_note_evidence n JOIN learning_evidence e ON e.learning_workspace_id=n.learning_workspace_id AND e.evidence_id=n.evidence_id WHERE n.note_id=? ORDER BY e.seq").map_err(storage)?;
        let rows = q
            .query_map([note_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(storage)?;
        rows.map(|r| {
            let (workspace, data) = r.map_err(storage)?;
            Ok((
                (!workspace.is_empty()).then_some(workspace),
                serde_json::from_str(&data).map_err(storage)?,
            ))
        })
        .collect()
    }
    pub fn note_deliveries(
        &self,
        note_id: &str,
    ) -> Result<Vec<(Option<String>, crate::teaching::TeachingEvent)>, ToolError> {
        let mut q=self.connection.prepare("SELECT learning_workspace_id,event FROM teaching_trace WHERE json_extract(event,'$.kind')='message_delivered' AND EXISTS(SELECT 1 FROM json_each(event,'$.payload.move.note_refs') n WHERE json_extract(n.value,'$.note_id')=?) ORDER BY seq").map_err(storage)?;
        let rows = q
            .query_map([note_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(storage)?;
        rows.map(|r| {
            let (workspace, data) = r.map_err(storage)?;
            Ok((
                (!workspace.is_empty()).then_some(workspace),
                serde_json::from_str(&data).map_err(storage)?,
            ))
        })
        .collect()
    }
    pub fn focus_candidates(
        &self,
        source: &str,
        revision: &str,
        limit: u32,
    ) -> Result<Vec<TeachingTarget>, ToolError> {
        let mut q = self.connection.prepare("SELECT target FROM understanding_focus WHERE learning_workspace_id=? AND source_id=? AND source_revision=? ORDER BY rowid DESC LIMIT ?").map_err(storage)?;
        let rows = q
            .query_map(
                params![self.workspace_scope, source, revision, limit.min(24)],
                |r| r.get::<_, String>(0),
            )
            .map_err(storage)?;
        rows.map(|r| serde_json::from_str(&r.map_err(storage)?).map_err(storage))
            .collect()
    }

    /// An exact definition can be resumed without guessing semantic equivalence.
    /// Rephrased definitions use an explicitly offered focus_ref or form a new focus.
    pub fn bind_focus(
        &self,
        target: &mut TeachingTarget,
        binding: &TeachingBinding,
        identity: &str,
    ) -> Result<(), ToolError> {
        target.validate(binding)?;
        if let Some(id) = &target.focus_ref {
            let previous: Option<String> = self.connection.query_row("SELECT target FROM understanding_focus WHERE learning_workspace_id=? AND focus_ref=? AND source_id=? AND source_revision=?",
                params![self.workspace_scope,id,binding.source_id,binding.source_revision], |r|r.get(0)).optional().map_err(storage)?;
            if previous.is_none() && id == identity {
                return Ok(());
            }
            let previous: TeachingTarget = serde_json::from_str(
                &previous.ok_or_else(|| invalid("关系引用不属于当前学习范围和材料版本"))?,
            )
            .map_err(storage)?;
            if previous.learning_focus != target.learning_focus
                || previous.object_refs != target.object_refs
            {
                return Err(invalid(
                    "沿用关系时请保留候选的准确关系陈述与对象；关系变化应省略 focus_ref 建立新关系",
                ));
            }
            return Ok(());
        }
        let mut q = self.connection.prepare("SELECT focus_ref,target FROM understanding_focus WHERE learning_workspace_id=? AND source_id=? AND source_revision=? AND json_extract(target,'$.learning_focus')=?").map_err(storage)?;
        let rows = q
            .query_map(
                params![
                    self.workspace_scope,
                    binding.source_id,
                    binding.source_revision,
                    target.learning_focus
                ],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .map_err(storage)?;
        for row in rows {
            let (id, data) = row.map_err(storage)?;
            let saved: TeachingTarget = serde_json::from_str(&data).map_err(storage)?;
            if saved.source_bindings == target.source_bindings
                && saved.media_bindings == target.media_bindings
                && saved.music_bindings == target.music_bindings
                && saved.object_refs == target.object_refs
            {
                target.focus_ref = Some(id);
                return Ok(());
            }
        }
        target.focus_ref = Some(identity.to_owned());
        Ok(())
    }

    pub fn remember_focus(
        &self,
        target: &TeachingTarget,
        binding: &TeachingBinding,
    ) -> Result<(), ToolError> {
        if target.focus_ref.is_none() {
            return Ok(());
        }
        self.connection.execute("INSERT OR IGNORE INTO understanding_focus(learning_workspace_id,focus_ref,source_id,source_revision,target) VALUES(?,?,?,?,?)",
            params![self.workspace_scope,target.focus_ref,binding.source_id,binding.source_revision,serde_json::to_string(target).map_err(storage)?]).map_err(storage)?;
        Ok(())
    }

    pub fn evidence_for_expression(
        &self,
        session: &str,
        turn: &str,
    ) -> Result<Vec<LearningEvidence>, ToolError> {
        let mut q=self.connection.prepare("SELECT DISTINCT e.evidence FROM teaching_trace t JOIN learning_evidence e ON e.learning_workspace_id=t.learning_workspace_id AND (json_extract(e.evidence,'$.action_ref')=t.event_id OR t.event_id IN (SELECT value FROM json_each(e.evidence,'$.fact_refs'))) WHERE t.learning_workspace_id=? AND json_extract(t.event,'$.binding.chat_session_id')=? AND json_extract(t.event,'$.binding.turn_id')=? ORDER BY e.seq").map_err(storage)?;
        let rows = q
            .query_map(params![self.workspace_scope, session, turn], |r| {
                r.get::<_, String>(0)
            })
            .map_err(storage)?;
        rows.map(|r| serde_json::from_str(&r.map_err(storage)?).map_err(storage))
            .collect()
    }

    pub fn evidence_is_questionable(&self, e: &LearningEvidence) -> Result<bool, ToolError> {
        let mut deliveries = e.delivery_ref.iter().cloned().collect::<Vec<_>>();
        if e.nature == EvidenceNature::Hypothesis {
            for (_, fact) in self.evidence_expressions(e)? {
                if let Some(id) = fact.payload["delivery_ref"].as_str() {
                    deliveries.push(id.into());
                }
                if let Ok(response) =
                    self.teaching_event(&format!("response:{}", fact.binding.turn_id))
                {
                    if response.binding == fact.binding {
                        if let Some(id) = response.payload["delivery_ref"].as_str() {
                            deliveries.push(id.into());
                        }
                    }
                }
            }
        }
        if deliveries.is_empty() {
            return Ok(false);
        }
        self.connection.query_row("SELECT EXISTS(SELECT 1 FROM teaching_trace WHERE learning_workspace_id=? AND json_extract(event,'$.kind')='practice_checked' AND json_extract(event,'$.payload.delivery_ref') IN (SELECT value FROM json_each(?)) AND json_extract(event,'$.payload.verdict')='questionable')",params![self.workspace_scope,serde_json::to_string(&deliveries).map_err(storage)?],|r|r.get(0)).map_err(storage)
    }

    /// Shared read projection for notes and the understanding overview. Order by
    /// original expression time, including late coach jobs and explicit corrections.
    pub fn understanding_summaries(
        &self,
        source: &str,
        revision: &str,
    ) -> Result<Vec<UnderstandingSummary>, ToolError> {
        self.understanding_summaries_for(source, revision, &[])
    }
    pub fn understanding_summaries_for(
        &self,
        source: &str,
        revision: &str,
        focuses: &[String],
    ) -> Result<Vec<UnderstandingSummary>, ToolError> {
        let mut groups: BTreeMap<(String, String, String), UnderstandingSummary> = BTreeMap::new();
        let mut latest: BTreeMap<(String, String), String> = BTreeMap::new();
        let mut order = Vec::new();
        for e in self.understanding_evidence(source, revision, focuses)? {
            if self.evidence_is_questionable(&e)? {
                continue;
            }
            if e.nature == EvidenceNature::Performance
                && e.criterion_refs.is_empty()
                && e.object_id.is_some()
            {
                if let Some(delivery) = &e.delivery_ref {
                    if self.teaching_event(delivery)?.payload["object_versions"]
                        .as_array()
                        .is_some_and(|objects| objects.len() > 1)
                    {
                        continue;
                    }
                }
            }
            let key = relationship_key(&e);
            let relationship=(key.clone(),e.capability.clone());
            let later_evidence_ref=latest.get(&relationship).cloned();
            latest.entry(relationship).or_insert_with(||e.evidence_id.clone());
            // A later observation about another branch must not relocate the old
            // expression, or replace its quote with an unrelated latest answer.
            let mut references:Vec<_>=e.understanding.knowledge_links.iter().map(|l|json!(l.reference).to_string()).collect();
            references.sort();references.dedup();
            let group_key=(key.clone(),e.capability.clone(),json!(references).to_string());
            if let Some(current) = groups.get_mut(&group_key) {
                if current.previous.is_none() && current.correction.is_none()
                    && e.correction.is_none() && !e.learner_quote.is_empty()
                    && current.learner_quote != e.learner_quote {
                    current.previous = Some(PreviousUnderstanding {
                        evidence_ref: e.evidence_id.clone(), learner_quote: e.learner_quote.clone(),
                        assistance: assistance_label(&e),
                    });
                }
                // All original versions remain navigable even when later evidence
                // about the same relationship was produced from another note.
                for reference in e.note_refs {
                    if !current.note_refs.contains(&reference) {
                        current.note_refs.push(reference)
                    }
                }
                continue;
            }
            let metadata = self.evidence_chronology(&e.evidence_id)?;
            let assisted = !e.assistance_refs.is_empty() || !e.support_delivery_refs.is_empty();
            let assistance = assistance_label(&e);
            let mut observation = e.understanding.clone();
            if e.correction.is_some() {
                observation = UnderstandingObservation {
                    unconfirmed: vec!["原判断已撤回，依据用户补充重新确认".into()],
                    ..Default::default()
                };
            } else if e.nature == EvidenceNature::Performance {
                observation = UnderstandingObservation::default();
                match e.status {
                    AssessmentStatus::Correct => observation
                        .demonstrated
                        .push(format!("{assistance}，本次回答满足这一目标的标准")),
                    AssessmentStatus::Partial => observation
                        .difficulties
                        .push("本次回答只满足这一目标的部分标准".into()),
                    AssessmentStatus::Incorrect => observation
                        .difficulties
                        .push("本次回答尚未满足这一目标的标准".into()),
                    _ => observation
                        .unconfirmed
                        .push("本次表现还不足以作出判断".into()),
                }
                if assisted {
                    observation
                        .unconfirmed
                        .push("尚需确认不借助这些帮助时能否运用".into())
                }
            } else if observation.demonstrated.is_empty()
                && observation.difficulties.is_empty()
                && observation.unconfirmed.is_empty()
            {
                observation.unconfirmed.push(e.interpretation.clone());
            }
            order.push(group_key.clone());
            groups.insert(
                group_key,
                UnderstandingSummary {
                    learner_summary: observation.learner_summary.clone(),
                    previous: None,
                    later_evidence_ref,
                    knowledge_links: e.understanding.knowledge_links,
                    focus_ref: key,
                    source_id: e.source_id,
                    source_revision: e.source_revision,
                    workspace_id: self.learning_workspace_id().map(str::to_owned),
                    label: e.label,
                    capability: e.capability,
                    target: e.target,
                    demonstrated: observation.demonstrated,
                    difficulties: observation.difficulties,
                    unconfirmed: observation.unconfirmed,
                    evidence_ref: e.evidence_id.clone(),
                    evidence_refs: vec![e.evidence_id],
                    note_refs: e.note_refs,
                    next_step: observation.learner_summary.as_ref().and_then(|s|s.next_step.clone()).unwrap_or_default(),
                    occurred_at: metadata["occurred_at"].as_str().map(str::to_owned),
                    source_session_id: metadata["source_session_id"].as_str().map(str::to_owned),
                    source_turn_id: metadata["source_turn_id"].as_str().map(str::to_owned),
                    source_session_title: None,
                    assistance,
                    nature: e.nature,
                    learner_quote: e.learner_quote,
                    correction: e.correction,
                    interpreter_version: e.interpreter_version,
                },
            );
        }
        Ok(order
            .into_iter()
            .filter_map(|key| groups.remove(&key))
            .collect())
    }

    pub fn understanding_scope(&self, source: &str, revision: &str) -> Value {
        json!({"workspace_id":self.learning_workspace_id(),"source_id":source,"source_revision":revision})
    }
}
