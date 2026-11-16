//! Append-only manual evidence; the charge row is the current balance projection.
use crate::{
    admin_store::invalid,
    model_spend_store::{api_storage, ModelSpendStore},
    user_storage_paths::error,
};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

fn charge(db: &Connection, id: &str) -> Result<Value, ToolError> {
    if let Some(value) = crate::web_usage::charge(db,id)? { return Ok(value); }
    let mut value = crate::model_spend_store::charge(db,id)?;
    value["entry_kind"] = json!("model");
    Ok(value)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Reconcile {
    pub revision: i64,
    pub provider_cost_micro_cny: Option<i64>,
    pub waive_account: bool,
    pub reason: String,
    pub evidence: String,
}
impl Reconcile {
    pub(crate) fn normalize(&mut self) -> Result<(), ToolError> {
        self.reason = self.reason.trim().into();
        self.evidence = self.evidence.trim().into();
        if self.reason.is_empty()
            || self.evidence.is_empty()
            || self.revision < 0
            || self.provider_cost_micro_cny.is_some_and(|v| v < 0)
            || (!self.waive_account && self.provider_cost_micro_cny.is_none())
        {
            return Err(invalid());
        }
        Ok(())
    }
    pub(crate) fn apply(
        &self,
        db: &Connection,
        actor: &str,
        op: &str,
        owner: &str,
        call: &str,
        now: i64,
    ) -> Result<Value, ToolError> {
        let before = charge(db, call)?;
        let web = matches!(before["entry_kind"].as_str(),Some("web_search"|"web_fetch"));
        if before["user_id"].as_str() != Some(owner) {
            return Err(crate::authorization::missing());
        }
        if before["revision"].as_i64() != Some(self.revision) {
            return Err(error(
                "CHARGE_REVISION_CONFLICT",
                "conflict",
                "Charge has changed",
            ));
        }
        if matches!(before["state"].as_str(), Some("reserved" | "sent")) {
            return Err(error(
                "CHARGE_IN_FLIGHT",
                "conflict",
                "Wait for the call to finish before reconciling",
            ));
        }
        // Waiving an account with no new supplier evidence preserves any already
        // confirmed supplier cost; unknown supplier cost remains explicitly null.
        let cost = self
            .provider_cost_micro_cny
            .or_else(|| before["provider_cost_micro_cny"].as_i64());
        let debit = if self.waive_account {
            0
        } else if web {
            cost.ok_or_else(invalid)?
        } else {
            crate::external_build_costs::capped_debit(db, call, cost.ok_or_else(invalid)?)?
        };
        let state = if self.waive_account {
            "released"
        } else {
            "settled"
        };
        if web {
            db.execute("UPDATE web_usage SET state=?,provider_cost_micro_cny=?,provider_cost_status=?,account_debit_micro_cny=?,evidence_json=?,settled_at=?,revision=revision+1,needs_reconciliation=0 WHERE usage_id=?",params![state,cost,if cost.is_some(){"confirmed"}else{"pending"},debit,json!({"source":"manual_reconciliation","actor":actor,"operation_id":op}).to_string(),now,call]).map_err(api_storage)?;
        } else {
            db.execute("UPDATE model_call_charges SET state=?,provider_cost_micro_cny=?,provider_cost_status=?,account_debit_micro_cny=?,evidence_kind='manual_reconciliation',settled_at=?,revision=revision+1,needs_reconciliation=0 WHERE call_id=?",params![state,cost,if cost.is_some(){"confirmed"}else{"pending"},debit,now,call]).map_err(api_storage)?;
        }
        let after = charge(db, call)?;
        let delta = debit
            .checked_sub(before["account_debit_micro_cny"].as_i64().unwrap_or(0))
            .ok_or_else(invalid)?;
        let provider_delta = cost
            .zip(before["provider_cost_micro_cny"].as_i64())
            .map(|(a, b)| a - b);
        db.execute(
            if web { "INSERT INTO web_usage_reconciliations VALUES(?,?,?,?,?,?,?,?,?,?,?)" } else { "INSERT INTO charge_reconciliations VALUES(?,?,?,?,?,?,?,?,?,?,?)" },
            params![
                actor,
                op,
                owner,
                call,
                before.to_string(),
                after.to_string(),
                delta,
                provider_delta,
                self.reason,
                self.evidence,
                now
            ],
        )
        .map_err(api_storage)?;
        Ok(
            json!({"charge":after,"account_delta_micro_cny":delta,"provider_delta_micro_cny":provider_delta}),
        )
    }
}

impl ModelSpendStore {
    pub(crate) fn allowance(&self, owner: &str, now: i64) -> Result<Value, ToolError> {
        let mut control = self.control.lock().unwrap();
        let tx = control.connection.transaction().map_err(api_storage)?;
        let current = crate::allowance_admin::current(&tx, owner, now)?;
        let latest_id: Option<String> = tx.query_row("SELECT period_id FROM allowance_periods WHERE user_id=? ORDER BY starts_at DESC,period_id DESC LIMIT 1", [owner], |r|r.get(0)).optional().map_err(api_storage)?;
        let mut latest = latest_id.map(|id|crate::allowance_admin::period(&tx, owner, &id)).transpose()?.unwrap_or(Value::Null);
        if !latest.is_null() { latest["validity_state"] = json!(crate::allowance_admin::validity_state(&latest, now)); }
        let status = if !current.is_null() { "active" } else if latest.is_null() { "no_period" } else { crate::allowance_admin::validity_state(&latest, now) };
        let offers = crate::allowance_offers::list(&tx, !current.is_null() && current["balance"]["unlimited"] != true, true)?;
        Ok(json!({"current_allowance":current,"latest_allowance":latest,"status":status,
            "replenishment":{"offers":offers["items"]}}))
    }
    pub(crate) fn charges(
        &self,
        owner: Option<&str>,
        pending: bool,
        limit: u32,
        offset: u32,
        reader: bool,
        range: Option<&crate::admin_usage::Range>,
        reference: Option<(&str, &str)>,
        cost_owner: Option<&str>,
    ) -> Result<Value, ToolError> {
        let mut control = self.control.lock().unwrap();
        let tx = control.connection.transaction().map_err(api_storage)?;
        let filter = "(?1 IS NULL OR user_id=?1) AND (?2=0 OR state='pending' OR (state='released' AND provider_cost_status='pending') OR needs_reconciliation=1) AND (?5 IS NULL OR (created_at>=?5 AND created_at<?6)) AND (?7 IS NULL OR (?7='run' AND run_ref=?8) OR (?7='task' AND cost_owner='account' AND run_ref IS NULL AND task_ref=?8) OR (?7='parent_task' AND coalesce(parent_task_ref,run_ref,task_ref)=?8) OR (?7='operator_task' AND cost_owner='platform' AND task_ref=?8) OR (?7='workspace' AND cost_owner='account' AND learning_workspace_id=?8) OR (?7='account_level' AND cost_owner='account' AND learning_workspace_id IS NULL)) AND (?9 IS NULL OR cost_owner=?9)";
        let mut stmt = tx.prepare(&format!("SELECT call_id FROM billable_usage WHERE {filter} ORDER BY created_at DESC,call_id DESC LIMIT ?3 OFFSET ?4")).map_err(api_storage)?;
        let ids = stmt
            .query_map(params![owner, pending, limit, offset, range.map(|r|r.start), range.map(|r|r.end), reference.map(|r|r.0), reference.map(|r|r.1), cost_owner], |r| {
                r.get::<_, String>(0)
            })
            .map_err(api_storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(api_storage)?;
        let items = ids
            .iter()
            .map(|id| {
                let value = if let Some(web) = crate::web_usage::charge(&tx,id)? { web } else { crate::service_usage::read_charge(&tx, id)? };
                Ok(if reader { reader_charge(value) } else { value })
            })
            .collect::<Result<Vec<_>, ToolError>>()?;
        let total: i64 = tx
            .query_row(
                &format!("SELECT count(*) FROM billable_usage WHERE {filter}"),
                params![owner, pending, limit, offset, range.map(|r|r.start), range.map(|r|r.end), reference.map(|r|r.0), reference.map(|r|r.1), cost_owner],
                |r| r.get(0),
            )
            .map_err(api_storage)?;
        // Group before percentage formatting, over the full selected history.
        // Different periods can have different denominators, so no combined %.
        let mut groups = tx.prepare(&format!("SELECT period_id,COALESCE(sum(account_debit_micro_cny),0),COALESCE(sum(CASE WHEN state IN ('reserved','sent') THEN reserved_micro_cny ELSE 0 END),0),COALESCE(sum(CASE WHEN state='pending' THEN reserved_micro_cny ELSE 0 END),0),sum(CASE WHEN provider_cost_status='pending' THEN 1 ELSE 0 END) FROM billable_usage WHERE ({filter}) AND period_id IS NOT NULL GROUP BY period_id ORDER BY period_id")).map_err(api_storage)?;
        let rows = groups.query_map(params![owner,pending,limit,offset,range.map(|r|r.start),range.map(|r|r.end),reference.map(|r|r.0),reference.map(|r|r.1),cost_owner], |r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?,r.get::<_,i64>(4)?))).map_err(api_storage)?.collect::<Result<Vec<_>,_>>().map_err(api_storage)?;
        let period_groups = rows.into_iter().map(|(period,debited,active,pending,unknown)| {
            let basis = crate::account_allowance::basis(&tx,&period)?;
            let percentages = basis.as_ref().map(|b|json!({"debited":b.percentage(debited),"active_reserved":b.percentage(active),"pending":b.percentage(pending)}));
            Ok(json!({"period_id":period,"allowance_basis":basis,"debited_micro_cny":debited,"active_reserved_micro_cny":active,"pending_micro_cny":pending,"unknown_provider_cost_count":unknown,"percentages":percentages}))
        }).collect::<Result<Vec<_>,ToolError>>()?;
        Ok(json!({"items":items,"total":total,"limit":limit,"offset":offset,"period_groups":period_groups}))
    }
    pub(crate) fn charge_detail(
        &self,
        call: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Value, ToolError> {
        let mut control = self.control.lock().unwrap();
        let tx = control.connection.transaction().map_err(api_storage)?;
        if let Some(mut value) = crate::service_usage::charge(&tx,call)? {
            let mut reports = tx.prepare("SELECT report_json,received_at FROM service_usage_receipts WHERE usage_id=? ORDER BY received_at DESC LIMIT ? OFFSET ?").map_err(api_storage)?;
            let rows = reports.query_map(params![call,limit,offset], |r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?))).map_err(api_storage)?.collect::<Result<Vec<_>,_>>().map_err(api_storage)?;
            let items = rows.into_iter().map(|(raw,at)|serde_json::from_str::<Value>(&raw).map(|outcome|json!({"outcome":outcome,"disposition":"accepted","received_at":at})).map_err(api_storage)).collect::<Result<Vec<_>,_>>()?;
            let total: i64=tx.query_row("SELECT count(*) FROM service_usage_receipts WHERE usage_id=?",[call],|r|r.get(0)).map_err(api_storage)?;
            value["reports"]=json!({"items":items,"total":total,"limit":limit,"offset":offset});
            let mut reconciliations=tx.prepare("SELECT actor,operation_id,json_extract(parameters_json,'$.input.reason'),json_extract(parameters_json,'$.input.evidence'),created_at FROM admin_operations WHERE kind='service-usage-reconcile' AND json_extract(parameters_json,'$.usage_id')=? ORDER BY created_at DESC LIMIT ? OFFSET ?").map_err(api_storage)?;
            let items=reconciliations.query_map(params![call,limit,offset],|r|Ok(json!({"actor":r.get::<_,String>(0)?,"operation_id":r.get::<_,String>(1)?,"reason":r.get::<_,String>(2)?,"evidence":r.get::<_,String>(3)?,"created_at":r.get::<_,i64>(4)?}))).map_err(api_storage)?.collect::<Result<Vec<_>,_>>().map_err(api_storage)?;
            let total:i64=tx.query_row("SELECT count(*) FROM admin_operations WHERE kind='service-usage-reconcile' AND json_extract(parameters_json,'$.usage_id')=?",[call],|r|r.get(0)).map_err(api_storage)?;
            value["reconciliations"]=json!({"items":items,"total":total,"limit":limit,"offset":offset});
            return Ok(value);
        }
        let mut value = charge(&tx, call)?;
        let web = matches!(value["entry_kind"].as_str(),Some("web_search"|"web_fetch"));
        let (reconciliations, id_column) = if web { ("web_usage_reconciliations","usage_id") } else { ("charge_reconciliations","call_id") };
        let mut stmt=tx.prepare(&format!("SELECT actor,operation_id,before_json,after_json,account_delta_micro_cny,provider_delta_micro_cny,reason,evidence,created_at FROM {reconciliations} WHERE {id_column}=? ORDER BY created_at DESC,actor,operation_id LIMIT ? OFFSET ?")).map_err(api_storage)?;
        let items=stmt.query_map(params![call,limit,offset],|r|Ok(json!({"actor":r.get::<_,String>(0)?,"operation_id":r.get::<_,String>(1)?,"before":r.get::<_,String>(2)?,"after":r.get::<_,String>(3)?,"account_delta_micro_cny":r.get::<_,i64>(4)?,"provider_delta_micro_cny":r.get::<_,Option<i64>>(5)?,"reason":r.get::<_,String>(6)?,"evidence":r.get::<_,String>(7)?,"created_at":r.get::<_,i64>(8)?}))).map_err(api_storage)?.collect::<Result<Vec<_>,_>>().map_err(api_storage)?;
        let items = items
            .into_iter()
            .map(|mut item| {
                for field in ["before", "after"] {
                    item[field] =
                        serde_json::from_str(item[field].as_str().unwrap()).map_err(api_storage)?;
                }
                Ok(item)
            })
            .collect::<Result<Vec<_>, ToolError>>()?;
        let total: i64 = tx
            .query_row(
                &format!("SELECT count(*) FROM {reconciliations} WHERE {id_column}=?"),
                [call],
                |r| r.get(0),
            )
            .map_err(api_storage)?;
        value["reconciliations"] =
            json!({"items":items,"total":total,"limit":limit,"offset":offset});
        let mut stmt=tx.prepare(if web { "SELECT report_json,'accepted',received_at FROM web_usage_receipts WHERE usage_id=? ORDER BY received_at DESC,report_json LIMIT ? OFFSET ?" } else { "SELECT outcome_json,disposition,received_at FROM charge_reports WHERE call_id=? ORDER BY received_at DESC,outcome_json LIMIT ? OFFSET ?" }).map_err(api_storage)?;
        let rows = stmt
            .query_map(params![call, limit, offset], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .map_err(api_storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(api_storage)?;
        let items=rows.into_iter().map(|(outcome,disposition,received_at)|Ok(json!({"outcome":serde_json::from_str::<Value>(&outcome).map_err(api_storage)?,"disposition":disposition,"received_at":received_at}))).collect::<Result<Vec<_>,ToolError>>()?;
        let total: i64 = tx
            .query_row(
                if web { "SELECT count(*) FROM web_usage_receipts WHERE usage_id=?" } else { "SELECT count(*) FROM charge_reports WHERE call_id=?" },
                [call],
                |r| r.get(0),
            )
            .map_err(api_storage)?;
        value["reports"] = json!({"items":items,"total":total,"limit":limit,"offset":offset});
        Ok(value)
    }
}
fn reader_charge(value: Value) -> Value {
    let mut out = json!({});
    for field in [
        "call_id",
        "period_id",
        "run_ref",
        "task_ref",
        "parent_task_ref",
        "learning_workspace_id",
        "related_learning_workspace_ids",
        "purpose",
        "model",
        "entry_kind",
        "service_kind",
        "request_count",
        "quantity_ms",
        "state",
        "provider_cost_status",
        "needs_reconciliation",
        "reserved_micro_cny",
        "account_debit_micro_cny",
        "created_at",
        "settled_at",
        "allowance_basis",
        "allowance_percentages",
    ] {
        out[field] = value[field].clone();
    }
    out
}
