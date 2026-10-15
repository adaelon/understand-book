//! Account-month call credits. Content preparation happens before the debit transaction.
use crate::{
    admin_store, auth::Principal, control_store::ControlStore, published_library::PublishedBookRef,
    user_storage_paths::error,
};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use time::{format_description::well_known::Rfc3339, Date, Month, OffsetDateTime, UtcOffset};

pub const MONTHLY_FREE_UNITS: i64 = 1_000;

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct Usage {
    pub month_hk: String,
    pub free_granted: i64,
    pub free_used: i64,
    pub free_remaining: i64,
    pub free_resets_at: String,
    pub paid_remaining: i64,
    pub paid_next_expires_at: Option<String>,
    pub call_packs: Vec<crate::mcp_pack::CallPack>,
}

/// The debit result explicitly identifies the month or purchased batch.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DebitSource {
    FreeMonth { month_hk: String },
    CallPack { pack_id: String },
}

#[derive(Debug, Serialize)]
pub struct Consumption {
    pub call_id: String,
    pub charged_units: i64,
    pub charged_from: DebitSource,
    pub completed_at: i64,
    pub usage: Usage,
}

fn storage() -> ToolError {
    error(
        "MCP_USAGE_STORAGE_UNAVAILABLE",
        "unavailable",
        "MCP usage could not be read or committed",
    )
}
fn invalid() -> ToolError {
    error(
        "MCP_USAGE_INPUT_INVALID",
        "validation",
        "Use a content tool and publication, or positive gift units with a reason",
    )
}

struct FreeMonth {
    key: String,
    resets_at: String,
}
impl FreeMonth {
    fn at(now: i64) -> Result<Self, ToolError> {
        let offset = UtcOffset::from_hms(8, 0, 0).unwrap();
        let local = OffsetDateTime::from_unix_timestamp(now)
            .map_err(|_| storage())?
            .to_offset(offset);
        let (year, month) = if local.month() == Month::December {
            (local.year() + 1, Month::January)
        } else {
            (local.year(), local.month().next())
        };
        let next = Date::from_calendar_date(year, month, 1)
            .map_err(|_| storage())?
            .midnight()
            .assume_offset(offset);
        Ok(Self {
            key: format!("{:04}-{:02}", local.year(), local.month() as u8),
            resets_at: next.format(&Rfc3339).map_err(|_| storage())?,
        })
    }
}

fn enabled(db: &Connection, owner: &str) -> Result<(), ToolError> {
    let enabled: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE user_id=? AND disabled=0)",
            [owner],
            |r| r.get(0),
        )
        .map_err(|_| storage())?;
    if enabled {
        Ok(())
    } else {
        Err(error(
            "USER_UNAVAILABLE",
            "not_found",
            "User is unavailable",
        ))
    }
}
fn ensure_month(db: &Connection, owner: &str, month: &FreeMonth) -> Result<(), ToolError> {
    db.execute(
        "INSERT INTO mcp_free_months(user_id,month_hk,granted_units) VALUES(?,?,?) ON CONFLICT(user_id,month_hk) DO NOTHING",
        params![owner, month.key, MONTHLY_FREE_UNITS],
    ).map_err(|_| storage())?;
    Ok(())
}
fn balance(db: &Connection, owner: &str, month: &FreeMonth, now: i64) -> Result<Usage, ToolError> {
    let call_packs = crate::mcp_pack::packs(db, owner, now)?;
    let paid_remaining = call_packs
        .iter()
        .try_fold(0_i64, |sum, p| sum.checked_add(p.available_units))
        .ok_or_else(storage)?;
    let paid_next_expires_at = call_packs
        .iter()
        .find(|p| p.available_units > 0)
        .map(|p| p.expires_at.clone());
    db.query_row(
        "SELECT granted_units,used_units FROM mcp_free_months WHERE user_id=? AND month_hk=?",
        params![owner, month.key],
        |r| {
            let granted: i64 = r.get(0)?;
            let used: i64 = r.get(1)?;
            Ok(Usage {
                month_hk: month.key.clone(),
                free_granted: granted,
                free_used: used,
                free_remaining: granted - used,
                free_resets_at: month.resets_at.clone(),
                paid_remaining,
                paid_next_expires_at,
                call_packs,
            })
        },
    )
    .map_err(|_| storage())
}

pub struct McpUsage {
    control: Mutex<ControlStore>,
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
}
impl McpUsage {
    pub fn new(control: ControlStore) -> Self {
        Self {
            control: Mutex::new(control),
            clock: Arc::new(|| OffsetDateTime::now_utc().unix_timestamp()),
        }
    }

    /// Trusted server entry: the caller derives owner from authenticated identity.
    /// Opening/querying an account's current month costs no call units.
    pub fn status(&self, owner: &str) -> Result<Usage, ToolError> {
        let mut control = self.control.lock().unwrap();
        let tx = control
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        let now = (self.clock)();
        let month = FreeMonth::at(now)?;
        enabled(&tx, owner)?;
        ensure_month(&tx, owner, &month)?;
        let result = balance(&tx, owner, &month, now)?;
        tx.commit().map_err(|_| storage())?;
        Ok(result)
    }

