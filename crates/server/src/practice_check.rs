//! Cloud authorization and accounting for shared background work.
use crate::*;
use runtime::run_context::CancellationToken;
use std::sync::{Arc, Mutex};
pub use reader_service::practice_check::*;

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
    let adapter = access.runs.teaching_adapter()?;
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
