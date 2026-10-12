//! Pending registration never reserves an invite. Account creation, email binding,
//! invite consumption and the completion receipt commit in one transaction.
use crate::{
    account_mail::{
        self, AccountMail, VerificationPurpose, MAX_VERIFICATION_ATTEMPTS, RESEND_SECONDS,
        VERIFICATION_SECONDS,
    },
    auth,
    control_store::ControlStore,
    user_storage_paths::error,
};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Start {
    email: String,
    password: String,
    invite_code: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Resend {
    request_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Complete {
    request_id: String,
    verification_code: String,
}

pub(crate) struct Failure {
    pub error: ToolError,
    // Only populated after a durable request exists; contains no credentials.
    pub request: Option<Value>,
}
impl From<ToolError> for Failure {
    fn from(error: ToolError) -> Self {
        Self {
            error,
            request: None,
        }
    }
}
fn storage() -> ToolError {
    error(
        "REGISTRATION_STORAGE_UNAVAILABLE",
        "unavailable",
        "Registration could not be committed",
    )
}
fn limited() -> ToolError {
    error(
        "REGISTRATION_RATE_LIMITED",
        "rate_limit",
        "Too many registration attempts; try again later",
    )
}
fn expired() -> ToolError {
    error(
        "REGISTRATION_EXPIRED",
        "conflict",
        "Start a new registration request",
    )
}
fn check_email(db: &Connection, email: &str) -> Result<(), ToolError> {
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE email=?)",
            [email],
            |r| r.get(0),
        )
        .map_err(|_| storage())?;
    if exists {
        return Err(error(
            "ACCOUNT_EMAIL_IN_USE",
            "conflict",
            "Email already belongs to an account",
        ));
    }
    Ok(())
}
fn check_invite(db: &Connection, id: &str) -> Result<(), ToolError> {
    let usable: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM beta_invites WHERE invite_id=? AND state='unused')",
            [id],
            |r| r.get(0),
        )
        .map_err(|_| storage())?;
    if !usable {
        return Err(error(
            "INVITE_UNAVAILABLE",
            "conflict",
            "Invite is unavailable",
        ));
    }
    Ok(())
}
fn pending_reply(id: &str, expires_at: i64) -> Value {
    json!({"request_id":id,"expires_at":expires_at,"resend_after":RESEND_SECONDS})
}
fn completed_reply(id: &str) -> Value {
    json!({"request_id":id,"completed":true})
}
struct Pending {
    email: String,
    hash: Option<String>,
    invite_id: String,
    code: Option<String>,
    expires_at: i64,
    failures: u32,
    completed: Option<String>,
}
fn request(db: &Connection, id: &str) -> Result<Pending, ToolError> {
    db.query_row("SELECT email,password_hash,invite_id,verification_code,expires_at,failed_attempts,completed_user_id FROM registration_requests WHERE request_id=?", [id], |r| Ok(Pending {
        email:r.get(0)?, hash:r.get(1)?, invite_id:r.get(2)?, code:r.get(3)?, expires_at:r.get(4)?, failures:r.get(5)?, completed:r.get(6)?,
    })).optional().map_err(|_| storage())?.ok_or_else(|| error("REGISTRATION_NOT_FOUND", "not_found", "Registration request is unavailable"))
}

