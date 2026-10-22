//! System-browser consent followed by a PKCE exchange into a separate WebView session.
use crate::auth::{self, AuthService, Principal, SESSION_SECONDS};
use crate::user_storage_paths::error;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand_core::{OsRng, RngCore};
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CODE_SECONDS: i64 = 120;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DesktopAuthorize {
    pub redirect_uri: String,
    pub code_challenge: String,
    pub state: String,
}
#[derive(Serialize)]
pub(crate) struct DesktopAuthorized {
    pub redirect_url: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DesktopExchange {
    pub code: String,
    pub code_verifier: String,
    pub redirect_uri: String,
}
#[derive(Serialize)]
pub(crate) struct DesktopSession {
    pub session_token: String,
    pub expires_in: i64,
}
fn invalid() -> ToolError {
    error(
        "DESKTOP_AUTH_INVALID",
        "validation",
        "Desktop authorization is invalid or expired; start sign-in again",
    )
}
fn storage() -> ToolError {
    error(
        "DESKTOP_AUTH_UNAVAILABLE",
        "unavailable",
        "Desktop sign-in could not be completed",
    )
}
fn random() -> Result<String, ToolError> {
    let mut bytes = [0; 32];
    OsRng.try_fill_bytes(&mut bytes).map_err(|_| storage())?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
fn url_token(value: &str) -> bool {
    value.len() == 43
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
fn callback(value: &str) -> Result<url::Url, ToolError> {
    let url = url::Url::parse(value).map_err(|_| invalid())?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port().is_none()
        || url.path() != "/callback"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid());
    }
    Ok(url)
}
impl AuthService {
    pub(crate) fn desktop_authorize(
        &self,
        principal: &Principal,
        input: DesktopAuthorize,
        now: i64,
    ) -> Result<DesktopAuthorized, ToolError> {
        let mut redirect = callback(&input.redirect_uri)?;
        if !url_token(&input.code_challenge) || !url_token(&input.state) {
            return Err(invalid());
        }
        let code = random()?;
        let mut control = self.control.lock().unwrap();
        let tx = control.connection.transaction().map_err(|_| storage())?;
        principal.validate_in(&tx, now)?;
        tx.execute(
            "DELETE FROM desktop_authorizations WHERE expires_at<=?",
            [now],
        )
        .map_err(|_| storage())?;
        tx.execute("INSERT INTO desktop_authorizations(code,owner_user_id,auth_epoch,redirect_uri,code_challenge,expires_at) VALUES(?,?,?,?,?,?)",
            params![code, principal.user_id(), principal.auth_epoch(), input.redirect_uri, input.code_challenge, now + CODE_SECONDS]).map_err(|_| storage())?;
        tx.commit().map_err(|_| storage())?;
        redirect
            .query_pairs_mut()
            .append_pair("code", &code)
            .append_pair("state", &input.state);
        Ok(DesktopAuthorized {
            redirect_url: redirect.to_string(),
        })
    }

