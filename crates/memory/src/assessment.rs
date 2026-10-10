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
    pub source_lids: Vec<String>,
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
    pub source_lids: Vec<String>,
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
        if self.source_lids.is_empty() {
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
                            || i.source_lids.is_empty()
                            || i.source_lids
                                .iter()
                                .any(|lid| !self.source_lids.contains(lid))
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
    pub lid: String,
    pub quote: String,
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

fn normalize(value: &str) -> String {
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
            || item.sources.iter().any(|s| {
                s.quote.trim().is_empty()
                    || !criterion.source_lids.contains(&s.lid)
                    || !sources
                        .get(&s.lid)
                        .is_some_and(|text| text.contains(&s.quote))
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
                "SELECT assessment FROM teaching_assessments WHERE action_ref=?",
                [action],
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
                "INSERT INTO teaching_assessments(action_ref,assessment) VALUES (?,?)",
                params![
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
    #[test]
    fn frozen_choice_rules_distinguish_equivalence_partial_error_and_uncertainty() {
        let mut c = AssessmentContract {
            rule: ScoringRule::ChoiceSet {
                choices: vec!["A".into(), "B".into(), "C".into()],
                correct: vec!["A".into(), "B".into()],
            },
            source_lids: vec!["1".into()],
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
                    RubricItem {
                        id: "distance".into(),
                        criterion: "Uses total distance, equivalent wording accepted".into(),
                        source_lids: vec!["1".into()],
                    },
                    RubricItem {
                        id: "time".into(),
                        criterion: "Uses total time".into(),
                        source_lids: vec!["1".into()],
                    },
                ],
            },
            source_lids: vec!["1".into()],
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
                    quote: "total time".into(),
                }],
                reason: "Equivalent denominator".into(),
            },
        ];
        assert_eq!(
            accept_open(&c, response, &sources, &items).unwrap(),
            AssessmentStatus::Correct
        );
        items[1].verdict = ItemVerdict::Unsupported;
        items[1].response_quote = String::new();
        assert_eq!(
            accept_open(&c, response, &sources, &items).unwrap(),
            AssessmentStatus::Partial
        );
        items[1].verdict = ItemVerdict::Uncertain;
        items[1].sources.clear();
        assert_eq!(
            accept_open(&c, response, &sources, &items).unwrap(),
            AssessmentStatus::Uncertain
        );
        assert!(accept_open(&c, response, &sources, &items[..1]).is_err());
        items[0].sources[0].quote = "invented".into();
        assert!(accept_open(&c, response, &sources, &items).is_err());
    }
}
