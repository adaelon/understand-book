//! Local learning commands and stdio MCP for an explicitly granted personal connection.
#[path = "personal_cli/client.rs"]
mod client;
#[path = "personal_cli/mcp.rs"]
mod mcp;
use client::PersonalClient;
use serde_json::{json, Value};
use std::{
    io::{self, Read},
    path::PathBuf,
};

const HELP: &str = r#"Understand Book · personal learning tools

  understand-book connect --token-stdin
  understand-book disconnect
  understand-book serve [book-directory]
  understand-book mcp
  understand-book tools
  understand-book material list
  understand-book book search <book-id> <text>
  understand-book book text <book-id> <lid>
  understand-book paper import <PDF-path>
  understand-book paper metadata <book-id>
  understand-book note add --at <source-ref JSON> --text <note> [--operation-id <id>]
  understand-book highlight list <book-id> [--before <id>]
  understand-book highlight add --at <source-ref JSON> [--operation-id <id>]
  understand-book task list [book-id]
  understand-book call <tool> <arguments JSON>

JSON arguments accept @file or - for stdin. Output is JSON; --json is also accepted.
Enable the personal connection in the local Reader. Set UNDERSTAND_BOOK_PERSONAL_TOKEN
or pair using connect --token-stdin. Use book_mcp <book-directory> for visitor access."#;

fn json_input(arg: &str) -> Result<Value, String> {
    let input = match arg.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?,
        None if arg == "-" => {
            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .map_err(|e| e.to_string())?;
            input
        }
        None => arg.to_owned(),
    };
    serde_json::from_str(input.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("Invalid JSON: {e}"))
}

fn command(args: &[String]) -> Result<(String, Value), String> {
    let args: Vec<_> = args.iter().map(String::as_str).collect();
    let action = match args.as_slice() {
        ["tools"] => ("tools_list", json!({})),
        ["material", "list"] => ("material_list", json!({})),
        ["book", "search", id, text] => ("book_search_text", json!({"book_id":id, "query":text})),
        ["book", "text", id, lid] => ("book_text", json!({"book_id":id, "lid":lid})),
        ["highlight", "list", id] => ("highlight_list", json!({"book_id":id})),
        ["highlight", "list", id, "--before", cursor] => {
            ("highlight_list", json!({"book_id":id, "before":cursor}))
        }
        ["paper", "metadata", id] => ("paper_metadata", json!({"book_id":id})),
        ["paper", "import", input] => {
            let path = std::path::absolute(input).map_err(|e| e.to_string())?;
            ("paper_import", json!({"path":path}))
        }
        ["task", "list"] => ("task_list", json!({})),
        ["task", "list", id] => ("task_list", json!({"book_id":id})),
        ["call", tool, args] => {
            let value = json_input(args)?;
            if !value.is_object() {
                return Err("Tool arguments must be a JSON object".into());
            }
            (*tool, value)
        }
        [kind @ ("note" | "highlight"), "add", options @ ..] => {
            let mut args = serde_json::Map::new();
            let mut options = options.iter();
            while let Some(flag) = options.next() {
                let value = options
                    .next()
                    .ok_or_else(|| format!("Missing value for {flag}"))?;
                let (key, value) = match *flag {
                    "--at" => {
                        let source = json_input(value)?;
                        let at = source.get("source_ref").cloned().unwrap_or(source);
                        ("source_ref", at)
                    }
                    "--text" if *kind == "note" => ("text", json!(value)),
                    "--operation-id" => ("operation_id", json!(value)),
                    _ => return Err(format!("Unknown option {flag}")),
                };
                if args.insert(key.into(), value).is_some() {
                    return Err(format!("Duplicate option {flag}"));
                }
            }
            if !args.contains_key("source_ref") || (*kind == "note" && !args.contains_key("text")) {
                return Err("A source is required (--at), and notes require --text".into());
            }
            args.entry("operation_id")
                .or_insert_with(|| json!(uuid::Uuid::now_v7().to_string()));
            (
                if *kind == "note" {
                    "note_add"
                } else {
                    "highlight_add"
                },
                Value::Object(args),
            )
        }
        _ => return Err(format!("Unrecognized command.\n{HELP}")),
    };
    Ok((action.0.to_owned(), action.1))
}

fn run() -> Result<(), String> {
    if reader_service::acp_evidence::serve_stdio_if_requested() {
        return Ok(());
    }
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.last().map(String::as_str) == Some("--json") {
        args.pop();
    }
    match args.first().map(String::as_str) {
        Some("--help" | "-h" | "help") | None => {
            println!("{HELP}");
            return Ok(());
        }
        Some("--version") => {
            println!("understand-book {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("serve") => {
            return reader_service::cli::serve_profile(
                args[1..].iter().map(PathBuf::from).collect(),
            )
        }
        Some("mcp") if args.len() == 1 => {
            return mcp::serve(io::stdin().lock(), io::stdout().lock())
        }
        Some("connect")
            if args.get(1).map(String::as_str) == Some("--token-stdin") && args.len() == 2 =>
        {
            let mut token = String::new();
            io::stdin()
                .read_to_string(&mut token)
                .map_err(|e| e.to_string())?;
            PersonalClient::pair(token.trim())?;
            println!("{}", json!({"connected":true}));
            return Ok(());
        }
        Some("disconnect") if args.len() == 1 => {
            PersonalClient::forget()?;
            println!("{}", json!({"saved_token_removed":true}));
            return Ok(());
        }
        _ => {}
    }
    let (name, arguments) = command(&args)?;
    let result = PersonalClient::load()?.call(&name, arguments)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}
