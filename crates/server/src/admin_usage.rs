//! Read-only reports rebuilt from minimal usage facts and current charge/receipt projections.
use crate::{
    admin_store::{invalid, AdminStore},
    model_spend_store::api_storage,
};
use read_tools::ToolError;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::HashMap;

pub(crate) fn record(
    db: &Connection,
    owner: &str,
    kind: &str,
    reference: &str,
    at: i64,
) -> Result<(), ToolError> {
    db.execute("INSERT INTO admin_usage_events(user_id,kind,event_ref,occurred_at) VALUES(?,?,?,?) ON CONFLICT(user_id,kind,event_ref) DO NOTHING", params![owner,kind,reference,at]).map_err(api_storage)?;
    Ok(())
}

/// Matches the existing Goal delivery predicate and saved turn diagnostics.
pub(crate) fn delivered(session: &crate::AgentChatSession, turn: &crate::AgentChatTurn) -> bool {
    if turn.status != crate::AgentAssistantStatus::Completed
        || turn.error.is_some()
        || turn
            .delivery_diagnostics
            .as_ref()
            .and_then(|d| d.repair.as_ref())
            .is_some_and(|r| !r.issues.is_empty())
    {
        return false;
    }
    let Some(outcome) = &turn.outcome else {
        return false;
    };
    if let Some(goal) = turn
        .goal_ref
        .as_ref()
        .and_then(|r| session.goals.iter().find(|g| g.id == r.id))
    {
        crate::goal_turn_delivered(goal, outcome)
    } else {
        !outcome.incomplete
            && outcome.warning.is_none()
            && outcome.answer.is_some()
            && outcome.answer_view.is_some()
    }
}

pub(crate) fn account_activity(db: &Connection, owner: &str) -> Result<Value, ToolError> {
    db.query_row("SELECT min(CASE WHEN kind IN ('read','question') THEN occurred_at END),max(CASE WHEN kind='read' THEN occurred_at END),max(CASE WHEN kind='question' THEN occurred_at END),count(DISTINCT CASE WHEN kind IN ('read','question') THEN (occurred_at+28800)/86400 END),count(CASE WHEN kind='completed' THEN 1 END) FROM admin_usage_events WHERE user_id=?", [owner], |r| {
        let days: i64 = r.get(3)?;
        Ok(json!({"first_used_at":r.get::<_,Option<i64>>(0)?,"last_read_at":r.get::<_,Option<i64>>(1)?,"last_question_at":r.get::<_,Option<i64>>(2)?,"active_days":days,"return_days":(days-1).max(0),"completed_runs":r.get::<_,i64>(4)?}))
    }).map_err(api_storage)
}

pub(crate) struct Range {
    pub start: i64,
    pub end: i64,
    pub from: String,
    pub to: String,
}
pub(crate) fn range(query: &mut HashMap<String, String>, now: i64) -> Result<Range, ToolError> {
    let today = day_label(now)?;
    let from = query.remove("from").unwrap_or_else(|| today.clone());
    let to = query.remove("to").unwrap_or(today);
    let parse = |s: &str| -> Result<i64, ToolError> {
        let fmt = time::format_description::parse_borrowed::<1>("[year]-[month]-[day]").unwrap();
        let date = time::Date::parse(s, &fmt).map_err(|_| invalid())?;
        let start = date
            .midnight()
            .assume_offset(time::UtcOffset::from_hms(8, 0, 0).unwrap())
            .unix_timestamp();
        if start < 0 {
            return Err(invalid());
        }
        Ok(start)
    };
    let start = parse(&from)?;
    let end = parse(&to)? + 86400;
    if start >= end {
        return Err(invalid());
    }
    Ok(Range {
        start,
        end,
        from,
        to,
    })
}
fn day_label(now: i64) -> Result<String, ToolError> {
    let date = time::OffsetDateTime::from_unix_timestamp(now)
        .map_err(|_| invalid())?
        .to_offset(time::UtcOffset::from_hms(8, 0, 0).unwrap())
        .date();
    Ok(date.to_string())
}

const METRICS: &str = "count(*) AS request_count,
 coalesce(sum(provider_cost_micro_cny),0) AS confirmed_cost_micro_cny,
 coalesce(sum(account_debit_micro_cny),0) AS account_debit_micro_cny,
 coalesce(sum(CASE WHEN state IN ('reserved','sent') THEN reserved_micro_cny ELSE 0 END),0) AS active_reserved_micro_cny,
 count(CASE WHEN state IN ('reserved','sent') THEN 1 END) AS active_requests,
 coalesce(sum(CASE WHEN state='pending' THEN reserved_micro_cny ELSE 0 END),0) AS pending_micro_cny,
 count(CASE WHEN state='pending' OR (state='released' AND provider_cost_status='pending') OR needs_reconciliation=1 THEN 1 END) AS pending_requests,
 count(CASE WHEN provider_cost_status='pending' THEN 1 END) AS unknown_cost_requests";
