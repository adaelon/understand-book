//! User-selected paper sources and teaching skills in the local Reader.
mod agentero;
mod skills;
#[cfg(test)]
mod tests;

use crate::{AppState, Book, ToolError};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn scope(workspace: &crate::reader_workspace::ReaderWorkspace) -> String {
    match &workspace.learning_workspace_id {
        Some(id) => format!("workspace:{id}"),
        None => format!("book:{}", workspace.book.base.book_id),
    }
}

pub fn skill_context(root: &Path, scope: &str) -> Result<String, String> {
    skills::context(&skills::workspace(root, scope))
}

pub fn skill_workspace(root: &Path, scope: &str) -> PathBuf {
    skills::workspace(root, scope)
}

fn is_skill_id(name: &str) -> bool {
    crate::is_valid_book_id(name) && name.len() <= 64 && !name.contains("--")
}

fn error(code: &str, category: &str, message: impl Into<String>) -> ToolError {
    ToolError {
        error_code: code.into(),
        category: category.into(),
        message: message.into(),
    }
}

fn invalid(message: impl Into<String>) -> ToolError {
    error("LEARNING_EXTENSION_INPUT", "validation", message)
}
fn io(e: std::io::Error) -> ToolError {
    error("LEARNING_EXTENSION_IO", "internal", e.to_string())
}
fn remote(message: String) -> ToolError {
    error("AGENTERO_MCP_ERROR", "unavailable", message)
}

fn string<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}
fn required<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    string(args, key).ok_or_else(|| invalid(format!("Missing {key}")))
}

pub fn tools() -> Vec<Value> {
    let s = json!({"type":"string","minLength":1});
    let mut result = Vec::new();
    let mut tool = |name: &str, desc: &str, properties: Value, required: &[&str]| {
        result.push(json!({"name":name,"description":desc,"inputSchema":{"type":"object","properties":properties,
            "required":required,"additionalProperties":false}}));
    };
    tool("paper_import","Import a local PDF through Reader Workbench. To attach a PDF to a metadata-only record, pass its book_id.",json!({"path":s,"book_id":s}),&["path"]);
    tool(
        "paper_metadata",
        "Read Reader paper metadata with evidence and any original catalog metadata.",
        json!({"book_id":s}),
        &[],
    );
    tool(
        "agentero_papers",
        "Find papers in the currently open Agentero Vault.",
        json!({"query":s}),
        &[],
    );
    tool("agentero_import","Import an Agentero paper by its catalog reference or resolve one DOI/arXiv identifier. Without a PDF a metadata record is saved for later attachment.",json!({"paper":s,"identifier":s,"local_pdf":s,"book_id":s}),&[]);
    tool(
        "skill_list",
        "List teaching Skills installed for the current Reader scope.",
        json!({}),
        &[],
    );
    tool(
        "skill_import",
        "Install a Skill directory from disk or GitHub. Enable it explicitly for future turns.",
        json!({"source":s,"subdir":s}),
        &["source"],
    );
    tool(
        "skill_enable",
        "Enable or disable an installed Skill for future turns in this scope.",
        json!({"name":s,"enabled":{"type":"boolean"}}),
        &["name", "enabled"],
    );
    tool(
        "skill_remove",
        "Remove this scope's installed copy of a Skill.",
        json!({"name":s}),
        &["name"],
    );
    result
}

pub fn dispatch(
    state: &mut AppState,
    root: &Path,
    name: &str,
    args: &Value,
    now: &str,
) -> Result<Value, ToolError> {
    state.user.check_owner(crate::user_runtime::LOCAL_USER_ID)?;
    let prepared = prepare(root, &scope(&state.workspace), name, args)?;
    finish(state, root, prepared, now)
}

