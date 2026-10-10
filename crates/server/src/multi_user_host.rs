//! Explicit HTTPS-proxy application Host. Every data route, including aliases,
//! passes authentication and the same capability resolver before dispatch.
use crate::{
    auth,
    authorization::{self, Authorization, Capability},
    user_registry::UserRegistry,
};
use read_tools::ToolError;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    io::Read,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};
use tiny_http::{Header, Response, Server};

pub struct MultiUserConfig {
    pub root: PathBuf,
    pub addr: SocketAddr,
    pub origin: String,
}
pub struct RunningMultiUserServer {
    pub url: String,
    stop: Arc<AtomicBool>,
    handles: Vec<thread::JoinHandle<()>>,
    access: Arc<Authorization>,
    waiters: Arc<Mutex<Vec<thread::JoinHandle<()>>>>,
}
impl RunningMultiUserServer {
    pub fn observability_status(&self) -> crate::observability::ObservabilityStatus {
        self.access.runs.observability.status()
    }

    pub fn shutdown(mut self) {
        self.access.runs.stop();
        self.stop.store(true, Ordering::Release);
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
        for h in self.waiters.lock().unwrap().drain(..) {
            let _ = h.join();
        }
        self.access.runs.observability.shutdown();
        if let Err(e) = self.access.users.lock().unwrap().flush_reads() {
            eprintln!("User flush: {}", e.error_code);
        }
    }
    pub fn wait(mut self) {
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}
pub(crate) struct Site {
    origin: String,
    host: String,
    pub(crate) mail: crate::account_mail::AccountMail,
}
impl Site {
    pub(crate) fn new(origin: &str) -> Result<Self, String> {
        let parsed = url::Url::parse(origin).map_err(|_| "Expected an HTTPS origin".to_string())?;
        if parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || parsed.path() != "/"
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err("Expected an HTTPS origin without credentials or a path".into());
        }
        let normalized = parsed.origin().ascii_serialization();
        let host = normalized.strip_prefix("https://").unwrap().to_string();
        Ok(Self {
            mail: crate::account_mail::AccountMail::disabled(&normalized),
            origin: normalized,
            host,
        })
    }
    fn validate(&self, headers: &Headers, write: bool) -> Result<(), ToolError> {
        if headers.get("Host") != Some(self.host.as_str()) {
            return Err(authorization::forbidden());
        }
        if (write || headers.get("Origin").is_some())
            && headers.get("Origin") != Some(self.origin.as_str())
        {
            return Err(crate::user_storage_paths::error(
                "ORIGIN_REJECTED",
                "permission",
                "Request origin is not trusted",
            ));
        }
        Ok(())
    }
}
pub(crate) struct Headers(pub Vec<(String, String)>);
impl Headers {
    fn get(&self, name: &str) -> Option<&str> {
        let mut values = self
            .0
            .iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case(name));
        let value = values.next()?.1.as_str();
        if values.next().is_some() {
            None
        } else {
            Some(value)
        }
    }
}
pub(crate) struct HttpReply {
    pub status: u16,
    pub body: Vec<u8>,
    pub content_type: String,
    pub cookie: Option<String>,
    waiting: Option<SyncWait>,
    observation: Option<(
        Arc<crate::agent_stream::RunStream>,
        authorization::AuthorizedObservation,
        Option<u64>,
    )>,
}
impl HttpReply {
    fn json(value: Value) -> Self {
        Self {
            status: 200,
            body: value.to_string().into_bytes(),
            content_type: "application/json; charset=utf-8".into(),
            cookie: None,
            observation: None,
            waiting: None,
        }
    }
    fn from_reply(reply: crate::Reply) -> Self {
        if reply.status >= 400 {
            let mut result = Self::json(
                json!({"error_code":"REQUEST_FAILED","message":"The requested operation could not be completed"}),
            );
            result.status = reply.status;
            result
        } else {
            Self {
                status: reply.status,
                body: reply.body.into_bytes(),
                content_type: "application/json; charset=utf-8".into(),
                cookie: None,
                observation: None,
                waiting: None,
            }
        }
    }
    fn error(error: ToolError) -> Self {
        let status = match error.category.as_str() {
            "authentication" => 401,
            "permission" => 403,
            "not_found" => 404,
            "rate_limit" => 429,
            "validation" => 400,
            "conflict" => 409,
            "not_implemented" => 501,
            _ => 503,
        };
        let message = match status {
            401 => "Sign in with an active account",
            403 => "Operation or request origin is not permitted",
            404 => "Object is unavailable",
            429 => "Capacity reached; try again later",
            501 => "This operation is not enabled in this service version",
            400 => "Invalid request",
            _ => "The operation could not be completed",
        };
        let mut reply = Self::json(json!({"error_code":error.error_code,"category":error.category,"message":message}));
        reply.status = status;
        reply
    }
}
fn method_error() -> ToolError {
    crate::user_storage_paths::error("METHOD_NOT_ALLOWED", "method", "Unsupported method")
}
fn expect_method(method: &str, expected: &str) -> Result<(), ToolError> {
    if method == expected {
        Ok(())
    } else {
        Err(method_error())
    }
}
fn invalid() -> ToolError {
    crate::user_storage_paths::error("INVALID_REQUEST", "validation", "Invalid request")
}
pub(crate) fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

