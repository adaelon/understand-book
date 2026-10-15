use crate::governance::ProfileGovernanceState;
use crate::profile::{EvidenceExclusion, ProfileFact};
use crate::Record;
use crate::ReviewState;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MEMORY_SCHEMA_VERSION: u32 = 4;
#[cfg(test)]
pub(crate) const PREVIOUS_MEMORY_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryDocument {
    pub schema_version: u32,
    pub document_revision: u64,
    pub projection_revision: u64,
    pub records: Vec<Record>,
    /// Read-only navigation from an earlier note identity to its current record.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) note_replacements: BTreeMap<String, String>,
    #[serde(default)]
    pub profile_facts: Vec<ProfileFact>,
    #[serde(default)]
    pub(crate) review_state: ReviewState,
    #[serde(default)]
    pub exclusions: Vec<EvidenceExclusion>,
    #[serde(default, skip_serializing_if = "ProfileGovernanceState::is_empty")]
    pub(crate) governance_state: ProfileGovernanceState,
}

impl MemoryDocument {
    pub(crate) fn replace_note_identity(&mut self, previous: &str, current: &str) {
        if previous == current { return; }
        for target in self.note_replacements.values_mut() {
            if target == previous { *target = current.to_owned(); }
        }
        self.note_replacements.remove(current);
        self.note_replacements.insert(previous.to_owned(), current.to_owned());
    }

    pub(crate) fn empty() -> MemoryDocument {
        MemoryDocument {
            schema_version: MEMORY_SCHEMA_VERSION,
            document_revision: 0,
            projection_revision: 0,
            records: Vec::new(),
            note_replacements: BTreeMap::new(),
            profile_facts: Vec::new(),
            review_state: ReviewState::default(),
            exclusions: Vec::new(),
            governance_state: ProfileGovernanceState::default(),
        }
    }

    pub(crate) fn from_legacy(records: Vec<Record>) -> MemoryDocument {
        MemoryDocument {
            schema_version: MEMORY_SCHEMA_VERSION,
            document_revision: 1,
            projection_revision: 1,
            records,
            note_replacements: BTreeMap::new(),
            profile_facts: Vec::new(),
            review_state: ReviewState::default(),
            exclusions: Vec::new(),
            governance_state: ProfileGovernanceState::default(),
        }
    }

    pub(crate) fn migrate_previous(mut self) -> Result<MemoryDocument, String> {
        if !matches!(self.schema_version, 2 | 3) {
            return Err(format!(
                "无法从 memory schema_version {} 迁移到 {}",
                self.schema_version, MEMORY_SCHEMA_VERSION
            ));
        }
        self.schema_version = MEMORY_SCHEMA_VERSION;
        self.validate()?;
        Ok(self)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema_version != MEMORY_SCHEMA_VERSION {
            return Err(format!(
                "不支持的 memory schema_version: {} (expected {})",
                self.schema_version, MEMORY_SCHEMA_VERSION
            ));
        }
        if self.projection_revision > self.document_revision {
            return Err(format!(
                "memory projection_revision {} 超过 document_revision {}",
                self.projection_revision, self.document_revision
            ));
        }
        let mut fact_ids = BTreeSet::new();
        for fact in &self.profile_facts {
            fact.validate_persisted().map_err(|error| error.message)?;
            if !fact_ids.insert(fact.fact_id.as_str()) {
                return Err(format!("重复 profile fact_id: {}", fact.fact_id));
            }
        }
        let mut evidence_ids = BTreeSet::new();
        for exclusion in &self.exclusions {
            exclusion.validate().map_err(|error| error.message)?;
            if !evidence_ids.insert(exclusion.evidence_id.as_str()) {
                return Err(format!(
                    "重复 evidence exclusion: {}",
                    exclusion.evidence_id
                ));
            }
        }
        self.review_state.validate()?;
        crate::backfill::validate_historical_backfill_fact_links(
            &self.review_state.historical_backfill_jobs,
            &self.profile_facts,
        )?;
        crate::global_consolidation::validate_promotion_links(
            &self.review_state.global_promotions,
            &self.profile_facts,
        )?;
        self.governance_state
            .validate(self.document_revision, self.projection_revision)?;
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum StoredMemory {
    Document(Box<MemoryDocument>),
    Legacy(Vec<Record>),
}
