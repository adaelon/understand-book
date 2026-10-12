//! Single Resend HTTP channel for account mail. Reserve before committing a
//! request, release its database lock, then consume the reservation to send.
use crate::user_storage_paths::error;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand_core::{OsRng, RngCore};
use read_tools::ToolError;
use serde_json::json;
use std::{
    collections::VecDeque,
    error::Error,
    net::{SocketAddr, ToSocketAddrs},
    sync::Mutex,
    time::{Duration, Instant},
};

pub(crate) const VERIFICATION_SECONDS: i64 = 15 * 60;
pub(crate) const RESET_SECONDS: i64 = 30 * 60;
pub(crate) const MAX_VERIFICATION_ATTEMPTS: u32 = 5;
pub(crate) const RESEND_SECONDS: u64 = 60;
const GLOBAL_PER_MINUTE: usize = 60;
const DEFAULT_ENDPOINT: &str = "https://api.resend.com/emails";

// Intentionally no Debug/Serialize: these values include mail credentials.
pub(crate) struct AccountMail {
    origin: String,
    transport: Option<Resend>,
    attempts: Mutex<VecDeque<(Instant, String)>>,
}
struct Resend {
    endpoint: String,
    from: String,
    api_key: String,
    agent: ureq::Agent,
}
pub(crate) enum VerificationPurpose {
    Registration,
    EmailBinding,
}

/// Owned recipient and no lock guard. Consuming this value sends at most once.
pub(crate) struct MailSend<'a> {
    mail: &'a AccountMail,
    recipient: String,
}

fn unavailable() -> ToolError {
    error(
        "ACCOUNT_MAIL_UNAVAILABLE",
        "unavailable",
        "Account email is not configured",
    )
}
fn unknown() -> ToolError {
    error(
        "ACCOUNT_MAIL_DELIVERY_UNKNOWN",
        "unavailable",
        "Email delivery is unconfirmed; wait or request a new message after 60 seconds",
    )
}
fn config_error() -> String {
    "Invalid account mail configuration: set UNDERSTAND_BOOK_MAIL_FROM and UNDERSTAND_BOOK_MAIL_API_KEY together; use an HTTPS endpoint and a timeout of 1 to 30 seconds".into()
}

// ureq's request deadline cannot interrupt the OS resolver. Bound the caller's
// wait as well; a late DNS result is discarded before any HTTP send can occur.
fn resolve_bounded(
    lookup: impl FnOnce() -> std::io::Result<Vec<SocketAddr>> + Send + 'static,
    timeout: Duration,
) -> std::io::Result<Vec<SocketAddr>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("account-mail-dns".into())
        .spawn(move || {
            let _ = tx.send(lookup());
        })?;
    rx.recv_timeout(timeout).unwrap_or_else(|_| {
        Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "Account mail DNS timeout",
        ))
    })
}

/// Shared normalization for registration, binding, login and recovery. Keep
/// aliases and dots intact; the control schema stores this lowercase value.
pub(crate) fn normalize_email(input: &str) -> Result<String, ToolError> {
    let email = input.trim().to_ascii_lowercase();
    let valid = email.split_once('@').is_some_and(|(local, domain)| {
        !local.is_empty()
            && local.len() <= 64
            && !domain.is_empty()
            && !local.starts_with('.')
            && !local.ends_with('.')
            && !local.contains("..")
            && local
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&b))
            && domain.split('.').all(|part| {
                !part.is_empty()
                    && !part.starts_with('-')
                    && !part.ends_with('-')
                    && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
    });
    if email.len() > 254 || !valid {
        return Err(error(
            "ACCOUNT_EMAIL_INVALID",
            "validation",
            "Enter a valid email address",
        ));
    }
    Ok(email)
}

pub(crate) fn verification_code() -> Result<String, ToolError> {
    let mut code = String::with_capacity(6);
    while code.len() < 6 {
        let mut byte = [0];
        OsRng.try_fill_bytes(&mut byte).map_err(|_| unavailable())?;
        // Rejection sampling keeps all ten digits equally likely.
        if byte[0] < 250 {
            code.push(char::from(b'0' + byte[0] % 10));
        }
    }
    Ok(code)
}
pub(crate) fn reset_token() -> Result<String, ToolError> {
    let mut bytes = [0; 32];
    OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| unavailable())?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

impl AccountMail {
    pub(crate) fn disabled(origin: &str) -> Self {
        Self {
            origin: origin.into(),
            transport: None,
            attempts: Mutex::new(VecDeque::new()),
        }
    }

    // `origin` is already normalized and checked by Site, never request headers.
    pub(crate) fn from_env(origin: &str) -> Result<Self, String> {
        Self::from_settings(origin, |key| std::env::var(key).ok())
    }

