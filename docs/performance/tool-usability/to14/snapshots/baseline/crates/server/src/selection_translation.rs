//! Ephemeral, billed Reader translation. Preparation never holds a lock over a model call.
use crate::{
    authorization::{Authorization, AuthorizedContext},
    published_library::PublishedBookRef,
    service_limits::{LimitedAdapter, Permit, Resource},
    SelectionTranslationWork,
};
use read_tools::ToolError;
use runtime::{model_spend::ChargeScope, run_context::CancellationToken, ModelAdapter};
use serde_json::Value;
use std::sync::{Arc, Mutex};

pub(crate) struct PreparedTranslation {
    context: AuthorizedContext,
    work: SelectionTranslationWork,
    publication: PublishedBookRef,
    provider: Box<dyn ModelAdapter + Send>,
    _permit: Permit,
}

impl PreparedTranslation {
    pub(crate) fn prepare(
        access: &Arc<Authorization>,
        context: AuthorizedContext,
        id: &str,
        input: &Value,
    ) -> Result<Self, ToolError> {
        if access.runs.is_stopping() {
            return Err(crate::user_storage_paths::error(
                "SERVICE_STOPPING",
                "unavailable",
                "Service is stopping",
            ));
        }
        access.workspace(&context, id)?;
        let (work, publication) = {
            let user = context.user.lock().unwrap();
            access.workspaces.lock().unwrap().prepare_translation(
                &context,
                &user,
                &access.library,
                id,
                input,
            )?
        };
        let provider = access.runs.task_adapter()?;
        let permit = access
            .resources
            .try_acquire(Resource::SyncWait, context.user_id())
            .ok_or_else(|| {
                crate::user_storage_paths::error(
                    "TRANSLATION_CAPACITY",
                    "rate_limit",
                    "Translation capacity reached",
                )
            })?;
        Ok(Self {
            context,
            work,
            publication,
            provider,
            _permit: permit,
        })
    }

    pub(crate) fn execute(
        self,
        access: &Arc<Authorization>,
        cancellation: CancellationToken,
    ) -> Result<Value, ToolError> {
        access
            .auth
            .validate(&self.context.principal, crate::multi_user_host::now())?;
        let scope = ChargeScope::ReaderTask {
            user_id: self.context.user_id().into(),
            task_ref: format!("translation-{}", uuid::Uuid::now_v7()),
        };
        self.provider.set_spend_context(
            scope.clone(),
            Some(
                access
                    .spend
                    .port(scope, self.publication.clone(), cancellation.clone()),
            ),
        );
        self.provider.set_model_purpose("selection_translation");
        self.provider.set_run_cancellation(cancellation.clone());
        let adapter = LimitedAdapter {
            inner: self.provider.as_ref(),
            resources: access.resources.clone(),
            owner: self.context.user_id().into(),
            cancellation,
            usage: Mutex::new(Default::default()),
            stream: None,
            authorization: Some((access, &self.publication)),
        };
        let result = crate::execute_selection_translation_with_adapter(&adapter, &self.work)?;
        access
            .auth
            .validate(&self.context.principal, crate::multi_user_host::now())?;
        access
            .library
            .lock()
            .unwrap()
            .authorize(self.context.user_id(), &self.publication)?;
        Ok(serde_json::json!({"result": result}))
    }
}
