//! Per-transport cost boundary. No database or account policy lives in Runtime.
use crate::provider_stream::ModelUsage;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChargeScope {
    ReaderRun { user_id: String, run_ref: String },
    ReaderTask { user_id: String, task_ref: String },
    OperatorTask { task_ref: String },
}
impl ChargeScope {
    pub fn valid(&self) -> bool {
        match self {
            Self::ReaderRun { user_id, run_ref } => !user_id.is_empty() && !run_ref.is_empty(),
            Self::ReaderTask { user_id, task_ref } => !user_id.is_empty() && !task_ref.is_empty(),
            Self::OperatorTask { task_ref } => !task_ref.is_empty(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendIdentity {
    pub call_id: String,
    pub logical_call_id: String,
    pub attempt: u32,
    pub scope: Option<ChargeScope>,
    pub purpose: String,
    pub model: String,
    pub provider: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpendStop {
    MissingScope,
    RateUnavailable,
    InsufficientAllowance,
    AllowanceExpired,
    StorageUnavailable,
    PermissionRevoked,
    ReconciliationRequired,
}
impl SpendStop {
    /// Decode the structured error-code boundary used by tools and persisted turns.
    /// Provider/user prose is never inspected for stop reasons.
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "MODEL_SPEND_SCOPE_MISSING" => Self::MissingScope,
            "MODEL_RATE_UNAVAILABLE" => Self::RateUnavailable,
            "ALLOWANCE_INSUFFICIENT" => Self::InsufficientAllowance,
            "ALLOWANCE_EXPIRED" => Self::AllowanceExpired,
            "MODEL_SPEND_STORAGE_UNAVAILABLE" => Self::StorageUnavailable,
            "RUN_PERMISSION_REVOKED" => Self::PermissionRevoked,
            "MODEL_CHARGE_RECONCILIATION_REQUIRED" => Self::ReconciliationRequired,
            _ => return None,
        })
    }
    pub fn message(self) -> &'static str {
        match self {
            Self::MissingScope => "本次模型调用缺少费用归属，已停止后续调用。",
            Self::RateUnavailable => "模型计价配置暂不可用，已停止后续调用。",
            Self::InsufficientAllowance => {
                "本期额度不足，已停止后续模型调用。补充额度后可显式继续。"
            }
            Self::AllowanceExpired => {
                "当前没有有效额度期，已停止后续模型调用。开通或续期后可显式继续。"
            }
            Self::StorageUnavailable => "费用记录暂时无法保存，已停止后续模型调用。",
            Self::PermissionRevoked => "账号或材料权限已失效，已停止后续模型调用。",
            Self::ReconciliationRequired => "本次模型费用需要核对，已停止后续模型调用。",
        }
    }
    pub fn tool_error(self) -> read_tools::ToolError {
        read_tools::ToolError {
            error_code: self.code().into(),
            category: "model_spend".into(),
            message: self.message().into(),
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::MissingScope => "MODEL_SPEND_SCOPE_MISSING",
            Self::RateUnavailable => "MODEL_RATE_UNAVAILABLE",
            Self::InsufficientAllowance => "ALLOWANCE_INSUFFICIENT",
            Self::AllowanceExpired => "ALLOWANCE_EXPIRED",
            Self::StorageUnavailable => "MODEL_SPEND_STORAGE_UNAVAILABLE",
            Self::PermissionRevoked => "RUN_PERMISSION_REVOKED",
            Self::ReconciliationRequired => "MODEL_CHARGE_RECONCILIATION_REQUIRED",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SendEvidence {
    Response,
    OutcomeUnknown,
    NotSent,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendOutcome {
    pub evidence: SendEvidence,
    pub usage: Option<ModelUsage>,
    pub succeeded: bool,
}
/// before_send must commit both authorization and the sent marker before returning.
/// after_send runs even on response parsing/cancellation errors. Failure stops retries.
/// Implementations must not retain the request body: it contains private text/media.
pub trait ModelSpendPort: Send + Sync {
    fn before_send(&self, identity: &SendIdentity, final_request: &Value) -> Result<(), SpendStop>;
    fn after_send(&self, identity: &SendIdentity, outcome: &SendOutcome) -> Result<(), SpendStop>;
}

#[cfg(test)]
#[path = "model_spend_tests.rs"]
mod tests;
