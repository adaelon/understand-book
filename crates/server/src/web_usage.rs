//! Per-request Search/Fetch receipts share the existing account allowance.
//! A cache hit never enters this ledger; each physical retry gets its own row.
use crate::{
    admin_store::invalid,
    control_store::ControlStore,
    model_spend_store::{api_storage, ModelSpendStore},
    user_storage_paths::error,
};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WebServiceKind {
    #[serde(rename = "web_search")]
    Search,
    #[serde(rename = "web_fetch")]
    Fetch,
}
impl WebServiceKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Search => "web_search",
            Self::Fetch => "web_fetch",
        }
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WebChargeOutcome {
    Billed,
    Free,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WebRate {
    pub version: String,
    pub provider: String,
    pub service_kind: WebServiceKind,
    pub starts_at: i64,
    pub expires_at: i64,
    pub cost_micro_cny_per_request: i64,
    pub source: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rates {
    rates: Vec<WebRate>,
}
fn conflict(code: &str, message: &str) -> ToolError {
    error(code, "conflict", message)
}
fn unavailable() -> ToolError {
    conflict(
        "WEB_RATE_UNAVAILABLE",
        "联网服务尚未配置有效费率，暂不能启动付费请求",
    )
}
pub(crate) fn rate(root: &Path, kind: WebServiceKind, now: i64) -> Result<WebRate, ToolError> {
    let bytes = std::fs::read(root.join("web-rates.json")).map_err(|_| unavailable())?;
    let rates: Rates = serde_json::from_slice(&bytes).map_err(|_| unavailable())?;
    for (i, r) in rates.rates.iter().enumerate() {
        if r.version.trim().is_empty()
            || r.provider != "deepseek"
            || r.source.trim().is_empty()
            || r.starts_at >= r.expires_at
            || r.cost_micro_cny_per_request < 0
            || rates.rates[..i].iter().any(|p| {
                p.service_kind == r.service_kind
                    && (p.version == r.version
                        || (p.starts_at < r.expires_at && r.starts_at < p.expires_at))
            })
        {
            return Err(unavailable());
        }
    }
    rates
        .rates
        .into_iter()
        .find(|r| r.service_kind == kind && r.starts_at <= now && now < r.expires_at)
        .ok_or_else(unavailable)
}

pub(crate) fn charge(db: &Connection, id: &str) -> Result<Option<Value>, ToolError> {
    let value = db.query_row("SELECT usage_id,logical_usage_id,attempt,run_ref,user_id,period_id,learning_workspace_id,service_kind,rate_snapshot,state,reserved_micro_cny,account_debit_micro_cny,provider_cost_micro_cny,provider_cost_status,evidence_json,created_at,settled_at,revision,needs_reconciliation FROM web_usage WHERE usage_id=?",[id],|r| Ok(json!({
        "call_id":r.get::<_,String>(0)?,"usage_id":r.get::<_,String>(0)?,"logical_usage_id":r.get::<_,String>(1)?,"attempt":r.get::<_,i64>(2)?,
        "run_ref":r.get::<_,String>(3)?,"parent_task_ref":r.get::<_,String>(3)?,"task_ref":Value::Null,
        "cost_owner":"account","user_id":r.get::<_,String>(4)?,"period_id":r.get::<_,String>(5)?,"learning_workspace_id":r.get::<_,Option<String>>(6)?,
        "related_learning_workspace_ids":[],"entry_kind":r.get::<_,String>(7)?,"service_kind":r.get::<_,String>(7)?,"purpose":r.get::<_,String>(7)?,"model":Value::Null,
        "request_count":1,"rate_snapshot":r.get::<_,String>(8)?,"state":r.get::<_,String>(9)?,"reserved_micro_cny":r.get::<_,i64>(10)?,
        "account_debit_micro_cny":r.get::<_,Option<i64>>(11)?,"provider_cost_micro_cny":r.get::<_,Option<i64>>(12)?,"provider_cost_status":r.get::<_,String>(13)?,
        "evidence":r.get::<_,Option<String>>(14)?,"created_at":r.get::<_,i64>(15)?,"settled_at":r.get::<_,Option<i64>>(16)?,"revision":r.get::<_,i64>(17)?,"needs_reconciliation":r.get::<_,bool>(18)?
    }))).optional().map_err(api_storage)?;
    value
        .map(|mut v| {
            for key in ["rate_snapshot", "evidence"] {
                if let Some(raw) = v[key].as_str() {
                    v[key] = serde_json::from_str(raw).map_err(api_storage)?;
                }
            }
            crate::account_allowance::project_charge(db, &mut v)?;
            v["provider"] = v["rate_snapshot"]["provider"].clone();
            v["platform_covered_micro_cny"] = v["provider_cost_micro_cny"]
                .as_i64()
                .zip(v["account_debit_micro_cny"].as_i64())
                .map(|(cost, debit)| json!(cost.saturating_sub(debit).max(0)))
                .unwrap_or(Value::Null);
            Ok(v)
        })
        .transpose()
}

impl ModelSpendStore {
    /// Call after the queue wait, immediately before an actual HTTP send.
    pub(crate) fn web_admit_run(
        &self,
        scope: &crate::run_scope::RunScope,
        kind: WebServiceKind,
        logical_usage_id: &str,
        attempt: u32,
        now: i64,
    ) -> Result<String, ToolError> {
        let mut c = self.control.lock().unwrap();
        admit(
            &mut c,
            &scope.user_id,
            &scope.turn_id,
            scope.learning_workspace_id.as_deref(),
            kind,
            logical_usage_id,
            attempt,
            now,
        )
    }
    pub(crate) fn web_receipt(
        &self,
        run_ref: &str,
        usage_id: &str,
        outcome: WebChargeOutcome,
        evidence: Value,
        now: i64,
    ) -> Result<Value, ToolError> {
        let mut c = self.control.lock().unwrap();
        let tx = c
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(api_storage)?;
        let value = settle(&tx, run_ref, usage_id, outcome, evidence, now)?;
        tx.commit().map_err(api_storage)?;
        Ok(value)
    }
}

fn admit(
    c: &mut ControlStore,
    owner: &str,
    run: &str,
    workspace: Option<&str>,
    kind: WebServiceKind,
    logical: &str,
    attempt: u32,
    now: i64,
) -> Result<String, ToolError> {
    if owner.trim().is_empty() || run.trim().is_empty() || logical.trim().is_empty() || attempt == 0
    {
        return Err(invalid());
    }
    let rate = rate(c.writer.root(), kind, now)?;
    let reserved = rate.cost_micro_cny_per_request;
    let tx = c
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(api_storage)?;
    let active: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE user_id=? AND disabled=0)",
            [owner],
            |r| r.get(0),
        )
        .map_err(api_storage)?;
    if !active {
        return Err(crate::authorization::missing());
    }
    if let Some(workspace) = workspace {
        let owned:bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM learning_workspaces WHERE owner_user_id=? AND learning_workspace_id=? AND archived=0)",params![owner,workspace],|r|r.get(0)).map_err(api_storage)?;
        if !owned {
            return Err(crate::authorization::missing());
        }
    }
    let previous:Option<(i64,String,String)> = tx.query_row("SELECT attempt,state,service_kind FROM web_usage WHERE run_ref=? AND logical_usage_id=? ORDER BY attempt DESC LIMIT 1",params![run,logical],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(api_storage)?;
    if let Some((prior, state, prior_kind)) = previous {
        if i64::from(attempt) != prior + 1
            || state == "sent"
            || state == "settled"
            || prior_kind != kind.as_str()
        {
            return Err(conflict(
                "WEB_ALREADY_ADMITTED",
                "该请求尝试已登记或已完成，不能重复发送",
            ));
        }
    } else if attempt != 1 {
        return Err(invalid());
    }
    let period:Option<String> = tx.query_row("SELECT period_id FROM allowance_periods WHERE user_id=? AND closed_at IS NULL AND starts_at<=? AND ?<expires_at",params![owner,now,now],|r|r.get(0)).optional().map_err(api_storage)?;
    let period = period.ok_or_else(|| conflict("ALLOWANCE_EXPIRED", "当前没有有效服务额度期"))?;
    if !crate::account_allowance::balance(&tx, &period)?.can_reserve(reserved) {
        return Err(conflict(
            "ALLOWANCE_INSUFFICIENT",
            "当前服务额度不足以开始联网请求",
        ));
    }
    let id = format!("web_{}", uuid::Uuid::now_v7());
    tx.execute("INSERT INTO web_usage(usage_id,logical_usage_id,attempt,run_ref,user_id,period_id,learning_workspace_id,service_kind,rate_snapshot,state,reserved_micro_cny,provider_cost_status,created_at) VALUES(?,?,?,?,?,?,?,?,?,'sent',?,'pending',?)",params![id,logical,attempt,run,owner,period,workspace,kind.as_str(),json!(rate).to_string(),reserved,now]).map_err(api_storage)?;
    tx.commit().map_err(api_storage)?;
    Ok(id)
}

