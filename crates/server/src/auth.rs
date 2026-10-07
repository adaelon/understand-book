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

impl ControlStore {
    /// Trusted administrator entry, also used by the offline account tool.
    pub fn provision_password(
        &mut self,
        owner: &str,
        password: &str,
        create: bool,
    ) -> Result<(), ToolError> {
        validate_user_id(owner)?;
        if !(12..=1024).contains(&password.len()) {
            return Err(error(
                "PASSWORD_INVALID",
                "validation",
                "Use a password of 12 to 1024 bytes",
            ));
        }
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| storage())?
            .to_string();
        let tx = self.connection.transaction().map_err(|_| storage())?;
        let changed = if create {
            tx.execute(
                "INSERT INTO users(user_id,password_hash) VALUES(?,?)",
                params![owner, hash],
            )
        } else {
            tx.execute(
                "UPDATE users SET password_hash=?,auth_epoch=auth_epoch+1 WHERE user_id=?",
                params![hash, owner],
            )
        }
        .map_err(|_| storage())?;
        if changed != 1 {
            return Err(unauthenticated());
        }
        tx.execute("DELETE FROM auth_sessions WHERE owner_user_id=?", [owner])
            .map_err(|_| storage())?;
        tx.commit().map_err(|_| storage())
    }
    /// Disabling or re-enabling never revives old cookies.
    pub fn set_user_disabled(&mut self, owner: &str, disabled: bool) -> Result<(), ToolError> {
        let tx = self.connection.transaction().map_err(|_| storage())?;
        if tx
            .execute(
                "UPDATE users SET disabled=?,auth_epoch=auth_epoch+1 WHERE user_id=?",
                params![disabled, owner],
            )
            .map_err(|_| storage())?
            != 1
        {
            return Err(unauthenticated());
        }
        tx.execute("DELETE FROM auth_sessions WHERE owner_user_id=?", [owner])
            .map_err(|_| storage())?;
        tx.commit().map_err(|_| storage())
    }
    pub fn revoke_user_sessions(&mut self, owner: &str) -> Result<(), ToolError> {
        let tx = self.connection.transaction().map_err(|_| storage())?;
        if tx
            .execute(
                "UPDATE users SET auth_epoch=auth_epoch+1 WHERE user_id=?",
                [owner],
            )
            .map_err(|_| storage())?
            != 1
        {
            return Err(unauthenticated());
        }
        tx.execute("DELETE FROM auth_sessions WHERE owner_user_id=?", [owner])
            .map_err(|_| storage())?;
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
        owner: &str,
        password: &str,
        previous: Option<&str>,
        now: i64,
    ) -> Result<String, ToolError> {
        if validate_user_id(owner).is_err() || password.len() > 1024 {
            return Err(unauthenticated());
        }
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
