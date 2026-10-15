//! Purchased call batches and append-only corrections in the admin transaction.
use crate::{admin_store::invalid, allowance_admin::storage, user_storage_paths::error};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use time::{format_description::well_known::Rfc3339, Date, OffsetDateTime, UtcOffset};

pub const PACK_PRICE_FEN: i64 = 1_000;
pub const PACK_UNITS: i64 = 1_000;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Purchase {
    pub pack_count: i64,
    pub paid_at: i64,
    pub channel: String,
    pub external_ref: Option<String>,
    pub note: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Correction {
    pub receipt_id: String,
    pub revision: i64,
    pub delta_fen: i64,
    pub delta_units: i64,
    pub reason: String,
}
pub(crate) enum Mutation {
    Purchase(Purchase),
    Correction(Correction),
}
impl Mutation {
    pub fn normalize(&mut self) -> Result<(), ToolError> {
        match self {
            Self::Purchase(p) => {
                p.channel = p.channel.trim().to_owned();
                p.note = p.note.trim().to_owned();
                if let Some(reference) = &mut p.external_ref {
                    *reference = reference.trim().to_owned();
                    if reference.is_empty() {
                        return Err(invalid());
                    }
                }
                if p.pack_count <= 0 || p.channel.is_empty() {
                    return Err(invalid());
                }
                p.pack_count
                    .checked_mul(PACK_PRICE_FEN)
                    .ok_or_else(invalid)?;
                p.pack_count.checked_mul(PACK_UNITS).ok_or_else(invalid)?;
            }
            Self::Correction(c) => {
                c.reason = c.reason.trim().to_owned();
                if c.reason.is_empty() || (c.delta_fen == 0 && c.delta_units == 0) {
                    return Err(invalid());
                }
            }
        }
        Ok(())
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Purchase(_) => "mcp-packs",
            Self::Correction(_) => "mcp-corrections",
        }
    }
    pub fn parameters(&self) -> Value {
        match self {
            Self::Purchase(p) => json!(p),
            Self::Correction(c) => json!(c),
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
        let (pack_id, receipt_id) = match self {
            Self::Purchase(p) => {
                let enabled: bool = db
                    .query_row(
                        "SELECT disabled=0 FROM users WHERE user_id=?",
                        [owner],
                        |r| r.get(0),
                    )
                    .map_err(storage)?;
                if !enabled {
                    return Err(error(
                        "USER_UNAVAILABLE",
                        "not_found",
                        "User is unavailable",
                    ));
                }
                if let Some(reference) = &p.external_ref {
                    let exists: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM manual_receipts WHERE channel=? AND external_ref=?)", params![p.channel, reference], |r|r.get(0)).map_err(storage)?;
                    if exists {
                        return Err(error(
                            "RECEIPT_EXTERNAL_REF_CONFLICT",
                            "conflict",
                            "Receipt transaction already registered",
                        ));
                    }
                }
                let price = p
                    .pack_count
                    .checked_mul(PACK_PRICE_FEN)
                    .ok_or_else(invalid)?;
                let units = p.pack_count.checked_mul(PACK_UNITS).ok_or_else(invalid)?;
                let expires = expires_at(now)?;
                let receipt = uuid::Uuid::now_v7().to_string();
                let pack = uuid::Uuid::now_v7().to_string();
                db.execute("INSERT INTO manual_receipts(receipt_id,actor,operation_id,user_id,amount_fen,paid_at,channel,external_ref,note) VALUES(?,?,?,?,?,?,?,?,?)",
                    params![receipt,actor,op,owner,price,p.paid_at,p.channel,p.external_ref,p.note]).map_err(storage)?;
                db.execute("INSERT INTO mcp_call_packs(pack_id,user_id,receipt_id,pack_count,unit_price_fen,units_per_pack,price_fen,total_units,opened_at,expires_at) VALUES(?,?,?,?,?,?,?,?,?,?)",
                    params![pack,owner,receipt,p.pack_count,PACK_PRICE_FEN,PACK_UNITS,price,units,now,expires]).map_err(storage)?;
                (pack, receipt)
            }
            Self::Correction(c) => {
                let original: Option<(String,i64,i64,i64,i64)> = db.query_row("SELECT pack_id,total_units,adjusted_units,used_units,revision FROM mcp_call_packs WHERE user_id=? AND receipt_id=?", params![owner,c.receipt_id], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional().map_err(storage)?;
                let (pack, total, adjusted, used, revision) =
                    original.ok_or_else(crate::authorization::missing)?;
                if revision != c.revision {
                    return Err(error(
                        "MCP_PACK_REVISION_CONFLICT",
                        "conflict",
                        "Reload the call pack before correcting it",
                    ));
                }
                let adjusted = adjusted.checked_add(c.delta_units).ok_or_else(invalid)?;
                let granted = total.checked_add(adjusted).ok_or_else(invalid)?;
                if granted < used {
                    return Err(error(
                        "MCP_PACK_ALREADY_CONSUMED",
                        "conflict",
                        "Only unconsumed call units can be withdrawn",
                    ));
                }
                if c.delta_fen != 0 {
                    crate::allowance_admin::correct_receipt(
                        db,
                        actor,
                        op,
                        owner,
                        &c.receipt_id,
                        c.delta_fen,
                        &c.reason,
                        now,
                    )?;
                }
                if c.delta_units != 0 {
                    db.execute(
                        "INSERT INTO mcp_pack_adjustments VALUES(?,?,?,?,?,?,?)",
                        params![actor, op, owner, pack, c.delta_units, c.reason, now],
                    )
                    .map_err(storage)?;
                }
                db.execute(
                    "UPDATE mcp_call_packs SET adjusted_units=?,revision=? WHERE pack_id=?",
                    params![adjusted, revision.checked_add(1).ok_or_else(invalid)?, pack],
                )
                .map_err(storage)?;
                (pack, c.receipt_id.clone())
            }
        };
        Ok(
            json!({"receipt_id":receipt_id,"entitlement_kind":"mcp_call_pack","call_pack":pack(db,owner,&pack_id,now)?}),
        )
    }
}

/// Twelve Hong Kong calendar months, clamped to the last day of the target month.
fn expires_at(now: i64) -> Result<i64, ToolError> {
    let local = OffsetDateTime::from_unix_timestamp(now)
        .map_err(|_| invalid())?
        .to_offset(UtcOffset::from_hms(8, 0, 0).unwrap());
    let year = local.year().checked_add(1).ok_or_else(invalid)?;
    let mut day = local.day();
    let date = loop {
        match Date::from_calendar_date(year, local.month(), day) {
            Ok(date) => break date,
            Err(_) if day > 28 => day -= 1,
            Err(_) => return Err(invalid()),
        }
    };
    Ok(date
        .with_time(local.time())
        .assume_offset(local.offset())
        .unix_timestamp())
}

pub(crate) fn display_time(value: i64) -> Result<String, ToolError> {
    OffsetDateTime::from_unix_timestamp(value)
        .map_err(|_| invalid())?
        .to_offset(UtcOffset::from_hms(8, 0, 0).unwrap())
        .format(&Rfc3339)
        .map_err(|_| invalid())
}

#[derive(Debug, Serialize)]
pub struct CallPack {
    pub pack_id: String,
    pub receipt_id: String,
    pub pack_count: i64,
    pub unit_price_fen: i64,
    pub units_per_pack: i64,
    pub price_fen: i64,
    pub total_units: i64,
    pub adjusted_units: i64,
    pub used_units: i64,
    pub remaining_units: i64,
    pub available_units: i64,
    pub opened_at: String,
    pub expires_at: String,
    pub revision: i64,
}
fn row(r: &rusqlite::Row<'_>, now: i64) -> rusqlite::Result<CallPack> {
    let total: i64 = r.get(6)?;
    let adjusted: i64 = r.get(7)?;
    let used: i64 = r.get(8)?;
    let opened: i64 = r.get(9)?;
    let expires: i64 = r.get(10)?;
    let remaining = total + adjusted - used;
    let time = |v| display_time(v).map_err(|_| rusqlite::Error::InvalidQuery);
    Ok(CallPack {
        pack_id: r.get(0)?,
        receipt_id: r.get(1)?,
        pack_count: r.get(2)?,
        unit_price_fen: r.get(3)?,
        units_per_pack: r.get(4)?,
        price_fen: r.get(5)?,
        total_units: total,
        adjusted_units: adjusted,
        used_units: used,
        remaining_units: remaining,
        available_units: if opened <= now && now < expires {
            remaining
        } else {
            0
        },
        opened_at: time(opened)?,
        expires_at: time(expires)?,
        revision: r.get(11)?,
    })
}
const COLUMNS: &str = "pack_id,receipt_id,pack_count,unit_price_fen,units_per_pack,price_fen,total_units,adjusted_units,used_units,opened_at,expires_at,revision";
pub(crate) fn pack(
    db: &Connection,
    owner: &str,
    id: &str,
    now: i64,
) -> Result<CallPack, ToolError> {
    db.query_row(
        &format!("SELECT {COLUMNS} FROM mcp_call_packs WHERE user_id=? AND pack_id=?"),
        params![owner, id],
        |r| row(r, now),
    )
    .optional()
    .map_err(storage)?
    .ok_or_else(crate::authorization::missing)
}
pub(crate) fn packs(db: &Connection, owner: &str, now: i64) -> Result<Vec<CallPack>, ToolError> {
    db.prepare(&format!("SELECT {COLUMNS} FROM mcp_call_packs WHERE user_id=? ORDER BY expires_at,opened_at,pack_seq")).map_err(storage)?
        .query_map([owner],|r|row(r,now)).map_err(storage)?.collect::<rusqlite::Result<_>>().map_err(storage)
}
