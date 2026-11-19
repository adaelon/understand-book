use super::permission::{choose, PermissionContext};
use super::{agent_environment, find_command, AgentConfig};
use runtime::run_context::CancellationToken;
use runtime::run_events::RunEvents;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

struct Connection<'a> {
    child: Child,
    outgoing: Option<mpsc::Sender<Value>>,
    incoming: mpsc::Receiver<Result<Value, String>>,
    stderr: Arc<Mutex<String>>,
    next_id: u64,
    session: Option<String>,
    permissions: Option<PermissionContext<'a>>,
}

impl<'a> Connection<'a> {
    fn start(
        agent: &AgentConfig,
        cwd: &Path,
        permissions: Option<PermissionContext<'a>>,
    ) -> Result<Self, String> {
        let bin = find_command(&agent.command)
            .ok_or_else(|| format!("找不到 Agent 启动命令：{}", agent.command))?;
        let mut command = Command::new(bin);
        agent_environment(&mut command);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command
            .args(&agent.args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("启动 {} 失败：{e}", agent.name))?;
        let mut stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        let (sender, receiver) = mpsc::channel::<Value>();
        let (responses, incoming) = mpsc::sync_channel(64);
        let write_errors = responses.clone();
        thread::spawn(move || {
            for message in receiver {
                let mut line = message.to_string();
                line.push('\n');
                if let Err(error) = stdin.write_all(line.as_bytes()).and_then(|_| stdin.flush()) {
                    let _ = write_errors.send(Err(format!("ACP 输入已断开：{error}")));
                    break;
                }
            }
        });
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let message = line.map_err(|e| e.to_string()).and_then(|line| {
                    serde_json::from_str(&line).map_err(|e| format!("ACP 输出不是 JSON-RPC：{e}"))
                });
                let fail = message.is_err();
                if responses.send(message).is_err() || fail {
                    return;
                }
            }
            let _ = responses.send(Err("ACP 进程已关闭输出".into()));
        });
        let last_error = Arc::new(Mutex::new(String::new()));
        let diagnostics = Arc::clone(&last_error);
        thread::spawn(move || {
            for line in BufReader::new(stderr).split(b'\n').map_while(Result::ok) {
                let tail = String::from_utf8_lossy(&line);
                if !tail.trim().is_empty() {
                    *diagnostics.lock().unwrap_or_else(|e| e.into_inner()) =
                        tail.trim().chars().take(600).collect();
                }
            }
        });
        Ok(Self {
            child,
            outgoing: Some(sender),
            incoming,
            stderr: last_error,
            next_id: 1,
            session: None,
            permissions,
        })
    }

    fn send(&self, message: Value) -> Result<(), String> {
        self.outgoing
            .as_ref()
            .ok_or("ACP 连接已关闭")?
            .send(message)
            .map_err(|_| "ACP 输入已断开".into())
    }

    fn request(
        &mut self,
        method: &str,
        params: Value,
        timeout: Option<Duration>,
        cancel: &CancellationToken,
        updates: &mut impl FnMut(&Value),
    ) -> Result<Value, String> {
        if cancel.is_cancelled() {
            return Err("ACP 回合已取消".into());
        }
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        let start = Instant::now();
        loop {
            if cancel.is_cancelled() {
                if let Some(session) = &self.session {
                    let _ = self.send(json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":session}}));
                }
                return Err("ACP 回合已取消".into());
            }
            if timeout.is_some_and(|t| start.elapsed() >= t) {
                let diagnostic = self.stderr.lock().unwrap_or_else(|e| e.into_inner());
                let mut reason = format!("等待 ACP {method} 超时。请在终端检查 Agent 能否完成首次启动、登录和模型目录加载，再重新连接。");
                if !diagnostic.is_empty() {
                    reason.push_str(diagnostic.as_str());
                }
                return Err(reason);
            }
            let message = match self.incoming.recv_timeout(Duration::from_millis(75)) {
                Ok(Ok(message)) => message,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                other => {
                    let reason = match other {
                        Ok(Err(message)) => message,
                        _ => "ACP 连接已关闭".into(),
                    };
                    let diagnostic = self.stderr.lock().unwrap_or_else(|e| e.into_inner());
                    return Err(if diagnostic.is_empty() {
                        reason
                    } else {
                        format!("{reason}。{diagnostic}")
                    });
                }
            };
            if let Some(inbound_method) = message["method"].as_str() {
                if let Some(inbound_id) = message.get("id") {
                    let response = if inbound_method == "session/request_permission" {
                        let outcome = self
                            .permissions
                            .as_ref()
                            .filter(|_| {
                                method == "session/prompt"
                                    && message["params"]["sessionId"].as_str()
                                        == self.session.as_deref()
                            })
                            .map(|ctx| {
                                choose(
                                    ctx,
                                    &message["params"],
                                    cancel,
                                    &mut self.child,
                                    Duration::from_secs(180),
                                )
                            })
                            .unwrap_or_else(|| json!({"outcome":"cancelled"}));
                        json!({"jsonrpc":"2.0","id":inbound_id,"result":{"outcome":outcome}})
                    } else {
                        json!({"jsonrpc":"2.0","id":inbound_id,"error":{"code":-32601,"message":"Unsupported client method"}})
                    };
                    self.send(response)?;
                } else if inbound_method == "session/update"
                    && method == "session/prompt"
                    && message.pointer("/params/sessionId").and_then(Value::as_str)
                        == self.session.as_deref()
                {
                    updates(&message["params"]["update"]);
                }
            } else if message["id"].as_u64() == Some(id) {
                if let Some(error) = message.get("error") {
                    let reason = error["message"].as_str().unwrap_or("协议请求失败");
                    if error["code"].as_i64() == Some(-32000) {
                        return Err(format!("请先在 Agent 中登录，再测试连接。{reason}"));
                    }
                    return Err(format!("ACP {method} 失败：{reason}"));
                }
                return message
                    .get("result")
                    .cloned()
                    .ok_or_else(|| format!("ACP {method} 缺少结果"));
            }
        }
    }

    fn initialize(&mut self, cancel: &CancellationToken) -> Result<Value, String> {
        let result = self.request("initialize", json!({"protocolVersion":1,"clientCapabilities":{},
            "clientInfo":{"name":"understand-book","title":"Understand Book","version":env!("CARGO_PKG_VERSION")}}),
            Some(Duration::from_secs(60)),cancel,&mut |_| {})?;
        if result["protocolVersion"].as_u64() != Some(1) {
            return Err(format!(
                "不支持此 Agent 的 ACP 版本：{}",
                result["protocolVersion"]
            ));
        }
        Ok(result)
    }
}

