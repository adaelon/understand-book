//! Private Resident log contract (ADR-0152). These types are never public history DTOs.
use crate::*;

pub(crate) const VERSION: u32 = 1;
mod projection;
pub(crate) use projection::SessionProjection;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SessionEvent {
    pub version: u32,
    pub seq: u64,
    pub at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(flatten)]
    pub body: EventBody,
}

// Do not deserialize through serde's flattened/tagged Content buffer: the
// workspace enables arbitrary_precision and frozen ReaderState contains f32.
impl<'de> Deserialize<'de> for SessionEvent {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            version: u32,
            seq: u64,
            at: String,
            turn_id: Option<String>,
            kind: String,
            payload: Value,
        }
        let w = Wire::deserialize(deserializer)?;
        let parse = || -> Result<EventBody, serde_json::Error> {
            macro_rules! fields {
                ($variant:ident { $($field:ident : $ty:ty),* }) => {{
                    #[derive(Deserialize)] struct Payload { $($field: $ty),* }
                    let p: Payload = serde_json::from_value(w.payload)?;
                    EventBody::$variant { $($field: p.$field),* }
                }};
            }
            Ok(match w.kind.as_str() {
                "session.created" => EventBody::SessionCreated(serde_json::from_value(w.payload)?),
                "session.updated" => fields!(SessionUpdated { title: String }),
                "turn.accepted" => EventBody::TurnAccepted(serde_json::from_value(w.payload)?),
                "turn.prepared" => fields!(TurnPrepared {
                    teaching: teaching::FrozenTeachingPreparation
                }),
                "message.appended" => fields!(MessageAppended { messages: Vec<Message> }),
                "activity.recorded" => fields!(ActivityRecorded {
                    activity: runtime::run_events::RunActivity
                }),
                "history.revised" => EventBody::HistoryRevised(serde_json::from_value(w.payload)?),
                "checkpoint.installed" => fields!(CheckpointInstalled {
                    checkpoint: CompactionCheckpoint
                }),
                "goal.updated" => fields!(GoalUpdated {
                    goal: runtime::goal::ResidentGoal
                }),
                "teaching.linked" => EventBody::TeachingLinked(serde_json::from_value(w.payload)?),
                "sources.bound" => fields!(SourcesBound { bindings: Vec<SourceBinding> }),
                "effect.delivered" => fields!(EffectDelivered {
                    effect_id: String,
                    effect: DeliveredEffect
                }),
                "effect.disposition_started" => {
                    EventBody::DispositionStarted(serde_json::from_value(w.payload)?)
                }
                "effect.disposed" => EventBody::EffectDisposed(serde_json::from_value(w.payload)?),
                "turn.finished" => EventBody::TurnFinished(serde_json::from_value(w.payload)?),
                _ => {
                    return Err(serde::de::Error::custom(format!(
                        "unknown session event kind: {}",
                        w.kind
                    )))
                }
            })
        };
        let body = parse().map_err(serde::de::Error::custom)?;
        Ok(Self {
            version: w.version,
            seq: w.seq,
            at: w.at,
            turn_id: w.turn_id,
            body,
        })
    }
}

