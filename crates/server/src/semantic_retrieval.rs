//! Serial, persistent bridge to the installed local embedding model. Only authorized
//! publication snapshots are passed here; workspace search owns lexical fallback.
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{Mutex, OnceLock},
};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SemanticPublication {
    pub book_id: String,
    pub publication_id: String,
    pub directory: PathBuf,
    pub source_revision: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SemanticSearchRequest {
    pub query: String,
    pub top_k: usize,
    pub cache_dir: PathBuf,
    pub publications: Vec<SemanticPublication>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct SemanticSearchHit {
    pub book_id: String,
    pub publication_id: String,
    pub lid: String,
    pub start_utf16: u64,
    pub end_utf16: u64,
    pub score: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct SemanticSearchResult {
    pub status: String,
    pub reason: Option<String>,
    pub hits: Vec<SemanticSearchHit>,
    pub indexed_publications: usize,
    pub embedded_chunks: usize,
    pub cache_hits: usize,
    pub query_tokens: usize,
    pub document_tokens: usize,
}

impl SemanticSearchResult {
    fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            status: "unavailable".into(),
            reason: Some(reason.into()),
            hits: vec![],
            indexed_publications: 0,
            embedded_chunks: 0,
            cache_hits: 0,
            query_tokens: 0,
            document_tokens: 0,
        }
    }
}

struct Worker {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    config: OsString,
}
static WORKER: OnceLock<Mutex<Option<Worker>>> = OnceLock::new();

/// Warm the shared server model before the first reader needs semantic recall.
pub(crate) fn preload() -> Option<std::thread::JoinHandle<()>> {
    let config = std::env::var_os("UNDERSTAND_BOOK_EMBEDDING_CONFIG").filter(|v| !v.is_empty())?;
    Some(std::thread::spawn(move || {
        let result = (|| -> Result<serde_json::Value, String> {
            let mut worker = WORKER.get_or_init(|| Mutex::new(None)).lock()
                .map_err(|_| "semantic worker lock poisoned".to_string())?;
            if worker.is_none() { *worker = Some(Worker::start(config)?); }
            worker.as_mut().expect("started worker").search(&serde_json::json!({"kind":"warmup"}))
        })();
        match result {
            Ok(value) if value["status"] == "ready" => eprintln!("Semantic retrieval ready: {}", value["model"]),
            other => eprintln!("Semantic retrieval warmup failed; lexical recall remains available: {other:?}"),
        }
    }))
}

impl Worker {
    fn start(config: OsString) -> Result<Self, String> {
        let root = crate::workspace_root();
        let script = root.join("scripts").join("workspace-semantic-search.ts");
        let tsx = root
            .join("node_modules")
            .join("tsx")
            .join("dist")
            .join("cli.mjs");
        if !script.is_file() || !tsx.is_file() {
            return Err("workspace semantic retrieval runtime is not installed".into());
        }
        let mut command =
            Command::new(std::env::var_os("UNDERSTAND_BOOK_NODE").unwrap_or_else(|| "node".into()));
        command
            .arg(tsx)
            .arg(script)
            .current_dir(root)
            .env("UNDERSTAND_BOOK_EMBEDDING_CONFIG", &config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("cannot start semantic retrieval: {error}"))?;
        let input = child
            .stdin
            .take()
            .ok_or("semantic worker stdin unavailable")?;
        let output = BufReader::new(
            child
                .stdout
                .take()
                .ok_or("semantic worker stdout unavailable")?,
        );
        Ok(Self {
            child,
            input,
            output,
            config,
        })
    }

    fn search<T:Serialize,R:serde::de::DeserializeOwned>(&mut self, request: &T) -> Result<R, String> {
        serde_json::to_writer(&mut self.input, request).map_err(|error| error.to_string())?;
        self.input
            .write_all(b"\n")
            .and_then(|_| self.input.flush())
            .map_err(|error| error.to_string())?;
        let mut line = String::new();
        if self
            .output
            .read_line(&mut line)
            .map_err(|error| error.to_string())?
            == 0
        {
            return Err("semantic retrieval worker exited before returning a result".into());
        }
        serde_json::from_str(&line)
            .map_err(|error| format!("invalid semantic retrieval response: {error}"))
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The caller must release publication-library locks before entering this function.
pub(crate) fn search(request: &SemanticSearchRequest) -> Result<SemanticSearchResult, String> {
    let Some(config) =
        std::env::var_os("UNDERSTAND_BOOK_EMBEDDING_CONFIG").filter(|value| !value.is_empty())
    else {
        return Ok(SemanticSearchResult::unavailable(
            "local embedding model is not configured",
        ));
    };
    let mut worker = WORKER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| "semantic worker lock poisoned".to_string())?;
    let restart = match worker.as_mut() {
        Some(worker) => {
            worker.config != config
                || worker
                    .child
                    .try_wait()
                    .map_err(|error| error.to_string())?
                    .is_some()
        }
        None => true,
    };
    if restart {
        *worker = None;
        *worker = Some(Worker::start(config)?);
    }
    let result = worker.as_mut().expect("worker was started").search(request);
    if result.is_err() {
        *worker = None;
    }
    result
}

/// A busy material retrieval never makes a personal save or lexical search wait.
pub(crate) fn personal_notes(request:&serde_json::Value)->Result<serde_json::Value,String>{
    let Some(config)=std::env::var_os("UNDERSTAND_BOOK_EMBEDDING_CONFIG").filter(|v|!v.is_empty()) else {
        return Ok(serde_json::json!({"status":"unavailable","reason":"local embedding model is not configured","hits":[]}));
    };
    let Ok(mut worker)=WORKER.get_or_init(||Mutex::new(None)).try_lock() else {
        return Ok(serde_json::json!({"status":"partial","reason":"local model is serving another request","hits":[]}));
    };
    if worker.as_mut().is_none_or(|worker|worker.config!=config || worker.child.try_wait().ok().flatten().is_some()){
        *worker=Some(Worker::start(config)?);
    }
    let result=worker.as_mut().expect("started worker").search(request);
    if result.is_err(){*worker=None;}
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opt-in integration: exercises the actual Rust -> JSONL -> installed local model path.
    /// No model downloads, environment changes, or shared publication fixtures are needed.
    #[test]
    #[ignore = "requires an installed model via UNDERSTAND_BOOK_EMBEDDING_CONFIG"]
    fn local_model_bridge_reuses_publication_vectors() {
        assert!(
            std::env::var_os("UNDERSTAND_BOOK_EMBEDDING_CONFIG")
                .is_some_and(|value| !value.is_empty()),
            "set UNDERSTAND_BOOK_EMBEDDING_CONFIG before running this opt-in test"
        );
        let fixture = tempfile::tempdir().unwrap();
        let directory = fixture.path().join("publication");
        std::fs::create_dir(&directory).unwrap();
        let mut source = String::new();
        let mut nodes = Vec::new();
        for (index, text) in [
            "Overfitting occurs when a model memorizes training examples instead of learning general patterns. Training accuracy is high, but predictions on unseen data are poor.",
            "Photosynthesis converts sunlight into chemical energy in plants. Leaves use carbon dioxide and water to produce sugars and oxygen.",
            "A database transaction groups several operations into one atomic change. If an operation fails, all changes are rolled back.",
        ]
        .into_iter()
        .enumerate()
        {
            let start = source.encode_utf16().count();
            source.push_str(text);
            let end = source.encode_utf16().count();
            source.push_str("\n\n");
            nodes.push(serde_json::json!({
                "lid": format!("1.{}", index + 1), "path": [1, index + 1],
                "kind": "paragraph", "children": [], "span": {"start": start, "end": end}
            }));
        }
        std::fs::write(directory.join("source.txt"), source).unwrap();
        std::fs::write(
            directory.join("base.json"),
            serde_json::to_vec(&serde_json::json!({
                "book_id": "semantic-bridge-fixture", "lid_nodes": nodes
            }))
            .unwrap(),
        )
        .unwrap();
        let request = SemanticSearchRequest {
            query: "为什么模型在训练数据上表现很好，换成从没见过的数据却不行了？".into(),
            top_k: 3,
            cache_dir: fixture.path().join("cache"),
            publications: vec![SemanticPublication {
                book_id: "semantic-bridge-fixture".into(),
                publication_id: "fixture-v1".into(),
                directory,
                source_revision: "fixture-source-v1".into(),
            }],
        };
        let first = search(&request).unwrap();
        assert_eq!(first.status, "ready", "{:?}", first.reason);
        assert_eq!(first.hits[0].lid, "1.1");
        assert_eq!(first.indexed_publications, 1);
        assert_eq!(first.embedded_chunks, 3);
        assert!(first.query_tokens > 0 && first.document_tokens > 0);
        let second = search(&request).unwrap();
        assert_eq!(second.status, "ready", "{:?}", second.reason);
        assert_eq!(second.hits[0].lid, "1.1");
        assert_eq!(second.indexed_publications, 0);
        assert_eq!(second.embedded_chunks, 0);
        assert_eq!(second.cache_hits, 3);
        assert_eq!(second.document_tokens, 0);
        assert_eq!(first.hits[0].start_utf16, second.hits[0].start_utf16);
        assert_eq!(first.hits[0].end_utf16, second.hits[0].end_utf16);
        assert_eq!(first.hits[0].score, second.hits[0].score);
    }
}
