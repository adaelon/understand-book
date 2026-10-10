//! ADM3 business operations. The caller owns the immediate transaction and receipt.
use crate::{
    account_allowance::{MicroCny, ReceiptFen},
    admin_store::invalid,
    user_storage_paths::error,
};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Period {
    pub starts_at: i64,
    pub expires_at: i64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Adjustment {
    pub period_id: String,
    pub revision: i64,
    pub delta_micro_cny: MicroCny,
    pub reason: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Validity {
    pub period_id: String,
    pub revision: i64,
    pub starts_at: i64,
    pub expires_at: i64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Receipt {
    pub period_id: String,
    pub revision: i64,
    pub amount_fen: ReceiptFen,
    pub delta_micro_cny: MicroCny,
    pub paid_at: i64,
    pub channel: String,
    pub external_ref: Option<String>,
    pub note: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Correction {
    pub receipt_id: String,
    pub period_id: String,
    pub revision: i64,
    pub delta_fen: i64,
    pub delta_micro_cny: MicroCny,
    pub reason: String,
}
pub(crate) enum Mutation {
    Period(Period),
    Adjustment(Adjustment),
    Validity(Validity),
    Receipt(Receipt),
    Correction(Correction),
}
pub(crate) fn storage(_: rusqlite::Error) -> ToolError {
    error(
        "ADMIN_STORAGE_UNAVAILABLE",
        "unavailable",
        "Allowance data could not be read or committed",
    )
}
fn conflict(code: &str) -> ToolError {
    error(code, "conflict", code)
}
fn normalized(text: &mut String, required: bool) -> Result<(), ToolError> {
    *text = text.trim().to_owned();
    if required && text.is_empty() {
        return Err(invalid());
    }
    Ok(())
}
impl Mutation {
    pub fn normalize(&mut self) -> Result<(), ToolError> {
        match self {
            Self::Adjustment(a) => normalized(&mut a.reason, true)?,
            Self::Receipt(r) => {
                normalized(&mut r.channel, true)?;
                normalized(&mut r.note, false)?;
                if let Some(reference) = &mut r.external_ref {
                    normalized(reference, true)?;
                }
            }
            Self::Correction(c) => normalized(&mut c.reason, true)?,
            _ => (),
        }
        Ok(())
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Period(_) => "allowance-periods",
            Self::Adjustment(_) => "allowance-adjustments",
            Self::Validity(_) => "allowance-validity",
            Self::Receipt(_) => "receipts",
            Self::Correction(_) => "receipt-corrections",
        }
    }
    pub fn parameters(&self) -> Value {
        match self {
            Self::Period(v) => json!(v),
            Self::Adjustment(v) => json!(v),
            Self::Validity(v) => json!(v),
            Self::Receipt(v) => json!(v),
            Self::Correction(v) => json!(v),
        }
    }
    pub fn apply(
        &self,
        db: &Connection,
        actor: &str,
        op: &str,
        owner: &str,
        now: i64,
    ) -> Result<Value, ToolError> {
        let mut receipt_id = None;
        let period_id = match self {
            Self::Period(p) => {
                valid_dates(p.starts_at, p.expires_at)?;
                no_overlap(db, owner, "", p.starts_at, p.expires_at)?;
                let id = uuid::Uuid::now_v7().to_string();
                db.execute("INSERT INTO allowance_periods(period_id,user_id,starts_at,expires_at) VALUES(?,?,?,?)", params![id,owner,p.starts_at,p.expires_at]).map_err(storage)?;
                id
            }
            Self::Validity(p) => {
                valid_dates(p.starts_at, p.expires_at)?;
                revision(db, owner, &p.period_id, p.revision)?;
                no_overlap(db, owner, &p.period_id, p.starts_at, p.expires_at)?;
                db.execute(
                    "UPDATE allowance_periods SET starts_at=?,expires_at=? WHERE period_id=?",
                    params![p.starts_at, p.expires_at, p.period_id],
                )
                .map_err(storage)?;
                p.period_id.clone()
            }
            Self::Adjustment(a) => {
                revision(db, owner, &a.period_id, a.revision)?;
                adjust(
                    db,
                    actor,
                    op,
                    owner,
                    &a.period_id,
                    a.delta_micro_cny,
                    &a.reason,
                    None,
                    now,
                )?;
                a.period_id.clone()
            }
            Self::Receipt(r) => {
                if r.delta_micro_cny.value() <= 0 {
                    return Err(invalid());
                }
                revision(db, owner, &r.period_id, r.revision)?;
                if let Some(reference) = &r.external_ref {
                    let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM manual_receipts WHERE channel=? AND external_ref=?)", params![r.channel,reference], |r|r.get(0)).map_err(storage)?;
                    if exists {
                        return Err(conflict("RECEIPT_EXTERNAL_REF_CONFLICT"));
                    }
                }
                let id = uuid::Uuid::now_v7().to_string();
                db.execute("INSERT INTO manual_receipts(receipt_id,actor,operation_id,user_id,amount_fen,paid_at,channel,external_ref,note) VALUES(?,?,?,?,?,?,?,?,?)", params![id,actor,op,owner,r.amount_fen,r.paid_at,r.channel,r.external_ref,r.note]).map_err(storage)?;
                adjust(
                    db,
                    actor,
                    op,
                    owner,
                    &r.period_id,
                    r.delta_micro_cny,
                    "receipt grant",
                    Some(&id),
                    now,
                )?;
                receipt_id = Some(id);
                r.period_id.clone()
            }
            Self::Correction(c) => {
                if c.delta_fen == 0 {
                    return Err(invalid());
                }
                revision(db, owner, &c.period_id, c.revision)?;
                // The correction remains attached to the original grant's period.
                let original: Option<(i64,String)> = db.query_row("SELECT r.amount_fen,a.period_id FROM manual_receipts r JOIN allowance_adjustments a ON a.actor=r.actor AND a.operation_id=r.operation_id WHERE r.receipt_id=? AND r.user_id=?",params![c.receipt_id,owner],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(storage)?;
                let (original, period) = original.ok_or_else(crate::authorization::missing)?;
                if period != c.period_id {
                    return Err(invalid());
                }
                let corrections: i64 = db.query_row("SELECT COALESCE(sum(delta_fen),0) FROM receipt_corrections WHERE receipt_id=?", [&c.receipt_id], |r|r.get(0)).map_err(storage)?;
                let effective = original
                    .checked_add(corrections)
                    .and_then(|v| v.checked_add(c.delta_fen))
                    .ok_or_else(invalid)?;
                if effective < 0 {
                    return Err(conflict("RECEIPT_CORRECTION_EXCEEDS_AMOUNT"));
                }
                db.execute(
                    "INSERT INTO receipt_corrections VALUES(?,?,?,?,?,?,?)",
                    params![actor, op, owner, c.receipt_id, c.delta_fen, c.reason, now],
                )
                .map_err(storage)?;
                if c.delta_micro_cny != MicroCny::ZERO {
                    adjust(
                        db,
                        actor,
                        op,
                        owner,
                        &c.period_id,
                        c.delta_micro_cny,
                        &c.reason,
                        Some(&c.receipt_id),
                        now,
                    )?;
                }
                receipt_id = Some(c.receipt_id.clone());
                c.period_id.clone()
            }
        };
        Ok(json!({"allowance_period": period(db,owner,&period_id)?, "receipt_id":receipt_id}))
    }
}
fn valid_dates(start: i64, end: i64) -> Result<(), ToolError> {
    if start >= end {
        Err(invalid())
    } else {
        Ok(())
    }
}
fn no_overlap(
    db: &Connection,
    owner: &str,
    id: &str,
    start: i64,
    end: i64,
) -> Result<(), ToolError> {
    let overlaps: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM allowance_periods WHERE user_id=? AND period_id<>? AND closed_at IS NULL AND starts_at<? AND ?<expires_at)", params![owner,id,end,start],|r|r.get(0)).map_err(storage)?;
    if overlaps {
        Err(conflict("ALLOWANCE_PERIOD_OVERLAP"))
    } else {
        Ok(())
    }
}
fn revision(db: &Connection, owner: &str, id: &str, expected: i64) -> Result<(), ToolError> {
    let actual: Option<(i64, Option<i64>)> = db
        .query_row(
            "SELECT revision,closed_at FROM allowance_periods WHERE user_id=? AND period_id=?",
            params![owner, id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(storage)?;
    let (actual, closed) = actual.ok_or_else(crate::authorization::missing)?;
    if actual != expected {
        return Err(conflict("ALLOWANCE_REVISION_CONFLICT"));
    }
    if closed.is_some() {
        return Err(conflict("ALLOWANCE_PERIOD_CLOSED"));
    }
    let next = actual.checked_add(1).ok_or_else(invalid)?;
    db.execute(
        "UPDATE allowance_periods SET revision=? WHERE period_id=?",
        params![next, id],
    )
    .map_err(storage)?;
    Ok(())
}
fn adjust(
    db: &Connection,
    actor: &str,
    op: &str,
    owner: &str,
    period: &str,
    delta: MicroCny,
    reason: &str,
    receipt: Option<&str>,
    now: i64,
) -> Result<(), ToolError> {
    if delta == MicroCny::ZERO || reason.trim().is_empty() {
        return Err(invalid());
    }
    let balance = crate::account_allowance::balance(db, period)?;
    let available = balance
        .available_micro_cny
        .checked_add(delta)
        .ok_or_else(invalid)?;
    balance
        .granted_micro_cny
        .checked_add(delta)
        .ok_or_else(invalid)?;
    if delta.value() < 0 && available.value() < 0 {
        return Err(conflict("ALLOWANCE_ALREADY_COMMITTED"));
    }
    db.execute(
        "INSERT INTO allowance_adjustments VALUES(?,?,?,?,?,?,?,?)",
        params![actor, op, owner, period, delta, reason, receipt, now],
    )
    .map_err(storage)?;
    Ok(())
}
pub(crate) fn period(db: &Connection, owner: &str, id: &str) -> Result<Value, ToolError> {
    let mut value = db.query_row("SELECT period_id,starts_at,expires_at,closed_at,revision FROM allowance_periods WHERE user_id=? AND period_id=?",params![owner,id],|r|Ok(json!({"period_id":r.get::<_,String>(0)?,"starts_at":r.get::<_,i64>(1)?,"expires_at":r.get::<_,i64>(2)?,"closed_at":r.get::<_,Option<i64>>(3)?,"revision":r.get::<_,i64>(4)?}))).optional().map_err(storage)?.ok_or_else(crate::authorization::missing)?;
    value["balance"] = json!(crate::account_allowance::balance(db, id)?);
    Ok(value)
}
pub(crate) fn current(db: &Connection, owner: &str, now: i64) -> Result<Value, ToolError> {
    let id: Option<String> = db.query_row("SELECT period_id FROM allowance_periods WHERE user_id=? AND closed_at IS NULL AND starts_at<=? AND ?<expires_at",params![owner,now,now],|r|r.get(0)).optional().map_err(storage)?;
    id.map(|id| period(db, owner, &id))
        .unwrap_or(Ok(Value::Null))
}
pub(crate) fn list(
    db: &Connection,
    owner: &str,
    kind: &str,
    limit: u32,
    offset: u32,
) -> Result<Value, ToolError> {
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE user_id=?)",
            [owner],
            |r| r.get(0),
        )
        .map_err(storage)?;
    if !exists {
        return Err(crate::authorization::missing());
    }
    if matches!(kind, "allowance-adjustments" | "receipt-corrections") {
        let (sql, count) = if kind == "allowance-adjustments" {
            ("SELECT actor,operation_id,period_id,delta_micro_cny,reason,receipt_id,created_at FROM allowance_adjustments WHERE user_id=? ORDER BY created_at DESC,actor,operation_id LIMIT ? OFFSET ?", "SELECT count(*) FROM allowance_adjustments WHERE user_id=?")
        } else {
            ("SELECT actor,operation_id,receipt_id,delta_fen,reason,created_at FROM receipt_corrections WHERE user_id=? ORDER BY created_at DESC,actor,operation_id LIMIT ? OFFSET ?", "SELECT count(*) FROM receipt_corrections WHERE user_id=?")
        };
        let mut stmt = db.prepare(sql).map_err(storage)?;
        let items=stmt.query_map(params![owner,limit,offset],|r|Ok(if kind=="allowance-adjustments" {
            json!({"actor":r.get::<_,String>(0)?,"operation_id":r.get::<_,String>(1)?,"period_id":r.get::<_,String>(2)?,"delta_micro_cny":r.get::<_,i64>(3)?,"reason":r.get::<_,String>(4)?,"receipt_id":r.get::<_,Option<String>>(5)?,"created_at":r.get::<_,i64>(6)?})
        } else {
            json!({"actor":r.get::<_,String>(0)?,"operation_id":r.get::<_,String>(1)?,"receipt_id":r.get::<_,String>(2)?,"delta_fen":r.get::<_,i64>(3)?,"reason":r.get::<_,String>(4)?,"created_at":r.get::<_,i64>(5)?})
        })).map_err(storage)?.collect::<Result<Vec<_>,_>>().map_err(storage)?;
        let total: i64 = db
            .query_row(count, [owner], |r| r.get(0))
            .map_err(storage)?;
        return Ok(json!({"items":items,"total":total,"limit":limit,"offset":offset}));
    }
    let (sql, count) = match kind {
        "allowance-periods" => ("SELECT period_id FROM allowance_periods WHERE user_id=? ORDER BY starts_at DESC,period_id LIMIT ? OFFSET ?", "SELECT count(*) FROM allowance_periods WHERE user_id=?"),
        "receipts" => ("SELECT receipt_id FROM manual_receipts WHERE user_id=? ORDER BY paid_at DESC,receipt_id LIMIT ? OFFSET ?", "SELECT count(*) FROM manual_receipts WHERE user_id=?"),
        _ => return Err(invalid()),
    };
    let mut stmt = db.prepare(sql).map_err(storage)?;
    let ids = stmt
        .query_map(params![owner, limit, offset], |r| r.get::<_, String>(0))
        .map_err(storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage)?;
    let items = ids.iter().map(|id| if kind == "allowance-periods" { period(db,owner,id) } else {
        db.query_row("SELECT r.receipt_id,r.amount_fen,r.paid_at,r.channel,r.external_ref,r.note,r.amount_fen+COALESCE((SELECT sum(delta_fen) FROM receipt_corrections c WHERE c.receipt_id=r.receipt_id),0),(SELECT a.period_id FROM allowance_adjustments a WHERE a.receipt_id=r.receipt_id LIMIT 1) FROM manual_receipts r WHERE r.receipt_id=?",[id],|r|Ok(json!({"receipt_id":r.get::<_,String>(0)?,"amount_fen":r.get::<_,i64>(1)?,"paid_at":r.get::<_,i64>(2)?,"channel":r.get::<_,String>(3)?,"external_ref":r.get::<_,Option<String>>(4)?,"note":r.get::<_,String>(5)?,"effective_amount_fen":r.get::<_,i64>(6)?,"period_id":r.get::<_,String>(7)?}))).map_err(storage)
    }).collect::<Result<Vec<_>,_>>()?;
    let total: i64 = db
        .query_row(count, [owner], |r| r.get(0))
        .map_err(storage)?;
    Ok(json!({"items":items,"total":total,"limit":limit,"offset":offset}))
}
