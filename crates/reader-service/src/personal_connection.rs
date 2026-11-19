//! Personal tools are enabled by the local Reader and execute under its single private writer.
use crate::{err_reply, ok_json, user_storage_paths::error, AppState, Reply, Req};
use read_tools::{Book, ToolError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

pub(crate) mod acp;
mod commands;
mod schema;
#[cfg(test)]
mod tests;

type Result<T> = std::result::Result<T, ToolError>;

fn invalid(message: &str) -> ToolError {
    error("PERSONAL_INPUT_INVALID", "validation", message)
}
fn denied(message: &str) -> ToolError {
    error("PERSONAL_ACCESS_DENIED", "permission", message)
}
fn storage(message: impl std::fmt::Display) -> ToolError {
    error("PERSONAL_STORAGE", "unavailable", &message.to_string())
}
fn field<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| invalid(&format!("请指定 {key}")))
}

#[derive(Serialize, Deserialize, Clone)]
struct Material {
    book_id: String,
    title: String,
    dir: PathBuf,
}

#[derive(Serialize, Deserialize, Clone)]
struct Grant {
    token: String,
    books: Vec<Material>,
    write: bool,
    navigate: bool,
    #[serde(default)]
    manage: bool,
    workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    paper_imports: BTreeMap<String, PathBuf>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PassageRef {
    book_id: String,
    source_revision: String,
    lid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    range: Option<memory::TextRange>,
}

impl Grant {
    fn imported_in(&self, id: &str, library: &Path) -> bool {
        self.paper_imports
            .get(id)
            .is_some_and(|original| same_directory(original, library))
    }
    fn material(&self, id: &str) -> Result<&Material> {
        self.books
            .iter()
            .find(|b| b.book_id == id)
            .ok_or_else(|| denied("这本书不在个人连接范围内"))
    }
    fn load(&self, state: &AppState, id: &str) -> Result<Arc<Book>> {
        let material = self.material(id)?;
        if state.workspace.book.base.book_id == id
            && same_directory(&state.workspace.book_dir, &material.dir)
        {
            return Ok(state.workspace.book.clone());
        }
        let book = Book::load(
            material
                .dir
                .to_str()
                .ok_or_else(|| invalid("材料路径不可用"))?,
        )
        .map_err(storage)?;
        if book.base.book_id != id {
            return Err(denied("原材料已经更换，请重新选择"));
        }
        Ok(Arc::new(book))
    }
    fn resolve(&self, state: &AppState, v: &Value) -> Result<(Arc<Book>, PassageRef, String)> {
        let source: PassageRef = serde_json::from_value(v.clone())
            .map_err(|_| invalid("需要完整的材料、修订和段落位置"))?;
        let book = self.load(state, &source.book_id)?;
        if source.source_revision != book.source_fingerprint() {
            return Err(error(
                "PERSONAL_SOURCE_CHANGED",
                "conflict",
                "材料已更新，请重新读取原文",
            ));
        }
        let quote = passage(&book, &source.lid, source.range.as_ref())?;
        Ok((book, source, quote))
    }
}

fn same_directory(left: &Path, right: &Path) -> bool {
    left == right
        || left
            .canonicalize()
            .is_ok_and(|a| right.canonicalize().is_ok_and(|b| a == b))
}

fn passage(book: &Book, lid: &str, range: Option<&memory::TextRange>) -> Result<String> {
    let text = book.text(lid, None)?;
    let Some(range) = range else { return Ok(text) };
    let units: Vec<_> = text.encode_utf16().collect();
    if range.start >= range.end {
        return Err(invalid("选区范围为空或反向"));
    }
    let slice = units
        .get(range.start as usize..range.end as usize)
        .ok_or_else(|| invalid("选区越出原文"))?;
    String::from_utf16(slice).map_err(|_| invalid("选区未落在完整字符上"))
}

fn materials(state: &AppState) -> Vec<Material> {
    let mut books: Vec<_> = crate::list_mixed_book_library(&crate::state_library_root(state))
        .into_iter()
        .filter(|item| item.route == "reader")
        .map(|item| Material {
            book_id: item.book_id,
            title: item.title.unwrap_or(item.name),
            dir: PathBuf::from(item.dir),
        })
        .collect();
    let current = &state.workspace;
    if current.book_dir.file_name().and_then(|s| s.to_str()) != Some("__desktop_bootstrap__")
        && current.book_dir.join("source.txt").is_file()
        && !books.iter().any(|b| {
            b.book_id == current.book.base.book_id && same_directory(&b.dir, &current.book_dir)
        })
    {
        books.push(Material {
            book_id: current.book.base.book_id.clone(),
            title: current.book.base.book_id.clone(),
            dir: current.book_dir.clone(),
        });
    }
    books
}

fn dir(state: &AppState) -> Result<&Path> {
    if state.services.reader_only
        || state.user.user_id() != Some(crate::user_runtime::LOCAL_USER_ID)
    {
        return Err(denied("个人学习连接仅在本机私人 Reader 上可用"));
    }
    state
        .user
        .history_path
        .as_deref()
        .and_then(Path::parent)
        .ok_or_else(|| storage("当前没有本机私人存储"))
}

fn grant_path(dir: &Path) -> PathBuf {
    dir.join("personal-connection.json")
}

fn read_grant(dir: &Path) -> Result<Option<Grant>> {
    match fs::read(grant_path(dir)) {
        Ok(data) => serde_json::from_slice(&data).map(Some).map_err(storage),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(storage(e)),
    }
}

fn write_grant(dir: &Path, grant: &Grant) -> Result<()> {
    memory::ReaderPrivateStorageGate::create_dir_all(dir).map_err(storage)?;
    let mut temporary = tempfile::NamedTempFile::new_in(dir).map_err(storage)?;
    serde_json::to_writer(&mut temporary, grant).map_err(storage)?;
    temporary.persist(grant_path(dir)).map_err(storage)?;
    Ok(())
}

fn settings(state: &AppState, grant: Option<&Grant>) -> Value {
    json!({"enabled":grant.is_some(),
        "book_ids":grant.into_iter().flat_map(|g|g.books.iter().map(|b|&b.book_id)).collect::<Vec<_>>(),
        "write":grant.is_some_and(|g|g.write),"navigate":grant.is_some_and(|g|g.navigate),"manage":grant.is_some_and(|g|g.manage),
        "materials":materials(state).into_iter().map(|b|json!({"book_id":b.book_id,"title":b.title})).collect::<Vec<_>>()})
}

fn configure(state: &AppState, dir: &Path, v: &Value) -> Result<Value> {
    match field(v, "action")? {
        "enable" => {
            let choices = materials(state);
            let requested = match v.get("book_ids") {
                Some(value) => serde_json::from_value::<Vec<String>>(value.clone())
                    .map_err(|_| invalid("请选择允许的书"))?,
                None => vec![state.workspace.book.base.book_id.clone()],
            };
            if requested.is_empty() {
                return Err(invalid("至少选择一本资料"));
            }
            let mut books = Vec::new();
            for id in requested {
                if books.iter().any(|b: &Material| b.book_id == id) {
                    continue;
                }
                let mut choices = choices.iter().filter(|b| b.book_id == id);
                let material = choices
                    .next()
                    .ok_or_else(|| invalid("请选择本机可读材料"))?;
                if choices.next().is_some() {
                    return Err(invalid("书库里有相同身份的材料，请先选定唯一的书库副本"));
                }
                books.push(material.clone());
            }
            let grant = Grant {
                token: uuid::Uuid::now_v7().to_string(),
                books,
                write: v["write"].as_bool() == Some(true),
                navigate: v["navigate"].as_bool() == Some(true),
                manage: v["manage"].as_bool() == Some(true),
                workspace_id: state.workspace.learning_workspace_id.clone(),
                paper_imports: BTreeMap::new(),
            };
            write_grant(dir, &grant)?;
            NAV.lock().unwrap().remove(dir);
            let mut view = settings(state, Some(&grant));
            view["token"] = json!(grant.token);
            Ok(view)
        }
        "revoke" => {
            match fs::remove_file(grant_path(dir)) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(storage(e)),
            }
            NAV.lock().unwrap().remove(dir);
            Ok(settings(state, None))
        }
        _ => Err(invalid("未知的个人连接操作")),
    }
}

