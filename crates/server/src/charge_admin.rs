//! Append-only manual evidence; the charge row is the current balance projection.
use crate::{
    admin_store::invalid,
    model_spend_store::{api_storage, charge, ModelSpendStore},
    user_storage_paths::error,
};
use read_tools::ToolError;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
        } else {
            cost.ok_or_else(invalid)?
        };
        let state = if self.waive_account {
            "released"
        } else {
            "settled"
        };
        db.execute("UPDATE model_call_charges SET state=?,provider_cost_micro_cny=?,provider_cost_status=?,account_debit_micro_cny=?,evidence_kind='manual_reconciliation',settled_at=?,revision=revision+1,needs_reconciliation=0 WHERE call_id=?",params![state,cost,if cost.is_some(){"confirmed"}else{"pending"},debit,now,call]).map_err(api_storage)?;
        let after = charge(db, call)?;
        let delta = debit
            .checked_sub(before["account_debit_micro_cny"].as_i64().unwrap_or(0))
            .ok_or_else(invalid)?;
        let provider_delta = cost
            .zip(before["provider_cost_micro_cny"].as_i64())
            .map(|(a, b)| a - b);
        db.execute(
            "INSERT INTO charge_reconciliations VALUES(?,?,?,?,?,?,?,?,?,?,?)",
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
        Ok(json!({"current_allowance":current}))
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
    ) -> Result<Value, ToolError> {
        let mut control = self.control.lock().unwrap();
        let tx = control.connection.transaction().map_err(api_storage)?;
        let filter = "(?1 IS NULL OR user_id=?1) AND (?2=0 OR state='pending' OR (state='released' AND provider_cost_status='pending') OR needs_reconciliation=1) AND (?5 IS NULL OR (created_at>=?5 AND created_at<?6)) AND (?7 IS NULL OR (?7='run' AND run_ref=?8) OR (?7='task' AND run_ref IS NULL AND task_ref=?8))";
        let mut stmt = tx.prepare(&format!("SELECT call_id FROM model_call_charges WHERE {filter} ORDER BY created_at DESC,call_id DESC LIMIT ?3 OFFSET ?4")).map_err(api_storage)?;
        let ids = stmt
            .query_map(params![owner, pending, limit, offset, range.map(|r|r.start), range.map(|r|r.end), reference.map(|r|r.0), reference.map(|r|r.1)], |r| {
                r.get::<_, String>(0)
            })
            .map_err(api_storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(api_storage)?;
        let items = ids
            .iter()
            .map(|id| {
                let value = charge(&tx, id)?;
                Ok(if reader { reader_charge(value) } else { value })
            })
            .collect::<Result<Vec<_>, ToolError>>()?;
        let total: i64 = tx
            .query_row(
                &format!("SELECT count(*) FROM model_call_charges WHERE {filter}"),
                params![owner, pending, limit, offset, range.map(|r|r.start), range.map(|r|r.end), reference.map(|r|r.0), reference.map(|r|r.1)],
                |r| r.get(0),
            )
            .map_err(api_storage)?;
        Ok(json!({"items":items,"total":total,"limit":limit,"offset":offset}))
    }
    pub(crate) fn charge_detail(
        &self,
        call: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Value, ToolError> {
        let mut control = self.control.lock().unwrap();
        let tx = control.connection.transaction().map_err(api_storage)?;
        let mut value = charge(&tx, call)?;
        let mut stmt=tx.prepare("SELECT actor,operation_id,before_json,after_json,account_delta_micro_cny,provider_delta_micro_cny,reason,evidence,created_at FROM charge_reconciliations WHERE call_id=? ORDER BY created_at DESC,actor,operation_id LIMIT ? OFFSET ?").map_err(api_storage)?;
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
                "SELECT count(*) FROM charge_reconciliations WHERE call_id=?",
                [call],
                |r| r.get(0),
            )
            .map_err(api_storage)?;
        value["reconciliations"] =
            json!({"items":items,"total":total,"limit":limit,"offset":offset});
        let mut stmt=tx.prepare("SELECT outcome_json,disposition,received_at FROM charge_reports WHERE call_id=? ORDER BY received_at DESC,outcome_json LIMIT ? OFFSET ?").map_err(api_storage)?;
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
                "SELECT count(*) FROM charge_reports WHERE call_id=?",
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
        "purpose",
        "model",
        "state",
        "reserved_micro_cny",
        "account_debit_micro_cny",
        "created_at",
        "settled_at",
    ] {
        out[field] = value[field].clone();
    }
    out
}
