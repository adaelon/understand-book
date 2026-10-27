//! One invite creates one guest, a single allowance and the inviter's default
//! materials. No email, password or second balance system is involved.
use crate::{
    account_allowance::{AllowanceBasis, MicroCny},
    allowance_admin::{Adjustment, Mutation, Period},
    auth,
    published_library::PublishedBookRef,
    user_storage_paths::error,
};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Deserialize;
use serde_json::json;

pub(crate) const GRANT_MICRO_CNY: i64 = 5 * MicroCny::PER_YUAN;
pub(crate) const EXCLUDED_BOOK: &str = "understandbook";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Start {
    invite_code: String,
}

fn storage(_: rusqlite::Error) -> ToolError {
    error("GUEST_TRIAL_UNAVAILABLE", "unavailable", "Guest access could not be committed")
}
fn unavailable() -> ToolError {
    error("INVITE_UNAVAILABLE", "conflict", "Invite is unavailable")
}

pub(crate) fn redeem(
    db: &mut Connection,
    input: Start,
    previous: Option<&str>,
    now: i64,
) -> Result<String, ToolError> {
    let code = input.invite_code.trim().replace('-', "").to_ascii_uppercase();
    if code.len() != 20 || !code.bytes().all(|b| b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(&b)) {
        return Err(error("INVITE_INVALID", "validation", "Enter a valid invite code"));
    }
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
    let (invite, actor, state, used_by): (String, String, String, Option<String>) = tx.query_row(
        "SELECT invite_id,actor,state,used_by FROM beta_invites WHERE code=?", [&code],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    ).optional().map_err(storage)?.ok_or_else(unavailable)?;

    // Retrying a completed request only works with that guest's existing cookie.
    // Knowing a used invite never grants access to someone else's private work.
    if state == "used" {
        if let (Some(token), Some(owner)) = (previous, used_by) {
            let same_guest: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM auth_sessions s JOIN users u ON u.user_id=s.owner_user_id WHERE s.token_digest=? AND u.user_id=? AND u.password_hash IS NULL AND u.disabled=0 AND s.auth_epoch=u.auth_epoch AND s.expires_at>?)",
                params![auth::digest(token), owner, now], |r| r.get(0),
            ).map_err(storage)?;
            if same_guest { return Ok(token.to_owned()); }
        }
        return Err(unavailable());
    }
    if state != "unused" { return Err(unavailable()); }

    // The issuing administrator supplies the demonstration library. This avoids
    // exposing other readers' privately prepared publications in the same store.
    let materials = {
        let mut query = tx.prepare(
            "SELECT d.book_id,d.publication_id FROM book_defaults d JOIN book_grants g USING(book_id,publication_id) WHERE g.owner_user_id=? AND d.book_id<>? ORDER BY d.book_id",
        ).map_err(storage)?;
        let rows = query.query_map(params![actor, EXCLUDED_BOOK], |r| Ok(PublishedBookRef {
            book_id: r.get(0)?, publication_id: r.get(1)?,
        })).map_err(storage)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(storage)?
    };
    if materials.is_empty() {
        return Err(error("GUEST_MATERIALS_UNAVAILABLE", "unavailable", "No demonstration materials are available; the invite has not been used"));
    }

    let owner = format!("guest_{}", uuid::Uuid::now_v7());
    let operation = format!("guest-trial:{}", invite);
    tx.execute("INSERT INTO users(user_id) VALUES(?)", [&owner]).map_err(storage)?;
    // Existing deployments can grant default materials when any user is created.
    // A guest uses this redemption's exact selection, including its exclusions.
    tx.execute("DELETE FROM book_grants WHERE owner_user_id=?", [&owner]).map_err(storage)?;
    let period = Mutation::Period(Period {
        starts_at: now,
        expires_at: now + auth::SESSION_SECONDS,
        basis: Some(AllowanceBasis {
            version: "guest-trial-5-cny-v1".into(),
            basis_micro_cny: MicroCny::new(GRANT_MICRO_CNY),
        }),
    }).apply(&tx, &actor, &operation, &owner, now)?;
    let period_id = period["allowance_period"]["period_id"].as_str().unwrap();
    let parameters = json!({"invite_id":invite,"grant_micro_cny":GRANT_MICRO_CNY,"materials":materials});
    tx.execute(
        "INSERT INTO admin_operations(actor,operation_id,user_id,kind,parameters_json,result_json,created_at) VALUES(?,?,?,'guest_trial',?,'{}',?)",
        params![actor, operation, owner, parameters.to_string(), now],
    ).map_err(storage)?;
    let result = Mutation::Adjustment(Adjustment {
        period_id: period_id.into(), revision: 0,
        delta_micro_cny: MicroCny::new(GRANT_MICRO_CNY), reason: "内测码游客体验赠送 5 元额度".into(),
    }).apply(&tx, &actor, &operation, &owner, now)?;
    tx.execute("UPDATE admin_operations SET result_json=? WHERE actor=? AND operation_id=?",
        params![result.to_string(), actor, operation]).map_err(storage)?;
    for material in &materials {
        tx.execute("INSERT INTO book_grants(owner_user_id,book_id,publication_id) VALUES(?,?,?)",
            params![owner, material.book_id, material.publication_id]).map_err(storage)?;
    }
    tx.execute("UPDATE beta_invites SET state='used',used_by=?,used_at=? WHERE invite_id=?",
        params![owner, now, invite]).map_err(storage)?;
    let token = auth::write_session(&tx, &owner, 0, previous, now)?;
    tx.commit().map_err(storage)?;
    Ok(token)
}
