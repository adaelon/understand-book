//! First email binding changes only the existing account's verified address.
use crate::{
    account_mail::{
        self, AccountMail, VerificationPurpose, MAX_VERIFICATION_ATTEMPTS, RESEND_SECONDS,
        VERIFICATION_SECONDS,
    },
    auth::{AuthService, Principal},
    control_store::ControlStore,
    user_storage_paths::error,
};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Mutex;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Start {
    email: String,
    current_password: String,
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
        "EMAIL_BINDING_STORAGE_UNAVAILABLE",
        "unavailable",
        "Email binding could not be committed",
    )
}
fn stale() -> ToolError {
    error(
        "EMAIL_BINDING_EXPIRED",
        "conflict",
        "Start a new email binding request",
    )
}
fn check_available(db: &Connection, principal: &Principal, email: &str) -> Result<(), ToolError> {
    let bound: bool = db
        .query_row(
            "SELECT email IS NOT NULL FROM users WHERE user_id=?",
            [principal.user_id()],
            |r| r.get(0),
        )
        .map_err(|_| storage())?;
    if bound {
        return Err(error(
            "ACCOUNT_EMAIL_ALREADY_BOUND",
            "conflict",
            "Account already has a verified email",
        ));
    }
    let taken: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE email=?)",
            [email],
            |r| r.get(0),
        )
        .map_err(|_| storage())?;
    if taken {
        return Err(error(
            "ACCOUNT_EMAIL_IN_USE",
            "conflict",
            "Email already belongs to an account",
        ));
    }
    Ok(())
}
fn pending_reply(id: &str, expires_at: i64) -> Value {
    json!({"request_id":id,"expires_at":expires_at,"resend_after":RESEND_SECONDS})
}
fn completed_reply(principal: &Principal, email: &str) -> Value {
    json!({"user_id":principal.user_id(),"email":email,"completed":true})
}
struct Pending {
    email: String,
    code: String,
    expires: i64,
    failures: u32,
    epoch: i64,
    used: Option<i64>,
}
fn request(
    db: &Connection,
    principal: &Principal,
    id: &str,
    now: i64,
) -> Result<Pending, ToolError> {
    principal.validate_in(db, now)?;
    let pending = db.query_row("SELECT email,verification_code,expires_at,failed_attempts,auth_epoch,used_at FROM email_binding_requests WHERE request_id=? AND owner_user_id=?", params![id,principal.user_id()], |r| Ok(Pending {
        email:r.get(0)?, code:r.get(1)?, expires:r.get(2)?, failures:r.get(3)?, epoch:r.get(4)?, used:r.get(5)?,
    })).optional().map_err(|_| storage())?.ok_or_else(|| error("EMAIL_BINDING_NOT_FOUND", "not_found", "Email binding request is unavailable"))?;
    if pending.expires <= now || pending.epoch != principal.auth_epoch() {
        return Err(stale());
    }
    Ok(pending)
}

pub(crate) struct EmailBinding {
    pub(crate) control: Mutex<ControlStore>,
}
impl EmailBinding {
    pub(crate) fn new(control: ControlStore) -> Self {
        Self {
            control: Mutex::new(control),
        }
    }
    pub(crate) fn start(
        &self,
        principal: &Principal,
        input: Start,
        auth: &AuthService,
        mail: &AccountMail,
        now: i64,
    ) -> Result<Value, Failure> {
        let email = account_mail::normalize_email(&input.email)?;
        auth.verify_current_password(principal, &input.current_password, now)?;
        let code = account_mail::verification_code()?;
        let id = uuid::Uuid::now_v7().to_string();
        let expires = now + VERIFICATION_SECONDS;
        let send = {
            let mut control = self.control.lock().unwrap();
            let tx = control
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| storage())?;
            principal.validate_in(&tx, now)?;
            check_available(&tx, principal, &email)?;
            let send = mail.reserve(&email)?;
            // A corrected email starts a new request and supersedes this owner's earlier one.
            tx.execute("DELETE FROM email_binding_requests WHERE owner_user_id=? OR expires_at<=? OR used_at IS NOT NULL", params![principal.user_id(),now]).map_err(|_| storage())?;
            tx.execute("INSERT INTO email_binding_requests(request_id,owner_user_id,auth_epoch,email,verification_code,expires_at) VALUES(?,?,?,?,?,?)", params![id,principal.user_id(),principal.auth_epoch(),email,code,expires]).map_err(|_| storage())?;
            tx.commit().map_err(|_| storage())?;
            send
        };
        let reply = pending_reply(&id, expires);
        send.verification(VerificationPurpose::EmailBinding, &code)
            .map_err(|error| Failure {
                error,
                request: Some(reply.clone()),
            })?;
        Ok(reply)
    }
    pub(crate) fn resend(
        &self,
        principal: &Principal,
        input: Resend,
        mail: &AccountMail,
        now: i64,
    ) -> Result<Value, Failure> {
        let (send, code, reply) = {
            let mut control = self.control.lock().unwrap();
            let tx = control
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| storage())?;
            let pending = request(&tx, principal, &input.request_id, now)?;
            if pending.used.is_some() {
                return Ok(completed_reply(principal, &pending.email));
            }
            check_available(&tx, principal, &pending.email)?;
            let send = mail.reserve(&pending.email)?;
            let code = loop {
                let next = account_mail::verification_code()?;
                if next != pending.code {
                    break next;
                }
            };
            let expires = now + VERIFICATION_SECONDS;
            tx.execute("UPDATE email_binding_requests SET verification_code=?,expires_at=?,failed_attempts=0 WHERE request_id=?", params![code,expires,input.request_id]).map_err(|_| storage())?;
            tx.commit().map_err(|_| storage())?;
            (send, code, pending_reply(&input.request_id, expires))
        };
        send.verification(VerificationPurpose::EmailBinding, &code)
            .map_err(|error| Failure {
                error,
                request: Some(reply.clone()),
            })?;
        Ok(reply)
    }
    pub(crate) fn complete(
        &self,
        principal: &Principal,
        input: Complete,
        now: i64,
    ) -> Result<Value, ToolError> {
        let mut control = self.control.lock().unwrap();
        let tx = control
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        let pending = request(&tx, principal, &input.request_id, now)?;
        if pending.used.is_some() {
            return Ok(completed_reply(principal, &pending.email));
        }
        check_available(&tx, principal, &pending.email)?;
        if pending.failures >= MAX_VERIFICATION_ATTEMPTS {
            return Err(error(
                "EMAIL_BINDING_ATTEMPTS_EXCEEDED",
                "rate_limit",
                "Request a new verification code",
            ));
        }
        if pending.code != input.verification_code {
            tx.execute("UPDATE email_binding_requests SET failed_attempts=failed_attempts+1 WHERE request_id=?", [&input.request_id]).map_err(|_| storage())?;
            tx.commit().map_err(|_| storage())?;
            return Err(error(
                "EMAIL_BINDING_CODE_INVALID",
                "validation",
                "Verification code is incorrect",
            ));
        }
        tx.execute(
            "UPDATE users SET email=?,email_verified_at=? WHERE user_id=?",
            params![pending.email, now, principal.user_id()],
        )
        .map_err(|_| storage())?;
        tx.execute(
            "UPDATE email_binding_requests SET used_at=? WHERE request_id=?",
            params![now, input.request_id],
        )
        .map_err(|_| storage())?;
        tx.commit().map_err(|_| storage())?;
        Ok(completed_reply(principal, &pending.email))
    }
}