fn settle(
    db: &Connection,
    run: &str,
    id: &str,
    outcome: WebChargeOutcome,
    evidence: Value,
    now: i64,
) -> Result<Value, ToolError> {
    if !evidence.as_object().is_some_and(|v| !v.is_empty()) {
        return Err(invalid());
    }
    let before = charge(db, id)?
        .filter(|v| v["run_ref"] == run)
        .ok_or_else(crate::authorization::missing)?;
    let report = json!({"outcome":outcome,"evidence":evidence}).to_string();
    let duplicate: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM web_usage_receipts WHERE usage_id=? AND report_json=?)",
            params![id, report],
            |r| r.get(0),
        )
        .map_err(api_storage)?;
    if duplicate {
        return Ok(json!({"charge":before}));
    }
    if matches!(before["state"].as_str(), Some("settled" | "released")) {
        return Err(conflict(
            "WEB_RECEIPT_CONFLICT",
            "已结算的请求不能被不同回执改写",
        ));
    }
    let rate: WebRate =
        serde_json::from_value(before["rate_snapshot"].clone()).map_err(api_storage)?;
    let (state, cost) = match outcome {
        WebChargeOutcome::Billed => ("settled", Some(rate.cost_micro_cny_per_request)),
        WebChargeOutcome::Free => ("released", Some(0)),
        WebChargeOutcome::Unknown => ("pending", None),
    };
    db.execute("UPDATE web_usage SET state=?,account_debit_micro_cny=?,provider_cost_micro_cny=?,provider_cost_status=?,evidence_json=?,settled_at=?,revision=revision+1 WHERE usage_id=?",params![state,cost,cost,if cost.is_some(){"confirmed"}else{"pending"},evidence.to_string(),cost.map(|_|now),id]).map_err(api_storage)?;
    db.execute(
        "INSERT INTO web_usage_receipts VALUES(?,?,?)",
        params![id, report, now],
    )
    .map_err(api_storage)?;
    Ok(json!({"charge":charge(db,id)?}))
}

#[cfg(test)]
#[path = "web_usage_tests.rs"]
mod tests;
