use crate::global_consolidation::reconcile_global_promotions;
use crate::governance::collection_rules_block;
use crate::profile::{build_profile_fact, reject_excluded_evidence};
use crate::{
    fnv1a, CreateProfileFact, EvidenceRef, FactSource, MemoryStore, ProfileFact, ProfileScope,
    Sensitivity,
};
use read_tools::ToolError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewJobStatus {
    Queued,
    Running,
    Retryable,
    Completed,
    Failed,
    Cancelled,
}

impl ReviewJobStatus {
    pub fn is_pending(self) -> bool {
        matches!(self, Self::Queued | Self::Running | Self::Retryable)
    }
}

const MAX_REVIEW_ATTEMPTS: u32 = 5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewJob {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub turn_book_ids: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learning_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub goal_revisions: Vec<u32>,
    pub job_id: String,
    pub session_id: String,
    pub book_id: String,
    pub from_turn_exclusive: u64,
    pub to_turn_inclusive: u64,
    pub status: ReviewJobStatus,
    pub attempts: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_attempt_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<ReviewErrorState>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlobalConsolidationJob {
    pub job_id: String,
    pub affected_keys: Vec<String>,
    pub source_revision: u64,
    pub status: ReviewJobStatus,
    pub attempts: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_attempt_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentObservation {
    pub observation_id: String,
    pub intent_key: String,
    pub content_profile: String,
    pub evidence: Vec<EvidenceRef>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewFactCandidate {
    pub candidate_id: String,
    pub fact: CreateProfileFact,
    #[serde(default)]
    pub supersedes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentObservationCandidate {
    pub intent_key: String,
    pub content_profile: String,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewCommitOutcome {
    pub completed_job: ReviewJob,
    pub added_fact_ids: Vec<String>,
    pub added_observation_ids: Vec<String>,
    pub already_completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewErrorState {
    pub error_code: String,
    pub message: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewState {
    #[serde(default)]
    pub historical_baseline_initialized: bool,
    #[serde(default)]
    pub review_jobs: Vec<ReviewJob>,
    #[serde(default)]
    pub historical_backfill_jobs: Vec<crate::HistoricalBackfillJob>,
    #[serde(default)]
    pub consolidation_jobs: Vec<GlobalConsolidationJob>,
    #[serde(default)]
    pub global_promotions: Vec<crate::GlobalPromotionState>,
    #[serde(default)]
    pub intent_observations: Vec<IntentObservation>,
    #[serde(default)]
    pub reviewed_through: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_success_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<ReviewErrorState>,
}

impl ReviewState {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        let mut job_ids = BTreeSet::new();
        let mut session_books = BTreeMap::<&str, &str>::new();
        let mut active_by_session = BTreeMap::<&str, Vec<&ReviewJob>>::new();
        for job in &self.review_jobs {
            job.validate()?;
            if !job_ids.insert(job.job_id.as_str()) {
                return Err(format!("duplicate review job_id: {}", job.job_id));
            }
            match session_books.get(job.session_id.as_str()) {
                Some(book_id) if *book_id != job.book_id => {
                    return Err(format!(
                        "review session {} spans multiple books",
                        job.session_id
                    ));
                }
                Some(_) => {}
                None => {
                    session_books.insert(&job.session_id, &job.book_id);
                }
            }
            let watermark = self
                .reviewed_through
                .get(&job.session_id)
                .copied()
                .unwrap_or(0);
            if job.status == ReviewJobStatus::Completed {
                if job.to_turn_inclusive > watermark {
                    return Err(format!(
                        "completed review job {} exceeds session watermark {}",
                        job.job_id, watermark
                    ));
                }
            } else if job.status.is_pending() {
                active_by_session
                    .entry(&job.session_id)
                    .or_default()
                    .push(job);
            }
        }

        for (session_id, jobs) in &mut active_by_session {
            jobs.sort_by(|left, right| {
                left.from_turn_exclusive
                    .cmp(&right.from_turn_exclusive)
                    .then_with(|| left.to_turn_inclusive.cmp(&right.to_turn_inclusive))
                    .then_with(|| left.job_id.cmp(&right.job_id))
            });
            let mut covered_through = self.reviewed_through.get(*session_id).copied().unwrap_or(0);
            for job in jobs {
                if job.from_turn_exclusive != covered_through {
                    return Err(format!(
                        "review jobs for session {session_id} are not contiguous at {covered_through}"
                    ));
                }
                covered_through = job.to_turn_inclusive;
            }
        }

        for session_id in self.reviewed_through.keys() {
            if session_id.trim().is_empty() {
                return Err("reviewed_through session_id must not be empty".into());
            }
        }
        validate_optional_timestamp("last_success_at", self.last_success_at.as_deref())?;
        if let Some(error) = &self.last_error {
            error.validate()?;
        }
        validate_consolidation_jobs(&self.consolidation_jobs)?;
        crate::backfill::validate_historical_backfill_jobs(&self.historical_backfill_jobs)?;
        crate::global_consolidation::validate_promotion_states(&self.global_promotions)?;
        validate_intent_observations(&self.intent_observations)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewTurnOrigin {
    pub turn_id: String,
    pub user_turn_ordinal: u64,
    pub book_id: String,
    pub goal_revision: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewSessionCursor {
    pub learning_workspace_id: Option<String>,
    pub turns: Vec<ReviewTurnOrigin>,
    pub session_id: String,
    pub book_id: String,
    pub latest_user_turn_ordinal: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewReconciliation {
    pub created_job_ids: Vec<String>,
    pub requeued_job_ids: Vec<String>,
}

impl MemoryStore {
    pub fn review_state(&self) -> &ReviewState {
        &self.document.review_state
    }

    pub fn initialize_review_watermark_baseline(
        &mut self,
        sessions: &[ReviewSessionCursor],
        now: &str,
    ) -> Result<Vec<String>, ToolError> {
        validate_required("review baseline timestamp", now)?;
        if self.document.review_state.historical_baseline_initialized {
            return Ok(Vec::new());
        }
        let sessions = normalize_session_cursors(sessions)?;
        let mut candidate = self.document_mutation_candidate()?;
        let state = &mut candidate.review_state;
        let mut baselined = Vec::new();
        if state.review_jobs.is_empty() && state.reviewed_through.is_empty() {
            for cursor in sessions.values() {
                state
                    .reviewed_through
                    .insert(cursor.session_id.clone(), cursor.latest_user_turn_ordinal);
                baselined.push(cursor.session_id.clone());
            }
        }
        state.historical_baseline_initialized = true;
        self.commit_document(candidate)?;
        Ok(baselined)
    }

    pub fn reconcile_review_jobs(
        &mut self,
        sessions: &[ReviewSessionCursor],
        now: &str,
    ) -> Result<ReviewReconciliation, ToolError> {
        self.reconcile_review_jobs_inner(sessions, now, false)
    }

    pub fn resume_review_jobs(
        &mut self,
        sessions: &[ReviewSessionCursor],
        now: &str,
    ) -> Result<ReviewReconciliation, ToolError> {
        self.reconcile_review_jobs_inner(sessions, now, true)
    }

    pub fn claim_review_job(&mut self, job_id: &str, now: &str) -> Result<ReviewJob, ToolError> {
        validate_required("review transition timestamp", now)?;
        let mut state = self.document.review_state.clone();
        let current = state.review_jobs.iter().find(|job| job.job_id == job_id)
            .ok_or_else(|| review_job_not_found(job_id))?;
        let watermark = state.reviewed_through.get(&current.session_id).copied().unwrap_or(0);
        if current.from_turn_exclusive != watermark {
            return Err(review_state_conflict("previous review range has not completed".into()));
        }
        let job = find_job_mut(&mut state, job_id)?;
        if !matches!(
            job.status,
            ReviewJobStatus::Queued | ReviewJobStatus::Retryable
        ) {
            return Err(review_state_conflict(format!(
                "cannot claim {:?} review job: {job_id}",
                job.status
            )));
        }
        job.attempts = job
            .attempts
            .checked_add(1)
            .ok_or_else(|| invalid_review_state("review job attempts overflow"))?;
        job.status = ReviewJobStatus::Running;
        job.next_attempt_at = None;
        job.updated_at = now.into();
        let claimed = job.clone();
        self.commit_review_state(state)?;
        Ok(claimed)
    }

    pub fn mark_review_job_retryable(
        &mut self,
        job_id: &str,
        next_attempt_at: &str,
        error: ReviewErrorState,
        now: &str,
    ) -> Result<ReviewJob, ToolError> {
        validate_required("review transition timestamp", now)?;
        validate_required("review next_attempt_at", next_attempt_at)?;
        error.validate().map_err(invalid_review_state)?;
        let mut state = self.document.review_state.clone();
        let job = find_job_mut(&mut state, job_id)?;
        if job.status != ReviewJobStatus::Running {
            return Err(review_state_conflict(format!(
                "cannot retry {:?} review job: {job_id}",
                job.status
            )));
        }
        job.status = ReviewJobStatus::Retryable;
        job.next_attempt_at = Some(next_attempt_at.into());
        job.last_error = Some(error.clone());
        job.updated_at = now.into();
        if job.attempts >= MAX_REVIEW_ATTEMPTS {
            let session_id = job.session_id.clone();
            stop_session_jobs(&mut state, &session_id, ReviewJobStatus::Failed, &error, now);
        }
        let job = find_job_mut(&mut state, job_id)?;
        let retryable = job.clone();
        state.last_error = Some(error);
        self.commit_review_state(state)?;
        Ok(retryable)
    }

    /// Stop the whole unfinished chain without pretending its evidence was reviewed.
    pub fn fail_review_job(
        &mut self,
        job_id: &str,
        error: ReviewErrorState,
        now: &str,
    ) -> Result<(), ToolError> {
        validate_required("review transition timestamp", now)?;
        error.validate().map_err(invalid_review_state)?;
        let mut state = self.document.review_state.clone();
        let job = find_job_mut(&mut state, job_id)?;
        if !job.status.is_pending() {
            return Ok(());
        }
        let session_id = job.session_id.clone();
        stop_session_jobs(&mut state, &session_id, ReviewJobStatus::Failed, &error, now);
        state.last_error = Some(error);
        self.commit_review_state(state)
    }

    pub fn cancel_session_review_jobs(
        &mut self,
        session_id: &str,
        error: ReviewErrorState,
        now: &str,
    ) -> Result<(), ToolError> {
        validate_required("review transition timestamp", now)?;
        error.validate().map_err(invalid_review_state)?;
        let mut state = self.document.review_state.clone();
        if !stop_session_jobs(&mut state, session_id, ReviewJobStatus::Cancelled, &error, now) {
            return Ok(());
        }
        state.last_error = Some(error);
        clear_last_error_if_drained(&mut state);
        self.commit_review_state(state)
    }

    pub fn complete_review_job(&mut self, job_id: &str, now: &str) -> Result<ReviewJob, ToolError> {
        validate_required("review transition timestamp", now)?;
        let mut state = self.document.review_state.clone();
        let index = state
            .review_jobs
            .iter()
            .position(|job| job.job_id == job_id)
            .ok_or_else(|| review_job_not_found(job_id))?;
        let current = state.review_jobs[index].clone();
        if current.status == ReviewJobStatus::Completed {
            let watermark = state
                .reviewed_through
                .get(&current.session_id)
                .copied()
                .unwrap_or(0);
            if watermark < current.to_turn_inclusive {
                return Err(invalid_review_state(format!(
                    "completed review job {job_id} exceeds its watermark"
                )));
            }
            return Ok(current);
        }
        if current.status != ReviewJobStatus::Running {
            return Err(review_state_conflict(format!(
                "cannot complete {:?} review job: {job_id}",
                current.status
            )));
        }
        let watermark = state
            .reviewed_through
            .get(&current.session_id)
            .copied()
            .unwrap_or(0);
        if watermark != current.from_turn_exclusive {
            return Err(review_state_conflict(format!(
                "review job {job_id} starts at {}, current watermark is {watermark}",
                current.from_turn_exclusive
            )));
        }

        let job = &mut state.review_jobs[index];
        job.status = ReviewJobStatus::Completed;
        job.next_attempt_at = None;
        job.last_error = None;
        job.updated_at = now.into();
        let completed = job.clone();
        state
            .reviewed_through
            .insert(current.session_id, current.to_turn_inclusive);
        state.last_success_at = Some(now.into());
        clear_last_error_if_drained(&mut state);
        self.commit_review_state(state)?;
        Ok(completed)
    }

    pub fn record_review_error(&mut self, error: ReviewErrorState) -> Result<(), ToolError> {
        error.validate().map_err(invalid_review_state)?;
        let mut state = self.document.review_state.clone();
        state.last_error = Some(error);
        self.commit_review_state(state)
    }

    pub fn commit_review_result(
        &mut self,
        job_id: &str,
        eligible_turn_ids: &[String],
        reviewed_facts: &[ProfileFact],
        fact_candidates: &[ReviewFactCandidate],
        intent_candidates: &[IntentObservationCandidate],
        now: &str,
    ) -> Result<ReviewCommitOutcome, ToolError> {
        validate_required("review commit timestamp", now)?;
        let current = self
            .document
            .review_state
            .review_jobs
            .iter()
            .find(|job| job.job_id == job_id)
            .cloned()
            .ok_or_else(|| review_job_not_found(job_id))?;
        if current.status == ReviewJobStatus::Completed {
            return Ok(ReviewCommitOutcome {
                completed_job: current,
                added_fact_ids: Vec::new(),
                added_observation_ids: Vec::new(),
                already_completed: true,
            });
        }
        if current.status != ReviewJobStatus::Running {
            return Err(review_state_conflict(format!(
                "cannot commit result for {:?} review job: {job_id}",
                current.status
            )));
        }
        let watermark = self
            .document
            .review_state
            .reviewed_through
            .get(&current.session_id)
            .copied()
            .unwrap_or(0);
        if watermark != current.from_turn_exclusive {
            return Err(review_state_conflict(format!(
                "review job {job_id} starts at {}, current watermark is {watermark}",
                current.from_turn_exclusive
            )));
        }

        let eligible_turn_ids = normalize_eligible_turn_ids(eligible_turn_ids, &current)?;
        let mut facts = Vec::<ProfileFact>::new();
        let mut candidate_ids = BTreeSet::new();
        for candidate in fact_candidates {
            candidate.validate()?;
            if !candidate_ids.insert(candidate.candidate_id.as_str()) {
                return Err(invalid_review_result("duplicate review fact candidate"));
            }
            validate_review_evidence(
                &candidate.fact.evidence,
                &current.session_id,
                &eligible_turn_ids,
            )?;
            if matches!(
                &candidate.fact.scope,
                ProfileScope::Book { book_id } | ProfileScope::Workspace { book_id, .. }
                    if !candidate.fact.evidence.iter().all(|e| match e { EvidenceRef::Turn { turn_id, .. } => current.turn_book_ids.get(turn_id).unwrap_or(&current.book_id) == book_id, _ => false })
            ) {
                return Err(invalid_review_result(
                    "review fact book scope does not match the review job",
                ));
            }
            reject_excluded_evidence(&self.document.exclusions, &candidate.fact.evidence)?;
            let mut fact = candidate.fact.clone();
            match (&current.learning_workspace_id, &fact.scope) {
                (Some(learning_workspace_id), ProfileScope::Book { book_id }) => {
                    fact.scope = ProfileScope::Workspace { learning_workspace_id: learning_workspace_id.clone(), book_id: book_id.clone() };
                }
                (owner, ProfileScope::Workspace { learning_workspace_id, .. })
                    if owner.as_ref() != Some(learning_workspace_id) => {
                        return Err(invalid_review_result("review fact workspace scope does not match the review job"));
                    }
                _ => {}
            }
            if let Some(id) = candidate.supersedes.first() {
                let target = reviewed_facts.iter().find(|old| old.fact_id == *id)
                    .ok_or_else(|| invalid_review_result("revision target was not offered to this review"))?;
                fact.payload.reuse_key_from(&target.payload)?;
            }
            if collection_rules_block(
                self.collection_rules(),
                &fact.scope,
                &fact.applicability,
                &fact.payload,
            ) {
                continue;
            }
            let mut fact = build_profile_fact(fact, candidate.supersedes.clone(), now)?;
            fact.observed_at_ms = candidate.observed_at_ms;
            facts.push(fact);
        }
        facts.sort_by(|left, right| left.fact_id.cmp(&right.fact_id));

        let mut observations = Vec::new();
        let mut observation_ids = BTreeSet::<String>::new();
        for candidate in intent_candidates {
            let observation = candidate.build(now)?;
            validate_review_evidence(
                &observation.evidence,
                &current.session_id,
                &eligible_turn_ids,
            )?;
            reject_excluded_evidence(&self.document.exclusions, &observation.evidence)?;
            if !observation_ids.insert(observation.observation_id.clone()) {
                return Err(invalid_review_result(
                    "duplicate review intent observation candidate",
                ));
            }
            observations.push(observation);
        }
        observations.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));

        let mut profile_facts = self.document.profile_facts.clone();
        let (added_fact_ids, affected_keys) = crate::review_revision::reconcile(
            &mut profile_facts, facts, reviewed_facts, now,
        )?;
        let existing_observation_ids: BTreeSet<_> = self
            .document
            .review_state
            .intent_observations
            .iter()
            .map(|observation| observation.observation_id.as_str())
            .collect();
        let new_observations: Vec<_> = observations
            .into_iter()
            .filter(|observation| {
                !existing_observation_ids.contains(observation.observation_id.as_str())
            })
            .collect();

        let mut candidate = if added_fact_ids.is_empty() {
            self.document_mutation_candidate()?
        } else {
            self.projection_mutation_candidate()?
        };
        let added_observation_ids: Vec<String> = new_observations
            .iter()
            .map(|observation| observation.observation_id.clone())
            .collect();
        if !added_fact_ids.is_empty() {
            candidate.profile_facts = profile_facts;
        }
        candidate
            .review_state
            .intent_observations
            .extend(new_observations);
        let index = candidate
            .review_state
            .review_jobs
            .iter()
            .position(|job| job.job_id == job_id)
            .expect("review job was resolved from the same document");
        let job = &mut candidate.review_state.review_jobs[index];
        job.status = ReviewJobStatus::Completed;
        job.next_attempt_at = None;
        job.last_error = None;
        job.updated_at = now.into();
        let completed_job = job.clone();
        candidate
            .review_state
            .reviewed_through
            .insert(current.session_id, current.to_turn_inclusive);
        candidate.review_state.last_success_at = Some(now.into());
        clear_last_error_if_drained(&mut candidate.review_state);
        reconcile_global_promotions(&mut candidate, &affected_keys, now)?;
        self.commit_document(candidate)?;
        if !added_fact_ids.is_empty() {
            let _ = self.write_profile_files();
        }
        Ok(ReviewCommitOutcome {
            completed_job,
            added_fact_ids,
            added_observation_ids,
            already_completed: false,
        })
    }

    fn reconcile_review_jobs_inner(
        &mut self,
        sessions: &[ReviewSessionCursor],
        now: &str,
        recover_running: bool,
    ) -> Result<ReviewReconciliation, ToolError> {
        validate_required("review reconciliation timestamp", now)?;
        let sessions = normalize_session_cursors(sessions)?;
        let original = self.document.review_state.clone();
        let mut state = original.clone();
        let mut outcome = ReviewReconciliation::default();

        if recover_running {
            // resume receives the complete, successfully loaded chat inventory.
            // A deleted JSONL must not resurrect an orphan on every server start.
            let orphans: BTreeSet<_> = state.review_jobs.iter()
                .filter(|job| (job.status.is_pending() || job.status == ReviewJobStatus::Failed)
                    && !sessions.contains_key(&job.session_id))
                .map(|job| job.session_id.clone()).collect();
            for session_id in orphans {
                let error = ReviewErrorState {
                    error_code: "REVIEW_INPUT_MISSING".into(),
                    message: "Review source session no longer exists".into(),
                    occurred_at: now.into(),
                };
                stop_session_jobs(&mut state, &session_id, ReviewJobStatus::Cancelled, &error, now);
                state.last_error = Some(error);
            }
            let mismatched: BTreeSet<_> = state.review_jobs.iter()
                .filter(|job| job.status.is_pending() && sessions.get(&job.session_id).is_some_and(|cursor|
                    cursor.book_id != job.book_id
                        || cursor.learning_workspace_id != job.learning_workspace_id
                        || cursor.latest_user_turn_ordinal < job.to_turn_inclusive))
                .map(|job| job.session_id.clone()).collect();
            for session_id in mismatched {
                let error = ReviewErrorState {
                    error_code: "REVIEW_INPUT_MISMATCH".into(),
                    message: "Review source no longer matches the saved job range or ownership".into(),
                    occurred_at: now.into(),
                };
                stop_session_jobs(&mut state, &session_id, ReviewJobStatus::Failed, &error, now);
                state.last_error = Some(error);
            }
            let exhausted: BTreeSet<_> = state.review_jobs.iter()
                .filter(|job| job.status.is_pending() && job.attempts >= MAX_REVIEW_ATTEMPTS)
                .map(|job| job.session_id.clone()).collect();
            for session_id in exhausted {
                let error = ReviewErrorState {
                    error_code: "REVIEW_RETRY_EXHAUSTED".into(),
                    message: "Automatic review attempt limit reached; source evidence remains unreviewed".into(),
                    occurred_at: now.into(),
                };
                stop_session_jobs(&mut state, &session_id, ReviewJobStatus::Failed, &error, now);
                state.last_error = Some(error);
            }
            for job in &mut state.review_jobs {
                if job.status == ReviewJobStatus::Running {
                    job.status = ReviewJobStatus::Queued;
                    job.next_attempt_at = None;
                    job.updated_at = now.into();
                    outcome.requeued_job_ids.push(job.job_id.clone());
                }
            }
        }

        for cursor in sessions.values() {
            reconcile_session(&mut state, cursor, now, &mut outcome)?;
        }
        clear_last_error_if_drained(&mut state);
        sort_review_jobs(&mut state.review_jobs);
        state.validate().map_err(invalid_review_state)?;
        outcome.created_job_ids.sort();
        outcome.requeued_job_ids.sort();
        if state == original {
            return Ok(ReviewReconciliation::default());
        }
        self.commit_review_state(state)?;
        Ok(outcome)
    }

    fn commit_review_state(&mut self, state: ReviewState) -> Result<(), ToolError> {
        state.validate().map_err(invalid_review_state)?;
        let mut candidate = self.document_mutation_candidate()?;
        candidate.review_state = state;
        self.commit_document(candidate)
    }
}

impl ReviewJob {
    pub fn stable_id(
        session_id: &str,
        book_id: &str,
        from_turn_exclusive: u64,
        to_turn_inclusive: u64,
    ) -> String {
        #[derive(Serialize)]
        struct Identity<'a> {
            session_id: &'a str,
            book_id: &'a str,
            from_turn_exclusive: u64,
            to_turn_inclusive: u64,
        }
        let canonical = serde_json::to_string(&Identity {
            session_id,
            book_id,
            from_turn_exclusive,
            to_turn_inclusive,
        })
        .expect("review job identity has fixed serializable fields");
        format!("review_{:016x}", fnv1a(&canonical))
    }

    fn validate(&self) -> Result<(), String> {
        if self.session_id.trim().is_empty() || self.book_id.trim().is_empty() {
            return Err("review job session_id/book_id must not be empty".into());
        }
        if self.from_turn_exclusive >= self.to_turn_inclusive {
            return Err(format!(
                "review job {} has an empty turn range",
                self.job_id
            ));
        }
        let expected = Self::stable_id(
            &self.session_id,
            &self.book_id,
            self.from_turn_exclusive,
            self.to_turn_inclusive,
        );
        if self.job_id != expected {
            return Err(format!(
                "review job_id is not content-addressed: {}",
                self.job_id
            ));
        }
        if self.created_at.trim().is_empty() || self.updated_at.trim().is_empty() {
            return Err(format!(
                "review job {} timestamps must not be empty",
                self.job_id
            ));
        }
        if matches!(
            self.status,
            ReviewJobStatus::Running | ReviewJobStatus::Retryable | ReviewJobStatus::Completed
        ) && self.attempts == 0
        {
            return Err(format!(
                "review job {} status {:?} requires an attempt",
                self.job_id, self.status
            ));
        }
        match (self.status, self.next_attempt_at.as_deref()) {
            (ReviewJobStatus::Retryable, Some(value)) if !value.trim().is_empty() => {}
            (ReviewJobStatus::Retryable, _) => {
                return Err(format!(
                    "retryable review job {} requires next_attempt_at",
                    self.job_id
                ));
            }
            (_, None) => {}
            (_, Some(_)) => {
                return Err(format!(
                    "review job {} has next_attempt_at outside retryable state",
                    self.job_id
                ));
            }
        }
        if let Some(error) = &self.last_error {
            error.validate()?;
        }
        Ok(())
    }
}

impl ReviewErrorState {
    fn validate(&self) -> Result<(), String> {
        if self.error_code.trim().is_empty()
            || self.message.trim().is_empty()
            || self.occurred_at.trim().is_empty()
        {
            return Err("review error code/message/occurred_at must not be empty".into());
        }
        Ok(())
    }
}

impl GlobalConsolidationJob {
    pub fn stable_id(affected_keys: &[String], source_revision: u64) -> String {
        #[derive(Serialize)]
        struct Identity<'a> {
            affected_keys: &'a [String],
            source_revision: u64,
        }
        let affected_keys = normalized_strings(affected_keys);
        let canonical = serde_json::to_string(&Identity {
            affected_keys: &affected_keys,
            source_revision,
        })
        .expect("consolidation job identity has fixed serializable fields");
        format!("consolidation_{:016x}", fnv1a(&canonical))
    }
}

impl IntentObservation {
    pub fn stable_id(intent_key: &str, content_profile: &str, evidence: &[EvidenceRef]) -> String {
        #[derive(Serialize)]
        struct Identity<'a> {
            intent_key: &'a str,
            content_profile: &'a str,
            evidence: &'a [EvidenceRef],
        }
        let mut evidence = evidence.to_vec();
        evidence.sort();
        evidence.dedup();
        let canonical = serde_json::to_string(&Identity {
            intent_key,
            content_profile,
            evidence: &evidence,
        })
        .expect("intent observation identity has fixed serializable fields");
        format!("observation_{:016x}", fnv1a(&canonical))
    }
}

impl ReviewFactCandidate {
    pub fn new(fact: CreateProfileFact) -> Result<Self, ToolError> {
        Self::revision(fact, Vec::new(), None)
    }

    pub fn revision(mut fact: CreateProfileFact, mut supersedes: Vec<String>, observed_at_ms: Option<u64>) -> Result<Self, ToolError> {
        fact.evidence.sort();
        fact.evidence.dedup();
        supersedes.sort();
        supersedes.dedup();
        if !matches!(
            fact.source,
            FactSource::UserStated | FactSource::AgentInferred
        ) {
            return Err(invalid_review_result(
                "review extractor may only emit user-stated or agent-inferred facts",
            ));
        }
        if fact.sensitivity != Sensitivity::Normal {
            return Err(invalid_review_result(
                "background review may not store sensitive profile facts",
            ));
        }
        let built = build_profile_fact(fact.clone(), supersedes.clone(), "candidate")?;
        Ok(Self {
            candidate_id: built.fact_id.replacen("fact_", "candidate_", 1),
            fact,
            supersedes,
            observed_at_ms,
        })
    }

    pub(crate) fn validate(&self) -> Result<(), ToolError> {
        if Self::revision(self.fact.clone(), self.supersedes.clone(), self.observed_at_ms)? != *self {
            return Err(invalid_review_result(
                "review candidate_id is not content-addressed",
            ));
        }
        Ok(())
    }
}

impl IntentObservationCandidate {
    pub fn new(
        intent_key: impl Into<String>,
        content_profile: impl Into<String>,
        mut evidence: Vec<EvidenceRef>,
    ) -> Result<Self, ToolError> {
        let intent_key = intent_key.into();
        let content_profile = content_profile.into();
        evidence.sort();
        evidence.dedup();
        let candidate = Self {
            intent_key,
            content_profile,
            evidence,
        };
        candidate.build("candidate")?;
        Ok(candidate)
    }

    fn build(&self, now: &str) -> Result<IntentObservation, ToolError> {
        validate_required("intent observation timestamp", now)?;
        if self.intent_key.trim().is_empty()
            || self.content_profile.trim().is_empty()
            || self.evidence.is_empty()
        {
            return Err(invalid_review_result(
                "intent observation fields must not be empty",
            ));
        }
        let mut evidence = self.evidence.clone();
        evidence.sort();
        evidence.dedup();
        if evidence != self.evidence {
            return Err(invalid_review_result(
                "intent observation evidence must be normalized",
            ));
        }
        Ok(IntentObservation {
            observation_id: IntentObservation::stable_id(
                &self.intent_key,
                &self.content_profile,
                &evidence,
            ),
            intent_key: self.intent_key.clone(),
            content_profile: self.content_profile.clone(),
            evidence,
            created_at: now.into(),
        })
    }
}

fn normalize_eligible_turn_ids(
    turn_ids: &[String],
    job: &ReviewJob,
) -> Result<BTreeSet<String>, ToolError> {
    let normalized: BTreeSet<_> = turn_ids
        .iter()
        .map(|turn_id| turn_id.trim().to_string())
        .collect();
    let expected = job.to_turn_inclusive - job.from_turn_exclusive;
    if normalized.iter().any(String::is_empty)
        || u64::try_from(normalized.len()).ok() != Some(expected)
    {
        return Err(invalid_review_result(
            "eligible user turn IDs do not cover the review job range",
        ));
    }
    Ok(normalized)
}

fn validate_review_evidence(
    evidence: &[EvidenceRef],
    session_id: &str,
    eligible_turn_ids: &BTreeSet<String>,
) -> Result<(), ToolError> {
    if evidence.is_empty()
        || evidence.iter().any(|evidence| {
            !matches!(
                evidence,
                EvidenceRef::Turn {
                    session_id: evidence_session_id,
                    turn_id,
                } if evidence_session_id == session_id && eligible_turn_ids.contains(turn_id)
            )
        })
    {
        return Err(invalid_review_result(
            "review output must cite eligible resident user turns",
        ));
    }
    Ok(())
}

fn normalize_session_cursors(
    sessions: &[ReviewSessionCursor],
) -> Result<BTreeMap<String, ReviewSessionCursor>, ToolError> {
    let mut normalized = BTreeMap::new();
    for cursor in sessions {
        validate_required("review cursor session_id", &cursor.session_id)?;
        validate_required("review cursor book_id", &cursor.book_id)?;
        match normalized.get(&cursor.session_id) {
            Some(existing) if existing != cursor => {
                return Err(invalid_review_state(format!(
                    "conflicting review cursors for session {}",
                    cursor.session_id
                )));
            }
            Some(_) => {}
            None => {
                normalized.insert(cursor.session_id.clone(), cursor.clone());
            }
        }
    }
    Ok(normalized)
}

fn reconcile_session(
    state: &mut ReviewState,
    cursor: &ReviewSessionCursor,
    now: &str,
    outcome: &mut ReviewReconciliation,
) -> Result<(), ToolError> {
    if state.review_jobs.iter().any(|job| job.session_id == cursor.session_id
        && job.status == ReviewJobStatus::Cancelled) {
        return Ok(());
    }
    // New source evidence may recover a failed batch, but an unchanged or still
    // invalid input must never restart it merely because reconciliation ran.
    let failed_through = state.review_jobs.iter()
        .filter(|job| job.session_id == cursor.session_id && job.status == ReviewJobStatus::Failed)
        .map(|job| job.to_turn_inclusive).max();
    if let Some(failed_through) = failed_through {
        if cursor.latest_user_turn_ordinal <= failed_through
            || state.review_jobs.iter().any(|job| job.session_id == cursor.session_id
                && (job.book_id != cursor.book_id || job.learning_workspace_id != cursor.learning_workspace_id)) {
            return Ok(());
        }
        let watermark = state.reviewed_through.get(&cursor.session_id).copied().unwrap_or(0);
        let turns: Vec<_> = cursor.turns.iter().filter(|turn|
            turn.user_turn_ordinal > watermark && turn.user_turn_ordinal <= cursor.latest_user_turn_ordinal).collect();
        if turns.len() as u64 != cursor.latest_user_turn_ordinal.saturating_sub(watermark)
            || turns.iter().enumerate().any(|(index, turn)| turn.user_turn_ordinal != watermark + index as u64 + 1) {
            return Ok(());
        }
    }
    if let Some(job) = state
        .review_jobs
        .iter()
        .find(|job| job.session_id == cursor.session_id && (job.book_id != cursor.book_id || job.learning_workspace_id != cursor.learning_workspace_id))
    {
        return Err(invalid_review_state(format!(
            "review cursor book {} conflicts with job {} book {}",
            cursor.book_id, job.job_id, job.book_id
        )));
    }
    let watermark = state
        .reviewed_through
        .get(&cursor.session_id)
        .copied()
        .unwrap_or(0);
    if cursor.latest_user_turn_ordinal <= watermark {
        return Ok(());
    }

    let mut active_indices: Vec<usize> = state
        .review_jobs
        .iter()
        .enumerate()
        .filter(|(_, job)| {
            job.session_id == cursor.session_id && job.status.is_pending()
        })
        .map(|(index, _)| index)
        .collect();
    active_indices.sort_by_key(|index| state.review_jobs[*index].from_turn_exclusive);
    let covered_through = active_indices
        .last()
        .map(|index| state.review_jobs[*index].to_turn_inclusive)
        .unwrap_or(watermark);
    if cursor.latest_user_turn_ordinal <= covered_through {
        return Ok(());
    }

    if let Some(index) = active_indices.last().copied() {
        let previous = &state.review_jobs[index];
        if previous.status == ReviewJobStatus::Queued
            && previous.to_turn_inclusive == covered_through
        {
            let previous = state.review_jobs.remove(index);
            let mut merged = new_review_job(
                &cursor.session_id,
                &cursor.book_id,
                previous.from_turn_exclusive,
                cursor.latest_user_turn_ordinal,
                previous.attempts,
                &previous.created_at,
                now,
            );
            bind_review_origin(&mut merged, cursor);
            outcome.created_job_ids.push(merged.job_id.clone());
            state.review_jobs.push(merged);
            return Ok(());
        }
    }

    let mut job = new_review_job(
        &cursor.session_id,
        &cursor.book_id,
        covered_through,
        cursor.latest_user_turn_ordinal,
        0,
        now,
        now,
    );
    bind_review_origin(&mut job, cursor);
    outcome.created_job_ids.push(job.job_id.clone());
    state.review_jobs.push(job);
    Ok(())
}

fn bind_review_origin(job: &mut ReviewJob, cursor: &ReviewSessionCursor) {
    job.learning_workspace_id = cursor.learning_workspace_id.clone();
    let turns: Vec<_> = cursor.turns.iter().filter(|turn| turn.user_turn_ordinal > job.from_turn_exclusive && turn.user_turn_ordinal <= job.to_turn_inclusive).collect();
    job.goal_revisions = turns.iter().filter_map(|turn|turn.goal_revision).collect::<BTreeSet<_>>().into_iter().collect();
    job.turn_book_ids = turns.iter().map(|turn|(turn.turn_id.clone(),turn.book_id.clone())).collect();
}

fn new_review_job(
    session_id: &str,
    book_id: &str,
    from_turn_exclusive: u64,
    to_turn_inclusive: u64,
    attempts: u32,
    created_at: &str,
    updated_at: &str,
) -> ReviewJob {
    ReviewJob {
        turn_book_ids: BTreeMap::new(),
        learning_workspace_id: None,
        goal_revisions: Vec::new(),
        job_id: ReviewJob::stable_id(session_id, book_id, from_turn_exclusive, to_turn_inclusive),
        session_id: session_id.into(),
        book_id: book_id.into(),
        from_turn_exclusive,
        to_turn_inclusive,
        status: ReviewJobStatus::Queued,
        attempts,
        next_attempt_at: None,
        last_error: None,
        created_at: created_at.into(),
        updated_at: updated_at.into(),
    }
}

fn stop_session_jobs(
    state: &mut ReviewState,
    session_id: &str,
    status: ReviewJobStatus,
    error: &ReviewErrorState,
    now: &str,
) -> bool {
    let mut changed = false;
    for job in &mut state.review_jobs {
        let cancelling_failed = status == ReviewJobStatus::Cancelled
            && job.status == ReviewJobStatus::Failed;
        if job.session_id == session_id && (job.status.is_pending() || cancelling_failed) {
            job.status = status;
            job.next_attempt_at = None;
            // Deleting a failed source resolves its outstanding status; the
            // original failure remains useful diagnostic evidence on its receipt.
            if !cancelling_failed || job.last_error.is_none() {
                job.last_error = Some(error.clone());
            }
            job.updated_at = now.into();
            changed = true;
        }
    }
    changed
}

fn find_job_mut<'a>(
    state: &'a mut ReviewState,
    job_id: &str,
) -> Result<&'a mut ReviewJob, ToolError> {
    state
        .review_jobs
        .iter_mut()
        .find(|job| job.job_id == job_id)
        .ok_or_else(|| review_job_not_found(job_id))
}

fn sort_review_jobs(jobs: &mut [ReviewJob]) {
    jobs.sort_by(|left, right| {
        left.session_id
            .cmp(&right.session_id)
            .then_with(|| left.from_turn_exclusive.cmp(&right.from_turn_exclusive))
            .then_with(|| left.to_turn_inclusive.cmp(&right.to_turn_inclusive))
            .then_with(|| left.job_id.cmp(&right.job_id))
    });
}

fn clear_last_error_if_drained(state: &mut ReviewState) {
    if state
        .review_jobs
        .iter()
        .all(|job| !job.status.is_pending() && (job.status != ReviewJobStatus::Failed
            || job.to_turn_inclusive <= state.reviewed_through.get(&job.session_id).copied().unwrap_or(0)))
    {
        state.last_error = None;
    }
}

fn validate_consolidation_jobs(jobs: &[GlobalConsolidationJob]) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    for job in jobs {
        if job.affected_keys.is_empty() || job.affected_keys.iter().any(|key| key.trim().is_empty())
        {
            return Err("invalid global consolidation job".into());
        }
        if job.affected_keys != normalized_strings(&job.affected_keys)
            || job.job_id
                != GlobalConsolidationJob::stable_id(&job.affected_keys, job.source_revision)
        {
            return Err(format!(
                "consolidation job_id is not content-addressed: {}",
                job.job_id
            ));
        }
        if matches!(
            job.status,
            ReviewJobStatus::Running | ReviewJobStatus::Retryable | ReviewJobStatus::Completed
        ) && job.attempts == 0
        {
            return Err(format!(
                "consolidation job {} status {:?} requires an attempt",
                job.job_id, job.status
            ));
        }
        match (job.status, job.next_attempt_at.as_deref()) {
            (ReviewJobStatus::Retryable, Some(value)) if !value.trim().is_empty() => {}
            (ReviewJobStatus::Retryable, _) => {
                return Err(format!(
                    "retryable consolidation job {} requires next_attempt_at",
                    job.job_id
                ));
            }
            (_, None) => {}
            (_, Some(_)) => {
                return Err(format!(
                    "consolidation job {} has next_attempt_at outside retryable state",
                    job.job_id
                ));
            }
        }
        if !ids.insert(job.job_id.as_str()) {
            return Err(format!("duplicate consolidation job_id: {}", job.job_id));
        }
    }
    Ok(())
}

fn validate_intent_observations(observations: &[IntentObservation]) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    for observation in observations {
        if observation.intent_key.trim().is_empty()
            || observation.content_profile.trim().is_empty()
            || observation.evidence.is_empty()
            || observation.created_at.trim().is_empty()
        {
            return Err("invalid intent observation".into());
        }
        let mut normalized_evidence = observation.evidence.clone();
        normalized_evidence.sort();
        normalized_evidence.dedup();
        if observation.evidence != normalized_evidence
            || observation.observation_id
                != IntentObservation::stable_id(
                    &observation.intent_key,
                    &observation.content_profile,
                    &observation.evidence,
                )
        {
            return Err(format!(
                "intent observation_id is not content-addressed: {}",
                observation.observation_id
            ));
        }
        if !ids.insert(observation.observation_id.as_str()) {
            return Err(format!(
                "duplicate intent observation_id: {}",
                observation.observation_id
            ));
        }
    }
    Ok(())
}

fn normalized_strings(values: &[String]) -> Vec<String> {
    let mut normalized = values.to_vec();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn validate_optional_timestamp(name: &str, value: Option<&str>) -> Result<(), String> {
    if value.is_some_and(|timestamp| timestamp.trim().is_empty()) {
        return Err(format!("{name} must not be empty"));
    }
    Ok(())
}

fn validate_required(name: &str, value: &str) -> Result<(), ToolError> {
    if value.trim().is_empty() {
        return Err(invalid_review_state(format!("{name} must not be empty")));
    }
    Ok(())
}

fn invalid_review_state(message: impl Into<String>) -> ToolError {
    ToolError {
        error_code: "INVALID_REVIEW_STATE".into(),
        category: "validation".into(),
        message: message.into(),
    }
}

fn invalid_review_result(message: impl Into<String>) -> ToolError {
    ToolError {
        error_code: "INVALID_REVIEW_RESULT".into(),
        category: "validation".into(),
        message: message.into(),
    }
}

fn review_job_not_found(job_id: &str) -> ToolError {
    ToolError {
        error_code: "REVIEW_JOB_NOT_FOUND".into(),
        category: "not_found".into(),
        message: format!("review job does not exist: {job_id}"),
    }
}

fn review_state_conflict(message: String) -> ToolError {
    ToolError {
        error_code: "REVIEW_JOB_STATE_CONFLICT".into(),
        category: "conflict".into(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Applicability, Confidence, PreferenceClaim, ProfilePayload};
    use std::path::PathBuf;

    fn store(name: &str) -> (PathBuf, MemoryStore) {
        let dir = std::env::temp_dir().join(format!("ub-review-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("memory.json");
        let store = MemoryStore::open(&path).unwrap();
        (path, store)
    }

    fn cursor(latest: u64) -> ReviewSessionCursor {
        ReviewSessionCursor {
            turns: Vec::new(),
            learning_workspace_id: None,
            session_id: "session-a".into(),
            book_id: "book-a".into(),
            latest_user_turn_ordinal: latest,
        }
    }

    #[test]
    fn one_time_baseline_skips_pre_upgrade_history_but_not_new_turns() {
        let (path, mut store) = store("historical-baseline");
        let baselined = store
            .initialize_review_watermark_baseline(&[cursor(3)], "baseline")
            .unwrap();
        assert_eq!(baselined, vec!["session-a"]);
        assert!(store.review_state().historical_baseline_initialized);
        assert_eq!(store.review_state().reviewed_through["session-a"], 3);
        assert!(store.review_state().review_jobs.is_empty());
        let revision = store.document_revision();

        assert!(store
            .initialize_review_watermark_baseline(&[cursor(9)], "duplicate")
            .unwrap()
            .is_empty());
        assert_eq!(store.document_revision(), revision);
        assert!(store
            .reconcile_review_jobs(&[cursor(3)], "same-history")
            .unwrap()
            .created_job_ids
            .is_empty());
        let next = store
            .reconcile_review_jobs(&[cursor(4)], "new-turn")
            .unwrap();
        assert_eq!(next.created_job_ids.len(), 1);
        assert_eq!(only_job(&store).from_turn_exclusive, 3);
        assert_eq!(only_job(&store).to_turn_inclusive, 4);

        let reopened = MemoryStore::open(path).unwrap();
        assert!(reopened.review_state().historical_baseline_initialized);
        assert_eq!(reopened.review_state().reviewed_through["session-a"], 3);
    }

    fn only_job(store: &MemoryStore) -> ReviewJob {
        assert_eq!(store.review_state().review_jobs.len(), 1);
        store.review_state().review_jobs[0].clone()
    }

    fn fact_candidate(
        session_id: &str,
        turn_id: &str,
        source: FactSource,
        scope: ProfileScope,
    ) -> ReviewFactCandidate {
        ReviewFactCandidate::new(CreateProfileFact {
            scope,
            applicability: Applicability::Any,
            payload: ProfilePayload::ExplanationPreference(PreferenceClaim {
                key: format!("example_order_{turn_id}"),
                value: "worked_examples_first".into(),
            }),
            source,
            evidence: vec![EvidenceRef::Turn {
                session_id: session_id.into(),
                turn_id: turn_id.into(),
            }],
            confidence: (source == FactSource::AgentInferred).then_some(Confidence::Medium),
            sensitivity: Sensitivity::Normal,
            valid_until: None,
        })
        .unwrap()
    }

    #[test]
    fn deleted_failed_review_is_cancelled_without_erasing_its_diagnostic() {
        for deletion in ["explicit", "startup"] {
            let (path, mut store) = store(&format!("deleted-failed-review-{deletion}"));
            store.reconcile_review_jobs(&[cursor(1)], "1").unwrap();
            let job = only_job(&store);
            store.claim_review_job(&job.job_id, "2").unwrap();
            let failure = ReviewErrorState {
                error_code: "REVIEW_INPUT_GAP".into(),
                message: "source range was incomplete".into(), occurred_at: "3".into(),
            };
            store.fail_review_job(&job.job_id, failure.clone(), "3").unwrap();
            assert!(store.review_state().last_error.is_some());
            drop(store);
            let mut store = MemoryStore::open(&path).unwrap();
            if deletion == "explicit" {
                store.cancel_session_review_jobs(&job.session_id, ReviewErrorState {
                    error_code: "REVIEW_SOURCE_DELETED".into(),
                    message: "source chat deleted".into(), occurred_at: "4".into(),
                }, "4").unwrap();
            } else {
                store.resume_review_jobs(&[], "4").unwrap();
            }
            let cancelled = only_job(&store);
            assert_eq!(cancelled.status, ReviewJobStatus::Cancelled);
            assert_eq!(cancelled.attempts, 1);
            assert_eq!(cancelled.last_error.as_ref(), Some(&failure));
            assert_eq!(cancelled.updated_at, "4");
            assert!(store.review_state().last_error.is_none());
            assert!(store.review_state().reviewed_through.is_empty());

            let mut healthy = cursor(1);
            healthy.session_id = "healthy-session".into();
            store.reconcile_review_jobs(&[healthy.clone()], "5").unwrap();
            let healthy_job = store.review_state().review_jobs.iter()
                .find(|job| job.session_id == healthy.session_id).unwrap().clone();
            store.claim_review_job(&healthy_job.job_id, "6").unwrap();
            store.complete_review_job(&healthy_job.job_id, "7").unwrap();
            assert!(store.review_state().last_error.is_none());
            assert_eq!(store.review_state().reviewed_through["healthy-session"], 1);
            assert!(!store.review_state().reviewed_through.contains_key("session-a"));
            let expected = store.review_state().clone();
            drop(store);
            let mut reopened = MemoryStore::open(&path).unwrap();
            reopened.resume_review_jobs(&[healthy], "8").unwrap();
            assert_eq!(reopened.review_state(), &expected);
        }
    }

    #[test]
    fn missing_review_source_is_cancelled_on_resume_without_losing_attempts() {
        let (path, mut store) = store("orphan-review-resume");
        store.reconcile_review_jobs(&[cursor(1)], "1").unwrap();
        let mut state = store.review_state().clone();
        state.review_jobs[0].status = ReviewJobStatus::Retryable;
        state.review_jobs[0].attempts = 24_008;
        state.review_jobs[0].next_attempt_at = Some("2".into());
        store.commit_review_state(state).unwrap();

        store.resume_review_jobs(&[], "3").unwrap();
        let orphan = only_job(&store);
        assert_eq!(orphan.status, ReviewJobStatus::Cancelled);
        assert_eq!(orphan.attempts, 24_008);
        assert_eq!(orphan.next_attempt_at, None);
        assert_eq!(orphan.last_error.as_ref().unwrap().error_code, "REVIEW_INPUT_MISSING");
        assert!(store.review_state().reviewed_through.is_empty());
        let revision = store.document_revision();
        store.resume_review_jobs(&[], "4").unwrap();
        assert_eq!(store.document_revision(), revision);
        assert_eq!(MemoryStore::open(path).unwrap().review_state(), store.review_state());
    }

    #[test]
    fn review_retry_limit_stops_its_chain_but_other_sessions_can_complete() {
        let (_path, mut store) = store("bounded-review-retry");
        store.reconcile_review_jobs(&[cursor(1)], "1").unwrap();
        let first = only_job(&store);
        store.claim_review_job(&first.job_id, "2").unwrap();
        store.reconcile_review_jobs(&[cursor(2)], "3").unwrap();
        let tail = store.review_state().review_jobs[1].clone();
        assert!(store.claim_review_job(&tail.job_id, "3").is_err(),
            "a later range must not spend model capacity before its predecessor completes");
        let error = ReviewErrorState {
            error_code: "REVIEW_EXECUTOR_FAILED".into(),
            message: "temporary upstream failure".into(), occurred_at: "4".into(),
        };
        for attempt in 1..=MAX_REVIEW_ATTEMPTS {
            if attempt > 1 { store.claim_review_job(&first.job_id, "4").unwrap(); }
            store.mark_review_job_retryable(&first.job_id, "5", error.clone(), "4").unwrap();
        }
        assert!(store.review_state().review_jobs.iter().all(|job| job.status == ReviewJobStatus::Failed));
        assert_eq!(store.review_state().review_jobs[0].attempts, MAX_REVIEW_ATTEMPTS);
        assert_eq!(store.review_state().review_jobs[1].attempts, 0);
        assert_eq!(store.review_state().review_jobs[0].last_error.as_ref(), Some(&error));
        assert!(store.claim_review_job(&first.job_id, "6").is_err());
        assert!(store.review_state().reviewed_through.is_empty());

        let mut healthy = cursor(1);
        healthy.session_id = "healthy-session".into();
        store.resume_review_jobs(&[cursor(2), healthy], "7").unwrap();
        let healthy = store.review_state().review_jobs.iter()
            .find(|job| job.session_id == "healthy-session").unwrap().clone();
        store.claim_review_job(&healthy.job_id, "8").unwrap();
        store.complete_review_job(&healthy.job_id, "9").unwrap();
        assert_eq!(store.review_state().reviewed_through["healthy-session"], 1);
        assert!(!store.review_state().reviewed_through.contains_key("session-a"));
        assert_eq!(store.review_state().review_jobs.iter().filter(|job| job.session_id == "session-a").count(), 2);

        let mut new_input = cursor(3);
        new_input.turns = (1..=3).map(|ordinal| ReviewTurnOrigin {
            turn_id: format!("turn-{ordinal}"), user_turn_ordinal: ordinal,
            book_id: "book-a".into(), goal_revision: None,
        }).collect();
        store.reconcile_review_jobs(&[new_input], "10").unwrap();
        let resumed = store.review_state().review_jobs.iter()
            .find(|job| job.session_id == "session-a" && job.status == ReviewJobStatus::Queued)
            .unwrap().clone();
        assert_eq!((resumed.from_turn_exclusive, resumed.to_turn_inclusive), (0, 3));
        assert_eq!(resumed.attempts, 0);
        store.claim_review_job(&resumed.job_id, "11").unwrap();
        store.complete_review_job(&resumed.job_id, "12").unwrap();
        assert_eq!(store.review_state().reviewed_through["session-a"], 3);
        assert_eq!(store.review_state().review_jobs.iter().filter(|job| job.status == ReviewJobStatus::Failed).count(), 2);
        assert!(store.review_state().last_error.is_none());
    }

    #[test]
    fn cancelled_review_rejects_late_results_and_is_not_resumed() {
        let (_path, mut store) = store("cancel-inflight-review");
        store.reconcile_review_jobs(&[cursor(1)], "1").unwrap();
        let job = only_job(&store);
        store.claim_review_job(&job.job_id, "2").unwrap();
        store.cancel_session_review_jobs(&job.session_id, ReviewErrorState {
            error_code: "REVIEW_SOURCE_DELETED".into(), message: "chat deleted".into(),
            occurred_at: "3".into(),
        }, "3").unwrap();
        let revision = store.document_revision();
        let error = store.commit_review_result(&job.job_id, &["turn-1".into()], &[], &[], &[], "4").unwrap_err();
        assert_eq!(error.error_code, "REVIEW_JOB_STATE_CONFLICT");
        store.resume_review_jobs(&[], "5").unwrap();
        assert_eq!(store.document_revision(), revision);
        assert_eq!(only_job(&store).status, ReviewJobStatus::Cancelled);
        assert!(store.review_state().reviewed_through.is_empty());
    }

    #[test]
    fn resume_stops_exhausted_or_mismatched_review_inputs() {
        let (_path, mut store) = store("resume-invalid-review");
        store.reconcile_review_jobs(&[cursor(2)], "1").unwrap();
        store.resume_review_jobs(&[cursor(1)], "2").unwrap();
        assert_eq!(only_job(&store).status, ReviewJobStatus::Failed);
        assert_eq!(only_job(&store).last_error.unwrap().error_code, "REVIEW_INPUT_MISMATCH");

        let mut changed = cursor(3);
        changed.book_id = "different-book".into();
        store.reconcile_review_jobs(&[changed], "3").unwrap();
        let mut gapped = cursor(3);
        gapped.turns = vec![ReviewTurnOrigin {
            turn_id: "turn-3".into(), user_turn_ordinal: 3, book_id: "book-a".into(), goal_revision: None,
        }];
        store.reconcile_review_jobs(&[gapped], "3").unwrap();
        assert_eq!(store.review_state().review_jobs.len(), 1, "new input must not revive mismatched or incomplete evidence");
        assert_eq!(only_job(&store).status, ReviewJobStatus::Failed);

        let (_path, mut store) = self::store("resume-exhausted-review");
        store.reconcile_review_jobs(&[cursor(1)], "1").unwrap();
        let mut state = store.review_state().clone();
        state.review_jobs[0].status = ReviewJobStatus::Running;
        state.review_jobs[0].attempts = 73;
        store.commit_review_state(state).unwrap();
        store.resume_review_jobs(&[cursor(1)], "2").unwrap();
        assert_eq!(only_job(&store).status, ReviewJobStatus::Failed);
        assert_eq!(only_job(&store).attempts, 73);
        assert_eq!(only_job(&store).last_error.unwrap().error_code, "REVIEW_RETRY_EXHAUSTED");
    }

    #[test]
    fn resume_requeues_interrupted_running_job() {
        let (_path, mut store) = store("resume-running");
        store
            .reconcile_review_jobs(&[cursor(4)], "2026-07-14T00:00:00Z")
            .unwrap();
        let queued = only_job(&store);
        store
            .claim_review_job(&queued.job_id, "2026-07-14T00:01:00Z")
            .unwrap();

        let outcome = store
            .resume_review_jobs(&[cursor(4)], "2026-07-14T00:02:00Z")
            .unwrap();

        assert_eq!(outcome.requeued_job_ids, vec![queued.job_id.clone()]);
        let recovered = only_job(&store);
        assert_eq!(recovered.status, ReviewJobStatus::Queued);
        assert_eq!(recovered.attempts, 1);
        assert_eq!(recovered.next_attempt_at, None);
    }

    #[test]
    fn repeated_reconciliation_is_idempotent_and_merges_unclaimed_tail() {
        let (_path, mut store) = store("reconcile-idempotent");
        let first = store
            .reconcile_review_jobs(&[cursor(2)], "2026-07-14T00:00:00Z")
            .unwrap();
        assert_eq!(first.created_job_ids.len(), 1);
        let first_revision = store.document_revision();
        assert_eq!(store.projection_revision(), 0);

        let duplicate = store
            .reconcile_review_jobs(&[cursor(2)], "2026-07-14T00:01:00Z")
            .unwrap();
        assert_eq!(duplicate, ReviewReconciliation::default());
        assert_eq!(store.document_revision(), first_revision);

        store
            .reconcile_review_jobs(&[cursor(5)], "2026-07-14T00:02:00Z")
            .unwrap();
        let merged = only_job(&store);
        assert_eq!(
            (merged.from_turn_exclusive, merged.to_turn_inclusive),
            (0, 5)
        );
        assert_ne!(merged.job_id, first.created_job_ids[0]);
        assert_eq!(store.projection_revision(), 0);
    }

    #[test]
    fn running_job_can_retry_and_is_durable() {
        let (path, mut store) = store("retryable-durable");
        store
            .reconcile_review_jobs(&[cursor(2)], "2026-07-14T00:00:00Z")
            .unwrap();
        let queued = only_job(&store);
        store
            .claim_review_job(&queued.job_id, "2026-07-14T00:01:00Z")
            .unwrap();
        let error = ReviewErrorState {
            error_code: "PROVIDER_FAILED".into(),
            message: "temporary failure".into(),
            occurred_at: "2026-07-14T00:02:00Z".into(),
        };
        let retryable = store
            .mark_review_job_retryable(
                &queued.job_id,
                "2026-07-14T00:10:00Z",
                error.clone(),
                "2026-07-14T00:02:00Z",
            )
            .unwrap();
        assert_eq!(retryable.status, ReviewJobStatus::Retryable);
        assert_eq!(
            retryable.next_attempt_at.as_deref(),
            Some("2026-07-14T00:10:00Z")
        );
        assert_eq!(store.review_state().last_error.as_ref(), Some(&error));
        assert_eq!(store.projection_revision(), 0);

        let reopened = MemoryStore::open(path).unwrap();
        assert_eq!(reopened.review_state(), store.review_state());
    }

    #[test]
    fn completed_job_is_terminal_and_watermark_never_moves_backwards() {
        let (_path, mut store) = store("completed-terminal");
        store
            .reconcile_review_jobs(&[cursor(3)], "2026-07-14T00:00:00Z")
            .unwrap();
        let queued = only_job(&store);
        store
            .claim_review_job(&queued.job_id, "2026-07-14T00:01:00Z")
            .unwrap();
        let completed = store
            .complete_review_job(&queued.job_id, "2026-07-14T00:02:00Z")
            .unwrap();
        assert_eq!(completed.status, ReviewJobStatus::Completed);
        assert_eq!(store.review_state().reviewed_through["session-a"], 3);
        let revision = store.document_revision();

        let duplicate = store
            .complete_review_job(&queued.job_id, "2026-07-14T00:03:00Z")
            .unwrap();
        assert_eq!(duplicate, completed);
        assert_eq!(store.review_state().reviewed_through["session-a"], 3);
        assert_eq!(store.document_revision(), revision);
        assert!(store
            .reconcile_review_jobs(&[cursor(2)], "2026-07-14T00:04:00Z")
            .unwrap()
            .created_job_ids
            .is_empty());
        assert_eq!(store.review_state().reviewed_through["session-a"], 3);
        assert_eq!(
            store
                .claim_review_job(&queued.job_id, "2026-07-14T00:05:00Z")
                .unwrap_err()
                .error_code,
            "REVIEW_JOB_STATE_CONFLICT"
        );
    }

    #[test]
    fn illegal_transitions_are_rejected_without_mutation() {
        let (_path, mut store) = store("illegal-transition");
        store
            .reconcile_review_jobs(&[cursor(1)], "2026-07-14T00:00:00Z")
            .unwrap();
        let queued = only_job(&store);
        let revision = store.document_revision();
        let error = ReviewErrorState {
            error_code: "PROVIDER_FAILED".into(),
            message: "temporary failure".into(),
            occurred_at: "2026-07-14T00:01:00Z".into(),
        };

        assert_eq!(
            store
                .mark_review_job_retryable(
                    &queued.job_id,
                    "2026-07-14T00:10:00Z",
                    error,
                    "2026-07-14T00:01:00Z",
                )
                .unwrap_err()
                .error_code,
            "REVIEW_JOB_STATE_CONFLICT"
        );
        assert_eq!(
            store
                .complete_review_job(&queued.job_id, "2026-07-14T00:02:00Z")
                .unwrap_err()
                .error_code,
            "REVIEW_JOB_STATE_CONFLICT"
        );
        assert_eq!(store.document_revision(), revision);
        assert_eq!(only_job(&store), queued);
    }

    #[test]
    fn reconciliation_recovers_cross_file_crash_orders_with_one_job() {
        let (path, mut store) = store("cross-file-crash");
        store
            .reconcile_review_jobs(&[cursor(1)], "2026-07-14T00:00:00Z")
            .unwrap();
        drop(store);

        let mut reopened = MemoryStore::open(&path).unwrap();
        reopened
            .resume_review_jobs(&[cursor(2)], "2026-07-14T00:01:00Z")
            .unwrap();
        let rebuilt = only_job(&reopened);
        assert_eq!(
            (rebuilt.from_turn_exclusive, rebuilt.to_turn_inclusive),
            (0, 2)
        );
        let revision = reopened.document_revision();

        reopened
            .resume_review_jobs(&[cursor(2)], "2026-07-14T00:02:00Z")
            .unwrap();
        assert_eq!(reopened.review_state().review_jobs.len(), 1);
        assert_eq!(reopened.document_revision(), revision);
    }

    #[test]
    fn failed_review_commit_preserves_memory_and_disk() {
        let (path, mut store) = store("failed-commit");
        store
            .reconcile_review_jobs(&[cursor(1)], "2026-07-14T00:00:00Z")
            .unwrap();
        let job = only_job(&store);
        let before_state = store.review_state().clone();
        let before_revision = store.document_revision();
        let before_disk = std::fs::read_to_string(&path).unwrap();

        let blocker = std::env::temp_dir().join("ub-review-test-parent-blocker");
        let _ = std::fs::remove_file(&blocker);
        let _ = std::fs::remove_dir_all(&blocker);
        std::fs::write(&blocker, "not a directory").unwrap();
        store.path = blocker.join("memory.json");

        let error = store
            .claim_review_job(&job.job_id, "2026-07-14T00:01:00Z")
            .unwrap_err();
        assert_eq!(error.category, "internal");
        assert_eq!(store.review_state(), &before_state);
        assert_eq!(store.document_revision(), before_revision);
        assert_eq!(std::fs::read_to_string(path).unwrap(), before_disk);
    }

    #[test]
    fn open_rejects_tampered_job_identity() {
        let (path, mut store) = store("tampered-state");
        store
            .reconcile_review_jobs(&[cursor(2)], "2026-07-14T00:00:00Z")
            .unwrap();
        let mut persisted: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        persisted["review_state"]["review_jobs"][0]["job_id"] =
            serde_json::Value::String("review_forged".into());
        std::fs::write(&path, serde_json::to_string_pretty(&persisted).unwrap()).unwrap();
        drop(store);

        let error = MemoryStore::open(path).err().unwrap();
        assert_eq!(error.category, "internal");
        assert!(error.message.contains("not content-addressed"));
    }

    #[test]
    fn open_rejects_completed_job_ahead_of_watermark() {
        let (path, mut store) = store("tampered-watermark");
        store
            .reconcile_review_jobs(&[cursor(2)], "2026-07-14T00:00:00Z")
            .unwrap();
        let queued = only_job(&store);
        store
            .claim_review_job(&queued.job_id, "2026-07-14T00:01:00Z")
            .unwrap();
        store
            .complete_review_job(&queued.job_id, "2026-07-14T00:02:00Z")
            .unwrap();
        let mut persisted: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        persisted["review_state"]["reviewed_through"]["session-a"] = serde_json::Value::from(1);
        std::fs::write(&path, serde_json::to_string_pretty(&persisted).unwrap()).unwrap();
        drop(store);

        let error = MemoryStore::open(path).err().unwrap();
        assert_eq!(error.category, "internal");
        assert!(error.message.contains("exceeds session watermark"));
    }

    #[test]
    fn review_result_atomically_applies_trust_status_observation_and_watermark() {
        let (path, mut store) = store("atomic-result");
        store
            .reconcile_review_jobs(&[cursor(2)], "2026-07-14T00:00:00Z")
            .unwrap();
        let job = only_job(&store);
        store
            .claim_review_job(&job.job_id, "2026-07-14T00:01:00Z")
            .unwrap();
        let before_document_revision = store.document_revision();
        let before_projection_revision = store.projection_revision();
        let user_stated = fact_candidate(
            "session-a",
            "turn-1",
            FactSource::UserStated,
            ProfileScope::Book {
                book_id: "book-a".into(),
            },
        );
        let global_inference = fact_candidate(
            "session-a",
            "turn-2",
            FactSource::AgentInferred,
            ProfileScope::Global,
        );
        let observation = IntentObservationCandidate::new(
            "request_diagram",
            "technical_learning",
            vec![EvidenceRef::Turn {
                session_id: "session-a".into(),
                turn_id: "turn-2".into(),
            }],
        )
        .unwrap();

        let outcome = store
            .commit_review_result(
                &job.job_id,
                &["turn-1".into(), "turn-2".into()],
                &[],
                &[user_stated, global_inference],
                &[observation],
                "2026-07-14T00:02:00Z",
            )
            .unwrap();

        assert!(!outcome.already_completed);
        assert_eq!(outcome.added_fact_ids.len(), 2);
        assert_eq!(outcome.added_observation_ids.len(), 1);
        assert_eq!(store.document_revision(), before_document_revision + 1);
        assert_eq!(store.projection_revision(), before_projection_revision + 1);
        assert_eq!(store.review_state().reviewed_through["session-a"], 2);
        assert_eq!(only_job(&store).status, ReviewJobStatus::Completed);
        assert_eq!(
            store
                .profile_facts()
                .iter()
                .find(|fact| fact.source == FactSource::UserStated)
                .unwrap()
                .status,
            crate::FactStatus::Confirmed
        );
        assert_eq!(
            store
                .profile_facts()
                .iter()
                .find(|fact| fact.source == FactSource::AgentInferred)
                .unwrap()
                .status,
            crate::FactStatus::Pending
        );
        assert_eq!(store.review_state().intent_observations.len(), 1);
        assert_eq!(
            store
                .resolve_profile_facts(&crate::ProfileResolutionContext {
                    book_id: Some("book-a".into()),
                    ..Default::default()
                })
                .len(),
            1,
            "pending global inference and intent observations must not enter projection"
        );

        let revision = store.document_revision();
        let duplicate = store
            .commit_review_result(
                &job.job_id,
                &["turn-1".into(), "turn-2".into()],
                &[],
                &[],
                &[],
                "2026-07-14T00:03:00Z",
            )
            .unwrap();
        assert!(duplicate.already_completed);
        assert_eq!(store.document_revision(), revision);
        let reopened = MemoryStore::open(path).unwrap();
        assert_eq!(reopened.profile_facts(), store.profile_facts());
        assert_eq!(reopened.review_state(), store.review_state());
    }

    #[test]
    fn repeated_intent_observations_do_not_change_profile_snapshot() {
        let (_path, mut store) = store("observation-only");
        let request = crate::SnapshotRequest::current(crate::SnapshotContext {
            book_id: Some("book-a".into()),
            content_profile: Some("technical_learning".into()),
            ..Default::default()
        });
        let snapshot_before = store.project_reader_profile_snapshot(&request);
        store
            .reconcile_review_jobs(&[cursor(1)], "2026-07-14T00:00:00Z")
            .unwrap();
        let job = only_job(&store);
        store
            .claim_review_job(&job.job_id, "2026-07-14T00:01:00Z")
            .unwrap();
        let projection_revision = store.projection_revision();
        let observation = IntentObservationCandidate::new(
            "request_diagram",
            "technical_learning",
            vec![EvidenceRef::Turn {
                session_id: "session-a".into(),
                turn_id: "turn-1".into(),
            }],
        )
        .unwrap();

        store
            .commit_review_result(
                &job.job_id,
                &["turn-1".into()],
                &[],
                &[],
                &[observation],
                "2026-07-14T00:02:00Z",
            )
            .unwrap();

        store
            .reconcile_review_jobs(&[cursor(2)], "2026-07-14T00:03:00Z")
            .unwrap();
        let second_job = store
            .review_state()
            .review_jobs
            .iter()
            .find(|job| job.status == ReviewJobStatus::Queued)
            .unwrap()
            .clone();
        store
            .claim_review_job(&second_job.job_id, "2026-07-14T00:04:00Z")
            .unwrap();
        let repeated_observation = IntentObservationCandidate::new(
            "request_diagram",
            "technical_learning",
            vec![EvidenceRef::Turn {
                session_id: "session-a".into(),
                turn_id: "turn-2".into(),
            }],
        )
        .unwrap();
        store
            .commit_review_result(
                &second_job.job_id,
                &["turn-2".into()],
                &[],
                &[],
                &[repeated_observation],
                "2026-07-14T00:05:00Z",
            )
            .unwrap();

        assert_eq!(store.projection_revision(), projection_revision);
        assert!(store.profile_facts().is_empty());
        assert_eq!(store.review_state().intent_observations.len(), 2);
        assert_eq!(
            store.project_reader_profile_snapshot(&request),
            snapshot_before
        );
    }

    #[test]
    fn ineligible_review_evidence_rejects_the_whole_result_without_mutation() {
        let (path, mut store) = store("invalid-evidence-atomic");
        store
            .reconcile_review_jobs(&[cursor(1)], "2026-07-14T00:00:00Z")
            .unwrap();
        let job = only_job(&store);
        store
            .claim_review_job(&job.job_id, "2026-07-14T00:01:00Z")
            .unwrap();
        let before_state = store.review_state().clone();
        let before_revision = store.document_revision();
        let before_disk = std::fs::read_to_string(&path).unwrap();
        let invalid = fact_candidate(
            "visitor-session",
            "turn-1",
            FactSource::UserStated,
            ProfileScope::Book {
                book_id: "book-a".into(),
            },
        );

        let error = store
            .commit_review_result(
                &job.job_id,
                &["turn-1".into()],
                &[],
                &[invalid],
                &[],
                "2026-07-14T00:02:00Z",
            )
            .unwrap_err();

        assert_eq!(error.error_code, "INVALID_REVIEW_RESULT");
        assert_eq!(store.review_state(), &before_state);
        assert!(store.profile_facts().is_empty());
        assert_eq!(store.document_revision(), before_revision);
        assert_eq!(std::fs::read_to_string(path).unwrap(), before_disk);
    }

    #[test]
    fn failed_atomic_result_persistence_keeps_facts_observations_and_watermark_unchanged() {
        let (path, mut store) = store("result-persist-failure");
        store
            .reconcile_review_jobs(&[cursor(1)], "2026-07-14T00:00:00Z")
            .unwrap();
        let job = only_job(&store);
        store
            .claim_review_job(&job.job_id, "2026-07-14T00:01:00Z")
            .unwrap();
        let before_state = store.review_state().clone();
        let before_revision = store.document_revision();
        let before_disk = std::fs::read_to_string(&path).unwrap();
        let fact = fact_candidate(
            "session-a",
            "turn-1",
            FactSource::UserStated,
            ProfileScope::Book {
                book_id: "book-a".into(),
            },
        );
        let observation = IntentObservationCandidate::new(
            "request_diagram",
            "technical_learning",
            vec![EvidenceRef::Turn {
                session_id: "session-a".into(),
                turn_id: "turn-1".into(),
            }],
        )
        .unwrap();
        let blocker = std::env::temp_dir().join("ub-review-result-parent-blocker");
        let _ = std::fs::remove_file(&blocker);
        let _ = std::fs::remove_dir_all(&blocker);
        std::fs::write(&blocker, "not a directory").unwrap();
        store.path = blocker.join("memory.json");

        let error = store
            .commit_review_result(
                &job.job_id,
                &["turn-1".into()],
                &[],
                &[fact],
                &[observation],
                "2026-07-14T00:02:00Z",
            )
            .unwrap_err();

        assert_eq!(error.category, "internal");
        assert_eq!(store.review_state(), &before_state);
        assert!(store.profile_facts().is_empty());
        assert_eq!(store.document_revision(), before_revision);
        assert_eq!(std::fs::read_to_string(path).unwrap(), before_disk);
    }
    #[test]
    fn w3_review_retry_and_restart_preserve_original_workspace_and_goal() {
        let (path, mut store) = store("w3-review-workspace");
        let mut origin = cursor(1);
        origin.learning_workspace_id = Some("a".into());
        origin.turns = vec![ReviewTurnOrigin { turn_id: "turn-1".into(), user_turn_ordinal: 1, book_id: "book-b".into(), goal_revision: Some(2) }];
        store.reconcile_review_jobs(&[origin.clone()], "1").unwrap();
        let original = only_job(&store);
        store.claim_review_job(&original.job_id, "2").unwrap();
        store.mark_review_job_retryable(&original.job_id, "4", ReviewErrorState {
            error_code: "RETRY".into(), message: "retry".into(), occurred_at: "3".into(),
        }, "3").unwrap();
        drop(store);
        let mut store = MemoryStore::open(path).unwrap();
        let mut other = origin.clone(); other.session_id = "session-b".into();
        other.learning_workspace_id = Some("b".into()); other.turns[0].goal_revision = Some(1);
        store.resume_review_jobs(&[origin, other], "4").unwrap();
        let retried = store.claim_review_job(&original.job_id, "5").unwrap();
        assert_eq!(retried.learning_workspace_id.as_deref(), Some("a"));
        assert_eq!(retried.goal_revisions, [2]);
        assert_eq!(retried.attempts, 2);
        let candidate = fact_candidate("session-a", "turn-1", FactSource::UserStated,
            ProfileScope::Book { book_id: "book-b".into() });
        let result = store.commit_review_result(&original.job_id, &["turn-1".into()], &[], &[candidate], &[], "6").unwrap();
        assert_eq!(result.added_fact_ids.len(), 1);
        assert_eq!(store.profile_facts()[0].scope, ProfileScope::Workspace { learning_workspace_id: "a".into(), book_id: "book-b".into() });
        assert!(store.resolve_profile_facts(&crate::ProfileResolutionContext {
            learning_workspace_id: Some("b".into()), book_id: Some("book-b".into()), ..Default::default()
        }).is_empty());
        let mut continuation = cursor(2);
        continuation.learning_workspace_id = Some("a".into());
        continuation.turns = vec![
            ReviewTurnOrigin { turn_id: "turn-1".into(), user_turn_ordinal: 1, book_id: "book-b".into(), goal_revision: Some(2) },
            ReviewTurnOrigin { turn_id: "turn-2".into(), user_turn_ordinal: 2, book_id: "book-c".into(), goal_revision: Some(3) },
        ];
        store.reconcile_review_jobs(&[continuation], "7").unwrap();
        let next = store.review_state().review_jobs.iter().find(|job| job.session_id == "session-a" && job.status == ReviewJobStatus::Queued).unwrap();
        assert_eq!(next.goal_revisions, [3]);
        assert_eq!(next.turn_book_ids, BTreeMap::from([("turn-2".into(), "book-c".into())]));
    }

}
