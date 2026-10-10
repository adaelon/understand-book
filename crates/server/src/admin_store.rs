//! Online account mutations and their durable receipts share one SQLite transaction.
use crate::{
    auth,
    control_store::ControlStore,
    published_library::PublishedBookRef,
    user_storage_paths::{error, validate_user_id},
};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::{json, Value};
use std::sync::Mutex;

pub(crate) fn invalid() -> ToolError {
    error(
        "INVALID_REQUEST",
        "validation",
        "Invalid management request",
    )
}
fn storage() -> ToolError {
    error(
        "ADMIN_STORAGE_UNAVAILABLE",
        "unavailable",
        "Management operation could not be committed",
    )
}
fn conflict() -> ToolError {
    error(
        "ADMIN_OPERATION_CONFLICT",
        "conflict",
        "Operation ID has different business parameters",
    )
}
pub(crate) fn operation_id(id: &str) -> Result<(), ToolError> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) enum Command {
    Create(String),
    Password(String),
    Status(bool),
    RevokeSessions,
    Grant(PublishedBookRef),
    Revoke(PublishedBookRef),
    Allowance(crate::allowance_admin::Mutation),
    Reconcile(String, crate::charge_admin::Reconcile),
}
impl Command {
    fn kind(&self) -> &'static str {
        match self {
            Self::Create(_) => "create-user",
            Self::Password(_) => "password",
            Self::Status(_) => "status",
            Self::RevokeSessions => "revoke-sessions",
            Self::Grant(_) => "book-grants",
            Self::Revoke(_) => "book-revocations",
            Self::Allowance(m) => m.kind(),
            Self::Reconcile(..) => "charge-reconcile",
        }
    }
    fn parameters(&self) -> Value {
        match self {
            Self::Allowance(m) => m.parameters(),
            Self::Reconcile(call, r) => json!({"call_id":call,"reconciliation":r}),
            Self::Status(disabled) => json!({"disabled":disabled}),
            Self::Grant(reference) | Self::Revoke(reference) => {
                json!({"published_book_ref":reference})
            }
            // Secrets are never audit parameters. Reusing this ID only retrieves
            // its receipt; supplying a new password requires a new operation ID.
            _ => json!({}),
        }
    }
}

pub(crate) struct AdminStore {
    pub(crate) control: Mutex<ControlStore>,
}
impl AdminStore {
    pub(crate) fn new(control: ControlStore) -> Self {
        Self {
            control: Mutex::new(control),
        }
    }

