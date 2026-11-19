//! Read-only facts through one committed position (ADR-0153).
use crate::session_event::{DeliveredEffect, DispositionAction, EventBody, SessionProjection};
use crate::*;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct Evidence {
    pub turn_id: String,
    pub event_seq: u64,
}

#[derive(Serialize)]
pub struct Question {
    pub text: String,
    pub status: &'static str,
    pub evidence: Vec<Evidence>,
}

#[derive(Serialize)]
pub struct Source {
    pub source_ref_id: String,
    pub label: String,
    pub quote: String,
    pub published_book_ref: Option<published_library::PublishedBookRef>,
    pub evidence: Vec<Evidence>,
    pub unavailable_reason: Option<String>,
}

#[derive(Serialize)]
pub struct Effect {
    pub effect_id: String,
    pub label: String,
    pub status: &'static str,
    pub effect: DeliveredEffect,
    pub object_id: Option<String>,
    pub published_book_ref: Option<published_library::PublishedBookRef>,
    pub evidence: Vec<Evidence>,
    pub unavailable_reason: Option<String>,
}

#[derive(Serialize)]
pub struct Continuation {
    pub goal_id: Option<String>,
    pub text: String,
    pub status: &'static str,
    pub evidence: Vec<Evidence>,
}

#[derive(Serialize)]
pub struct Recap {
    pub session_id: String,
    pub through_seq: u64,
    pub through_at: String,
    pub generated_at: String,
    pub questions: Vec<Question>,
    pub sources: Vec<Source>,
    pub effects: Vec<Effect>,
    pub continuations: Vec<Continuation>,
}

pub fn request(query: &HashMap<String, String>) -> Result<(&str, Option<u64>), ToolError> {
    let invalid = || {
        user_storage_paths::error(
            "INVALID_RECAP_RANGE",
            "validation",
            "A session and a committed positive position are required",
        )
    };
    let id = query
        .get("session_id")
        .filter(|s| !s.is_empty())
        .ok_or_else(invalid)?;
    let through = query
        .get("through_seq")
        .map(|s| s.parse::<u64>().ok().filter(|n| *n > 0).ok_or_else(invalid))
        .transpose()?;
    Ok((id, through))
}

pub fn snapshot(
    user: &user_runtime::UserRuntime,
    id: &str,
    through: Option<u64>,
) -> Result<SessionProjection, ToolError> {
    let log = user
        .session_store
        .as_ref()
        .and_then(|s| s.logs.get(id))
        .ok_or_else(service_error::missing)?;
    let seq = through.unwrap_or(log.projection.through_seq);
    if seq == 0 || seq > log.projection.through_seq {
        return Err(user_storage_paths::error(
            "INVALID_RECAP_RANGE",
            "validation",
            "The requested position is not committed",
        ));
    }
    // Do not settle an uncertain append here: reads cannot commit facts.
    log.at(seq)
}

fn status(turn: &AgentChatTurn) -> &'static str {
    match turn.status {
        AgentAssistantStatus::PendingAssistant => "running",
        AgentAssistantStatus::Failed => "interrupted",
        AgentAssistantStatus::Cancelled => "cancelled",
        AgentAssistantStatus::Completed if turn.outcome.as_ref().is_some_and(|o| o.incomplete) => {
            "incomplete"
        }
        AgentAssistantStatus::Completed => "answered",
    }
}

fn evidence(turn: &str, seq: u64) -> Evidence {
    Evidence {
        turn_id: turn.into(),
        event_seq: seq,
    }
}