pub(crate) struct Registration {
    control: Mutex<ControlStore>,
    starts: Mutex<VecDeque<Instant>>,
    completions: Mutex<VecDeque<Instant>>,
    password_slot: Mutex<()>,
}
fn reserve(attempts: &Mutex<VecDeque<Instant>>) -> Result<(), ToolError> {
    let mut attempts = attempts.lock().unwrap();
    let now = Instant::now();
    while attempts
        .front()
        .is_some_and(|at| now.duration_since(*at) >= Duration::from_secs(60))
    {
        attempts.pop_front();
    }
    if attempts.len() >= 60 {
        return Err(limited());
    }
    attempts.push_back(now);
    Ok(())
}
impl Registration {
    pub(crate) fn new(control: ControlStore) -> Self {
        Self {
            control: Mutex::new(control),
            starts: Mutex::new(VecDeque::new()),
            completions: Mutex::new(VecDeque::new()),
            password_slot: Mutex::new(()),
        }
    }
    pub(crate) fn start(
        &self,
        input: Start,
        mail: &AccountMail,
        now: i64,
    ) -> Result<Value, Failure> {
        reserve(&self.starts)?;
        let email = account_mail::normalize_email(&input.email)?;
        let code = input
            .invite_code
            .trim()
            .replace('-', "")
            .to_ascii_uppercase();
        if code.len() != 20
            || !code
                .bytes()
                .all(|b| b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(&b))
        {
            return Err(error("INVITE_INVALID", "validation", "Enter a valid invite code").into());
        }
        // Check before expensive hashing, then recheck under the write transaction.
        let invite_id: String = {
            let control = self.control.lock().unwrap();
            check_email(&control.connection, &email)?;
            let id: String = control
                .connection
                .query_row(
                    "SELECT invite_id FROM beta_invites WHERE code=?",
                    [&code],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|_| storage())?
                .ok_or_else(|| error("INVITE_UNAVAILABLE", "conflict", "Invite is unavailable"))?;
            check_invite(&control.connection, &id)?;
            id
        };
        let hash = {
            let _slot = self.password_slot.try_lock().map_err(|_| limited())?;
            auth::encode_password(&input.password)?
        };
        let send = mail.reserve(&email)?;
        let verification = account_mail::verification_code()?;
        let id = uuid::Uuid::now_v7().to_string();
        let expires_at = now + VERIFICATION_SECONDS;
        {
            let mut control = self.control.lock().unwrap();
            let tx = control
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| storage())?;
            check_email(&tx, &email)?;
            check_invite(&tx, &invite_id)?;
            tx.execute(
                "DELETE FROM registration_requests WHERE expires_at<=?",
                [now],
            )
            .map_err(|_| storage())?;
            tx.execute("INSERT INTO registration_requests(request_id,email,password_hash,invite_id,verification_code,expires_at) VALUES(?,?,?,?,?,?)", params![id,email,hash,invite_id,verification,expires_at]).map_err(|_| storage())?;
            tx.commit().map_err(|_| storage())?;
        }
        let reply = pending_reply(&id, expires_at);
        send.verification(VerificationPurpose::Registration, &verification)
            .map_err(|error| Failure {
                error,
                request: Some(reply.clone()),
            })?;
        Ok(reply)
    }
    pub(crate) fn resend(
        &self,
        input: Resend,
        mail: &AccountMail,
        now: i64,
    ) -> Result<Value, Failure> {
        let (send, verification, reply) = {
            let mut control = self.control.lock().unwrap();
            let tx = control
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| storage())?;
            let pending = request(&tx, &input.request_id)?;
            if pending.expires_at <= now {
                return Err(expired().into());
            }
            if pending.completed.is_some() {
                return Ok(completed_reply(&input.request_id));
            }
            check_email(&tx, &pending.email)?;
            check_invite(&tx, &pending.invite_id)?;
            let send = mail.reserve(&pending.email)?;
            // Replacement must invalidate the old code, even when random generation repeats it.
            let verification = loop {
                let next = account_mail::verification_code()?;
                if pending.code.as_deref() != Some(next.as_str()) {
                    break next;
                }
            };
            let expires_at = now + VERIFICATION_SECONDS;
            tx.execute("UPDATE registration_requests SET verification_code=?,expires_at=?,failed_attempts=0 WHERE request_id=?", params![verification,expires_at,input.request_id]).map_err(|_| storage())?;
            tx.commit().map_err(|_| storage())?;
            (
                send,
                verification,
                pending_reply(&input.request_id, expires_at),
            )
        };
        send.verification(VerificationPurpose::Registration, &verification)
            .map_err(|error| Failure {
                error,
                request: Some(reply.clone()),
            })?;
        Ok(reply)
    }
    pub(crate) fn complete(&self, input: Complete, now: i64) -> Result<Value, ToolError> {
        reserve(&self.completions)?;
        let mut control = self.control.lock().unwrap();
        let tx = control
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        let pending = request(&tx, &input.request_id)?;
        if pending.expires_at <= now {
            return Err(expired());
        }
        // The receipt survives clearing both credentials and does not create a session.
        if pending.completed.is_some() {
            return Ok(completed_reply(&input.request_id));
        }
        if pending.failures >= MAX_VERIFICATION_ATTEMPTS {
            return Err(error(
                "REGISTRATION_ATTEMPTS_EXCEEDED",
                "rate_limit",
                "Request a new verification code",
            ));
        }
        if pending.code.as_deref() != Some(input.verification_code.as_str()) {
            tx.execute("UPDATE registration_requests SET failed_attempts=failed_attempts+1 WHERE request_id=?", [&input.request_id]).map_err(|_| storage())?;
            tx.commit().map_err(|_| storage())?;
            return Err(error(
                "REGISTRATION_CODE_INVALID",
                "validation",
                "Verification code is incorrect",
            ));
        }
        check_email(&tx, &pending.email)?;
        check_invite(&tx, &pending.invite_id)?;
        let owner = format!("reader_{}", uuid::Uuid::now_v7());
        auth::write_password(
            &tx,
            &owner,
            pending.hash.as_deref().ok_or_else(storage)?,
            true,
        )?;
        tx.execute(
            "UPDATE users SET email=?,email_verified_at=? WHERE user_id=?",
            params![pending.email, now, owner],
        )
        .map_err(|_| storage())?;
        tx.execute(
            "UPDATE beta_invites SET state='used',used_by=?,used_at=? WHERE invite_id=?",
            params![owner, now, pending.invite_id],
        )
        .map_err(|_| storage())?;
        tx.execute("UPDATE registration_requests SET completed_user_id=?,password_hash=NULL,verification_code=NULL WHERE request_id=?", params![owner,input.request_id]).map_err(|_| storage())?;
        tx.commit().map_err(|_| storage())?;
        Ok(completed_reply(&input.request_id))
    }
}

#[cfg(test)]
#[path = "account_registration_tests.rs"]
pub(crate) mod tests;