    pub(crate) fn desktop_exchange(
        &self,
        input: DesktopExchange,
        now: i64,
    ) -> Result<DesktopSession, ToolError> {
        callback(&input.redirect_uri)?;
        if !url_token(&input.code)
            || !(43..=128).contains(&input.code_verifier.len())
            || !input
                .code_verifier
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-._~".contains(&c))
        {
            return Err(invalid());
        }
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(input.code_verifier.as_bytes()));
        let mut control = self.control.lock().unwrap();
        let tx = control.connection.transaction().map_err(|_| storage())?;
        let owner: Option<(String, i64)> = tx.query_row(
            "SELECT a.owner_user_id,a.auth_epoch FROM desktop_authorizations a JOIN users u ON u.user_id=a.owner_user_id WHERE a.code=? AND a.redirect_uri=? AND a.code_challenge=? AND a.expires_at>? AND u.disabled=0 AND u.auth_epoch=a.auth_epoch",
            params![input.code, input.redirect_uri, challenge, now], |row| Ok((row.get(0)?, row.get(1)?))
        ).optional().map_err(|_| storage())?;
        let (owner, epoch) = owner.ok_or_else(invalid)?;
        let session_token = random()?;
        tx.execute(
            "DELETE FROM desktop_authorizations WHERE code=?",
            [&input.code],
        )
        .map_err(|_| storage())?;
        tx.execute("DELETE FROM auth_sessions WHERE expires_at<=?", [now])
            .map_err(|_| storage())?;
        tx.execute("DELETE FROM auth_sessions WHERE token_digest IN (SELECT token_digest FROM auth_sessions WHERE owner_user_id=? ORDER BY expires_at DESC,rowid DESC LIMIT -1 OFFSET ?)", params![owner, auth::MAX_SESSIONS - 1]).map_err(|_| storage())?;
        tx.execute("INSERT INTO auth_sessions(token_digest,owner_user_id,auth_epoch,expires_at) VALUES(?,?,?,?)",
            params![auth::digest(&session_token), owner, epoch, now + SESSION_SECONDS]).map_err(|_| storage())?;
        tx.commit().map_err(|_| storage())?;
        Ok(DesktopSession {
            session_token,
            expires_in: SESSION_SECONDS,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control_store::{ControlStore, ServiceWriter};
    fn setup() -> (tempfile::TempDir, AuthService, String) {
        let dir = tempfile::tempdir().unwrap();
        let writer = ServiceWriter::acquire(dir.path()).unwrap();
        let mut store = ControlStore::open(writer.clone()).unwrap();
        store
            .provision_password("student", "fixture-only-password", true)
            .unwrap();
        let auth = AuthService::new(ControlStore::open(writer).unwrap());
        let browser = auth
            .login("student", "fixture-only-password", None, 100)
            .unwrap();
        (dir, auth, browser)
    }
    fn authorize(auth: &AuthService, browser: &str, now: i64) -> DesktopExchange {
        let principal = auth.authenticate(browser, now).unwrap();
        let verifier = "A".repeat(43);
        let result = auth
            .desktop_authorize(
                &principal,
                DesktopAuthorize {
                    redirect_uri: "http://127.0.0.1:49152/callback".into(),
                    code_challenge: URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
                    state: "B".repeat(43),
                },
                now,
            )
            .unwrap();
        let url = url::Url::parse(&result.redirect_url).unwrap();
        assert_eq!(
            url.query_pairs().find(|(k, _)| k == "state").unwrap().1,
            "B".repeat(43)
        );
        DesktopExchange {
            code: url
                .query_pairs()
                .find(|(k, _)| k == "code")
                .unwrap()
                .1
                .into_owned(),
            code_verifier: verifier,
            redirect_uri: "http://127.0.0.1:49152/callback".into(),
        }
    }
    #[test]
    fn pkce_one_use_and_independent_session_logout() {
        let (_dir, auth, browser) = setup();
        let input = authorize(&auth, &browser, 101);
        assert!(auth
            .desktop_exchange(
                DesktopExchange {
                    code: input.code.clone(),
                    code_verifier: "C".repeat(43),
                    redirect_uri: input.redirect_uri.clone()
                },
                102
            )
            .is_err());
        let code = input.code.clone();
        let desktop = auth.desktop_exchange(input, 102).unwrap();
        assert_ne!(browser, desktop.session_token);
        assert!(auth
            .desktop_exchange(
                DesktopExchange {
                    code,
                    code_verifier: "A".repeat(43),
                    redirect_uri: "http://127.0.0.1:49152/callback".into()
                },
                103
            )
            .is_err());
        let principal = auth.authenticate(&desktop.session_token, 104).unwrap();
        auth.logout(&principal).unwrap();
        assert!(auth.authenticate(&desktop.session_token, 105).is_err());
        assert!(auth.authenticate(&browser, 105).is_ok());
    }
    #[test]
    fn expired_code_wrong_callback_and_revoked_account_do_not_issue_sessions() {
        let (_dir, auth, browser) = setup();
        assert!(auth
            .desktop_exchange(authorize(&auth, &browser, 101), 221)
            .is_err());
        let mut input = authorize(&auth, &browser, 222);
        input.redirect_uri = "http://127.0.0.1:49153/callback".into();
        assert!(auth.desktop_exchange(input, 223).is_err());
        let input = authorize(&auth, &browser, 224);
        auth.control
            .lock()
            .unwrap()
            .set_user_disabled("student", true)
            .unwrap();
        assert!(auth.desktop_exchange(input, 225).is_err());
        for value in [
            "https://example.com/callback",
            "http://localhost:49152/callback",
            "http://127.0.0.1:49152/other",
            "http://127.0.0.1:49152/callback?code=bad",
        ] {
            assert!(callback(value).is_err());
        }
    }
}
