//! Authorized and billed cloud suggestions using the shared guidance work.
use read_tools::ToolError;
use runtime::ModelAdapter;
use serde_json::{json, Value};
pub use reader_service::reading_guidance::*;

// Network readers use the same authorization, allowance and concurrency boundaries
// as the existing one-shot selection translation. Model work runs after locks release.
pub(crate) struct PreparedGuidance {
    context: crate::authorization::AuthorizedContext,
    work: GuidanceWork,
    workspace_id: String,
    publication: crate::published_library::PublishedBookRef,
    provider: Box<dyn ModelAdapter + Send>,
    _permit: crate::service_limits::Permit,
}

impl PreparedGuidance {
    pub(crate) fn prepare(access: &std::sync::Arc<crate::authorization::Authorization>,
        context: crate::authorization::AuthorizedContext, id: &str, action: &str, input: &Value) -> Result<Self, ToolError> {
        if access.runs.is_stopping() {
            return Err(crate::user_storage_paths::error("SERVICE_STOPPING", "unavailable", "Service is stopping"));
        }
        access.workspace(&context, id)?;
        let (work, publication) = {
            let user = context.user.lock().unwrap();
            access.workspaces.lock().unwrap().prepare_guidance(&context, &user, &access.library, id, action, input)?
        };
        let provider = access.runs.teaching_adapter()?;
        let permit = access.resources.try_acquire(crate::service_limits::Resource::SyncWait, context.user_id())
            .ok_or_else(|| crate::user_storage_paths::error("READING_GUIDANCE_CAPACITY", "rate_limit", "Reading guidance capacity reached"))?;
        Ok(Self { context, work, workspace_id: id.into(), publication, provider, _permit: permit })
    }

    pub(crate) fn execute(self, access: &std::sync::Arc<crate::authorization::Authorization>,
        cancellation: runtime::run_context::CancellationToken) -> Result<Value, ToolError> {
        access.auth.validate(&self.context.principal, crate::service_error::now())?;
        let scope = runtime::model_spend::ChargeScope::ReaderTask {
            user_id: self.context.user_id().into(), task_ref: format!("guidance-{}", uuid::Uuid::now_v7()),
        };
        self.provider.set_spend_context(scope.clone(), Some(access.spend.port(scope, self.publication.clone(), cancellation.clone())));
        self.provider.set_run_cancellation(cancellation.clone());
        let adapter = crate::service_limits::LimitedAdapter {
            inner: self.provider.as_ref(), resources: access.resources.clone(), owner: self.context.user_id().into(),
            background: false, cancellation, usage: std::sync::Mutex::new(Default::default()), stream: None,
            authorization: Some((access, &self.publication)),
        };
        let result = self.work.execute(&adapter)?;
        access.auth.validate(&self.context.principal, crate::service_error::now())?;
        access.library.lock().unwrap().authorize(self.context.user_id(), &self.publication)?;
        let result = if self.work.requires_finish() {
            let user = self.context.user.lock().unwrap();
            access.workspaces.lock().unwrap().finish_guidance(&self.context, &user, &access.library, &self.workspace_id, &self.publication, &self.work, result)?
        } else { result };
        Ok(json!({"result":result}))
    }
}

