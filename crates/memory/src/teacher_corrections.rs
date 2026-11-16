//! Source-scoped teacher corrections, derived from the existing teaching event log.
//! Search crosses chats/stages inside one workspace; learner state is never written.
use crate::learning::{storage, LearningStore};
use crate::teaching::{invalid, TeachingBinding, TeachingEvent, TeachingFact};
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CorrectionQuote {
    pub lid: String,
    pub quote: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TeacherCorrection {
    pub previous_event_ref: String,
    /// An exact excerpt of the teacher's actual old statement.
    pub mistake_quote: String,
    /// The specific corrected relationship, not a generic reminder to be careful.
    pub corrected_claim: String,
    /// Conditions where this correction applies; do not prescribe a teaching mode.
    pub applies_when: String,
    /// Searchable subject terms in the conversation and original source language.
    pub topics: Vec<String>,
    /// Exact original-source excerpts at LIDs read in this turn.
    pub source_quotes: Vec<CorrectionQuote>,
    #[serde(default)]
    pub supersedes: Vec<String>,
}

fn same_source(a: &TeachingBinding, b: &TeachingBinding) -> bool {
    a.source_id == b.source_id && a.source_revision == b.source_revision
}

fn teacher_text(event: &TeachingEvent) -> Option<&str> {
    match event.kind {
        TeachingFact::MessageDelivered => event.payload["answer"]
            .as_str()
            .or_else(|| event.payload["move"]["delivery_text"].as_str()),
        TeachingFact::UsageObserved if event.payload["usage"]["kind"] == "assistant_delivery" => {
            event.payload["answer"].as_str()
        }
        TeachingFact::AgentWorkReviewed => event.payload["proposed_delivery"]["text"].as_str(),
        _ => None,
    }
}

// Legacy reviews allow quotes spanning contiguous source leaves (e.g. inline
// formula nodes). Both LIDs and every intervening read span must match.
fn citation_supported(citation: &Value, sources: &Value) -> bool {
    let Some(quote) = citation["quote"].as_str().filter(|q| !q.is_empty()) else {
        return false;
    };
    sources.as_array().into_iter().flatten().any(|source| {
        if source["source_id"] != citation["source_id"]
            || source["source_revision"] != citation["source_revision"]
        {
            return false;
        }
        let Some(segments) = source["segments"].as_array() else {
            return false;
        };
        segments.iter().enumerate().any(|(start, segment)| {
            if segment["lid"] != citation["lid"] {
                return false;
            }
            let Some(first) = segment["text"].as_str() else {
                return false;
            };
            let Some(end) = citation["end_lid"]
                .as_str()
                .filter(|end| Some(*end) != citation["lid"].as_str())
            else {
                return first.contains(quote);
            };
            let mut text = first.to_owned();
            let mut previous_end = segment["source_range"]["end"].as_u64();
            for next in &segments[start + 1..] {
                let Some(next_start) = next["source_range"]["start"].as_u64() else {
                    return false;
                };
                if previous_end != Some(next_start) {
                    return false;
                }
                let Some(part) = next["text"].as_str() else {
                    return false;
                };
                let last_start = text.len();
                text.push_str(part);
                if next["lid"] == end {
                    return text.match_indices(quote).any(|(offset, matched)| {
                        offset < first.len() && offset + matched.len() > last_start
                    });
                }
                previous_end = next["source_range"]["end"].as_u64();
            }
            false
        })
    })
}

impl LearningStore {
    pub fn record_teacher_correction(
        &mut self,
        binding: &TeachingBinding,
        operation: &str,
        correction: TeacherCorrection,
        request: &str,
        now: &str,
    ) -> Result<TeachingEvent, ToolError> {
        if operation.trim().is_empty()
            || operation.len() > 160
            || [
                &correction.mistake_quote,
                &correction.corrected_claim,
                &correction.applies_when,
            ]
            .iter()
            .any(|s| s.trim().is_empty() || s.chars().count() > 1600)
            || correction.topics.is_empty()
            || correction.topics.len() > 8
            || correction
                .topics
                .iter()
                .any(|s| s.trim().chars().count() < 2 || s.chars().count() > 80)
            || correction.source_quotes.is_empty()
            || correction.source_quotes.len() > 6
            || correction.source_quotes.iter().any(|q| {
                q.lid.is_empty() || q.quote.trim().is_empty() || q.quote.chars().count() > 1600
            })
        {
            return Err(invalid(
                "教师纠错需提供非空原话、具体修正和适用条件（各至多1600字），1–8个主题（每个2–80字），1–6条原文引句（每条至多1600字），以及至多160字节的 operation_id",
            ));
        }
        let old = self.teaching_event(&correction.previous_event_ref)?;
        if !same_source(binding, &old.binding)
            || !teacher_text(&old).is_some_and(|text| text.contains(&correction.mistake_quote))
        {
            return Err(invalid(
                "错误原话必须来自同一资料版本的实际教师交付或历史审核稿",
            ));
        }
        let event = TeachingEvent {
            event_id: format!("teacher-correction:{}:{operation}", binding.turn_id),
            binding: binding.clone(),
            kind: TeachingFact::TeacherCorrectionRecorded,
            causal_refs: vec![],
            payload: json!({"correction":correction,"request":request}),
            occurred_at: now.into(),
        };
        if let Ok(previous) = self.teaching_event(&event.event_id) {
            event.validate_unchanged_retry(&previous)?;
            return Ok(previous);
        }
        let cards = self.teacher_correction_cards(binding)?;
        for id in &correction.supersedes {
            if !cards.iter().any(|card| card["correction_ref"] == *id) {
                return Err(invalid(
                    "待替代纠正已失效或不属于当前工作区和资料版本，请重新检索",
                ));
            }
        }
        self.append_teaching(&event)
    }

    /// Includes grounded legacy review corrections, without migrating/copying them.
    fn teacher_correction_cards(&self, binding: &TeachingBinding) -> Result<Vec<Value>, ToolError> {
        let mut statement = self.connection.prepare("SELECT event FROM teaching_trace
            WHERE learning_workspace_id=?1 AND json_extract(event,'$.binding.source_id')=?2
            AND json_extract(event,'$.binding.source_revision')=?3
            AND json_extract(event,'$.kind') IN ('teacher_correction_recorded','agent_work_reviewed') ORDER BY seq").map_err(storage)?;
        let events: Vec<TeachingEvent> = statement
            .query_map(
                params![
                    self.workspace_scope,
                    binding.source_id,
                    binding.source_revision
                ],
                |row| row.get::<_, String>(0),
            )
            .map_err(storage)?
            .map(|row| serde_json::from_str(&row.map_err(storage)?).map_err(storage))
            .collect::<Result<_, _>>()?;
        let mut cards = Vec::new();
        let mut superseded = BTreeSet::new();
        let latest_reviews: BTreeMap<_, _> = events
            .iter()
            .filter(|event| event.kind == TeachingFact::AgentWorkReviewed)
            .map(|event| {
                (
                    (&event.binding.chat_session_id, &event.binding.turn_id),
                    event,
                )
            })
            .collect();
        for event in &events {
            if event.kind == TeachingFact::TeacherCorrectionRecorded {
                // Saving a correction during generation is not proof it was delivered.
                let delivered: Option<String> = self.connection.query_row("SELECT event_id FROM teaching_trace
                    WHERE learning_workspace_id=?1 AND json_extract(event,'$.binding.chat_session_id')=?2 AND json_extract(event,'$.binding.turn_id')=?3
                    AND json_extract(event,'$.binding.source_id')=?5 AND json_extract(event,'$.binding.source_revision')=?6
                    AND json_extract(event,'$.kind')='usage_observed' AND json_extract(event,'$.payload.usage.kind')='assistant_delivery'
                    AND COALESCE(json_extract(event,'$.payload.incomplete'),0)=0
                    AND seq > (SELECT seq FROM teaching_trace WHERE learning_workspace_id=?1 AND event_id=?4) LIMIT 1",
                    params![self.workspace_scope,event.binding.chat_session_id,event.binding.turn_id,event.event_id,binding.source_id,binding.source_revision], |row|row.get(0)).optional().map_err(storage)?;
                let Some(delivered) = delivered else {
                    continue;
                };
                let correction: TeacherCorrection =
                    serde_json::from_value(event.payload["correction"].clone()).map_err(storage)?;
                superseded.extend(correction.supersedes.iter().cloned());
                cards.push(json!({"correction_ref":event.event_id,"origin":"delivered_correction","binding":event.binding,
                    "occurred_at":event.occurred_at,"correction":correction,"original_request":event.payload["request"],"delivery_ref":delivered}));
            } else if event.payload["review"]["decision"] == "revise" {
                let latest =
                    latest_reviews[&(&event.binding.chat_session_id, &event.binding.turn_id)];
                let knowledge = &latest.payload["review"]["agent_work"]["understanding"];
                let grounded: Vec<_> = knowledge
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|claim| {
                        let citations = claim["sources"].as_array();
                        claim["statement"]
                            .as_str()
                            .is_some_and(|s| !s.trim().is_empty())
                            && citations.is_some_and(|sources| {
                                !sources.is_empty()
                                    && sources.iter().all(|source| {
                                        source["source_id"] == binding.source_id
                                            && source["source_revision"] == binding.source_revision
                                            && source["quote"]
                                                .as_str()
                                                .is_some_and(|quote| !quote.is_empty())
                                            && citation_supported(
                                                source,
                                                &latest.payload["source_evidence"],
                                            )
                                    })
                            })
                    })
                    .cloned()
                    .collect();
                if latest.payload["review"]["decision"] != "accept" || grounded.is_empty() {
                    continue;
                }
                cards.push(json!({"correction_ref":event.event_id,"origin":"historical_review","binding":event.binding,
                    "occurred_at":event.occurred_at,"original_request":event.payload["original_request"],
                    "correction":{"previous_event_ref":event.event_id,"mistake_quote":event.payload["proposed_delivery"]["text"],
                        "corrected_claim":grounded,"applies_when":event.payload["review"]["revision"],"topics":[]},
                    "accepted_review_ref":latest.event_id,"meaning":"Past review acceptance is not a truth verdict; inspect the quoted source and applicability."}));
            }
        }
        cards.retain(|card| !superseded.contains(card["correction_ref"].as_str().unwrap()));
        Ok(cards)
    }

    pub fn teacher_correction_hint(&self, binding: &TeachingBinding) -> Result<Value, ToolError> {
        // Metadata only: discovering older corrections does not inject the history.
        let count: u64 = self.connection.query_row("SELECT count(*) FROM teaching_trace
            WHERE learning_workspace_id=?1 AND json_extract(event,'$.binding.source_id')=?2
            AND json_extract(event,'$.binding.source_revision')=?3
            AND json_extract(event,'$.kind') IN ('teacher_correction_recorded','agent_work_reviewed')
            AND (json_extract(event,'$.kind')='teacher_correction_recorded' OR
                (json_extract(event,'$.kind')='agent_work_reviewed' AND json_extract(event,'$.payload.review.decision')='revise'))",
            params![self.workspace_scope,binding.source_id,binding.source_revision], |row| row.get(0)).map_err(storage)?;
        Ok(
            json!({"record_count":count,"scope":"current reader, workspace and source version; includes other chats and stages",
            "tool":"tutor.step","operation":"corrections"}),
        )
    }

    pub fn search_teacher_corrections(
        &self,
        binding: &TeachingBinding,
        queries: &[String],
        after: usize,
    ) -> Result<Value, ToolError> {
        if queries.is_empty()
            || queries.len() > 6
            || queries
                .iter()
                .any(|q| q.trim().chars().count() < 2 || q.chars().count() > 80)
        {
            return Err(invalid("用 1–6 个具体主题词检索教师纠错，每词 2–80 字符"));
        }
        let queries: Vec<_> = queries.iter().map(|q| q.trim().to_lowercase()).collect();
        let terms: Vec<_> = queries
            .iter()
            .flat_map(|query| query.split_whitespace())
            .filter(|term| term.chars().count() >= 2)
            .collect();
        let cards = self.teacher_correction_cards(binding)?;
        let matches: Vec<_> = cards
            .into_iter()
            .filter(|card| {
                let text =
                    json!({"request":card["original_request"],"correction":card["correction"]})
                        .to_string()
                        .to_lowercase();
                terms.iter().any(|term| text.contains(term))
                    || card["correction"]["topics"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .any(|topic| {
                            queries
                                .iter()
                                .any(|query| query.contains(&topic.to_lowercase()))
                        })
            })
            .collect();
        let total = matches.len();
        // tutor.step has a 4 KiB model-result budget. Preserve whole candidates
        // and a real continuation, rather than losing source/conditions to the
        // generic result truncator. Large records remain exactly chunk-readable.
        let mut candidates = Vec::new();
        for card in matches.into_iter().rev().skip(after).take(4) {
            let card = if card.to_string().len() > 2800 {
                let excerpt = |field: &str| {
                    let value = &card["correction"][field];
                    value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string())
                        .chars()
                        .take(160)
                        .collect::<String>()
                };
                json!({"correction_ref":card["correction_ref"],"origin":card["origin"],"details_truncated":true,
                    "correction":{"mistake_quote":excerpt("mistake_quote"),"corrected_claim":excerpt("corrected_claim"),"applies_when":excerpt("applies_when")},
                    "read":"Use correction_action=read with correction_ref; after is a character offset. Read the source and conditions before deciding."})
            } else {
                card
            };
            candidates.push(card);
            if serde_json::to_vec(&candidates).map_err(storage)?.len() > 3300 {
                candidates.pop();
                break;
            }
        }
        let end = after + candidates.len();
        Ok(
            json!({"candidates":candidates,"total":total,"next":(end<total).then_some(end),
            "meaning":"Teacher corrections are candidates, not learner evidence. Check conditions and source; use records apply/not_applicable. Read truncated details first. Current explicit requests control delivery."}),
        )
    }

    pub fn read_teacher_correction(
        &self,
        binding: &TeachingBinding,
        reference: &str,
        after: usize,
    ) -> Result<Value, ToolError> {
        let mut card = self
            .teacher_correction_cards(binding)?
            .into_iter()
            .find(|card| card["correction_ref"] == reference)
            .ok_or_else(|| invalid("教师纠正不存在、已被替代或不属于当前资料版本"))?;
        if let Some(reference) = card["delivery_ref"].as_str() {
            let delivered = self.teaching_event(reference)?;
            card["actual_delivery"] = json!({"event_ref":reference,"answer":delivered.payload["answer"],"occurred_at":delivered.occurred_at});
        }
        let text = card.to_string();
        let total = text.chars().count();
        Ok(
            json!({"correction_ref":reference,"chunk":text.chars().skip(after).take(768).collect::<String>(),
            "total_chars":total,"next":(after+768<total).then_some(after+768)}),
        )
    }

    pub fn use_teacher_correction(
        &mut self,
        binding: &TeachingBinding,
        operation: &str,
        reference: &str,
        decision: &str,
        reason: &str,
        now: &str,
    ) -> Result<TeachingEvent, ToolError> {
        if operation.trim().is_empty()
            || operation.len() > 160
            || !matches!(decision, "apply" | "not_applicable")
            || reason.trim().is_empty()
            || reason.chars().count() > 1600
        {
            return Err(invalid(
                "教师纠错使用记录需给出 apply/not_applicable 与本次适用理由",
            ));
        }
        let cards = self.teacher_correction_cards(binding)?;
        if !cards.iter().any(|card| card["correction_ref"] == reference) {
            return Err(invalid(
                "该纠正已被后续修正替代，或不属于当前资料版本，请重新检索",
            ));
        }
        self.append_teaching(&TeachingEvent {
            event_id: format!("teacher-correction-use:{}:{operation}", binding.turn_id),
            binding: binding.clone(),
            kind: TeachingFact::TeacherCorrectionUsed,
            causal_refs: vec![],
            payload: json!({"correction_ref":reference,"decision":decision,"reason":reason}),
            occurred_at: now.into(),
        })
    }
}