/// Facts come exclusively from the prefix; availability is a current, read-only lookup.
pub fn project(projection: &SessionProjection, generated_at: &str) -> Recap {
    let session = projection.session.as_ref().unwrap();
    let mut recap = Recap {
        session_id: session.id.clone(),
        through_seq: projection.through_seq,
        through_at: session.updated_at.clone(),
        generated_at: generated_at.into(),
        questions: vec![],
        sources: vec![],
        effects: vec![],
        continuations: vec![],
    };
    for turn in &session.turns {
        let accepted = evidence(&turn.turn_id, projection.accepted_seq[&turn.turn_id]);
        let latest = evidence(&turn.turn_id, projection.turn_seq[&turn.turn_id]);
        let mut turn_evidence = vec![accepted];
        if latest.event_seq != turn_evidence[0].event_seq {
            turn_evidence.push(latest.clone());
        }
        recap.questions.push(Question {
            text: turn.user.clone(),
            status: status(turn),
            evidence: turn_evidence,
        });
        if status(turn) != "answered" {
            recap.continuations.push(Continuation {
                goal_id: None,
                text: turn.user.clone(),
                status: status(turn),
                evidence: vec![latest.clone()],
            });
        }
        for binding in &turn.source_bindings {
            let seq = projection
                .facts
                .iter()
                .rev()
                .find_map(|event| {
                    if event.turn_id.as_deref() != Some(&turn.turn_id) {
                        return None;
                    }
                    match &event.body {
                        EventBody::SourcesBound { bindings } if bindings.contains(binding) => {
                            Some(event.seq)
                        }
                        _ => None,
                    }
                })
                .unwrap_or(latest.event_seq);
            recap.sources.push(Source {
                source_ref_id: binding.source_ref_id.clone(),
                label: binding.label_snapshot.clone(),
                quote: binding.preview_snapshot.clone(),
                published_book_ref: source_publication(turn, binding),
                evidence: vec![evidence(&turn.turn_id, seq)],
                unavailable_reason: None,
            });
        }
        for record in &turn.domain.effects {
            let mut refs = vec![];
            let mut dispositions = BTreeMap::new();
            let mut retained_object = None;
            for event in &projection.facts {
                if event.turn_id.as_deref() != Some(&turn.turn_id) {
                    continue;
                }
                let applies = match &event.body {
                    EventBody::EffectDelivered { effect_id, .. } => effect_id == &record.effect_id,
                    EventBody::DispositionStarted(started)
                        if started.effect_id == record.effect_id =>
                    {
                        dispositions.insert(&started.disposition_id, &started.action);
                        true
                    }
                    EventBody::EffectDisposed(receipt) => {
                        let action = dispositions.get(&receipt.disposition_id);
                        if action == Some(&&DispositionAction::Keep) && receipt.error.is_none() {
                            retained_object = receipt.result_object_id.clone();
                        }
                        action.is_some()
                    }
                    _ => false,
                };
                if applies {
                    refs.push(evidence(&turn.turn_id, event.seq));
                }
            }
            let (label, original) = match &record.effect {
                DeliveredEffect::Presentation { reference } => {
                    (format!("演示 · 版本 {}", reference.revision), None)
                }
                DeliveredEffect::Reader { effect } => match effect {
                    AgentEffect::Note { mem_id, text, .. } => {
                        (format!("笔记：{text}"), Some(mem_id.clone()))
                    }
                    AgentEffect::Highlight { mem_id, .. } => {
                        ("原文标注".into(), Some(mem_id.clone()))
                    }
                    AgentEffect::Goto { after_anchor, .. } => {
                        (format!("阅读定位：{after_anchor}"), None)
                    }
                    AgentEffect::Layout { .. } => ("阅读布局".into(), None),
                    AgentEffect::LayoutProposal { proposal } => {
                        (format!("布局提议：{}", proposal.summary), None)
                    }
                    AgentEffect::PaperMinimap { .. } => ("阅读地图".into(), None),
                    AgentEffect::PaperMinimapProposal { .. } => ("地图提议".into(), None),
                },
            };
            let original = retained_object.or(original);
            let (state, object_id) = match &record.disposition {
                None => (
                    if matches!(record.effect, DeliveredEffect::Presentation { .. }) {
                        "delivered"
                    } else {
                        "pending"
                    },
                    original,
                ),
                Some(d) => match &d.receipt {
                    None => ("unconfirmed", original),
                    Some(receipt) if receipt.error.is_some() => ("failed", original),
                    Some(receipt) => match d.started.action {
                        DispositionAction::Keep => (
                            if matches!(
                                record.effect,
                                DeliveredEffect::Reader {
                                    effect: AgentEffect::LayoutProposal { .. }
                                        | AgentEffect::PaperMinimapProposal { .. }
                                }
                            ) {
                                "applied"
                            } else {
                                "kept"
                            },
                            receipt.result_object_id.clone(),
                        ),
                        DispositionAction::Undo => ("undone", None),
                        DispositionAction::Dismiss => ("dismissed", None),
                    },
                },
            };
            recap.effects.push(Effect {
                effect_id: record.effect_id.clone(),
                label,
                status: state,
                effect: record.effect.clone(),
                object_id,
                published_book_ref: turn.published_book_ref.clone(),
                evidence: refs,
                unavailable_reason: None,
            });
        }
    }
    for goal in &session.goals {
        if goal.status == runtime::goal::GoalStatus::Open {
            recap.continuations.push(Continuation {
                goal_id: Some(goal.id.clone()),
                text: goal.interpretation.clone(),
                status: "open",
                evidence: vec![evidence(
                    &goal.origin_turn_id,
                    projection.goal_seq[&goal.id],
                )],
            });
        }
    }
    recap
}

