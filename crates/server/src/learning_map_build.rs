//! Cloud authorization and accounting for shared background work.
use crate::*;
use runtime::run_context::CancellationToken;
use std::sync::{Arc, Mutex};
use memory::teaching::TeachingEvent;
pub use reader_service::learning_map_build::*;

pub(crate) fn tick_network(access: &Arc<authorization::Authorization>, handle: &user_registry::UserHandle, cancel: &CancellationToken) -> Result<bool, ToolError> {
    let (owner, pending, publication) = {
        let user = handle.lock().unwrap();
        let owner = user.user_id().expect("network owner").to_owned();
        let publication_of = |delivery: &TeachingEvent| user.agent_history.sessions.iter()
            .find(|s| s.id == delivery.binding.chat_session_id)
            .and_then(|s| s.turns.iter().find(|t| t.turn_id == delivery.binding.turn_id))
            .and_then(|t| t.published_book_ref.clone());
        let open = |delivery: &TeachingEvent| match publication_of(delivery) {
            None => Opened::Gone,
            // A load can fail transiently (budget, I/O); retry on a later tick.
            Some(reference) => match access.library.lock().unwrap().load(&owner, &reference) {
                Ok(published) => Opened::Book(published.book.clone(), Some(published.reference.publication_id.clone())),
                Err(_) => Opened::Elsewhere,
            },
        };
        let Some(pending) = next(&user, &open)? else { return Ok(false); };
        let publication = publication_of(&pending.delivery);
        (owner, pending, publication)
    };
    let task_ref = format!("learning-map:{}", pending.delivery.event_id);
    let adapter = access.runs.task_adapter()?;
    let scope = runtime::model_spend::ChargeScope::ReaderTask { user_id: owner.clone(), task_ref: task_ref.clone() };
    adapter.set_spend_context(scope.clone(), Some(access.spend.port_with_optional_publication(scope, publication.clone(),
        runtime::model_spend::ChargeAttribution { parent_task_ref: task_ref, learning_workspace_id: pending.workspace.clone(),
            related_learning_workspace_ids: vec![] }, cancel.clone())));
    let limited = service_limits::LimitedAdapter { inner: adapter.as_ref(), resources: access.resources.clone(),
        owner: owner.clone(), background: true, cancellation: cancel.clone(), usage: Mutex::new(Default::default()), stream: None,
        authorization: publication.as_ref().map(|reference| (access.as_ref(), reference)) };
    let Some(result) = run(&limited, &pending, cancel) else { return Ok(false); };
    record(&handle.lock().unwrap(), &pending, result)?;
    Ok(true)
}
