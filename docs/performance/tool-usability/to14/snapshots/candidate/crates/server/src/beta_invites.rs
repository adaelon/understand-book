//! Invite batches have their own operation identity, before a reader exists.
use crate::{
    admin_store::{self, AdminStore},
    user_storage_paths::error,
};
use rand_core::{OsRng, RngCore};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::{json, Value};

const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const COLUMNS: &str = "i.invite_id,i.code,i.actor,i.operation_id,i.state,i.created_at,i.used_by,i.used_at,i.disabled_at,u.email";

fn storage() -> ToolError {
    error(
        "INVITE_STORAGE_UNAVAILABLE",
        "unavailable",
        "Invite operation could not be committed",
    )
}
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(
        json!({"invite_id":r.get::<_,String>(0)?,"code":r.get::<_,String>(1)?,
        "actor":r.get::<_,String>(2)?,"operation_id":r.get::<_,String>(3)?,
        "state":r.get::<_,String>(4)?,"created_at":r.get::<_,i64>(5)?,
        "used_by":r.get::<_,Option<String>>(6)?,"used_at":r.get::<_,Option<i64>>(7)?,
        "disabled_at":r.get::<_,Option<i64>>(8)?,"used_email":r.get::<_,Option<String>>(9)?}),
    )
}
fn batch(db: &Connection, actor: &str, id: &str) -> Result<Option<Value>, ToolError> {
    let saved: Option<(u32, i64)> = db
        .query_row(
            "SELECT count,created_at FROM invite_batches WHERE actor=? AND operation_id=?",
            params![actor, id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|_| storage())?;
    let Some((count, created_at)) = saved else {
        return Ok(None);
    };
    let mut stmt = db.prepare(&format!("SELECT {COLUMNS} FROM beta_invites i LEFT JOIN users u ON u.user_id=i.used_by WHERE i.actor=? AND i.operation_id=? ORDER BY i.invite_id")).map_err(|_|storage())?;
    let items = stmt
        .query_map(params![actor, id], row)
        .map_err(|_| storage())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| storage())?;
    Ok(Some(
        json!({"actor":actor,"operation_id":id,"count":count,"created_at":created_at,"invites":items}),
    ))
}
fn invite(db: &Connection, id: &str) -> Result<Value, ToolError> {
    db.query_row(&format!("SELECT {COLUMNS} FROM beta_invites i LEFT JOIN users u ON u.user_id=i.used_by WHERE i.invite_id=?"), [id], row)
        .optional().map_err(|_|storage())?.ok_or_else(crate::authorization::missing)
}

impl AdminStore {
    pub(crate) fn invite_batch(&self, actor: &str, id: &str) -> Result<Value, ToolError> {
        admin_store::operation_id(id)?;
        let mut control = self.control.lock().unwrap();
        let tx = control.connection.transaction().map_err(|_| storage())?;
        batch(&tx, actor, id)?.ok_or_else(crate::authorization::missing)
    }

    pub(crate) fn create_invites(
        &self,
        actor: &str,
        id: &str,
        count: u32,
        now: i64,
    ) -> Result<Value, ToolError> {
        admin_store::operation_id(id)?;
        if !(1..=100).contains(&count) {
            return Err(admin_store::invalid());
        }
        let mut control = self.control.lock().unwrap();
        let tx = control
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        if let Some(saved) = batch(&tx, actor, id)? {
            if saved["count"] != count {
                return Err(error(
                    "INVITE_BATCH_CONFLICT",
                    "conflict",
                    "Operation ID has a different invite count",
                ));
            }
            return Ok(saved);
        }
        tx.execute(
            "INSERT INTO invite_batches(actor,operation_id,count,created_at) VALUES(?,?,?,?)",
            params![actor, id, count, now],
        )
        .map_err(|_| storage())?;
        for _ in 0..count {
            let mut bytes = [0u8; 20];
            OsRng.try_fill_bytes(&mut bytes).map_err(|_| storage())?;
            // 32 symbols divides the byte range exactly; every symbol is equiprobable.
            let code: String = bytes
                .iter()
                .map(|b| ALPHABET[(b & 31) as usize] as char)
                .collect();
            tx.execute("INSERT INTO beta_invites(invite_id,code,actor,operation_id,state,created_at) VALUES(?,?,?,?,'unused',?)",
                params![uuid::Uuid::now_v7().to_string(),code,actor,id,now]).map_err(|_|storage())?;
        }
        let result = batch(&tx, actor, id)?.unwrap();
        tx.commit().map_err(|_| storage())?;
        Ok(result)
    }

    pub(crate) fn invites(
        &self,
        state: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Value, ToolError> {
        if state.is_some_and(|s| !matches!(s, "unused" | "used" | "disabled")) {
            return Err(admin_store::invalid());
        }
        let mut control = self.control.lock().unwrap();
        let tx = control.connection.transaction().map_err(|_| storage())?;
        let mut stmt = tx.prepare(&format!("SELECT {COLUMNS} FROM beta_invites i LEFT JOIN users u ON u.user_id=i.used_by WHERE (?1 IS NULL OR i.state=?1) ORDER BY i.created_at DESC,i.invite_id DESC LIMIT ?2 OFFSET ?3")).map_err(|_|storage())?;
        let items = stmt
            .query_map(params![state, limit, offset], row)
            .map_err(|_| storage())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|_| storage())?;
        let total: i64 = tx
            .query_row(
                "SELECT count(*) FROM beta_invites WHERE (?1 IS NULL OR state=?1)",
                params![state],
                |r| r.get(0),
            )
            .map_err(|_| storage())?;
        Ok(json!({"invites":items,"total":total,"limit":limit,"offset":offset}))
    }

    pub(crate) fn disable_invite(&self, id: &str, now: i64) -> Result<Value, ToolError> {
        let mut control = self.control.lock().unwrap();
        let tx = control
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        let current = invite(&tx, id)?;
        if current["state"] == "used" {
            return Err(error(
                "INVITE_ALREADY_USED",
                "conflict",
                "A used invite cannot be disabled",
            ));
        }
        if current["state"] == "unused" {
            tx.execute(
                "UPDATE beta_invites SET state='disabled',disabled_at=? WHERE invite_id=?",
                params![now, id],
            )
            .map_err(|_| storage())?;
        }
        let result = invite(&tx, id)?;
        tx.commit().map_err(|_| storage())?;
        Ok(result)
    }
}
