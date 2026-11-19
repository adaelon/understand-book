use super::client::PersonalClient;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

fn error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

fn reply(
    message: Value,
    call: &mut impl FnMut(&str, Value) -> Result<Value, String>,
) -> Option<Value> {
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    if message["jsonrpc"] != "2.0" || !message["method"].is_string() {
        return Some(error(id, -32600, "Invalid JSON-RPC request"));
    }
    if message.get("id").is_none() {
        return None;
    }
    let params = &message["params"];
    let result = match message["method"].as_str()? {
        "initialize" => {
            let supported = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
            let version = params["protocolVersion"]
                .as_str()
                .filter(|v| supported.contains(v))
                .unwrap_or(supported[0]);
            json!({"protocolVersion":version,"serverInfo":{"name":"understand-book-personal","version":env!("CARGO_PKG_VERSION")},
                "capabilities":{"tools":{}},"instructions":"Work with the books and actions granted by this Reader. Copy actual source_ref values from tool results. Save learning questions explicitly to resume them in Reader."})
        }
        "ping" => json!({}),
        "tools/list" => match call("tools_list", json!({})) {
            Ok(list) => list,
            Err(message) => return Some(error(id, -32000, &message)),
        },
        "tools/call" => {
            let Some(name) = params["name"].as_str() else {
                return Some(error(id, -32602, "Missing tool name"));
            };
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            if !args.is_object() {
                return Some(error(id, -32602, "Tool arguments must be an object"));
            }
            match call(name, args) {
                Ok(value) => {
                    let structured = if value.is_object() {
                        value
                    } else {
                        json!({"result":value})
                    };
                    json!({"isError":false,"content":[{"type":"text","text":structured.to_string()}],"structuredContent":structured})
                }
                Err(message) => json!({"isError":true,"content":[{"type":"text","text":message}]}),
            }
        }
        _ => return Some(error(id, -32601, "Unknown MCP method")),
    };
    Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
}

pub fn serve(reader: impl BufRead, mut writer: impl Write) -> Result<(), String> {
    let mut client = None;
    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str(&line) {
            Ok(message) => reply(message, &mut |name, args| {
                if client.is_none() {
                    client = Some(PersonalClient::load()?);
                }
                client.as_ref().unwrap().call(name, args)
            }),
            Err(e) => Some(error(Value::Null, -32700, &e.to_string())),
        };
        if let Some(response) = response {
            writeln!(writer, "{response}")
                .and_then(|_| writer.flush())
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notifications_are_silent_and_a_denied_tool_is_not_a_success() {
        let mut deny = |_: &str, _: Value| Err("Grant revoked".to_string());
        assert!(reply(
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            &mut deny
        )
        .is_none());
        let result = reply(
            json!({"jsonrpc":"2.0","method":"tools/call","id":3,"params":{"name":"note_add"}}),
            &mut deny,
        )
        .unwrap();
        assert_eq!(result["id"], 3);
        assert_eq!(result["result"]["isError"], true);
        assert!(result["result"].get("structuredContent").is_none());
    }
}
