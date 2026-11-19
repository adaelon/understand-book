use crate::{InstructionModule, Message, Role};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const CONTEXT_FRAGMENT_VERSION: &str = "context_fragment.v1";
pub const READER_PROFILE_FRAGMENT_KEY: &str = "reader.profile_snapshot";
pub const MEMORY_OPERATION_FRAGMENT_KEY: &str = "memory.operation_result";
pub const PAPER_MINIMAP_FRAGMENT_KEY: &str = "reader.paper_minimap_agent_context";
pub const ARTIFACT_ROUTING_FRAGMENT_KEY: &str = "reader.artifact_routing_cards";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FragmentScope {
    SessionStable,
    TurnFrozen,
    Dynamic,
}

impl FragmentScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::SessionStable => "session_stable",
            Self::TurnFrozen => "turn_frozen",
            Self::Dynamic => "dynamic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FragmentSensitivity {
    Public,
    Private,
    Sensitive,
}

impl FragmentSensitivity {
    fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::Sensitive => "sensitive",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextFragment {
    pub key: String,
    pub revision: String,
    pub scope: FragmentScope,
    pub role: Role,
    pub content: String,
    pub sensitivity: FragmentSensitivity,
}

impl ContextFragment {
    pub fn new(
        key: impl Into<String>,
        scope: FragmentScope,
        role: Role,
        content: impl Into<String>,
        sensitivity: FragmentSensitivity,
    ) -> Self {
        let key = key.into();
        let content = content.into();
        let revision = fragment_revision(&key, &content);
        Self {
            key,
            revision,
            scope,
            role,
            content,
            sensitivity,
        }
    }

