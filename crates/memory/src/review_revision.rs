//! Settle review output against the current document, including changes made while
//! the model was running. All mutations are committed with the review watermark.
use crate::profile::build_profile_fact;
use crate::{CreateProfileFact, FactSource, FactStatus, ProfileFact};
use read_tools::ToolError;
use std::collections::BTreeSet;

pub(crate) fn reconcile(
    stored: &mut Vec<ProfileFact>,
    mut incoming: Vec<ProfileFact>,
    reviewed: &[ProfileFact],
    now: &str,
) -> Result<(Vec<String>, Vec<String>), ToolError> {
    // Older documents acquire assertion times from original session events, never
    // from a review's completion time. Do not overwrite a newer foreground edit.
    for fact in stored.iter_mut() {
        if fact.observed_at_ms.is_none() {
            if let Some(context) = reviewed
                .iter()
                .find(|old| old.fact_id == fact.fact_id && old.evidence == fact.evidence)
            {
                fact.observed_at_ms = context.observed_at_ms;
            }
        }
    }
    incoming.sort_by(|a, b| {
        a.observed_at_ms
            .cmp(&b.observed_at_ms)
            .then_with(|| a.fact_id.cmp(&b.fact_id))
    });
    let mut added = Vec::new();
    let mut affected = BTreeSet::new();
    for mut fact in incoming {
        if stored.iter().any(|old| old.fact_id == fact.fact_id) {
            continue;
        }
        let mut related: BTreeSet<String> = fact.supersedes.iter().cloned().collect();
        for id in &fact.supersedes {
            if !reviewed.iter().any(|old| old.fact_id == *id) {
                return Err(invalid("revision target was not offered to this review"));
            }
            let old = stored
                .iter()
                .find(|old| old.fact_id == *id)
                .ok_or_else(|| {
                    invalid("revision target was removed while the review was running")
                })?;
            if old.status == FactStatus::Expired
                || old.scope != fact.scope
                || old.applicability != fact.applicability
            {
                return Err(invalid(
                    "revision target is inactive or belongs to another scope",
                ));
            }
            if !may_replace(fact.source, old) {
                return Err(invalid(
                    "an inferred revision cannot replace a user statement",
                ));
            }
            let mut payload = fact.payload.clone();
            payload.reuse_key_from(&old.payload)?;
        }
        if let Some(id) = fact.supersedes.first() {
            let old = stored.iter().find(|old| old.fact_id == *id).unwrap();
            fact.payload.reuse_key_from(&old.payload)?;
        }
        related.extend(
            stored
                .iter()
                .filter(|old| {
                    old.scope == fact.scope
                        && old.applicability == fact.applicability
                        && old.payload.semantic_key() == fact.payload.semantic_key()
                })
                .map(|old| old.fact_id.clone()),
        );
        // Follow a target replaced during extraction, even if a foreground edit
        // changed its key. This prevents stale results from reviving its ancestor.
        loop {
            let before = related.len();
            let descendants: Vec<_> = stored
                .iter()
                .filter(|old| old.supersedes.iter().any(|id| related.contains(id)))
                .map(|old| old.fact_id.clone())
                .collect();
            related.extend(descendants);
            if related.len() == before {
                break;
            }
        }
        let active: Vec<usize> = stored
            .iter()
            .enumerate()
            .filter(|(_, old)| {
                related.contains(&old.fact_id)
                    && (old.payload != fact.payload || fact.supersedes.contains(&old.fact_id))
                    && matches!(
                        old.status,
                        FactStatus::Confirmed | FactStatus::Provisional | FactStatus::Pending
                    )
            })
            .map(|(i, _)| i)
            .collect();
        if active.iter().any(|&i| {
            let old = &stored[i];
            !may_replace(fact.source, old)
                || old.scope != fact.scope
                || old.applicability != fact.applicability
                || (fact.status == FactStatus::Pending && old.status != FactStatus::Pending)
                || match (fact.observed_at_ms, old.observed_at_ms) {
                    (Some(new), Some(previous)) => new <= previous,
                    (Some(_), None) => !reviewed.iter().any(|known| known.fact_id == old.fact_id),
                    _ => true,
                }
        }) {
            continue;
        }
        if let Some(&i) = active.first() {
            fact.payload.reuse_key_from(&stored[i].payload)?;
        }
        fact.supersedes
            .extend(active.iter().map(|&i| stored[i].fact_id.clone()));
        let mut built = build_profile_fact(
            CreateProfileFact {
                scope: fact.scope,
                applicability: fact.applicability,
                payload: fact.payload,
                source: fact.source,
                evidence: fact.evidence,
                confidence: fact.confidence,
                sensitivity: fact.sensitivity,
                valid_until: fact.valid_until,
            },
            fact.supersedes,
            now,
        )?;
        built.observed_at_ms = fact.observed_at_ms;
        if stored.iter().any(|old| old.fact_id == built.fact_id) {
            continue;
        }
        for i in active {
            affected.insert(stored[i].payload.semantic_key());
            stored[i].status = FactStatus::Superseded;
            stored[i].updated_at = now.into();
        }
        affected.insert(built.payload.semantic_key());
        added.push(built.fact_id.clone());
        stored.push(built);
    }
    Ok((added, affected.into_iter().collect()))
}

fn may_replace(new: FactSource, old: &ProfileFact) -> bool {
    new == FactSource::UserStated
        || (new == FactSource::AgentInferred
            && old.source == FactSource::AgentInferred
            && old.status != FactStatus::Confirmed)
}

fn invalid(message: &str) -> ToolError {
    ToolError {
        error_code: "INVALID_REVIEW_RESULT".into(),
        category: "validation".into(),
        message: message.into(),
    }
}
