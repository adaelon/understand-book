//! Trusted external executors share the Reader ledger. A frozen parent ceiling
//! limits concurrent admissions and the eventual account debit independently.
use crate::{admin_store::invalid, model_spend_store::{api_storage, charge, ModelSpendStore}, user_storage_paths::error};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Budget {
    pub cost_owner: String,
    pub user_id: Option<String>,
    pub learning_workspace_id: Option<String>,
    pub max_micro_cny: i64,
    pub confirmation: String,
    pub expected_revision: Option<i64>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Admission {
    pub execution_id: String,
    pub call_id: String,
    pub logical_call_id: String,
    pub attempt: u32,
    pub granularity: String,
    pub reserved_micro_cny: i64,
    pub provider: String,
    pub model: String,
    pub purpose: String,
    pub budget_enforced: bool,
    pub receipt_capability: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Receipt {
    pub call_id: String,
    pub status: String,
    pub provider_cost_micro_cny: Option<i64>,
    pub usage: Option<Value>,
    pub evidence: Value,
    pub succeeded: bool,
}

fn nonempty(value: &str) -> bool { !value.trim().is_empty() }
fn conflict(code: &str, message: &str) -> ToolError { error(code, "conflict", message) }

pub(crate) fn summary(db: &Connection, task: &str) -> Result<Value, ToolError> {
    let mut value = db.query_row("SELECT cost_owner,user_id,learning_workspace_id,max_micro_cny,confirmation,revision FROM build_task_budgets WHERE task_ref=?", [task], |row| Ok(json!({
        "task_ref":task,"cost_owner":row.get::<_,String>(0)?,"user_id":row.get::<_,Option<String>>(1)?,
        "learning_workspace_id":row.get::<_,Option<String>>(2)?,"max_micro_cny":row.get::<_,i64>(3)?,"confirmation":row.get::<_,String>(4)?,"revision":row.get::<_,i64>(5)?
    }))).optional().map_err(api_storage)?.ok_or_else(crate::authorization::missing)?;
    let (debit, active, pending, cost, unknown):(i64,i64,i64,i64,i64) = db.query_row(
        "SELECT COALESCE(sum(c.account_debit_micro_cny),0),COALESCE(sum(CASE WHEN c.state IN ('reserved','sent') THEN c.reserved_micro_cny ELSE 0 END),0),COALESCE(sum(CASE WHEN c.state='pending' THEN c.reserved_micro_cny ELSE 0 END),0),COALESCE(sum(c.provider_cost_micro_cny),0),COALESCE(sum(CASE WHEN c.provider_cost_status='pending' THEN 1 ELSE 0 END),0) FROM model_call_charges c JOIN external_build_calls x USING(call_id) JOIN external_build_executions e USING(execution_id) WHERE e.task_ref=?",[task],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(api_storage)?;
    let spent = if value["cost_owner"] == "platform" { cost } else { debit };
    let remaining = value["max_micro_cny"].as_i64().unwrap().saturating_sub(spent).saturating_sub(active).saturating_sub(pending).max(0);
    value["debited_micro_cny"] = json!(debit);
    value["reserved_micro_cny"] = json!(active);
    value["pending_micro_cny"] = json!(pending);
    value["provider_cost_micro_cny"] = json!(cost);
    value["platform_covered_micro_cny"] = json!(cost.saturating_sub(debit));
    value["remaining_micro_cny"] = json!(remaining);
    value["unknown_count"] = json!(unknown);
    // These are planning amounts under today's active basis, not a conversion
    // of historical debits that may span several differently priced periods.
    value["planning_allowance"] = Value::Null;
    if let Some(owner) = value["user_id"].as_str() {
        let now = crate::multi_user_host::now();
        let current: Option<(String,i64)> = db.query_row("SELECT period_id,expires_at FROM allowance_periods WHERE user_id=? AND closed_at IS NULL AND starts_at<=? AND ?<expires_at",params![owner,now,now],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(api_storage)?;
        if let Some((period, expires_at)) = current {
            let basis = crate::account_allowance::basis(db,&period)?;
            let percentages = basis.as_ref().map(|basis|json!({"maximum":basis.percentage(value["max_micro_cny"].as_i64().unwrap()),"remaining":basis.percentage(remaining)}));
            value["planning_allowance"] = json!({"period_id":period,"expires_at":expires_at,"allowance_basis":basis,"percentages":percentages});
        }
    }
    Ok(value)
}

/// Also used by manual reconciliation so it cannot bypass an accepted ceiling.
pub(crate) fn capped_debit(db: &Connection, call: &str, cost: i64) -> Result<i64, ToolError> {
    let budget: Option<(String,i64)> = db.query_row("SELECT e.task_ref,b.max_micro_cny FROM external_build_calls x JOIN external_build_executions e USING(execution_id) JOIN build_task_budgets b USING(task_ref) WHERE x.call_id=? AND b.cost_owner='account'",[call],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(api_storage)?;
    let Some((task, limit)) = budget else { return Ok(cost) };
    let other: i64 = db.query_row("SELECT COALESCE(sum(c.account_debit_micro_cny),0) FROM model_call_charges c JOIN external_build_calls x USING(call_id) JOIN external_build_executions e USING(execution_id) WHERE e.task_ref=? AND c.call_id<>?",params![task,call],|r|r.get(0)).map_err(api_storage)?;
    Ok(cost.min(limit.saturating_sub(other).max(0)))
}

pub(crate) fn set_budget(db: &Connection, actor: &str, task: &str, input: Budget, now: i64) -> Result<Value, ToolError> {
        if !nonempty(task) || input.max_micro_cny <= 0 || !nonempty(&input.confirmation)
            || !matches!(input.cost_owner.as_str(), "account"|"platform")
            || (input.cost_owner == "account" && (input.user_id.is_none() || input.learning_workspace_id.is_none()))
            || (input.cost_owner == "platform" && (input.user_id.is_some() || input.learning_workspace_id.is_some())) { return Err(invalid()) }
        let request = json!(input).to_string();
        let repeated: Option<String> = db.query_row("SELECT request_json FROM build_budget_confirmations WHERE task_ref=? AND confirmation=?",params![task,input.confirmation],|r|r.get(0)).optional().map_err(api_storage)?;
        if let Some(repeated) = repeated { return if repeated == request { summary(db,task) } else { Err(conflict("BUILD_BUDGET_CONFLICT","This confirmation was already used for a different budget")) }; }
        let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM build_task_budgets WHERE task_ref=?)",[task],|r|r.get(0)).map_err(api_storage)?;
        if input.cost_owner == "account" { authorize(db,input.user_id.as_deref().unwrap(),input.learning_workspace_id.as_deref().unwrap())?; }
        let revision = if exists {
            let previous = summary(db,task)?;
            if previous["cost_owner"] != json!(input.cost_owner) || previous["user_id"] != json!(input.user_id)
                || previous["learning_workspace_id"] != json!(input.learning_workspace_id)
                || input.expected_revision != previous["revision"].as_i64()
                || input.max_micro_cny <= previous["max_micro_cny"].as_i64().unwrap() {
                return Err(conflict("BUILD_BUDGET_CONFLICT","Use the current revision and a new explicit confirmation to increase this task budget"));
            }
            db.execute("UPDATE build_task_budgets SET max_micro_cny=?,confirmation=?,request_json=?,revision=revision+1 WHERE task_ref=?",params![input.max_micro_cny,input.confirmation,request,task]).map_err(api_storage)?;
            previous["revision"].as_i64().unwrap()+1
        } else {
            if input.expected_revision.is_some() { return Err(conflict("BUILD_BUDGET_CONFLICT","The task budget has not been created")); }
            db.execute("INSERT INTO build_task_budgets(task_ref,cost_owner,user_id,learning_workspace_id,max_micro_cny,confirmation,request_json,created_by,created_at) VALUES(?,?,?,?,?,?,?,?,?)",params![task,input.cost_owner,input.user_id,input.learning_workspace_id,input.max_micro_cny,input.confirmation,request,actor,now]).map_err(api_storage)?;
            0
        };
        db.execute("INSERT INTO build_budget_confirmations VALUES(?,?,?,?,?,?)",params![task,input.confirmation,request,revision,actor,now]).map_err(api_storage)?;
        let value = summary(db,task)?;
        Ok(value)
}

impl ModelSpendStore {
    pub(crate) fn build_budget(&self, actor: &str, task: &str, input: Budget, now: i64) -> Result<Value, ToolError> {
        let mut control = self.control.lock().unwrap();
        let tx = control.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(api_storage)?;
        let value = set_budget(&tx, actor, task, input, now)?;
        tx.commit().map_err(api_storage)?;
        Ok(value)
    }

    pub(crate) fn build_status(&self, task: &str, call: Option<&str>) -> Result<Value, ToolError> {
        let control = self.control.lock().unwrap();
        let budget = summary(&control.connection,task)?;
        if let Some(call) = call {
            require_call(&control.connection,task,call)?;
            Ok(json!({"charge":charge(&control.connection,call)?,"budget":budget}))
        } else { Ok(budget) }
    }

    pub(crate) fn build_admit(&self, task: &str, input: Admission, now: i64) -> Result<Value, ToolError> {
        self.build_admit_priced(task, input, None, now)
    }

    pub(crate) fn build_metered_admit(&self, task: &str, mut input: Value, now: i64) -> Result<Value, ToolError> {
        let request = input.as_object_mut().and_then(|v| v.remove("request")).ok_or_else(invalid)?;
        let rate_error = |e: runtime::model_spend::SpendStop| error(e.code(), "conflict", "Configured model pricing is unavailable for this preparation");
        let rate = self.rates.as_ref().map_err(|e| rate_error(*e))?.select(input["provider"].as_str().ok_or_else(invalid)?, input["model"].as_str().ok_or_else(invalid)?, now).map_err(rate_error)?;
        if request["model"] != rate.model { return Err(invalid()); }
        let count = |key: &str| request[key].as_u64().and_then(|v| u32::try_from(v).ok()).filter(|v| *v > 0).ok_or_else(invalid);
        let estimate = rate.cost(&runtime::provider_stream::ModelUsage {input_tokens:Some(count("input_tokens_estimate")?),output_tokens:Some(count("max_tokens")?),cached_input_tokens:Some(0),..Default::default()}).map_err(|_|invalid())?;
        input["reserved_micro_cny"] = json!(estimate);
        input["budget_enforced"] = json!(true);
        input["receipt_capability"] = json!("complete");
        input["granularity"] = json!("calls");
        let admission = serde_json::from_value(input).map_err(|_| invalid())?;
        self.build_admit_priced(task, admission, Some(json!(rate)), now)
    }

    pub(crate) fn build_metered_receipt(&self, task: &str, mut input: Receipt, now: i64) -> Result<Value, ToolError> {
        let rate: crate::model_rates::ModelRateSnapshot = {
            let c = self.control.lock().unwrap();
            require_call(&c.connection, task, &input.call_id)?;
            let raw: String = c.connection.query_row("SELECT rate_snapshot FROM model_call_charges WHERE call_id=?", [&input.call_id], |r| r.get(0)).map_err(api_storage)?;
            serde_json::from_str(&raw).map_err(|_| invalid())?
        };
        input.provider_cost_micro_cny = None;
        if input.status != "not_sent" {
            let usage = input.usage.clone().and_then(|v| serde_json::from_value::<runtime::provider_stream::ModelUsage>(v).ok());
            let cost = usage.as_ref().and_then(|u| rate.cost(u).ok());
            input.status = if cost.is_some() { "confirmed" } else if input.usage.is_some() { "partial" } else { "unknown" }.into();
            input.provider_cost_micro_cny = cost.map(|c| c.value());
        }
        input.evidence["source"] = json!("provider_calls_v1");
        input.evidence["rate_version"] = json!(rate.version);
        self.build_receipt(task, input, now)
    }

    fn build_admit_priced(&self, task: &str, input: Admission, rate: Option<Value>, now: i64) -> Result<Value, ToolError> {
        if [&input.execution_id,&input.call_id,&input.logical_call_id,&input.provider,&input.model,&input.purpose].iter().any(|v|!nonempty(v))
            || input.attempt == 0 || input.reserved_micro_cny <= 0
            || !matches!(input.granularity.as_str(),"execution_total"|"calls")
            || !matches!(input.receipt_capability.as_str(),"complete"|"partial"|"none") { return Err(invalid()) }
        let mut control = self.control.lock().unwrap();
        let tx = control.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(api_storage)?;
        let budget = summary(&tx,task)?;
        let owner = budget["user_id"].as_str();
        let workspace = budget["learning_workspace_id"].as_str();
        if owner.is_some() && (!input.budget_enforced || input.receipt_capability != "complete") {
            return Err(conflict("BUILD_EXECUTOR_UNMETERED","Personal paid builds require enforceable admission and complete cost receipts"));
        }
        let duplicate: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM model_call_charges WHERE call_id=? OR (logical_call_id=? AND attempt=?))",params![input.call_id,input.logical_call_id,input.attempt],|r|r.get(0)).map_err(api_storage)?;
        if duplicate { return Err(conflict("BUILD_SEND_ALREADY_ADMITTED","This transport identity was already admitted; read its receipt before continuing")); }
        let existing: Option<(String,String)> = tx.query_row("SELECT task_ref,granularity FROM external_build_executions WHERE execution_id=?",[&input.execution_id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(api_storage)?;
        if let Some((prior_task,granularity)) = &existing {
            if prior_task != task || granularity != &input.granularity || granularity == "execution_total" {
                return Err(conflict("BUILD_SETTLEMENT_LEVEL_CONFLICT","An execution uses exactly one settlement level; a new physical total execution needs a new identity"));
            }
        }
        if budget["remaining_micro_cny"].as_i64().unwrap() < input.reserved_micro_cny { return Err(conflict("BUILD_BUDGET_EXHAUSTED","The parent task has insufficient remaining budget")); }
        let period = if let Some(owner) = owner {
            authorize(&tx,owner,workspace.unwrap())?;
            let period: Option<String> = tx.query_row("SELECT period_id FROM allowance_periods WHERE user_id=? AND closed_at IS NULL AND starts_at<=? AND ?<expires_at",params![owner,now,now],|r|r.get(0)).optional().map_err(api_storage)?;
            let period = period.ok_or_else(|| conflict("MODEL_ALLOWANCE_EXPIRED","No active allowance period"))?;
            if crate::account_allowance::balance(&tx,&period)?.available_micro_cny.value() < input.reserved_micro_cny { return Err(conflict("MODEL_ALLOWANCE_INSUFFICIENT","Insufficient account allowance")); }
            let confirmed: Option<String> = tx.query_row("SELECT json_extract(task_json,'$.confirmed_period_id') FROM operator_builds WHERE task_id=?", [task], |r| r.get(0)).optional().map_err(api_storage)?.flatten();
            if confirmed.as_ref().is_some_and(|expected| expected != &period) { return Err(conflict("BUILD_PERIOD_CONFIRMATION_REQUIRED", "Confirm the current allowance period before continuing")); }
            Some(period)
        } else { None };
        if existing.is_none() { tx.execute("INSERT INTO external_build_executions VALUES(?,?,?,?)",params![input.execution_id,task,input.granularity,now]).map_err(api_storage)?; }
        let identity = json!({"external":true,"execution_id":input.execution_id,"call_id":input.call_id,"logical_call_id":input.logical_call_id,"attempt":input.attempt,"provider":input.provider,"model":input.model,"granularity":input.granularity});
        tx.execute("INSERT INTO model_call_charges(call_id,logical_call_id,attempt,user_id,period_id,task_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_cost_status,evidence_kind,created_at,send_identity,cost_owner,parent_task_ref,learning_workspace_id) VALUES(?,?,?,?,?,?,?,?,?,?,'sent',?,'pending','external_admitted',?,?,?,?,?)",params![input.call_id,input.logical_call_id,input.attempt,owner,period,input.execution_id,input.purpose,input.model,rate.unwrap_or_else(|| json!({"source":"external_receipt","provider":input.provider})).to_string(),json!({"reserved_micro_cny":input.reserved_micro_cny,"budget_enforced":input.budget_enforced,"receipt_capability":input.receipt_capability}).to_string(),input.reserved_micro_cny,now,identity.to_string(),budget["cost_owner"].as_str().unwrap(),task,workspace]).map_err(api_storage)?;
        tx.execute("INSERT INTO external_build_calls VALUES(?,?,?)",params![input.call_id,input.execution_id,json!(input).to_string()]).map_err(api_storage)?;
        let result = json!({"admitted":true,"call_id":input.call_id,"budget":summary(&tx,task)?});
        tx.commit().map_err(api_storage)?;
        Ok(result)
    }

    pub(crate) fn build_receipt(&self, task: &str, input: Receipt, now: i64) -> Result<Value, ToolError> {
        if !matches!(input.status.as_str(),"confirmed"|"partial"|"unknown"|"not_sent")
            || input.provider_cost_micro_cny.is_some_and(|v|v<0)
            || !input.evidence.as_object().is_some_and(|v|!v.is_empty())
            || (input.status == "confirmed" && input.provider_cost_micro_cny.is_none())
            || (input.status == "not_sent" && input.provider_cost_micro_cny.is_some_and(|v|v!=0)) { return Err(invalid()) }
        let report = json!(input).to_string();
        let mut control = self.control.lock().unwrap();
        let tx = control.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(api_storage)?;
        require_call(&tx,task,&input.call_id)?;
        let before = charge(&tx,&input.call_id)?;
        let prior: Option<String> = tx.query_row("SELECT disposition FROM charge_reports WHERE call_id=? AND outcome_json=?",params![input.call_id,report],|r|r.get(0)).optional().map_err(api_storage)?;
        if let Some(prior) = prior {
            return if prior == "accepted" { Ok(json!({"charge":before,"budget":summary(&tx,task)?})) } else { Err(conflict("BUILD_RECEIPT_CONFLICT","A conflicting receipt requires reconciliation")) };
        }
        let waived_pending = before["state"] == "released"
            && before["evidence_kind"] == "manual_reconciliation"
            && before["provider_cost_status"] == "pending";
        if matches!(before["state"].as_str(),Some("settled"|"released")) && !waived_pending {
            tx.execute("INSERT INTO charge_reports VALUES(?,?,'conflict',?)",params![input.call_id,report,now]).map_err(api_storage)?;
            tx.execute("UPDATE model_call_charges SET needs_reconciliation=1,revision=revision+1 WHERE call_id=?",[&input.call_id]).map_err(api_storage)?;
            tx.commit().map_err(api_storage)?;
            return Err(conflict("BUILD_RECEIPT_CONFLICT","The settled transport already has a different receipt"));
        }
        let cost = match input.status.as_str() { "confirmed" => input.provider_cost_micro_cny, "not_sent" => Some(0), _ => None };
        let debit = if waived_pending { Some(0) } else { cost.map(|cost| if before["cost_owner"] == "platform" { Ok(0) } else { capped_debit(&tx,&input.call_id,cost) }).transpose()? };
        let state = if waived_pending { "released" } else { match input.status.as_str() { "confirmed" => "settled", "not_sent" => "released", _ => "pending" } };
        let evidence = if waived_pending { "manual_reconciliation".to_owned() } else { format!("external_{}",input.status) };
        tx.execute("UPDATE model_call_charges SET state=?,provider_usage=?,provider_cost_micro_cny=?,provider_cost_status=?,account_debit_micro_cny=?,evidence_kind=?,settled_at=?,send_outcome=?,revision=revision+1 WHERE call_id=?",params![state,input.usage.as_ref().map(Value::to_string),cost,if cost.is_some(){"confirmed"}else{"pending"},debit,evidence,debit.map(|_|now),report,input.call_id]).map_err(api_storage)?;
        tx.execute("INSERT INTO charge_reports VALUES(?,?,'accepted',?)",params![input.call_id,report,now]).map_err(api_storage)?;
        let result = json!({"charge":charge(&tx,&input.call_id)?,"budget":summary(&tx,task)?});
        tx.commit().map_err(api_storage)?;
        Ok(result)
    }
}

fn authorize(db: &Connection, owner: &str, workspace: &str) -> Result<(), ToolError> {
    let allowed: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM learning_workspaces w JOIN users u ON u.user_id=w.owner_user_id WHERE w.owner_user_id=? AND w.learning_workspace_id=? AND w.archived=0 AND u.disabled=0)",params![owner,workspace],|r|r.get(0)).map_err(api_storage)?;
    if allowed { Ok(()) } else { Err(conflict("BUILD_WORKSPACE_UNAVAILABLE","The originating account or workspace is unavailable")) }
}
fn require_call(db: &Connection, task: &str, call: &str) -> Result<(), ToolError> {
    let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM external_build_calls x JOIN external_build_executions e USING(execution_id) WHERE x.call_id=? AND e.task_ref=?)",params![call,task],|r|r.get(0)).map_err(api_storage)?;
    if exists { Ok(()) } else { Err(crate::authorization::missing()) }
}