pub fn source_publication(turn: &AgentChatTurn, binding: &SourceBinding) -> Option<published_library::PublishedBookRef> {
    binding.publication_id.as_ref().map(|publication_id| published_library::PublishedBookRef {
        book_id: binding.book_id.clone(), publication_id: publication_id.clone(),
    }).or_else(|| turn.published_book_ref.as_ref().filter(|reference| reference.book_id == binding.book_id).cloned())
}

pub fn resolve(
    recap: &mut Recap,
    projection: &SessionProjection,
    user: &user_runtime::UserRuntime,
    mut material: impl FnMut(&AgentChatTurn, Option<&published_library::PublishedBookRef>) -> Option<(Arc<Book>, PathBuf)>,
) {
    let session = projection.session.as_ref().unwrap();
    for turn in &session.turns {
        let turn_book = turn.published_book_ref.as_ref().map_or(session.book_id.as_str(), |r| r.book_id.as_str());
        let records = user.store.recall(&RecallQuery { book_id: Some(turn_book.to_owned()), ..Default::default() });
        let book = material(turn, turn.published_book_ref.as_ref());
        for source in recap
            .sources
            .iter_mut()
            .filter(|s| s.evidence[0].turn_id == turn.turn_id)
        {
            let source_book = material(turn, source.published_book_ref.as_ref());
            source.unavailable_reason = match &source_book {
                None => Some("原发布当前不可用".into()),
                Some((book, directory)) => {
                    let binding = turn
                        .source_bindings
                        .iter()
                        .find(|b| b.source_ref_id == source.source_ref_id)
                        .unwrap();
                    if let Some(media) = &binding.media {
                        crate::video_source::snapshot(book, directory, media.target.clone()).err().map(|_| "原媒体当前不可用".into())
                    } else { book.resolve_source(
                        binding.evidence_range.as_ref().expect("text binding"),
                        "zh-CN",
                        Some(&binding.evidence_text_digest),
                    )
                    .err()
                    .map(|_| "原文当前不可用".into()) }
                }
            };
        }
        for effect in recap
            .effects
            .iter_mut()
            .filter(|e| e.evidence[0].turn_id == turn.turn_id)
        {
            effect.unavailable_reason = if let Some((book, directory)) = &book {
                match &effect.effect {
                    DeliveredEffect::Presentation { reference } => {
                        let private = PrivateBookContext {
                            user,
                            learning_workspace_id: session.learning_workspace_id.as_deref(),
                            book,
                            book_dir: directory,
                            messages: &[],
                            selected_chat: Some(&session.id),
                        };
                        private
                            .read_presentation(&session.id, reference)
                            .err()
                            .map(|_| "原演示版本当前不可用".into())
                    }
                    DeliveredEffect::Reader {
                        effect: AgentEffect::Note { .. } | AgentEffect::Highlight { .. },
                    } => {
                        if let Some(record) = effect.object_id.as_deref()
                            .and_then(|id| user.store.current_note_record(id))
                            .filter(|record| record.book_id == turn_book)
                        {
                            effect.object_id = Some(record.mem_id.clone());
                        }
                        if effect
                            .object_id
                            .as_ref()
                            .is_some_and(|id| records.iter().any(|r| &r.mem_id == id))
                        {
                            None
                        } else {
                            Some("原成果当前不可用".into())
                        }
                    }
                    _ => None,
                }
            } else {
                Some("原发布当前不可用".into())
            };
        }
    }
}

pub(crate) fn local(
    state: &AppState,
    query: &HashMap<String, String>,
    now: &str,
) -> Result<Recap, ToolError> {
    let (id, through) = request(query)?;
    let projection = snapshot(&state.user, id, through)?;
    let mut recap = project(&projection, now);
    resolve(&mut recap, &projection, &state.user, |_, reference| {
        (projection.session.as_ref().unwrap().book_id == state.workspace.book.base.book_id
            && reference
                == state.workspace.publication.as_ref().map(|p| &p.reference))
        .then(|| {
            (
                state.workspace.book.clone(),
                state.workspace.book_dir.clone(),
            )
        })
    });
    Ok(recap)
}

