use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand_core::{OsRng, RngCore};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{
    webview::cookie::{
        time::{Duration as CookieDuration, OffsetDateTime},
        Cookie, SameSite,
    },
    Manager,
};
use url::Url;

pub struct CloudDesktop {
    pub origin: Url,
    pending: Mutex<Option<Arc<AtomicBool>>>,
}
impl CloudDesktop {
    pub fn new(origin: Url) -> Self {
        Self {
            origin,
            pending: Mutex::new(None),
        }
    }
    pub fn cancel(&self) {
        if let Some(pending) = self.pending.lock().unwrap().take() {
            pending.store(true, Ordering::Release);
        }
    }
}
pub fn configured_origin() -> Result<Url, String> {
    let value = option_env!("UNDERSTAND_BOOK_CLOUD_ORIGIN").unwrap_or("https://understandbook.top");
    parse_origin(value)
}
fn parse_origin(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|_| "Cloud Reader needs an HTTPS origin".to_string())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Cloud Reader needs an HTTPS origin without a path or credentials".into());
    }
    Ok(url)
}
fn random() -> Result<String, String> {
    let mut bytes = [0; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| "无法开始登录，请重试。".to_string())?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
pub fn open_browser(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", url])
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|_| "无法打开系统浏览器。".to_string())?;
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new(if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        })
        .arg(url)
        .spawn()
        .map_err(|_| "无法打开系统浏览器。".to_string())?;
    }
    Ok(())
}
fn authorization_url(origin: &Url, redirect: &str, verifier: &str, state: &str) -> Url {
    let mut result = origin.clone();
    result
        .query_pairs_mut()
        .append_pair("desktop_authorize", "1")
        .append_pair("redirect_uri", redirect)
        .append_pair(
            "code_challenge",
            &URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
        )
        .append_pair("state", state);
    result
}
fn callback_code(path: &str, state: &str) -> Result<Option<String>, String> {
    let url = Url::parse(&format!("http://127.0.0.1{path}"))
        .map_err(|_| "登录返回地址无效。".to_string())?;
    if url.path() != "/callback" {
        return Ok(None);
    }
    let pairs = url.query_pairs().collect::<Vec<_>>();
    let values = |name: &str| {
        pairs
            .iter()
            .filter(|(key, _)| key == name)
            .map(|(_, value)| value.as_ref())
            .collect::<Vec<_>>()
    };
    if values("state") != [state] {
        return Ok(None);
    }
    if values("error") == ["access_denied"] {
        return Err("已取消登录。".into());
    }
    let codes = values("code");
    if codes.len() != 1
        || codes[0].len() != 43
        || !codes[0]
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err("登录返回信息无效，请重试。".into());
    }
    Ok(Some(codes[0].to_string()))
}
#[derive(Deserialize)]
struct Session {
    session_token: String,
    expires_in: i64,
}
pub fn session_cookie(origin: &Url, token: String, expires_in: i64) -> Cookie<'static> {
    Cookie::build(("__Host-ub_session", token))
        .domain(origin.host_str().unwrap().to_string())
        .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .expires(OffsetDateTime::now_utc() + CookieDuration::seconds(expires_in))
        .build()
}
pub fn sign_in(app: &tauri::AppHandle) -> Result<(), String> {
    let desktop = app.state::<CloudDesktop>();
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut pending = desktop.pending.lock().unwrap();
        if pending.is_some() {
            return Err("登录已在进行中，请先完成或取消。".into());
        }
        *pending = Some(cancelled.clone());
    }
    let result = sign_in_inner(app, &desktop.origin, &cancelled);
    let mut pending = desktop.pending.lock().unwrap();
    if pending
        .as_ref()
        .is_some_and(|value| Arc::ptr_eq(value, &cancelled))
    {
        *pending = None;
    }
    result
}
fn sign_in_inner(
    app: &tauri::AppHandle,
    origin: &Url,
    cancelled: &AtomicBool,
) -> Result<(), String> {
    let listener = tiny_http::Server::http("127.0.0.1:0")
        .map_err(|_| "无法接收浏览器登录返回。".to_string())?;
    let address = listener
        .server_addr()
        .to_ip()
        .ok_or("无法接收浏览器登录返回。")?;
    let redirect_uri = format!("http://127.0.0.1:{}/callback", address.port());
    let verifier = random()?;
    let state = random()?;
    open_browser(authorization_url(origin, &redirect_uri, &verifier, &state).as_str())?;
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(300) {
        if cancelled.load(Ordering::Acquire) {
            return Err("已取消登录。".into());
        }
        let Some(request) = listener
            .recv_timeout(Duration::from_millis(150))
            .map_err(|_| "接收登录返回失败。".to_string())?
        else {
            continue;
        };
        if request.method() != &tiny_http::Method::Get {
            let _ = request.respond(tiny_http::Response::empty(405));
            continue;
        }
        let code = match callback_code(request.url(), &state) {
            Ok(Some(code)) => code,
            Ok(None) => {
                let _ = request.respond(tiny_http::Response::empty(404));
                continue;
            }
            Err(error) => {
                let _ = request.respond(tiny_http::Response::from_string(
                    "Sign-in cancelled. You can return to Understand Book.",
                ));
                return Err(error);
            }
        };
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(30))
            .redirects(0)
            .build();
        let response = agent.post(origin.join("/api/auth/desktop/exchange").unwrap().as_str())
            .set("Origin", &origin.origin().ascii_serialization())
            .send_json(serde_json::json!({"code":code,"code_verifier":verifier,"redirect_uri":redirect_uri}));
        let session: Session =
            match response.and_then(|response| response.into_json().map_err(ureq::Error::from)) {
                Ok(session) => session,
                Err(_) => {
                    let _ = request.respond(
                        tiny_http::Response::from_string(
                            "Sign-in failed. Return to Understand Book and try again.",
                        )
                        .with_status_code(400),
                    );
                    return Err("登录已过期或服务暂时无法连接，请重新登录。".into());
                }
            };
        if cancelled.load(Ordering::Acquire) {
            return Err("已取消登录。".into());
        }
        let window = app.get_webview_window("main").ok_or("阅读窗口已关闭")?;
        let cookie = session_cookie(origin, session.session_token, session.expires_in);
        window
            .set_cookie(cookie)
            .map_err(|_| "无法保存本机登录会话，请重试。".to_string())?;
        window
            .navigate(origin.clone())
            .map_err(|_| "无法打开云端阅读器，请重新打开应用。".to_string())?;
        let _ = request.respond(
            tiny_http::Response::from_string(
                "Signed in. You can return to the Understand Book Windows app.",
            )
            .with_header(tiny_http::Header::from_bytes("Cache-Control", "no-store").unwrap()),
        );
        return Ok(());
    }
    Err("等待浏览器确认超时，请重新登录。".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_request_uses_pkce_and_callback_accepts_only_matching_attempt() {
        let origin = parse_origin("https://reader.example").unwrap();
        let verifier = "A".repeat(43);
        let state = "B".repeat(43);
        let url = authorization_url(
            &origin,
            "http://127.0.0.1:49152/callback",
            &verifier,
            &state,
        );
        assert!(!url.as_str().contains("code_verifier"));
        assert!(url.query_pairs().any(|(key, value)| key == "code_challenge"
            && value == URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))));
        let code = "C".repeat(43);
        assert_eq!(
            callback_code(&format!("/callback?state={state}&code={code}"), &state).unwrap(),
            Some(code.clone())
        );
        assert!(
            callback_code(&format!("/callback?state=other&code={code}"), &state)
                .unwrap()
                .is_none()
        );
        assert!(callback_code(
            &format!("/callback?state={state}&error=access_denied"),
            &state
        )
        .is_err());
        assert!(callback_code(
            &format!("/callback?state={state}&code={code}&code={code}"),
            &state
        )
        .is_err());
    }
    #[test]
    fn cloud_origin_and_cancellation_are_scoped() {
        for value in [
            "http://reader.example",
            "https://reader.example/path",
            "https://user:pass@reader.example",
        ] {
            assert!(parse_origin(value).is_err());
        }
        let desktop = CloudDesktop::new(parse_origin("https://reader.example").unwrap());
        let pending = Arc::new(AtomicBool::new(false));
        *desktop.pending.lock().unwrap() = Some(pending.clone());
        desktop.cancel();
        assert!(pending.load(Ordering::Acquire));
        assert!(desktop.pending.lock().unwrap().is_none());
    }
}
