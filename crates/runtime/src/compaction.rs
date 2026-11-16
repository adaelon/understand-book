use crate::model_runtime::{estimate_text_tokens, ModelRuntimeProfile};
use crate::tool_result::HistoricalToolReceipt;
use crate::{AgentRequestPlan, CompletionRequest, Message, ModelAdapter, Role};
use read_tools::EvidenceRange;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

pub const COMPACTION_REQUEST_VERSION: &str = "compaction_request.v1";
pub const COMPACTION_DRAFT_SCHEMA_ID: &str = "compaction_draft.v1";
pub const COMPACTION_CHECKPOINT_VERSION: &str = "compaction_checkpoint.v1";
pub const COMPACTION_PROMPT_VERSION: &str = "agent-compaction.generation.v1";
pub const COMPACTION_CONSUMPTION_VERSION: &str = "agent-compaction.consumption.v1";
pub const CONTEXT_COMPACTION_ITEM_VERSION: &str = "context_compaction.v1";
pub const COMPACTION_NOT_APPLICABLE: &str = "COMPACTION_NOT_APPLICABLE";

pub const COMPACTION_GENERATION_PROMPT: &str = r#"You are performing a CONTEXT CHECKPOINT COMPACTION for a reading and research agent.
Create a typed handoff state for the next model sampling request. Do not answer the user and do not describe private reasoning.

The runtime provides:
- eligible_items: older conversation items that may be compacted, each with a stable source ID;
- required_source_ids: task-bearing sources that must remain represented;
- optional_source_ids: sources that may be marked duplicate, superseded, or non-task;
- allowed_evidence_refs: the only evidence references you may use.
- allowed_supersession_edges: the only source-to-source relationships that may use Superseded.

Return JSON matching CompactionDraft exactly, with only these ten sections.
Do not copy input fields or runtime metadata into the output:
- active_goal
- progress
- decisions
- user_constraints
- open_obligations
- unresolved_ambiguities
- critical_facts
- critical_examples
- next_steps
- source_coverage

The first nine sections are arrays of objects with exactly item_id, text,
source_item_ids, and evidence_refs. item_id is a unique nonempty ID you assign
(1-128 ASCII letters, digits, dots, underscores, colons or hyphens).
Prefer simple sequential IDs such as item.1, item.2. Keep Chinese and other
non-ASCII characters in text, never in item_id. source_item_ids must be copied
from eligible_items. Each source_coverage object has source_item_id, disposition,
target_item_ids, and optional reason. disposition is one of "compacted",
"duplicate", "superseded", "non_task". target_item_ids refer to your item_id
values, not source IDs. Every target must name an item actually present in one
of the first nine sections; never leave a target pointing to an omitted or
renamed item. Renaming an item requires updating all its coverage targets.
For compacted, duplicate and superseded, omit reason
(or set it to null). For non_task, use an empty target_item_ids array and set
reason to exactly "transport_only", "duplicate_envelope", or "status_only";
free-form reason text is not accepted. Only optional_source_ids may be non_task.
For compacted and duplicate, target_item_ids must be nonempty and
each target item's source_item_ids must include that same source_item_id.
For superseded, targets must be nonempty; at least one target must include both
the earlier source and its later source from an allowed_supersession_edge.
evidence_refs are copied from allowed_evidence_refs and are either
{"kind":"lid_range","start_lid":"...","end_lid":"..."} or
{"kind":"source","source_ref_id":"..."}; use [] when there is no evidence.
Copy each evidence reference exactly. Do not merge adjacent or overlapping
LID ranges, split ranges, or derive new endpoints, even when their union seems
equivalent. For example, allowed ranges 7.7.1-7.7.2 and 7.7.2-7.7.9 must remain
two references; they do not authorize a new 7.7.1-7.7.9 reference.