pub fn start(config: MultiUserConfig) -> Result<RunningMultiUserServer, String> {
    if !config.addr.ip().is_loopback() {
        return Err(
            "Multi-user backend must bind loopback behind the configured HTTPS proxy".into(),
        );
    }
    let mut site = Site::new(&config.origin)?;
    site.mail = crate::account_mail::AccountMail::from_env(&site.origin)?;
    let site = Arc::new(site);
    let mut access = Authorization::new(UserRegistry::open(&config.root).map_err(|e| e.error_code)?)
        .map_err(|e| e.error_code)?;
    access.runs.observability = crate::observability::ObservabilityRuntime::from_env();
    let access = Arc::new(access);
    if let Ok(provider) = crate::ProviderConfig::from_env() {
        if let Err(e) = access.spend.validate_provider(&provider.base_url, &provider.model) {
            eprintln!("Managed model sends unavailable: {}; check model-rates.json", e.code());
        }
        access.runs.configure(provider).map_err(|e| e.error_code)?;
    }
    start_with_access(config.addr, site, access)
}
pub(crate) fn start_with_access(
    addr: SocketAddr,
    site: Arc<Site>,
    access: Arc<Authorization>,
) -> Result<RunningMultiUserServer, String> {
    if !addr.ip().is_loopback() {
        return Err("Multi-user backend must bind loopback".into());
    }
    access.runs.recover(&access).map_err(|e| e.error_code)?;
    let server =
        Arc::new(Server::http(addr).map_err(|_| "Could not bind multi-user backend".to_string())?);
    let url = format!(
        "http://{}",
        server.server_addr().to_ip().ok_or("Expected IP listener")?
    );
    let stop = Arc::new(AtomicBool::new(false));
    let mut handles = Vec::new();
    let waiters = Arc::new(Mutex::new(Vec::<thread::JoinHandle<()>>::new()));
    {
        let (access, stop) = (access.clone(), stop.clone());
        handles.push(thread::spawn(move || {
            let mut last = std::time::Instant::now();
            while !stop.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(100));
                if last.elapsed() < Duration::from_secs(60) {
                    continue;
                }
                if let Err(e) = access
                    .workspaces
                    .lock()
                    .unwrap()
                    .evict_idle(std::time::Instant::now())
                {
                    eprintln!("Workspace maintenance: {}", e.error_code);
                }
                last = std::time::Instant::now();
            }
        }));
    }
    for _ in 0..access.resources.limits.active_runs {
        let (access, stop) = (access.clone(), stop.clone());
        handles.push(thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                match access.runs.run_one(&access) {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(e) => eprintln!("Run worker: {}", e.error_code),
                }
                thread::sleep(Duration::from_millis(100));
            }
        }));
    }
    for _ in 0..4 {
        let waiters = waiters.clone();
        let (server, stop, site, access) =
            (server.clone(), stop.clone(), site.clone(), access.clone());
        handles.push(thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                let Ok(Some(mut request)) = server.recv_timeout(Duration::from_millis(100)) else {
                    continue;
                };
                let method = request.method().to_string();
                let url = request.url().to_string();
                let headers = Headers(
                    request
                        .headers()
                        .iter()
                        .map(|h| (h.field.to_string(), h.value.to_string()))
                        .collect(),
                );
                let mut body = String::new();
                let limit = access.resources.limits.request_bytes;
                let read = request
                    .as_reader()
                    .take(limit as u64 + 1)
                    .read_to_string(&mut body);
                let reply = if read.is_err() || body.len() > limit {
                    HttpReply::error(invalid())
                } else {
                    dispatch(&access, &site, &method, &url, &headers, &body, now())
                };
                if let Some((stream, permit, cursor)) = reply.observation {
                    crate::agent_stream::serve_authorized(request, stream, cursor, permit);
                    continue;
                }
                if let Some(wait) = reply.waiting {
                    let access = access.clone();
                    let mut handles = waiters.lock().unwrap();
                    let mut pending = Vec::new();
                    for handle in handles.drain(..) {
                        if handle.is_finished() {
                            let _ = handle.join();
                        } else {
                            pending.push(handle);
                        }
                    }
                    *handles = pending;
                    handles.push(thread::spawn(move || {
                        respond(request, wait_for_turn(&access, &wait));
                    }));
                    continue;
                }
                respond(request, reply);
            }
        }));
    }
    Ok(RunningMultiUserServer {
        url,
        stop,
        handles,
        access,
        waiters,
    })
}

