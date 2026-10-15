//! Only operation identity and expiry accompany a Run; candidate plaintext stays in memory.
use crate::AgentHistory;
use std::time::{Duration, Instant};
pub const CONFIRMATION_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone)]
pub(crate) struct PendingConfirmation {
    pub id: String,
    pub expires_at: Instant,
}

impl AgentHistory {
    pub(crate) fn arm_confirmation(&mut self, session: &str) {
        // Repeating the same request does not prolong an already pending operation.
        self.pending_confirmations
            .entry(session.into())
            .or_insert_with(|| PendingConfirmation {
                id: uuid::Uuid::now_v7().to_string(),
                expires_at: Instant::now() + CONFIRMATION_TTL,
            });
    }
    pub(crate) fn expire_confirmations(&mut self, now: Instant) {
        let expired: Vec<_> = self
            .pending_confirmations
            .iter()
            .filter(|(_, pending)| now >= pending.expires_at)
            .map(|(id, _)| id.clone())
            .collect();
        for id in expired {
            self.pending_memory_ops.remove(&id);
            self.pending_governance_mutations.remove(&id);
            self.pending_confirmations.remove(&id);
        }
        self.pending_confirmations.retain(|id, _| {
            self.pending_memory_ops.contains_key(id)
                || self.pending_governance_mutations.contains_key(id)
        });
    }
    pub(crate) fn confirmation_id(&self, session: &str) -> Option<&str> {
        self.pending_confirmations
            .get(session)
            .map(|p| p.id.as_str())
    }
}

pub(crate) fn required() -> read_tools::ToolError {
    crate::user_storage_paths::error("SENSITIVE_CONFIRMATION_EXPIRED", "needs_user",
        "The original operation is unavailable or expired; submit it again and confirm the new operation")
}
