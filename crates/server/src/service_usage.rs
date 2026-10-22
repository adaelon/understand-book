//! Server-priced processing receipts share account allowance and the parent
//! build ceiling. The service fee and the platform's operating cost are separate.
use crate::{admin_store::invalid, external_build_costs, model_spend_store::{api_storage, ModelSpendStore}, user_storage_paths::error};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ServiceRate {
    pub version: String,
    pub service_kind: String,
    pub starts_at: i64,
    pub expires_at: i64,
    pub debit_micro_cny_per_second: i64,
    /// An operating-cost estimate, not a supplier invoice or an account debit.
    pub estimated_cost_micro_cny_per_second: Option<i64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rates { rates: Vec<ServiceRate> }
fn unavailable() -> ToolError {
    error("SERVICE_RATE_UNAVAILABLE", "conflict", "视频转写尚未配置可用服务费率，暂不能启动个人付费处理")
}
fn conflict(code: &str, message: &str) -> ToolError { error(code, "conflict", message) }
fn price(quantity_ms: i64, per_second: i64) -> Result<i64, ToolError> {
    if quantity_ms < 0 || per_second < 0 { return Err(invalid()); }
    i64::try_from((i128::from(quantity_ms) * i128::from(per_second) + 999) / 1000).map_err(|_| invalid())
}
pub(crate) fn rate(root: &Path, kind: &str, now: i64) -> Result<ServiceRate, ToolError> {
    let bytes = std::fs::read(root.join("service-rates.json")).map_err(|_| unavailable())?;
    let rates: Rates = serde_json::from_slice(&bytes).map_err(|_| unavailable())?;
    for (i, r) in rates.rates.iter().enumerate() {
        if r.version.trim().is_empty() || r.service_kind != "video_transcription"
            || r.starts_at >= r.expires_at || r.debit_micro_cny_per_second <= 0
            || r.estimated_cost_micro_cny_per_second.is_some_and(|v| v < 0)
            || rates.rates[..i].iter().any(|p| p.version == r.version
                || (p.service_kind == r.service_kind && p.starts_at < r.expires_at && r.starts_at < p.expires_at)) {
            return Err(unavailable());
        }
    }
    rates.rates.into_iter().find(|r| r.service_kind == kind && r.starts_at <= now && now < r.expires_at).ok_or_else(unavailable)
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ServiceAdmission {
    pub usage_id: String,
    pub logical_usage_id: String,
    pub attempt: u32,
    pub service_kind: String,
    pub quantity_ms: i64,
    pub processing_confirmation: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ServiceReceipt {
    pub usage_id: String,
    pub status: String,
    pub quantity_ms: Option<i64>,
    pub evidence: Value,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ServiceReconcile {
    pub operation_id: String,
    pub revision: i64,
    pub status: String,
    pub quantity_ms: Option<i64>,
    pub reason: String,
    pub evidence: String,
}

pub(crate) fn charge(db: &Connection, id: &str) -> Result<Option<Value>, ToolError> {
    let value = db.query_row("SELECT usage_id,logical_usage_id,attempt,task_ref,cost_owner,user_id,period_id,learning_workspace_id,service_kind,admitted_quantity_ms,quantity_ms,rate_snapshot,state,reserved_micro_cny,service_charge_micro_cny,account_debit_micro_cny,provider_cost_micro_cny,provider_cost_status,evidence_json,created_at,settled_at,revision,run_ref,parent_task_ref FROM service_usage WHERE usage_id=?", [id], |r| Ok(json!({
        "entry_kind":"service","call_id":r.get::<_,String>(0)?,"usage_id":r.get::<_,String>(0)?,
        "logical_usage_id":r.get::<_,String>(1)?,"attempt":r.get::<_,i64>(2)?,
        "task_ref":r.get::<_,Option<String>>(3)?,"parent_task_ref":r.get::<_,String>(23)?,"run_ref":r.get::<_,Option<String>>(22)?,
        "cost_owner":r.get::<_,String>(4)?,"user_id":r.get::<_,Option<String>>(5)?,
        "period_id":r.get::<_,Option<String>>(6)?,"learning_workspace_id":r.get::<_,Option<String>>(7)?,
        "service_kind":r.get::<_,String>(8)?,"purpose":if r.get::<_,Option<String>>(22)?.is_some() { "video_audio_review".to_owned() } else { r.get::<_,String>(8)? },"model":Value::Null,
        "admitted_quantity_ms":r.get::<_,i64>(9)?,"quantity_ms":r.get::<_,Option<i64>>(10)?,
        "rate_snapshot":r.get::<_,String>(11)?,"state":r.get::<_,String>(12)?,
        "reserved_micro_cny":r.get::<_,i64>(13)?,"service_charge_micro_cny":r.get::<_,Option<i64>>(14)?,
        "account_debit_micro_cny":r.get::<_,Option<i64>>(15)?,
        "provider_cost_micro_cny":r.get::<_,Option<i64>>(16)?,"provider_cost_status":r.get::<_,String>(17)?,
        "evidence":r.get::<_,Option<String>>(18)?,"created_at":r.get::<_,i64>(19)?,
        "settled_at":r.get::<_,Option<i64>>(20)?,"revision":r.get::<_,i64>(21)?,
        "needs_reconciliation":false,"related_learning_workspace_ids":[]
    }))).optional().map_err(api_storage)?;
    value.map(|mut value| {
        for key in ["rate_snapshot","evidence"] {
            if let Some(raw) = value[key].as_str() { value[key] = serde_json::from_str(raw).map_err(api_storage)?; }
        }
        crate::account_allowance::project_charge(db,&mut value)?;
        value["platform_covered_micro_cny"] = value["provider_cost_micro_cny"].as_i64()
            .zip(value["account_debit_micro_cny"].as_i64()).map(|(cost,debit)|json!(cost.saturating_sub(debit).max(0))).unwrap_or(Value::Null);
        Ok(value)
    }).transpose()
}

pub(crate) fn read_charge(db: &Connection, id: &str) -> Result<Value, ToolError> {
    if let Some(value) = charge(db,id)? { Ok(value) } else { crate::model_spend_store::charge(db,id) }
}

impl ModelSpendStore {
    pub(crate) fn service_reconcile(&self, actor:&str, id:&str, input:ServiceReconcile, now:i64) -> Result<Value,ToolError> {
        crate::admin_store::operation_id(&input.operation_id)?;
        if input.reason.trim().is_empty() || input.evidence.trim().is_empty() || !matches!(input.status.as_str(),"completed"|"not_started") { return Err(invalid()); }
        let parameters=json!({"usage_id":id,"input":input}).to_string();
        let mut c=self.control.lock().unwrap();
        let tx=c.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(api_storage)?;
        let prior:Option<(String,String)>=tx.query_row("SELECT parameters_json,result_json FROM admin_operations WHERE actor=? AND operation_id=?",params![actor,input.operation_id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(api_storage)?;
        if let Some((request,result))=prior {
            return if request==parameters {serde_json::from_str(&result).map_err(api_storage)} else {Err(conflict("SERVICE_RECEIPT_CONFLICT","该核对操作标识已用于其他内容"))};
        }
        let before=charge(&tx,id)?.ok_or_else(crate::authorization::missing)?;
        if before["revision"].as_i64()!=Some(input.revision) || before["state"]!="pending" {
            return Err(conflict("SERVICE_RECEIPT_CONFLICT","仅可核对当前版本的待确认处理，请刷新费用详情"));
        }
        let result=settle(&tx,before["parent_task_ref"].as_str().ok_or_else(invalid)?,ServiceReceipt {
            usage_id:id.into(),status:input.status,quantity_ms:input.quantity_ms,
            evidence:json!({"source":"operator_processing_evidence","actor":actor,"operation_id":input.operation_id,"reason":input.reason,"evidence":input.evidence})
        },now)?;
        let receipt=json!({"operation_id":input.operation_id,"actor":actor,"user_id":before["user_id"],"kind":"service-usage-reconcile","created_at":now,"ok":true,"charge":result["charge"]});
        tx.execute("INSERT INTO admin_operations(actor,operation_id,user_id,kind,parameters_json,result_json,created_at) VALUES(?,?,?,'service-usage-reconcile',?,?,?)",
            params![actor,input.operation_id,before["user_id"].as_str().unwrap_or(actor),parameters,receipt.to_string(),now]).map_err(api_storage)?;
        tx.commit().map_err(api_storage)?;
        Ok(receipt)
    }
    pub(crate) fn service_charge(&self, task: &str, id: &str) -> Result<Value, ToolError> {
        let c = self.control.lock().unwrap();
        let value = charge(&c.connection,id)?.filter(|v|v["parent_task_ref"] == task).ok_or_else(crate::authorization::missing)?;
        Ok(json!({"budget":if value["task_ref"].is_null() { Value::Null } else {external_build_costs::summary(&c.connection,task)?},"charge":value}))
    }
    pub(crate) fn service_admit(&self, task: &str, input: ServiceAdmission, now: i64) -> Result<Value, ToolError> {
        if input.usage_id.trim().is_empty() || input.logical_usage_id.trim().is_empty()
            || input.attempt == 0 || input.quantity_ms <= 0 || input.service_kind != "video_transcription" { return Err(invalid()); }
        let request = json!(input).to_string();
        let mut c = self.control.lock().unwrap();
        let rate = rate(c.writer.root(),&input.service_kind,now);
        let tx = c.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(api_storage)?;
        let previous: Option<(String,String)> = tx.query_row("SELECT parent_task_ref,admission_json FROM service_usage WHERE usage_id=?", [&input.usage_id], |r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(api_storage)?;
        if let Some((prior_task, prior_request)) = previous {
            if prior_task != task || prior_request != request { return Err(conflict("SERVICE_ADMISSION_CONFLICT","该处理标识已用于其他用量")); }
            return Ok(json!({"admitted":true,"usage_id":input.usage_id,"charge":charge(&tx,&input.usage_id)?,"budget":external_build_costs::summary(&tx,task)?}));
        }
        let previous:Option<(String,i64,String)>=tx.query_row("SELECT state,attempt,admission_json FROM service_usage WHERE task_ref=? AND logical_usage_id=? ORDER BY attempt DESC LIMIT 1",params![task,input.logical_usage_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(api_storage)?;
        if let Some((state,attempt,prior))=previous {
            let prior:Value=serde_json::from_str(&prior).map_err(api_storage)?;
            let current:Option<String>=tx.query_row("SELECT json_extract(task_json,'$.processing_confirmation') FROM operator_builds WHERE task_id=?",[task],|r|r.get(0)).optional().map_err(api_storage)?.flatten();
            let continued=state=="pending" && input.processing_confirmation.is_some()
                && input.processing_confirmation==current
                && input.processing_confirmation.as_deref()!=prior["processing_confirmation"].as_str();
            if i64::from(input.attempt)!=attempt+1 || (state!="released" && !continued) {
                return Err(conflict("SERVICE_ALREADY_ADMITTED","已完成片段不能重复处理；未决片段需要明确继续并保留原预占"));
            }
        }
        let budget = external_build_costs::summary(&tx,task)?;
        let owner = budget["user_id"].as_str();
        let workspace = budget["learning_workspace_id"].as_str();
        let rate = if owner.is_some() { Some(rate?) } else { rate.ok() };
        let reserve = rate.as_ref().map(|r|price(input.quantity_ms,r.debit_micro_cny_per_second)).transpose()?.unwrap_or(0);
        if reserve > budget["remaining_micro_cny"].as_i64().ok_or_else(invalid)? {
            return Err(conflict("BUILD_BUDGET_EXHAUSTED","处理与模型调用已达到本次确认的总额度上限"));
        }
        let period = if let Some(owner) = owner {
            let allowed: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM learning_workspaces w JOIN users u ON u.user_id=w.owner_user_id WHERE w.owner_user_id=? AND w.learning_workspace_id=? AND w.archived=0 AND u.disabled=0)",params![owner,workspace],|r|r.get(0)).map_err(api_storage)?;
            if !allowed { return Err(conflict("BUILD_WORKSPACE_UNAVAILABLE","The originating account or workspace is unavailable")); }
            let period: Option<String> = tx.query_row("SELECT period_id FROM allowance_periods WHERE user_id=? AND closed_at IS NULL AND starts_at<=? AND ?<expires_at",params![owner,now,now],|r|r.get(0)).optional().map_err(api_storage)?;
            let period = period.ok_or_else(||conflict("MODEL_ALLOWANCE_EXPIRED","No active allowance period"))?;
            if crate::account_allowance::balance(&tx,&period)?.available_micro_cny.value() < reserve {
                return Err(conflict("MODEL_ALLOWANCE_INSUFFICIENT","Insufficient account allowance"));
            }
            let confirmed: Option<String> = tx.query_row("SELECT json_extract(task_json,'$.confirmed_period_id') FROM operator_builds WHERE task_id=?",[task],|r|r.get(0)).optional().map_err(api_storage)?.flatten();
            if confirmed.as_ref().is_some_and(|p|p != &period) { return Err(conflict("BUILD_PERIOD_CONFIRMATION_REQUIRED","Confirm the current allowance period before continuing")); }
            Some(period)
        } else { None };
        tx.execute("INSERT INTO service_usage(usage_id,logical_usage_id,attempt,task_ref,parent_task_ref,cost_owner,user_id,period_id,learning_workspace_id,service_kind,admitted_quantity_ms,rate_snapshot,admission_json,state,reserved_micro_cny,provider_cost_status,created_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,'sent',?,'pending',?)",
            params![input.usage_id,input.logical_usage_id,input.attempt,task,task,budget["cost_owner"].as_str(),owner,period,workspace,input.service_kind,input.quantity_ms,json!(rate).to_string(),request,reserve,now]).map_err(api_storage)?;
        let value = json!({"admitted":true,"usage_id":input.usage_id,"charge":charge(&tx,&input.usage_id)?,"budget":external_build_costs::summary(&tx,task)?});
        tx.commit().map_err(api_storage)?;
        Ok(value)
    }
    pub(crate) fn service_receipt(&self, task: &str, input: ServiceReceipt, now: i64) -> Result<Value, ToolError> {
        let mut c=self.control.lock().unwrap();
        let tx=c.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(api_storage)?;
        let result=settle(&tx,task,input,now)?;
        tx.commit().map_err(api_storage)?;
        Ok(result)
    }
    /// Called only by the already authorized network Run immediately before
    /// starting its bounded Whisper process. This is not an operator build.
    pub(crate) fn service_admit_run(&self, scope: &crate::run_scope::RunScope, quantity_ms: i64, now: i64) -> Result<String,ToolError> {
        if quantity_ms <= 0 || quantity_ms > 30_000 { return Err(invalid()); }
        let mut c=self.control.lock().unwrap();
        let rate=rate(c.writer.root(),"video_transcription",now)?;
        let reserved=price(quantity_ms,rate.debit_micro_cny_per_second)?;
        let tx=c.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(api_storage)?;
        let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM users WHERE user_id=? AND disabled=0)",[&scope.user_id],|r|r.get(0)).map_err(api_storage)?;
        if !active { return Err(crate::authorization::missing()); }
        let period:Option<String>=tx.query_row("SELECT period_id FROM allowance_periods WHERE user_id=? AND closed_at IS NULL AND starts_at<=? AND ?<expires_at",params![scope.user_id,now,now],|r|r.get(0)).optional().map_err(api_storage)?;
        let period=period.ok_or_else(||conflict("ALLOWANCE_EXPIRED","当前没有有效服务额度期"))?;
        if crate::account_allowance::balance(&tx,&period)?.available_micro_cny.value() < reserved {
            return Err(conflict("ALLOWANCE_INSUFFICIENT","当前服务额度不足以开始音频复核"));
        }
        let id=format!("svc_{}",uuid::Uuid::now_v7());
        tx.execute("INSERT INTO service_usage(usage_id,logical_usage_id,attempt,run_ref,parent_task_ref,cost_owner,user_id,period_id,learning_workspace_id,service_kind,admitted_quantity_ms,rate_snapshot,admission_json,state,reserved_micro_cny,provider_cost_status,created_at) VALUES(?,?,1,?,?,'account',?,?,?,'video_transcription',?,?,?,'sent',?,'pending',?)",
            params![id,id,scope.turn_id,scope.turn_id,scope.user_id,period,scope.learning_workspace_id,quantity_ms,json!(rate).to_string(),json!({"source":"video.review_audio","run_ref":scope.turn_id,"quantity_ms":quantity_ms}).to_string(),reserved,now]).map_err(api_storage)?;
        tx.commit().map_err(api_storage)?;
        Ok(id)
    }
}

fn settle(tx: &Connection, task: &str, input: ServiceReceipt, now: i64) -> Result<Value,ToolError> {
        if !matches!(input.status.as_str(),"completed"|"unknown"|"not_started")
            || input.quantity_ms.is_some_and(|v|v < 0)
            || (input.status == "completed" && input.quantity_ms.is_none())
            || (input.status == "not_started" && input.quantity_ms.is_some_and(|v|v != 0))
            || !input.evidence.as_object().is_some_and(|v|!v.is_empty()) { return Err(invalid()); }
        let report = json!(input).to_string();
        let before = charge(&tx,&input.usage_id)?.filter(|v|v["parent_task_ref"] == task).ok_or_else(crate::authorization::missing)?;
        let budget = |db:&Connection| if before["task_ref"].is_null() { Ok(Value::Null) } else { external_build_costs::summary(db,task) };
        let duplicate: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM service_usage_receipts WHERE usage_id=? AND report_json=?)",params![input.usage_id,report],|r|r.get(0)).map_err(api_storage)?;
        if duplicate { return Ok(json!({"charge":before,"budget":budget(&tx)?})); }
        if matches!(before["state"].as_str(),Some("settled"|"released")) { return Err(conflict("SERVICE_RECEIPT_CONFLICT","已结算的处理不能被不同回执改写")); }
        if input.quantity_ms.is_some_and(|v|v > before["admitted_quantity_ms"].as_i64().unwrap()) { return Err(conflict("SERVICE_QUANTITY_EXCEEDED","实际处理量超过获准范围")); }
        let rate: Option<ServiceRate> = serde_json::from_value(before["rate_snapshot"].clone()).map_err(|_|invalid())?;
        let quantity = match input.status.as_str() { "completed"=>input.quantity_ms, "not_started"=>Some(0), _=>None };
        let service_charge = quantity.map(|q|rate.as_ref().map(|r|price(q,r.debit_micro_cny_per_second)).transpose()).transpose()?.flatten();
        let operating_cost = if input.status == "not_started" { Some(0) } else {
            quantity.zip(rate.as_ref().and_then(|r|r.estimated_cost_micro_cny_per_second)).map(|(q,r)|price(q,r)).transpose()?
        };
        let debit = if input.status == "unknown" { None } else if before["cost_owner"] == "platform" || input.status == "not_started" { Some(0) } else if before["task_ref"].is_null() { Some(service_charge.ok_or_else(invalid)?) } else {
            let maximum:i64=tx.query_row("SELECT max_micro_cny FROM build_task_budgets WHERE task_ref=?",[task],|r|r.get(0)).map_err(api_storage)?;
            let other:i64=tx.query_row("SELECT coalesce(sum(account_debit_micro_cny),0) FROM billable_usage WHERE parent_task_ref=? AND NOT(entry_kind='service' AND call_id=?)",params![task,input.usage_id],|r|r.get(0)).map_err(api_storage)?;
            Some(service_charge.ok_or_else(invalid)?.min(maximum.saturating_sub(other).max(0)))
        };
        let state = match input.status.as_str() { "completed"=>"settled", "not_started"=>"released", _=>"pending" };
        let cost_status = if input.status == "not_started" { "confirmed" } else if operating_cost.is_some() { "estimated" } else { "pending" };
        tx.execute("UPDATE service_usage SET state=?,quantity_ms=?,service_charge_micro_cny=?,account_debit_micro_cny=?,provider_cost_micro_cny=?,provider_cost_status=?,evidence_json=?,settled_at=?,revision=revision+1 WHERE usage_id=?",
            params![state,quantity,service_charge,debit,operating_cost,cost_status,input.evidence.to_string(),debit.map(|_|now),input.usage_id]).map_err(api_storage)?;
        tx.execute("INSERT INTO service_usage_receipts VALUES(?,?,?)",params![input.usage_id,report,now]).map_err(api_storage)?;
        let result = json!({"charge":charge(&tx,&input.usage_id)?,"budget":budget(&tx)?});
        Ok(result)
}