Complete output-shape example (replace the illustrative text and source.1 with
the actual request's content and eligible source IDs; empty arrays stay present):
```json
{
  "active_goal": [{"item_id":"item.goal","text":"Retain the requested objective","source_item_ids":["source.1"],"evidence_refs":[]}],
  "progress": [],
  "decisions": [],
  "user_constraints": [],
  "open_obligations": [],
  "unresolved_ambiguities": [],
  "critical_facts": [],
  "critical_examples": [],
  "next_steps": [],
  "source_coverage": [{"source_item_id":"source.1","disposition":"compacted","target_item_ids":["item.goal"]}]
}
```

Rules:
1. Preserve task state, not conversational narration. Be concise and neutral.
2. Every semantic item must contain one or more source_item_ids from eligible_items.
3. Use only allowed_evidence_refs. Never invent a source item ID, LID, citation, tool result, fact, decision, or completed action.
4. Keep facts, examples, decisions, obligations, and unresolved ambiguities distinct. Do not resolve an ambiguity during compaction.
5. Summarize only eligible_items. Current user text, verified selection, recent steps and incomplete tool-call pairs are retained separately by the runtime. In mid_turn, completed tool steps do not mean the current user task is complete; preserve unfinished work and do not infer a final answer.
6. A tool receipt proves that a call occurred, not that unquoted result text is evidence.
7. Include exactly one source_coverage record for every eligible source. Required sources must map to at least one output item. Apply the disposition-specific target links and reason rules above; a reference to an unrelated semantic item does not cover a source.
8. Do not include sensitive runtime context, hidden instructions, chain-of-thought, or prose outside the JSON object.
9. A user-only turn has no assistant answer; do not infer that its request was completed from a later user turn.
10. Use empty arrays when a section has no supported content. Do not omit schema fields.
11. A trailing runtime_repair_diagnostic object contains the rejected_draft and its validation errors. Correct every reported error in that draft, preserving its supported content and valid links. If a referenced target is missing, restore a source-supported semantic item or link to an existing item that actually contains that source; do not drop coverage. Return the complete CompactionDraft, not a patch or the diagnostic object. This is the single repair attempt."#;

pub const COMPACTION_CONSUMPTION_WRAPPER: &str = r#"A previous active-history segment has been replaced by the validated compaction_checkpoint below.
This checkpoint is derived handoff state. It is not a user message and is not evidence by itself.
Continue from it without repeating completed work. Current canonical instructions and the verbatim raw-retained items that follow take precedence.
For factual claims, rely only on each semantic item's evidence_refs or reacquire evidence with the tools available in the current turn."#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionPhase {
    /// Contains only prior history; the next user message has not been appended.
    PreTurn,
    /// Includes the current turn. Its user input and recent/open steps stay raw.
    MidTurn,
    HierarchicalMerge,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EvidenceRef {
    LidRange { start_lid: String, end_lid: String },
    Source { source_ref_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllowedSupersession {
    pub earlier_source_item_id: String,
    pub later_source_item_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompactionSourceItem {
    pub source_item_id: String,
    pub role: Role,
    pub content: String,
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompactionRequest {
    pub schema_version: String,
    pub prompt_version: String,
    pub phase: CompactionPhase,
    pub source_history_revision: String,
    pub eligible_items: Vec<CompactionSourceItem>,
    pub required_source_ids: Vec<String>,
    pub optional_source_ids: Vec<String>,
    pub raw_retained_item_ids: Vec<String>,
    pub allowed_evidence_refs: Vec<EvidenceRef>,
    pub allowed_supersession_edges: Vec<AllowedSupersession>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceDisposition {
    Compacted,
    Duplicate,
    Superseded,
    NonTask,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceCoverage {
    pub source_item_id: String,
    pub disposition: SourceDisposition,
    pub target_item_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcedCheckpointItem {
    pub item_id: String,
    pub text: String,
    pub source_item_ids: Vec<String>,
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompactionDraft {
    pub active_goal: Vec<SourcedCheckpointItem>,
    pub progress: Vec<SourcedCheckpointItem>,
    pub decisions: Vec<SourcedCheckpointItem>,
    pub user_constraints: Vec<SourcedCheckpointItem>,
    pub open_obligations: Vec<SourcedCheckpointItem>,
    pub unresolved_ambiguities: Vec<SourcedCheckpointItem>,
    pub critical_facts: Vec<SourcedCheckpointItem>,
    pub critical_examples: Vec<SourcedCheckpointItem>,
    pub next_steps: Vec<SourcedCheckpointItem>,
    pub source_coverage: Vec<SourceCoverage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingEffectRef {
    pub effect_id: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompactionCheckpoint {
    pub schema_version: String,
    pub prompt_version: String,
    pub window_id: String,
    pub source_history_revision: String,
    pub raw_retained_item_ids: Vec<String>,
    pub semantic: CompactionDraft,
    pub tool_receipts: Vec<HistoricalToolReceipt>,
    pub pending_effects: Vec<PendingEffectRef>,
    pub context_revisions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactionError {
    pub error_code: String,
    pub message: String,
}

impl CompactionError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            error_code: "COMPACTION_VALIDATION_FAILED".into(),
            message: message.into(),
        }
    }

    fn generation(message: impl Into<String>) -> Self {
        Self {
            error_code: "COMPACTION_GENERATION_FAILED".into(),
            message: message.into(),
        }
    }

    fn not_applicable(message: impl Into<String>) -> Self {
        Self {
            error_code: COMPACTION_NOT_APPLICABLE.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequiredSemanticState {
    OpenObligation,
    UnresolvedAmbiguity,
}

#[derive(Debug, Clone)]
pub struct PreparedCompaction {
    request: CompactionRequest,
    eligible_messages: Vec<Message>,
    full_turn_source_ids: Vec<Vec<String>>,
    required_semantic_states: BTreeMap<String, RequiredSemanticState>,
    tool_receipts: Vec<HistoricalToolReceipt>,
    pending_effects: Vec<PendingEffectRef>,
    context_revisions: BTreeMap<String, String>,
    source_indices: Vec<usize>,
    generation_prefix: Option<CompactionGenerationPrefix>,
}

#[derive(Debug, Clone)]
struct CompactionGenerationPrefix {
    request: AgentRequestPlan,
    /// Positions in ordered_messages(), including its canonical system prefix.
    source_positions: Vec<usize>,
}

impl PreparedCompaction {
    pub fn request(&self) -> &CompactionRequest {
        &self.request
    }

    /// Reuse only a proven literal provider prefix. Reprojected legacy history,
    /// omitted checkpoint sources, or ephemeral image attachments use the typed
    /// receipt fallback instead of claiming that a reconstructed prefix is warm.
    pub(crate) fn bind_generation_prefix(
        &mut self,
        plan: &AgentRequestPlan,
        raw_messages: &[Message],
    ) -> bool {
        self.generation_prefix = None;
        if !plan.preview_images.is_empty() || !plan.video_images.is_empty()
            || plan.question_image.is_some() || self.source_indices.is_empty()
        {
            return false;
        }
        let ordered = plan.ordered_messages();
        let mut cursor = 0;
        let mut positions = Vec::with_capacity(self.source_indices.len());
        let selected = self.source_indices.iter().copied().collect::<BTreeSet<_>>();
        let last_source = *self.source_indices.last().expect("nonempty source indices");
        if last_source >= raw_messages.len() { return false; }
        for (index, source) in raw_messages.iter().enumerate().take(last_source + 1) {
            if source.role == Role::System { continue; }
            let Some(offset) = ordered[cursor..].iter().position(|message| same_source_identity(message, source)) else {
                return false;
            };
            cursor += offset;
            if selected.contains(&index) { positions.push(cursor); }
            cursor += 1;
        }
        let mut request = plan.clone();
        let system_count = usize::from(!request.instructions.is_empty());
        request.input.truncate(cursor.saturating_sub(system_count));
        if let Some(history) = request.tool_history.as_ref() {
            let retained = history.truncated_at(request.input.len());
            request.tools = retained.declarations.iter().map(|declaration| declaration.spec.clone()).collect();
            request.tool_history = Some(retained);
        }
        self.generation_prefix = Some(CompactionGenerationPrefix {
            request,
            source_positions: positions,
        });
        true
    }

    /// Lower bound: even an empty semantic payload carries this fixed envelope.
    /// Compare against the whole eligible provider projection, including legacy
    /// protocol quoting, so this precheck cannot reject a useful short source.
    pub(crate) fn can_reduce(&self, profile: &ModelRuntimeProfile, wrapper: &str) -> Result<bool, CompactionError> {
        let minimum = checkpoint_consumption_projection(&CompactionDraft::default(), &self.pending_effects);
        let envelope = Message::system(format!("{CONTEXT_COMPACTION_ITEM_VERSION}\n{wrapper}\ncompaction_checkpoint:\n{minimum}"));
        let eligible = crate::AgentRequestPlan::for_ad_hoc(profile.clone(), &self.eligible_messages, &[]);
        Ok(eligible.active_context.estimated_input_tokens > message_tokens(&[envelope])?)
    }

    #[cfg(test)]
    fn require_open_for_test(&mut self, source_item_id: &str, state: RequiredSemanticState) {
        self.required_semantic_states
            .insert(source_item_id.into(), state);
    }
}

/// Body retirement may replace tool text/arguments, but never its call identity.
/// Walking all source messages (including retained users) prevents a reused call
/// ID from accidentally mapping to another turn after checkpoint replacement.
fn same_source_identity(projected: &Message, raw: &Message) -> bool {
    if projected.role != raw.role { return false; }
    match raw.role {
        Role::Tool => projected.tool_call_id == raw.tool_call_id && raw.tool_call_id.is_some(),
        Role::Assistant if !raw.tool_calls.is_empty() => projected.content == raw.content
            && projected.tool_calls.len() == raw.tool_calls.len()
            && projected.tool_calls.iter().zip(&raw.tool_calls)
                .all(|(left, right)| left.id == right.id && left.name == right.name),
        _ => projected == raw,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CompactionLimits {
    pub generation_input_limit_tokens: u32,
    pub target_active_tokens: u32,
}

impl Default for CompactionLimits {
    fn default() -> Self {
        Self {
            generation_input_limit_tokens: 32_000,
            target_active_tokens: 64_000,
        }
    }
}

fn digest(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn source_item_id(index: usize, message: &Message) -> Result<String, CompactionError> {
    let body = serde_json::to_string(message)
        .map_err(|error| CompactionError::invalid(format!("message serialize failed: {error}")))?;
    Ok(format!(
        "source.{index}.{}",
        digest(&body).replace(':', "-")
    ))
}

fn source_history_revision(items: &[(usize, String)]) -> String {
    let body = items
        .iter()
        .map(|(index, id)| format!("{index}:{id}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("history.{}", digest(&body).replace(':', "-"))
}

fn evidence_refs_from_receipt(receipt: &HistoricalToolReceipt) -> Vec<EvidenceRef> {
    let mut refs = receipt
        .accepted_evidence
        .iter()
        .map(|range| EvidenceRef::LidRange {
            start_lid: range.start_lid.clone(),
            end_lid: range.end_lid.clone(),
        })
        .chain(
            receipt
                .source_refs
                .iter()
                .cloned()
                .map(|source_ref_id| EvidenceRef::Source { source_ref_id }),
        )
        .collect::<Vec<_>>();
    refs.sort();
    refs.dedup();
    refs
}

fn assistant_source_content(message: &Message) -> Result<String, CompactionError> {
    if message.tool_calls.is_empty() {
        return Ok(message.content.clone().unwrap_or_default());
    }
    serde_json::to_string(&serde_json::json!({
        "text": message.content,
        "tool_calls": message.tool_calls,
    }))
    .map_err(|error| CompactionError::invalid(format!("assistant source failed: {error}")))
}

fn validate_message_projection(
    raw: &[Message],
    deterministic: &[Message],
) -> Result<(), CompactionError> {
    if raw.len() != deterministic.len() {
        return Err(CompactionError::invalid(
            "raw and deterministic histories have different lengths",
        ));
    }
    for (index, (raw, deterministic)) in raw.iter().zip(deterministic).enumerate() {
        if raw.role != deterministic.role
            || raw.tool_call_id != deterministic.tool_call_id
            || raw.tool_calls.len() != deterministic.tool_calls.len()
            || raw
                .tool_calls
                .iter()
                .zip(&deterministic.tool_calls)
                .any(|(left, right)| left.id != right.id || left.name != right.name)
        {
            return Err(CompactionError::invalid(format!(
                "deterministic history changed message identity at index {index}"
            )));
        }
    }
    Ok(())
}

fn turn_is_complete(messages: &[Message]) -> Result<bool, CompactionError> {
    let mut calls = BTreeMap::new();
    let mut results = HashSet::new();
    for message in messages {
        match message.role {
            Role::Assistant => {
                for call in &message.tool_calls {
                    if call.id.trim().is_empty() || calls.insert(call.id.clone(), ()).is_some() {
                        return Err(CompactionError::invalid(
                            "tool call IDs must be nonempty and unique within a turn",
                        ));
                    }
                }
            }
            Role::Tool => {
                let id = message
                    .tool_call_id
                    .as_deref()
                    .ok_or_else(|| CompactionError::invalid("tool result has no tool_call_id"))?;
                if !calls.contains_key(id) || !results.insert(id.to_string()) {
                    return Err(CompactionError::invalid(
                        "tool result is orphaned or duplicated",
                    ));
                }
            }
            Role::System | Role::User => {}
        }
    }
    let pairs_complete = calls.keys().all(|id| results.contains(id));
    let has_final = messages
        .last()
        .is_some_and(|message| message.role == Role::Assistant && message.tool_calls.is_empty());
    Ok(pairs_complete && has_final)
}

fn conversation_turns(messages: &[Message], phase: CompactionPhase) -> Result<Vec<(usize, usize, bool)>, CompactionError> {
    let system_end = messages
        .iter()
        .position(|message| message.role != Role::System)
        .unwrap_or(messages.len());
    if messages[system_end..]
        .iter()
        .any(|message| message.role == Role::System)
    {
        return Err(CompactionError::invalid(
            "durable system messages may only appear in the canonical prefix",
        ));
    }
    if system_end == messages.len() {
        return Ok(Vec::new());
    }
    if messages[system_end].role != Role::User {
        return Err(CompactionError::invalid(
            "conversation history must start each turn with a user message",
        ));
    }
    let starts = messages
        .iter()
        .enumerate()
        .skip(system_end)
        .filter_map(|(index, message)| (message.role == Role::User).then_some(index))
        .collect::<Vec<_>>();
    let mut turns = Vec::with_capacity(starts.len());
    for (ordinal, start) in starts.iter().copied().enumerate() {
        let end = starts.get(ordinal + 1).copied().unwrap_or(messages.len());
        // A later user turn, or the pre-turn boundary before appending that
        // user, closes a prior user-only turn left by a failed run. Cover its
        // question once without inventing an answer or consuming current text.
        let past_user_only = end == start + 1
            && (end < messages.len() || phase == CompactionPhase::PreTurn);
        turns.push((
            start,
            end,
            turn_is_complete(&messages[start..end])? || past_user_only,
        ));
    }
    Ok(turns)
}

pub fn prepare_compaction(
    phase: CompactionPhase,
    raw_messages: &[Message],
    deterministic_messages: &[Message],
    allowed_evidence_refs: Vec<EvidenceRef>,
    allowed_supersession_edges: Vec<AllowedSupersession>,
    pending_effects: Vec<PendingEffectRef>,
    context_revisions: BTreeMap<String, String>,
) -> Result<PreparedCompaction, CompactionError> {
    prepare_compaction_with_tail_budget(
        phase, raw_messages, deterministic_messages, allowed_evidence_refs,
        allowed_supersession_edges, pending_effects, context_revisions, u32::MAX,
    )
}

/// Select completed tool steps only when they lie before the recent token tail.
/// A parallel batch is one indivisible step, including any accompanying text.
/// The current user message is never selected, even when its steps are selected.
fn completed_step_prefix(
    messages: &[Message],
    current_start: usize,
    retain_recent_tokens: u32,
) -> Result<Vec<(usize, usize, bool)>, CompactionError> {
    if retain_recent_tokens == u32::MAX { return Ok(Vec::new()); }
    let mut groups = Vec::new();
    let mut pending = HashSet::new();
    let mut group_start = current_start + 1;
    for (index, message) in messages.iter().enumerate().skip(group_start) {
        match message.role {
            Role::Assistant => {
                for call in &message.tool_calls { pending.insert(call.id.as_str()); }
            }
            Role::Tool => {
                if let Some(id) = message.tool_call_id.as_deref() { pending.remove(id); }
                if pending.is_empty() {
                    groups.push((group_start, index + 1, true));
                    group_start = index + 1;
                }
            }
            Role::User | Role::System => {}
        }
    }
    // Always keep the latest closed step; an unclosed batch following it is
    // retained in full too. This also prevents an empty current working tail.
    let mut retained = message_tokens(&messages[group_start..])?;
    let mut keep_from = groups.len();
    for (ordinal, (start, end, _)) in groups.iter().enumerate().rev() {
        if keep_from < groups.len() && retained >= retain_recent_tokens { break; }
        retained = retained.saturating_add(message_tokens(&messages[*start..*end])?);
        keep_from = ordinal;
    }
    groups.truncate(keep_from);
    Ok(groups)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_compaction_with_tail_budget(
    phase: CompactionPhase,
    raw_messages: &[Message],
    deterministic_messages: &[Message],
    allowed_evidence_refs: Vec<EvidenceRef>,
    allowed_supersession_edges: Vec<AllowedSupersession>,
    pending_effects: Vec<PendingEffectRef>,
    context_revisions: BTreeMap<String, String>,
    retain_recent_tokens: u32,
) -> Result<PreparedCompaction, CompactionError> {
    if phase == CompactionPhase::HierarchicalMerge {
        return Err(CompactionError::invalid(
            "hierarchical merge requests are runtime-generated only",
        ));
    }
    validate_message_projection(raw_messages, deterministic_messages)?;
    let turns = conversation_turns(raw_messages, phase)?;
    if turns.is_empty() {
        return Err(CompactionError::not_applicable(
            "history contains no conversation turn to compact",
        ));
    }

    let raw_start = match phase {
        CompactionPhase::MidTurn => turns
            .last()
            .map(|turn| turn.0)
            .unwrap_or(raw_messages.len()),
        CompactionPhase::PreTurn => turns
            .iter()
            .find_map(|(start, _, complete)| (!complete).then_some(*start))
            .unwrap_or(raw_messages.len()),
        CompactionPhase::HierarchicalMerge => unreachable!(),
    };
    let mut compactable_turns = turns
        .iter()
        .take_while(|(_, end, complete)| *complete && *end <= raw_start)
        .copied()
        .collect::<Vec<_>>();
    if !compactable_turns.is_empty()
        && compactable_turns.last().map(|turn| turn.1) != Some(raw_start)
    {
        return Err(CompactionError::invalid(
            "compaction may not cross an incomplete turn boundary",
        ));
    }
    if phase == CompactionPhase::MidTurn
        && turns.iter().take(turns.len() - 1).all(|turn| turn.2)
    {
        compactable_turns.extend(completed_step_prefix(
            raw_messages, raw_start, retain_recent_tokens,
        )?);
    }
    if compactable_turns.is_empty() {
        return Err(CompactionError::not_applicable(
            "history contains no completed turn or tool step before the retained tail",
        ));
    }

    let all_ids = raw_messages
        .iter()
        .enumerate()
        .filter(|(_, message)| message.role != Role::System)
        .map(|(index, message)| source_item_id(index, message).map(|id| (index, id)))
        .collect::<Result<Vec<_>, _>>()?;
    let id_by_index = all_ids.iter().cloned().collect::<BTreeMap<_, _>>();
    let mut eligible_items = Vec::new();
    let mut required_source_ids = Vec::new();
    let mut optional_source_ids = Vec::new();
    let mut derived_evidence_refs = BTreeSet::new();
    let mut tool_receipts = Vec::new();

    let source_indices = compactable_turns
        .iter()
        .flat_map(|(start, end, _)| *start..*end)
        .collect::<Vec<_>>();
    for &index in &source_indices {
        let raw = &raw_messages[index];
        let deterministic = &deterministic_messages[index];
        let id = id_by_index
            .get(&index)
            .cloned()
            .ok_or_else(|| CompactionError::invalid("missing source item identity"))?;
        let (content, evidence_refs, required) = match raw.role {
            Role::User => (raw.content.clone().unwrap_or_default(), Vec::new(), true),
            Role::Assistant => {
                let content = assistant_source_content(raw)?;
                let required = raw
                    .content
                    .as_deref()
                    .is_some_and(|text| !text.trim().is_empty());
                (content, Vec::new(), required)
            }
            Role::Tool => {
                let receipt = serde_json::from_str::<HistoricalToolReceipt>(
                    deterministic.content.as_deref().unwrap_or_default(),
                )
                .map_err(|error| {
                    CompactionError::invalid(format!(
                        "eligible tool result is not a deterministic receipt: {error}"
                    ))
                })?;
                // conversation_turns already validated call/result uniqueness
                // within each user turn. Across turns the same provider call ID
                // can recur; source_item_id keeps each occurrence distinct.
                let refs = evidence_refs_from_receipt(&receipt);
                let required = !refs.is_empty();
                let content = serde_json::to_string(&receipt).map_err(|error| {
                    CompactionError::invalid(format!("receipt serialize failed: {error}"))
                })?;
                tool_receipts.push(receipt);
                (content, refs, required)
            }
            Role::System => continue,
        };
        for evidence_ref in &evidence_refs {
            derived_evidence_refs.insert(evidence_ref.clone());
        }
        if required {
            required_source_ids.push(id.clone());
        } else {
            optional_source_ids.push(id.clone());
        }
        eligible_items.push(CompactionSourceItem {
            source_item_id: id,
            role: raw.role,
            content,
            evidence_refs,
        });
    }

    let mut allowed_evidence_refs = allowed_evidence_refs
        .into_iter()
        .chain(derived_evidence_refs)
        .collect::<Vec<_>>();
    allowed_evidence_refs.sort();
    allowed_evidence_refs.dedup();
    let selected_indices = source_indices.iter().copied().collect::<BTreeSet<_>>();
    let raw_retained_item_ids = all_ids
        .iter()
        .filter_map(|(index, id)| (!selected_indices.contains(index)).then_some(id.clone()))
        .collect::<Vec<_>>();
    let full_turn_source_ids = compactable_turns
        .iter()
        .map(|(start, end, _)| {
            (*start..*end)
                .filter_map(|index| id_by_index.get(&index).cloned())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let request = CompactionRequest {
        schema_version: COMPACTION_REQUEST_VERSION.into(),
        prompt_version: COMPACTION_PROMPT_VERSION.into(),
        phase,
        source_history_revision: source_history_revision(&all_ids),
        eligible_items,
        required_source_ids,
        optional_source_ids,
        raw_retained_item_ids,
        allowed_evidence_refs,
        allowed_supersession_edges,
    };
    validate_request(&request)?;
    Ok(PreparedCompaction {
        request,
        eligible_messages: source_indices.iter().map(|index| deterministic_messages[*index].clone()).collect(),
        full_turn_source_ids,
        required_semantic_states: BTreeMap::new(),
        tool_receipts,
        pending_effects,
        context_revisions,
        source_indices,
        generation_prefix: None,
    })
}

fn validate_request(request: &CompactionRequest) -> Result<(), CompactionError> {
    if request.schema_version != COMPACTION_REQUEST_VERSION
        || request.prompt_version != COMPACTION_PROMPT_VERSION
        || request.source_history_revision.trim().is_empty()
    {
        return Err(CompactionError::invalid(
            "compaction request version or history revision is invalid",
        ));
    }
    let eligible = request
        .eligible_items
        .iter()
        .map(|item| item.source_item_id.as_str())
        .collect::<BTreeSet<_>>();
    if eligible.len() != request.eligible_items.len()
        || request
            .eligible_items
            .iter()
            .any(|item| item.source_item_id.trim().is_empty() || item.content.trim().is_empty())
    {
        return Err(CompactionError::invalid(
            "eligible source IDs/content must be nonempty and unique",
        ));
    }
    let required = request
        .required_source_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let optional = request
        .optional_source_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if required.len() != request.required_source_ids.len()
        || optional.len() != request.optional_source_ids.len()
        || !required.is_disjoint(&optional)
        || required.union(&optional).copied().collect::<BTreeSet<_>>() != eligible
    {
        return Err(CompactionError::invalid(
            "required and optional source partitions must exactly cover eligible items",
        ));
    }
    let raw = request
        .raw_retained_item_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if raw.len() != request.raw_retained_item_ids.len() || !raw.is_disjoint(&eligible) {
        return Err(CompactionError::invalid(
            "raw-retained IDs must be unique and outside eligible items",
        ));
    }
    let allowed_refs = request
        .allowed_evidence_refs
        .iter()
        .collect::<BTreeSet<_>>();
    if request.eligible_items.iter().any(|item| {
        item.evidence_refs
            .iter()
            .any(|reference| !allowed_refs.contains(reference))
    }) {
        return Err(CompactionError::invalid(
            "source item contains an evidence reference outside the request whitelist",
        ));
    }
    let edges = request
        .allowed_supersession_edges
        .iter()
        .map(|edge| {
            (
                edge.earlier_source_item_id.as_str(),
                edge.later_source_item_id.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();
    if edges.len() != request.allowed_supersession_edges.len()
        || edges
            .iter()
            .any(|(earlier, later)| !eligible.contains(earlier) || !eligible.contains(later))
    {
        return Err(CompactionError::invalid(
            "supersession edges must be unique and refer to eligible sources",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SemanticSection {
    ActiveGoal,
    Progress,
    Decisions,
    UserConstraints,
    OpenObligations,
    UnresolvedAmbiguities,
    CriticalFacts,
    CriticalExamples,
    NextSteps,
}

impl SemanticSection {
    fn as_str(self) -> &'static str {
        match self {
            Self::ActiveGoal => "active_goal",
            Self::Progress => "progress",
            Self::Decisions => "decisions",
            Self::UserConstraints => "user_constraints",
            Self::OpenObligations => "open_obligations",
            Self::UnresolvedAmbiguities => "unresolved_ambiguities",
            Self::CriticalFacts => "critical_facts",
            Self::CriticalExamples => "critical_examples",
            Self::NextSteps => "next_steps",
        }
    }
}

fn semantic_items(draft: &CompactionDraft) -> Vec<(SemanticSection, &SourcedCheckpointItem)> {
    [
        (SemanticSection::ActiveGoal, &draft.active_goal),
        (SemanticSection::Progress, &draft.progress),
        (SemanticSection::Decisions, &draft.decisions),
        (SemanticSection::UserConstraints, &draft.user_constraints),
        (SemanticSection::OpenObligations, &draft.open_obligations),
        (
            SemanticSection::UnresolvedAmbiguities,
            &draft.unresolved_ambiguities,
        ),
        (SemanticSection::CriticalFacts, &draft.critical_facts),
        (SemanticSection::CriticalExamples, &draft.critical_examples),
        (SemanticSection::NextSteps, &draft.next_steps),
    ]
    .into_iter()
    .flat_map(|(section, items)| items.iter().map(move |item| (section, item)))
    .collect()
}

fn semantic_items_mut(
    draft: &mut CompactionDraft,
) -> Vec<(SemanticSection, &mut SourcedCheckpointItem)> {
    let CompactionDraft {
        active_goal,
        progress,
        decisions,
        user_constraints,
        open_obligations,
        unresolved_ambiguities,
        critical_facts,
        critical_examples,
        next_steps,
        source_coverage: _,
    } = draft;
    active_goal
        .iter_mut()
        .map(|item| (SemanticSection::ActiveGoal, item))
        .chain(
            progress
                .iter_mut()
                .map(|item| (SemanticSection::Progress, item)),
        )
        .chain(
            decisions
                .iter_mut()
                .map(|item| (SemanticSection::Decisions, item)),
        )
        .chain(
            user_constraints
                .iter_mut()
                .map(|item| (SemanticSection::UserConstraints, item)),
        )
        .chain(
            open_obligations
                .iter_mut()
                .map(|item| (SemanticSection::OpenObligations, item)),
        )
        .chain(
            unresolved_ambiguities
                .iter_mut()
                .map(|item| (SemanticSection::UnresolvedAmbiguities, item)),
        )
        .chain(
            critical_facts
                .iter_mut()
                .map(|item| (SemanticSection::CriticalFacts, item)),
        )
        .chain(
            critical_examples
                .iter_mut()
                .map(|item| (SemanticSection::CriticalExamples, item)),
        )
        .chain(
            next_steps
                .iter_mut()
                .map(|item| (SemanticSection::NextSteps, item)),
        )
        .collect()
}

fn valid_generated_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
}

fn validate_draft(
    request: &CompactionRequest,
    draft: &CompactionDraft,
    required_states: &BTreeMap<String, RequiredSemanticState>,
) -> Result<(), CompactionError> {
    validate_request(request)?;
    let eligible = request
        .eligible_items
        .iter()
        .map(|item| (item.source_item_id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let required = request
        .required_source_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let optional = request
        .optional_source_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let allowed_refs = request
        .allowed_evidence_refs
        .iter()
        .collect::<BTreeSet<_>>();
    let edges = request
        .allowed_supersession_edges
        .iter()
        .map(|edge| {
            (
                edge.earlier_source_item_id.as_str(),
                edge.later_source_item_id.as_str(),
            )
        })
        .collect::<BTreeSet<_>>();

    let items = semantic_items(draft);
    let mut item_ids = BTreeMap::new();
    let mut errors = Vec::new();
    for (section, item) in &items {
        if !valid_generated_id(&item.item_id) {
            errors.push(format!(
                "{}.item_id {:?} must use 1-128 ASCII letters, digits, dots, underscores, colons or hyphens; rename it and update every source_coverage.target_item_ids reference",
                section.as_str(), item.item_id
            ));
        }
        if item.text.trim().is_empty()
            || item.text.chars().count() > 4_000
            || item.source_item_ids.is_empty()
        {
            errors.push(format!(
                "{} item {:?} requires nonempty text of at most 4000 characters and nonempty source_item_ids",
                section.as_str(), item.item_id
            ));
        }
        if item_ids
            .insert(item.item_id.as_str(), (*section, *item))
            .is_some()
        {
            errors.push(format!(
                "duplicate semantic item_id {}",
                item.item_id
            ));
        }
        let sources = item
            .source_item_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if sources.len() != item.source_item_ids.len()
            || sources.iter().any(|source| !eligible.contains_key(source))
        {
            errors.push(format!(
                "semantic item {} invented or duplicated a source ID",
                item.item_id
            ));
        }
        let refs = item.evidence_refs.iter().collect::<BTreeSet<_>>();
        if refs.len() != item.evidence_refs.len() {
            errors.push(format!(
                "semantic item {}.evidence_refs contains duplicate references; keep each reference once",
                item.item_id
            ));
        }
        for reference in refs.difference(&allowed_refs) {
            errors.push(format!(
                "semantic item {}.evidence_refs contains {}; copy exact entries from allowed_evidence_refs without merging, splitting or deriving ranges",
                item.item_id, serde_json::to_string(reference).expect("EvidenceRef is serializable")
            ));
        }
    }

    for (source_id, required_state) in required_states {
        let expected = match required_state {
            RequiredSemanticState::OpenObligation => SemanticSection::OpenObligations,
            RequiredSemanticState::UnresolvedAmbiguity => SemanticSection::UnresolvedAmbiguities,
        };
        let matching = items
            .iter()
            .filter(|(_, item)| item.source_item_ids.contains(source_id))
            .map(|(section, _)| *section)
            .collect::<BTreeSet<_>>();
        if !matching.contains(&expected)
            || matching.contains(&SemanticSection::Progress)
            || matching.contains(&SemanticSection::Decisions)
        {
            errors.push(format!(
                "source {source_id} must remain in {} and cannot be marked complete",
                expected.as_str()
            ));
        }
    }

    let mut coverage_by_source = BTreeMap::new();
    for coverage in &draft.source_coverage {
        if coverage_by_source
            .insert(coverage.source_item_id.as_str(), coverage)
            .is_some()
        {
            errors.push(format!(
                "duplicate source coverage for {}",
                coverage.source_item_id
            ));
        }
    }
    if coverage_by_source.len() != eligible.len()
        || eligible
            .keys()
            .any(|source| !coverage_by_source.contains_key(source))
        || coverage_by_source
            .keys()
            .any(|source| !eligible.contains_key(source))
    {
        errors.push("source coverage must contain exactly one record for every eligible source".into());
    }

    const OPTIONAL_NON_TASK_REASONS: [&str; 3] =
        ["transport_only", "duplicate_envelope", "status_only"];
    for (source_id, coverage) in coverage_by_source {
        let targets = coverage
            .target_item_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if targets.len() != coverage.target_item_ids.len() {
            errors.push(format!("coverage for {source_id}.target_item_ids contains duplicate targets"));
        }
        let missing = targets.iter().filter(|target| !item_ids.contains_key(**target)).copied().collect::<Vec<_>>();
        if !missing.is_empty() {
            errors.push(format!(
                "coverage for {source_id}.target_item_ids references undefined semantic items {missing:?}; restore the source-supported items or use existing items whose source_item_ids include this source"
            ));
            // These targets cannot be dereferenced for the disposition checks.
            // The draft remains rejected; continue finding independent errors.
            continue;
        }
        match coverage.disposition {
            SourceDisposition::NonTask => {
                if !optional.contains(source_id)
                    || !targets.is_empty()
                    || !coverage
                        .reason
                        .as_deref()
                        .is_some_and(|reason| OPTIONAL_NON_TASK_REASONS.contains(&reason))
                {
                    errors.push(format!(
                        "NonTask coverage for {source_id} is not an allowed optional disposition"
                    ));
                }
            }
            SourceDisposition::Superseded => {
                if targets.is_empty() || coverage.reason.is_some() {
                    errors.push(format!(
                        "Superseded coverage for {source_id} must have targets and no free-form reason"
                    ));
                }
                let allowed = targets.iter().any(|target| {
                    let (_, item) = item_ids[target];
                    item.source_item_ids.iter().any(|later| {
                        edges.contains(&(source_id, later.as_str()))
                            && item
                                .source_item_ids
                                .iter()
                                .any(|source| source == source_id)
                    })
                });
                if !allowed {
                    errors.push(format!(
                        "Superseded coverage for {source_id} has no allowed edge"
                    ));
                }
            }
            SourceDisposition::Compacted | SourceDisposition::Duplicate => {
                if targets.is_empty()
                    || coverage.reason.is_some()
                    || !targets.iter().all(|target| {
                        let (_, item) = item_ids[target];
                        item.source_item_ids
                            .iter()
                            .any(|source| source == source_id)
                    })
                {
                    errors.push(format!(
                        "coverage for {source_id} does not map to a source-linked semantic item"
                    ));
                }
            }
        }
        if required.contains(source_id) && coverage.disposition == SourceDisposition::NonTask {
            errors.push(format!(
                "required source {source_id} cannot be NonTask"
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(CompactionError::invalid(errors.join("\n")))
    }
}

pub fn decode_compaction_draft_strict(input: &str) -> Result<CompactionDraft, CompactionError> {
    let value = serde_json::from_str::<serde_json::Value>(input.trim()).map_err(|error| {
        CompactionError::generation(format!(
            "compaction output must be exactly one JSON object: {error}"
        ))
    })?;
    serde_json::from_value(value).map_err(|error| {
        CompactionError::generation(format!("compaction draft schema mismatch: {error}"))
    })
}

fn generation_input(request: &CompactionRequest) -> Result<String, CompactionError> {
    // Versions, history revision and raw-retained identities belong to runtime
    // validation/installation. They provide no material for semantic generation.
    // Keep the source array byte-exact and ordered; phase metadata follows it.
    #[derive(Serialize)]
    struct Input<'a> {
        eligible_items: &'a [CompactionSourceItem],
        required_source_ids: &'a [String],
        optional_source_ids: &'a [String],
        allowed_evidence_refs: &'a [EvidenceRef],
        allowed_supersession_edges: &'a [AllowedSupersession],
        phase: CompactionPhase,
    }
    serde_json::to_string(&Input {
        eligible_items: &request.eligible_items,
        required_source_ids: &request.required_source_ids,
        optional_source_ids: &request.optional_source_ids,
        allowed_evidence_refs: &request.allowed_evidence_refs,
        allowed_supersession_edges: &request.allowed_supersession_edges,
        phase: request.phase,
    }).map_err(|error| CompactionError::invalid(format!("request serialize failed: {error}")))
}

fn request_input_tokens(user: &str, system: &str) -> u32 {
    estimate_text_tokens(system).saturating_add(estimate_text_tokens(user))
}

pub fn compaction_output_token_limit(profile: &ModelRuntimeProfile) -> u32 {
    profile.output_reserve_tokens
}

fn call_generator(
    adapter: &dyn ModelAdapter,
    profile: &ModelRuntimeProfile,
    system: &str,
    request: &CompactionRequest,
    required_states: &BTreeMap<String, RequiredSemanticState>,
    generation_input_limit_tokens: u32,
) -> Result<CompactionDraft, CompactionError> {
    let mut user = generation_input(request)?;
    let mut repairing = false;
    loop {
        let value = adapter
            .complete_structured(CompletionRequest {
                followup_user: None,
                images: Vec::new(),
                output_token_limit: Some(compaction_output_token_limit(profile)),
                reasoning_effort: profile.matched_model.starts_with("deepseek-").then(|| "low".into()),
                system: system.to_string(),
                user: user.clone(),
            })
            .map_err(|error| match error.spend_stop {
                Some(reason) => CompactionError { error_code: reason.code().into(), message: reason.message().into() },
                None => CompactionError::generation(error.message),
            })?;
        let checked = CompactionDraft::deserialize(&value).map_err(|error| {
            CompactionError::generation(format!("compaction draft schema mismatch: {error}"))
        }).and_then(|draft| {
            validate_draft(request, &draft, required_states)?;
            Ok(draft)
        });
        match checked {
            Ok(draft) => return Ok(draft),
            Err(error) => {
                if repairing { return Err(error); }
                // EX11 real revisions produced forbidden metadata and broken
                // coverage links. Give one validation-guided repair of the actual
                // rejected draft; never install or sanitize it locally.
                user.push_str("\n\n");
                user.push_str(&serde_json::json!({"runtime_repair_diagnostic": {
                    "error_code": error.error_code, "message": error.message,
                    "rejected_draft": value,
                }}).to_string());
                if request_input_tokens(&user, system) > generation_input_limit_tokens {
                    return Err(error);
                }
                repairing = true;
            }
        }
    }
}

fn prefix_generation_instruction(
    system: &str,
    request: &CompactionRequest,
    prefix: &CompactionGenerationPrefix,
) -> String {
    let sources = request.eligible_items.iter().zip(&prefix.source_positions).map(|(item, position)| {
        serde_json::json!({
            "source_item_id": item.source_item_id,
            "ordered_message_index": position,
            "role": item.role,
            "evidence_refs": item.evidence_refs,
        })
    }).collect::<Vec<_>>();
    let metadata = serde_json::json!({
        "eligible_items": sources,
        "required_source_ids": request.required_source_ids,
        "optional_source_ids": request.optional_source_ids,
        "allowed_evidence_refs": request.allowed_evidence_refs,
        "allowed_supersession_edges": request.allowed_supersession_edges,
        "phase": request.phase,
    });
    format!("{system}\n\nFor this checkpoint, eligible_items refer to the preceding messages by zero-based ordered_message_index (including the leading system message when present). Read their content in place; it is deliberately not duplicated here. Summarize only mapped sources. Unmapped instructions, runtime state and retained user material remain active context, not sources to cover. Do not call any tools. Return only the complete CompactionDraft JSON object.\n{metadata}")
}

fn prefix_generation_plan(
    adapter: &dyn ModelAdapter,
    prefix: &CompactionGenerationPrefix,
    instruction: &str,
) -> AgentRequestPlan {
    let mut plan = prefix.request.clone();
    plan.input.push(Message::user(instruction));
    let tokens = adapter.context_request_projection(&plan)
        .map(|value| crate::model_runtime::estimate_provider_request_input_tokens(&value))
        .unwrap_or_else(|| {
            estimate_text_tokens(&serde_json::to_string(&plan.ordered_messages()).unwrap_or_default())
                .saturating_add(estimate_text_tokens(&serde_json::to_string(&plan.tools).unwrap_or_default()))
        });
    plan.update_input_estimate(tokens);
    plan
}

fn call_prefix_generator(
    adapter: &dyn ModelAdapter,
    prepared: &PreparedCompaction,
    prefix: &CompactionGenerationPrefix,
    system: &str,
    generation_input_limit_tokens: u32,
) -> Result<CompactionDraft, CompactionError> {
    let mut instruction = prefix_generation_instruction(system, &prepared.request, prefix);
    let mut repairing = false;
    loop {
        let plan = prefix_generation_plan(adapter, prefix, &instruction);
        if plan.active_context.estimated_input_tokens > generation_input_limit_tokens || !plan.active_context.fits {
            return Err(CompactionError::generation("compaction prefix and instruction exceed the generator input budget"));
        }
        let value = crate::complete_structured_in_context(adapter, &plan).map_err(|error| match error.spend_stop {
            Some(reason) => CompactionError { error_code: reason.code().into(), message: reason.message().into() },
            None => CompactionError::generation(error.message),
        })?;
        let checked = CompactionDraft::deserialize(&value).map_err(|error| {
            CompactionError::generation(format!("compaction draft schema mismatch: {error}"))
        }).and_then(|draft| {
            validate_draft(&prepared.request, &draft, &prepared.required_semantic_states)?;
            Ok(draft)
        });
        match checked {
            Ok(draft) => return Ok(draft),
            Err(error) if repairing => return Err(error),
            Err(error) => {
                instruction.push_str("\n\n");
                instruction.push_str(&serde_json::json!({"runtime_repair_diagnostic": {
                    "error_code": error.error_code, "message": error.message,
                    "rejected_draft": value,
                }}).to_string());
                let repair = prefix_generation_plan(adapter, prefix, &instruction);
                if repair.active_context.estimated_input_tokens > generation_input_limit_tokens || !repair.active_context.fits {
                    return Err(error);
                }
                repairing = true;
            }
        }
    }
}

fn child_request(
    original: &CompactionRequest,
    source_ids: &[String],
) -> Result<CompactionRequest, CompactionError> {
    let selected = source_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let eligible_items = original
        .eligible_items
        .iter()
        .filter(|item| selected.contains(item.source_item_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let allowed_evidence_refs = original.allowed_evidence_refs.clone();
    let request = CompactionRequest {
        schema_version: original.schema_version.clone(),
        prompt_version: original.prompt_version.clone(),
        phase: original.phase,
        source_history_revision: original.source_history_revision.clone(),
        eligible_items,
        required_source_ids: original
            .required_source_ids
            .iter()
            .filter(|id| selected.contains(id.as_str()))
            .cloned()
            .collect(),
        optional_source_ids: original
            .optional_source_ids
            .iter()
            .filter(|id| selected.contains(id.as_str()))
            .cloned()
            .collect(),
        raw_retained_item_ids: original.raw_retained_item_ids.clone(),
        allowed_evidence_refs,
        allowed_supersession_edges: original
            .allowed_supersession_edges
            .iter()
            .filter(|edge| {
                selected.contains(edge.earlier_source_item_id.as_str())
                    && selected.contains(edge.later_source_item_id.as_str())
            })
            .cloned()
            .collect(),
    };
    validate_request(&request)?;
    Ok(request)
}

#[derive(Debug, Clone)]
struct MergeSource {
    source: CompactionSourceItem,
    original_source_ids: Vec<String>,
    required_state: Option<RequiredSemanticState>,
    child_index: usize,
    child_item_id: String,
}

fn hierarchical_merge_request(
    original: &CompactionRequest,
    child_drafts: &[CompactionDraft],
) -> Result<(CompactionRequest, Vec<MergeSource>), CompactionError> {
    let mut sources = Vec::new();
    for (child_index, draft) in child_drafts.iter().enumerate() {
        for (section, item) in semantic_items(draft) {
            let source_item_id = format!("merge.{child_index}.{}", item.item_id);
            let required_state = match section {
                SemanticSection::OpenObligations => Some(RequiredSemanticState::OpenObligation),
                SemanticSection::UnresolvedAmbiguities => {
                    Some(RequiredSemanticState::UnresolvedAmbiguity)
                }
                _ => None,
            };
            sources.push(MergeSource {
                source: CompactionSourceItem {
                    source_item_id,
                    role: Role::System,
                    content: format!("section={}\n{}", section.as_str(), item.text),
                    evidence_refs: item.evidence_refs.clone(),
                },
                original_source_ids: item.source_item_ids.clone(),
                required_state,
                child_index,
                child_item_id: item.item_id.clone(),
            });
        }
    }
    if sources.is_empty() {
        return Err(CompactionError::invalid(
            "hierarchical child drafts produced no semantic state",
        ));
    }
    let request = CompactionRequest {
        schema_version: COMPACTION_REQUEST_VERSION.into(),
        prompt_version: COMPACTION_PROMPT_VERSION.into(),
        phase: CompactionPhase::HierarchicalMerge,
        source_history_revision: original.source_history_revision.clone(),
        eligible_items: sources.iter().map(|source| source.source.clone()).collect(),
        required_source_ids: sources
            .iter()
            .map(|source| source.source.source_item_id.clone())
            .collect(),
        optional_source_ids: Vec::new(),
        raw_retained_item_ids: original.raw_retained_item_ids.clone(),
        allowed_evidence_refs: original.allowed_evidence_refs.clone(),
        allowed_supersession_edges: Vec::new(),
    };
    validate_request(&request)?;
    Ok((request, sources))
}

fn expand_hierarchical_draft(
    original: &CompactionRequest,
    child_drafts: &[CompactionDraft],
    merge_sources: &[MergeSource],
    mut merged: CompactionDraft,
) -> Result<CompactionDraft, CompactionError> {
    let source_map = merge_sources
        .iter()
        .map(|source| {
            (
                source.source.source_item_id.clone(),
                source.original_source_ids.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let merged_target_map = semantic_items(&merged)
        .into_iter()
        .map(|(_, item)| (item.item_id.clone(), item.source_item_ids.clone()))
        .collect::<BTreeMap<_, _>>();
    for (_, item) in semantic_items_mut(&mut merged) {
        let mut expanded = item
            .source_item_ids
            .iter()
            .flat_map(|source| source_map.get(source).cloned().unwrap_or_default())
            .collect::<Vec<_>>();
        expanded.sort();
        expanded.dedup();
        item.source_item_ids = expanded;
    }

    let child_item_to_merge_source = merge_sources
        .iter()
        .map(|source| {
            (
                (source.child_index, source.child_item_id.as_str()),
                source.source.source_item_id.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut original_coverage = BTreeMap::new();
    for (child_index, child) in child_drafts.iter().enumerate() {
        for coverage in &child.source_coverage {
            original_coverage.insert(
                coverage.source_item_id.clone(),
                (child_index, coverage.clone()),
            );
        }
    }
    let mut source_coverage = Vec::with_capacity(original.eligible_items.len());
    for source in &original.eligible_items {
        let (child_index, child) = original_coverage
            .get(&source.source_item_id)
            .ok_or_else(|| CompactionError::invalid("hierarchical coverage lost a source"))?;
        if child.disposition == SourceDisposition::NonTask {
            source_coverage.push(child.clone());
            continue;
        }
        let child_merge_sources = child
            .target_item_ids
            .iter()
            .filter_map(|item_id| {
                child_item_to_merge_source
                    .get(&(*child_index, item_id.as_str()))
                    .copied()
            })
            .collect::<BTreeSet<_>>();
        let targets = merged_target_map
            .iter()
            .filter_map(|(item_id, merge_ids)| {
                merge_ids
                    .iter()
                    .any(|source_id| child_merge_sources.contains(source_id.as_str()))
                    .then_some(item_id.clone())
            })
            .collect::<Vec<_>>();
        source_coverage.push(SourceCoverage {
            source_item_id: source.source_item_id.clone(),
            disposition: child.disposition,
            target_item_ids: targets,
            reason: child.reason.clone(),
        });
    }
    merged.source_coverage = source_coverage;
    Ok(merged)
}

fn generate_draft(
    adapter: &dyn ModelAdapter,
    profile: &ModelRuntimeProfile,
    prepared: &PreparedCompaction,
    generation_input_limit_tokens: u32,
) -> Result<CompactionDraft, CompactionError> {
    if profile.compaction.output_schema_id != COMPACTION_DRAFT_SCHEMA_ID {
        return Err(CompactionError::invalid(
            "model profile does not use the canonical compaction draft schema",
        ));
    }
    let system = profile
        .compaction
        .prompt_asset
        .resolve(COMPACTION_GENERATION_PROMPT);
    if let Some(prefix) = prepared.generation_prefix.as_ref().filter(|prefix| {
        prefix.request.runtime_profile.matched_model == profile.matched_model
            && prefix.request.runtime_profile == *profile
    }) {
        let instruction = prefix_generation_instruction(&system, &prepared.request, prefix);
        let plan = prefix_generation_plan(adapter, prefix, &instruction);
        if plan.active_context.fits && plan.active_context.estimated_input_tokens <= generation_input_limit_tokens {
            return call_prefix_generator(adapter, prepared, prefix, &system, generation_input_limit_tokens);
        }
    }
    if request_input_tokens(&generation_input(&prepared.request)?, &system) <= generation_input_limit_tokens {
        return call_generator(
            adapter,
            profile,
            &system,
            &prepared.request,
            &prepared.required_semantic_states,
            generation_input_limit_tokens,
        );
    }

    let mut chunks = Vec::<Vec<String>>::new();
    for turn in &prepared.full_turn_source_ids {
        let mut candidate = chunks.last().cloned().unwrap_or_default();
        candidate.extend(turn.iter().cloned());
        let candidate_request = child_request(&prepared.request, &candidate)?;
        if request_input_tokens(&generation_input(&candidate_request)?, &system) <= generation_input_limit_tokens {
            if let Some(last) = chunks.last_mut() {
                *last = candidate;
            } else {
                chunks.push(candidate);
            }
            continue;
        }
        let single_turn = child_request(&prepared.request, turn)?;
        if request_input_tokens(&generation_input(&single_turn)?, &system) > generation_input_limit_tokens {
            return Err(CompactionError {
                error_code: "COMPACTION_SOURCE_TURN_TOO_LARGE".into(),
                message: "one complete source turn or closed tool step cannot fit the compaction generator".into(),
            });
        }
        chunks.push(turn.clone());
    }
    if chunks.len() < 2 {
        return Err(CompactionError::invalid(
            "oversized compaction request did not produce multiple closed-source chunks",
        ));
    }

    let mut child_drafts = Vec::with_capacity(chunks.len());
    for chunk in &chunks {
        let request = child_request(&prepared.request, chunk)?;
        let states = prepared
            .required_semantic_states
            .iter()
            .filter(|(source, _)| chunk.contains(source))
            .map(|(source, state)| (source.clone(), *state))
            .collect();
        child_drafts.push(call_generator(adapter, profile, &system, &request, &states, generation_input_limit_tokens)?);
    }
    let (merge_request, merge_sources) =
        hierarchical_merge_request(&prepared.request, &child_drafts)?;
    if request_input_tokens(&generation_input(&merge_request)?, &system) > generation_input_limit_tokens {
        return Err(CompactionError {
            error_code: "COMPACTION_HIERARCHICAL_MERGE_TOO_LARGE".into(),
            message: "hierarchical child drafts still exceed the merge generator limit".into(),
        });
    }
    let merge_states = merge_sources
        .iter()
        .filter_map(|source| {
            source
                .required_state
                .map(|state| (source.source.source_item_id.clone(), state))
        })
        .collect();
    let merged = call_generator(adapter, profile, &system, &merge_request, &merge_states, generation_input_limit_tokens)?;
    let expanded =
        expand_hierarchical_draft(&prepared.request, &child_drafts, &merge_sources, merged)?;
    validate_draft(
        &prepared.request,
        &expanded,
        &prepared.required_semantic_states,
    )?;
    Ok(expanded)
}

fn message_tokens(messages: &[Message]) -> Result<u32, CompactionError> {
    let body = serde_json::to_string(messages)
        .map_err(|error| CompactionError::invalid(format!("message serialize failed: {error}")))?;
    Ok(estimate_text_tokens(&body))
}

fn checkpoint_window_id(checkpoint: &CompactionCheckpoint) -> Result<String, CompactionError> {
    let identity = serde_json::to_string(&serde_json::json!({
        "schema_version": checkpoint.schema_version,
        "prompt_version": checkpoint.prompt_version,
        "source_history_revision": checkpoint.source_history_revision,
        "raw_retained_item_ids": checkpoint.raw_retained_item_ids,
        "semantic": checkpoint.semantic,
        "tool_receipts": checkpoint.tool_receipts,
        "pending_effects": checkpoint.pending_effects,
        "context_revisions": checkpoint.context_revisions,
    }))
    .map_err(|error| {
        CompactionError::invalid(format!("checkpoint identity serialize failed: {error}"))
    })?;
    Ok(format!("window.{}", digest(&identity).replace(':', "-")))
}

fn validate_checkpoint_structure(checkpoint: &CompactionCheckpoint) -> Result<(), CompactionError> {
    let items = semantic_items(&checkpoint.semantic);
    let item_ids = items
        .iter()
        .map(|(_, item)| item.item_id.as_str())
        .collect::<BTreeSet<_>>();
    if item_ids.len() != items.len()
        || items.iter().any(|(_, item)| {
            !valid_generated_id(&item.item_id)
                || item.text.trim().is_empty()
                || item.source_item_ids.is_empty()
        })
    {
        return Err(CompactionError::invalid(
            "checkpoint semantic items are invalid or duplicated",
        ));
    }
    let coverage_ids = checkpoint
        .semantic
        .source_coverage
        .iter()
        .map(|coverage| coverage.source_item_id.as_str())
        .collect::<BTreeSet<_>>();
    let raw_ids = checkpoint
        .raw_retained_item_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if coverage_ids.len() != checkpoint.semantic.source_coverage.len()
        || raw_ids.len() != checkpoint.raw_retained_item_ids.len()
        || !coverage_ids.is_disjoint(&raw_ids)
    {
        return Err(CompactionError::invalid(
            "checkpoint source coverage/raw identities are duplicated or overlap",
        ));
    }
    if items.iter().any(|(_, item)| {
        item.source_item_ids
            .iter()
            .any(|source| !coverage_ids.contains(source.as_str()))
    }) {
        return Err(CompactionError::invalid(
            "checkpoint semantic item references a source outside coverage",
        ));
    }
    for coverage in &checkpoint.semantic.source_coverage {
        let targets = coverage
            .target_item_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if targets.len() != coverage.target_item_ids.len()
            || targets.iter().any(|target| !item_ids.contains(target))
            || (coverage.disposition == SourceDisposition::NonTask && !targets.is_empty())
            || (coverage.disposition != SourceDisposition::NonTask && targets.is_empty())
        {
            return Err(CompactionError::invalid(
                "checkpoint source coverage targets are invalid",
            ));
        }
    }
    if checkpoint.tool_receipts.iter().any(|receipt| {
        receipt.version != "historical_tool_receipt.v1"
            || receipt.tool.trim().is_empty()
            || receipt.opaque_result_digest.trim().is_empty()
    }) {
        return Err(CompactionError::invalid(
            "checkpoint tool receipts are invalid or duplicated",
        ));
    }
    let effect_ids = checkpoint
        .pending_effects
        .iter()
        .map(|effect| effect.effect_id.as_str())
        .collect::<BTreeSet<_>>();
    if effect_ids.len() != checkpoint.pending_effects.len()
        || checkpoint
            .pending_effects
            .iter()
            .any(|effect| effect.effect_id.trim().is_empty() || effect.kind.trim().is_empty())
        || checkpoint
            .context_revisions
            .iter()
            .any(|(key, revision)| key.trim().is_empty() || revision.trim().is_empty())
    {
        return Err(CompactionError::invalid(
            "checkpoint pending effects or context revisions are invalid",
        ));
    }
    Ok(())
}

pub fn compact_with_adapter(
    adapter: &dyn ModelAdapter,
    profile: &ModelRuntimeProfile,
    prepared: &PreparedCompaction,
    limits: CompactionLimits,
) -> Result<CompactionCheckpoint, CompactionError> {
    let draft = generate_draft(
        adapter,
        profile,
        prepared,
        limits.generation_input_limit_tokens,
    )?;
    validate_draft(
        &prepared.request,
        &draft,
        &prepared.required_semantic_states,
    )?;
    let mut checkpoint = CompactionCheckpoint {
        schema_version: COMPACTION_CHECKPOINT_VERSION.into(),
        prompt_version: prepared.request.prompt_version.clone(),
        window_id: String::new(),
        source_history_revision: prepared.request.source_history_revision.clone(),
        raw_retained_item_ids: prepared.request.raw_retained_item_ids.clone(),
        semantic: draft,
        tool_receipts: prepared.tool_receipts.clone(),
        pending_effects: prepared.pending_effects.clone(),
        context_revisions: prepared.context_revisions.clone(),
    };
    checkpoint.window_id = checkpoint_window_id(&checkpoint)?;
    validate_checkpoint_structure(&checkpoint)?;
    // This is a validated candidate. Only the caller has the complete sampling
    // projection (live tool bodies, frozen context, tools and output reserve)
    // needed to decide whether to install it.
    Ok(checkpoint)
}

fn checkpoint_source_ids(checkpoint: &CompactionCheckpoint) -> BTreeSet<&str> {
    checkpoint
        .semantic
        .source_coverage
        .iter()
        .map(|coverage| coverage.source_item_id.as_str())
        .chain(checkpoint.raw_retained_item_ids.iter().map(String::as_str))
        .collect()
}

/// Model handoff content. Audit and source identities remain in the persisted
/// checkpoint and are validated before this projection is installed.
fn checkpoint_consumption_projection(
    semantic: &CompactionDraft,
    pending_effects: &[PendingEffectRef],
) -> serde_json::Value {
    let items = |section: &[SourcedCheckpointItem]| {
        section.iter().map(|item| serde_json::json!({
            "text": item.text,
            "evidence_refs": item.evidence_refs,
        })).collect::<Vec<_>>()
    };
    serde_json::json!({
        "semantic": {
            "active_goal": items(&semantic.active_goal),
            "progress": items(&semantic.progress),
            "decisions": items(&semantic.decisions),
            "user_constraints": items(&semantic.user_constraints),
            "open_obligations": items(&semantic.open_obligations),
            "unresolved_ambiguities": items(&semantic.unresolved_ambiguities),
            "critical_facts": items(&semantic.critical_facts),
            "critical_examples": items(&semantic.critical_examples),
            "next_steps": items(&semantic.next_steps),
        },
        "pending_effects": pending_effects,
    })
}

pub fn project_compaction_checkpoint_messages(
    raw_messages: &[Message],
    checkpoint: &CompactionCheckpoint,
    context_messages: &[Message],
    consumption_wrapper: &str,
) -> Result<Vec<Message>, CompactionError> {
    if checkpoint.schema_version != COMPACTION_CHECKPOINT_VERSION
        || checkpoint.prompt_version != COMPACTION_PROMPT_VERSION
        || checkpoint.window_id.trim().is_empty()
    {
        return Err(CompactionError::invalid(
            "checkpoint version or window identity is invalid",
        ));
    }
    validate_checkpoint_structure(checkpoint)?;
    if context_messages
        .iter()
        .any(|message| message.role != Role::System)
    {
        return Err(CompactionError::invalid(
            "checkpoint context fragments must project as system messages",
        ));
    }
    let system_end = raw_messages
        .iter()
        .position(|message| message.role != Role::System)
        .unwrap_or(raw_messages.len());
    let ids = raw_messages
        .iter()
        .enumerate()
        .filter(|(_, message)| message.role != Role::System)
        .map(|(index, message)| source_item_id(index, message).map(|id| (index, id)))
        .collect::<Result<Vec<_>, _>>()?;
    let by_id = ids
        .iter()
        .map(|(index, id)| (id.as_str(), *index))
        .collect::<BTreeMap<_, _>>();
    let covered = checkpoint_source_ids(checkpoint);
    if covered.len()
        != checkpoint.semantic.source_coverage.len() + checkpoint.raw_retained_item_ids.len()
        || covered.iter().any(|source| !by_id.contains_key(source))
    {
        return Err(CompactionError::invalid(
            "checkpoint source IDs are duplicated or stale",
        ));
    }
    let mut covered_items = covered
        .iter()
        .map(|source| (by_id[source], (*source).to_string()))
        .collect::<Vec<_>>();
    covered_items.sort_by_key(|(index, _)| *index);
    if source_history_revision(&covered_items) != checkpoint.source_history_revision {
        return Err(CompactionError::invalid(
            "checkpoint history revision does not match its source messages",
        ));
    }
    if checkpoint_window_id(checkpoint)? != checkpoint.window_id {
        return Err(CompactionError::invalid(
            "checkpoint window identity does not match its deterministic contents",
        ));
    }
    let max_covered = covered_items
        .iter()
        .map(|(index, _)| *index)
        .max()
        .ok_or_else(|| CompactionError::invalid("checkpoint covers no history messages"))?;
    if ids
        .iter()
        .any(|(index, id)| *index <= max_covered && !covered.contains(id.as_str()))
    {
        return Err(CompactionError::invalid(
            "checkpoint omitted a message inside its covered history prefix",
        ));
    }

    let checkpoint_json = serde_json::to_string(&checkpoint_consumption_projection(
        &checkpoint.semantic, &checkpoint.pending_effects,
    )).map_err(|error| {
        CompactionError::invalid(format!("checkpoint serialize failed: {error}"))
    })?;
    let checkpoint_message = Message::system(format!(
        "{CONTEXT_COMPACTION_ITEM_VERSION}\n{consumption_wrapper}\ncompaction_checkpoint:\n{checkpoint_json}"
    ));
    let raw_retained = checkpoint
        .raw_retained_item_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut projected = Vec::with_capacity(
        system_end + context_messages.len() + 1 + raw_retained.len() + raw_messages.len(),
    );
    projected.extend_from_slice(&raw_messages[..system_end]);
    projected.extend_from_slice(context_messages);
    projected.push(checkpoint_message);
    for (index, message) in raw_messages.iter().enumerate().skip(system_end) {
        let id = source_item_id(index, message)?;
        if raw_retained.contains(id.as_str()) || index > max_covered {
            projected.push(message.clone());
        }
    }
    Ok(projected)
}

pub fn validate_persisted_checkpoint(
    raw_messages: &[Message],
    checkpoint: &CompactionCheckpoint,
) -> Result<(), CompactionError> {
    project_compaction_checkpoint_messages(
        raw_messages,
        checkpoint,
        &[],
        COMPACTION_CONSUMPTION_WRAPPER,
    )
    .map(|_| ())
}

pub fn evidence_refs_from_ranges(ranges: &[EvidenceRange]) -> Vec<EvidenceRef> {
    ranges
        .iter()
        .map(|range| EvidenceRef::LidRange {
            start_lid: range.start_lid.clone(),
            end_lid: range.end_lid.clone(),
        })
        .collect()
}

#[cfg(test)]
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RecordedCompactionInput {
    pub phase: CompactionPhase,
    pub eligible_items: Vec<CompactionSourceItem>,
    pub required_source_ids: Vec<String>,
}

#[cfg(test)]
#[path = "compaction_recorded_tests.rs"]
mod recorded_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_runtime::ProviderToolProtocol;
    use crate::tool_result::HistoricalToolStatus;
    use crate::{AdapterError, AssistantTurn, ParsedResponse, ToolCall};
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;

    fn assistant(content: &str) -> Message {
        Message {
            provider_continuation: None,
            role: Role::Assistant,
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    fn completed_turn(user: &str, answer: &str) -> Vec<Message> {
        vec![Message::user(user), assistant(answer)]
    }

    fn empty_draft() -> CompactionDraft {
        CompactionDraft {
            active_goal: Vec::new(),
            progress: Vec::new(),
            decisions: Vec::new(),
            user_constraints: Vec::new(),
            open_obligations: Vec::new(),
            unresolved_ambiguities: Vec::new(),
            critical_facts: Vec::new(),
            critical_examples: Vec::new(),
            next_steps: Vec::new(),
            source_coverage: Vec::new(),
        }
    }

    fn covering_draft(request: &CompactionRequest, text: &str) -> CompactionDraft {
        covering_sources(&request.eligible_items, text)
    }

    fn covering_sources(sources: &[CompactionSourceItem], text: &str) -> CompactionDraft {
        let mut draft = empty_draft();
        let item_id = "item.state".to_string();
        draft.active_goal.push(SourcedCheckpointItem {
            item_id: item_id.clone(),
            text: text.into(),
            source_item_ids: sources
                .iter()
                .map(|item| item.source_item_id.clone())
                .collect(),
            evidence_refs: Vec::new(),
        });
        draft.source_coverage = sources
            .iter()
            .map(|source| SourceCoverage {
                source_item_id: source.source_item_id.clone(),
                disposition: SourceDisposition::Compacted,
                target_item_ids: vec![item_id.clone()],
                reason: None,
            })
            .collect();
        draft
    }

    struct ScriptedCompactor {
        outputs: RefCell<VecDeque<serde_json::Value>>,
        requests: RefCell<Vec<CompletionRequest>>,
    }

    impl ScriptedCompactor {
        fn new(outputs: Vec<serde_json::Value>) -> Self {
            Self {
                outputs: RefCell::new(outputs.into()),
                requests: RefCell::new(Vec::new()),
            }
        }
    }

    impl ModelAdapter for ScriptedCompactor {
        fn complete(&self, _req: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
            Err(AdapterError {
                spend_stop: None,
                message: "compaction must use structured completion".into(),
            })
        }

        fn complete_structured(
            &self,
            req: CompletionRequest,
        ) -> Result<serde_json::Value, AdapterError> {
            self.requests.borrow_mut().push(req);
            self.outputs
                .borrow_mut()
                .pop_front()
                .ok_or_else(|| AdapterError {
                    spend_stop: None,
                    message: "compaction script exhausted".into(),
                })
        }

        fn chat(&self, _request: &crate::AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
            Err(AdapterError {
                spend_stop: None,
                message: "compaction must not call chat or expose tools".into(),
            })
        }
    }

    struct RequestDrivenCompactor {
        calls: Cell<usize>,
    }

    impl ModelAdapter for RequestDrivenCompactor {
        fn complete(&self, _req: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
            Err(AdapterError {
                spend_stop: None,
                message: "compaction must use structured completion".into(),
            })
        }

        fn complete_structured(
            &self,
            req: CompletionRequest,
        ) -> Result<serde_json::Value, AdapterError> {
            self.calls.set(self.calls.get() + 1);
            let request =
                serde_json::from_str::<RecordedCompactionInput>(&req.user).map_err(|error| {
                    AdapterError {
                        spend_stop: None,
                        message: error.to_string(),
                    }
                })?;
            serde_json::to_value(covering_sources(&request.eligible_items, "merged state")).map_err(|error| {
                AdapterError {
                    spend_stop: None,
                    message: error.to_string(),
                }
            })
        }

        fn chat(&self, _request: &crate::AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
            Err(AdapterError {
                spend_stop: None,
                message: "compaction must not call chat or expose tools".into(),
            })
        }
    }

    fn profile() -> ModelRuntimeProfile {
        ModelRuntimeProfile::fallback("fixture", ProviderToolProtocol::Native)
    }

    struct PrefixCompactor {
        requests: RefCell<Vec<AgentRequestPlan>>,
        outputs: RefCell<VecDeque<serde_json::Value>>,
        return_tool: bool,
    }

    impl ModelAdapter for PrefixCompactor {
        fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
            panic!("a proven prefix must use its existing chat projection")
        }

        fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
            self.requests.borrow_mut().push(request.clone());
            Ok(AssistantTurn {
                provider_continuation: None,
                text: self.outputs.borrow_mut().pop_front().map(|value| value.to_string()),
                tool_calls: if self.return_tool { vec![ToolCall {
                    id: "forbidden".into(), name: "book.text".into(), arguments: "{}".into(),
                }] } else { vec![] },
                usage_total_tokens: None,
            })
        }
    }

    #[test]
    fn compaction_prefix_preserves_headers_messages_and_repair_without_duplicating_sources() {
        let mut raw = vec![Message::system("original system policy")];
        raw.extend(completed_turn("unique older question material", "unique older answer material"));
        raw.push(Message::user("current user stays verbatim"));
        let mut prepared = prepare_compaction(CompactionPhase::MidTurn, &raw, &raw,
            vec![], vec![], vec![], BTreeMap::new()).unwrap();
        let tools = vec![crate::ToolSpec { name: "book.text".into(), description: "Read".into(),
            parameters: serde_json::json!({"type":"object","properties":{"lid":{"type":"string"}}}) }];
        let plan = AgentRequestPlan::for_ad_hoc(profile(), &raw, &tools);
        assert!(prepared.bind_generation_prefix(&plan, &raw));
        let valid = serde_json::to_value(covering_draft(prepared.request(), "continue requested work")).unwrap();
        let mut invalid = valid.clone();
        invalid["schema_version"] = serde_json::json!("forbidden");
        let adapter = PrefixCompactor { requests: RefCell::new(vec![]),
            outputs: RefCell::new(vec![invalid, valid].into()), return_tool: false };
        let checkpoint = compact_with_adapter(&adapter, &profile(), &prepared,
            CompactionLimits { generation_input_limit_tokens: 100_000, target_active_tokens: 20_000 }).unwrap();
        validate_persisted_checkpoint(&raw, &checkpoint).unwrap();
        let requests = adapter.requests.borrow();
        assert_eq!(requests.len(), 2);
        for request in requests.iter() {
            assert_eq!(request.instructions, plan.instructions);
            assert_eq!(request.tools, plan.tools);
            assert_eq!(request.tool_choice, plan.tool_choice);
            assert_eq!(request.output_token_limit, plan.output_token_limit);
            assert_eq!(&request.ordered_messages()[..3], &raw[..3]);
            let tail = request.input.last().unwrap().content.as_deref().unwrap();
            assert!(!tail.contains("unique older question material"));
            assert!(!tail.contains("unique older answer material"));
            assert!(!tail.contains("current user stays verbatim"));
            assert!(tail.contains("\"ordered_message_index\":1"));
            assert!(tail.contains("\"ordered_message_index\":2"));
        }
        assert!(requests[1].input.last().unwrap().content.as_ref().unwrap()
            .starts_with(requests[0].input.last().unwrap().content.as_ref().unwrap()));
        assert_eq!(plan.ordered_messages(), raw, "binding must not mutate the caller's plan");
    }

    #[test]
    fn compaction_uses_bounded_sources_when_wire_prefix_exceeds_the_window() {
        struct WireCompactor {
            native: crate::NativeAdapter,
            scripted: ScriptedCompactor,
        }
        impl ModelAdapter for WireCompactor {
            fn context_request_projection(&self, request: &AgentRequestPlan) -> Option<serde_json::Value> {
                self.native.context_request_projection(request)
            }
            fn complete(&self, request: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
                self.scripted.complete(request)
            }
            fn complete_structured(&self, request: CompletionRequest) -> Result<serde_json::Value, AdapterError> {
                self.scripted.complete_structured(request)
            }
            fn chat(&self, _: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
                panic!("an oversized wire prefix must not be sent to the model");
            }
        }
        let mut profile = profile();
        profile.context_window_tokens = 20_000;
        profile.output_reserve_tokens = 1_000;
        profile.safety_margin_tokens = 1_000;
        let mut raw = vec![Message::system("base instructions")];
        for _ in 0..8 {
            raw.extend(completed_turn("read source", "observed answer"));
        }
        let mut prepared = prepare_compaction(CompactionPhase::PreTurn, &raw, &raw,
            vec![], vec![], vec![], BTreeMap::new()).unwrap();
        // Dynamic snapshots are in the model projection, not the durable
        // business transcript consumed by the compaction source contract.
        let mut projected = Vec::new();
        for message in &raw {
            projected.push(message.clone());
            if message.role == Role::User { projected.push(Message::system("runtime state ".repeat(350))); }
        }
        let plan = AgentRequestPlan::for_ad_hoc(profile.clone(), &projected, &[]);
        assert!(plan.active_context.fits);
        assert!(prepared.bind_generation_prefix(&plan, &raw));
        let native = crate::NativeAdapter::from_config_with_runtime_profile(crate::ProviderConfig {
            agent_reasoning_effort: Default::default(),
            mode: crate::ProviderMode::Native, api_key: "unused".into(),
            base_url: "http://127.0.0.1:1".into(), model: profile.matched_model.clone(),
        }, Some(profile.clone()));
        native.messages_capabilities.set(Some(crate::deepseek_messages::MessagesCapabilities {
            tool_update: crate::deepseek_messages::ToolUpdateMode::InHistory, system_in_history: true,
        })).unwrap();
        let expected = covering_draft(prepared.request(), "preserved learning request");
        let adapter = WireCompactor { native,
            scripted: ScriptedCompactor::new(vec![serde_json::to_value(&expected).unwrap()]) };
        let checkpoint = compact_with_adapter(&adapter, &profile, &prepared,
            CompactionLimits { generation_input_limit_tokens: 18_000, target_active_tokens: 10_000 }).unwrap();
        validate_persisted_checkpoint(&raw, &checkpoint).unwrap();
        assert_eq!(adapter.scripted.requests.borrow().len(), 1);
    }

    #[test]
    fn compaction_prefix_rejects_tool_generation_and_unmapped_sources() {
        let raw = completed_turn("question", "answer");
        let mut prepared = prepare_compaction(CompactionPhase::PreTurn, &raw, &raw,
            vec![], vec![], vec![], BTreeMap::new()).unwrap();
        let plan = AgentRequestPlan::for_ad_hoc(profile(), &raw, &[]);
        assert!(prepared.bind_generation_prefix(&plan, &raw));
        let adapter = PrefixCompactor { requests: RefCell::new(vec![]),
            outputs: RefCell::new(VecDeque::new()), return_tool: true };
        let error = generate_draft(&adapter, &profile(), &prepared, 100_000).unwrap_err();
        assert!(error.message.contains("returned tool calls"));
        assert_eq!(adapter.requests.borrow().len(), 1);
        let changed = AgentRequestPlan::for_ad_hoc(profile(), &[Message::user("receipt replacement")], &[]);
        assert!(!prepared.bind_generation_prefix(&changed, &raw));
        assert!(prepared.generation_prefix.is_none());
        let fallback = ScriptedCompactor::new(vec![serde_json::to_value(covering_draft(prepared.request(), "state")).unwrap()]);
        generate_draft(&fallback, &profile(), &prepared, 100_000).unwrap();
        assert_eq!(fallback.requests.borrow().len(), 1);
    }

    #[test]
    fn compaction_prefix_repairs_only_when_the_full_request_still_fits() {
        let raw = completed_turn("long source ".repeat(80).as_str(), "answer");
        let mut prepared = prepare_compaction(CompactionPhase::PreTurn, &raw, &raw,
            vec![], vec![], vec![], BTreeMap::new()).unwrap();
        let plan = AgentRequestPlan::for_ad_hoc(profile(), &raw, &[]);
        assert!(prepared.bind_generation_prefix(&plan, &raw));
        let mut invalid = serde_json::to_value(covering_draft(prepared.request(), "state")).unwrap();
        invalid["schema_version"] = serde_json::json!("forbidden");
        let adapter = PrefixCompactor { requests: RefCell::new(vec![]),
            outputs: RefCell::new(vec![invalid].into()), return_tool: false };
        let prefix = prepared.generation_prefix.as_ref().unwrap();
        let instruction = prefix_generation_instruction(COMPACTION_GENERATION_PROMPT, prepared.request(), prefix);
        let initial = prefix_generation_plan(&adapter, prefix, &instruction).active_context.estimated_input_tokens;
        assert!(generate_draft(&adapter, &profile(), &prepared, initial).is_err());
        assert_eq!(adapter.requests.borrow().len(), 1, "a diagnostic must not push repair past the full input limit");
    }

    #[test]
    fn compaction_prefix_uses_observed_cancellable_chat_dispatch() {
        let raw = completed_turn("preserve source", "prior result");
        let mut prepared = prepare_compaction(CompactionPhase::PreTurn, &raw, &raw,
            vec![], vec![], vec![], BTreeMap::new()).unwrap();
        let plan = AgentRequestPlan::for_ad_hoc(profile(), &raw, &[]);
        assert!(prepared.bind_generation_prefix(&plan, &raw));
        let valid = serde_json::to_value(covering_draft(prepared.request(), "continue task")).unwrap();
        let provider = PrefixCompactor { requests: RefCell::new(vec![]),
            outputs: RefCell::new(vec![valid].into()), return_tool: false };
        let cancellation = crate::run_context::CancellationToken::default();
        let cancellable = crate::run_context::CancellableAdapter {
            inner: &provider, cancellation: cancellation.clone(),
        };
        let events = crate::run_events::RunEvents::default();
        let observed = crate::run_events::ObservedAdapter {
            inner: &cancellable, cancellation: cancellation.clone(),
            runtime_profile: profile(), events: events.clone(),
        };
        let _purpose = crate::run_events::purpose(&observed, "compaction");
        compact_with_adapter(&observed, &profile(), &prepared, CompactionLimits::default()).unwrap();
        assert_eq!(provider.requests.borrow().len(), 1);
        let activities = events.activities();
        assert_eq!(activities.len(), 1);
        assert_eq!(activities[0].name, "compaction");
        assert_eq!(activities[0].status, crate::run_events::ActivityStatus::Succeeded);
        cancellation.cancel();
        let error = generate_draft(&observed, &profile(), &prepared, 100_000).unwrap_err();
        assert!(error.message.contains("cancelled"));
        assert_eq!(provider.requests.borrow().len(), 1, "cancellation prevents another provider call");
        assert_eq!(events.activities().len(), 1, "cancelled-before-start calls produce no model activity");
    }

    #[test]
    fn mid_turn_compacts_closed_parallel_steps_and_preserves_user_recent_and_open_pairs() {
        let mut raw = vec![Message::system("base"), Message::user("Current goal <selection>Eq.9</selection>")];
        let mut deterministic = raw.clone();
        for (batch, ids) in [(0, vec!["a", "b"]), (1, vec!["c"]), (2, vec!["d", "e"])] {
            let call = Message { provider_continuation: None, role: Role::Assistant, content: None,
                tool_calls: ids.iter().map(|id| ToolCall { id: (*id).into(), name: "book.text".into(),
                    arguments: serde_json::json!({"lid":format!("1.{batch}")}).to_string() }).collect(), tool_call_id: None };
            raw.push(call.clone()); deterministic.push(call);
            for id in ids.into_iter().filter(|id| *id != "e") {
                let result = Message { provider_continuation: None, role: Role::Tool,
                    content: Some(format!("{id} full source body {}", "evidence ".repeat(50))),
                    tool_calls: vec![], tool_call_id: Some(id.into()) };
                raw.push(result.clone());
                let receipt = HistoricalToolReceipt { version: "historical_tool_receipt.v1".into(),
                    tool: "book.text".into(), locator_args: serde_json::json!({"lid":format!("1.{batch}")}),
                    status: HistoricalToolStatus::Ok, error_code: None, accepted_evidence: vec![],
                    source_refs: vec![], opaque_result_digest: format!("receipt-{id}") };
                deterministic.push(Message { content: Some(serde_json::to_string(&receipt).unwrap()), ..result });
            }
        }
        let prepared = prepare_compaction_with_tail_budget(CompactionPhase::MidTurn, &raw, &deterministic,
            vec![], vec![], vec![PendingEffectRef { effect_id: "pending.write".into(), kind: "write".into() }],
            BTreeMap::new(), 1).unwrap();
        assert_eq!(prepared.source_indices, vec![2, 3, 4]);
        assert_eq!(prepared.full_turn_source_ids.len(), 1, "parallel call and both results stay in one generator chunk");
        assert_eq!(prepared.tool_receipts.len(), 2);
        assert_eq!(prepared.request.raw_retained_item_ids.len(), raw.len() - 4);
        assert!(!prepared.request.eligible_items.iter().any(|item| item.content.contains("Eq.9")));
        let draft = covering_draft(prepared.request(), "Older reads completed; current goal remains in progress.");
        let adapter = ScriptedCompactor::new(vec![serde_json::to_value(draft).unwrap()]);
        let checkpoint = compact_with_adapter(&adapter, &profile(), &prepared, CompactionLimits::default()).unwrap();
        let projected = project_compaction_checkpoint_messages(&raw, &checkpoint, &[], COMPACTION_CONSUMPTION_WRAPPER).unwrap();
        assert_eq!(checkpoint.pending_effects, prepared.pending_effects);
        assert_eq!(&projected[2..], [&raw[1..2], &raw[5..]].concat());
        assert!(prepare_compaction_with_tail_budget(CompactionPhase::MidTurn, &raw, &deterministic,
            vec![], vec![], vec![], BTreeMap::new(), 100_000).is_err(), "a task shorter than the recent budget stays raw");
    }

    fn feedback5_recording(name: &str) -> serde_json::Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/performance/presentation-staged-authoring-ex12-4/mechanism-staged-feedback5")
            .join(name);
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn ex13_compaction_replays_source_failure_with_stable_repair_prefix() {
        let original = feedback5_recording("completion-00-request.json");
        let request: CompactionRequest = serde_json::from_str(original["user"].as_str().unwrap()).unwrap();
        let rejected = feedback5_recording("completion-00-response.json")["response"].clone();
        let repaired = feedback5_recording("completion-01-response.json")["response"].clone();
        let invalid: CompactionDraft = serde_json::from_value(rejected.clone()).unwrap();
        let error = validate_draft(&request, &invalid, &BTreeMap::new()).unwrap_err();
        assert_eq!(error.message, concat!(
            "semantic item item.fact.quote_receipts invented or duplicated a source ID\n",
            "source coverage must contain exactly one record for every eligible source"
        ));
        let profile = crate::ModelRuntimeCatalog::default().resolve("deepseek-flash", ProviderToolProtocol::Native, None);
        let adapter = ScriptedCompactor::new(vec![rejected, repaired.clone()]);
        let result = call_generator(&adapter, &profile, COMPACTION_GENERATION_PROMPT, &request, &BTreeMap::new(), 100_000).unwrap();
        assert_eq!(serde_json::to_value(result).unwrap(), repaired);
        let requests = adapter.requests.borrow();
        assert_eq!(requests.len(), 2);
        if let Ok(directory) = std::env::var("EX13_COMPACTION_RECORDING_DIR") {
            std::fs::create_dir_all(&directory).unwrap();
            for (index, req) in requests.iter().enumerate() {
                let record = serde_json::json!({"system":req.system,"user":req.user,
                    "output_token_limit":req.output_token_limit,"reasoning_effort":req.reasoning_effort,"usage":null});
                std::fs::write(std::path::Path::new(&directory).join(format!("completion-{index:02}-request.json")), serde_json::to_vec_pretty(&record).unwrap()).unwrap();
            }
        }
        assert_eq!(requests[0].system, requests[1].system, "repair must keep the entire system prefix");
        assert!(requests[1].user.starts_with(&requests[0].user));
        let repair_tail: serde_json::Value = serde_json::from_str(&requests[1].user[requests[0].user.len()..]).unwrap();
        assert_eq!(repair_tail["runtime_repair_diagnostic"]["message"], error.message);
        let input: serde_json::Value = serde_json::from_str(&requests[0].user).unwrap();
        for field in ["eligible_items", "required_source_ids", "optional_source_ids", "allowed_evidence_refs", "allowed_supersession_edges", "phase"] {
            assert_eq!(input[field], serde_json::to_value(&request).unwrap()[field], "changed {field}");
        }
        for field in ["schema_version", "prompt_version", "source_history_revision", "raw_retained_item_ids"] {
            assert!(input.get(field).is_none(), "runtime metadata must stay local: {field}");
        }
        assert_eq!(requests[0].output_token_limit, Some(131_072));
        assert_eq!(requests[1].output_token_limit, Some(131_072));
        assert_eq!(requests[1].reasoning_effort.as_deref(), Some("low"));
    }

    #[test]
    fn compaction_repairs_rejected_draft_once_without_relaxing_validation() {
        let raw=completed_turn("Keep the requested page", "The page was delivered");
        let prepared=prepare_compaction(CompactionPhase::PreTurn,&raw,&raw,vec![],vec![],vec![],BTreeMap::new()).unwrap();
        let valid=serde_json::to_value(covering_draft(&prepared.request,"delivered page")).unwrap();
        let mut invalid=valid.clone();invalid["schema_version"]=serde_json::json!("compaction_draft.v1");
        let adapter=ScriptedCompactor::new(vec![invalid.clone(),valid]);
        generate_draft(&adapter,&profile(),&prepared,100_000).unwrap();
        let requests=adapter.requests.borrow();
        assert_eq!(requests.len(),2);
        assert!(requests[1].user.contains("unknown field `schema_version`"));
        assert_eq!(requests[0].system,requests[1].system);
        assert!(requests[1].user.starts_with(&requests[0].user));
        drop(requests);
        let twice=ScriptedCompactor::new(vec![invalid.clone(),invalid.clone()]);
        assert!(generate_draft(&twice,&profile(),&prepared,100_000).is_err());
        assert_eq!(twice.requests.borrow().len(),2);
        let tight=ScriptedCompactor::new(vec![invalid]);
        let tokens=request_input_tokens(&generation_input(&prepared.request).unwrap(),COMPACTION_GENERATION_PROMPT);
        assert!(generate_draft(&tight,&profile(),&prepared,tokens).is_err());
        assert_eq!(tight.requests.borrow().len(),1,"repair cannot exceed the input budget");
    }

    #[test]
    fn ex13_compaction_budgets_the_exact_projected_input_and_repair_tail() {
        let mut raw = completed_turn("保留用户的全部要求", "The page is still incomplete");
        raw.push(Message::user("current user material stays raw"));
        let prepared = prepare_compaction(CompactionPhase::MidTurn, &raw, &raw, vec![], vec![], vec![], BTreeMap::new()).unwrap();
        let valid = serde_json::to_value(covering_draft(&prepared.request, "Continue the incomplete page")).unwrap();
        let mut invalid = valid.clone();
        invalid["schema_version"] = serde_json::json!("forbidden");
        let profile = profile();
        let probe = ScriptedCompactor::new(vec![invalid.clone(), valid.clone()]);
        generate_draft(&probe, &profile, &prepared, 100_000).unwrap();
        let requests = probe.requests.borrow();
        let initial_tokens = request_input_tokens(&requests[0].user, &requests[0].system);
        let repair_tokens = request_input_tokens(&requests[1].user, &requests[1].system);
        assert_eq!(repair_tokens, estimate_text_tokens(&requests[1].system) + estimate_text_tokens(&requests[1].user));
        assert!(repair_tokens > initial_tokens);
        assert!(initial_tokens < request_input_tokens(&serde_json::to_string(&prepared.request).unwrap(), &requests[0].system));
        assert!(!requests[0].user.contains(&prepared.request.raw_retained_item_ids[0]));
        // Exact initial fit must use the projected body, not the larger runtime request.
        let exact_initial = ScriptedCompactor::new(vec![valid.clone()]);
        generate_draft(&exact_initial, &profile, &prepared, initial_tokens).unwrap();
        assert_eq!(exact_initial.requests.borrow().len(), 1);
        for (limit, calls, succeeds) in [(repair_tokens - 1, 1, false), (repair_tokens, 2, true)] {
            let adapter = ScriptedCompactor::new(vec![invalid.clone(), valid.clone()]);
            assert_eq!(generate_draft(&adapter, &profile, &prepared, limit).is_ok(), succeeds);
            assert_eq!(adapter.requests.borrow().len(), calls);
            assert!(adapter.requests.borrow().iter().all(|req| request_input_tokens(&req.user, &req.system) <= limit));
        }
    }

    #[test]
    fn ex13_compaction_repair_cannot_bypass_source_or_schema_validation() {
        let raw = completed_turn("Keep every requested requirement", "Pending work remains");
        let prepared = prepare_compaction(CompactionPhase::PreTurn, &raw, &raw, vec![], vec![], vec![], BTreeMap::new()).unwrap();
        let valid = serde_json::to_value(covering_draft(&prepared.request, "Continue pending work")).unwrap();
        let mut missing = valid.clone();
        missing["source_coverage"].as_array_mut().unwrap().pop();
        let mut duplicate = valid.clone();
        duplicate["source_coverage"].as_array_mut().unwrap().push(valid["source_coverage"][0].clone());
        let mut non_task = valid.clone();
        non_task["source_coverage"][0] = serde_json::json!({"source_item_id":prepared.request.required_source_ids[0], "disposition":"non_task", "target_item_ids":[], "reason":"status_only"});
        let mut evidence = valid.clone();
        evidence["active_goal"][0]["evidence_refs"] = serde_json::json!([{"kind":"source","source_ref_id":"unknown"}]);
        let mut unlinked = valid.clone();
        unlinked["active_goal"][0]["source_item_ids"].as_array_mut().unwrap().pop();
        let mut nine_sections = valid.clone();
        nine_sections.as_object_mut().unwrap().remove("next_steps");
        for invalid in [missing, duplicate, non_task, evidence, unlinked, nine_sections] {
            // A valid third output is deliberately available: it must never be requested.
            let adapter = ScriptedCompactor::new(vec![invalid.clone(), invalid, valid.clone()]);
            assert!(generate_draft(&adapter, &profile(), &prepared, 100_000).is_err());
            assert_eq!(adapter.requests.borrow().len(), 2);
            assert_eq!(adapter.outputs.borrow().len(), 1);
        }
    }

    #[test]
    fn compaction_checkpoint_prompt_and_schema_are_canonical_and_strict() {
        let profile = profile();
        assert_eq!(
            profile.compaction.prompt_asset.text_override.as_deref(),
            Some(COMPACTION_GENERATION_PROMPT)
        );
        assert_eq!(
            profile
                .compaction
                .consumption_wrapper_asset
                .text_override
                .as_deref(),
            Some(COMPACTION_CONSUMPTION_WRAPPER)
        );
        assert_eq!(
            profile.compaction.output_schema_id,
            COMPACTION_DRAFT_SCHEMA_ID
        );
        // A schema name and section list do not tell the generator required item fields.
        let example = COMPACTION_GENERATION_PROMPT.split("```json\n").nth(1)
            .expect("generator must receive the complete output shape").split("\n```").next().unwrap();
        let example = decode_compaction_draft_strict(example).unwrap();
        assert_eq!(example.active_goal[0].item_id, example.source_coverage[0].target_item_ids[0]);
        assert_eq!(example.active_goal[0].source_item_ids[0], example.source_coverage[0].source_item_id);
        // EX11 revisions hit the real validator: free-form NonTask reasons and
        // reasons on Duplicate/Superseded were instructed but are not accepted.
        for rule in [
            "For compacted, duplicate and superseded, omit reason",
            "transport_only", "duplicate_envelope", "status_only",
            "each target item's source_item_ids must include that same source_item_id",
        ] {
            assert!(COMPACTION_GENERATION_PROMPT.contains(rule), "missing coverage rule: {rule}");
        }
        let mut missing_item_id = serde_json::to_value(&example).unwrap();
        missing_item_id["active_goal"][0].as_object_mut().unwrap().remove("item_id");
        assert!(decode_compaction_draft_strict(&missing_item_id.to_string()).is_err());
        let draft = empty_draft();
        let exact = serde_json::to_string(&draft).unwrap();
        assert_eq!(decode_compaction_draft_strict(&exact).unwrap(), draft);
        assert!(decode_compaction_draft_strict(&format!("prose\n{exact}")).is_err());
        let mut unknown = serde_json::to_value(&draft).unwrap();
        unknown["outside_schema"] = serde_json::json!(true);
        assert!(decode_compaction_draft_strict(&unknown.to_string()).is_err());
        let mut missing = serde_json::to_value(&draft).unwrap();
        missing.as_object_mut().unwrap().remove("next_steps");
        assert!(decode_compaction_draft_strict(&missing.to_string()).is_err());
    }

    #[test]
    fn compaction_checkpoint_prepare_keeps_current_text_and_incomplete_pairs_raw() {
        let mut raw = vec![Message::system("base")];
        raw.extend(completed_turn("older question", "older answer"));
        raw.extend(completed_turn("second question", "second answer"));
        raw.push(Message::user(
            "current verbatim selection <selection>Eq. 9</selection>",
        ));
        raw.push(Message {
            provider_continuation: None,
            role: Role::Assistant,
            content: None,
            tool_calls: vec![ToolCall {
                id: "pending-call".into(),
                name: "book.text".into(),
                arguments: r#"{"lid":"1.9"}"#.into(),
            }],
            tool_call_id: None,
        });
        let prepared = prepare_compaction(
            CompactionPhase::MidTurn,
            &raw,
            &raw,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(prepared.full_turn_source_ids.len(), 2);
        assert_eq!(prepared.request.raw_retained_item_ids.len(), 2);
        assert!(prepared
            .request
            .eligible_items
            .iter()
            .all(|item| !item.content.contains("Eq. 9")));
        let raw_ids = raw
            .iter()
            .enumerate()
            .skip(5)
            .map(|(index, message)| source_item_id(index, message).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(prepared.request.raw_retained_item_ids, raw_ids);

        let preturn = prepare_compaction(
            CompactionPhase::PreTurn,
            &raw,
            &raw,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(preturn.request.raw_retained_item_ids, raw_ids);
    }

    #[test]
    fn compaction_pre_turn_covers_closed_user_only_before_mid_turn() {
        let mut raw = vec![Message::system("base")];
        raw.extend(completed_turn("first question", "first answer"));
        raw.push(Message::user("failed question without an answer"));
        let preturn = prepare_compaction(
            CompactionPhase::PreTurn, &raw, &raw, vec![], vec![], vec![], BTreeMap::new(),
        ).unwrap();
        let failed_id = source_item_id(3, &raw[3]).unwrap();
        assert!(preturn.request.required_source_ids.contains(&failed_id));
        assert!(preturn.request.raw_retained_item_ids.is_empty());

        raw.push(Message::user("current question"));
        let midturn = prepare_compaction(
            CompactionPhase::MidTurn, &raw, &raw, vec![], vec![], vec![], BTreeMap::new(),
        ).unwrap();
        assert_eq!(preturn.request.eligible_items, midturn.request.eligible_items);
        assert_eq!(midturn.request.raw_retained_item_ids, vec![source_item_id(4, &raw[4]).unwrap()]);
    }

    #[test]
    fn compaction_prepare_covers_past_user_only_turn_without_consuming_current_turn() {
        let mut raw = vec![Message::system("base")];
        raw.extend(completed_turn("first question", "first answer"));
        raw.push(Message::user("failed question without an answer"));
        raw.extend(completed_turn("retry question", "retry answer"));
        let current_start = raw.len();
        raw.push(Message::user("current question"));
        raw.push(Message {
            provider_continuation: None,
            role: Role::Assistant,
            content: None,
            tool_calls: vec![ToolCall {
                id: "pending-call".into(),
                name: "book.text".into(),
                arguments: r#"{"lid":"1.9"}"#.into(),
            }],
            tool_call_id: None,
        });

        for phase in [CompactionPhase::PreTurn, CompactionPhase::MidTurn] {
            let prepared = prepare_compaction(
                phase,
                &raw,
                &raw,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                BTreeMap::new(),
            )
            .unwrap();
            assert_eq!(prepared.full_turn_source_ids.len(), 3);
            let failed_id = source_item_id(3, &raw[3]).unwrap();
            assert_eq!(prepared.full_turn_source_ids[1], vec![failed_id.clone()]);
            assert!(prepared.request.required_source_ids.contains(&failed_id));
            assert!(prepared.request.eligible_items.iter().any(|item| {
                item.source_item_id == failed_id
                    && item.content == "failed question without an answer"
            }));
            assert_eq!(
                prepared.request.raw_retained_item_ids,
                raw.iter()
                    .enumerate()
                    .skip(current_start)
                    .map(|(index, message)| source_item_id(index, message).unwrap())
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn compaction_checkpoint_validator_rejects_forgery_coverage_and_closed_pending_state() {
        let mut raw = vec![Message::system("base")];
        raw.extend(completed_turn("question", "answer"));
        let mut prepared = prepare_compaction(
            CompactionPhase::PreTurn,
            &raw,
            &raw,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            BTreeMap::new(),
        )
        .unwrap();
        let required = prepared.request.required_source_ids[0].clone();
        prepared.require_open_for_test(&required, RequiredSemanticState::OpenObligation);

        let mut forged = covering_draft(&prepared.request, "claims completion");
        forged.active_goal[0]
            .source_item_ids
            .push("source.forged".into());
        assert!(validate_draft(
            &prepared.request,
            &forged,
            &prepared.required_semantic_states
        )
        .is_err());

        let mut missing_coverage = covering_draft(&prepared.request, "state");
        missing_coverage.source_coverage.pop();
        assert!(validate_draft(
            &prepared.request,
            &missing_coverage,
            &prepared.required_semantic_states
        )
        .is_err());

        let closed = covering_draft(&prepared.request, "done");
        assert!(validate_draft(
            &prepared.request,
            &closed,
            &prepared.required_semantic_states
        )
        .is_err());

        let mut still_open = covering_draft(&prepared.request, "pending");
        still_open.open_obligations = std::mem::take(&mut still_open.active_goal);
        assert!(validate_draft(
            &prepared.request,
            &still_open,
            &prepared.required_semantic_states
        )
        .is_ok());
    }

    fn cc3_consumption_fixture() -> (Vec<Message>, CompactionCheckpoint) {
        let receipt = HistoricalToolReceipt {
            version: "historical_tool_receipt.v1".into(),
            tool: "book.text".into(),
            locator_args: serde_json::json!({"lid":"1.9"}),
            status: HistoricalToolStatus::Ok,
            error_code: None,
            accepted_evidence: vec![EvidenceRange {
                start_lid: "1.9".into(),
                end_lid: "1.10".into(),
                ranges: Vec::new(),
            }],
            source_refs: vec!["source.observed".into()],
            opaque_result_digest: "tool-result-audit-only".into(),
        };
        let raw = vec![
            Message::system("canonical base"),
            Message::user("Explain the observed passage and preserve unfinished work."),
            Message {
                provider_continuation: None,
                role: Role::Assistant,
                content: None,
                tool_calls: vec![ToolCall {
                    id: "read-cc3".into(),
                    name: "book.text".into(),
                    arguments: r#"{"lid":"1.9"}"#.into(),
                }],
                tool_call_id: None,
            },
            Message {
                provider_continuation: None,
                role: Role::Tool,
                content: Some(serde_json::to_string(&receipt).unwrap()),
                tool_calls: Vec::new(),
                tool_call_id: Some("read-cc3".into()),
            },
            assistant("The first part is explained; the comparison remains open."),
            Message::user("Continue with the comparison; preserve this exact text."),
        ];
        let prepared = prepare_compaction(
            CompactionPhase::MidTurn,
            &raw,
            &raw,
            Vec::new(),
            Vec::new(),
            vec![PendingEffectRef {
                effect_id: "effect.pending".into(),
                kind: "reader_layout".into(),
            }],
            BTreeMap::from([("reader.profile_snapshot".into(), "revision-7".into())]),
        )
        .unwrap();
        let mut draft = serde_json::to_value(covering_draft(&prepared.request, "goal")).unwrap();
        let source_item_ids = prepared.request.eligible_items.iter()
            .map(|item| item.source_item_id.clone()).collect::<Vec<_>>();
        for section in [
            "active_goal", "progress", "decisions", "user_constraints", "open_obligations",
            "unresolved_ambiguities", "critical_facts", "critical_examples", "next_steps",
        ] {
            draft[section] = serde_json::json!([{
                "item_id": format!("item.{section}"),
                "text": format!("{section}: preserve exact wording.\nSecond line."),
                "source_item_ids": source_item_ids,
                "evidence_refs": prepared.request.allowed_evidence_refs,
            }]);
        }
        for coverage in draft["source_coverage"].as_array_mut().unwrap() {
            coverage["target_item_ids"] = serde_json::json!(["item.active_goal"]);
        }
        let checkpoint = compact_with_adapter(
            &ScriptedCompactor::new(vec![draft]),
            &profile(),
            &prepared,
            CompactionLimits::default(),
        )
        .unwrap();
        (raw, checkpoint)
    }

    #[test]
    fn cc3_checkpoint_consumption_preserves_semantics_without_audit_payload() {
        let (raw, checkpoint) = cc3_consumption_fixture();
        let raw_before = serde_json::to_vec(&raw).unwrap();
        let stored_before = serde_json::to_vec(&checkpoint).unwrap();
        let context = Message::system("current frozen profile");
        let projected = project_compaction_checkpoint_messages(
            &raw, &checkpoint, &[context.clone()], COMPACTION_CONSUMPTION_WRAPPER,
        )
        .unwrap();
        assert_eq!(projected[0], raw[0]);
        assert_eq!(projected[1], context);
        assert_eq!(projected.last(), raw.last());
        let payload: serde_json::Value = serde_json::from_str(
            projected[2].content.as_deref().unwrap().split_once("compaction_checkpoint:\n").unwrap().1,
        )
        .unwrap();
        let stored = serde_json::to_value(&checkpoint).unwrap();
        let mut expected_semantic = serde_json::Map::new();
        for section in [
            "active_goal", "progress", "decisions", "user_constraints", "open_obligations",
            "unresolved_ambiguities", "critical_facts", "critical_examples", "next_steps",
        ] {
            let items = stored["semantic"][section].as_array().unwrap().iter().map(|item| {
                serde_json::json!({"text": item["text"], "evidence_refs": item["evidence_refs"]})
            }).collect::<Vec<_>>();
            expected_semantic.insert(section.into(), serde_json::Value::Array(items));
        }
        assert_eq!(payload, serde_json::json!({
            "semantic": expected_semantic,
            "pending_effects": checkpoint.pending_effects,
        }));
        assert!(!checkpoint.semantic.source_coverage.is_empty());
        assert!(!checkpoint.tool_receipts.is_empty());
        assert!(!checkpoint.raw_retained_item_ids.is_empty());
        assert!(!checkpoint.context_revisions.is_empty());
        assert_eq!(serde_json::to_vec(&raw).unwrap(), raw_before);
        assert_eq!(serde_json::to_vec(&checkpoint).unwrap(), stored_before);
        let restored: CompactionCheckpoint = serde_json::from_slice(&stored_before).unwrap();
        validate_persisted_checkpoint(&raw, &restored).unwrap();
    }

    #[test]
    fn cc3_checkpoint_consumption_validates_coverage_before_omitting_it() {
        let (raw, mut checkpoint) = cc3_consumption_fixture();
        checkpoint.semantic.source_coverage.pop();
        // A fresh identity cannot turn missing source coverage into a valid handoff.
        checkpoint.window_id = checkpoint_window_id(&checkpoint).unwrap();
        let error = project_compaction_checkpoint_messages(
            &raw, &checkpoint, &[], COMPACTION_CONSUMPTION_WRAPPER,
        )
        .unwrap_err();
        assert_eq!(error.error_code, "COMPACTION_VALIDATION_FAILED");
        assert!(error.message.contains("source outside coverage"));
        assert!(validate_persisted_checkpoint(&raw, &checkpoint).is_err());
    }

    #[test]
    fn cc3_can_reduce_ignores_audit_overhead_for_retained_current_turn() {
        let mut raw = vec![Message::system("base")];
        raw.extend(completed_turn(&"older question ".repeat(40), &"older answer ".repeat(40)));
        raw.push(Message::user("Current task is still in progress."));
        for index in 0..12 {
            let id = format!("current-read-{index}");
            raw.push(Message {
                provider_continuation: None,
                role: Role::Assistant,
                content: None,
                tool_calls: vec![ToolCall {
                    id: id.clone(),
                    name: "book.text".into(),
                    arguments: format!(r#"{{"lid":"1.{index}"}}"#),
                }],
                tool_call_id: None,
            });
            raw.push(Message {
                provider_continuation: None,
                role: Role::Tool,
                content: Some("Current source remains outside compaction.".into()),
                tool_calls: Vec::new(),
                tool_call_id: Some(id),
            });
        }
        let prepared = prepare_compaction(
            CompactionPhase::MidTurn, &raw, &raw, Vec::new(), Vec::new(), Vec::new(), BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(prepared.request.raw_retained_item_ids.len(), 25);
        let draft = covering_draft(&prepared.request, "Continue from the earlier answer.");
        let checkpoint = compact_with_adapter(
            &ScriptedCompactor::new(vec![serde_json::to_value(draft).unwrap()]),
            &profile(), &prepared, CompactionLimits::default(),
        )
        .unwrap();
        let projected = project_compaction_checkpoint_messages(
            &raw, &checkpoint, &[], COMPACTION_CONSUMPTION_WRAPPER,
        )
        .unwrap();
        assert!(message_tokens(&projected).unwrap() < message_tokens(&raw).unwrap(),
            "the lean checkpoint must make this supported mid-turn history smaller");
        assert!(prepared.can_reduce(&profile(), COMPACTION_CONSUMPTION_WRAPPER).unwrap(),
            "raw-retained source IDs are validation data, not unavoidable model input");
    }

    #[test]
    #[ignore = "requires CC3_CHECKPOINT_INPUT and CC3_CHECKPOINT_OUTPUT for recorded projection measurement"]
    fn cc3_record_consumption_projection() {
        let input = std::env::var_os("CC3_CHECKPOINT_INPUT").expect("CC3_CHECKPOINT_INPUT path");
        let output = std::env::var_os("CC3_CHECKPOINT_OUTPUT").expect("CC3_CHECKPOINT_OUTPUT path");
        let stored = std::fs::read(input).unwrap();
        let checkpoint: CompactionCheckpoint = serde_json::from_slice(&stored).unwrap();
        // This records the production serializer against a captured checkpoint.
        // Full source validation requires its original raw history, exercised by
        // the fixture tests above rather than inferred from this capture.
        let consumed = serde_json::to_vec(&checkpoint_consumption_projection(
            &checkpoint.semantic, &checkpoint.pending_effects,
        )).unwrap();
        std::fs::write(output, &consumed).unwrap();
        println!("stored_bytes={} consumption_bytes={}", stored.len(), consumed.len());
    }

    #[test]
    fn compaction_checkpoint_install_projects_fixed_order_without_mutating_history() {
        let long = "historical detail ".repeat(1_200);
        let mut raw = vec![Message::system("canonical base")];
        raw.extend(completed_turn(&long, &long));
        raw.last_mut().unwrap().provider_continuation = Some(crate::ProviderContinuation { model: "deepseek-flash".into(), reasoning_content: "private-compaction-probe".into(), messages_content: Vec::new() });
        raw.push(Message::user("current user text must remain byte exact"));
        let before = serde_json::to_vec(&raw).unwrap();
        let prepared = prepare_compaction(
            CompactionPhase::MidTurn,
            &raw,
            &raw,
            Vec::new(),
            Vec::new(),
            vec![PendingEffectRef {
                effect_id: "effect.pending".into(),
                kind: "reader_layout".into(),
            }],
            BTreeMap::from([("reader.profile_snapshot".into(), "revision-7".into())]),
        )
        .unwrap();
        let draft = covering_draft(&prepared.request, "Continue the current explanation.");
        let adapter = ScriptedCompactor::new(vec![serde_json::to_value(draft).unwrap()]);
        assert!(!serde_json::to_string(prepared.request()).unwrap().contains("private-compaction-probe"));
        let profile = crate::ModelRuntimeCatalog::default().resolve("deepseek-flash", ProviderToolProtocol::Native, None);
        let checkpoint = compact_with_adapter(
            &adapter,
            &profile,
            &prepared,
            CompactionLimits {
                generation_input_limit_tokens: 100_000,
                target_active_tokens: 20_000,
            },
        )
        .unwrap();
        assert_eq!(serde_json::to_vec(&raw).unwrap(), before);
        assert_eq!(checkpoint.pending_effects.len(), 1);
        assert_eq!(
            checkpoint.context_revisions["reader.profile_snapshot"],
            "revision-7"
        );
        let context = Message::system("context_fragment.v1\nkey=reader.profile_snapshot");
        let projected = project_compaction_checkpoint_messages(
            &raw,
            &checkpoint,
            &[context.clone()],
            COMPACTION_CONSUMPTION_WRAPPER,
        )
        .unwrap();
        assert_eq!(projected[0].content.as_deref(), Some("canonical base"));
        assert_eq!(projected[1].content, context.content);
        assert!(projected[2]
            .content
            .as_deref()
            .unwrap()
            .starts_with(CONTEXT_COMPACTION_ITEM_VERSION));
        assert_eq!(
            projected[3].content.as_deref(),
            Some("current user text must remain byte exact")
        );
        assert_eq!(adapter.requests.borrow().len(), 1);
        assert_eq!(adapter.requests.borrow()[0].output_token_limit, Some(131_072));
        assert_eq!(adapter.requests.borrow()[0].reasoning_effort.as_deref(), Some("low"));
        assert_eq!(
            adapter.requests.borrow()[0].system,
            COMPACTION_GENERATION_PROMPT
        );
    }

    #[test]
    fn compaction_checkpoint_failure_is_byte_equivalent_and_stale_history_fails_closed() {
        let long = "old state ".repeat(1_000);
        let mut raw = vec![Message::system("base")];
        raw.extend(completed_turn(&long, &long));
        raw.push(Message::user("current"));
        let before = serde_json::to_vec(&raw).unwrap();
        let prepared = prepare_compaction(
            CompactionPhase::MidTurn,
            &raw,
            &raw,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            BTreeMap::new(),
        )
        .unwrap();
        let mut invalid = covering_draft(&prepared.request, "state");
        invalid.source_coverage.pop();
        let failed = ScriptedCompactor::new(vec![serde_json::to_value(invalid).unwrap()]);
        assert!(compact_with_adapter(
            &failed,
            &profile(),
            &prepared,
            CompactionLimits {
                generation_input_limit_tokens: 100_000,
                target_active_tokens: 20_000,
            }
        )
        .is_err());
        assert_eq!(serde_json::to_vec(&raw).unwrap(), before);

        let valid = covering_draft(&prepared.request, "state");
        let valid_adapter = ScriptedCompactor::new(vec![serde_json::to_value(valid).unwrap()]);
        let checkpoint = compact_with_adapter(
            &valid_adapter,
            &profile(),
            &prepared,
            CompactionLimits {
                generation_input_limit_tokens: 100_000,
                target_active_tokens: 20_000,
            },
        )
        .unwrap();
        let mut appended = raw.clone();
        appended.push(Message::user("a later continuation"));
        let projected = project_compaction_checkpoint_messages(
            &appended,
            &checkpoint,
            &[],
            COMPACTION_CONSUMPTION_WRAPPER,
        )
        .unwrap();
        assert!(projected
            .iter()
            .any(|message| message.content.as_deref() == Some("a later continuation")));
        let mut tampered = raw;
        tampered[1].content = Some("changed old source".into());
        assert!(validate_persisted_checkpoint(&tampered, &checkpoint).is_err());
    }

    #[test]
    fn compaction_preserves_reused_call_ids_across_turns_and_rejects_duplicates_within_one() {
        let mut raw=vec![Message::system("base")];
        let mut receipts=Vec::new();
        for lid in ["1.1","1.2"] {
            let receipt=HistoricalToolReceipt {version:"historical_tool_receipt.v1".into(),tool:"book.text".into(),
                locator_args:serde_json::json!({"lid":lid}),status:HistoricalToolStatus::Ok,error_code:None,
                accepted_evidence:vec![],source_refs:vec![],opaque_result_digest:format!("receipt-{lid}")};
            raw.push(Message::user(format!("Explain {lid}: {}", "question context ".repeat(100))));
            raw.push(Message {provider_continuation:None,role:Role::Assistant,content:None,
                tool_calls:vec![ToolCall {id:"read".into(),name:"book.text".into(),arguments:serde_json::json!({"lid":lid}).to_string()}],tool_call_id:None});
            raw.push(Message {provider_continuation:None,role:Role::Tool,content:Some(format!("source body {lid}")),
                tool_calls:vec![],tool_call_id:Some("read".into())});
            raw.push(assistant(&format!("Answer {lid}: {}", "answer context ".repeat(100))));
            receipts.push(receipt);
        }
        raw.push(Message::user("Continue the current question verbatim."));
        let before=serde_json::to_vec(&raw).unwrap();
        let mut deterministic=raw.clone();
        for (index,receipt) in [3,7].into_iter().zip(&receipts) { deterministic[index].content=Some(serde_json::to_string(receipt).unwrap()); }
        let prepared=prepare_compaction(CompactionPhase::MidTurn,&raw,&deterministic,vec![],vec![],vec![],BTreeMap::new()).unwrap();
        let tools=prepared.request.eligible_items.iter().filter(|item|item.role==Role::Tool).collect::<Vec<_>>();
        assert_eq!(tools.len(),2);
        assert_ne!(tools[0].source_item_id,tools[1].source_item_id);
        assert!(tools[0].content.contains("1.1") && tools[1].content.contains("1.2"));
        let adapter=ScriptedCompactor::new(vec![serde_json::to_value(covering_draft(&prepared.request,"Both prior passages were explained; continue the current question.")).unwrap()]);
        let checkpoint=compact_with_adapter(&adapter,&profile(),&prepared,CompactionLimits {generation_input_limit_tokens:100_000,target_active_tokens:20_000}).unwrap();
        assert_eq!(checkpoint.tool_receipts,receipts);
        validate_persisted_checkpoint(&raw,&checkpoint).unwrap();
        let projected=project_compaction_checkpoint_messages(&raw,&checkpoint,&[],COMPACTION_CONSUMPTION_WRAPPER).unwrap();
        assert_eq!(projected.last(),raw.last());
        assert_eq!(serde_json::to_vec(&raw).unwrap(),before,"compaction must not rename or mutate historical call IDs");
        for duplicate_call in [true,false] {
            let mut duplicate=raw[..5].to_vec();
            let mut repeated=vec![];
            if duplicate_call { repeated.push(raw[2].clone()); }
            repeated.push(raw[3].clone());
            duplicate.splice(4..4,repeated);
            let error=prepare_compaction(CompactionPhase::PreTurn,&duplicate,&duplicate,vec![],vec![],vec![],BTreeMap::new()).unwrap_err();
            assert!(error.message.contains(if duplicate_call {"unique within a turn"} else {"orphaned or duplicated"}));
        }
    }

    #[test]
    fn compaction_checkpoint_receipts_are_runtime_owned_and_evidence_is_whitelisted() {
        let range = EvidenceRange {
            start_lid: "1.9".into(),
            end_lid: "1.10".into(),
            ranges: Vec::new(),
        };
        let receipt = HistoricalToolReceipt {
            version: "historical_tool_receipt.v1".into(),
            tool: "book.text".into(),
            locator_args: serde_json::json!({"lid":"1.9"}),
            status: HistoricalToolStatus::Ok,
            error_code: None,
            accepted_evidence: vec![range.clone()],
            source_refs: Vec::new(),
            opaque_result_digest: "tool-result-digest".into(),
        };
        let mut raw = vec![
            Message::system("base"),
            Message::user(format!("explain {}", "question context ".repeat(700))),
        ];
        raw.push(Message {
            provider_continuation: None,
            role: Role::Assistant,
            content: None,
            tool_calls: vec![ToolCall {
                id: "read-1".into(),
                name: "book.text".into(),
                arguments: r#"{"lid":"1.9"}"#.into(),
            }],
            tool_call_id: None,
        });
        raw.push(Message {
            provider_continuation: None,
            role: Role::Tool,
            content: Some("raw body that the model must not carry as evidence".repeat(500)),
            tool_calls: Vec::new(),
            tool_call_id: Some("read-1".into()),
        });
        raw.push(assistant(&format!(
            "the answer {}",
            "answer context ".repeat(700)
        )));
        let mut deterministic = raw.clone();
        deterministic[3].content = Some(serde_json::to_string(&receipt).unwrap());
        let prepared = prepare_compaction(
            CompactionPhase::PreTurn,
            &raw,
            &deterministic,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            BTreeMap::new(),
        )
        .unwrap();
        let tool_source = prepared
            .request
            .eligible_items
            .iter()
            .find(|item| item.role == Role::Tool)
            .unwrap();
        let evidence_ref = EvidenceRef::LidRange {
            start_lid: "1.9".into(),
            end_lid: "1.10".into(),
        };
        assert_eq!(tool_source.evidence_refs, vec![evidence_ref.clone()]);
        let mut draft = covering_draft(&prepared.request, "supported state");
        draft.active_goal[0].evidence_refs = vec![evidence_ref];
        let adapter = ScriptedCompactor::new(vec![serde_json::to_value(draft).unwrap()]);
        let checkpoint = compact_with_adapter(
            &adapter,
            &profile(),
            &prepared,
            CompactionLimits {
                generation_input_limit_tokens: 100_000,
                target_active_tokens: 20_000,
            },
        )
        .unwrap();
        assert_eq!(checkpoint.tool_receipts, vec![receipt]);
        assert!(!serde_json::to_string(&checkpoint)
            .unwrap()
            .contains("raw body that the model"));
    }

    #[test]
    fn compaction_checkpoint_hierarchical_chunks_preserve_every_complete_turn() {
        let detail = "turn detail ".repeat(350);
        let mut raw = vec![Message::system("base")];
        for ordinal in 0..4 {
            raw.extend(completed_turn(
                &format!("question {ordinal} {detail}"),
                &format!("answer {ordinal} {detail}"),
            ));
        }
        let prepared = prepare_compaction(
            CompactionPhase::PreTurn,
            &raw,
            &raw,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            BTreeMap::new(),
        )
        .unwrap();
        let system = COMPACTION_GENERATION_PROMPT;
        let full_tokens = request_input_tokens(&generation_input(&prepared.request).unwrap(), system);
        let largest_child = prepared
            .full_turn_source_ids
            .iter()
            .map(|turn| {
                request_input_tokens(&generation_input(&child_request(&prepared.request, turn).unwrap()).unwrap(), system)
            })
            .max()
            .unwrap();
        assert!(largest_child < full_tokens);
        let adapter = RequestDrivenCompactor {
            calls: Cell::new(0),
        };
        let checkpoint = compact_with_adapter(
            &adapter,
            &profile(),
            &prepared,
            CompactionLimits {
                generation_input_limit_tokens: largest_child + 20,
                target_active_tokens: full_tokens,
            },
        )
        .unwrap();
        assert!(adapter.calls.get() >= 3);
        assert_eq!(
            checkpoint.semantic.source_coverage.len(),
            prepared.request.eligible_items.len()
        );
        let covered = checkpoint
            .semantic
            .source_coverage
            .iter()
            .map(|coverage| coverage.source_item_id.as_str())
            .collect::<BTreeSet<_>>();
        for turn in &prepared.full_turn_source_ids {
            assert!(turn.iter().all(|source| covered.contains(source.as_str())));
        }
    }
}
