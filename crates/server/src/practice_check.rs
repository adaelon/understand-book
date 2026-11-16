//! After-delivery soundness check for practice problems (ADR-0178 §3). The
//! queue is the teaching trace itself: a delivered practice problem without a
//! final verdict is pending. Foreground teaching never waits for this work.
use crate::*;
use memory::teaching::{TeachingEvent, TeachingFact};
use runtime::run_context::{CancellableAdapter, CancellationToken};
use std::sync::{Arc, Mutex};

/// Failed attempts are recorded; after this many the problem keeps counting as unchecked.
const MAX_FAILURES: u32 = 3;

fn now() -> String { crate::multi_user_host::now().to_string() }

pub(crate) struct PendingCheck {
    pub(crate) workspace: Option<String>,
    pub(crate) delivery: TeachingEvent,
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
        "hints":movement["practice"]["hints"],"frozen_sources":selected.payload["scoring_sources"]})
}

pub(crate) fn next(user: &user_runtime::UserRuntime) -> Result<Option<PendingCheck>, ToolError> {
    if !user.store.private_storage_available() { return Ok(None); }
    for workspace in scopes(user) {
        let store = user.learning_store_for_workspace(workspace.as_deref())?;
        let Some(delivery) = store.unchecked_practice_deliveries(MAX_FAILURES, 1)?.into_iter().next() else { continue; };
        let selected = store.teaching_event(delivery.causal_refs.first().map(String::as_str).unwrap_or_default())?;
        return Ok(Some(PendingCheck { workspace, packet: packet(&selected), delivery }));
    }
    Ok(None)
}

/// Persist a verdict, or one bounded failure. A questionable problem leaves the
/// learner projection at once; its original facts are never rewritten.
pub(crate) fn record(user: &user_runtime::UserRuntime, pending: &PendingCheck,
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

fn run(adapter: &dyn ModelAdapter, pending: &PendingCheck, cancel: &CancellationToken) -> Option<Result<runtime::practice::PracticeCheck, String>> {
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

pub(crate) fn tick_network(access: &Arc<authorization::Authorization>, handle: &user_registry::UserHandle, cancel: &CancellationToken) -> Result<bool, ToolError> {
    let (owner, pending, publication) = {
        let user = handle.lock().unwrap();
        let Some(pending) = next(&user)? else { return Ok(false); };
        let binding = &pending.delivery.binding;
        let publication = user.agent_history.sessions.iter().find(|s| s.id == binding.chat_session_id)
            .and_then(|s| s.turns.iter().find(|t| t.turn_id == binding.turn_id))
            .and_then(|t| t.published_book_ref.clone());
        (user.user_id().expect("network owner").to_owned(), pending, publication)
    };
    let adapter = access.runs.task_adapter()?;
    let scope = runtime::model_spend::ChargeScope::ReaderTask { user_id: owner.clone(),
        task_ref: format!("practice-check:{}", pending.delivery.event_id) };
    adapter.set_spend_context(scope.clone(), Some(access.spend.port_with_optional_publication(scope, publication.clone(),
        runtime::model_spend::ChargeAttribution { parent_task_ref: format!("practice-check:{}", pending.delivery.event_id),
            learning_workspace_id: pending.workspace.clone(), related_learning_workspace_ids: vec![] }, cancel.clone())));
    let limited = service_limits::LimitedAdapter { inner: adapter.as_ref(), resources: access.resources.clone(),
        owner: owner.clone(), background: true, cancellation: cancel.clone(), usage: Mutex::new(Default::default()), stream: None,
        authorization: publication.as_ref().map(|reference| (access.as_ref(), reference)) };
    let Some(result) = run(&limited, &pending, cancel) else { return Ok(false); };
    record(&handle.lock().unwrap(), &pending, result)?;
    Ok(true)
}
