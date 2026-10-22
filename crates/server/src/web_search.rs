//! DeepSeek web retrieval. Network attempts share allowance with model calls;
//! cached reads and pagination never reserve another supplier request.
use crate::{
    model_spend_store::ModelSpendStore,
    run_scope::RunScope,
    web_usage::{WebChargeOutcome, WebServiceKind},
};
use read_tools::ToolError;
use runtime::{
    run_context::CancellationToken,
    web::{FetchRequest, SearchRequest},
};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    io::Read,
    net::ToSocketAddrs,
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::{Duration, Instant},
};

const SEARCH_TTL: Duration = Duration::from_secs(300);
const FETCH_TTL: Duration = Duration::from_secs(1800);
const FAILURE_TTL: Duration = Duration::from_secs(30);
const CACHE_CAPACITY: usize = 128;
const CACHE_BYTES: usize = 16 * 1024 * 1024;
const TOTAL_TIMEOUT: Duration = Duration::from_secs(45);
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(15);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;

fn error(code: &str, message: &str) -> ToolError {
    ToolError {
        error_code: code.into(),
        category: "unavailable".into(),
        message: message.into(),
    }
}
fn copy_error(e: &ToolError) -> ToolError {
    ToolError {
        error_code: e.error_code.clone(),
        category: e.category.clone(),
        message: e.message.clone(),
    }
}

#[derive(Clone, PartialEq)]
struct Config {
    base: String,
    key: String,
    search_qps: u32,
    fetch_qps: u32,
}
impl Config {
    fn load() -> Result<Self, ToolError> {
        let key = std::env::var("DEEPSEEK_SEARCH_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(runtime::web::unavailable)?;
        let base = std::env::var("DEEPSEEK_SEARCH_BASE_URL")
            .unwrap_or_else(|_| "https://api.deepseek.com".into());
        let parsed = url::Url::parse(&base)
            .map_err(|_| error("WEB_CONFIG_INVALID", "Invalid web API base URL"))?;
        if !matches!(parsed.scheme(), "http" | "https")
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(error("WEB_CONFIG_INVALID", "Invalid web API base URL"));
        }
        let qps = |name: &str, default: u32| -> Result<u32, ToolError> {
            match std::env::var(name) {
                Ok(v) => v
                    .parse::<u32>()
                    .ok()
                    .filter(|q| (1..=1000).contains(q))
                    .ok_or_else(|| {
                        error("WEB_CONFIG_INVALID", "Web QPS must be between 1 and 1000")
                    }),
                Err(_) => Ok(default),
            }
        };
        Ok(Self {
            base: base.trim_end_matches('/').into(),
            key,
            search_qps: qps("DEEPSEEK_SEARCH_QPS", 100)?,
            fetch_qps: qps("DEEPSEEK_FETCH_QPS", 25)?,
        })
    }
}

pub(crate) fn available() -> bool {
    Config::load().is_ok()
}

pub(crate) fn call(
    scope: &RunScope,
    spend: &ModelSpendStore,
    name: &str,
    args: Value,
    cancel: &CancellationToken,
) -> Result<Value, ToolError> {
    let request = Request::parse(name, args)?;
    cancel.check()?;
    let config = Config::load()?;
    static CLIENTS: OnceLock<Mutex<Vec<Arc<Client>>>> = OnceLock::new();
    let client = {
        let mut clients = CLIENTS
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap();
        if let Some(client) = clients.iter().find(|c| c.config == config) {
            client.clone()
        } else {
            let client = Arc::new(Client::new(config));
            clients.push(client.clone());
            client
        }
    };
    client
        .call(request, &RunMeter { scope, spend }, cancel)
        .map_err(
            |error| match runtime::model_spend::SpendStop::from_code(&error.error_code) {
                Some(stop) => stop.tool_error(),
                None => error,
            },
        )
}