    pub fn projected_message(&self) -> Message {
        Message {
            provider_continuation: None,
            role: self.role,
            content: Some(format!(
                "{CONTEXT_FRAGMENT_VERSION}\nkey={}\nrevision={}\nscope={}\nsensitivity={}\ncontent:\n{}",
                self.key,
                self.revision,
                self.scope.as_str(),
                self.sensitivity.as_str(),
                self.content
            )),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentUpsertOutcome {
    Inserted,
    Replaced,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextFragmentError {
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct ContextFragmentLedger {
    active: BTreeMap<String, ContextFragment>,
    projection_order: Vec<String>,
    // Run-local snapshots anchored after a complete raw message/tool-result group.
    sampling_snapshots: Vec<(usize, Vec<Message>)>,
    sampling_instructions: Vec<InstructionModule>,
    sampling_fragments: Vec<ContextFragment>,
    sampling_messages: Vec<Message>,
    recorded_sampling_anchor: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextFragmentSnapshot {
    pub revision: String,
    pub fragments: Vec<ContextFragment>,
}

impl ContextFragmentLedger {
    /// Expire only records owned by this key namespace. The next sampling's
    /// active_keys manifest makes removal visible without rewriting old bodies.
    pub fn retain_keys_with_prefix(&mut self, prefix: &str, retained: &[String]) {
        self.active.retain(|key, _| !key.starts_with(prefix) || retained.contains(key));
        self.projection_order.retain(|key| self.active.contains_key(key));
    }

    pub fn upsert(
        &mut self,
        fragment: ContextFragment,
    ) -> Result<FragmentUpsertOutcome, ContextFragmentError> {
        if fragment.key.trim().is_empty() || fragment.revision.trim().is_empty() {
            return Err(ContextFragmentError {
                message: "context fragment key and revision must be nonempty".into(),
            });
        }
        if let Some(current) = self.active.get(&fragment.key) {
            if current.revision == fragment.revision {
                if current == &fragment {
                    return Ok(FragmentUpsertOutcome::Unchanged);
                }
                return Err(ContextFragmentError {
                    message: format!(
                        "context fragment revision collision for key {} revision {}",
                        fragment.key, fragment.revision
                    ),
                });
            }
            self.active.insert(fragment.key.clone(), fragment);
            return Ok(FragmentUpsertOutcome::Replaced);
        }

        self.projection_order.push(fragment.key.clone());
        self.active.insert(fragment.key.clone(), fragment);
        Ok(FragmentUpsertOutcome::Inserted)
    }

    pub fn snapshot(&self) -> ContextFragmentSnapshot {
        let fragments = self
            .projection_order
            .iter()
            .filter_map(|key| self.active.get(key).cloned())
            .collect::<Vec<_>>();
        let revision_input = fragments
            .iter()
            .map(|fragment| format!("{}={}", fragment.key, fragment.revision))
            .collect::<Vec<_>>()
            .join("\n");
        ContextFragmentSnapshot {
            revision: digest(&revision_input),
            fragments,
        }
    }

    pub fn project_session_stable_messages(&self, messages: &[Message]) -> Vec<Message> {
        let insert_at = messages
            .iter()
            .position(|message| message.role != Role::System)
            .unwrap_or(messages.len());
        let fragments = self.projected_messages(FragmentScope::SessionStable);
        let mut projected = Vec::with_capacity(messages.len() + fragments.len());
        projected.extend_from_slice(&messages[..insert_at]);
        projected.extend(fragments);
        projected.extend_from_slice(&messages[insert_at..]);
        projected
    }

    /// Keep the reusable history before this turn's frozen state. Apply this
    /// before sampling snapshots or provider adaptation can add synthetic users.
    pub fn project_turn_frozen_messages(&self, messages: &mut Vec<Message>) {
        let insert_at = messages
            .iter()
            .rposition(|message| message.role == Role::User)
            .unwrap_or(messages.len());
        messages.splice(
            insert_at..insert_at,
            self.projected_messages(FragmentScope::TurnFrozen),
        );
    }

    pub fn projected_messages(&self, scope: FragmentScope) -> Vec<Message> {
        self.snapshot()
            .fragments
            .iter()
            .filter(|fragment| fragment.scope == scope)
            .map(ContextFragment::projected_message)
            .collect()
    }

    /// Messages contributed by the current turn, excluding the session prefix.
    pub fn turn_projection_message_count(&self) -> usize {
        self.active.values().filter(|fragment| fragment.scope == FragmentScope::TurnFrozen).count()
            + self.sampling_snapshots.iter().map(|(_, messages)| messages.len()).sum::<usize>()
    }

    /// Append only changed dynamic bodies while retaining an authoritative key
    /// manifest. A rebuild after compaction reuses this decision's original delta.
    pub fn record_sampling(
        &mut self,
        raw_message_count: usize,
        instructions: &[InstructionModule],
        current_messages: Vec<Message>,
    ) {
        if self.recorded_sampling_anchor == Some(raw_message_count) {
            return;
        }
        self.recorded_sampling_anchor = Some(raw_message_count);
        let fragments = self.projection_order.iter()
            .filter_map(|key| self.active.get(key))
            .filter(|fragment| fragment.scope == FragmentScope::Dynamic)
            .cloned().collect::<Vec<_>>();
        // A restored model prefix can contain the previous turn's manifests.
        // Publish all three replacement boundaries once, even when now empty.
        let first_sampling = self.sampling_snapshots.is_empty();
        if !first_sampling && self.sampling_fragments == fragments
            && self.sampling_instructions == instructions
            && self.sampling_messages == current_messages
        {
            return;
        }
        let mut snapshot = sampling_state(
            self.sampling_snapshots.len() + 1, &fragments, &self.sampling_fragments,
        );
        if first_sampling || self.sampling_instructions != instructions {
            snapshot.insert(1, sampling_guidance(instructions, &self.sampling_instructions));
        }
        if first_sampling || self.sampling_messages != current_messages {
            snapshot.extend(sampling_messages(&current_messages));
        }
        self.sampling_snapshots.push((raw_message_count, snapshot));
        self.sampling_fragments = fragments;
        self.sampling_instructions = instructions.to_vec();
        self.sampling_messages = current_messages;
    }

    /// Retain only sampling history; transient dynamic rules stay on their decision.
    pub fn retain_sampling_history(&mut self, sampled: &Self) {
        self.sampling_snapshots = sampled.sampling_snapshots.clone();
        self.sampling_instructions = sampled.sampling_instructions.clone();
        self.sampling_fragments = sampled.sampling_fragments.clone();
        self.sampling_messages = sampled.sampling_messages.clone();
        // A rejected premature final answer can start another sampling without
        // adding raw messages. It still needs fresh budget/Goal state.
        self.recorded_sampling_anchor = None;
    }

    /// At request pressure, obsolete runtime decisions can leave the model
    /// context. Republish the complete current state and guidance because the
    /// earlier bodies referenced by later deltas are being removed too.
    pub fn retire_sampling_history(&mut self) -> bool {
        if self.sampling_snapshots.len() <= 1 { return false; }
        let anchor=self.sampling_snapshots.last().unwrap().0;
        let mut snapshot=sampling_state(1, &self.sampling_fragments, &[]);
        snapshot.insert(1,sampling_guidance(&self.sampling_instructions, &[]));
        snapshot.extend(sampling_messages(&self.sampling_messages));
        self.sampling_snapshots.clear();
        self.sampling_snapshots.push((anchor,snapshot));
        true
    }

    pub fn project_sampling_snapshots(
        &self,
        messages: &mut Vec<Message>,
        raw_message_count: usize,
    ) {
        // Mid-turn compaction preserves the complete current turn. Anchors are in
        // that suffix, so counting back from its end also works after compaction.
        let projected_count = messages.len();
        for (anchor, snapshot) in self.sampling_snapshots.iter().rev() {
            let insert_at = projected_count - (raw_message_count - anchor);
            messages.splice(insert_at..insert_at, snapshot.clone());
        }
    }
}

fn sampling_state(
    sampling: usize,
    fragments: &[ContextFragment],
    previous: &[ContextFragment],
) -> Vec<Message> {
    let active_keys = fragments.iter().map(|fragment| &fragment.key).collect::<Vec<_>>();
    let mut content = format!(
        "runtime_state_snapshot.v1\nsampling={sampling}\nThe latest active_keys manifest replaces earlier selections of dynamic context. Apply each listed key's latest published body below or in earlier runtime snapshots. Unchanged bodies remain active while listed; unlisted keys have expired. Superseded bodies describe earlier decisions. Sampling guidance and decision messages have their own replacement rules.\nactive_keys={}\n",
        serde_json::json!(active_keys),
    );
    let mut task_data = Vec::new();
    for fragment in fragments.iter().filter(|fragment| !previous.contains(fragment)) {
        if fragment.role == Role::System {
            content.push_str(&format!("\nkey={}\n{}\n", fragment.key, fragment.content));
        } else {
            task_data.push(fragment.projected_message());
        }
    }
    let mut messages = vec![Message::system(content)];
    messages.extend(task_data);
    messages
}

fn sampling_messages(messages: &[Message]) -> Vec<Message> {
    let mut projected = vec![Message::system(format!(
        "runtime_decision_messages.v1\nmessage_count={}\nThe next message_count messages replace all earlier decision messages. They remain active until the next runtime_decision_messages replacement; zero expires all previous decision messages. Dynamic context and sampling guidance keep their own manifests.\n",
        messages.len(),
    ))];
    projected.extend_from_slice(messages);
    projected
}

fn sampling_guidance(instructions: &[InstructionModule], previous: &[InstructionModule]) -> Message {
    let mut content = String::from(
        "sampling_guidance.v1\nThe latest active_assets manifest replaces all earlier selections. Apply only its listed workflows, phase and references, using each asset's latest published definition below or in earlier sampling guidance. Unchanged definitions remain valid while listed; unlisted assets are inactive. This selection remains active until the next sampling_guidance manifest; runtime state snapshots do not expire it. Guidance does not grant tools or permissions.\n",
    );
    let active_assets = instructions.iter().map(|module| serde_json::json!({
        "asset_id": module.asset_id,
        "revision": module.revision,
    })).collect::<Vec<_>>();
    content.push_str(&format!("active_assets={}\n", serde_json::json!(active_assets)));
    if instructions.is_empty() {
        content.push_str("No task workflows, presentation phase or technical references are active. Earlier sampling guidance and design data have expired.\n");
    }
    content.push_str("definitions:\n");
    for module in instructions.iter().filter(|module| !previous.contains(module)) {
        content.push_str(&format!("\nasset={}\nrevision={}\n{}\n", module.asset_id, module.revision, module.text));
    }
    Message::system(content)
}

fn fragment_revision(key: &str, content: &str) -> String {
    digest(&format!("{key}\n{content}"))
}

fn digest(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fragment(key: &str, content: &str) -> ContextFragment {
        ContextFragment::new(
            key,
            FragmentScope::TurnFrozen,
            Role::System,
            content,
            FragmentSensitivity::Private,
        )
    }

    #[test]
    fn first_empty_sampling_expires_restored_turn_state_guidance_and_decision_messages() {
        let mut ledger = ContextFragmentLedger::default();
        let retained = vec![Message::user("Previous turn's retained runtime messages")];
        ledger.record_sampling(1, &[], Vec::new());
        let mut projected = retained.clone();
        ledger.project_sampling_snapshots(&mut projected, 1);
        assert_eq!(projected[0], retained[0]);
        let text = projected.iter().skip(1).filter_map(|message| message.content.as_deref()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("active_keys=[]") && text.contains("active_assets=[]") && text.contains("message_count=0"));
        assert_eq!(ledger.turn_projection_message_count(), 3);
        ledger.record_sampling(2, &[], Vec::new());
        assert_eq!(ledger.turn_projection_message_count(), 3, "unchanged empty state must not repeat clears at every step");
    }

    #[test]
    fn context_fragment_ledger_keeps_one_latest_revision_per_key_in_stable_order() {
        let mut ledger = ContextFragmentLedger::default();
        let profile_v1 = fragment("reader.profile_snapshot", "profile v1");
        let profile_v1_revision = profile_v1.revision.clone();
        assert_eq!(
            ledger.upsert(profile_v1.clone()).unwrap(),
            FragmentUpsertOutcome::Inserted
        );
        assert_eq!(
            ledger.upsert(profile_v1).unwrap(),
            FragmentUpsertOutcome::Unchanged
        );
        assert_eq!(
            ledger
                .upsert(fragment("memory.operation_result", "memory result"))
                .unwrap(),
            FragmentUpsertOutcome::Inserted
        );
        assert_eq!(
            ledger
                .upsert(fragment("reader.profile_snapshot", "profile v2"))
                .unwrap(),
            FragmentUpsertOutcome::Replaced
        );

        let snapshot = ledger.snapshot();
        assert_eq!(snapshot.fragments.len(), 2);
        assert_eq!(snapshot.fragments[0].key, "reader.profile_snapshot");
        assert_eq!(snapshot.fragments[0].content, "profile v2");
        assert_ne!(snapshot.fragments[0].revision, profile_v1_revision);
        assert_eq!(snapshot.fragments[1].key, "memory.operation_result");
    }

    #[test]
    fn sampling_snapshots_survive_compacted_prefix_and_do_not_enter_frozen_context() {
        let mut history = ContextFragmentLedger::default();
        history.upsert(fragment("scene", "frozen current scene")).unwrap();
        let mut first = history.clone();
        first
            .upsert(ContextFragment::new(
                "budget",
                FragmentScope::Dynamic,
                Role::System,
                "remaining=2",
                FragmentSensitivity::Private,
            ))
            .unwrap();
        first.record_sampling(10, &[], Vec::new());
        history.retain_sampling_history(&first);
        let mut second = history.clone();
        second
            .upsert(ContextFragment::new(
                "budget",
                FragmentScope::Dynamic,
                Role::System,
                "remaining=1",
                FragmentSensitivity::Private,
            ))
            .unwrap();
        second.record_sampling(12, &[], Vec::new());
        let mut compacted = vec![
            Message::system("compacted older history"),
            Message::user("current task"),
            Message {
                role: Role::Assistant, content: None, provider_continuation: None,
                tool_calls: vec![crate::ToolCall {
                    id: "read".into(), name: "book.text".into(), arguments: "{}".into(),
                }],
                tool_call_id: None,
            },
            Message {
                role: Role::Tool, content: Some("complete tool group".into()),
                provider_continuation: None, tool_calls: vec![], tool_call_id: Some("read".into()),
            },
        ];
        second.project_turn_frozen_messages(&mut compacted);
        second.project_sampling_snapshots(&mut compacted, 12);
        assert!(compacted[1].content.as_deref().unwrap().contains("frozen current scene"));
        assert_eq!(compacted[2].content.as_deref(), Some("current task"));
        assert!(compacted[3]
            .content
            .as_deref()
            .unwrap()
            .contains("remaining=2"));
        assert!(compacted[4].content.as_deref().unwrap().contains("active_assets=[]"));
        assert!(compacted[5].content.as_deref().unwrap().contains("message_count=0"));
        assert_eq!(compacted[6].tool_calls[0].id, "read");
        assert_eq!(compacted[7].tool_call_id.as_deref(), Some("read"));
        assert!(compacted[8]
            .content
            .as_deref()
            .unwrap()
            .contains("remaining=1"));
        assert!(second.projected_messages(FragmentScope::SessionStable).is_empty());
        assert_eq!(second.projected_messages(FragmentScope::TurnFrozen).len(), 1);
    }

    #[test]
    fn sampling_rebuild_keeps_guidance_once_and_task_data_at_its_role() {
        let mut ledger = ContextFragmentLedger::default();
        ledger.upsert(ContextFragment::new(
            "design", FragmentScope::Dynamic, Role::User, "Agent design prose", FragmentSensitivity::Private,
        )).unwrap();
        let modules = [InstructionModule::new("fixture.phase", "v1", "Compiled host guidance")];
        ledger.record_sampling(10, &modules, vec![Message::user("current design")]);
        // The same raw anchor is rebuilt after compaction before model sampling.
        ledger.record_sampling(10, &modules, vec![Message::user("current design")]);
        let mut projected = vec![Message::system("checkpoint"), Message::user("current question")];
        ledger.project_sampling_snapshots(&mut projected, 10);
        assert_eq!(projected.iter().filter(|m| m.content.as_deref().is_some_and(|s| s.contains("Compiled host guidance"))).count(), 1);
        assert!(projected.iter().filter(|m| m.role == Role::System).all(|m| !m.content.as_deref().unwrap().contains("Agent design prose")));
        assert!(projected.iter().any(|m| m.role == Role::User && m.content.as_deref().unwrap().contains("Agent design prose")));
        let mut retry = ContextFragmentLedger::default();
        retry.retain_sampling_history(&ledger);
        retry.record_sampling(10, &modules, vec![Message::system("Goal delivery is still missing")]);
        assert_eq!(retry.sampling_snapshots.len(), 2, "new decision at the same raw anchor was skipped");
        ledger.record_sampling(12, &modules, Vec::new());
        assert_eq!(ledger.sampling_snapshots[1].1.len(), 2, "same selection re-appended guidance");
        ledger.record_sampling(14, &[], Vec::new());
        assert!(ledger.sampling_snapshots[2].1.iter().any(|m| m.content.as_deref().unwrap().contains("Earlier sampling guidance and design data have expired")));
    }

    #[test]
    fn unchanged_large_dynamic_state_is_shared_until_changed_or_retired() {
        let mut ledger = ContextFragmentLedger::default();
        let learner = "PAST_DELIVERIES_AND_READER_EXPRESSIONS ".repeat(1_000);
        ledger.upsert(ContextFragment::new(
            "agent.learner_context", FragmentScope::Dynamic, Role::System,
            &learner, FragmentSensitivity::Private,
        )).unwrap();
        ledger.record_sampling(2, &[], vec![Message::user("CURRENT_AUTHOR_DESIGN")]);
        let initial = ledger.sampling_snapshots[0].clone();
        for anchor in [4, 6, 8] {
            ledger.upsert(ContextFragment::new(
                "budget", FragmentScope::Dynamic, Role::System,
                format!("remaining={}", 10-anchor), FragmentSensitivity::Private,
            )).unwrap();
            ledger.record_sampling(anchor, &[], vec![Message::user("CURRENT_AUTHOR_DESIGN")]);
        }
        assert_eq!(ledger.sampling_snapshots[0], initial, "sent prefixes must stay unchanged");
        let history = serde_json::to_string(&ledger.sampling_snapshots).unwrap();
        assert_eq!(history.matches("PAST_DELIVERIES_AND_READER_EXPRESSIONS").count(), 1_000);
        assert_eq!(history.matches("CURRENT_AUTHOR_DESIGN").count(), 1);
        let appended_bytes = ledger.sampling_snapshots[1..].iter()
            .map(|(_, messages)| serde_json::to_vec(messages).unwrap().len()).sum::<usize>();
        assert!(appended_bytes < learner.len() / 10, "small budget changes repeated the large learner context");

        ledger.record_sampling(10, &[], vec![Message::user("CURRENT_AUTHOR_DESIGN")]);
        assert_eq!(ledger.sampling_snapshots.len(), 4, "an unchanged decision appended another state");
        assert!(ledger.retire_sampling_history());
        let rebased = serde_json::to_string(&ledger.sampling_snapshots).unwrap();
        assert_eq!(rebased.matches("PAST_DELIVERIES_AND_READER_EXPRESSIONS").count(), 1_000);
        assert!(rebased.contains("CURRENT_AUTHOR_DESIGN") && rebased.contains("remaining=2"));
        assert!(!rebased.contains("remaining=6"));
        ledger.record_sampling(12, &[], vec![Message::user("CURRENT_AUTHOR_DESIGN")]);
        assert_eq!(ledger.sampling_snapshots.len(), 1, "retirement lost the comparison baseline");
    }

    #[test]
    fn dynamic_manifests_expire_missing_keys_and_republish_reactivated_bodies() {
        let data = ContextFragment::new(
            "design", FragmentScope::Dynamic, Role::User,
            "PRIOR_DESIGN_BODY", FragmentSensitivity::Private,
        );
        let mut first = ContextFragmentLedger::default();
        first.upsert(data.clone()).unwrap();
        first.record_sampling(2, &[], vec![Message::user("PRIOR_DECISION_DATA")]);

        // Per-decision ledgers inherit sampling history, not transient active rules.
        let mut next = ContextFragmentLedger::default();
        next.retain_sampling_history(&first);
        next.record_sampling(4, &[], Vec::new());
        let removal = &next.sampling_snapshots.last().unwrap().1;
        assert!(removal[0].content.as_deref().unwrap().contains("active_keys=[]"));
        assert!(removal.iter().any(|message| message.content.as_deref()
            .is_some_and(|text| text.contains("runtime_decision_messages.v1\nmessage_count=0"))));
        assert!(!serde_json::to_string(removal).unwrap().contains("PRIOR_DESIGN_BODY"));
        next.upsert(data).unwrap();
        next.record_sampling(6, &[], Vec::new());
        let reactivated = &next.sampling_snapshots.last().unwrap().1;
        assert!(reactivated[0].content.as_deref().unwrap().contains("active_keys=[\"design\"]"));
        assert!(reactivated.iter().any(|message| message.role == Role::User
            && message.content.as_deref().unwrap().contains("PRIOR_DESIGN_BODY")));
        assert!(next.retire_sampling_history());
        let rebased = serde_json::to_string(&next.sampling_snapshots).unwrap();
        assert!(rebased.contains("PRIOR_DESIGN_BODY"));
        assert!(!rebased.contains("PRIOR_DECISION_DATA"));
    }

    #[test]
    fn retained_sampling_retry_at_same_raw_anchor_publishes_changed_state() {
        let mut first = ContextFragmentLedger::default();
        first.upsert(ContextFragment::new(
            "draft", FragmentScope::Dynamic, Role::System,
            "PREMATURE_FINAL", FragmentSensitivity::Private,
        )).unwrap();
        first.record_sampling(2, &[], Vec::new());
        let mut retry = ContextFragmentLedger::default();
        retry.retain_sampling_history(&first);
        retry.upsert(ContextFragment::new(
            "draft", FragmentScope::Dynamic, Role::System,
            "DELIVERY_STILL_REQUIRED", FragmentSensitivity::Private,
        )).unwrap();
        retry.record_sampling(2, &[], Vec::new());
        let mut projected = vec![Message::system("checkpoint"), Message::user("current task")];
        retry.project_sampling_snapshots(&mut projected, 2);
        assert_eq!(retry.sampling_snapshots.len(), 2);
        assert!(projected.last().unwrap().content.as_deref().unwrap().contains("DELIVERY_STILL_REQUIRED"));
        assert!(retry.retire_sampling_history());
        let rebased = serde_json::to_string(&retry.sampling_snapshots).unwrap();
        assert!(!rebased.contains("PREMATURE_FINAL") && rebased.contains("DELIVERY_STILL_REQUIRED"));
    }

    #[test]
    fn sampling_guidance_delta_keeps_manifest_and_retained_definitions() {
        fn guidance(ledger: &ContextFragmentLedger) -> &str {
            ledger.sampling_snapshots.last().unwrap().1.iter()
                .filter_map(|message| message.content.as_deref())
                .find(|content| content.starts_with("sampling_guidance.v1\n"))
                .unwrap()
        }
        fn active_assets(content: &str) -> serde_json::Value {
            serde_json::from_str(content.lines()
                .find_map(|line| line.strip_prefix("active_assets="))
                .expect("each selection must publish its complete active manifest"))
                .unwrap()
        }
        let common = InstructionModule::new("fixture.common", "v1", "Shared policy body");
        let first_phase = InstructionModule::new("fixture.phase", "v1", "First phase body");
        let next_phase = InstructionModule::new("fixture.phase", "v1", "Changed phase body");
        let both_assets = serde_json::json!([
            {"asset_id":"fixture.common", "revision":"v1"},
            {"asset_id":"fixture.phase", "revision":"v1"},
        ]);
        let mut ledger = ContextFragmentLedger::default();
        ledger.record_sampling(10, &[common.clone(), first_phase], Vec::new());
        assert_eq!(active_assets(guidance(&ledger)), both_assets);
        assert!(guidance(&ledger).contains("Shared policy body"));
        assert!(guidance(&ledger).contains("First phase body"));

        // A phase can change its body without changing its asset ID or revision.
        ledger.record_sampling(12, &[common.clone(), next_phase.clone()], Vec::new());
        assert_eq!(active_assets(guidance(&ledger)), both_assets);
        assert!(!guidance(&ledger).contains("Shared policy body"));
        assert!(guidance(&ledger).contains("Changed phase body"));

        let mut retained = ContextFragmentLedger::default();
        retained.retain_sampling_history(&ledger);
        retained.record_sampling(14, std::slice::from_ref(&common), Vec::new());
        assert_eq!(active_assets(guidance(&retained)), serde_json::json!([
            {"asset_id":"fixture.common", "revision":"v1"},
        ]));
        assert!(!guidance(&retained).contains("Shared policy body"));
        assert!(!guidance(&retained).contains("Changed phase body"));

        // Re-activation republishes its definition; unchanged common text stays shared.
        retained.record_sampling(16, &[common, next_phase], Vec::new());
        assert_eq!(active_assets(guidance(&retained)), both_assets);
        assert!(!guidance(&retained).contains("Shared policy body"));
        assert!(guidance(&retained).contains("Changed phase body"));
        retained.record_sampling(18, &[], Vec::new());
        assert_eq!(active_assets(guidance(&retained)), serde_json::json!([]));
        assert!(guidance(&retained).contains("Earlier sampling guidance and design data have expired"));

        let mut compacted = vec![Message::system("checkpoint"), Message::user("current question")];
        compacted.extend((0..8).map(|_| Message::user("raw current-turn message")));
        retained.project_sampling_snapshots(&mut compacted, 18);
        let bodies = compacted.iter().filter_map(|message| message.content.as_deref()).collect::<Vec<_>>();
        assert_eq!(bodies.iter().filter(|body| body.contains("Shared policy body")).count(), 1);
        assert_eq!(bodies.iter().filter(|body| body.contains("First phase body")).count(), 1);
        assert_eq!(bodies.iter().filter(|body| body.contains("Changed phase body")).count(), 2);
        assert!(retained.projected_messages(FragmentScope::TurnFrozen).is_empty());
    }

    #[test]
    fn sampling_history_retirement_preserves_current_state_and_complete_active_definitions() {
        let mut ledger=ContextFragmentLedger::default();
        let common=InstructionModule::new("common","v1","COMMON_DEFINITION");
        let old=InstructionModule::new("phase.old","v1","OBSOLETE_PHASE");
        let current=InstructionModule::new("phase.current","v1","CURRENT_PHASE");
        ledger.upsert(ContextFragment::new("frozen",FragmentScope::TurnFrozen,Role::System,"ORIGINAL_SCENE",FragmentSensitivity::Private)).unwrap();
        ledger.upsert(ContextFragment::new("learner",FragmentScope::Dynamic,Role::System,"OLDER_LEARNER_STATE",FragmentSensitivity::Private)).unwrap();
        ledger.record_sampling(2,&[common.clone(),old],vec![Message::user("OLD_AUTHOR_DESIGN")]);
        ledger.upsert(ContextFragment::new("learner",FragmentScope::Dynamic,Role::System,"CURRENT_LEARNER_STATE",FragmentSensitivity::Private)).unwrap();
        ledger.record_sampling(4,&[common,current.clone()],vec![Message::user("CURRENT_AUTHOR_DESIGN")]);
        let frozen=ledger.projected_messages(FragmentScope::TurnFrozen);
        assert!(ledger.retire_sampling_history());
        assert!(!ledger.retire_sampling_history(),"the retained current state is not repeatedly retired");
        assert_eq!(ledger.projected_messages(FragmentScope::TurnFrozen),frozen);
        let raw=vec![Message::system("base"),Message::user("ORIGINAL_USER"),Message {provider_continuation:None,role:Role::Assistant,content:None,
            tool_calls:vec![crate::ToolCall {id:"call".into(),name:"book.text".into(),arguments:r#"{"lid":"1.1"}"#.into()}],tool_call_id:None},
            Message {provider_continuation:None,role:Role::Tool,content:Some("OBSERVED_TOOL_BODY".into()),tool_calls:vec![],tool_call_id:Some("call".into())}];
        let mut projected=raw.clone(); ledger.project_sampling_snapshots(&mut projected,raw.len());
        assert_eq!(&projected[..raw.len()],raw.as_slice());
        let text=serde_json::to_string(&projected).unwrap();
        for kept in ["CURRENT_LEARNER_STATE","CURRENT_AUTHOR_DESIGN","COMMON_DEFINITION","CURRENT_PHASE"] { assert!(text.contains(kept)); }
        for retired in ["OLDER_LEARNER_STATE","OLD_AUTHOR_DESIGN","OBSOLETE_PHASE"] { assert!(!text.contains(retired)); }
        ledger.record_sampling(6,std::slice::from_ref(&current),vec![]);
        let latest=ledger.sampling_snapshots.last().unwrap().1.iter().filter_map(|message|message.content.as_deref()).collect::<Vec<_>>().join("\n");
        assert!(!latest.contains("CURRENT_PHASE"),"the full rebase remains the reference for later normal deltas");
    }

    #[test]
    #[ignore="requires SAMPLING_PRESSURE_INPUT and SAMPLING_PRESSURE_OUTPUT for a real recorded request"]
    fn recorded_sampling_pressure_retires_only_obsolete_runtime_context() {
        let recorded:serde_json::Value=serde_json::from_slice(&std::fs::read(std::env::var_os("SAMPLING_PRESSURE_INPUT").unwrap()).unwrap()).unwrap();
        let input:Vec<Message>=serde_json::from_value(recorded["input"].clone()).unwrap();
        let tools=recorded["tools"].as_array().unwrap().iter().map(|tool|crate::ToolSpec {
            name:tool["name"].as_str().unwrap().into(),description:tool["description"].as_str().unwrap().into(),parameters:tool["parameters"].clone(),
        }).collect::<Vec<_>>();
        let mut ledger=ContextFragmentLedger::default();
        let mut raw=vec![];
        let mut definitions=BTreeMap::<(String,String),String>::new();
        let mut manifest=vec![];
        for message in &input {
            let text=message.content.as_deref().unwrap_or_default();
            if text.starts_with("runtime_state_snapshot.v1\n") {
                ledger.sampling_snapshots.push((raw.len(),vec![message.clone()]));
            } else if text.starts_with("sampling_guidance.v1\n") {
                manifest=serde_json::from_str::<Vec<serde_json::Value>>(text.lines().find_map(|line|line.strip_prefix("active_assets=")).unwrap()).unwrap();
                for part in text.split("\nasset=").skip(1) {
                    let (id,rest)=part.split_once('\n').unwrap();
                    let (revision,body)=rest.strip_prefix("revision=").unwrap().split_once('\n').unwrap();
                    definitions.insert((id.into(),revision.into()),body.trim().into());
                }
                ledger.sampling_snapshots.last_mut().unwrap().1.push(message.clone());
            } else if text.starts_with("presentation_guidance_state.v1\n") || text.starts_with("context_fragment.v1\nkey=presentation.authoring_context\n") {
                ledger.sampling_snapshots.last_mut().unwrap().1.push(message.clone());
            } else { raw.push(message.clone()); }
        }
        ledger.sampling_instructions=manifest.iter().map(|asset| {
            let id=asset["asset_id"].as_str().unwrap(); let revision=asset["revision"].as_str().unwrap();
            InstructionModule::new(id,revision,definitions[&(id.into(),revision.into())].clone())
        }).collect();
        let snapshots=ledger.sampling_snapshots.len();
        let latest=ledger.sampling_snapshots.last().unwrap().1.clone();
        // This replay consumes recorded complete snapshots from before dynamic
        // deltas. Keep that last opaque state as decision data so the retirement
        // measurement still retains its exact bodies without parsing user prose.
        ledger.sampling_messages=latest.iter().filter(|message| !message.content.as_deref()
            .is_some_and(|text|text.starts_with("sampling_guidance.v1\n"))).cloned().collect();
        let original_raw=raw.clone();
        let mut before=raw.clone(); ledger.project_sampling_snapshots(&mut before,raw.len());
        assert_eq!(before,input,"reconstructed sampling history must reproduce the actual recorded request exactly");
        let mut profile=crate::ModelRuntimeProfile::fallback("recorded-pressure",crate::ProviderToolProtocol::Native);
        profile.context_window_tokens=recorded["active_context"]["context_window_tokens"].as_u64().unwrap() as u32;
        profile.output_reserve_tokens=recorded["active_context"]["output_reserve_tokens"].as_u64().unwrap() as u32;
        profile.safety_margin_tokens=recorded["active_context"]["safety_margin_tokens"].as_u64().unwrap() as u32;
        let plan=|input:&[Message]| {
            let mut messages=vec![Message::system(recorded["instructions"].as_str().unwrap())]; messages.extend_from_slice(input);
            crate::AgentRequestPlan::for_ad_hoc(profile.clone(),&messages,&tools)
        };
        let before_tokens=plan(&before).active_context.estimated_input_tokens;
        let image_tokens=recorded["preview_images"].as_array().unwrap().len() as u32*4096;
        assert_eq!(before_tokens+image_tokens,recorded["active_context"]["estimated_input_tokens"].as_u64().unwrap() as u32);
        assert!(ledger.retire_sampling_history());
        let mut after=raw.clone(); ledger.project_sampling_snapshots(&mut after,raw.len());
        let after_plan=plan(&after);
        assert!(after_plan.active_context.remaining_tokens>=i64::from(image_tokens));
        assert!(after_plan.active_context.estimated_input_tokens<before_tokens);
        assert_eq!(raw,original_raw);
        for message in latest.iter().filter(|message|!message.content.as_deref().unwrap_or_default().starts_with("sampling_guidance.v1\n")) { assert!(after.contains(message)); }
        for module in &ledger.sampling_instructions { assert!(after.iter().any(|message|message.content.as_deref().is_some_and(|text|text.contains(&module.text)))); }
        let report=serde_json::json!({"recorded_input_tokens_with_images":before_tokens+image_tokens,
            "retained_input_tokens_with_images":after_plan.active_context.estimated_input_tokens+image_tokens,
            "snapshots_before":snapshots,"snapshots_after":ledger.sampling_snapshots.len(),"raw_messages_preserved":raw.len(),
            "active_modules_preserved":ledger.sampling_instructions.len(),"fits_with_original_images":true});
        std::fs::write(std::env::var_os("SAMPLING_PRESSURE_OUTPUT").unwrap(),serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        println!("{report}");
    }

    #[test]
    fn context_fragment_projection_is_repeatable_and_revision_collisions_fail_closed() {
        let mut ledger = ContextFragmentLedger::default();
        let profile = fragment(
            "reader.profile_snapshot",
            "reader_profile_snapshot.v1 synthetic",
        );
        ledger.upsert(profile.clone()).unwrap();
        ledger.upsert(ContextFragment::new(
            "stable", FragmentScope::SessionStable, Role::System, "stable session rules",
            FragmentSensitivity::Public,
        )).unwrap();
        ledger.upsert(ContextFragment::new(
            "task_data", FragmentScope::TurnFrozen, Role::User, "frozen task data",
            FragmentSensitivity::Private,
        )).unwrap();
        let raw = vec![
            Message::system("base"), Message::user("previous question"),
            Message {
                role: Role::Assistant, content: Some("previous answer".into()),
                provider_continuation: None, tool_calls: vec![], tool_call_id: None,
            },
            Message::user("current question"),
        ];
        let project = |ledger: &ContextFragmentLedger, messages: &[Message]| {
            let mut projected = ledger.project_session_stable_messages(messages);
            ledger.project_turn_frozen_messages(&mut projected);
            projected
        };
        let first = project(&ledger, &raw);
        let second = project(&ledger, &raw);
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap()
        );
        assert_eq!(
            first
                .iter()
                .filter_map(|message| message.content.as_deref())
                .filter(|content| content.contains("reader_profile_snapshot.v1"))
                .count(),
            1
        );
        assert_eq!(first[0], raw[0]);
        assert!(first[1].content.as_deref().unwrap().contains("stable session rules"));
        assert_eq!(&first[2..4], &raw[1..3]);
        assert_eq!(first[4].role, Role::System);
        assert!(first[4].content.as_deref().unwrap().contains("reader_profile_snapshot.v1"));
        assert_eq!(first[5].role, Role::User);
        assert!(first[5].content.as_deref().unwrap().contains("frozen task data"));
        assert_eq!(first[6], raw[3]);

        let mut collision = profile;
        collision.content = "different content with a forged revision".into();
        assert!(ledger.upsert(collision).is_err());
        ledger.upsert(fragment("reader.profile_snapshot", "updated profile next turn")).unwrap();
        let changed = project(&ledger, &raw);
        assert_eq!(&changed[..4], &first[..4], "updated turn state must not alter the earlier history prefix");
        assert_ne!(changed[4], first[4]);
        assert_eq!(&changed[5..], &first[5..]);
        assert!(!serde_json::to_string(&raw).unwrap().contains(CONTEXT_FRAGMENT_VERSION));

        let no_user = vec![raw[0].clone(), raw[2].clone()];
        let projected = project(&ledger, &no_user);
        assert_eq!(projected[2], no_user[1]);
        assert_eq!(&projected[3..], ledger.projected_messages(FragmentScope::TurnFrozen).as_slice());
    }
}
