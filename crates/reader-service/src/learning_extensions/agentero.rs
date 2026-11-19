use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

pub(super) struct Agentero {
    url: String,
    agent: ureq::Agent,
    session: Option<String>,
    version: String,
    next_id: u64,
}

impl Agentero {
    pub(super) fn connect() -> Result<Self, String> {
        let url = std::env::var("UNDERSTAND_BOOK_AGENTERO_MCP")
            .unwrap_or_else(|_| "http://127.0.0.1:8765/mcp".into());
        Self::open(url)
    }

    pub(super) fn open(url: String) -> Result<Self, String> {
        let mut client = Self {
            url,
            agent: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(45))
                .build(),
            session: None,
            version: "2025-11-25".into(),
            next_id: 0,
        };
        let init = client.rpc(
            "initialize",
            json!({"protocolVersion":client.version,"capabilities":{},
            "clientInfo":{"name":"understand-book","version":env!("CARGO_PKG_VERSION")}}),
            false,
        )?;
        client.version = init["protocolVersion"]
            .as_str()
            .ok_or("Agentero did not negotiate an MCP version")?
            .to_owned();
        client.rpc("notifications/initialized", json!({}), true)?;
        Ok(client)
    }

    fn rpc(&mut self, method: &str, params: Value, notify: bool) -> Result<Value, String> {
        self.next_id += 1;
        let mut request = self
            .agent
            .post(&self.url)
            .set("Accept", "application/json, text/event-stream");
        if method != "initialize" {
            request = request.set("MCP-Protocol-Version", &self.version);
        }
        if let Some(s) = &self.session {
            request = request.set("Mcp-Session-Id", s);
        }
        let mut body = json!({"jsonrpc":"2.0","method":method,"params":params});
        if !notify {
            body["id"] = self.next_id.into();
        }
        let response = request
            .send_json(body)
            .map_err(|e| format!("Agentero MCP ({}) {e}", self.url))?;
        if let Some(s) = response.header("Mcp-Session-Id") {
            self.session = Some(s.to_string());
        }
        if notify {
            return Ok(Value::Null);
        }
        let msg: Value = if response
            .header("Content-Type")
            .unwrap_or("")
            .starts_with("text/event-stream")
        {
            let mut data = String::new();
            let mut result = None;
            for line in BufReader::new(response.into_reader()).lines() {
                let line = line.map_err(|e| e.to_string())?;
                if let Some(value) = line.strip_prefix("data:") {
                    data.push_str(value.trim_start());
                    data.push('\n');
                } else if line.is_empty() && !data.is_empty() {
                    if !data.trim().is_empty() {
                        let obj: Value = serde_json::from_str(&data)
                            .map_err(|e| format!("Invalid Agentero event: {e}"))?;
                        if obj["id"].as_u64() == Some(self.next_id) {
                            result = Some(obj);
                            break;
                        }
                    }
                    data.clear();
                }
            }
            result.ok_or("Agentero ended the stream without a response")?
        } else {
            response.into_json().map_err(|e| e.to_string())?
        };
        if let Some(err) = msg.get("error") {
            return Err(format!("Agentero: {err}"));
        }
        msg.get("result")
            .cloned()
            .ok_or_else(|| "Missing Agentero result".into())
    }

    pub(super) fn tool(&mut self, name: &str, args: Value) -> Result<Value, String> {
        let result = self.rpc("tools/call", json!({"name":name,"arguments":args}), false)?;
        if result["isError"] == true {
            return Err(format!(
                "Agentero {name}: {}",
                result["content"][0]["text"]
                    .as_str()
                    .unwrap_or("tool failed")
            ));
        }
        if let Some(body) = result.get("structuredContent") {
            return Ok(body.clone());
        }
        serde_json::from_str(
            result["content"][0]["text"]
                .as_str()
                .ok_or("Missing Agentero tool content")?,
        )
        .map_err(|e| e.to_string())
    }

    pub(super) fn select(
        &mut self,
        paper: Option<&str>,
        identifier: Option<&str>,
    ) -> Result<(Value, Option<PathBuf>), String> {
        let vaults = self.tool("vault_list", json!({}))?;
        let vault = vaults["open"]
            .as_str()
            .ok_or("Open a local Vault in Agentero")?;
        let reference = match (paper, identifier) {
            (Some(reference), None) => reference.to_owned(),
            (None, Some(identifier)) => {
                let imported = self.tool("import_id", json!({"text":identifier,"vault":vault}))?;
                imported["path"]
                    .as_str()
                    .ok_or("Agentero returned no imported paper path")?
                    .to_owned()
            }
            _ => return Err("Provide one paper reference or one DOI/arXiv identifier".into()),
        };
        let args = json!({"ref":reference,"vault":vault});
        let meta = self.tool("paper_get", args.clone())?;
        let paths = self.tool("paper_paths", args)?;
        let vault_dir = Path::new(vault)
            .canonicalize()
            .map_err(|e| format!("Local Agentero Vault: {e}"))?;
        let mut pdf = None;
        for raw in paths["paths"]
            .as_array()
            .ok_or("Missing Agentero paper paths")?
            .iter()
            .filter_map(Value::as_str)
        {
            let path = Path::new(raw);
            if path
                .extension()
                .and_then(|v| v.to_str())
                .is_some_and(|v| v.eq_ignore_ascii_case("pdf"))
                && !path.components().any(|c| matches!(c, Component::ParentDir))
            {
                let candidate = vault_dir.join(path);
                if candidate.is_file() {
                    let path = candidate.canonicalize().map_err(|e| e.to_string())?;
                    if path.starts_with(&vault_dir) {
                        pdf = Some(path);
                        break;
                    }
                }
            }
        }
        Ok((
            json!({"provider":"agentero","vault":vault,"ref":reference,"metadata":meta}),
            pdf,
        ))
    }
}

impl Drop for Agentero {
    fn drop(&mut self) {
        if let Some(id) = self.session.take() {
            let _ = self
                .agent
                .delete(&self.url)
                .set("Mcp-Session-Id", &id)
                .set("MCP-Protocol-Version", &self.version)
                .timeout(Duration::from_secs(2))
                .call();
        }
    }
}
