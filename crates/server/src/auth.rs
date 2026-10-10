//! Application identity for the explicit multi-user host. No proxy identity fallback.
use crate::control_store::ControlStore;
use crate::user_storage_paths::{error, validate_user_id};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand_core::{OsRng, RngCore};
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::Mutex,
    time::{Duration, Instant},
};

pub const COOKIE: &str = "__Host-ub_session";
pub const SESSION_SECONDS: i64 = 12 * 60 * 60;
const MAX_SESSIONS: usize = 8;

// Only this module constructs identity; it cannot be deserialized from a request.
#[derive(Clone)]
pub struct Principal {
    user_id: String,
    auth_session_id: Vec<u8>,
    auth_epoch: i64,
}
impl Principal {
    pub fn user_id(&self) -> &str {
        &self.user_id
    }
    pub(crate) fn auth_epoch(&self) -> i64 {
        self.auth_epoch
    }
    pub(crate) fn validate_in(&self, db: &rusqlite::Connection, now: i64) -> Result<(), ToolError> {
        let valid: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM auth_sessions s JOIN users u ON u.user_id=s.owner_user_id WHERE s.token_digest=? AND s.owner_user_id=? AND s.auth_epoch=? AND s.expires_at>? AND u.disabled=0 AND u.auth_epoch=s.auth_epoch)",
            params![self.auth_session_id, self.user_id, self.auth_epoch, now], |r| r.get(0),
        ).map_err(|_| storage())?;
        if valid {
            Ok(())
        } else {
            Err(unauthenticated())
        }
    }
}

pub(crate) fn unauthenticated() -> ToolError {
    error(
        "AUTH_REQUIRED",
        "authentication",
        "Sign in with an active account",
    )
}
fn storage() -> ToolError {
    error(
        "AUTH_STORAGE_UNAVAILABLE",
        "unavailable",
        "Account state could not be read or committed",
    )
}
fn limited() -> ToolError {
    error(
        "LOGIN_RATE_LIMITED",
        "rate_limit",
        "Too many sign-in attempts; try again later",
    )
}
fn digest(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}
pub(crate) fn csrf(token: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"understand-book/csrf/v1\0");
    h.update(token.as_bytes());
    URL_SAFE_NO_PAD.encode(h.finalize())
}
pub(crate) fn cookie(token: &str, clear: bool) -> String {
    format!(
        "{COOKIE}={token}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age={}",
        if clear { 0 } else { SESSION_SECONDS }
    )
}
pub(crate) fn token_from_cookie(value: &str) -> Result<&str, ToolError> {
    let mut tokens = value
        .split(';')
        .filter_map(|p| p.trim().split_once('='))
        .filter(|(key, _)| *key == COOKIE)
        .map(|(_, value)| value);
    let token = tokens.next().ok_or_else(unauthenticated)?;
    if tokens.next().is_some()
        || token.len() != 43
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(unauthenticated());
    }
    Ok(token)
}

pub(crate) fn encode_password(password: &str) -> Result<String, ToolError> {
    if !(12..=1024).contains(&password.len()) {
        return Err(error(
            "PASSWORD_INVALID",
            "validation",
            "Use a password of 12 to 1024 bytes",
        ));
    }
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| storage())
}

// The caller owns the transaction, so online mutations and their receipts commit together.
pub(crate) fn write_password(
    db: &rusqlite::Connection,
    owner: &str,
    hash: &str,
    create: bool,
) -> Result<(), ToolError> {
    let changed = if create {
        db.execute(
            "INSERT INTO users(user_id,password_hash) VALUES(?,?)",
            params![owner, hash],
        )
    } else {
        db.execute(
            "UPDATE users SET password_hash=?,auth_epoch=auth_epoch+1 WHERE user_id=?",
            params![hash, owner],
        )
    }
    .map_err(|_| storage())?;
    if changed != 1 {
        return Err(unauthenticated());
    }
    db.execute("DELETE FROM auth_sessions WHERE owner_user_id=?", [owner])
        .map_err(|_| storage())?;
    Ok(())
}

pub(crate) fn write_session_revocation(
    db: &rusqlite::Connection,
    owner: &str,
    disabled: Option<bool>,
) -> Result<(), ToolError> {
    let changed = if let Some(disabled) = disabled {
        db.execute(
            "UPDATE users SET disabled=?,auth_epoch=auth_epoch+1 WHERE user_id=?",
            params![disabled, owner],
        )
    } else {
        db.execute(
            "UPDATE users SET auth_epoch=auth_epoch+1 WHERE user_id=?",
            [owner],
        )
    }
    .map_err(|_| storage())?;
    if changed != 1 {
        return Err(unauthenticated());
    }
    db.execute("DELETE FROM auth_sessions WHERE owner_user_id=?", [owner])
        .map_err(|_| storage())?;
    Ok(())
}

