//! Q5 projections of the existing per-send ledger. References never create costs.
use crate::{
    account_allowance::AllowanceBasis,
    admin_store::invalid,
    model_spend_store::{api_storage, ModelSpendStore},
};
use read_tools::ToolError;
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default, Serialize)]
struct Metrics {
    request_count: i64,
    confirmed_cost_micro_cny: i64,
    account_debit_micro_cny: i64,
    active_reserved_micro_cny: i64,
    active_requests: i64,
    pending_micro_cny: i64,
    pending_requests: i64,
    unknown_cost_requests: i64,
    reconciliation_requests: i64,
    platform_covered_micro_cny: i64,
}
impl Metrics {
    fn add(&mut self, other: &Self) -> Result<(), ToolError> {
        macro_rules! add { ($($field:ident),+) => { $(self.$field = self.$field.checked_add(other.$field).ok_or_else(invalid)?;)+ }; }
        add!(
            request_count,
            confirmed_cost_micro_cny,
            account_debit_micro_cny,
            active_reserved_micro_cny,
            active_requests,
            pending_micro_cny,
            pending_requests,
            unknown_cost_requests,
            reconciliation_requests,
            platform_covered_micro_cny
        );
        Ok(())
    }
    fn value(&self) -> Value {
        let mut value = json!(self);
        let complete = self.unknown_cost_requests == 0 && self.reconciliation_requests == 0;
        value["provider_cost_complete"] = json!(complete);
        value["provider_cost_micro_cny"] = if complete {
            json!(self.confirmed_cost_micro_cny)
        } else {
            Value::Null
        };
        value
    }
}

#[derive(Default)]
struct Summary {
    metrics: Metrics,
    periods: BTreeMap<(String, String), Metrics>,
}
struct PeriodProjection {
    basis: Option<AllowanceBasis>,
    starts_at: i64,
    expires_at: i64,
}
impl Summary {
    fn add(&mut self, row: &Partition) -> Result<(), ToolError> {
        self.metrics.add(&row.metrics)?;
        if let (Some(owner), Some(period)) = (&row.owner, &row.period) {
            self.periods
                .entry((owner.clone(), period.clone()))
                .or_default()
                .add(&row.metrics)?;
        }
        Ok(())
    }
    fn value(&self, bases: &BTreeMap<String, PeriodProjection>) -> Value {
        let mut value = self.metrics.value();
        value["period_groups"] = json!(self
            .periods
            .iter()
            .map(|((owner, period), metrics)| {
                let projection = bases.get(period);
                let basis = projection.and_then(|p|p.basis.as_ref());
                json!({"user_id":owner,"period_id":period,"allowance_basis":basis,
                "starts_at":projection.map(|p|p.starts_at),"expires_at":projection.map(|p|p.expires_at),
                "debited_micro_cny":metrics.account_debit_micro_cny,
                "active_reserved_micro_cny":metrics.active_reserved_micro_cny,
                "pending_micro_cny":metrics.pending_micro_cny,
                "unknown_provider_cost_count":metrics.unknown_cost_requests,
                "reconciliation_requests":metrics.reconciliation_requests,
                "percentages":basis.map(|b|json!({
                    "debited":b.percentage(metrics.account_debit_micro_cny),
                    "active_reserved":b.percentage(metrics.active_reserved_micro_cny),
                    "pending":b.percentage(metrics.pending_micro_cny)
                }))})
            })
            .collect::<Vec<_>>());
        value
    }
}

struct Partition {
    cost_owner: String,
    owner: Option<String>,
    parent: String,
    workspace: Option<String>,
    period: Option<String>,
    related: BTreeSet<String>,
    purposes: BTreeSet<String>,
    latest_at: i64,
    metrics: Metrics,
}
#[derive(Default)]
struct Task {
    summary: Summary,
    workspaces: BTreeSet<String>,
    related: BTreeSet<String>,
    purposes: BTreeSet<String>,
    latest_at: i64,
}

