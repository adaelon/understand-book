//! Account-owned MCP credentials. Remote identity never grants a web Principal.
use crate::{
    auth::Principal, authorization, control_store::ControlStore, user_storage_paths::error,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand_core::{OsRng, RngCore};
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Mutex;

const TOKEN_PREFIX: &str = "ub_mcp_";

/// An authenticated MCP caller, separate from browser/private-workspace authority.
#[derive(Debug)]
pub struct McpIdentity {
    user_id: String,
    credential_id: String,
}
impl McpIdentity {
    pub fn user_id(&self) -> &str {
        &self.user_id
    }
    pub fn credential_id(&self) -> &str {
        &self.credential_id
    }
}

#[derive(Debug, Serialize)]
pub struct Credential {
    pub credential_id: String,
    pub label: String,
    pub created_at: i64,
    pub revoked_at: Option<i64>,
    /// False also covers an old account authorization epoch.
    pub active: bool,
}

// Deliberately not Debug: the full token is returned only by creation.
#[derive(Serialize)]
pub struct CreatedCredential {
    pub credential: Credential,
    pub token: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Create {
    pub label: String,
}

fn storage() -> ToolError {
    error(
        "MCP_ACCESS_STORAGE_UNAVAILABLE",
        "unavailable",
        "MCP credentials could not be read or committed",
    )
}
fn unauthenticated() -> ToolError {
    error(
        "MCP_AUTH_REQUIRED",
        "authentication",
        "Use an active MCP access credential",
    )
}
fn digest(token: &str) -> Vec<u8> {
    // Same at-rest token verification convention as auth_sessions.
    Sha256::digest(token.as_bytes()).to_vec()
}

pub struct McpAccess {
    control: Mutex<ControlStore>,
}
impl McpAccess {
    pub fn new(control: ControlStore) -> Self {
        Self {
            control: Mutex::new(control),
        }
    }

    pub fn create(
        &self,
        principal: &Principal,
        label: &str,
        now: i64,
    ) -> Result<CreatedCredential, ToolError> {
        let label = label.trim();
        if label.is_empty() || label.chars().count() > 80 {
            return Err(error(
                "MCP_LABEL_INVALID",
                "validation",
                "Use a credential label of 1 to 80 characters",
            ));
        }
        let mut bytes = [0u8; 32];
        OsRng.try_fill_bytes(&mut bytes).map_err(|_| storage())?;
        let token = format!("{TOKEN_PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes));
        let credential = Credential {
            credential_id: uuid::Uuid::now_v7().to_string(),
            label: label.into(),
            created_at: now,
            revoked_at: None,
            active: true,
        };
        let mut control = self.control.lock().unwrap();
        let tx = control
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        principal.validate_in(&tx, now)?;
        tx.execute(
            "INSERT INTO mcp_credentials(credential_id,owner_user_id,auth_epoch,token_digest,label,created_at) VALUES(?,?,?,?,?,?)",
            params![credential.credential_id, principal.user_id(), principal.auth_epoch(), digest(&token), label, now],
        ).map_err(|_| storage())?;
        tx.commit().map_err(|_| storage())?;
        Ok(CreatedCredential { credential, token })
    }

    pub fn list(&self, principal: &Principal, now: i64) -> Result<Vec<Credential>, ToolError> {
        let mut control = self.control.lock().unwrap();
        let tx = control.connection.transaction().map_err(|_| storage())?;
        principal.validate_in(&tx, now)?;
        let credentials = {
            let mut query = tx.prepare(
                "SELECT credential_id,label,created_at,revoked_at,revoked_at IS NULL AND auth_epoch=? FROM mcp_credentials WHERE owner_user_id=? ORDER BY created_at DESC,credential_id DESC",
            ).map_err(|_| storage())?;
            let rows = query
                .query_map(params![principal.auth_epoch(), principal.user_id()], |r| {
                    Ok(Credential {
                        credential_id: r.get(0)?,
                        label: r.get(1)?,
                        created_at: r.get(2)?,
                        revoked_at: r.get(3)?,
                        active: r.get(4)?,
                    })
                })
                .map_err(|_| storage())?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|_| storage())?
        };
        tx.commit().map_err(|_| storage())?;
        Ok(credentials)
    }

    pub fn revoke(
        &self,
        principal: &Principal,
        credential_id: &str,
        now: i64,
    ) -> Result<(), ToolError> {
        let mut control = self.control.lock().unwrap();
        let tx = control
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        principal.validate_in(&tx, now)?;
        let changed = tx.execute(
            "UPDATE mcp_credentials SET revoked_at=coalesce(revoked_at,?) WHERE credential_id=? AND owner_user_id=?",
            params![now, credential_id, principal.user_id()],
        ).map_err(|_| storage())?;
        if changed != 1 {
            return Err(authorization::missing());
        }
        tx.commit().map_err(|_| storage())
    }

    /// Resolve afresh on every remote request; Cookie and CSRF are not MCP credentials.
    pub fn authenticate(&self, authorization: Option<&str>) -> Result<McpIdentity, ToolError> {
        let (scheme, token) = authorization
            .and_then(|value| value.split_once(' '))
            .ok_or_else(unauthenticated)?;
        let secret = token
            .strip_prefix(TOKEN_PREFIX)
            .ok_or_else(unauthenticated)?;
        if !scheme.eq_ignore_ascii_case("Bearer")
            || secret.len() != 43
            || !secret
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(unauthenticated());
        }
        self.control.lock().unwrap().connection.query_row(
            "SELECT c.owner_user_id,c.credential_id FROM mcp_credentials c JOIN users u ON u.user_id=c.owner_user_id WHERE c.token_digest=? AND c.revoked_at IS NULL AND u.disabled=0 AND u.auth_epoch=c.auth_epoch",
            [digest(token)],
            |r| Ok(McpIdentity { user_id: r.get(0)?, credential_id: r.get(1)? }),
        ).optional().map_err(|_| storage())?.ok_or_else(unauthenticated)
    }
}

#[cfg(test)]
#[path = "mcp_access_tests.rs"]
mod tests;
