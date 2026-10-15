//! Background profile extraction for the multi-user service. Jobs remain in MemoryStore.
use crate::{
    authorization::Authorization,
    host::{copy_user_review_input, mark_review_retryable, ReviewMoment},
    service_limits::LimitedAdapter,
    user_registry::UserHandle,
    AgentAssistantStatus,
};
use memory::ReviewJobStatus;
use read_tools::ToolError;
use runtime::{model_spend::ChargeScope, run_context::CancellationToken};
use std::sync::{Arc, Mutex};

/// Keep newly queued work discoverable after its resident user is evicted.
pub(crate) fn remember_pending(
    access: &Arc<Authorization>,
    backlog: &mut std::collections::VecDeque<String>,
) {
    let users = access.users.lock().unwrap().loaded_users();
    for handle in users {
        let user = handle.lock().unwrap();
        if user
            .store
            .review_state()
            .review_jobs
            .iter()
            .any(|job| job.status != ReviewJobStatus::Completed)
        {
            let owner = user.user_id().expect("service user has owner");
            if !backlog.iter().any(|queued| queued == owner) {
                backlog.push_back(owner.to_string());
            }
        }
    }
}

// Network history timestamps are seconds; the original memory scheduler uses milliseconds.
fn millis(value: &str) -> u64 {
    value
        .parse::<u64>()
        .map(|n| {
            if n < 100_000_000_000 {
                n.saturating_mul(1000)
            } else {
                n
            }
        })
        .unwrap_or(0)
}

pub(crate) fn tick(access: &Arc<Authorization>, cancel: &CancellationToken, now_ms: u64) -> usize {
    let users = access.users.lock().unwrap().loaded_users();
    let mut completed = 0;
    for user in users {
        if cancel.is_cancelled() || access.runs.is_stopping() {
            break;
        }
        match run_one(access, &user, cancel, now_ms) {
            Ok(true) => completed += 1,
            Ok(false) => {}
            Err(error) => eprintln!("Memory review: {}", error.error_code),
        }
    }
    completed
}

fn run_one(
    access: &Arc<Authorization>,
    handle: &UserHandle,
    cancel: &CancellationToken,
    now_ms: u64,
) -> Result<bool, ToolError> {
    let now = now_ms.to_string();
    let (owner, job, publication) = {
        let mut user = handle.lock().unwrap();
        if user.agent_history.sessions.iter().any(|s| {
            s.turns
                .iter()
                .any(|t| t.status == AgentAssistantStatus::PendingAssistant)
        }) {
            return Ok(false);
        }
        let latest = user
            .agent_history
            .sessions
            .iter()
            .map(|s| millis(&s.updated_at))
            .max()
            .unwrap_or(0);
        let Some(job) = user
            .store
            .review_state()
            .review_jobs
            .iter()
            .find(|job| match job.status {
                ReviewJobStatus::Queued => {
                    job.to_turn_inclusive
                        .saturating_sub(job.from_turn_exclusive)
                        >= 8
                        || now_ms.saturating_sub(latest.max(millis(&job.updated_at))) >= 60_000
                }
                // Retry deadlines are always written in milliseconds by both schedulers.
                ReviewJobStatus::Retryable => job
                    .next_attempt_at
                    .as_deref()
                    .and_then(|s| s.parse::<u64>().ok())
                    .is_none_or(|at| at <= now_ms),
                _ => false,
            })
            .cloned()
        else {
            return Ok(false);
        };
        let publication = user
            .agent_history
            .sessions
            .iter()
            .find(|s| s.id == job.session_id)
            .and_then(|s| {
                s.turns
                    .iter()
                    .rev()
                    .find(|t| t.user_turn_ordinal <= job.to_turn_inclusive)
            })
            .and_then(|t| t.published_book_ref.clone());
        let owner = user.user_id().expect("service user has owner").to_string();
        let claimed = user.store.claim_review_job(&job.job_id, &now)?;
        (owner, claimed, publication)
    };
    let result = (|| {
        let publication = match publication {
            Some(reference) => reference,
            None => access.library.lock().unwrap().default_ref(&job.book_id)?,
        };
        let book = access.library.lock().unwrap().load(&owner, &publication)?;
        let input = copy_user_review_input(
            &handle.lock().unwrap(),
            &job,
            crate::current_content_profile(&book.book),
        )?;
        let adapter = access.runs.task_adapter().map_err(|mut e| {
            e.error_code = "REVIEW_PROVIDER_UNCONFIGURED".into();
            e
        })?;
        let scope = ChargeScope::ReaderTask {
            user_id: owner.clone(),
            task_ref: format!("memory-review:{}:{}", job.job_id, job.attempts),
        };
        adapter.set_spend_context(
            scope.clone(),
            Some(
                access
                    .spend
                    .port(scope, publication.clone(), cancel.clone()),
            ),
        );
        adapter.set_model_purpose("memory_review");
        adapter.set_run_cancellation(cancel.clone());
        let limited = LimitedAdapter {
            inner: adapter.as_ref(),
            resources: access.resources.clone(),
            owner: owner.clone(),
            cancellation: cancel.clone(),
            usage: Mutex::new(Default::default()),
            stream: None,
            authorization: Some((access, &publication)),
        };
        let output = runtime::memory_review::execute_review(&limited, &input).map_err(|e| {
            e.spend_stop.map(|s| s.tool_error()).unwrap_or_else(|| {
                crate::user_storage_paths::error("REVIEW_EXECUTOR_FAILED", "provider", &e.message)
            })
        })?;
        let turns: Vec<_> = input.turns.iter().map(|t| t.turn_id.clone()).collect();
        let mut user = handle.lock().unwrap();
        user.check_owner(&owner)?;
        user.store.commit_review_result(
            &job.job_id,
            &turns,
            &output.fact_candidates,
            &output.intent_observations,
            &now,
        )?;
        Ok(true)
    })();
    if let Err(error) = &result {
        mark_review_retryable(
            &mut handle.lock().unwrap().store,
            &job,
            error,
            &ReviewMoment::from_millis(now_ms),
        )?;
    }
    result
}