fn partitions(
    db: &Connection,
    owner: Option<&str>,
    range: Option<&crate::admin_usage::Range>,
) -> Result<Vec<Partition>, ToolError> {
    // SQL reduces actual sends to ownership/period partitions before any display rounding.
    let mut stmt = db.prepare("SELECT cost_owner,user_id,coalesce(parent_task_ref,run_ref,task_ref),learning_workspace_id,period_id,
        json_group_array(related_learning_workspace_ids),json_group_array(DISTINCT purpose),max(max(created_at,coalesce(settled_at,created_at))),
        count(*),coalesce(sum(provider_cost_micro_cny),0),coalesce(sum(account_debit_micro_cny),0),
        coalesce(sum(CASE WHEN state IN ('reserved','sent') THEN reserved_micro_cny ELSE 0 END),0),
        count(CASE WHEN state IN ('reserved','sent') THEN 1 END),
        coalesce(sum(CASE WHEN state='pending' THEN reserved_micro_cny ELSE 0 END),0),
        count(CASE WHEN state='pending' OR (state='released' AND provider_cost_status='pending') OR needs_reconciliation=1 THEN 1 END),
        count(CASE WHEN provider_cost_status='pending' THEN 1 END),count(CASE WHEN needs_reconciliation=1 THEN 1 END),
        coalesce(sum(CASE WHEN provider_cost_status='confirmed' THEN provider_cost_micro_cny-coalesce(account_debit_micro_cny,0) ELSE 0 END),0)
        FROM model_call_charges WHERE (?1 IS NULL OR user_id=?1) AND (?2 IS NULL OR (created_at>=?2 AND created_at<?3))
        GROUP BY cost_owner,user_id,coalesce(parent_task_ref,run_ref,task_ref),learning_workspace_id,period_id").map_err(api_storage)?;
    let rows = stmt
        .query_map(
            params![owner, range.map(|r| r.start), range.map(|r| r.end)],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, i64>(7)?,
                    Metrics {
                        request_count: r.get(8)?,
                        confirmed_cost_micro_cny: r.get(9)?,
                        account_debit_micro_cny: r.get(10)?,
                        active_reserved_micro_cny: r.get(11)?,
                        active_requests: r.get(12)?,
                        pending_micro_cny: r.get(13)?,
                        pending_requests: r.get(14)?,
                        unknown_cost_requests: r.get(15)?,
                        reconciliation_requests: r.get(16)?,
                        platform_covered_micro_cny: r.get(17)?,
                    },
                ))
            },
        )
        .map_err(api_storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(api_storage)?;
    rows.into_iter()
        .map(
            |(
                cost_owner,
                owner,
                parent,
                workspace,
                period,
                related,
                purposes,
                latest_at,
                metrics,
            )| {
                let related: Vec<String> = serde_json::from_str(&related).map_err(api_storage)?;
                let mut related_ids = BTreeSet::new();
                for group in related {
                    related_ids
                        .extend(serde_json::from_str::<Vec<String>>(&group).map_err(api_storage)?);
                }
                Ok(Partition {
                    cost_owner,
                    owner,
                    parent,
                    workspace,
                    period,
                    related: related_ids,
                    purposes: serde_json::from_str(&purposes).map_err(api_storage)?,
                    latest_at,
                    metrics,
                })
            },
        )
        .collect()
}