enum Request {
    Search(SearchRequest),
    Fetch(FetchRequest),
}
impl Request {
    fn parse(name: &str, args: Value) -> Result<Self, ToolError> {
        match name {
            "web.search" => runtime::web::parse_search(args).map(Self::Search),
            "web.fetch" => runtime::web::parse_fetch(args).map(Self::Fetch),
            _ => Err(error("WEB_TOOL_UNKNOWN", "Unknown web tool")),
        }
    }
    fn kind(&self) -> WebServiceKind {
        match self {
            Self::Search(_) => WebServiceKind::Search,
            Self::Fetch(_) => WebServiceKind::Fetch,
        }
    }
    fn endpoint(&self) -> &'static str {
        match self {
            Self::Search(_) => "search",
            Self::Fetch(_) => "fetch",
        }
    }
    fn refresh(&self) -> bool {
        match self {
            Self::Search(r) => r.refresh,
            Self::Fetch(r) => r.refresh,
        }
    }
    fn payload(&self) -> Value {
        match self {
            Self::Search(r) => {
                let mut v = json!({"query":r.query,"count":r.count,"need_content":r.need_content});
                if let Some(question) = &r.ori_question {
                    v["ori_question"] = json!(question);
                }
                if let Some(ip) = &r.localization {
                    v["localization"] = json!(ip);
                }
                v
            }
            Self::Fetch(r) => json!({"url":r.url}),
        }
    }
    fn key(&self) -> String {
        format!("{}:{}", self.endpoint(), self.payload())
    }
    fn ttl(&self) -> Duration {
        match self {
            Self::Search(_) => SEARCH_TTL,
            Self::Fetch(_) => FETCH_TTL,
        }
    }
}

// This seam lets transport tests assert the exact physical attempts that will
// reserve/settle a charge, without substituting the server's accounting logic.
trait Meter {
    fn admit(&self, kind: WebServiceKind, logical: &str, attempt: u32)
        -> Result<String, ToolError>;
    fn receipt(
        &self,
        id: &str,
        outcome: WebChargeOutcome,
        evidence: Value,
    ) -> Result<Value, ToolError>;
}
struct RunMeter<'a> {
    scope: &'a RunScope,
    spend: &'a ModelSpendStore,
}
impl Meter for RunMeter<'_> {
    fn admit(
        &self,
        kind: WebServiceKind,
        logical: &str,
        attempt: u32,
    ) -> Result<String, ToolError> {
        self.spend.web_admit_run(
            self.scope,
            kind,
            logical,
            attempt,
            crate::multi_user_host::now(),
        )
    }
    fn receipt(
        &self,
        id: &str,
        outcome: WebChargeOutcome,
        evidence: Value,
    ) -> Result<Value, ToolError> {
        self.spend.web_receipt(
            &self.scope.turn_id,
            id,
            outcome,
            evidence,
            crate::multi_user_host::now(),
        )
    }
}

