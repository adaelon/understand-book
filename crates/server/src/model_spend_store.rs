//! Short SQLite transactions around each transport. Neither the request body nor
//! a database lock survives into Provider I/O or private-history persistence.
use crate::{
    control_store::ControlStore, model_rates::ModelRates, published_library::PublishedBookRef,
};
use runtime::model_spend::{
    ChargeScope, ModelSpendPort, SendEvidence, SendIdentity, SendOutcome, SpendStop,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

fn storage<T>(_: T) -> SpendStop {
    SpendStop::StorageUnavailable
}
pub(crate) fn api_storage<T>(_: T) -> read_tools::ToolError {
    crate::user_storage_paths::error(
        SpendStop::StorageUnavailable.code(),
        "unavailable",
        "Model charge storage is unavailable",
    )
}

pub(crate) struct ModelSpendStore {
    pub(crate) control: Mutex<ControlStore>,
    rates: Result<ModelRates, SpendStop>,
    ready: AtomicBool,
}
impl ModelSpendStore {
    pub(crate) fn new(control: ControlStore) -> Self {
        let rates = ModelRates::load(&control.writer.root().join("model-rates.json"));
        let store = Self {
            control: Mutex::new(control),
            rates,
            ready: AtomicBool::new(false),
        };
        if let Err(e) = store.recover() {
            eprintln!("Model charge recovery: {}", e.code());
        }
        store
    }
    pub(crate) fn validate_provider(&self, provider: &str, model: &str) -> Result<(), SpendStop> {
        self.rates
            .as_ref()
            .map_err(|e| *e)?
            .select(provider, model, crate::multi_user_host::now())
            .map(|_| ())
    }
    /// Called before workers start, never as a repair while requests are in flight.
    pub(crate) fn recover(&self) -> Result<(), SpendStop> {
        self.ready.store(false, Ordering::Release);
        let mut control = self.control.lock().unwrap();
        let tx = control
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        tx.execute("UPDATE model_call_charges SET state='released',provider_cost_status='confirmed',provider_cost_micro_cny=0,account_debit_micro_cny=0,settled_at=?,evidence_kind='recovery_not_sent',revision=revision+1 WHERE state='reserved'", [crate::multi_user_host::now()]).map_err(storage)?;
        tx.execute("UPDATE model_call_charges SET state='pending',evidence_kind='recovery_outcome_unknown',revision=revision+1 WHERE state='sent'", []).map_err(storage)?;
        tx.commit().map_err(storage)?;
        self.ready.store(true, Ordering::Release);
        Ok(())
    }
    pub(crate) fn port(
        self: &Arc<Self>,
        scope: ChargeScope,
        publication: PublishedBookRef,
        cancellation: runtime::run_context::CancellationToken,
    ) -> Arc<dyn ModelSpendPort> {
        Arc::new(RunSpendPort {
            store: self.clone(),
            scope,
            publication,
            cancellation,
        })
    }
    fn before(
        &self,
        id: &SendIdentity,
        request: &Value,
        publication: &PublishedBookRef,
    ) -> Result<(), SpendStop> {
        if !self.ready.load(Ordering::Acquire) {
            return Err(SpendStop::StorageUnavailable);
        }
        let (owner, run, task) = attribution(id)?;
        let now = crate::multi_user_host::now();
        let rate = self
            .rates
            .as_ref()
            .map_err(|e| *e)?
            .select(&id.provider, &id.model, now)?;
        let estimate = rate.estimate(request)?;
        let mut control = self.control.lock().unwrap();
        {
            let tx = control
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(storage)?;
            authorize(&tx, owner, publication)?;
            let period: Option<String> = tx.query_row("SELECT period_id FROM allowance_periods WHERE user_id=? AND closed_at IS NULL AND starts_at<=? AND ?<expires_at", params![owner,now,now], |r|r.get(0)).optional().map_err(storage)?;
            let period = period.ok_or(SpendStop::AllowanceExpired)?;
            let balance = crate::account_allowance::balance(&tx, &period).map_err(storage)?;
            if balance.available_micro_cny < estimate.reserved_micro_cny {
                return Err(SpendStop::InsufficientAllowance);
            }
            // Reusing a send identity never authorizes another physical send.
            tx.execute("INSERT INTO model_call_charges(call_id,logical_call_id,attempt,user_id,period_id,run_ref,task_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_cost_status,evidence_kind,created_at,send_identity) VALUES(?,?,?,?,?,?,?,?,?,?,?,'reserved',?,'pending','reserved',?,?)", params![id.call_id,id.logical_call_id,id.attempt,owner,period,run,task,id.purpose,id.model,json!(rate).to_string(),json!(estimate).to_string(),estimate.reserved_micro_cny,now,json!(id).to_string()]).map_err(storage)?;
            tx.commit().map_err(storage)?;
        }
        // A crash between these commits is provably not sent. A committed sent
        // marker remains uncertain even if the process dies before socket I/O.
        let tx = control
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let allowed = authorize(&tx, owner, publication);
        let current: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM model_call_charges c JOIN allowance_periods p USING(period_id) WHERE c.call_id=? AND p.closed_at IS NULL AND p.starts_at<=? AND ?<p.expires_at)",params![id.call_id,crate::multi_user_host::now(),crate::multi_user_host::now()],|r|r.get(0)).map_err(storage)?;
        if let Err(reason) = allowed.and(if current {
            Ok(())
        } else {
            Err(SpendStop::AllowanceExpired)
        }) {
            tx.execute("UPDATE model_call_charges SET state='released',provider_cost_status='confirmed',provider_cost_micro_cny=0,account_debit_micro_cny=0,settled_at=?,evidence_kind='admission_not_sent',revision=revision+1 WHERE call_id=?",params![crate::multi_user_host::now(),id.call_id]).map_err(storage)?;
            tx.commit().map_err(storage)?;
            return Err(reason);
        }
        tx.execute("UPDATE model_call_charges SET state='sent',evidence_kind='sent',revision=revision+1 WHERE call_id=? AND state='reserved'",[&id.call_id]).map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(())
    }
    fn after(&self, id: &SendIdentity, outcome: &SendOutcome) -> Result<(), SpendStop> {
        let result = self.settle(id, outcome);
        if result == Err(SpendStop::StorageUnavailable) {
            self.ready.store(false, Ordering::Release);
        }
        result
    }
    fn settle(&self, id: &SendIdentity, outcome: &SendOutcome) -> Result<(), SpendStop> {
        let mut control = self.control.lock().unwrap();
        let tx = control
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let charge = charge(&tx, &id.call_id).map_err(storage)?;
        if charge["send_identity"] != json!(id) {
            return Err(SpendStop::MissingScope);
        }
        let report = json!(outcome).to_string();
        let previous: Option<String> = tx
            .query_row(
                "SELECT disposition FROM charge_reports WHERE call_id=? AND outcome_json=?",
                params![id.call_id, report],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        if let Some(previous) = previous {
            return if previous == "accepted" {
                Ok(())
            } else {
                Err(SpendStop::ReconciliationRequired)
            };
        }
        let now = crate::multi_user_host::now();
        if matches!(charge["state"].as_str(), Some("settled" | "released")) {
            tx.execute(
                "INSERT INTO charge_reports VALUES(?,?,'conflict',?)",
                params![id.call_id, report, now],
            )
            .map_err(storage)?;
            tx.execute("UPDATE model_call_charges SET needs_reconciliation=1,revision=revision+1 WHERE call_id=?",[&id.call_id]).map_err(storage)?;
            tx.commit().map_err(storage)?;
            return Err(SpendStop::ReconciliationRequired);
        }
        let rate: crate::model_rates::ModelRateSnapshot =
            serde_json::from_value(charge["rate_snapshot"].clone()).map_err(storage)?;
        let (state, cost, debit, evidence) = if let Some(usage) = &outcome.usage {
            match rate.cost(usage) {
                Ok(amount) => (
                    "settled",
                    Some(amount.value()),
                    Some(amount.value()),
                    "provider_usage".to_owned(),
                ),
                Err(reason) => (
                    "pending",
                    None,
                    None,
                    format!("usage_unknown:{}", json!(reason).as_str().unwrap()),
                ),
            }
        } else if outcome.evidence == SendEvidence::NotSent {
            (
                "released",
                Some(0),
                Some(0),
                "transport_not_sent".to_owned(),
            )
        } else {
            ("pending", None, None, "usage_missing".to_owned())
        };
        tx.execute("UPDATE model_call_charges SET state=?,provider_usage=?,provider_cost_micro_cny=?,provider_cost_status=?,account_debit_micro_cny=?,evidence_kind=?,settled_at=?,send_outcome=?,revision=revision+1 WHERE call_id=?",params![state,outcome.usage.as_ref().map(|u|json!(u).to_string()),cost,if cost.is_some(){"confirmed"}else{"pending"},debit,evidence,debit.map(|_|now),report,id.call_id]).map_err(storage)?;
        tx.execute(
            "INSERT INTO charge_reports VALUES(?,?,'accepted',?)",
            params![id.call_id, report, now],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(())
    }
}

fn attribution(id: &SendIdentity) -> Result<(&str, Option<&str>, Option<&str>), SpendStop> {
    if id.scope.as_ref().is_none_or(|s| !s.valid()) {
        return Err(SpendStop::MissingScope);
    }
    match id.scope.as_ref().unwrap() {
        ChargeScope::ReaderRun { user_id, run_ref } => Ok((user_id, Some(run_ref), None)),
        ChargeScope::ReaderTask { user_id, task_ref } => Ok((user_id, None, Some(task_ref))),
        // Offline operator work does not use the reader allowance store.
        ChargeScope::OperatorTask { .. } => Err(SpendStop::MissingScope),
    }
}
fn authorize(
    db: &Connection,
    owner: &str,
    publication: &PublishedBookRef,
) -> Result<(), SpendStop> {
    let allowed: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM book_grants g JOIN users u ON u.user_id=g.owner_user_id WHERE u.disabled=0 AND g.owner_user_id=? AND g.book_id=? AND g.publication_id=?)",params![owner,publication.book_id,publication.publication_id],|r|r.get(0)).map_err(storage)?;
    if allowed {
        Ok(())
    } else {
        Err(SpendStop::PermissionRevoked)
    }
}
struct RunSpendPort {
    store: Arc<ModelSpendStore>,
    scope: ChargeScope,
    publication: PublishedBookRef,
    cancellation: runtime::run_context::CancellationToken,
}
impl ModelSpendPort for RunSpendPort {
    fn before_send(&self, id: &SendIdentity, request: &Value) -> Result<(), SpendStop> {
        if id.scope.as_ref() != Some(&self.scope) {
            return Err(SpendStop::MissingScope);
        }
        self.cancellation
            .check()
            .map_err(|_| SpendStop::PermissionRevoked)?;
        let result = self.store.before(id, request, &self.publication);
        if result == Err(SpendStop::StorageUnavailable) {
            self.store.ready.store(false, Ordering::Release);
        }
        result
    }
    fn after_send(&self, id: &SendIdentity, outcome: &SendOutcome) -> Result<(), SpendStop> {
        if id.scope.as_ref() != Some(&self.scope) {
            return Err(SpendStop::MissingScope);
        }
        self.store.after(id, outcome)
    }
}

/// The stored JSON columns contain billing metadata only, never the final request.
pub(crate) fn charge(db: &Connection, id: &str) -> Result<Value, read_tools::ToolError> {
    let value = db.query_row("SELECT call_id,logical_call_id,attempt,user_id,period_id,run_ref,task_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_usage,provider_cost_micro_cny,provider_cost_status,account_debit_micro_cny,evidence_kind,created_at,settled_at,revision,send_identity,send_outcome,needs_reconciliation FROM model_call_charges WHERE call_id=?",[id],|r| {
        Ok(json!({
            "call_id":r.get::<_,String>(0)?,"logical_call_id":r.get::<_,String>(1)?,"attempt":r.get::<_,i64>(2)?,
            "user_id":r.get::<_,String>(3)?,"period_id":r.get::<_,String>(4)?,"run_ref":r.get::<_,Option<String>>(5)?,"task_ref":r.get::<_,Option<String>>(6)?,
            "purpose":r.get::<_,String>(7)?,"model":r.get::<_,String>(8)?,"rate_snapshot":r.get::<_,String>(9)?,"reservation_estimate":r.get::<_,String>(10)?,
            "state":r.get::<_,String>(11)?,"reserved_micro_cny":r.get::<_,i64>(12)?,"provider_usage":r.get::<_,Option<String>>(13)?,
            "provider_cost_micro_cny":r.get::<_,Option<i64>>(14)?,"provider_cost_status":r.get::<_,String>(15)?,"account_debit_micro_cny":r.get::<_,Option<i64>>(16)?,
            "evidence_kind":r.get::<_,String>(17)?,"created_at":r.get::<_,i64>(18)?,"settled_at":r.get::<_,Option<i64>>(19)?,"revision":r.get::<_,i64>(20)?,
            "send_identity":r.get::<_,Option<String>>(21)?,"send_outcome":r.get::<_,Option<String>>(22)?,"needs_reconciliation":r.get::<_,bool>(23)?
        }))
    }).optional().map_err(api_storage)?;
    let mut value = value.ok_or_else(crate::authorization::missing)?;
    for field in [
        "rate_snapshot",
        "reservation_estimate",
        "provider_usage",
        "send_identity",
        "send_outcome",
    ] {
        if let Some(s) = value[field].as_str() {
            value[field] = serde_json::from_str(s).map_err(api_storage)?;
        }
    }
    Ok(value)
}
