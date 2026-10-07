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
    recorded_sampling_anchor: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextFragmentSnapshot {
    pub revision: String,
    pub fragments: Vec<ContextFragment>,
}

impl ContextFragmentLedger {
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

    pub fn project_messages(&self, messages: &[Message]) -> Vec<Message> {
        let insert_at = messages
            .iter()
            .position(|message| message.role != Role::System)
            .unwrap_or(messages.len());
        let fragments = self.projected_messages();
        let mut projected = Vec::with_capacity(messages.len() + fragments.len());
        projected.extend_from_slice(&messages[..insert_at]);
        projected.extend(fragments);
        projected.extend_from_slice(&messages[insert_at..]);
        projected
    }

    pub fn projected_messages(&self) -> Vec<Message> {
        self.snapshot()
            .fragments
            .iter()
            .filter(|fragment| fragment.scope != FragmentScope::Dynamic)
            .map(ContextFragment::projected_message)
            .collect()
    }

    /// Capture each decision's complete dynamic state, including unchanged fields.
    /// A rebuild after compaction reuses this decision's original snapshot.
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
        let mut content = format!(
            "runtime_state_snapshot.v1\nsampling={}\nThe last runtime state snapshot is authoritative. Earlier snapshots describe earlier decisions; rules applying only to those decisions have expired.\n",
            self.sampling_snapshots.len() + 1,
        );
        let mut task_data = Vec::new();
        for fragment in self
            .snapshot()
            .fragments
            .iter()
            .filter(|f| f.scope == FragmentScope::Dynamic)
        {
            if fragment.role == Role::System {
                content.push_str(&format!("\nkey={}\n{}\n", fragment.key, fragment.content));
            } else {
                task_data.push(fragment.projected_message());
            }
        }
        let mut snapshot = vec![Message::system(content)];
        if self.sampling_instructions != instructions {
            let mut content = String::from(
                "sampling_guidance.v1\nThis selection replaces all earlier sampling guidance. It remains active until the next sampling_guidance selection; runtime state snapshots do not expire it. Only the selected phase and references apply.\n",
            );
            if instructions.is_empty() {
                content.push_str("No presentation phase or technical references are active. Earlier presentation guidance and design data have expired.\n");
            }
            for module in instructions {
                content.push_str(&format!("\nasset={}\nrevision={}\n{}\n", module.asset_id, module.revision, module.text));
            }
            snapshot.push(Message::system(content));
            self.sampling_instructions = instructions.to_vec();
        }
        snapshot.extend(task_data);
        snapshot.extend(current_messages);
        self.sampling_snapshots.push((raw_message_count, snapshot));
    }

    /// Retain only sampling history; transient dynamic rules stay on their decision.
    pub fn retain_sampling_history(&mut self, sampled: &Self) {
        self.sampling_snapshots = sampled.sampling_snapshots.clone();
        self.sampling_instructions = sampled.sampling_instructions.clone();
        // A rejected premature final answer can start another sampling without
        // adding raw messages. It still needs fresh budget/Goal state.
        self.recorded_sampling_anchor = None;
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
            Message::user("assistant/tool placeholder"),
            Message::user("complete tool group"),
        ];
        second.project_sampling_snapshots(&mut compacted, 12);
        assert!(compacted[2]
            .content
            .as_deref()
            .unwrap()
            .contains("remaining=2"));
        assert!(compacted[5]
            .content
            .as_deref()
            .unwrap()
            .contains("remaining=1"));
        assert!(second.projected_messages().is_empty());
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
        assert!(ledger.sampling_snapshots[2].1.iter().any(|m| m.content.as_deref().unwrap().contains("Earlier presentation guidance and design data have expired")));
    }

    #[test]
    fn context_fragment_projection_is_repeatable_and_revision_collisions_fail_closed() {
        let mut ledger = ContextFragmentLedger::default();
        let profile = fragment(
            "reader.profile_snapshot",
            "reader_profile_snapshot.v1 synthetic",
        );
        ledger.upsert(profile.clone()).unwrap();
        let first = ledger.project_messages(&[Message::system("base"), Message::user("question")]);
        let second = ledger.project_messages(&[Message::system("base"), Message::user("question")]);
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

        let mut collision = profile;
        collision.content = "different content with a forged revision".into();
        assert!(ledger.upsert(collision).is_err());
    }
}