/// Position is in this session's committed log, not the SSE stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum HistoryPosition {
    Committed { history_through_seq: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SessionCreated {
    pub session_id: String,
    pub book_id: String,
    pub title: String,
    pub created_at: String,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TurnAccepted {
    /// Pending turn only; legacy admission_input is forbidden here.
    pub turn: AgentChatTurn,
    pub history_through_seq: u64,
    pub input: Option<run_admission::FrozenTurn<HistoryPosition>>,
    pub goals: Vec<runtime::goal::ResidentGoal>,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MessageRevision {
    /// Zero-based offset in the raw messages projection before this event.
    pub from: usize,
    pub suffix: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TurnFinished {
    pub status: AgentAssistantStatus,
    pub outcome: Option<OuterOutcome>,
    pub error: Option<AgentTurnError>,
    pub run_summary: Option<agent_run::AgentRunSummary>,
    pub source_bindings: Vec<SourceBinding>,
    pub delivery_diagnostics: Option<AnswerDeliveryDiagnostics>,
    pub goal_ref: Option<AgentGoalRef>,
    pub goals: Vec<runtime::goal::ResidentGoal>,
    /// Terminal status and its necessary cleanup/truncation commit together.
    pub history_revision: Option<MessageRevision>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeachingLink {
    pub session_id: String,
    pub revision: u64,
    pub receipt_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeliveredEffect {
    Reader {
        effect: AgentEffect,
    },
    Presentation {
        reference: runtime::presentation::PresentationRef,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DispositionAction {
    Keep,
    Undo,
    Dismiss,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispositionStarted {
    pub disposition_id: String,
    pub effect_id: String,
    pub action: DispositionAction,
    #[serde(default)]
    pub result_object_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispositionReceipt {
    pub disposition_id: String,
    pub original_object_id: Option<String>,
    pub result_object_id: Option<String>,
    pub error: Option<AgentTurnError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TurnDomain {
    pub scene: Option<ReadingScene>,
    pub teaching: Vec<TeachingLink>,
    pub effects: Vec<EffectRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingScene {
    pub workspace_id: String,
    pub generation: u64,
    pub anchor_lid: String,
    pub selection: Option<String>,
}
impl ReadingScene {
    pub(crate) fn capture(workspace: &reader_workspace::ReaderWorkspace) -> Self {
        let state = workspace.reader.state();
        Self { workspace_id: workspace.id.clone(), generation: workspace.generation,
            anchor_lid: state.viewport.anchor_lid, selection: state.selection }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectRecord {
    pub effect_id: String,
    pub effect: DeliveredEffect,
    pub disposition: Option<EffectDisposition>,
}
impl EffectRecord {
    /// The one supported successor disposition operates on the actual retained object.
    pub(crate) fn retained_object_for_undo(&self, action: &DispositionAction) -> Option<&str> {
        if *action != DispositionAction::Undo || !matches!(self.effect,
            DeliveredEffect::Reader { effect: AgentEffect::Note { .. } | AgentEffect::Highlight { .. } }) {
            return None;
        }
        let disposition = self.disposition.as_ref()?;
        let receipt = disposition.receipt.as_ref()?;
        if disposition.started.action != DispositionAction::Keep || receipt.error.is_some() { return None; }
        receipt.result_object_id.as_deref()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectDisposition {
    pub started: DispositionStarted,
    pub receipt: Option<DispositionReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub(crate) enum EventBody {
    #[serde(rename = "session.created")]
    SessionCreated(SessionCreated),
    #[serde(rename = "session.updated")]
    SessionUpdated { title: String },
    #[serde(rename = "turn.accepted")]
    TurnAccepted(TurnAccepted),
    #[serde(rename = "turn.prepared")]
    TurnPrepared {
        teaching: teaching::FrozenTeachingPreparation,
    },
    #[serde(rename = "message.appended")]
    MessageAppended { messages: Vec<Message> },
    #[serde(rename = "activity.recorded")]
    ActivityRecorded {
        activity: runtime::run_events::RunActivity,
    },
    #[serde(rename = "history.revised")]
    HistoryRevised(MessageRevision),
    #[serde(rename = "checkpoint.installed")]
    CheckpointInstalled { checkpoint: CompactionCheckpoint },
    #[serde(rename = "goal.updated")]
    GoalUpdated { goal: runtime::goal::ResidentGoal },
    #[serde(rename = "teaching.linked")]
    TeachingLinked(TeachingLink),
    #[serde(rename = "sources.bound")]
    SourcesBound { bindings: Vec<SourceBinding> },
    #[serde(rename = "effect.delivered")]
    EffectDelivered {
        effect_id: String,
        effect: DeliveredEffect,
    },
    #[serde(rename = "effect.disposition_started")]
    DispositionStarted(DispositionStarted),
    #[serde(rename = "effect.disposed")]
    EffectDisposed(DispositionReceipt),
    #[serde(rename = "turn.finished")]
    TurnFinished(TurnFinished),
}

impl SessionEvent {
    pub fn new(seq: u64, at: &str, turn_id: Option<&str>, body: EventBody) -> Self {
        Self {
            version: VERSION,
            seq,
            at: at.into(),
            turn_id: turn_id.map(str::to_owned),
            body,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
