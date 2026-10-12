//! Stateless Streamable HTTP: prepare content, commit one debit, then deliver JSON.
use crate::{
    authorization::Authorization,
    mcp_access::McpIdentity,
    mcp_read::{self, McpRead},
    mcp_usage::Usage,
    multi_user_host::{Headers, HttpReply},
    user_storage_paths::error,
};
use read_tools::ToolError;
use serde_json::{json, Value};

const PROTOCOL: &str = "2025-11-25";

const INSTRUCTIONS: &str = r#"Understand Book provides read-only access to this account's authorized, already-built publications. You perform the reasoning and answer the user from retrieved evidence.
1. Select material: call book_list (free), choose an item by name and copy its published_book_ref (book_id + publication_id). Check available_tools for that publication. Never invent a publication or LID, or substitute another publication version mid-read.
2. Locate: use book_search_text for literal occurrences or book_concept for concept/entity candidates. If available, book_structure and book_guide_path provide structure and reading stops; paper tools provide built metadata, terminology and reading aids. These are pointers/projections, not a substitute for reading the supporting source. book_context expands around a known LID.
3. Read evidence: call book_text on returned LIDs. Each page contains at most 8,000 Unicode characters. source_range is an absolute canonical-source UTF-16 half-open range [start, end), not byte or character indices. Follow only needed next_calls by copying their name and arguments unchanged. complete=false means more pages exist; next_calls may contain multiple independent collection pages, not a single linear cursor. Collection pages use data_path and total_items to locate their data in the original projection. Search also returns occurrence cursors; use next_calls rather than guessing them. Concept candidate limits/truncated still bound recall even when the current projection is complete.
4. Cite: use sources[].published_book_ref, start_lid/end_lid, source_range and source_url from the evidence you actually read. Keep each citation tied to that publication and exact range. Search hits have exact occurrence positions in data; other pointer sources may cover a whole LID. If sources is empty, there is no source citation for that projection: locate and read supporting text before making an attributed claim. The source_url opens that publication and exact range in Reader; sign in first if needed. Unavailable material requires the user to contact the administrator.
Costs: initialize, tools/list, book_list and mcp_usage cost 0 calls. Each successful content call costs 1, including an empty search, each continuation page and a repeated successful request. JSON-RPC id does not deduplicate charges. Failures cost 0. A response lost after commit may already be charged; a successful retry spends another call. Read only pages needed for the task.
Use server-returned usage, never calculate your own balance. free_remaining and paid_remaining are shared by all clients of this account; successful content returns the balance after this call with charged_units and charged_from. Monthly free credits (1,000) reset at free_resets_at, on the first day at 00:00 Asia/Hong_Kong; unused free credits do not carry over. Free credits are used first, then valid packs with the earliest expiry. call_packs gives each batch's expiry and available_units; paid_next_expires_at concerns the next still-usable batch. Packs cost CNY 10 per 1,000 calls, are valid for 12 calendar months after activation, and carry across months until expiry.
Recovery: inspect isError and structuredContent.error_code, message, retryable and next_step (also mirrored as JSON text in content). quota_exhausted: stop content retries, tell the user free_resets_at and top_up_url or, when null, top_up_instruction. Discovery, book_list and mcp_usage remain available with the same tools. unauthenticated (HTTP 401): ask the user to check or recreate the credential. material_unavailable: refresh book_list and select an authorized publication with the needed capability, or ask the user to contact the administrator. invalid_arguments: correct parameters using inputSchema and returned next_calls. service_busy: wait retry_after_seconds; service_unavailable: retry later as directed, without a tight retry loop. Purchases are performed by the user through the administrator; buying calls does not grant material access."#;

fn http(status: u16, value: Value) -> HttpReply {
    let mut reply = HttpReply::json(value);
    reply.status = status;
    if status == 202 {
        reply.body.clear();
    }
    reply
}
fn rpc_error(status: u16, id: Value, code: i64, message: &str) -> HttpReply {
    http(
        status,
        json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}}),
    )
}
fn success(id: Value, result: Value) -> HttpReply {
    HttpReply::json(json!({"jsonrpc":"2.0","id":id,"result":result}))
}
fn valid_id(id: &Value) -> bool {
    id.is_string() || id.is_i64() || id.is_u64()
}
fn accepts(headers: &Headers, expected: &str) -> bool {
    headers.get("Accept").is_some_and(|value| {
        value.split(',').any(|part| {
            let mut fields = part.split(';');
            fields
                .next()
                .is_some_and(|s| s.trim().eq_ignore_ascii_case(expected))
                && fields.all(|field| {
                    let Some((key, value)) = field.trim().split_once('=') else {
                        return true;
                    };
                    key.trim() != "q" || value.trim().parse::<f32>().is_ok_and(|q| q > 0.0)
                })
        })
    })
}