fn respond(request: tiny_http::Request, reply: HttpReply) {
    let mut response = Response::from_data(reply.body).with_status_code(reply.status);
    if reply.content_type == "application/pdf" {
        // PDF bytes are already buffered. Keep their known Content-Length so the
        // HTTPS proxy can serve byte ranges and PDF.js can render before EOF.
        response = response.with_chunked_threshold(usize::MAX);
    }
    for (key, value) in [
                    ("Content-Type", reply.content_type.as_str()), ("Cache-Control", "no-store"),
                    ("X-Content-Type-Options", "nosniff"), ("Referrer-Policy", "same-origin"),
                    ("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'")
                ] { response.add_header(Header::from_bytes(key, value).unwrap()); }
    if let Some(cookie) = reply.cookie {
        response.add_header(Header::from_bytes("Set-Cookie", cookie).unwrap());
    }
    if reply.status == 429 {
        response.add_header(Header::from_bytes("Retry-After", "60").unwrap());
    }
    let _ = request.respond(response);
}

pub(crate) fn dispatch(
    access: &Arc<Authorization>,
    site: &Site,
    method: &str,
    url: &str,
    headers: &Headers,
    body: &str,
    now: i64,
) -> HttpReply {
    let result = (|| -> Result<HttpReply, ToolError> {
        site.validate(headers, !matches!(method, "GET" | "HEAD"))?;
        // Only this fixed credential-free shell is public. No SPA/static fallback for unknown URLs.
        if method == "GET" {
            let public = match url {
                "/" | "/login" => Some((
                    include_str!("multi_user_login.html"),
                    "text/html; charset=utf-8",
                )),
                "/auth.js" => Some((
                    include_str!("multi_user_login.js"),
                    "text/javascript; charset=utf-8",
                )),
                "/auth.css" => Some((
                    include_str!("multi_user_login.css"),
                    "text/css; charset=utf-8",
                )),
                _ => None,
            };
            if let Some((body, content_type)) = public {
                return Ok(HttpReply {
                    status: 200,
                    body: body.as_bytes().to_vec(),
                    content_type: content_type.into(),
                    cookie: None,
                    observation: None,
                    waiting: None,
                });
            }
        }
        let (path, _) = authorization::canonical(url)?;
        if matches!(url, "/api/auth/register/start" | "/api/auth/register/resend" | "/api/auth/register/complete") {
            expect_method(method, "POST")?;
            let result = match path {
                "/auth/register/start" => access.registration.start(serde_json::from_str(body).map_err(|_| invalid())?, &site.mail, now),
                "/auth/register/resend" => access.registration.resend(serde_json::from_str(body).map_err(|_| invalid())?, &site.mail, now),
                _ => access.registration.complete(serde_json::from_str(body).map_err(|_| invalid())?, now).map_err(Into::into),
            };
            return Ok(match result {
                Ok(value) => HttpReply::json(value),
                Err(failure) => {
                    let mut reply = HttpReply::error(failure.error);
                    if let Some(request) = failure.request {
                        let mut body: Value = serde_json::from_slice(&reply.body).unwrap();
                        body.as_object_mut().unwrap().extend(request.as_object().unwrap().clone());
                        reply.body = body.to_string().into_bytes();
                    }
                    reply
                }
            });
        }
        let token = headers
            .get("Cookie")
            .and_then(|c| auth::token_from_cookie(c).ok());
        if matches!(url, "/api/auth/password/forgot" | "/api/auth/password/reset") {
            expect_method(method, "POST")?;
            let resetting = path == "/auth/password/reset";
            let value = if resetting {
                access.password.reset(serde_json::from_str(body).map_err(|_| invalid())?, now)?
            } else {
                access.password.forgot(serde_json::from_str(body).map_err(|_| invalid())?, &site.mail, now)?
            };
            let mut reply = HttpReply::json(value);
            if resetting { reply.cookie = Some(auth::cookie("", true)); }
            return Ok(reply);
        }
        if path == "/auth/login" {
            expect_method(method, "POST")?;
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Login {
                username: String,
                password: String,
            }
            let input: Login = serde_json::from_str(body).map_err(|_| invalid())?;
            let token = access
                .auth
                .login(&input.username, &input.password, token, now)?;
            let principal = access.auth.authenticate(&token, now)?;
            let mut reply =
                HttpReply::json(json!({"user_id":principal.user_id(),"email":access.auth.email(&principal,now)?,"csrf_token":auth::csrf(&token)}));
            reply.cookie = Some(auth::cookie(&token, false));
            return Ok(reply);
        }
        let token = token.ok_or_else(auth::unauthenticated)?;
        let principal = access.auth.authenticate(token, now)?;
        if !matches!(method, "GET" | "HEAD")
            && headers.get("X-CSRF-Token") != Some(auth::csrf(token).as_str())
        {
            return Err(crate::user_storage_paths::error(
                "CSRF_REJECTED",
                "permission",
                "CSRF token is required",
            ));
        }
        let (_, query) = crate::parse_query(url);
        let input: Value = if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(body).map_err(|_| invalid())?
        };
        if path.starts_with("/admin/") && url.starts_with("/api/admin/") {
            access.auth.require_reader_admin(&principal, now)?;
            return crate::admin_api::dispatch(access, &principal, method, path, &query, input, now)
                .map(HttpReply::json);
        }
        for key in [
            "user_id",
            "owner_user_id",
            "dir",
            "book_dir",
            "service_root",
        ] {
            if query.contains_key(key) || input.get(key).is_some() {
                return Err(invalid());
            }
        }
        let capability = authorization::capability(path)?;
        // Login/logout/me do not load private state; all other routes get the authenticated authority.
        match capability {
            Capability::Password => {
                expect_method(method, "POST")?;
                if !query.is_empty() { return Err(invalid()); }
                let value = access.password.change(&principal, serde_json::from_value(input).map_err(|_| invalid())?, &access.auth, now)?;
                let mut reply = HttpReply::json(value);
                reply.cookie = Some(auth::cookie("", true));
                return Ok(reply);
            }
            Capability::EmailBinding => {
                expect_method(method, "POST")?;
                if !query.is_empty() { return Err(invalid()); }
                let result = match path {
                    "/account/email/start" => access.email_binding.start(&principal, serde_json::from_value(input).map_err(|_| invalid())?, &access.auth, &site.mail, now),
                    "/account/email/resend" => access.email_binding.resend(&principal, serde_json::from_value(input).map_err(|_| invalid())?, &site.mail, now),
                    _ => access.email_binding.complete(&principal, serde_json::from_value(input).map_err(|_| invalid())?, now).map_err(Into::into),
                };
                return Ok(match result {
                    Ok(value) => HttpReply::json(value),
                    Err(failure) => {
                        let mut reply = HttpReply::error(failure.error);
                        if let Some(request) = failure.request {
                            let mut body: Value = serde_json::from_slice(&reply.body).unwrap();
                            body.as_object_mut().unwrap().extend(request.as_object().unwrap().clone());
                            reply.body = body.to_string().into_bytes();
                        }
                        reply
                    }
                });
            }
            Capability::Allowance => {
                expect_method(method, "GET")?;
                if !query.is_empty() || input.as_object().is_some_and(|v|!v.is_empty()) { return Err(invalid()); }
                return access.spend.allowance(principal.user_id(), now).map(HttpReply::json);
            }
            Capability::AccountUsage => {
                expect_method(method, "GET")?;
                if input.as_object().is_some_and(|v|!v.is_empty()) { return Err(invalid()); }
                let (limit,offset) = crate::admin_api::page(&query)?;
                return access.spend.charges(Some(principal.user_id()),false,limit,offset,true,None,None).map(HttpReply::json);
            }
            Capability::Me => {
                expect_method(method, "GET")?;
                return Ok(HttpReply::json(
                    json!({"user_id":principal.user_id(),"email":access.auth.email(&principal,now)?,"csrf_token":auth::csrf(token),"capabilities":{"presentation":access.sandbox.capability(),"admin":access.auth.is_reader_admin(&principal, now)?}}),
                ));
            }
            Capability::Logout => {
                expect_method(method, "POST")?;
                access.auth.logout(&principal)?;
                let mut reply = HttpReply::json(json!({"ok":true}));
                reply.cookie = Some(auth::cookie("", true));
                return Ok(reply);
            }
            _ => {}
        }
        let context = access.context(principal, now)?;
        match capability {
            Capability::Library => {
                expect_method(method, "GET")?;
                Ok(HttpReply::json(
                    json!({"books":access.library.lock().unwrap().list(context.user_id())?}),
                ))
            }
            Capability::Book(reference, leaf) => {
                let user = context.user.lock().unwrap();
                let mut library = access.library.lock().unwrap();
                library
                    .authorize(context.user_id(), &reference)
                    .map_err(|_| authorization::missing())?;
                if !authorization::book_leaf_allowed(&leaf) {
                    return Err(authorization::missing());
                }
                // HEAD is authorized first, then handled explicitly. Range is supported by returning
                // the complete representation (200); conditional requests never bypass authorization.
                if !matches!(method, "GET" | "HEAD") {
                    return Err(method_error());
                }
                let reply = if leaf.starts_with("assets/")
                    || matches!(leaf.as_str(), "pdf/original" | "original.pdf")
                {
                    let asset = library
                        .asset(
                            context.user_id(),
                            &reference,
                            if leaf.starts_with("assets/") {
                                &leaf
                            } else {
                                "original.pdf"
                            },
                        )
                        .map_err(|_| authorization::missing())?;
                    HttpReply {
                        status: asset.status,
                        body: asset.body,
                        content_type: asset.content_type,
                        cookie: None,
                        observation: None,
                        waiting: None,
                    }
                } else {
                    HttpReply::from_reply(library.read(
                        context.user_id(),
                        &reference,
                        &leaf,
                        &query,
                        &user,
                    )?)
                };
                // tiny_http suppresses HEAD bytes while preserving GET's Content-Length.
                Ok(reply)
            }
            Capability::Recap => {
                expect_method(method, "GET")?;
                Ok(HttpReply::json(serde_json::to_value(crate::session_recap::network(access, &context, &query, &now.to_string())?).unwrap()))
            }
            Capability::History => {
                expect_method(method, "GET")?;
                Ok(HttpReply::json(
                    context.history(query.get("session_id").map(String::as_str))?,
                ))
            }
            Capability::Chat(id) => {
                if method == "DELETE" {
                    return Ok(HttpReply::json(
                        access.runs.delete_chat(access, &context, &id)?,
                    ));
                }
                let value = context.history(Some(&id))?;
                expect_method(method, "GET")?;
                Ok(HttpReply::json(value))
            }
            Capability::Admission(wait) => {
                if method == "GET" && !wait {
                    return Ok(HttpReply::json(reader_run_view(access, &context, access.runs.lookup(
                        &context,
                        query.get("client_request_id").ok_or_else(invalid)?,
                    )?)?));
                }
                expect_method(method, "POST")?;
                let workspace = input["workspace_id"].as_str().ok_or_else(invalid)?;
                admit_reply(access, &context, workspace, &input, &now.to_string(), wait)
            }
            Capability::Workspace(id) => {
                let prefix = format!("/workspaces/{id}");
                let action = path.strip_prefix(&prefix).unwrap().trim_start_matches('/');
                expect_method(method, if action.is_empty() { "GET" } else { "POST" })?;
                if matches!(action, "agent/runs" | "agent/chat") {
                    return admit_reply(
                        access,
                        &context,
                        &id,
                        &input,
                        &now.to_string(),
                        action == "agent/chat",
                    );
                }
                access.workspace(&context, &id)?;
                let mut user = context.user.lock().unwrap();
                #[cfg(test)]
                let _timing = crate::tests::mu10_tests::measure("user_lock");
                let value = access.workspaces.lock().unwrap().request(
                    &context,
                    &mut user,
                    &access.library,
                    &id,
                    action,
                    &input,
                    &now.to_string(),
                )?;
                if matches!(action, "attach" | "takeover" | "fork" | "book/open") {
                    access.admin.record_read(context.user_id(), now)?;
                }
                Ok(HttpReply::json(value))
            }
            Capability::ChatAction => {
                let id = input["session_id"].as_str().ok_or_else(invalid)?;
                context.history(Some(id))?;
                expect_method(method, "POST")?;
                if path == "/agent/history/select" {
                    let workspace = input["workspace_id"].as_str().ok_or_else(invalid)?;
                    access.workspace(&context, workspace)?;
                    let mut user = context.user.lock().unwrap();
                    return Ok(HttpReply::json(access.workspaces.lock().unwrap().request(
                        &context,
                        &mut user,
                        &access.library,
                        workspace,
                        "chat/select",
                        &input,
                        &now.to_string(),
                    )?));
                }
                Ok(HttpReply::json(
                    access.runs.delete_chat(access, &context, id)?,
                ))
            }
            Capability::NewWorkspace => {
                expect_method(method, "POST")?;
                let user = context.user.lock().unwrap();
                let value = access.workspaces.lock().unwrap().create(
                    &context,
                    &user,
                    &access.library,
                    &input,
                )?;
                access.admin.record_read(context.user_id(), now)?;
                Ok(HttpReply::json(value))
            }
            Capability::Tutor(mutate) => {
                expect_method(method, if mutate { "POST" } else { "GET" })?;
                let user = context.user.lock().unwrap();
                let mut learning = user.learning_store()?;
                let value = if mutate {
                    let request = serde_json::from_value(input).map_err(|_| invalid())?;
                    learning.mutate(&request, &now.to_string())?
                } else {
                    learning.state()?
                };
                Ok(HttpReply::json(json!(value)))
            }
            Capability::Run(id, action) => {
                let value = reader_run_view(access, &context, context.turn(&id)?)?;
                expect_method(
                    method,
                    if matches!(action.as_str(), "cancel" | "retry-save") {
                        "POST"
                    } else {
                        "GET"
                    },
                )?;
                if action == "retry-save" {
                    return Ok(HttpReply::json(reader_run_view(access, &context, access.runs.retry_save(&context, &id)?)?));
                }
                if action == "cancel" {
                    match access.runs.cancel(&context, &id) {
                        Ok(v) => return Ok(HttpReply::json(reader_run_view(access, &context, v)?)),
                        Err(e)
                            if e.error_code == "OBJECT_NOT_FOUND"
                                && value["turn"]["status"] != "pending_assistant" =>
                        {
                            return Ok(HttpReply::json(value))
                        }
                        Err(e) => return Err(e),
                    }
                }
                if action.is_empty() {
                    match access.runs.status(&context, &id) {
                        Ok(v) => return Ok(HttpReply::json(reader_run_view(access, &context, v)?)),
                        Err(e) if e.error_code == "OBJECT_NOT_FOUND" => {
                            return Ok(HttpReply::json(value))
                        }
                        Err(e) => return Err(e),
                    }
                }
                if action == "events" || action == "cancel" {
                    let reference: crate::published_library::PublishedBookRef =
                        serde_json::from_value(value["turn"]["published_book_ref"].clone())
                            .map_err(|_| authorization::missing())?;
                    access
                        .library
                        .lock()
                        .unwrap()
                        .authorize(context.user_id(), &reference)
                        .map_err(|_| authorization::missing())?;
                    let permit = access.observation(&context, &id)?;
                    let stream = if let Some(stream) = access.runs.stream(&context, &id) {
                        stream
                    } else {
                        // Read again after selecting the live stream: completion may have crossed the first read.
                        let value = reader_run_view(access, &context, context.turn(&id)?)?;
                        let snapshot = crate::agent_stream::RunSnapshot {
                            descriptor: crate::agent_stream::RunDescriptor {
                                book_id: value["book_id"].as_str().unwrap().into(),
                                session_id: value["session_id"].as_str().unwrap().into(),
                                turn_id: id.clone(),
                            },
                            last_seq: 0,
                            execution_state: value["turn"]["status"].as_str().unwrap().into(),
                            persistence_state: "saved".into(),
                            activities: vec![],
                            effects: vec![],
                            reader_state: None,
                            draft: None,
                            final_view: Some(value["turn"].clone()),
                            error: value["turn"].get("error").filter(|v| !v.is_null()).cloned(),
                        };
                        crate::agent_stream::RunStream::with_observation(
                            snapshot,
                            Some(format!("{}-{}", access.runs.boot, uuid::Uuid::now_v7())),
                            access.resources.limits.event_bytes,
                        )
                    };
                    let cursor = stream.cursor(
                        headers
                            .get("Last-Event-ID")
                            .or_else(|| query.get("after").map(String::as_str)),
                    )?;
                    return Ok(HttpReply {
                        status: 200,
                        body: vec![],
                        content_type: "text/event-stream".into(),
                        cookie: None,
                        waiting: None,
                        observation: Some((stream, permit, cursor)),
                    });
                }
                Ok(HttpReply::json(value))
            }
            Capability::Presentation(observe) => {
                expect_method(method, "POST")?;
                let session_id = input["session_id"].as_str().ok_or_else(invalid)?;
                let turn_id = input["turn_id"].as_str().ok_or_else(invalid)?;
                let user = context.user.lock().unwrap();
                let session = user
                    .agent_history
                    .sessions
                    .iter()
                    .find(|s| s.id == session_id)
                    .ok_or_else(authorization::missing)?;
                let turn = session
                    .turns
                    .iter()
                    .find(|t| t.turn_id == turn_id)
                    .ok_or_else(authorization::missing)?;
                let reference = turn
                    .published_book_ref
                    .as_ref()
                    .ok_or_else(authorization::missing)?;
                let publication = access
                    .library
                    .lock()
                    .unwrap()
                    .load(context.user_id(), reference)
                    .map_err(|_| authorization::missing())?;
                let private = crate::PrivateBookContext {
                    user: &user,
                    book: &publication.book,
                    book_dir: publication.directory(),
                    messages: &session.messages,
                    selected_chat: Some(session_id),
                };
                let reply = crate::presentation_api::route(&private, body, observe);
                if reply.status != 200 {
                    return Err(authorization::missing());
                }
                Ok(HttpReply::from_reply(reply))
            }
            Capability::Me | Capability::Logout | Capability::Allowance | Capability::AccountUsage | Capability::EmailBinding | Capability::Password => unreachable!(),
        }
    })();
    match result {
        Ok(reply) => reply,
        Err(e) if e.category == "method" => {
            let mut r = HttpReply::error(e);
            r.status = 405;
            r
        }
        Err(e) => HttpReply::error(e),
    }
}