static NAV: LazyLock<Mutex<BTreeMap<PathBuf, Value>>> = LazyLock::new(Default::default);

static CHANGES: LazyLock<Mutex<BTreeMap<PathBuf, u64>>> = LazyLock::new(Default::default);
fn mark_changed(root: &Path) {
    *CHANGES.lock().unwrap().entry(root.into()).or_default() += 1;
}
fn change_id(root: &Path) -> u64 {
    CHANGES.lock().unwrap().get(root).copied().unwrap_or(0)
}

fn closed_source(state: &AppState, book: &str) -> Result<bool> {
    let scope = state.workspace.learning_workspace_id.as_deref();
    let store = state.user.learning_store_for_workspace(scope)?;
    Ok(!store
        .open_closed_book_chats(scope.is_none().then_some(book))?
        .is_empty())
}

fn closed(state: &AppState) -> Result<bool> {
    closed_source(state, &state.workspace.book.base.book_id)
}

fn authenticated(state: &AppState, root: &Path, body: &Value) -> Result<Grant> {
    let grant = read_grant(root)?
        .filter(|g| body["token"].as_str() == Some(g.token.as_str()))
        .ok_or_else(|| denied("个人连接未启用、已撤销或令牌不匹配"))?;
    if grant.workspace_id != state.workspace.learning_workspace_id {
        return Err(denied("请返回已授权的学习工作区"));
    }
    Ok(grant)
}