pub fn dispatch_local(
    state: &std::sync::Mutex<AppState>,
    root: &Path,
    name: &str,
    args: &Value,
    now: &str,
) -> Result<Value, ToolError> {
    let (active_scope, library) = {
        let guard = state.lock().unwrap();
        guard.user.check_owner(crate::user_runtime::LOCAL_USER_ID)?;
        (scope(&guard.workspace), crate::state_library_root(&guard))
    };
    // Network and directory transfers can run while the reader continues to navigate.
    let prepared = prepare(root, &active_scope, name, args)?;
    let mut guard = state.lock().unwrap();
    if scope(&guard.workspace) != active_scope || crate::state_library_root(&guard) != library {
        return Err(invalid(
            "The open material or library changed. Retry in the intended Reader scope.",
        ));
    }
    finish(&mut guard, root, prepared, now)
}

pub(crate) enum Prepared {
    Done(Value),
    InstallSkill {
        workspace: PathBuf,
        staging: tempfile::TempDir,
        name: String,
    },
    ChangeSkill {
        workspace: PathBuf,
        name: String,
        enabled: Option<bool>,
    },
    Metadata(Option<String>),
    Import {
        book_id: Option<String>,
        source: Option<Value>,
        pdf: Option<PathBuf>,
    },
}

pub(crate) fn finish(
    state: &mut AppState,
    root: &Path,
    action: Prepared,
    now: &str,
) -> Result<Value, ToolError> {
    match action {
        Prepared::Done(value) => Ok(value),
        Prepared::InstallSkill {
            workspace,
            staging,
            name,
        } => skills::install(&workspace, staging, &name).map_err(invalid),
        Prepared::ChangeSkill {
            workspace,
            name,
            enabled,
        } => skills::change(&workspace, &name, enabled).map_err(invalid),
        Prepared::Metadata(id) => metadata(
            state,
            root,
            id.as_deref().unwrap_or(&state.workspace.book.base.book_id),
        ),
        Prepared::Import {
            book_id,
            source,
            pdf,
        } => import(state, root, book_id.as_deref(), source, pdf, now),
    }
}

pub(crate) fn prepare(
    root: &Path,
    scope: &str,
    name: &str,
    args: &Value,
) -> Result<Prepared, ToolError> {
    let workspace = skills::workspace(root, scope);
    let immediate = match name {
        "skill_list" => {
            let mut result = skills::list(&workspace).map_err(invalid)?;
            result["scope"] = scope.into();
            Ok(result)
        }
        "skill_import" => {
            let (staging, name) = skills::stage(
                &workspace,
                required(args, "source")?,
                string(args, "subdir").unwrap_or(""),
            )
            .map_err(invalid)?;
            return Ok(Prepared::InstallSkill {
                workspace,
                staging,
                name,
            });
        }
        "skill_enable" | "skill_remove" => {
            let enabled = if name == "skill_enable" {
                Some(
                    args["enabled"]
                        .as_bool()
                        .ok_or_else(|| invalid("Missing enabled flag"))?,
                )
            } else {
                None
            };
            return Ok(Prepared::ChangeSkill {
                workspace,
                name: required(args, "name")?.to_owned(),
                enabled,
            });
        }
        "agentero_papers" => {
            let query = match string(args, "query") {
                Some(q) => json!({"query":q}),
                None => json!({}),
            };
            agentero::Agentero::connect()
                .and_then(|mut c| c.tool("paper_list", query))
                .map_err(remote)
        }
        "paper_metadata" => {
            return Ok(Prepared::Metadata(
                string(args, "book_id").map(str::to_owned),
            ))
        }
        "paper_import" | "agentero_import" => {
            let (source, pdf) = if name == "agentero_import" {
                let (source, found) = agentero::Agentero::connect()
                    .and_then(|mut c| c.select(string(args, "paper"), string(args, "identifier")))
                    .map_err(remote)?;
                (
                    Some(source),
                    string(args, "local_pdf").map(PathBuf::from).or(found),
                )
            } else {
                (None, Some(PathBuf::from(required(args, "path")?)))
            };
            return Ok(Prepared::Import {
                book_id: string(args, "book_id").map(str::to_owned),
                source,
                pdf,
            });
        }
        _ => Err(invalid(format!("Unknown paper or Skill operation {name}"))),
    }?;
    Ok(Prepared::Done(immediate))
}