pub(crate) fn dispatch(
    access: &Authorization,
    origin: &str,
    method: &str,
    headers: &Headers,
    body: &str,
) -> HttpReply {
    let identity = match access.mcp_access.authenticate(headers.get("Authorization")) {
        Ok(identity) => identity,
        Err(err) => {
            return http(
                if err.category == "authentication" {
                    401
                } else {
                    503
                },
                failure(&err),
            )
        }
    };
    if headers
        .get("MCP-Protocol-Version")
        .is_some_and(|v| v != PROTOCOL)
    {
        return http(
            400,
            json!({"error_code":"unsupported_protocol_version","message":"Use MCP-Protocol-Version: 2025-11-25"}),
        );
    }
    if method != "POST" {
        return http(
            405,
            json!({"error_code":"method_not_allowed","message":"Use POST; server push and sessions are not provided"}),
        );
    }
    if !headers.get("Content-Type").is_some_and(|v| {
        v.split(';')
            .next()
            .unwrap_or("")
            .trim()
            .eq_ignore_ascii_case("application/json")
    }) {
        return http(
            415,
            json!({"error_code":"unsupported_media_type","message":"Use application/json"}),
        );
    }
    if !accepts(headers, "application/json") || !accepts(headers, "text/event-stream") {
        return http(
            406,
            json!({"error_code":"not_acceptable","message":"Accept must include application/json and text/event-stream"}),
        );
    }
    let msg: Value = match serde_json::from_str(body) {
        Ok(msg) => msg,
        Err(_) => return rpc_error(400, Value::Null, -32700, "Parse error"),
    };
    let id = msg.get("id").cloned();
    let response_id = id.clone().filter(valid_id).unwrap_or(Value::Null);
    if !msg.is_object() || msg["jsonrpc"] != "2.0" || id.as_ref().is_some_and(|v| !valid_id(v)) {
        return rpc_error(400, response_id, -32600, "Invalid Request");
    }
    let method = msg.get("method").and_then(Value::as_str);
    // Without session state the negotiated version must accompany every later request.
    // Missing headers default to 2025-03-26 in the transport spec, which we do not support.
    if method != Some("initialize") && headers.get("MCP-Protocol-Version").is_none() {
        return rpc_error(
            400,
            response_id,
            -32600,
            "Use MCP-Protocol-Version: 2025-11-25 after initialization",
        );
    }
    let Some(method) = method else {
        // No server-initiated requests are issued, but transport-level client responses
        // are accepted without creating another JSON-RPC response.
        let result = msg.get("result");
        let err = msg.get("error");
        if id.is_some()
            && (result.is_some() ^ err.is_some())
            && err.is_none_or(|e| e["code"].is_i64() && e["message"].is_string())
        {
            return http(202, Value::Null);
        }
        return rpc_error(400, response_id, -32600, "Invalid Request");
    };
    if msg.get("params").is_some_and(|v| !v.is_object())
        || msg.get("result").is_some()
        || msg.get("error").is_some()
    {
        return rpc_error(400, response_id, -32600, "Invalid Request");
    }
    if id.is_none() {
        // Notifications never invoke tools or spend credits, even with tools/call as method.
        return http(202, Value::Null);
    }
    let params = &msg["params"];
    match method {
        "initialize" => {
            if !params["protocolVersion"].is_string()
                || !params["capabilities"].is_object()
                || !params["clientInfo"]["name"].is_string()
                || !params["clientInfo"]["version"].is_string()
            {
                return rpc_error(
                    400,
                    response_id,
                    -32602,
                    "Provide protocolVersion, capabilities and clientInfo",
                );
            }
            success(
                response_id,
                json!({"protocolVersion":PROTOCOL,"capabilities":{"tools":{}},
                "serverInfo":{"name":"understand-book-remote","version":env!("CARGO_PKG_VERSION")},
                "instructions":INSTRUCTIONS}),
            )
        }
        "ping" => success(response_id, json!({})),
        "tools/list" => {
            if params.get("cursor").is_some_and(|v| !v.is_null()) {
                return rpc_error(
                    400,
                    response_id,
                    -32602,
                    "The complete tool list has no cursor",
                );
            }
            let mut tools = mcp_read::tool_definitions();
            tools.push(json!({"name":"mcp_usage","description":"Check this account's server-calculated call balance before planning more reads or after quota_exhausted. Costs 0 calls, even when exhausted. Takes no arguments. Returns usage: free_remaining, paid_remaining, free_resets_at (next Hong Kong month), paid_next_expires_at and call_packs with per-batch expiry and available_units. Free credits are spent first, then valid packs by earliest expiry; all clients share this account's balance. charged_units is 0 and charged_from is null for this query. If both balances are zero, stop content retries and tell the user the reset time and top_up_url, or top_up_instruction when the URL is null. The user arranges purchase; a purchase does not grant material access. No evidence sources or continuation pages are returned.",
                "inputSchema":{"type":"object","properties":{},"additionalProperties":false}}));
            success(response_id, json!({"tools":tools}))
        }
        "tools/call" => {
            let Some(name) = params["name"].as_str() else {
                return rpc_error(400, response_id, -32602, "tools/call requires a tool name");
            };
            if name != "book_list" && name != "mcp_usage" && !mcp_read::is_content_tool(name) {
                return rpc_error(200, response_id, -32602, "Unknown tool; use tools/list");
            }
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match call(access, origin, &identity, name, arguments) {
                Ok(result) => success(response_id, result),
                Err(err) => {
                    let mut value = failure(&err);
                    if err.error_code == "MCP_QUOTA_EXHAUSTED" {
                        if let Ok(usage) = access.mcp_usage.status(identity.user_id()) {
                            value["usage"] = usage_value(usage, 0, Value::Null, origin);
                        }
                    }
                    success(response_id, tool_result(value, true))
                }
            }
        }
        _ => rpc_error(200, response_id, -32601, "Method not found"),
    }
}