#[derive(Clone)]
struct Saved {
    body: Value,
    at: Instant,
    observed_at: i64,
}
struct Entry {
    key: String,
    saved: Saved,
    ttl: Duration,
    bytes: usize,
}
struct Failed {
    key: String,
    error: Arc<ToolError>,
    at: Instant,
}
struct Flight {
    result: Mutex<Option<Arc<Result<Saved, ToolError>>>>,
    ready: Condvar,
}
#[derive(Default)]
struct State {
    cache: VecDeque<Entry>,
    failures: VecDeque<Failed>,
    flights: Vec<(String, Arc<Flight>)>,
    search_next: Option<Instant>,
    fetch_next: Option<Instant>,
}
struct Client {
    config: Config,
    http: ureq::Agent,
    state: Mutex<State>,
}
enum Lookup {
    Hit(Saved),
    Failed(Arc<ToolError>),
    Wait(Arc<Flight>),
    Lead(Arc<Flight>),
    MissingPage,
}
impl Client {
    fn new(config: Config) -> Self {
        let http = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout_connect(CONNECT_TIMEOUT)
            .resolver(|address: &str| {
                let address = address.to_owned();
                crate::account_mail::resolve_bounded(
                    move || address.to_socket_addrs().map(|a| a.collect()),
                    CONNECT_TIMEOUT,
                )
            })
            .build();
        Self {
            config,
            http,
            state: Mutex::new(State::default()),
        }
    }
    fn lookup(&self, request: &Request, key: &str) -> Lookup {
        let mut state = self.state.lock().unwrap();
        state.cache.retain(|e| e.saved.at.elapsed() < e.ttl);
        state.failures.retain(|e| e.at.elapsed() < FAILURE_TTL);
        if !request.refresh() {
            if let Some(entry) = state.cache.iter().find(|e| e.key == key) {
                return Lookup::Hit(entry.saved.clone());
            }
            if let Some(entry) = state.failures.iter().find(|e| e.key == key) {
                return Lookup::Failed(entry.error.clone());
            }
        }
        if let Some((_, flight)) = state.flights.iter().find(|(k, _)| k == key) {
            return Lookup::Wait(flight.clone());
        }
        if matches!(request, Request::Fetch(r) if r.offset > 0 && !r.refresh) {
            return Lookup::MissingPage;
        }
        let flight = Arc::new(Flight {
            result: Mutex::new(None),
            ready: Condvar::new(),
        });
        state.flights.push((key.into(), flight.clone()));
        Lookup::Lead(flight)
    }
    fn cache(state: &mut State, key: String, saved: Saved, ttl: Duration) {
        state.cache.retain(|e| e.key != key);
        let bytes = value_bytes(&saved.body) + key.len();
        state.cache.push_back(Entry {
            key,
            saved,
            ttl,
            bytes,
        });
        let mut total: usize = state.cache.iter().map(|e| e.bytes).sum();
        while state.cache.len() > CACHE_CAPACITY || total > CACHE_BYTES {
            if let Some(entry) = state.cache.pop_front() {
                total = total.saturating_sub(entry.bytes);
            } else {
                break;
            }
        }
    }
    fn call(
        &self,
        request: Request,
        meter: &dyn Meter,
        cancel: &CancellationToken,
    ) -> Result<Value, ToolError> {
        let started = Instant::now();
        let deadline = started + TOTAL_TIMEOUT;
        let key = request.key();
        cancel.check()?;
        let mut usage = json!({"provider":"deepseek","service_kind":match request {Request::Search(_)=>"web_search",Request::Fetch(_)=>"web_fetch"},"physical_requests":0,"queue_ms":0,"request_ms":0,"charges":[]});
        let saved = match self.lookup(&request, &key) {
            Lookup::Failed(error) => return Err(copy_error(&error)),
            Lookup::MissingPage => return Err(error("WEB_PAGE_EXPIRED", "The cached page is no longer available. A continuation does not purchase another fetch; explicitly refresh from offset 0 if this page is still needed.")),
            Lookup::Hit(saved) => { usage["cache"] = json!("hit"); saved }
            Lookup::Wait(flight) => {
                usage["cache"] = json!("coalesced");
                let mut value = flight.result.lock().unwrap();
                while value.is_none() {
                    check(cancel, deadline)?;
                    value = flight.ready.wait_timeout(value, Duration::from_millis(25)).unwrap().0;
                }
                usage["queue_ms"] = json!(millis(started.elapsed()));
                match value.as_ref().unwrap().as_ref() { Ok(saved) => saved.clone(), Err(e) => return Err(copy_error(e)) }
            }
            Lookup::Lead(flight) => {
                usage["cache"] = json!("miss");
                let result = self.send(&request, meter, cancel, deadline, &mut usage);
                let shared = Arc::new(result);
                {
                    let mut state = self.state.lock().unwrap();
                    if let Ok(saved) = shared.as_ref() {
                        state.failures.retain(|e| e.key != key);
                        // Empty search results are cached too: repeating an identical
                        // query cannot add evidence and should not loop on empty 200s.
                        Self::cache(&mut state, key.clone(), saved.clone(), request.ttl());
                        if let Request::Search(_) = request {
                            for row in saved.body["results"].as_array().into_iter().flatten() {
                                if row["content"].as_str().is_some_and(|s| !s.trim().is_empty()) {
                                    if let Some(url) = row["url"].as_str() {
                                        let page = Saved { body: json!({"url":url,"title":row["title"],"content":row["content"]}), ..saved.clone() };
                                        Self::cache(&mut state, format!("fetch:{}", json!({"url":url})), page, FETCH_TTL);
                                    }
                                }
                            }
                        }
                    } else if let Err(e) = shared.as_ref() {
                        // An Agent repeating the same failed request should not
                        // restart a fresh set of paid/unknown attempts immediately.
                        // Account admission and cancellation errors are never shared.
                        if e.error_code == "WEB_RETRIEVAL_FAILED" {
                            state.failures.retain(|e| e.key != key);
                            state.failures.push_back(Failed {key:key.clone(),error:Arc::new(copy_error(e)),at:Instant::now()});
                            while state.failures.len() > CACHE_CAPACITY { state.failures.pop_front(); }
                        }
                    }
                    *flight.result.lock().unwrap() = Some(shared.clone());
                    state.flights.retain(|(k, _)| k != &key);
                }
                flight.ready.notify_all();
                match shared.as_ref() { Ok(saved) => saved.clone(), Err(e) => return Err(copy_error(e)) }
            }
        };
        cancel.check()?;
        usage["elapsed_ms"] = json!(millis(started.elapsed()));
        usage["cache_age_ms"] = json!(millis(saved.at.elapsed()));
        let mut result = project(&request, &saved.body)?;
        result["source_scope"] = json!("web");
        result["observed_at"] = json!(saved.observed_at);
        result["usage"] = usage;
        Ok(result)
    }
    fn queue(
        &self,
        request: &Request,
        cancel: &CancellationToken,
        deadline: Instant,
    ) -> Result<u64, ToolError> {
        let started = Instant::now();
        let due = {
            let mut state = self.state.lock().unwrap();
            let (next, qps) = match request {
                Request::Search(_) => (&mut state.search_next, self.config.search_qps),
                Request::Fetch(_) => (&mut state.fetch_next, self.config.fetch_qps),
            };
            let now = Instant::now();
            let due = next.unwrap_or(now).max(now);
            let step = Duration::from_secs_f64(1.0 / f64::from(qps));
            if due >= deadline {
                return Err(error(
                    "WEB_QUEUE_TIMEOUT",
                    "Web retrieval queue exceeded the tool deadline",
                ));
            }
            *next = Some(due + step);
            due
        };
        wait_until(due, cancel, deadline)?;
        Ok(millis(started.elapsed()))
    }
    fn send(
        &self,
        request: &Request,
        meter: &dyn Meter,
        cancel: &CancellationToken,
        deadline: Instant,
        usage: &mut Value,
    ) -> Result<Saved, ToolError> {
        let logical = format!("web_{}", uuid::Uuid::now_v7());
        for attempt in 1..=3 {
            check(cancel, deadline)?;
            let queue_ms = self.queue(request, cancel, deadline)?;
            check(cancel, deadline)?;
            // The OS resolver uses the shared bounded helper. Leave enough time
            // for its bound before reserving a request, including late retries.
            if deadline.saturating_duration_since(Instant::now()) < CONNECT_TIMEOUT {
                return Err(error(
                    "WEB_TIMEOUT",
                    "Too little time remains for another bounded web request",
                ));
            }
            let id = meter.admit(request.kind(), &logical, attempt)?;
            // Admission is immediately before send; cancellation during admission
            // releases this unsent reservation rather than leaving it unknown.
            if cancel.is_cancelled() || Instant::now() >= deadline {
                meter.receipt(&id, WebChargeOutcome::Free, json!({"source":"web_transport","reason":"not_sent","attempt":attempt,"queue_ms":queue_ms}))?;
                check(cancel, deadline)?;
            }
            let started = Instant::now();
            let timeout = ATTEMPT_TIMEOUT.min(deadline.saturating_duration_since(started));
            let response = self
                .http
                .post(&format!(
                    "{}/v1/tools/{}",
                    self.config.base,
                    request.endpoint()
                ))
                .set("Authorization", &format!("Bearer {}", self.config.key))
                .timeout(timeout)
                .send_json(request.payload());
            let mut status = None;
            let mut retry_after = Duration::from_millis(250 * u64::from(attempt));
            let (body, outcome, reason, retry) = match response {
                Ok(response) => {
                    status = Some(response.status());
                    let mut bytes = Vec::new();
                    let read = response
                        .into_reader()
                        .take(MAX_RESPONSE_BYTES + 1)
                        .read_to_end(&mut bytes);
                    let parsed = if read.is_ok() && bytes.len() as u64 <= MAX_RESPONSE_BYTES {
                        serde_json::from_slice::<Value>(&bytes).ok()
                    } else {
                        None
                    };
                    match parsed {
                        Some(value) if value.get("error").is_some_and(|v| !v.is_null()) => {
                            (None, WebChargeOutcome::Free, "api_error", false)
                        }
                        Some(value) => match valid_body(request, &value) {
                            Some(billed) => (
                                Some(value),
                                if billed {
                                    WebChargeOutcome::Billed
                                } else {
                                    WebChargeOutcome::Free
                                },
                                if billed {
                                    "content_received"
                                } else {
                                    "empty_result"
                                },
                                false,
                            ),
                            None => (None, WebChargeOutcome::Unknown, "invalid_response", false),
                        },
                        None => (
                            None,
                            WebChargeOutcome::Unknown,
                            "unreadable_response",
                            false,
                        ),
                    }
                }
                Err(ureq::Error::Status(code, response)) => {
                    status = Some(code);
                    if let Some(seconds) = response
                        .header("Retry-After")
                        .and_then(|v| v.parse::<u64>().ok())
                    {
                        retry_after = Duration::from_secs(seconds.min(30));
                    }
                    (
                        None,
                        WebChargeOutcome::Free,
                        "http_error",
                        matches!(code, 408 | 429 | 500..=599),
                    )
                }
                Err(ureq::Error::Transport(_)) => (
                    None,
                    WebChargeOutcome::Unknown,
                    "transport_outcome_unknown",
                    true,
                ),
            };
            let request_ms = millis(started.elapsed());
            let receipt = meter.receipt(&id, outcome, json!({"source":"web_transport","attempt":attempt,"http_status":status,"reason":reason,"queue_ms":queue_ms,"request_ms":request_ms}))?;
            usage["physical_requests"] = json!(attempt);
            usage["queue_ms"] = json!(usage["queue_ms"].as_u64().unwrap_or(0) + queue_ms);
            usage["request_ms"] = json!(usage["request_ms"].as_u64().unwrap_or(0) + request_ms);
            let charge = &receipt["charge"];
            usage["charges"].as_array_mut().unwrap().push(json!({"usage_id":id,"attempt":attempt,"http_status":status,"state":charge["state"],"provider_cost_status":charge["provider_cost_status"],"provider_cost_micro_cny":charge["provider_cost_micro_cny"],"account_debit_micro_cny":charge["account_debit_micro_cny"]}));
            if let Some(body) = body {
                // Retain paid content before honoring this caller's cancellation.
                // Client::call publishes it to the cache and coalesced readers,
                // then checks cancellation before returning to this caller.
                return Ok(Saved {
                    body,
                    at: Instant::now(),
                    observed_at: crate::multi_user_host::now(),
                });
            }
            cancel.check()?;
            if !retry || attempt == 3 {
                return Err(error("WEB_RETRIEVAL_FAILED", &format!("Web retrieval failed ({reason}, HTTP {}). Attempt usage was recorded; do not repeat an unchanged failing request.", status.map(|s|s.to_string()).unwrap_or_else(||"unknown".into()))));
            }
            wait_until(Instant::now() + retry_after, cancel, deadline)?;
        }
        unreachable!()
    }
}

