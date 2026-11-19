//! After-delivery soundness check for practice problems (ADR-0178 §3). The
//! queue is the teaching trace itself: a delivered practice problem without a
//! final verdict is pending. Foreground teaching never waits for this work.
use crate::*;
use memory::teaching::{TeachingEvent, TeachingFact};
use runtime::run_context::{CancellableAdapter, CancellationToken};
use std::sync::{Arc, Mutex};

/// Failed attempts are recorded; after this many the problem keeps counting as unchecked.
const MAX_FAILURES: u32 = 3;

fn now() -> String { crate::service_error::now().to_string() }

pub struct PendingCheck {
    pub workspace: Option<String>,
    pub delivery: TeachingEvent,
    pub(crate) packet: Value,
}

/// Workspace scopes that can hold this user's teaching facts.
pub(crate) fn scopes(user: &user_runtime::UserRuntime) -> Vec<Option<String>> {
    let mut scopes = vec![None];
    for session in &user.agent_history.sessions {
        if !scopes.contains(&session.learning_workspace_id) { scopes.push(session.learning_workspace_id.clone()); }
    }
    scopes
}

/// Only what the problem was posed with: no learner answer, no later facts.
fn packet(selected: &TeachingEvent) -> Value {
    let movement = &selected.payload["move"];
    json!({"problem":{"prompt":movement["prompt"],"delivery_text":movement["delivery_text"],"answer_input":movement["answer_input"]},
        "practiced_relationship":{"learning_focus":movement["target"]["learning_focus"],"expected_performance":movement["target"]["expected_performance"]},
        "mode":movement["practice"]["mode"],"committed_solution":movement["explanation"],"answer_contract":movement["assessment"],
        "hints":movement["practice"]["hints"],"frozen_sources":selected.payload["scoring_sources"],
        "source_references":selected.payload["scoring_source_refs"]})
}

pub fn next(user: &user_runtime::UserRuntime) -> Result<Option<PendingCheck>, ToolError> {
    if !user.store.private_storage_available() { return Ok(None); }
    for workspace in scopes(user) {
        let store = user.learning_store_for_workspace(workspace.as_deref())?;
        let Some(delivery) = store.unchecked_practice_deliveries(MAX_FAILURES, 1)?.into_iter().next() else { continue; };
        let selected = store.teaching_event(delivery.causal_refs.first().map(String::as_str).unwrap_or_default())?;
        let mut packet = packet(&selected);
        if let Some(case) = selected.payload["move"]["practice"]["application_situation"].as_str() {
            let binding = store.teaching_event(&format!("binding:{}", selected.binding.turn_id))?;
            if binding.payload["application_requested"] == true {
                let action = &binding.payload["current_action"];
                packet["application"] = json!({"new_case":case,"previous_answer":action["answer_text"],
                    "source_excerpt":action["source_excerpt"],
                    "visible_lesson_material":crate::course_context::application_material(&binding.payload["inquiry_course"]["lesson"]).collect::<Vec<_>>()});
            }
        }
        return Ok(Some(PendingCheck { workspace, packet, delivery }));
    }
    Ok(None)
}

/// Persist a verdict, or one bounded failure. A questionable problem leaves the
/// learner projection at once; its original facts are never rewritten.
pub fn record(user: &user_runtime::UserRuntime, pending: &PendingCheck,
    result: Result<runtime::practice::PracticeCheck, String>) -> Result<(), ToolError> {
    let delivery = &pending.delivery;
    let mut store = user.learning_store_for_workspace(pending.workspace.as_deref())?;
    let (event_id, payload) = match &result {
        Ok(check) => (format!("practice-check:{}", delivery.event_id),
            json!({"delivery_ref":delivery.event_id,"verdict":check.verdict,"issues":check.issues,"reason":check.reason})),
        Err(message) => (format!("practice-check:{}:failed:{}", delivery.event_id, store.practice_check_failures(&delivery.event_id)? + 1),
            json!({"delivery_ref":delivery.event_id,"verdict":"failed","reason":message})),
    };
    store.append_teaching(&TeachingEvent { event_id, binding: delivery.binding.clone(), kind: TeachingFact::PracticeChecked,
        causal_refs: vec![delivery.event_id.clone()], payload, occurred_at: now() })?;
    if result.as_ref().is_ok_and(|check| check.verdict == runtime::practice::CheckVerdict::Questionable) {
        store.rebuild_learner_projection(&delivery.binding.source_id)?;
    }
    Ok(())
}

pub fn run(adapter: &dyn ModelAdapter, pending: &PendingCheck, cancel: &CancellationToken) -> Option<Result<runtime::practice::PracticeCheck, String>> {
    adapter.set_run_cancellation(cancel.clone());
    let adapter = CancellableAdapter { inner: adapter, cancellation: cancel.clone() };
    let result = runtime::practice::check(&adapter, &pending.packet).map_err(|e| e.message);
    // An interrupted host leaves the problem pending instead of spending a failure.
    (!cancel.is_cancelled()).then_some(result)
}

pub(crate) fn tick_local(state: &Arc<Mutex<AppState>>, provider: Option<ProviderConfig>, cancel: &CancellationToken) -> Result<bool, ToolError> {
    let Some(config) = provider else { return Ok(false); };
    let Some(pending) = next(&state.lock().unwrap().user)? else { return Ok(false); };
    let adapter = ProviderRegistry::adapter_from_config_with_timeout(config, crate::SELECTION_TRANSLATION_TIMEOUT);
    let Some(result) = run(adapter.as_ref(), &pending, cancel) else { return Ok(false); };
    record(&state.lock().unwrap().user, &pending, result)?;
    Ok(true)
}