// Admission storage keeps private turn facts; browser recovery needs the same
// quote/effect labels and answer projection as chat history and live completion.
fn reader_run_view(
    access: &Authorization,
    context: &authorization::AuthorizedContext,
    mut value: Value,
) -> Result<Value, ToolError> {
    if let Some(id) = value["turn"]["turn_id"].as_str() {
        let user = context.user.lock().unwrap();
        let turn = user.agent_history.sessions.iter()
            .flat_map(|session| &session.turns)
            .find(|turn| turn.turn_id == id)
            .ok_or_else(authorization::missing)?;
        let reference = turn.published_book_ref.as_ref().ok_or_else(authorization::missing)?;
        let publication = access.library.lock().unwrap().load(context.user_id(), reference)?;
        let view = json!(crate::turn_view(&publication.book, turn));
        value["turn"].as_object_mut().unwrap().extend(view.as_object().unwrap().clone());
    }
    Ok(value)
}

fn admit_reply(
    access: &Arc<Authorization>,
    context: &authorization::AuthorizedContext,
    workspace: &str,
    input: &Value,
    now: &str,
    wait: bool,
) -> Result<HttpReply, ToolError> {
    let mut accepted = reader_run_view(access, context, access.runs.admit(access, context, workspace, input, now)?)?;
    if accepted["dispatch_state"] == "admission_failed" {
        accepted["error_code"] = json!("ADMISSION_FAILED");
        let mut reply = HttpReply::json(accepted);
        reply.status = 409;
        return Ok(reply);
    }
    if !wait {
        let mut reply = HttpReply::json(accepted);
        reply.status = 202;
        return Ok(reply);
    }
    let turn = accepted["turn_id"].as_str().ok_or_else(invalid)?.to_owned();
    let permit = access
        .resources
        .try_acquire(crate::service_limits::Resource::SyncWait, context.user_id());
    let Some(permit) = permit else {
        // Acceptance already happened; return the same turn for asynchronous observation.
        let mut reply = HttpReply::json(accepted);
        reply.status = 202;
        return Ok(reply);
    };
    let mut reply = HttpReply::json(accepted);
    reply.waiting = Some(SyncWait {
        context: authorization::AuthorizedContext {
            principal: context.principal.clone(),
            user: context.user.clone(),
        },
        turn,
        _permit: permit,
    });
    Ok(reply)
}
struct SyncWait {
    context: authorization::AuthorizedContext,
    turn: String,
    _permit: crate::service_limits::Permit,
}
fn wait_for_turn(access: &Arc<Authorization>, wait: &SyncWait) -> HttpReply {
    let result = (|| -> Result<HttpReply, ToolError> {
        let context = &wait.context;
        let turn = wait.turn.as_str();
        loop {
            if access.runs.is_stopping() {
                return Err(crate::user_storage_paths::error(
                    "SERVICE_STOPPING",
                    "unavailable",
                    "Service is stopping",
                ));
            }
            access.auth.validate(&context.principal, self::now())?;
            let status = access.runs.status(context, turn)?;
            if status["persistence_state"] == "failed" {
                return Err(crate::user_storage_paths::error(
                    "TURN_UNSAVED",
                    "unavailable",
                    "Turn result is not saved",
                ));
            }
            if matches!(
                status["dispatch_state"].as_str(),
                Some("settled" | "admission_failed")
            ) {
                if !status["turn"]["outcome"].is_null() {
                    return Ok(HttpReply::json(status["turn"]["outcome"].clone()));
                }
                return Ok(HttpReply::json(status));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    })();
    result.unwrap_or_else(HttpReply::error)
}
