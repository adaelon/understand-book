use read_tools::{Book, EvidenceRange};
use reader_service::acp_evidence::AcpEvidence;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::Arc,
};

struct Bridge {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}

impl Bridge {
    fn start(turn: &AcpEvidence) -> Self {
        let description = turn.mcp_server().unwrap();
        let args = description["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|arg| arg.as_str().unwrap());
        // Release checks can exercise the installed GUI-subsystem executable with actual pipes.
        let executable = std::env::var_os("UNDERSTAND_BOOK_TEST_ACP_EXECUTABLE")
            .unwrap_or_else(|| std::ffi::OsString::from(env!("CARGO_BIN_EXE_reader")));
        let mut child = Command::new(executable)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        Self {
            input: child.stdin.take(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let input = self.input.as_mut().unwrap();
        writeln!(
            input,
            "{}",
            json!({"jsonrpc":"2.0","id":1,"method":method,"params":params})
        )
        .unwrap();
        input.flush().unwrap();
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        serde_json::from_str::<Value>(&line).unwrap()["result"].clone()
    }

    fn call(&mut self, name: &str, args: Value) -> Value {
        self.request("tools/call", json!({"name":name, "arguments":args}))
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        drop(self.input.take());
        self.child.wait().unwrap();
    }
}

fn book() -> Arc<Book> {
    let text = "Candidates locate passages. Read the supporting text.";
    let mut base = base_schema::sample_base();
    for node in &mut base.lid_nodes {
        node.span.end = text.len();
    }
    Arc::new(Book::new(base, text))
}

#[test]
fn acp_evidence_stdio_reads_and_binds_only_observed_quotes() {
    let evidence = AcpEvidence::start(book(), vec![], "What supports an answer?").unwrap();
    let mut bridge = Bridge::start(&evidence);
    assert_eq!(
        bridge.request("initialize", json!({"protocolVersion":"2025-11-25"}))["protocolVersion"],
        "2025-11-25"
    );
    let tools = bridge.request("tools/list", json!({}));
    assert!(tools["tools"].as_array().unwrap().iter().any(
        |tool| tool["name"] == "source_present" && tool["annotations"]["readOnlyHint"] == true
    ));
    assert!(tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .all(|tool| tool["annotations"]["readOnlyHint"] == true));
    let quote = json!({"quote":"Read the supporting text."});
    assert_eq!(
        bridge.call("source_present", quote.clone())["isError"],
        true
    );
    let ranked = bridge.call("book_search", json!({"query":"supporting text"}));
    assert_eq!(ranked["isError"], false);
    assert!(ranked["structuredContent"]["hits"][0]["start_lid"].is_string());
    assert_eq!(bridge.call("source_present", quote.clone())["isError"], true);
    let search = bridge.call("book_search_text", json!({"query":"Candidates"}));
    let lid = &search["structuredContent"]["occurrences"][0]["start_lid"];
    assert!(lid.is_string());
    assert_eq!(
        bridge.call("source_present", quote.clone())["isError"],
        true
    );
    assert_eq!(
        bridge.call("book_text", json!({"lid":lid}))["isError"],
        false
    );
    let source = bridge.call("source_present", quote);
    let marker = source["structuredContent"]["marker"]
        .as_str()
        .unwrap()
        .to_owned();
    drop(bridge);
    let outcome = evidence
        .finish(&format!("Read the original.{marker}"))
        .unwrap();
    assert_eq!(outcome.answer.as_deref(), Some("Read the original."));
    assert_eq!(outcome.answer_view.unwrap().sources.len(), 1);
    assert_eq!(outcome.source_bindings[0].book_id, "sample-book");
    assert_eq!(
        outcome.source_bindings[0]
            .evidence_range
            .as_ref()
            .unwrap()
            .start_lid,
        lid.as_str().unwrap()
    );
    let another_turn = AcpEvidence::start(book(), vec![], "Continue?").unwrap();
    assert!(another_turn
        .finish(&format!("Unsupported.{marker}"))
        .is_err());
}

#[test]
fn acp_evidence_verified_selection_can_be_cited_without_another_read() {
    let seed = EvidenceRange {
        start_lid: "1.1".into(),
        end_lid: "1.1".into(),
        ranges: Vec::new(),
    };
    let evidence = AcpEvidence::start(book(), vec![seed], "Why?").unwrap();
    let mut bridge = Bridge::start(&evidence);
    let source = bridge.call(
        "source_present",
        json!({"quote":"Read the supporting text."}),
    );
    assert_eq!(source["isError"], false);
    drop(bridge);
    let outcome = evidence
        .finish("A private teaching decision without displayed citations.")
        .unwrap();
    assert!(outcome.source_bindings.is_empty());
    assert!(outcome.answer_view.unwrap().sources.is_empty());
}