impl ControlStore {
    /// Trusted offline role provisioning under ServiceWriter. An enabled account
    /// with login credentials is required for granting administration.
    pub fn set_reader_admin(&mut self, owner: &str, admin: bool) -> Result<(), ToolError> {
        validate_user_id(owner)?;
        let changed = self.connection.execute(
            "UPDATE users SET is_admin=? WHERE user_id=? AND (?=0 OR (disabled=0 AND password_hash IS NOT NULL))",
            params![admin, owner, admin],
        ).map_err(|_| storage())?;
        if changed != 1 {
            return Err(error(
                "ADMIN_ACCOUNT_UNAVAILABLE",
                "validation",
                "Choose an existing active account with a password",
            ));
        }
        Ok(())
    }

    /// Trusted administrator entry, also used by the offline account tool.
    pub fn provision_password(
        &mut self,
        owner: &str,
        password: &str,
        create: bool,
    ) -> Result<(), ToolError> {
        validate_user_id(owner)?;
        let hash = encode_password(password)?;
        let tx = self.connection.transaction().map_err(|_| storage())?;
        write_password(&tx, owner, &hash, create)?;
        tx.commit().map_err(|_| storage())
    }
    /// Disabling or re-enabling never revives old cookies.
    pub fn set_user_disabled(&mut self, owner: &str, disabled: bool) -> Result<(), ToolError> {
        let tx = self.connection.transaction().map_err(|_| storage())?;
        write_session_revocation(&tx, owner, Some(disabled))?;
        tx.commit().map_err(|_| storage())
    }
    pub fn revoke_user_sessions(&mut self, owner: &str) -> Result<(), ToolError> {
        let tx = self.connection.transaction().map_err(|_| storage())?;
        write_session_revocation(&tx, owner, None)?;
        tx.commit().map_err(|_| storage())
    }
}

struct Attempts {
    start: Instant,
    total: usize,
    users: BTreeMap<String, usize>,
}
impl Attempts {
    fn reserve(&mut self, owner: &str, now: Instant) -> Result<(), ToolError> {
        if now.saturating_duration_since(self.start) >= Duration::from_secs(60) {
            self.start = now;
            self.total = 0;
            self.users.clear();
        }
        // A global bound also bounds unknown-account map size and proxy-shared traffic.
        if self.total >= 60 || self.users.get(owner).copied().unwrap_or(0) >= 5 {
            return Err(limited());
        }
        self.total += 1;
        *self.users.entry(owner.to_owned()).or_default() += 1;
        Ok(())
    }
}