    /// Call only after authorized content and its encodable response are prepared.
    /// A read/encoding error must never call this method. Deliver content only on Ok.
    /// Each invocation is a new consumption, including repeated or empty results.
    pub fn consume_success(
        &self,
        owner: &str,
        tool: &str,
        reference: &PublishedBookRef,
    ) -> Result<Consumption, ToolError> {
        if !crate::mcp_read::is_content_tool(tool)
            || reference.book_id.trim().is_empty()
            || reference.publication_id.trim().is_empty()
        {
            return Err(invalid());
        }
        let mut control = self.control.lock().unwrap();
        let tx = control
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        // Sample after acquiring the write transaction, not at request admission.
        let now = (self.clock)();
        let month = FreeMonth::at(now)?;
        enabled(&tx, owner)?;
        ensure_month(&tx, owner, &month)?;
        let changed = tx.execute(
            "UPDATE mcp_free_months SET used_units=used_units+1 WHERE user_id=? AND month_hk=? AND used_units<granted_units",
            params![owner, month.key],
        ).map_err(|_| storage())?;
        let (source, debit_ref, charged_from) = if changed == 1 {
            (
                "free_month",
                month.key.clone(),
                DebitSource::FreeMonth {
                    month_hk: month.key.clone(),
                },
            )
        } else {
            let pack: Option<String> = tx.query_row(
                "SELECT pack_id FROM mcp_call_packs WHERE user_id=? AND opened_at<=? AND ?<expires_at AND used_units<total_units+adjusted_units ORDER BY expires_at,opened_at,pack_seq LIMIT 1",
                params![owner,now,now], |r|r.get(0)).optional().map_err(|_|storage())?;
            let pack = pack.ok_or_else(|| {
                error(
                    "MCP_QUOTA_EXHAUSTED",
                    "quota_exhausted",
                    &format!(
                        "Stop retrying content calls; free calls reset at {}",
                        month.resets_at
                    ),
                )
            })?;
            tx.execute(
                "UPDATE mcp_call_packs SET used_units=used_units+1 WHERE user_id=? AND pack_id=?",
                params![owner, pack],
            )
            .map_err(|_| storage())?;
            (
                "call_pack",
                pack.clone(),
                DebitSource::CallPack { pack_id: pack },
            )
        };
        let call_id = uuid::Uuid::now_v7().to_string();
        tx.execute(
            "INSERT INTO mcp_usage(call_id,user_id,tool,book_id,publication_id,units,debit_source,debit_ref,completed_at) VALUES(?,?,?,?,?,1,?,?,?)",
            params![call_id, owner, tool, reference.book_id, reference.publication_id, source, debit_ref, now],
        ).map_err(|_| storage())?;
        let usage = balance(&tx, owner, &month, now)?;
        tx.commit().map_err(|_| storage())?;
        Ok(Consumption {
            call_id,
            charged_units: 1,
            charged_from,
            completed_at: now,
            usage,
        })
    }

    /// A current-month gift is an administrator operation, with no cash receipt.
    /// Replaying its ID returns the original month and receipt even after rollover.
    pub fn gift(
        &self,
        actor: &Principal,
        operation_id: &str,
        owner: &str,
        units: i64,
        reason: &str,
    ) -> Result<Value, ToolError> {
        admin_store::operation_id(operation_id)?;
        let reason = reason.trim();
        if units <= 0 || reason.is_empty() {
            return Err(invalid());
        }
        let parameters = json!({"units": units, "reason": reason});
        let kind = "mcp-free-gift";
        let mut control = self.control.lock().unwrap();
        let tx = control
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        let now = (self.clock)();
        actor.validate_in(&tx, now)?;
        let admin: bool = tx
            .query_row(
                "SELECT is_admin FROM users WHERE user_id=?",
                [actor.user_id()],
                |r| r.get(0),
            )
            .map_err(|_| storage())?;
        if !admin {
            return Err(error(
                "ADMIN_REQUIRED",
                "authorization",
                "Administrator access required",
            ));
        }
        if let Some(result) =
            admin_store::replay(&tx, actor.user_id(), operation_id, owner, kind, &parameters)?
        {
            return Ok(result);
        }
        enabled(&tx, owner)?;
        let month = FreeMonth::at(now)?;
        ensure_month(&tx, owner, &month)?;
        let before = balance(&tx, owner, &month, now)?;
        let granted = before.free_granted.checked_add(units).ok_or_else(invalid)?;
        tx.execute(
            "UPDATE mcp_free_months SET granted_units=? WHERE user_id=? AND month_hk=?",
            params![granted, owner, month.key],
        )
        .map_err(|_| storage())?;
        let result = json!({"operation_id":operation_id,"actor":actor.user_id(),"user_id":owner,
            "kind":kind,"parameters":parameters,"created_at":now,"ok":true,
            "month_hk":month.key,"usage":balance(&tx, owner, &month, now)?});
        tx.execute("INSERT INTO admin_operations(actor,operation_id,user_id,kind,parameters_json,result_json,created_at) VALUES(?,?,?,?,?,?,?)",
            params![actor.user_id(), operation_id, owner, kind, parameters.to_string(), result.to_string(), now]).map_err(|_| storage())?;
        tx.execute("INSERT INTO mcp_free_gifts(actor,operation_id,user_id,month_hk,units,reason,created_at) VALUES(?,?,?,?,?,?,?)",
            params![actor.user_id(), operation_id, owner, month.key, units, reason, now]).map_err(|_| storage())?;
        tx.commit().map_err(|_| storage())?;
        Ok(result)
    }
}

#[cfg(test)]
#[path = "mcp_usage_tests.rs"]
mod tests;