pub(crate) fn usage_value(usage: Usage, charged_units: i64, charged_from: Value, origin: &str) -> Value {
    let mut value = json!(usage);
    value["charged_units"] = json!(charged_units);
    value["charged_from"] = charged_from;
    value["top_up_url"] = json!(format!("{}/?account=mcp", origin.trim_end_matches('/')));
    value["top_up_instruction"] = json!("Ask the user to contact the beta administrator: CNY 10 buys 1,000 calls valid for 12 calendar months after activation. Free credits are used first; purchased credits carry across months until expiry. Buying calls does not grant material access.");
    value
}
fn tool_result(value: Value, is_error: bool) -> Value {
    json!({"content":[{"type":"text","text":value.to_string()}],"structuredContent":value,"isError":is_error})
}
fn call(
    access: &Authorization,
    origin: &str,
    identity: &McpIdentity,
    name: &str,
    arguments: Value,
) -> Result<Value, ToolError> {
    if name == "mcp_usage" {
        if !arguments.as_object().is_some_and(|o| o.is_empty()) {
            return Err(error(
                "MCP_READ_INPUT_INVALID",
                "validation",
                "mcp_usage takes no arguments",
            ));
        }
        return Ok(tool_result(
            json!({"usage":usage_value(access.mcp_usage.status(identity.user_id())?, 0, Value::Null, origin)}),
            false,
        ));
    }
    let reader = McpRead::new(&access.library, origin)?;
    if name == "book_list" {
        let mut value = reader.call(identity, name, arguments)?;
        value["usage"] = usage_value(access.mcp_usage.status(identity.user_id())?, 0, Value::Null, origin);
        return Ok(tool_result(value, false));
    }
    let prepared = reader.prepare(identity, name, arguments)?;
    let usage = access.mcp_usage.status(identity.user_id())?;
    if usage.free_remaining == 0 && usage.paid_remaining == 0 {
        return Err(error(
            "MCP_QUOTA_EXHAUSTED",
            "quota_exhausted",
            "Stop retrying content calls",
        ));
    }
    let reference = prepared.reference.clone();
    let value = reader.read_prepared(identity, prepared)?;
    // Encode the entire content projection before touching the ledger. Afterwards only
    // JSON Values and the typed debit receipt are added; no content conversion or IO remains.
    let mut result = tool_result(value, false);
    let debit = access
        .mcp_usage
        .consume_success(identity.user_id(), name, &reference)?;
    result["structuredContent"]["usage"] =
        usage_value(debit.usage, debit.charged_units, json!(debit.charged_from), origin);
    result["content"][0]["text"] = json!(result["structuredContent"].to_string());
    Ok(result)
}

fn failure(err: &ToolError) -> Value {
    let (code, message, retryable, next) = match err.error_code.as_str() {
        "MCP_AUTH_REQUIRED" | "USER_UNAVAILABLE" => ("unauthenticated", "An active MCP credential is required", false, "Ask the user to check or recreate the MCP credential"),
        "MCP_QUOTA_EXHAUSTED" => ("quota_exhausted", "No call credits remain", false, "Stop content requests; wait for free credit reset or ask the user to contact the administrator for a call pack"),
        "MCP_MATERIAL_UNAVAILABLE" | "PUBLICATION_UNAVAILABLE" | "MCP_CAPABILITY_UNAVAILABLE" => ("material_unavailable", "This publication or capability is unavailable", false, "Query book_list and select an authorized publication with the required capability"),
        _ if err.category == "validation" || matches!(err.error_code.as_str(), "LID_NOT_FOUND" | "CONCEPT_NOT_FOUND") => ("invalid_arguments", err.message.as_str(), false, "Correct arguments using tools/list and returned continuation calls"),
        _ if err.category == "rate_limit" => ("service_busy", "Read capacity is busy", true, "Wait 60 seconds before retrying"),
        _ => ("service_unavailable", "The read or its usage record could not be completed", true, "Retry later; this failed call was not charged"),
    };
    let mut value = json!({"error_code":code,"cause_code":err.error_code,"message":message,"retryable":retryable,"next_step":next,"charged_units":0});
    if code == "service_busy" {
        value["retry_after_seconds"] = json!(60);
    }
    value
}

#[cfg(test)]
#[path = "remote_mcp_tests.rs"]
mod tests;