pub struct AuthService {
    control: Mutex<ControlStore>,
    attempts: Mutex<Attempts>,
    password_slot: Mutex<()>,
}
impl AuthService {
    pub fn new(control: ControlStore) -> Self {
        Self {
            control: Mutex::new(control),
            attempts: Mutex::new(Attempts {
                start: Instant::now(),
                total: 0,
                users: BTreeMap::new(),
            }),
            password_slot: Mutex::new(()),
        }
    }
    pub(crate) fn login(
        &self,
        username: &str,
        password: &str,
        previous: Option<&str>,
        now: i64,
    ) -> Result<String, ToolError> {
        if password.len() > 1024 {
            return Err(unauthenticated());
        }
        let owner = if username.contains('@') {
            let email =
                crate::account_mail::normalize_email(username).map_err(|_| unauthenticated())?;
            // Resolve before reserving attempts so email and legacy name share one budget.
            self.control
                .lock()
                .unwrap()
                .connection
                .query_row(
                    "SELECT user_id FROM users WHERE email=? AND email_verified_at IS NOT NULL",
                    [&email],
                    |r| r.get::<_, String>(0),
                )
                .optional()
                .map_err(|_| storage())?
                .unwrap_or(email)
        } else {
            validate_user_id(username).map_err(|_| unauthenticated())?;
            username.to_owned()
        };
        let owner = owner.as_str();
        self.attempts
            .lock()
            .unwrap()
            .reserve(owner, Instant::now())?;
        let _slot = self.password_slot.try_lock().map_err(|_| limited())?;
        let credentials: Option<(String, i64)> = self.control.lock().unwrap().connection.query_row(
            "SELECT password_hash,auth_epoch FROM users WHERE user_id=? AND disabled=0 AND password_hash IS NOT NULL", [owner], |r| Ok((r.get(0)?, r.get(1)?))
        ).optional().map_err(|_| storage())?;
        let (hash, epoch) = credentials.ok_or_else(unauthenticated)?;
        // Expensive hashing runs without the database/authentication mutex.
        let hash_parsed = PasswordHash::new(&hash).map_err(|_| storage())?;
        Argon2::default()
            .verify_password(password.as_bytes(), &hash_parsed)
            .map_err(|_| unauthenticated())?;
        let mut bytes = [0u8; 32];
        OsRng.try_fill_bytes(&mut bytes).map_err(|_| storage())?;
        let token = URL_SAFE_NO_PAD.encode(bytes);
        let mut control = self.control.lock().unwrap();
        let tx = control.connection.transaction().map_err(|_| storage())?;
        let current: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM users WHERE user_id=? AND disabled=0 AND auth_epoch=? AND password_hash=?)", params![owner, epoch, hash], |r| r.get(0)).map_err(|_| storage())?;
        if !current {
            return Err(unauthenticated());
        }
        tx.execute("DELETE FROM auth_sessions WHERE expires_at<=?", [now])
            .map_err(|_| storage())?;
        if let Some(previous) = previous {
            tx.execute(
                "DELETE FROM auth_sessions WHERE token_digest=?",
                [digest(previous)],
            )
            .map_err(|_| storage())?;
        }
        tx.execute("DELETE FROM auth_sessions WHERE token_digest IN (SELECT token_digest FROM auth_sessions WHERE owner_user_id=? ORDER BY expires_at DESC, rowid DESC LIMIT -1 OFFSET ?)", params![owner, MAX_SESSIONS - 1]).map_err(|_| storage())?;
        tx.execute("INSERT INTO auth_sessions(token_digest,owner_user_id,auth_epoch,expires_at) VALUES(?,?,?,?)", params![digest(&token), owner, epoch, now + SESSION_SECONDS]).map_err(|_| storage())?;
        tx.commit().map_err(|_| storage())?;
        Ok(token)
    }
    pub(crate) fn authenticate(&self, token: &str, now: i64) -> Result<Principal, ToolError> {
        let id = digest(token);
        let identity: Option<(String, i64)> = self.control.lock().unwrap().connection.query_row(
            "SELECT s.owner_user_id,s.auth_epoch FROM auth_sessions s JOIN users u ON u.user_id=s.owner_user_id WHERE s.token_digest=? AND s.expires_at>? AND u.disabled=0 AND u.auth_epoch=s.auth_epoch", params![id, now], |r| Ok((r.get(0)?, r.get(1)?))
        ).optional().map_err(|_| storage())?;
        let (user_id, auth_epoch) = identity.ok_or_else(unauthenticated)?;
        Ok(Principal {
            user_id,
            auth_session_id: id,
            auth_epoch,
        })
    }
    pub(crate) fn verify_current_password(
        &self,
        principal: &Principal,
        password: &str,
        now: i64,
    ) -> Result<String, ToolError> {
        self.attempts
            .lock()
            .unwrap()
            .reserve(principal.user_id(), Instant::now())?;
        let invalid = || {
            error(
                "CURRENT_PASSWORD_INVALID",
                "validation",
                "Current password is incorrect",
            )
        };
        if password.len() > 1024 {
            return Err(invalid());
        }
        let _slot = self.password_slot.try_lock().map_err(|_| limited())?;
        let hash: String = {
            let control = self.control.lock().unwrap();
            principal.validate_in(&control.connection, now)?;
            control
                .connection
                .query_row(
                    "SELECT password_hash FROM users WHERE user_id=?",
                    [principal.user_id()],
                    |r| r.get(0),
                )
                .map_err(|_| storage())?
        };
        let parsed = PasswordHash::new(&hash).map_err(|_| storage())?;
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .map_err(|_| invalid())?;
        Ok(hash)
    }
    pub(crate) fn email(
        &self,
        principal: &Principal,
        now: i64,
    ) -> Result<Option<String>, ToolError> {
        let control = self.control.lock().unwrap();
        principal.validate_in(&control.connection, now)?;
        control
            .connection
            .query_row(
                "SELECT email FROM users WHERE user_id=? AND email_verified_at IS NOT NULL",
                [principal.user_id()],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(|_| storage())
    }
    pub fn validate(&self, principal: &Principal, now: i64) -> Result<(), ToolError> {
        let valid: bool = self.control.lock().unwrap().connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM auth_sessions s JOIN users u ON u.user_id=s.owner_user_id WHERE s.token_digest=? AND s.owner_user_id=? AND s.auth_epoch=? AND s.expires_at>? AND u.disabled=0 AND u.auth_epoch=s.auth_epoch)", params![principal.auth_session_id, principal.user_id, principal.auth_epoch, now], |r| r.get(0)
        ).map_err(|_| storage())?;
        if valid {
            Ok(())
        } else {
            Err(unauthenticated())
        }
    }
    /// Read the current role together with session validity; Principal does not
    /// cache privileges, so revocation also applies to already-held identities.
    pub fn is_reader_admin(&self, principal: &Principal, now: i64) -> Result<bool, ToolError> {
        self.control.lock().unwrap().connection.query_row(
            "SELECT u.is_admin FROM auth_sessions s JOIN users u ON u.user_id=s.owner_user_id WHERE s.token_digest=? AND s.owner_user_id=? AND s.auth_epoch=? AND s.expires_at>? AND u.disabled=0 AND u.auth_epoch=s.auth_epoch",
            params![principal.auth_session_id, principal.user_id, principal.auth_epoch, now],
            |r| r.get(0),
        ).optional().map_err(|_| storage())?.ok_or_else(unauthenticated)
    }

    pub fn require_reader_admin(&self, principal: &Principal, now: i64) -> Result<(), ToolError> {
        if self.is_reader_admin(principal, now)? {
            Ok(())
        } else {
            Err(error(
                "ADMIN_REQUIRED",
                "permission",
                "Administrator access is required",
            ))
        }
    }
    pub(crate) fn logout(&self, principal: &Principal) -> Result<(), ToolError> {
        self.control
            .lock()
            .unwrap()
            .connection
            .execute(
                "DELETE FROM auth_sessions WHERE token_digest=?",
                [&principal.auth_session_id],
            )
            .map_err(|_| storage())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inv5_email_and_legacy_login_share_identity_role_and_attempt_budget() {
        let root = tempfile::tempdir().unwrap();
        let writer = crate::control_store::ServiceWriter::acquire(root.path()).unwrap();
        let mut store = ControlStore::open(writer.clone()).unwrap();
        store
            .provision_password("legacy", "fixture-only-password", true)
            .unwrap();
        store.set_reader_admin("legacy", true).unwrap();
        store.connection.execute("UPDATE users SET email='reader+tag@example.com',email_verified_at=1 WHERE user_id='legacy'", []).unwrap();
        let auth = AuthService::new(ControlStore::open(writer).unwrap());
        for input in ["legacy", " Reader+Tag@Example.COM "] {
            let token = auth
                .login(input, "fixture-only-password", None, 100)
                .unwrap();
            let principal = auth.authenticate(&token, 101).unwrap();
            assert_eq!(principal.user_id(), "legacy");
            assert!(auth.is_reader_admin(&principal, 101).unwrap());
        }
        for input in [
            "legacy",
            "READER+TAG@example.com",
            " reader+tag@example.com ",
        ] {
            assert_eq!(
                auth.login(input, "wrong-password", None, 100)
                    .unwrap_err()
                    .error_code,
                "AUTH_REQUIRED"
            );
        }
        assert_eq!(
            auth.login("reader+tag@example.com", "fixture-only-password", None, 100)
                .unwrap_err()
                .error_code,
            "LOGIN_RATE_LIMITED"
        );
        auth.attempts.lock().unwrap().start = Instant::now() - Duration::from_secs(61);
        store.set_user_disabled("legacy", true).unwrap();
        assert_eq!(
            auth.login("reader+tag@example.com", "fixture-only-password", None, 100)
                .unwrap_err()
                .error_code,
            "AUTH_REQUIRED"
        );
        assert_eq!(
            auth.login("missing@example.com", "fixture-only-password", None, 100)
                .unwrap_err()
                .error_code,
            "AUTH_REQUIRED"
        );
    }
    #[test]
    fn adm1_admin_role_is_current_and_obeys_all_session_revocations() {
        let root = tempfile::tempdir().unwrap();
        let writer = crate::control_store::ServiceWriter::acquire(root.path()).unwrap();
        let mut control = ControlStore::open(writer.clone()).unwrap();
        control
            .provision_password("A", "fixture-only-password", true)
            .unwrap();
        control
            .provision_password("B", "fixture-only-password", true)
            .unwrap();
        control.create_user("no_password").unwrap();
        assert!(control.set_reader_admin("no_password", true).is_err());
        assert!(control.set_reader_admin("missing", true).is_err());
        let auth = AuthService::new(ControlStore::open(writer.clone()).unwrap());
        let token = auth.login("A", "fixture-only-password", None, 100).unwrap();
        let principal = auth.authenticate(&token, 101).unwrap();
        assert!(!auth.is_reader_admin(&principal, 101).unwrap());
        assert_eq!(
            auth.require_reader_admin(&principal, 101)
                .unwrap_err()
                .error_code,
            "ADMIN_REQUIRED"
        );
        control.set_reader_admin("A", true).unwrap();
        control.set_reader_admin("A", true).unwrap();
        auth.require_reader_admin(&principal, 101).unwrap();
        let b_token = auth.login("B", "fixture-only-password", None, 100).unwrap();
        let b = auth.authenticate(&b_token, 101).unwrap();
        assert!(!auth.is_reader_admin(&b, 101).unwrap());
        control.set_reader_admin("A", false).unwrap();
        assert!(!auth.is_reader_admin(&principal, 101).unwrap());
        control.set_reader_admin("A", true).unwrap();
        drop(auth);
        let auth = AuthService::new(ControlStore::open(writer).unwrap());
        auth.require_reader_admin(&principal, 101).unwrap();
        assert_eq!(
            auth.is_reader_admin(&principal, 100 + SESSION_SECONDS)
                .unwrap_err()
                .error_code,
            "AUTH_REQUIRED"
        );
        control.set_user_disabled("A", true).unwrap();
        assert!(control.set_reader_admin("A", true).is_err());
        assert!(auth.is_reader_admin(&principal, 101).is_err());
        control.set_user_disabled("A", false).unwrap();
        assert!(auth.authenticate(&token, 101).is_err());
        let token = auth.login("A", "fixture-only-password", None, 102).unwrap();
        let principal = auth.authenticate(&token, 103).unwrap();
        auth.require_reader_admin(&principal, 103).unwrap();
        control
            .provision_password("A", "replacement-password", false)
            .unwrap();
        assert!(auth.is_reader_admin(&principal, 103).is_err());
        let token = auth.login("A", "replacement-password", None, 104).unwrap();
        let principal = auth.authenticate(&token, 105).unwrap();
        control.revoke_user_sessions("A").unwrap();
        assert!(auth.is_reader_admin(&principal, 105).is_err());
        let token = auth.login("A", "replacement-password", None, 106).unwrap();
        let principal = auth.authenticate(&token, 107).unwrap();
        auth.logout(&principal).unwrap();
        assert!(auth.is_reader_admin(&principal, 107).is_err());
    }

    #[test]
    fn mu4_global_attempt_budget_resets_and_does_not_grow_unbounded() {
        let start = Instant::now();
        let mut attempts = Attempts {
            start,
            total: 0,
            users: BTreeMap::new(),
        };
        for n in 0..60 {
            attempts.reserve(&format!("unknown-{n}"), start).unwrap();
        }
        assert!(attempts.reserve("different", start).is_err());
        assert_eq!(attempts.users.len(), 60);
        attempts
            .reserve("different", start + Duration::from_secs(60))
            .unwrap();
        assert_eq!(attempts.users.len(), 1);
    }
    #[test]
    fn mu4_session_cap_prunes_expired_and_hash_slot_does_not_block_validation() {
        let root = tempfile::tempdir().unwrap();
        let writer = crate::control_store::ServiceWriter::acquire(root.path()).unwrap();
        let mut store = ControlStore::open(writer.clone()).unwrap();
        store
            .provision_password("A", "fixture-only-password", true)
            .unwrap();
        let service = AuthService::new(store);
        let mut tokens = Vec::new();
        for n in 0..9 {
            service.attempts.lock().unwrap().start = Instant::now() - Duration::from_secs(61);
            tokens.push(
                service
                    .login("A", "fixture-only-password", None, 100 + n)
                    .unwrap(),
            );
        }
        assert!(service.authenticate(&tokens[0], 110).is_err());
        assert!(service.authenticate(&tokens[1], 110).is_ok());
        assert_eq!(
            service
                .control
                .lock()
                .unwrap()
                .connection
                .query_row("SELECT count(*) FROM auth_sessions", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            8
        );
        let _slot = service.password_slot.lock().unwrap();
        assert!(service
            .login("A", "fixture-only-password", None, 110)
            .is_err());
        let principal = service.authenticate(&tokens[8], 110).unwrap();
        assert!(service.validate(&principal, 110).is_ok());
        service.logout(&principal).unwrap();
        assert!(service.validate(&principal, 110).is_err());
        drop(_slot);
        service
            .login(
                "A",
                "fixture-only-password",
                None,
                100 + SESSION_SECONDS + 10,
            )
            .unwrap();
        assert_eq!(
            service
                .control
                .lock()
                .unwrap()
                .connection
                .query_row("SELECT count(*) FROM auth_sessions", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