fn valid_body(request: &Request, body: &Value) -> Option<bool> {
    match request {
        Request::Search(_) => body["results"].as_array().map(|r| !r.is_empty()),
        Request::Fetch(_) => body["content"].as_str().map(|s| !s.trim().is_empty()),
    }
}
fn check(cancel: &CancellationToken, deadline: Instant) -> Result<(), ToolError> {
    cancel.check()?;
    if Instant::now() >= deadline {
        Err(error(
            "WEB_TIMEOUT",
            "Web retrieval exceeded the tool deadline",
        ))
    } else {
        Ok(())
    }
}
fn wait_until(
    until: Instant,
    cancel: &CancellationToken,
    deadline: Instant,
) -> Result<(), ToolError> {
    while Instant::now() < until {
        check(cancel, deadline)?;
        std::thread::sleep(
            Duration::from_millis(25).min(until.saturating_duration_since(Instant::now())),
        );
    }
    check(cancel, deadline)
}
fn millis(value: Duration) -> u64 {
    value.as_millis().min(u128::from(u64::MAX)) as u64
}
fn value_bytes(value: &Value) -> usize {
    match value {
        Value::String(s) => s.len(),
        Value::Array(values) => values.iter().map(value_bytes).sum(),
        Value::Object(fields) => fields.iter().map(|(k, v)| k.len() + value_bytes(v)).sum(),
        _ => 16,
    }
}
fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}
fn prefix(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}
fn content_status(content: &str) -> &'static str {
    if content.trim().is_empty() {
        return "unavailable";
    }
    let lower = content.to_lowercase();
    if content.chars().count() < 2000
        && [
            "access denied",
            "verify you are human",
            "captcha required",
            "please sign in to continue",
            "login required",
            "请登录后查看",
            "登录后阅读全文",
            "请检查插件以恢复正常内容展示",
        ]
        .iter()
        .any(|m| lower.contains(m))
    {
        "blocked"
    } else {
        "available"
    }
}
fn project(request: &Request, body: &Value) -> Result<Value, ToolError> {
    match request {
        Request::Search(r) => {
            let rows = body["results"].as_array().ok_or_else(|| {
                error(
                    "WEB_RESPONSE_INVALID",
                    "Search response has no results array",
                )
            })?;
            let content_limit = 6000 / usize::from(r.count);
            let results: Vec<Value> = rows.iter().take(usize::from(r.count)).map(|row| {
                let summary = text(&row["summary"]);
                let content = text(&row["content"]);
                let total = content.chars().count();
                json!({"title":prefix(text(&row["title"]),300),"url":row["url"],"date":row["date"],
                    "summary":prefix(summary,800),"summary_truncated":summary.chars().count()>800,
                    "snippet":prefix(text(&row["snippet"]),200),"content":prefix(content,content_limit),
                    "content_status":if content.is_empty(){"summary_only"}else{content_status(content)},
                    "total_chars":total,"content_truncated":total>content_limit,
                    "next_offset":if total>content_limit {Some(content_limit)}else{None},
                    "authority":row["authority"],"rerank_score":row["rerank_score"]})
            }).collect();
            Ok(
                json!({"query":r.query,"results":results,"evidence_note":"Search summaries and rankings locate candidate pages. Available text still needs relevance and claim checks; authority is not a truth score."}),
            )
        }
        Request::Fetch(r) => {
            let content = text(&body["content"]);
            let total = content.chars().count();
            if r.offset > total {
                return Err(ToolError {
                    error_code: "WEB_OFFSET_INVALID".into(),
                    category: "validation".into(),
                    message: format!(
                        "offset exceeds the page length of {total} Unicode characters"
                    ),
                });
            }
            let page: String = content.chars().skip(r.offset).take(r.max_chars).collect();
            let end = r.offset + page.chars().count();
            Ok(
                json!({"url":r.url,"title":prefix(text(&body["title"]),300),"content":page,
                "offset":r.offset,"end_offset":end,"total_chars":total,"next_offset":if end<total{Some(end)}else{None},
                "content_status":content_status(content),"evidence_note":"External page text; check relevance and conditions before citing this URL. General mechanisms alone do not establish this project's intent or measured outcomes; those require its own primary evidence."}),
            )
        }
    }
}

#[cfg(test)]
#[path = "web_search_tests.rs"]
mod tests;
