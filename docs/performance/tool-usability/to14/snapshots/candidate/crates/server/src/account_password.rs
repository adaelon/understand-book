//! Password changes and reset-token consumption share the credential transaction.
use crate::{
    account_mail::{self, AccountMail, RESET_SECONDS},
    auth::{self, AuthService, Principal},
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
pub(crate) struct Change {
    current_password: String,
    new_password: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Forgot {
    email: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Reset {
    token: String,
    new_password: String,
}

fn storage() -> ToolError {
    error(
        "PASSWORD_STORAGE_UNAVAILABLE",
        "unavailable",
        "Password request could not be committed",
    )
}
fn stale() -> ToolError {
    error(
        "PASSWORD_RESET_INVALID",
        "conflict",
        "Reset link is expired or unavailable; request a new link",
    )
}
fn limited() -> ToolError {
    error(
        "PASSWORD_RATE_LIMITED",
        "rate_limit",
        "Too many password requests; try again later",
    )
}
fn owner(db: &Connection, token: &str, now: i64) -> Result<String, ToolError> {
    db.query_row("SELECT u.user_id FROM password_reset_requests r JOIN users u ON u.user_id=r.owner_user_id WHERE r.token=? AND r.used_at IS NULL AND r.expires_at>? AND r.auth_epoch=u.auth_epoch AND u.disabled=0", params![token,now], |r| r.get(0)).optional().map_err(|_| storage())?.ok_or_else(stale)
}
pub(crate) struct AccountPassword {
    pub(crate) control: Mutex<ControlStore>,
    attempts: Mutex<VecDeque<Instant>>,
    password_slot: Mutex<()>,
}
impl AccountPassword {
    pub(crate) fn new(control: ControlStore) -> Self {
        Self {
            control: Mutex::new(control),
            attempts: Mutex::new(VecDeque::new()),
            password_slot: Mutex::new(()),
        }
    }
    fn reserve(&self) -> Result<(), ToolError> {
        let mut attempts = self.attempts.lock().unwrap();
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
    pub(crate) fn change(
        &self,
        principal: &Principal,
        input: Change,
        auth: &AuthService,
        now: i64,
    ) -> Result<Value, ToolError> {
        self.reserve()?;
        let _slot = self.password_slot.try_lock().map_err(|_| limited())?;
        let original = auth.verify_current_password(principal, &input.current_password, now)?;
        let hash = auth::encode_password(&input.new_password)?;
        let mut control = self.control.lock().unwrap();
        let tx = control
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        principal.validate_in(&tx, now)?;
        let unchanged: bool = tx
            .query_row(
                "SELECT password_hash=? FROM users WHERE user_id=?",
                params![original, principal.user_id()],
                |r| r.get(0),
            )
            .map_err(|_| storage())?;
        if !unchanged {
            return Err(auth::unauthenticated());
        }
        auth::write_password(&tx, principal.user_id(), &hash, false)?;
        tx.commit().map_err(|_| storage())?;
        Ok(json!({"completed":true}))
    }
    pub(crate) fn forgot(
        &self,
        input: Forgot,
        mail: &AccountMail,
        now: i64,
    ) -> Result<Value, ToolError> {
        self.reserve()?;
        let email = account_mail::normalize_email(&input.email)?;
        // Reserve for every valid address: missing/disabled accounts share the
        // configured-service and cooldown response without sending a message.
        let send = mail.reserve(&email)?;
        let token = account_mail::reset_token()?;
        let available = {
            let mut control = self.control.lock().unwrap();
            let tx = control
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| storage())?;
            let account: Option<(String,i64)> = tx.query_row("SELECT user_id,auth_epoch FROM users WHERE email=? AND email_verified_at IS NOT NULL AND disabled=0", [&email], |r| Ok((r.get(0)?,r.get(1)?))).optional().map_err(|_| storage())?;
            tx.execute(
                "DELETE FROM password_reset_requests WHERE expires_at<=? OR used_at IS NOT NULL",
                [now],
            )
            .map_err(|_| storage())?;
            if let Some((user, epoch)) = &account {
                tx.execute(
                    "DELETE FROM password_reset_requests WHERE owner_user_id=?",
                    [user],
                )
                .map_err(|_| storage())?;
                tx.execute("INSERT INTO password_reset_requests(token,owner_user_id,auth_epoch,expires_at) VALUES(?,?,?,?)", params![token,user,epoch,now+RESET_SECONDS]).map_err(|_| storage())?;
            }
            tx.commit().map_err(|_| storage())?;
            account.is_some()
        };
        if available {
            send.password_reset(&token)?;
        }
        Ok(json!({"accepted":true}))
    }
    pub(crate) fn reset(&self, input: Reset, now: i64) -> Result<Value, ToolError> {
        self.reserve()?;
        let _slot = self.password_slot.try_lock().map_err(|_| limited())?;
        // Reject unusable links before Argon2, and recheck after it in the write transaction.
        owner(&self.control.lock().unwrap().connection, &input.token, now)?;
        let hash = auth::encode_password(&input.new_password)?;
        let mut control = self.control.lock().unwrap();
        let tx = control
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        let user = owner(&tx, &input.token, now)?;
        auth::write_password(&tx, &user, &hash, false)?;
        tx.execute(
            "UPDATE password_reset_requests SET used_at=? WHERE token=?",
            params![now, input.token],
        )
        .map_err(|_| storage())?;
        tx.commit().map_err(|_| storage())?;
        Ok(json!({"completed":true}))
    }
}
