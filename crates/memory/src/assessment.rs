//! Frozen activity criteria and response-level judgments; never learner mastery.
use crate::{
    learning::{storage, LearningStore},
    teaching::invalid,
};
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RubricItem {
    pub id: String,
    /// Includes necessary conditions, accepted equivalents and permitted omissions.
    pub criterion: String,
    /// A solution-free prompt the learner may see while revising this item.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_prompt: Option<String>,
    /// The single relationship/capability this criterion can actually distinguish.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<crate::teaching::TeachingTarget>,
    /// Exact versions used by this criterion; omitted for an unattributed result.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note_refs: Vec<crate::NoteRef>,
    #[serde(default)]
    pub source_lids: Vec<String>,
    /// Current-turn source.present IDs for prepared music or observed video.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_ref_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScoringRule {
    /// Canonical choice tokens, submitted as a JSON array (or a single token).
    ChoiceSet {
        choices: Vec<String>,
        correct: Vec<String>,
    },
    Open {
        items: Vec<RubricItem>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssessmentContract {
    pub rule: ScoringRule,
    #[serde(default)]
    pub source_lids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_ref_ids: Vec<String>,
    /// An explicitly incomplete source cannot justify a correctness judgment.
    pub source_sufficient: bool,
    /// Criterion-level feedback is private until the learner explicitly reveals it.
    pub feedback: FeedbackPolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackPolicy {
    AfterSubmit,
    OnReveal,
}

impl AssessmentContract {
    pub fn validate(&self) -> Result<(), ToolError> {
        if self.source_lids.is_empty() && self.source_ref_ids.is_empty() {
            return Err(invalid("评分合同缺少来源"));
        }
        match &self.rule {
            ScoringRule::ChoiceSet { choices, correct } => {
                let normalized: BTreeSet<_> = choices.iter().map(|s| normalize(s)).collect();
                if choices.is_empty()
                    || normalized.len() != choices.len()
                    || normalized.contains("")
                    || correct.is_empty()
                    || correct.iter().any(|s| !normalized.contains(&normalize(s)))
                    || correct
                        .iter()
                        .map(|s| normalize(s))
                        .collect::<BTreeSet<_>>()
                        .len()
                        != correct.len()
                {
                    return Err(invalid("选项或答案集合无效"));
                }
            }
            ScoringRule::Open { items } => {
                if items.is_empty()
                    || items.len() > 12
                    || items.iter().map(|i| &i.id).collect::<BTreeSet<_>>().len() != items.len()
                    || items.iter().any(|i| {
                        i.id.trim().is_empty()
                            || i.criterion.trim().is_empty()
                            || (i.source_lids.is_empty() && i.source_ref_ids.is_empty())
                            || i.source_lids
                                .iter()
                                .any(|lid| !self.source_lids.contains(lid))
                            || i.source_ref_ids.iter().any(|id| !self.source_ref_ids.contains(id))
                    })
                {
                    return Err(invalid("原子评分标准或来源不完整"));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentStatus {
    Correct,
    Partial,
    Incorrect,
    Uncertain,
    Unassessed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemVerdict {
    Supported,
    Unsupported,
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceQuote {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub lid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref_id: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub quote: String,
    /// A visible feature in an attached frozen frame/crop, never a text quotation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<String>,
}

/// Host-frozen evidence for a media rubric; kept private with the selected move.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssessmentSource {
    pub source_id: String,
    pub source_revision: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual: Option<AssessmentVisual>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssessmentVisual {
    pub target: base_schema::EvidenceTarget,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemAssessment {
    pub id: String,
    pub verdict: ItemVerdict,
    /// Exact learner text. Unsupported missing conditions may use an empty quote.
    pub response_quote: String,
    pub sources: Vec<SourceQuote>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseAssessment {
    pub action_ref: String,
    pub contract_ref: String,
    pub evaluator_version: String,
    pub status: AssessmentStatus,
    pub items: Vec<ItemAssessment>,
}

pub(crate) fn normalize(value: &str) -> String {
    value.trim().to_lowercase()
}

pub fn assess_closed(
    contract: &AssessmentContract,
    response: &str,
) -> Result<AssessmentStatus, ToolError> {
    if !contract.source_sufficient {
        return Ok(AssessmentStatus::Uncertain);
    }
    let ScoringRule::ChoiceSet { choices, correct } = &contract.rule else {
        return Err(invalid("开放回答需要逐项评估"));
    };
    let selected: Vec<String> = if response.trim().starts_with('[') {
        serde_json::from_str(response).map_err(|_| invalid("请提交选项数组"))?
    } else {
        vec![response.into()]
    };
    let selected: BTreeSet<_> = selected.iter().map(|s| normalize(s)).collect();
    let choices: BTreeSet<_> = choices.iter().map(|s| normalize(s)).collect();
    let correct: BTreeSet<_> = correct.iter().map(|s| normalize(s)).collect();
    if selected.is_empty() || !selected.is_subset(&choices) {
        return Ok(AssessmentStatus::Uncertain);
    }
    Ok(if selected == correct {
        AssessmentStatus::Correct
    } else if selected.is_subset(&correct) {
        AssessmentStatus::Partial
    } else {
        AssessmentStatus::Incorrect
    })
}

/// Reject incomplete/forged reports; semantic uncertainty is a valid assessment.
pub fn accept_open(
    contract: &AssessmentContract,
    response: &str,
    sources: &BTreeMap<String, String>,
    source_references: &BTreeMap<String, AssessmentSource>,
    items: &[ItemAssessment],
) -> Result<AssessmentStatus, ToolError> {
    let ScoringRule::Open { items: rubric } = &contract.rule else {
        return Err(invalid("并非开放题合同"));
    };
    if items.len() != rubric.len()
        || items.iter().map(|i| &i.id).collect::<BTreeSet<_>>().len() != items.len()
    {
        return Err(invalid("评估结果缺项或重复"));
    }
    for item in items {
        let criterion = rubric
            .iter()
            .find(|r| r.id == item.id)
            .ok_or_else(|| invalid("评估引用未知标准"))?;
        if item.reason.trim().is_empty()
            || !response.contains(&item.response_quote)
            || (item.verdict == ItemVerdict::Supported && item.response_quote.trim().is_empty())
            || (item.verdict != ItemVerdict::Uncertain && item.sources.is_empty())
            || item.sources.iter().any(|s| !match &s.source_ref_id {
                Some(id) if s.lid.is_empty() && criterion.source_ref_ids.contains(id) =>
                    source_references.get(id).is_some_and(|source| match &s.observation {
                        Some(observation) => source.visual.is_some() && s.quote.is_empty() && !observation.trim().is_empty(),
                        None => !s.quote.trim().is_empty() && source.text.contains(&s.quote),
                    }),
                None => s.observation.is_none() && !s.quote.trim().is_empty()
                    && criterion.source_lids.contains(&s.lid)
                    && sources.get(&s.lid).is_some_and(|text| text.contains(&s.quote)),
                _ => false,
            })
        {
            return Err(invalid("评估缺少回答片段或引用不属于冻结来源"));
        }
    }
    Ok(
        if !contract.source_sufficient || items.iter().any(|i| i.verdict == ItemVerdict::Uncertain)
        {
            AssessmentStatus::Uncertain
        } else if items.iter().all(|i| i.verdict == ItemVerdict::Supported) {
            AssessmentStatus::Correct
        } else if items.iter().any(|i| i.verdict == ItemVerdict::Supported) {
            AssessmentStatus::Partial
        } else {
            AssessmentStatus::Incorrect
        },
    )
}

impl LearningStore {
    pub fn assessment(&self, action: &str) -> Result<Option<ResponseAssessment>, ToolError> {
        let saved: Option<String> = self
            .connection
            .query_row(
                "SELECT assessment FROM teaching_assessments WHERE learning_workspace_id=? AND action_ref=?",
                params![self.workspace_scope,action],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        saved
            .map(|s| serde_json::from_str(&s).map_err(storage))
            .transpose()
    }
    pub fn save_assessment(
        &mut self,
        assessment: &ResponseAssessment,
    ) -> Result<ResponseAssessment, ToolError> {
        if let Some(old) = self.assessment(&assessment.action_ref)? {
            if old != *assessment {
                return Err(invalid("已接纳判定不能覆写"));
            }
            return Ok(old);
        }
        self.teaching_event(&assessment.action_ref)?;
        self.teaching_event(&assessment.contract_ref)?;
        self.connection
            .execute(
                "INSERT INTO teaching_assessments(learning_workspace_id,action_ref,assessment) VALUES (?,?,?)",
                params![
                    self.workspace_scope,
                    assessment.action_ref,
                    serde_json::to_string(assessment).map_err(storage)?
                ],
            )
            .map_err(storage)?;
        Ok(assessment.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn media_assessment_contract_accepts_real_refs_without_text_lids_and_rejects_foreign_refs() {
        let mut contract: AssessmentContract = serde_json::from_value(json!({
            "rule":{"kind":"open","items":[{"id":"return","criterion":"Identifies the repeated theme", "source_ref_ids":["read-music"]}]},
            "source_ref_ids":["read-music"],"source_sufficient":true,"feedback":"after_submit"
        })).unwrap();
        contract.validate().unwrap();
        assert!(contract.source_lids.is_empty());
        let refs = BTreeMap::from([("read-music".into(), AssessmentSource {
            source_id:"music".into(),source_revision:"1".into(),
            text:"The opening theme returns in the same register.".into(),visual:None,
        })]);
        let response = "The opening theme returns";
        let mut items: Vec<ItemAssessment> = serde_json::from_value(json!([{
            "id":"return","verdict":"supported","response_quote":response,
            "sources":[{"source_ref_id":"read-music","quote":"opening theme returns"}],
            "reason":"Matches the frozen score analysis"
        }])).unwrap();
        assert_eq!(accept_open(&contract,response,&BTreeMap::new(),&refs,&items).unwrap(),AssessmentStatus::Correct);
        items[0].sources[0].source_ref_id=Some("other-read".into());
        assert!(accept_open(&contract,response,&BTreeMap::new(),&refs,&items).is_err());
        items[0].sources[0].source_ref_id=Some("read-music".into());
        items[0].sources[0].quote="invented analysis".into();
        assert!(accept_open(&contract,response,&BTreeMap::new(),&refs,&items).is_err());
        let ScoringRule::Open { items } = &mut contract.rule else { unreachable!() };
        items[0].source_ref_ids=vec!["other-read".into()];
        assert!(contract.validate().is_err());
    }

    #[test]
    fn media_assessment_visual_observations_require_frozen_visual_sources() {
        let contract: AssessmentContract = serde_json::from_value(json!({
            "rule":{"kind":"open","items":[{"id":"shape","criterion":"Compares visible slopes","source_ref_ids":["read-frame"]}]},
            "source_ref_ids":["read-frame"],"source_sufficient":true,"feedback":"after_submit"
        })).unwrap();
        contract.validate().unwrap();
        let mut refs = BTreeMap::from([("read-frame".into(),AssessmentSource {
            source_id:"video".into(),source_revision:"1".into(),text:String::new(),
            visual:Some(serde_json::from_value(json!({"target":{"kind":"visual",
                "publication":{"book_id":"video","publication_id":"original"},
                "frames":[{"video":{"media_id":"v","media_revision":"1"},"actual_pts_ms":1000,"width":640,"height":480}],"regions":[]},"duration_ms":2000})).unwrap()),
        })]);
        let items:Vec<ItemAssessment>=serde_json::from_value(json!([{"id":"shape","verdict":"supported",
            "response_quote":"A is steeper","sources":[{"source_ref_id":"read-frame","observation":"Line A rises faster than line B"}],"reason":"Matches the frame"}])).unwrap();
        assert_eq!(accept_open(&contract,"A is steeper",&BTreeMap::new(),&refs,&items).unwrap(),AssessmentStatus::Correct);
        refs.get_mut("read-frame").unwrap().visual=None;
        assert!(accept_open(&contract,"A is steeper",&BTreeMap::new(),&refs,&items).is_err(),"a locator or prepared music analysis does not supply visual pixels");
    }
    #[test]
    fn frozen_choice_rules_distinguish_equivalence_partial_error_and_uncertainty() {
        let mut c = AssessmentContract {
            rule: ScoringRule::ChoiceSet {
                choices: vec!["A".into(), "B".into(), "C".into()],
                correct: vec!["A".into(), "B".into()],
            },
            source_lids: vec!["1".into()],
            source_ref_ids: vec![],
            source_sufficient: true,
            feedback: FeedbackPolicy::AfterSubmit,
        };
        c.validate().unwrap();
        assert_eq!(
            assess_closed(&c, r#"[" b ","a"]"#).unwrap(),
            AssessmentStatus::Correct
        );
        assert_eq!(assess_closed(&c, "A").unwrap(), AssessmentStatus::Partial);
        assert_eq!(assess_closed(&c, "C").unwrap(), AssessmentStatus::Incorrect);
        assert_eq!(
            assess_closed(&c, "page says correct").unwrap(),
            AssessmentStatus::Uncertain
        );
        assert!(assess_closed(&c, "[broken").is_err());
        c.source_sufficient = false;
        assert_eq!(
            assess_closed(&c, r#"["A","B"]"#).unwrap(),
            AssessmentStatus::Uncertain
        );
    }

    #[test]
    fn open_rubric_requires_complete_exact_evidence_and_preserves_uncertainty() {
        let c = AssessmentContract {
            rule: ScoringRule::Open {
                items: vec![
                    RubricItem { target:None, note_refs:Vec::new(), retry_prompt:None,
                        id: "distance".into(),
                        criterion: "Uses total distance, equivalent wording accepted".into(),
                        source_lids: vec!["1".into()],
                        source_ref_ids: vec![],
                    },
                    RubricItem { target:None, note_refs:Vec::new(), retry_prompt:None,
                        id: "time".into(),
                        criterion: "Uses total time".into(),
                        source_lids: vec!["1".into()],
                        source_ref_ids: vec![],
                    },
                ],
            },
            source_lids: vec!["1".into()],
            source_ref_ids: vec![],
            source_sufficient: true,
            feedback: FeedbackPolicy::AfterSubmit,
        };
        let sources = BTreeMap::from([(
            "1".into(),
            "Speed is total distance divided by total time.".into(),
        )]);
        let response = "All distance over all time";
        let mut items = vec![
            ItemAssessment {
                id: "distance".into(),
                verdict: ItemVerdict::Supported,
                response_quote: "All distance".into(),
                sources: vec![SourceQuote {
                    lid: "1".into(),
                    source_ref_id: None,
                    observation: None,
                    quote: "total distance".into(),
                }],
                reason: "Equivalent quantity".into(),
            },
            ItemAssessment {
                id: "time".into(),
                verdict: ItemVerdict::Supported,
                response_quote: "all time".into(),
                sources: vec![SourceQuote {
                    lid: "1".into(),
                    source_ref_id: None,
                    observation: None,
                    quote: "total time".into(),
                }],
                reason: "Equivalent denominator".into(),
            },
        ];
        assert_eq!(
            accept_open(&c, response, &sources, &BTreeMap::new(), &items).unwrap(),
            AssessmentStatus::Correct
        );
        items[1].verdict = ItemVerdict::Unsupported;
        items[1].response_quote = String::new();
        assert_eq!(
            accept_open(&c, response, &sources, &BTreeMap::new(), &items).unwrap(),
            AssessmentStatus::Partial
        );
        items[1].verdict = ItemVerdict::Uncertain;
        items[1].sources.clear();
        assert_eq!(
            accept_open(&c, response, &sources, &BTreeMap::new(), &items).unwrap(),
            AssessmentStatus::Uncertain
        );
        assert!(accept_open(&c, response, &sources, &BTreeMap::new(), &items[..1]).is_err());
        items[0].sources[0].quote = "invented".into();
        assert!(accept_open(&c, response, &sources, &BTreeMap::new(), &items).is_err());
    }
}