impl ModelSpendStore {
    pub(crate) fn cost_summary(
        &self,
        owner: Option<&str>,
        range: Option<&crate::admin_usage::Range>,
        limit: u32,
        offset: u32,
        reader: bool,
        now: i64,
    ) -> Result<Value, ToolError> {
        if reader && owner.is_none() {
            return Err(invalid());
        }
        let mut control = self.control.lock().unwrap();
        let tx = control.connection.transaction().map_err(api_storage)?;
        if let Some(owner) = owner {
            crate::user_storage_paths::validate_user_id(owner)?;
            let exists: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM users WHERE user_id=?)",
                    [owner],
                    |r| r.get(0),
                )
                .map_err(api_storage)?;
            if !exists {
                return Err(crate::authorization::missing());
            }
        }
        let rows = partitions(&tx, owner, range)?;
        let mut bases = BTreeMap::new();
        let mut account = Summary::default();
        let mut platform = Summary::default();
        let mut accounts: BTreeMap<String, Summary> = BTreeMap::new();
        let mut workspaces: BTreeMap<(String, String), Summary> = BTreeMap::new();
        let mut account_level: BTreeMap<String, Summary> = BTreeMap::new();
        let mut tasks: BTreeMap<(String, Option<String>, String), Task> = BTreeMap::new();
        for row in &rows {
            if let Some(period) = &row.period {
                if !bases.contains_key(period) {
                    let projection = tx.query_row("SELECT p.starts_at,p.expires_at,b.version,b.basis_micro_cny
                        FROM allowance_periods p LEFT JOIN allowance_period_bases pb ON pb.period_id=p.period_id
                        LEFT JOIN allowance_bases b ON b.version=pb.basis_version WHERE p.period_id=?", [period], |r| {
                            let version: Option<String> = r.get(2)?;
                            Ok(PeriodProjection {starts_at:r.get(0)?,expires_at:r.get(1)?,
                                basis:version.map(|version|r.get(3).map(|basis_micro_cny|AllowanceBasis {version,basis_micro_cny})).transpose()?})
                        }).map_err(api_storage)?;
                    bases.insert(period.clone(), projection);
                }
            }
            if let Some(owner) = &row.owner {
                account.add(row)?;
                accounts.entry(owner.clone()).or_default().add(row)?;
                if let Some(workspace) = &row.workspace {
                    workspaces
                        .entry((owner.clone(), workspace.clone()))
                        .or_default()
                        .add(row)?;
                } else {
                    account_level.entry(owner.clone()).or_default().add(row)?;
                }
            } else {
                platform.add(row)?;
            }
            let task = tasks
                .entry((
                    row.cost_owner.clone(),
                    row.owner.clone(),
                    row.parent.clone(),
                ))
                .or_default();
            task.summary.add(row)?;
            task.workspaces.extend(row.workspace.iter().cloned());
            task.related.extend(row.related.iter().cloned());
            task.purposes.extend(row.purposes.iter().cloned());
            task.latest_at = task.latest_at.max(row.latest_at);
        }
        // Goals are current display labels only. Cost ownership always comes from the charge.
        let mut labels = tx.prepare("SELECT w.owner_user_id,w.learning_workspace_id,w.archived,
            (SELECT goal_text FROM learning_workspace_goals g WHERE g.owner_user_id=w.owner_user_id AND g.learning_workspace_id=w.learning_workspace_id ORDER BY revision DESC LIMIT 1)
            FROM learning_workspaces w WHERE (?1 IS NULL OR w.owner_user_id=?1) AND EXISTS(SELECT 1 FROM model_call_charges c WHERE c.user_id=w.owner_user_id AND c.learning_workspace_id=w.learning_workspace_id)").map_err(api_storage)?;
        let labels = labels
            .query_map([owner], |r| {
                Ok((
                    (r.get::<_, String>(0)?, r.get::<_, String>(1)?),
                    (r.get::<_, bool>(2)?, r.get::<_, Option<String>>(3)?),
                ))
            })
            .map_err(api_storage)?
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map_err(api_storage)?;
        let with_owner = |owner: &str, summary: &Summary| {
            let mut value = summary.value(&bases);
            value["user_id"] = json!(owner);
            value
        };
        let workspace_values = workspaces
            .iter()
            .map(|((owner, workspace), summary)| {
                let mut value = with_owner(owner, summary);
                value["learning_workspace_id"] = json!(workspace);
                let label = labels.get(&(owner.clone(), workspace.clone()));
                value["workspace_goal"] = json!(label.and_then(|(_, goal)| goal.as_ref()));
                value["archived"] = json!(label.is_some_and(|(archived, _)| *archived));
                value
            })
            .collect::<Vec<_>>();
        let mut tasks = tasks.into_iter().collect::<Vec<_>>();
        tasks.sort_by(|(ak, a), (bk, b)| b.latest_at.cmp(&a.latest_at).then_with(|| ak.cmp(bk)));
        let total = tasks.len();
        let task_values = tasks
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .map(|((cost_owner, owner, parent), task)| {
                let mut value = task.summary.value(&bases);
                value["cost_owner"] = json!(cost_owner);
                value["user_id"] = json!(owner);
                value["parent_task_ref"] = json!(parent);
                value["learning_workspace_ids"] = json!(task.workspaces);
                value["related_learning_workspace_ids"] = json!(task.related);
                value["purposes"] = json!(task.purposes);
                value["latest_at"] = json!(task.latest_at);
                value
            })
            .collect::<Vec<_>>();
        Ok(
            json!({"timezone":"Asia/Hong_Kong","from":range.map(|r|&r.from),"to":range.map(|r|&r.to),"as_of":now,
            "account":account.value(&bases),"platform":if reader {Value::Null} else {platform.value(&bases)},
            "accounts":accounts.iter().map(|(owner,s)|with_owner(owner,s)).collect::<Vec<_>>(),
            "workspaces":workspace_values,"account_level":account_level.iter().map(|(owner,s)|with_owner(owner,s)).collect::<Vec<_>>(),
            "tasks":{"items":task_values,"total":total,"limit":limit,"offset":offset}}),
        )
    }
}