    fn from_settings(origin: &str, get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let value = |key| get(key).filter(|v| !v.trim().is_empty());
        let from = value("UNDERSTAND_BOOK_MAIL_FROM");
        let key = value("UNDERSTAND_BOOK_MAIL_API_KEY");
        let endpoint = value("UNDERSTAND_BOOK_MAIL_ENDPOINT");
        let timeout = value("UNDERSTAND_BOOK_MAIL_TIMEOUT_SECONDS");
        if from.is_none() && key.is_none() && endpoint.is_none() && timeout.is_none() {
            return Ok(Self::disabled(origin));
        }
        let from = normalize_email(&from.ok_or_else(config_error)?).map_err(|_| config_error())?;
        let api_key = key.ok_or_else(config_error)?;
        if !api_key.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(config_error());
        }
        let endpoint = endpoint.unwrap_or_else(|| DEFAULT_ENDPOINT.into());
        let url = url::Url::parse(&endpoint).map_err(|_| config_error())?;
        let local_http = url.scheme() == "http"
            && match url.host() {
                Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
                Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
                _ => false,
            };
        if (url.scheme() != "https" && !local_http)
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(config_error());
        }
        let seconds = timeout
            .as_deref()
            .unwrap_or("10")
            .parse::<u64>()
            .map_err(|_| config_error())?;
        if !(1..=30).contains(&seconds) {
            return Err(config_error());
        }
        let timeout = Duration::from_secs(seconds);
        Ok(Self {
            origin: origin.into(),
            transport: Some(Resend {
                endpoint,
                from,
                api_key,
                agent: ureq::AgentBuilder::new()
                    .redirects(0)
                    .resolver(move |address: &str| {
                        let address = address.to_owned();
                        resolve_bounded(
                            move || address.to_socket_addrs().map(|a| a.collect()),
                            timeout,
                        )
                    })
                    .timeout_connect(timeout)
                    .timeout(timeout)
                    .build(),
            }),
            attempts: Mutex::new(VecDeque::new()),
        })
    }

    /// All purposes share a rolling 60-second window. Failed and abandoned
    /// attempts also consume capacity, so failures cannot cause a send storm.
    pub(crate) fn reserve(&self, email: &str) -> Result<MailSend<'_>, ToolError> {
        if self.transport.is_none() {
            return Err(unavailable());
        }
        let recipient = normalize_email(email)?;
        let mut attempts = self.attempts.lock().unwrap();
        let now = Instant::now();
        while attempts
            .front()
            .is_some_and(|(at, _)| now.duration_since(*at) >= Duration::from_secs(RESEND_SECONDS))
        {
            attempts.pop_front();
        }
        if attempts.len() >= GLOBAL_PER_MINUTE
            || attempts.iter().any(|(_, email)| email == &recipient)
        {
            return Err(error(
                "ACCOUNT_MAIL_RATE_LIMITED",
                "rate_limit",
                "Wait 60 seconds before requesting another account email",
            ));
        }
        attempts.push_back((now, recipient.clone()));
        Ok(MailSend {
            mail: self,
            recipient,
        })
    }
}

impl MailSend<'_> {
    pub(crate) fn verification(
        self,
        purpose: VerificationPurpose,
        code: &str,
    ) -> Result<(), ToolError> {
        let purpose = match purpose {
            VerificationPurpose::Registration => "注册账号",
            VerificationPurpose::EmailBinding => "绑定邮箱",
        };
        let text = format!("你正在 Understand Book {purpose}。\n\n验证码：{code}\n有效期：{} 分钟。重新发送后只有最新验证码有效。\n\n请返回 {} 输入验证码。\n如果不是你本人操作，请忽略此邮件。", VERIFICATION_SECONDS / 60, self.mail.origin);
        self.send(&format!("Understand Book {purpose}验证码"), &text)
    }

    pub(crate) fn password_reset(self, token: &str) -> Result<(), ToolError> {
        // Fragments stay out of HTTP access logs. INV8 consumes and clears this
        // fragment, then POSTs the token only when the new password is submitted.
        let fragment: String = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("token", token)
            .finish();
        let link = format!("{}/?account=reset-password#{fragment}", self.mail.origin);
        let text = format!("你申请了重置 Understand Book 登录密码。\n\n设置新密码：{link}\n\n链接有效期为 {} 分钟，仅能使用一次。打开链接不会修改密码，提交新密码后才会完成重置。\n如果不是你本人操作，请忽略此邮件。", RESET_SECONDS / 60);
        self.send("Understand Book 重置密码", &text)
    }

    fn send(self, subject: &str, text: &str) -> Result<(), ToolError> {
        let transport = self.mail.transport.as_ref().ok_or_else(unavailable)?;
        // No automatic resend, database handles, telemetry, or raw error bodies.
        let result = transport
            .agent
            .post(&transport.endpoint)
            .set("Authorization", &format!("Bearer {}", transport.api_key))
            .send_json(
                json!({"from":transport.from,"to":[self.recipient],"subject":subject,"text":text}),
            );
        match result {
            Ok(response) if (200..300).contains(&response.status()) => Ok(()),
            Err(ureq::Error::Status(429, _)) => Err(error(
                "ACCOUNT_MAIL_PROVIDER_RATE_LIMITED",
                "rate_limit",
                "Email provider is busy; try again later",
            )),
            Err(ureq::Error::Status(500..=599, _)) => Err(unknown()),
            Err(ureq::Error::Transport(e)) => {
                let timed_out = e
                    .source()
                    .and_then(|source| source.downcast_ref::<std::io::Error>())
                    .is_some_and(|e| {
                        matches!(
                            e.kind(),
                            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                        )
                    });
                if timed_out {
                    Err(error("ACCOUNT_MAIL_TIMEOUT", "unavailable", "Email delivery timed out and may have succeeded; wait or request a new message after 60 seconds"))
                } else {
                    Err(unknown())
                }
            }
            _ => Err(error(
                "ACCOUNT_MAIL_REJECTED",
                "unavailable",
                "Email provider did not accept the message; try again later",
            )),
        }
    }
}

#[cfg(test)]
#[path = "account_mail_tests.rs"]
pub(crate) mod tests;