fn origin_path(root: &Path, id: &str) -> PathBuf {
    root.join("learning-papers").join(id).join("origin.json")
}

pub(crate) fn has_paper_origin(root: &Path, id: &str) -> bool {
    crate::is_valid_book_id(id) && origin_path(root, id).is_file()
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StoredOrigin {
    source: Value,
    reader_dir: Option<PathBuf>,
}

impl StoredOrigin {
    fn for_directory(self, dir: &Path) -> Option<Value> {
        let canonical = dir.canonicalize().ok()?;
        (self.reader_dir.as_ref() == Some(&canonical)).then_some(self.source)
    }
}

fn save_origin(path: &Path, source: &Value, book_dir: Option<&Path>) -> Result<(), ToolError> {
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir).map_err(io)?;
    let mut staged = tempfile::NamedTempFile::new_in(dir).map_err(io)?;
    let record = StoredOrigin {
        source: source.clone(),
        reader_dir: book_dir.map(Path::canonicalize).transpose().map_err(io)?,
    };
    serde_json::to_writer_pretty(staged.as_file_mut(), &record)
        .map_err(|e| invalid(e.to_string()))?;
    staged.persist(path).map_err(|e| io(e.error))?;
    Ok(())
}

fn read_origin(path: &Path) -> Result<Option<StoredOrigin>, ToolError> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| invalid(e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(e)),
    }
}

fn import(
    state: &mut AppState,
    root: &Path,
    book_id: Option<&str>,
    source: Option<Value>,
    pdf: Option<PathBuf>,
    now: &str,
) -> Result<Value, ToolError> {
    let id = book_id
        .map(str::to_owned)
        .unwrap_or_else(|| format!("paper-{}", uuid::Uuid::now_v7().simple()));
    if !crate::is_valid_book_id(&id) {
        return Err(invalid("Invalid paper book_id"));
    }
    let target = crate::state_library_root(state).join(&id);
    if target.exists()
        || crate::list_mixed_book_library(&crate::state_library_root(state))
            .iter()
            .any(|entry| entry.book_id == id)
    {
        return Err(invalid(format!(
            "Book {id} already exists. Open the existing material or choose another ID."
        )));
    }
    let path = origin_path(root, &id);
    let previous = read_origin(&path)?;
    if previous
        .as_ref()
        .is_some_and(|old| old.reader_dir.is_some())
    {
        return Err(invalid(
            "This material ID already belongs to an imported paper. Choose another ID.",
        ));
    }
    let previous = previous.map(|old| old.source);
    if let (Some(new), Some(old)) = (&source, &previous) {
        if new["vault"] != old["vault"]
            || new["metadata"]["id"] != old["metadata"]["id"]
            || new["metadata"]["path"] != old["metadata"]["path"]
        {
            return Err(invalid(
                "This paper id is already associated with another catalog entry",
            ));
        }
    }
    let source = source.or(previous);
    let Some(pdf) = pdf else {
        let source = source.ok_or_else(|| invalid("Provide a paper PDF"))?;
        save_origin(&path, &source, None)?;
        return Ok(
            json!({"book_id":id,"status":"metadata_only","source":source,"next":"paper_import","arguments":{"book_id":id,"path":"<PDF>"}}),
        );
    };
    if !pdf
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("pdf"))
        || !pdf.is_file()
    {
        return Err(invalid(format!(
            "Select an existing PDF file: {}",
            pdf.display()
        )));
    }
    let pdf = pdf.canonicalize().map_err(io)?;
    let title = source
        .as_ref()
        .and_then(|v| v["metadata"]["title"].as_str())
        .or(pdf.file_stem().and_then(|v| v.to_str()))
        .unwrap_or(&id);
    let args = json!({"book_id":id,"target_dir":target,"display_title":title,"paper_pdf_path":pdf});
    // Workbench targets the active book internally; run this import against its own
    // draft and restore the reader's open book without changing the saved viewport.
    let old_dir = std::mem::replace(&mut state.workspace.book_dir, target.clone());
    let old_revision = state.workspace.workbench_loaded_revision.take();
    let session = state.workspace.session_path.take();
    let reply = crate::route_workbench_input_import(state, &args.to_string(), now);
    state.workspace.book_dir = old_dir;
    state.workspace.workbench_loaded_revision = old_revision;
    state.workspace.session_path = session;
    if target.join("paper.pdf").is_file() {
        if let Some(src) = &source {
            // Preserve provenance once the workbench has copied a PDF, including when a job fails to start.
            save_origin(&path, src, Some(&target))?;
        }
    }
    let receipt: Value = serde_json::from_str(&reply.body).map_err(|e| invalid(e.to_string()))?;
    if reply.status != 200 {
        return Err(error(
            receipt["error_code"]
                .as_str()
                .unwrap_or("PAPER_IMPORT_FAILED"),
            receipt["category"].as_str().unwrap_or("internal"),
            format!(
                "{} ({id})",
                receipt["message"]
                    .as_str()
                    .unwrap_or("Workbench import failed")
            ),
        ));
    }
    Ok(
        json!({"book_id":id,"dir":target,"source":source,"readiness":receipt["readiness"],"jobs":receipt["jobs"],"next_action":{"method":"POST","path":"/api/book/open","arguments":{"dir":target}}}),
    )
}

