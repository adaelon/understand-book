pub use reader_service::agent_run::*;
use crate::*;
use std::sync::Arc;
pub(crate) struct NetworkUserPort {
    pub user: crate::user_registry::UserHandle,
    pub access: Arc<crate::authorization::Authorization>,
    pub owner: String,
    pub publication: Option<crate::published_library::PublishedBookRef>,
}
impl UserStatePort for NetworkUserPort {
    fn check_access(&self) -> Result<(), ToolError> {
        self.publication.as_ref().map(|reference|self.access.library.lock().unwrap().authorize(&self.owner,reference).map_err(|_|crate::service_error::missing())).transpose().map(|_|())
    }
    fn with_user<R>(&self, operation: impl FnOnce(&mut crate::user_runtime::UserRuntime) -> R) -> R {
        operation(&mut self.user.lock().unwrap())
    }
}