fn manages_current(state: &AppState, grant: &Grant) -> bool {
    grant.manage && grant.write && grant.material(&state.workspace.book.base.book_id).is_ok()
}

fn handle(state: &mut AppState, req: Req<'_>) -> Result<Value> {
    let root = dir(state)?.to_path_buf();
    let path = req.url.trim_start_matches("/api");
    let body: Value = if req.body.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(req.body).map_err(|_| invalid("请求需要 JSON"))?
    };
    match (req.method, path) {
        ("GET", "/personal/connection") => Ok(settings(state, read_grant(&root)?.as_ref())),
        ("POST", "/personal/connection") => configure(state, &root, &body),
        ("GET", "/personal/navigation") => {
            let pending = NAV
                .lock()
                .unwrap()
                .get(&root)
                .filter(|p| p["displayed"].is_null())
                .cloned();
            let Some(pending) = pending else {
                return Ok(json!({"change_id":change_id(&root),"pending":null}));
            };
            let mut blocked = closed(state)?;
            for item in pending["sources"].as_array().into_iter().flatten() {
                let id = field(&item["source_ref"], "book_id")?;
                if id != state.workspace.book.base.book_id && closed_source(state, id)? {
                    blocked = true;
                }
            }
            if blocked {
                NAV.lock().unwrap().remove(&root);
                return Ok(json!({"change_id":change_id(&root),"pending":null}));
            }
            Ok(json!({"change_id":change_id(&root),"pending":pending}))
        }
        ("POST", "/personal/navigation") => {
            let mut nav = NAV.lock().unwrap();
            let ticket = nav
                .get_mut(&root)
                .filter(|p| p["id"].is_string() && p["id"] == body["id"])
                .ok_or_else(|| invalid("导航请求已失效"))?;
            let displayed = body["displayed"]
                .as_bool()
                .ok_or_else(|| invalid("缺少页面显示结果"))?;
            ticket["displayed"] = json!(displayed);
            Ok(json!({"id":ticket["id"],"displayed":displayed}))
        }
        ("POST", "/personal/call") => {
            let grant = authenticated(state, &root, &body)?;
            let name = field(&body, "name")?;
            let args = &body["arguments"];
            let limited = closed(state)?;
            let mut tools = schema::tools(grant.write, grant.navigate, limited);
            if !limited {
                let manage_current = manages_current(state, &grant);
                tools["tools"].as_array_mut().unwrap().extend(
                    crate::learning_extensions::tools()
                        .into_iter()
                        .filter(|tool| manage_current || tool["name"] == "paper_metadata"),
                );
            }
            if name == "tools_list" {
                return Ok(tools);
            }
            if !tools["tools"]
                .as_array()
                .is_some_and(|available| available.iter().any(|s| s["name"] == name))
            {
                return Err(denied(if limited {
                    "本次闭卷练习只开放任务入口，请返回 Reader 继续"
                } else {
                    "请检查连接的资料和操作权限"
                }));
            }
            if !matches!(
                name,
                "material_list"
                    | "task_list"
                    | "paper_import"
                    | "agentero_import"
                    | "paper_metadata"
            ) {
                for id in [
                    args["book_id"].as_str(),
                    args["source_ref"]["book_id"].as_str(),
                    args["other_source_ref"]["book_id"].as_str(),
                ]
                .into_iter()
                .flatten()
                {
                    grant.material(id)?;
                    if id != state.workspace.book.base.book_id && closed_source(state, id)? {
                        return Err(denied("这份材料有正在进行的闭卷练习，请先返回 Reader"));
                    }
                }
            }
            commands::call(state, &grant, &root, name, args, limited, req.now, None)
        }
        _ => Err(invalid("个人连接的请求方法或路径无效")),
    }
}

pub fn route(state: &mut AppState, req: Req<'_>) -> Reply {
    match handle(state, req) {
        Ok(value) => ok_json(&value),
        Err(e) => err_reply(&e),
    }
}

/// Remote catalogs and Skill downloads must not hold the Reader's single writer.
pub(crate) fn route_host(state: &Mutex<AppState>, req: Req<'_>) -> Reply {
    if req.method == "POST" && req.url.trim_start_matches("/api") == "/personal/call" {
        if let Ok(body) = serde_json::from_str::<Value>(req.body) {
            if let Some(name @ ("agentero_papers" | "agentero_import" | "skill_import")) =
                body["name"].as_str()
            {
                return match transfer(state, &body, name, req.now) {
                    Ok(value) => ok_json(&value),
                    Err(error) => err_reply(&error),
                };
            }
        }
    }
    route(&mut state.lock().unwrap(), req)
}

