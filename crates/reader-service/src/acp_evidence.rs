//! A private MCP connection bound to one frozen book and one Reader answer.
use book_tool_contracts::{
    contract_for, from_mcp_alias, input_schema, validate_input, BookToolId, BookToolInput,
};
use read_tools::{Book, EvidenceRange, ToolError};
use runtime::{
    orchestrator::{external_evidence::ExternalEvidence, OuterOutcome},
    run_events::RunEvents,
};
use serde_json::{json, Value};
use std::{
    io::{BufRead, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
};
use tiny_http::{Method, Response, Server};

mod actions;
use actions::ReaderActions;
use runtime::orchestrator::AgentEffect;

pub const READING_INSTRUCTIONS: &str =
    "Use the reader MCP tools for this book. Read a selected passage with book_text, or locate passages using book_search or book_search_text then read them. To cite a passage, call source_present with an exact quote from the selection or retrieved passage and put the returned marker after the supported sentence. Answer in the reader's language. Use visible titles and quotations when referring to the book.";

const BOOK_TOOLS: &[BookToolId] = &[
    BookToolId::Text,
    BookToolId::Search,
    BookToolId::SearchText,
    BookToolId::PaperMetadata,
];

fn error(code: &str, message: impl ToString) -> ToolError {
    ToolError {
        error_code: code.into(),
        category: "local_agent".into(),
        message: message.to_string(),
    }
}

struct EvidenceSession {
    book: Arc<Book>,
    turn: ExternalEvidence,
    actions: Option<ReaderActions>,
    effects: Vec<AgentEffect>,
    events: Option<RunEvents>,
}

impl EvidenceSession {
    fn call(&mut self, name: &str, args: Value) -> Result<Value, ToolError> {
        if let Some(actions) = &self.actions {
            if actions::is_action(name) {
                let activity = self
                    .events
                    .as_ref()
                    .map(|events| events.begin("tool", name, actions::label(name), true));
                let result = actions.apply(&self.turn, name, &args);
                if let (Some(events), Some(activity)) = (&self.events, activity) {
                    use runtime::run_events::ActivityStatus;
                    if let Ok((_, Some(effect))) = &result {
                        events.effect_created(activity.step_id, effect);
                    }
                    let status = if result.is_ok() {
                        ActivityStatus::Succeeded
                    } else {
                        ActivityStatus::Rejected
                    };
                    events.finish(
                        activity,
                        status,
                        None,
                        result.as_ref().err().map(|e| e.error_code.clone()),
                        None,
                    );
                }
                let (body, effect) = result?;
                self.effects.extend(effect);
                return Ok(body);
            }
        }
        if name == "source_present" {
            return self.turn.present(&self.book, &args);
        }
        let id = from_mcp_alias(name)
            .filter(|id| BOOK_TOOLS.contains(id))
            .ok_or_else(|| error("TOOL_NOT_FOUND", format!("Unknown reading tool {name}")))?;
        let input = validate_input(id, args.clone()).map_err(|err| error(err.code, err.message))?;
        let mut result = match input {
            BookToolInput::Text(input) => self.book.read_text(&input)?,
            BookToolInput::Search(input) => json!(self.book.search_for_reading(&input)?),
            BookToolInput::SearchText(input) => json!(self.book.search_text(&input)?),
            BookToolInput::Empty(_) if id == BookToolId::PaperMetadata => {
                json!(self.book.paper_metadata_projection())
            }
            _ => {
                return Err(error(
                    "BOOK_TOOL_CONTRACT_INVALID",
                    "Unsupported reading input",
                ))
            }
        };
        self.turn.observe(
            &self.book,
            contract_for(id).aliases.resident.unwrap(),
            &args,
            &result,
        );
        crate::mcp::project_candidate_read_calls(id, &mut result, None);
        Ok(result)
    }

    fn handle(&mut self, request: Value) -> Option<Value> {
        let id = request.get("id")?;
        let params = &request["params"];
        let result = match request["method"].as_str() {
            Some("initialize") => json!({
                "protocolVersion": match params["protocolVersion"].as_str() {
                    Some(v @ ("2025-11-25" | "2025-06-18" | "2025-03-26" | "2024-11-05")) => v,
                    _ => "2025-11-25",
                },
                "serverInfo":{"name":"understand-book-reading","version":env!("CARGO_PKG_VERSION")},
                "capabilities":{"tools":{}}, "instructions":READING_INSTRUCTIONS,
            }),
            Some("ping") => json!({}),
            Some("tools/list") => {
                let mut tools: Vec<_> = BOOK_TOOLS.iter().map(|&tool| {
                    let contract = contract_for(tool);
                    json!({"name":contract.aliases.mcp, "description":contract.mcp_description(),
                        "inputSchema":input_schema(tool),
                        "outputSchema":crate::mcp_output_schema::local(tool),
                        "annotations":crate::mcp_output_schema::local_annotations(tool)})
                }).collect();
                tools.push(json!({"name":"source_present","description":
                    "Create a clickable Reader citation from an exact quote observed in this turn. Put the returned marker after the sentence it supports.",
                    "annotations":crate::mcp_output_schema::free_read_annotations(),
                    "inputSchema":{"type":"object","properties":{"quote":{"type":"string","minLength":1}},
                        "required":["quote"],"additionalProperties":false}}));
                if self.actions.is_some() {
                    tools.extend(actions::tools());
                }
                json!({"tools":tools})
            }
            Some("tools/call") => {
                let (data, failed) = match self.call(
                    params["name"].as_str().unwrap_or(""),
                    params
                        .get("arguments")
                        .cloned()
                        .unwrap_or_else(|| json!({})),
                ) {
                    Ok(data) => (data, false),
                    Err(e) => (json!(e), true),
                };
                json!({"content":[{"type":"text","text":data.to_string()}],
                    "structuredContent":data,"isError":failed})
            }
            _ => {
                return Some(
                    json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Method not found"}}),
                )
            }
        };
        Some(json!({"jsonrpc":"2.0", "id":id, "result":result}))
    }
}

pub struct AcpEvidence {
    session: Arc<Mutex<EvidenceSession>>,
    listener: Arc<Server>,
    closed: Arc<AtomicBool>,
    endpoint: String,
    worker: Option<thread::JoinHandle<()>>,
}

impl AcpEvidence {
    pub fn start(
        book: Arc<Book>,
        seed: Vec<EvidenceRange>,
        question: &str,
    ) -> Result<Self, ToolError> {
        let turn = ExternalEvidence::new(&book, seed, question)?;
        let session = Arc::new(Mutex::new(EvidenceSession {
            book,
            turn,
            actions: None,
            effects: Vec::new(),
            events: None,
        }));
        let listener =
            Arc::new(Server::http("127.0.0.1:0").map_err(|e| error("ACP_MCP_START_FAILED", e))?);
        let path = format!("/turn/{}", uuid::Uuid::now_v7());
        let endpoint = format!("http://{}{path}", listener.server_addr());
        let closed = Arc::new(AtomicBool::new(false));
        let server = listener.clone();
        let state = session.clone();
        let stopped = closed.clone();
        let worker = thread::Builder::new()
            .name("reader-acp-evidence".into())
            .spawn(move || {
                while !stopped.load(Ordering::Acquire) {
                    let Ok(mut request) = server.recv() else {
                        break;
                    };
                    if request.url() != path || *request.method() != Method::Post {
                        let _ = request.respond(Response::empty(404));
                        continue;
                    }
                    let response = serde_json::from_reader(request.as_reader()).map(|rpc| {
                        let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
                        state.handle(rpc)
                    });
                    match response {
                        Ok(Some(value)) => {
                            let _ = request.respond(Response::from_string(value.to_string()));
                        }
                        Ok(None) => {
                            let _ = request.respond(Response::empty(204));
                        }
                        Err(_) => {
                            let _ = request.respond(Response::empty(400));
                        }
                    }
                }
            })
            .map_err(|e| error("ACP_MCP_START_FAILED", e))?;
        Ok(Self {
            session,
            listener,
            closed,
            worker: Some(worker),
            endpoint,
        })
    }

    pub fn mcp_server(&self) -> Result<Value, ToolError> {
        let executable =
            std::env::current_exe().map_err(|e| error("ACP_MCP_EXECUTABLE_MISSING", e))?;
        Ok(json!({"name":"understand-book", "command":executable,
            "args":["--acp-turn-tools", self.endpoint], "env":[]}))
    }

    pub fn start_preview(&self, events: RunEvents) {
        let mut state = self.session.lock().unwrap_or_else(|e| e.into_inner());
        state.events = Some(events.clone());
        state.turn.start_preview(events);
    }

    pub(crate) fn enable_actions(
        &self,
        state: Arc<Mutex<crate::AppState>>,
        scope: crate::run_scope::RunScope,
        now: String,
        cancellation: runtime::run_context::CancellationToken,
    ) {
        self.session
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .actions = Some(ReaderActions {
            state,
            scope,
            now,
            cancellation,
        });
    }

    pub fn action_effects(&self) -> Vec<AgentEffect> {
        self.session
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .effects
            .clone()
    }

    pub fn push_text(&self, delta: &str) {
        self.session
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .turn
            .push_text(delta);
    }

    pub fn finish(self, raw: &str) -> Result<OuterOutcome, ToolError> {
        let mut state = self.session.lock().unwrap_or_else(|e| e.into_inner());
        let mut outcome = state.turn.finish(raw)?;
        outcome.effects = std::mem::take(&mut state.effects);
        Ok(outcome)
    }
}

impl Drop for AcpEvidence {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Release);
        self.listener.unblock();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Entered before desktop initialization so a tool process never opens a window or loads private state.
pub fn serve_stdio_if_requested() -> bool {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("--acp-turn-tools") {
        return false;
    }
    let endpoint = args.next().unwrap_or_default();
    if let Err(reason) = serve_stdio(&endpoint) {
        eprintln!("Reader MCP: {reason}");
        std::process::exit(1);
    }
    true
}

fn serve_stdio(endpoint: &str) -> Result<(), String> {
    let url = url::Url::parse(endpoint).map_err(|e| e.to_string())?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || !url.path().starts_with("/turn/")
    {
        return Err("Expected a local Reader turn".into());
    }
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    let mut stdout = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let response = agent
            .post(endpoint)
            .set("Content-Type", "application/json")
            .send_string(&line)
            .map_err(|e| e.to_string())?;
        if response.status() != 204 {
            let body = response.into_string().map_err(|e| e.to_string())?;
            writeln!(stdout, "{body}")
                .and_then(|()| stdout.flush())
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