    pub(crate) fn operation(&self, actor: &str, id: &str) -> Result<Value, ToolError> {
        operation_id(id)?;
        let control = self.control.lock().unwrap();
        let result: Option<String> = control
            .connection
            .query_row(
                "SELECT result_json FROM admin_operations WHERE actor=? AND operation_id=?",
                params![actor, id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| storage())?;
        serde_json::from_str(&result.ok_or_else(crate::authorization::missing)?)
            .map_err(|_| storage())
    }

    /// The boolean requests cancellation notification only for a newly committed disable.
    pub(crate) fn apply(
        &self,
        actor: &str,
        id: &str,
        owner: &str,
        mut command: Command,
        now: i64,
    ) -> Result<(Value, bool), ToolError> {
        operation_id(id)?;
        validate_user_id(owner)?;
        if let Command::Allowance(m) = &mut command {
            m.normalize()?;
        }
        if let Command::Reconcile(_, r) = &mut command { r.normalize()?; }
        let kind = command.kind();
        let parameters = command.parameters();
        {
            let control = self.control.lock().unwrap();
            if let Some(result) = replay(&control.connection, actor, id, owner, kind, &parameters)?
            {
                return Ok((result, false));
            }
        }
        // Argon2 does not hold the management DB mutex or a SQLite write transaction.
        let password_hash = match &command {
            Command::Create(password) | Command::Password(password) => {
                Some(auth::encode_password(password)?)
            }
            _ => None,
        };
        let mut control = self.control.lock().unwrap();
        let tx = control
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        if let Some(result) = replay(&tx, actor, id, owner, kind, &parameters)? {
            return Ok((result, false));
        }
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM users WHERE user_id=?)",
                [owner],
                |r| r.get(0),
            )
            .map_err(|_| storage())?;
        if matches!(command, Command::Create(_)) {
            if exists {
                return Err(error(
                    "USER_ALREADY_EXISTS",
                    "conflict",
                    "User already exists",
                ));
            }
        } else if !exists {
            return Err(crate::authorization::missing());
        }
        let mut cancelled = 0;
        // Allowance rows reference the operation. Save its final result below in
        // the same transaction, so no placeholder is externally observable.
        if matches!(command, Command::Allowance(_) | Command::Reconcile(..)) {
            tx.execute(
                "INSERT INTO admin_operations VALUES(?,?,?,?,?,'{}',?)",
                params![actor, id, owner, kind, parameters.to_string(), now],
            )
            .map_err(|_| storage())?;
        }
        let mut allowance_result = None;
        match &command {
            Command::Allowance(m) => allowance_result = Some(m.apply(&tx, actor, id, owner, now)?),
            Command::Reconcile(call, r) => allowance_result = Some(r.apply(&tx, actor, id, owner, call, now)?),
            Command::Create(_) | Command::Password(_) => auth::write_password(
                &tx,
                owner,
                password_hash.as_deref().unwrap(),
                matches!(command, Command::Create(_)),
            )?,
            Command::Status(disabled) => {
                auth::write_session_revocation(&tx, owner, Some(*disabled))?;
                if *disabled {
                    cancelled = tx.execute("UPDATE run_admissions SET cancel_requested=1 WHERE owner_user_id=? AND dispatch_state IN ('preparing','queued','claimed') AND key_closed=0", [owner]).map_err(|_| storage())?;
                }
            }
            Command::RevokeSessions => auth::write_session_revocation(&tx, owner, None)?,
            Command::Grant(reference) | Command::Revoke(reference) => {
                let published: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM book_publications WHERE book_id=? AND publication_id=?)", params![reference.book_id, reference.publication_id], |r| r.get(0)).map_err(|_| storage())?;
                if !published {
                    return Err(crate::authorization::missing());
                }
                if matches!(command, Command::Grant(_)) {
                    tx.execute("INSERT OR IGNORE INTO book_grants(owner_user_id,book_id,publication_id) VALUES(?,?,?)", params![owner, reference.book_id, reference.publication_id]).map_err(|_| storage())?;
                } else {
                    tx.execute("DELETE FROM book_grants WHERE owner_user_id=? AND book_id=? AND publication_id=?", params![owner, reference.book_id, reference.publication_id]).map_err(|_| storage())?;
                }
            }
        }
        let mut result = json!({"operation_id":id,"actor":actor,"user_id":owner,"kind":kind,"parameters":parameters,"created_at":now,"ok":true});
        if matches!(command, Command::Status(true)) {
            result["cancel_requested_runs"] = json!(cancelled);
        }
        if let Some(fields) = allowance_result {
            result
                .as_object_mut()
                .unwrap()
                .extend(fields.as_object().unwrap().clone());
            tx.execute(
                "UPDATE admin_operations SET result_json=? WHERE actor=? AND operation_id=?",
                params![result.to_string(), actor, id],
            )
            .map_err(|_| storage())?;
        } else {
            tx.execute("INSERT INTO admin_operations(actor,operation_id,user_id,kind,parameters_json,result_json,created_at) VALUES(?,?,?,?,?,?,?)", params![actor,id,owner,kind,parameters.to_string(),result.to_string(),now]).map_err(|_| storage())?;
        }
        tx.commit().map_err(|_| storage())?;
        Ok((result, matches!(command, Command::Status(true))))
    }

    pub(crate) fn users(&self, limit: u32, offset: u32, search: &str, disabled: Option<bool>, now: i64) -> Result<Value, ToolError> {
        let mut control = self.control.lock().unwrap();
        let tx = control.connection.transaction().map_err(|_| storage())?;
        let filter = "(instr(lower(user_id),lower(?1))>0 OR instr(email,lower(?1))>0) AND (?2 IS NULL OR disabled=?2)";
        let mut query = tx.prepare(&format!("SELECT user_id,disabled,is_admin,email FROM users WHERE {filter} ORDER BY user_id LIMIT ?3 OFFSET ?4")).map_err(|_| storage())?;
        let mut users = query
            .query_map(params![search, disabled, limit, offset], user_row)
            .map_err(|_| storage())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| storage())?;
        for user in &mut users {
            user["current_allowance"] = crate::allowance_admin::current(&tx, user["user_id"].as_str().unwrap(), now)?;
            user["activity"] = crate::admin_usage::account_activity(&tx, user["user_id"].as_str().unwrap())?;
        }
        let total: i64 = tx
            .query_row(&format!("SELECT count(*) FROM users WHERE {filter}"), params![search, disabled], |r| r.get(0))
            .map_err(|_| storage())?;
        Ok(json!({"users":users,"total":total,"limit":limit,"offset":offset}))
    }

    pub(crate) fn user(&self, owner: &str, limit: u32, offset: u32) -> Result<Value, ToolError> {
        validate_user_id(owner)?;
        let control = self.control.lock().unwrap();
        let mut user = control
            .connection
            .query_row(
                "SELECT user_id,disabled,is_admin,email FROM users WHERE user_id=?",
                [owner],
                user_row,
            )
            .optional()
            .map_err(|_| storage())?
            .ok_or_else(crate::authorization::missing)?;
        let mut query = control.connection.prepare("SELECT book_id,publication_id FROM book_grants WHERE owner_user_id=? ORDER BY book_id,publication_id LIMIT ? OFFSET ?").map_err(|_| storage())?;
        let grants = query
            .query_map(params![owner, limit, offset], |r| {
                Ok(json!({"book_id":r.get::<_,String>(0)?,"publication_id":r.get::<_,String>(1)?}))
            })
            .map_err(|_| storage())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| storage())?;
        let total: i64 = control
            .connection
            .query_row(
                "SELECT count(*) FROM book_grants WHERE owner_user_id=?",
                [owner],
                |r| r.get(0),
            )
            .map_err(|_| storage())?;
        user["book_grants"] = json!({"items":grants,"total":total,"limit":limit,"offset":offset});
        user["activity"] = crate::admin_usage::account_activity(&control.connection, owner)?;
        user["current_allowance"] = crate::allowance_admin::current(
            &control.connection,
            owner,
            crate::multi_user_host::now(),
        )?;
        Ok(user)
    }

    pub(crate) fn allowance_list(
        &self,
        owner: &str,
        kind: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Value, ToolError> {
        validate_user_id(owner)?;
        let control = self.control.lock().unwrap();
        crate::allowance_admin::list(&control.connection, owner, kind, limit, offset)
    }

    pub(crate) fn user_operations(&self, owner: &str, limit: u32, offset: u32) -> Result<Value, ToolError> {
        validate_user_id(owner)?;
        let mut control = self.control.lock().unwrap();
        let tx = control.connection.transaction().map_err(|_| storage())?;
        let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM users WHERE user_id=?)", [owner], |r| r.get(0)).map_err(|_| storage())?;
        if !exists { return Err(crate::authorization::missing()); }
        let mut stmt = tx.prepare("SELECT actor,operation_id,kind,created_at FROM admin_operations WHERE user_id=? ORDER BY created_at DESC,actor,operation_id LIMIT ? OFFSET ?").map_err(|_| storage())?;
        let items = stmt.query_map(params![owner,limit,offset], |r| Ok(json!({"actor":r.get::<_,String>(0)?,"operation_id":r.get::<_,String>(1)?,"kind":r.get::<_,String>(2)?,"created_at":r.get::<_,i64>(3)?}))).map_err(|_| storage())?.collect::<Result<Vec<_>,_>>().map_err(|_| storage())?;
        let total: i64 = tx.query_row("SELECT count(*) FROM admin_operations WHERE user_id=?", [owner], |r| r.get(0)).map_err(|_| storage())?;
        Ok(json!({"items":items,"total":total,"limit":limit,"offset":offset}))
    }

    pub(crate) fn books(&self, limit: u32, offset: u32) -> Result<Value, ToolError> {
        let control = self.control.lock().unwrap();
        let mut query = control.connection.prepare("SELECT p.book_id,p.publication_id,COALESCE(d.publication_id=p.publication_id,0) FROM book_publications p LEFT JOIN book_defaults d USING(book_id) ORDER BY p.book_id,p.publication_id LIMIT ? OFFSET ?").map_err(|_| storage())?;
        let books = query.query_map(params![limit,offset], |r| Ok(json!({"published_book_ref":{"book_id":r.get::<_,String>(0)?,"publication_id":r.get::<_,String>(1)?},"is_default":r.get::<_,bool>(2)?}))).map_err(|_| storage())?.collect::<Result<Vec<_>,_>>().map_err(|_| storage())?;
        let total: i64 = control
            .connection
            .query_row("SELECT count(*) FROM book_publications", [], |r| r.get(0))
            .map_err(|_| storage())?;
        Ok(json!({"books":books,"total":total,"limit":limit,"offset":offset}))
    }
}

fn user_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(
        json!({"user_id":r.get::<_,String>(0)?,"disabled":r.get::<_,bool>(1)?,"is_admin":r.get::<_,bool>(2)?,"email":r.get::<_,Option<String>>(3)?}),
    )
}

fn replay(
    db: &Connection,
    actor: &str,
    id: &str,
    owner: &str,
    kind: &str,
    parameters: &Value,
) -> Result<Option<Value>, ToolError> {
    let saved: Option<(String,String,String,String)> = db.query_row("SELECT user_id,kind,parameters_json,result_json FROM admin_operations WHERE actor=? AND operation_id=?", params![actor,id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(|_| storage())?;
    let Some((saved_owner, saved_kind, saved_parameters, result)) = saved else {
        return Ok(None);
    };
    let saved_parameters: Value = serde_json::from_str(&saved_parameters).map_err(|_| storage())?;
    if saved_owner != owner || saved_kind != kind || saved_parameters != *parameters {
        return Err(conflict());
    }
    serde_json::from_str(&result)
        .map(Some)
        .map_err(|_| storage())
}