fn metrics(r: &rusqlite::Row<'_>, offset: usize) -> rusqlite::Result<Value> {
    let mut value = json!({});
    for (i, key) in [
        "request_count",
        "confirmed_cost_micro_cny",
        "account_debit_micro_cny",
        "active_reserved_micro_cny",
        "active_requests",
        "pending_micro_cny",
        "pending_requests",
        "unknown_cost_requests",
    ]
    .iter()
    .enumerate()
    {
        value[key] = json!(r.get::<_, i64>(offset + i)?);
    }
    Ok(value)
}
const FILTER: &str = "(?1 IS NULL OR user_id=?1) AND created_at>=?2 AND created_at<?3";

fn summary(db: &Connection, owner: Option<&str>, start: i64, end: i64) -> Result<Value, ToolError> {
    let mut value = db
        .query_row(
            &format!("SELECT {METRICS} FROM model_call_charges WHERE {FILTER}"),
            params![owner, start, end],
            |r| metrics(r, 0),
        )
        .map_err(api_storage)?;
    let (receipts,corrections):(i64,i64) = db.query_row("SELECT coalesce(sum(amount_fen),0),coalesce(sum((SELECT coalesce(sum(delta_fen),0) FROM receipt_corrections c WHERE c.receipt_id=r.receipt_id)),0) FROM manual_receipts r WHERE (?1 IS NULL OR user_id=?1) AND paid_at>=?2 AND paid_at<?3", params![owner,start,end], |r|Ok((r.get(0)?,r.get(1)?))).map_err(api_storage)?;
    value["receipt_original_fen"] = json!(receipts);
    value["receipt_correction_fen"] = json!(corrections);
    value["receipt_net_fen"] = json!(receipts.checked_add(corrections).ok_or_else(invalid)?);
    let (active, returning, completed):(i64,i64,i64) = db.query_row("SELECT
        count(DISTINCT CASE WHEN kind IN ('read','question') THEN user_id END),
        count(DISTINCT CASE WHEN kind IN ('read','question') AND EXISTS(SELECT 1 FROM admin_usage_events p WHERE p.user_id=e.user_id AND p.kind IN ('read','question') AND (p.occurred_at+28800)/86400 < (e.occurred_at+28800)/86400) THEN user_id END),
        count(CASE WHEN kind='completed' THEN 1 END)
        FROM admin_usage_events e WHERE (?1 IS NULL OR user_id=?1) AND occurred_at>=?2 AND occurred_at<?3",params![owner,start,end],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(api_storage)?;
    value["active_users"] = json!(active);
    value["returning_users"] = json!(returning);
    value["completed_runs"] = json!(completed);
    Ok(value)
}

impl AdminStore {
    pub(crate) fn record_read(&self, owner: &str, now: i64) -> Result<(), ToolError> {
        record(
            &self.control.lock().unwrap().connection,
            owner,
            "read",
            &uuid::Uuid::now_v7().to_string(),
            now,
        )
    }
    pub(crate) fn usage(
        &self,
        owner: Option<&str>,
        range: &Range,
        limit: u32,
        offset: u32,
        now: i64,
    ) -> Result<Value, ToolError> {
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
        let today_start = (now + 28800).div_euclid(86400) * 86400 - 28800;
        let today = summary(&tx, owner, today_start, today_start + 86400)?;
        let selected = summary(&tx, owner, range.start, range.end)?;
        let grouping = "user_id,CASE WHEN run_ref IS NOT NULL THEN 'run' ELSE 'task' END,coalesce(run_ref,task_ref)";
        let mut stmt = tx.prepare(&format!("SELECT {grouping},{METRICS} FROM model_call_charges WHERE {FILTER} GROUP BY {grouping} ORDER BY confirmed_cost_micro_cny DESC,account_debit_micro_cny DESC,user_id,2,3 LIMIT ?4 OFFSET ?5")).map_err(api_storage)?;
        let items = stmt
            .query_map(params![owner, range.start, range.end, limit, offset], |r| {
                let mut value = metrics(r, 3)?;
                value["user_id"] = json!(r.get::<_, String>(0)?);
                value["reference_kind"] = json!(r.get::<_, String>(1)?);
                value["reference"] = json!(r.get::<_, String>(2)?);
                Ok(value)
            })
            .map_err(api_storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(api_storage)?;
        let total:i64=tx.query_row(&format!("SELECT count(*) FROM (SELECT 1 FROM model_call_charges WHERE {FILTER} GROUP BY {grouping})"),params![owner,range.start,range.end],|r|r.get(0)).map_err(api_storage)?;
        Ok(
            json!({"timezone":"Asia/Hong_Kong","from":range.from,"to":range.to,"today_date":day_label(now)?,"as_of":now,"today":today,"summary":selected,"tasks":{"items":items,"total":total,"limit":limit,"offset":offset}}),
        )
    }
}
