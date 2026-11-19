//! Cloud scene and capacity guard for local dictation.
use read_tools::ToolError;
use runtime::run_context::CancellationToken;
use serde_json::{json, Value};
use std::sync::Arc;
pub use reader_service::voice_input::*;
fn error(code: &str, category: &str, message: impl Into<String>) -> ToolError {
    ToolError { error_code:code.into(), category:category.into(), message:message.into() }
}

pub(crate) struct PreparedVoice {
    context: crate::authorization::AuthorizedContext,
    work: Work,
    _permit: crate::service_limits::Permit,
}
impl PreparedVoice {
    pub(crate) fn prepare(access: &Arc<crate::authorization::Authorization>, context: crate::authorization::AuthorizedContext,
        workspace: &str, input: &Value) -> Result<Self,ToolError> {
        access.workspace(&context,workspace)?;
        {
            let user = context.user.lock().unwrap();
            access.workspaces.lock().unwrap().check_voice_scene(&context,&user,&access.library,workspace,input)?;
        }
        let permit = access.resources.try_acquire(crate::service_limits::Resource::SyncWait,context.user_id())
            .ok_or_else(|| error("VOICE_BUSY","rate_limit","语音服务繁忙，请稍后重试。"))?;
        Ok(Self { work:Work::prepare(input)?, context, _permit:permit })
    }
    pub(crate) fn execute(self, access: &Arc<crate::authorization::Authorization>, cancel: CancellationToken) -> Result<Value,ToolError> {
        access.auth.validate(&self.context.principal,crate::service_error::now())?;
        let result = self.work.execute(&cancel)?;
        access.auth.validate(&self.context.principal,crate::service_error::now())?;
        Ok(json!({"result":result}))
    }
}