pub(crate) fn prepared_metadata(root: &Path, book: &Book, dir: &Path) -> Result<Value, ToolError> {
    let id = &book.base.book_id;
    let source = if crate::is_valid_book_id(id) {
        read_origin(&origin_path(root, id))?.and_then(|old| old.for_directory(dir))
    } else {
        None
    };
    Ok(json!({"book_id":id,"source":source,"projection":book.paper_metadata_projection()}))
}

pub(crate) fn imported_metadata(root: &Path, dir: &Path, id: &str) -> Result<Value, ToolError> {
    let origin = if crate::is_valid_book_id(id) {
        read_origin(&origin_path(root, id))?
    } else {
        None
    };
    match crate::book_library_entry(dir) {
        Some(entry) if entry.book_id == id && entry.route == "reader" => {
            let book = Book::load(&entry.dir).map_err(|e| error("PAPER_LOAD_FAILED", "data", e))?;
            let source = origin.and_then(|old| old.for_directory(dir));
            Ok(json!({"book_id":id,"source":source,"projection":book.paper_metadata_projection()}))
        }
        Some(entry) if entry.book_id == id => {
            let source = origin.and_then(|old| old.for_directory(dir));
            Ok(json!({"book_id":id,"source":source,"status":entry.route,"projection":null}))
        }
        None if origin
            .as_ref()
            .is_some_and(|record| record.reader_dir.is_none()) =>
        {
            Ok(
                json!({"book_id":id,"source":origin.map(|old|old.source),"status":"metadata_only","projection":null}),
            )
        }
        _ => Err(crate::service_error::missing()),
    }
}

fn metadata(state: &AppState, root: &Path, id: &str) -> Result<Value, ToolError> {
    if state.workspace.book.base.book_id == id {
        return prepared_metadata(root, &state.workspace.book, &state.workspace.book_dir);
    }
    if !crate::is_valid_book_id(id) {
        return Err(crate::service_error::missing());
    }
    let library = crate::state_library_root(state);
    let hits: Vec<_> = crate::list_mixed_book_library(&library)
        .into_iter()
        .filter(|entry| entry.book_id == id)
        .collect();
    match hits.as_slice() {
        [] => imported_metadata(root, &library.join(id), id),
        [entry] => imported_metadata(root, Path::new(&entry.dir), id),
        _ => Err(invalid(
            "This book id occurs in multiple library entries. Open the intended book first.",
        )),
    }
}