impl Drop for Connection<'_> {
    fn drop(&mut self) {
        self.outgoing.take();
        // Closing stdin lets compliant agents release their own MCP servers.
        let deadline = Instant::now() + Duration::from_millis(400);
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            thread::sleep(Duration::from_millis(15));
        }
        let _ = crate::stage_processes::stop_process_tree(&mut self.child);
        let _ = self.child.wait();
    }
}

pub(super) fn probe(config: &AgentConfig) -> Result<Value, String> {
    let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
    Connection::start(config, scratch.path(), None)?.initialize(&CancellationToken::default())
}

/// A fresh ACP session receives only this turn's tool transport and prompt.
/// Reader history is supplied by the caller and outlives the child process.
pub fn run(
    agent: &AgentConfig,
    cwd: &Path,
    mcp_servers: &[Value],
    prompt: &str,
    cancel: &CancellationToken,
    on_update: impl FnMut(&Value),
) -> Result<String, String> {
    run_impl(agent, cwd, mcp_servers, prompt, cancel, on_update, None)
}

/// Connects ACP permission requests to their originating Reader turn. A hidden
/// (closed-book) turn supplies an empty turn id and receives no permission UI.
pub fn run_interactive(
    agent: &AgentConfig,
    cwd: &Path,
    mcp_servers: &[Value],
    prompt: &str,
    cancel: &CancellationToken,
    on_update: impl FnMut(&Value),
    profile: &Path,
    turn_id: &str,
    events: &RunEvents,
) -> Result<String, String> {
    let permissions = (!turn_id.is_empty()).then_some(PermissionContext {
        profile,
        turn_id,
        events,
    });
    run_impl(
        agent,
        cwd,
        mcp_servers,
        prompt,
        cancel,
        on_update,
        permissions,
    )
}

fn run_impl(
    agent: &AgentConfig,
    cwd: &Path,
    mcp_servers: &[Value],
    prompt: &str,
    cancel: &CancellationToken,
    mut on_update: impl FnMut(&Value),
    permissions: Option<PermissionContext<'_>>,
) -> Result<String, String> {
    let path = std::path::absolute(cwd).map_err(|e| e.to_string())?;
    let cwd = path.to_str().ok_or("Agent 工作路径不能转换为 UTF-8")?;
    let mut conn = Connection::start(agent, &path, permissions)?;
    let info = conn.initialize(cancel)?;
    for server in mcp_servers {
        if let Some(transport @ ("http" | "sse")) = server["type"].as_str() {
            if info["agentCapabilities"]["mcpCapabilities"][transport] != true {
                return Err(format!("此 Agent 未声明 {transport} MCP 连接能力"));
            }
        }
    }
    let mut session_params = json!({"cwd":cwd,"mcpServers":mcp_servers});
    if !mcp_servers.is_empty() {
        // Codex ACP otherwise may start its first prompt before the Reader
        // tools are registered. ACP adapters can ignore unsupported metadata.
        session_params["_meta"] = json!({"mcpStartupAwaitTimeoutMs":20_000});
    }
    let created = conn.request(
        "session/new",
        session_params,
        Some(Duration::from_secs(60)),
        cancel,
        &mut |_| {},
    )?;
    let session = created["sessionId"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("ACP 未返回会话标识")?;
    conn.session = Some(session.into());
    let mut text = String::new();
    let result = conn.request(
        "session/prompt",
        json!({"sessionId":session,"prompt":[{"type":"text","text":prompt}]}),
        None,
        cancel,
        &mut |update| {
            if update["sessionUpdate"] == "agent_message_chunk"
                && update["content"]["type"] == "text"
            {
                if let Some(chunk) = update["content"]["text"].as_str() {
                    text.push_str(chunk);
                }
            }
            on_update(update);
        },
    )?;
    // A usage_update notification carries context window occupancy. The agent
    // reports tokens consumed by this prompt on the terminal response itself.
    if let Some(usage) = result.get("usage").filter(|v| v.is_object()) {
        on_update(&json!({"sessionUpdate":"reader_prompt_usage","usage":usage}));
    }
    match result["stopReason"].as_str() {
        Some("end_turn") if !text.trim().is_empty() => Ok(text),
        Some("end_turn") => Err("Agent 完成回合，但没有交付文本回答。".into()),
        Some("cancelled") => Err("ACP 回合已取消".into()),
        Some(reason) => Err(format!("Agent 回合未完成：{reason}")),
        None => Err("ACP 未返回回合终态".into()),
    }
}
