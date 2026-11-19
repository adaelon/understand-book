//! Cloud task ownership, authorization and charge scope for the shared coach.
use crate::*;
use runtime::run_context::CancellationToken;
use std::sync::{Arc, Mutex};
pub use reader_service::teaching_coach::*;
fn error(code: &str, message: &str) -> ToolError {
    crate::user_storage_paths::error(code, "unavailable", message)
}

pub(crate) fn tick_network(access: &Arc<authorization::Authorization>, handle: &user_registry::UserHandle, cancel: &CancellationToken) -> Result<bool, ToolError> {
    let (owner, job) = {
        let user = handle.lock().unwrap();
        let Some(job) = claim(&user)? else { return Ok(false); };
        (user.user_id().expect("network owner").to_owned(), job)
    };
    let publication = serde_json::from_value::<published_library::PublishedBookRef>(job.input.context["publication"].clone())
        .map_err(|_|error("TUTOR_COACH_SOURCE_UNAVAILABLE","教练材料来源已不可用。"))
        .and_then(|reference|access.library.lock().unwrap().load(&owner,&reference));
    let prepared=publication.as_ref().map_err(|e|ToolError {error_code:e.error_code.clone(),category:e.category.clone(),message:e.message.clone()}).and_then(|publication|
        freeze_observation_reads(&handle.lock().unwrap(),&job,&publication.book,false));
    let job=prepared.as_ref().unwrap_or(&job);
    let result = with_task_cancellation(cancel,
        || applicable(&handle.lock().unwrap(),&job).unwrap_or(false), |cancel| {
        let publication = publication.as_ref().map_err(|e| error(&e.error_code,&e.message))?;
        prepared.as_ref().map_err(|e|ToolError {error_code:e.error_code.clone(),category:e.category.clone(),message:e.message.clone()})?;
        let adapter = access.runs.teaching_adapter_for_binding(job.input.context.get("provider_binding"))?;
        let scope = runtime::model_spend::ChargeScope::ReaderTask { user_id:owner.clone(),
            task_ref:format!("{}:{}",job.input.job_ref,job.attempt) };
        adapter.set_spend_context(scope.clone(),Some(access.spend.port_with_attribution(scope,publication.reference.clone(),
            runtime::model_spend::ChargeAttribution {parent_task_ref:job.input.job_ref.clone(),
                learning_workspace_id:job.learning_workspace_id.clone(),related_learning_workspace_ids:vec![]},cancel.clone())));
        let limited = service_limits::LimitedAdapter { inner:adapter.as_ref(),resources:access.resources.clone(),
            owner:owner.clone(),background:true,cancellation:cancel.clone(),usage:Mutex::new(Default::default()),stream:None,
            authorization:Some((access,&publication.reference)) };
        execute_with_material(&limited,job,&publication.book,publication.directory(),cancel)
    });
    save_result(cancel,result,|result|commit_with_material(&handle.lock().unwrap(),job,result,
        publication.as_ref().ok().map(|publication|(publication.book.as_ref(),publication.directory()))))?;
    Ok(true)
}


#[cfg(test)]
mod tests;