fn transfer(state: &Mutex<AppState>, body: &Value, name: &str, now: &str) -> Result<Value> {
    let args = &body["arguments"];
    let authorize = |state: &AppState, root: &Path| -> Result<Grant> {
        let grant = authenticated(state, root, body)?;
        if !manages_current(state, &grant) || closed(state)? {
            return Err(denied("请检查连接的管理权限和当前练习"));
        }
        check_import_origin(
            &grant,
            profile(root)?,
            &crate::state_library_root(state),
            name,
            args,
        )?;
        Ok(grant)
    };
    let (root, scope, library) = {
        let guard = state.lock().unwrap();
        let root = dir(&guard)?.to_path_buf();
        authorize(&guard, &root)?;
        (
            root,
            crate::learning_extensions::scope(&guard.workspace),
            crate::state_library_root(&guard),
        )
    };
    let prepared = crate::learning_extensions::prepare(profile(&root)?, &scope, name, args)?;
    let mut guard = state.lock().unwrap();
    if dir(&guard)? != root
        || crate::learning_extensions::scope(&guard.workspace) != scope
        || crate::state_library_root(&guard) != library
    {
        return Err(denied("学习范围已更换，请回到原资料继续"));
    }
    let grant = authorize(&guard, &root)?;
    let result = crate::learning_extensions::finish(&mut guard, profile(&root)?, prepared, now)?;
    record_extension_result(&root, &grant, &library, name, &result)?;
    Ok(result)
}

fn profile(root: &Path) -> Result<&Path> {
    root.parent().ok_or_else(|| storage("无法定位本机用户目录"))
}

fn check_import_origin(
    grant: &Grant,
    profile: &Path,
    library: &Path,
    name: &str,
    args: &Value,
) -> Result<()> {
    if matches!(name, "paper_import" | "agentero_import") {
        if let Some(id) = args["book_id"].as_str() {
            if crate::learning_extensions::has_paper_origin(profile, id)
                && !grant.imported_in(id, library)
                && grant.material(id).is_err()
            {
                return Err(denied("请在原连接中继续导入这篇论文"));
            }
        }
    }
    Ok(())
}

fn record_extension_result(
    root: &Path,
    grant: &Grant,
    library: &Path,
    name: &str,
    result: &Value,
) -> Result<()> {
    if matches!(name, "paper_import" | "agentero_import") {
        let id = field(result, "book_id")?;
        if !grant.imported_in(id, library) {
            let mut updated = grant.clone();
            updated.paper_imports.insert(id.into(), library.into());
            write_grant(root, &updated)?;
        }
    }
    if name != "agentero_papers" && name != "skill_list" {
        mark_changed(root);
    }
    Ok(())
}

fn navigation_result(root: &Path, id: &str) -> Result<Value> {
    let nav = NAV.lock().unwrap();
    let ticket = nav
        .get(root)
        .filter(|t| t["id"] == id)
        .ok_or_else(|| invalid("导航请求已被替换或关闭"))?;
    Ok(json!({"id":id,"displayed":ticket["displayed"]}))
}

pub(crate) fn current_navigation_result(state: &AppState, id: &str) -> Result<Value> {
    navigation_result(dir(state)?, id)
}

fn current_grant(state: &AppState) -> Grant {
    let scene = &state.workspace;
    Grant {
        token: String::new(),
        books: vec![Material {
            book_id: scene.book.base.book_id.clone(),
            title: scene.book.base.book_id.clone(),
            dir: scene.book_dir.clone(),
        }],
        write: true,
        navigate: true,
        manage: false,
        workspace_id: scene.learning_workspace_id.clone(),
        paper_imports: BTreeMap::new(),
    }
}

/// The active local ACP turn can explicitly request navigation in its frozen book.
/// The caller validates the active run before entering; the window acknowledges rendering separately.
pub(crate) fn request_current_navigation(
    state: &AppState,
    sources: &[(String, Option<memory::TextRange>)],
) -> Result<Value> {
    let root = dir(state)?;
    if closed(state)? {
        return Err(denied("请先离开闭卷现场再查看原文"));
    }
    if !(1..=2).contains(&sources.len()) {
        return Err(invalid("请选择一处或两处原文"));
    }
    let scene = &state.workspace;
    let id = &scene.book.base.book_id;
    let grant = current_grant(state);
    let refs: Vec<_> = sources
        .iter()
        .map(|(lid, range)| {
            json!(PassageRef {
                book_id: id.clone(),
                source_revision: scene.book.source_fingerprint().into(),
                lid: lid.clone(),
                range: range.clone()
            })
        })
        .collect();
    commands::request_navigation(
        state,
        &grant,
        root,
        &json!({"source_ref":refs[0],"other_source_ref":refs.get(1)}),
        refs.len() == 2,
    )
}
