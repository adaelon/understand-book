//! Account-wide reservations for uploaded original materials.
//! Learning artifacts use their own storage lifecycle, outside upload quotas.
//! Callers serialize upload admission in an IMMEDIATE control transaction.
use crate::{admin_store::invalid, authorization::{missing, Authorization}, cloud_plans,
    user_storage_paths::error};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}, sync::Arc};

const UPLOAD_IDLE_SECONDS: i64 = 24 * 60 * 60;
const BUILD_CACHE_SECONDS: i64 = 7 * 24 * 60 * 60;
fn storage<T>(_: T) -> ToolError { error("STORAGE_UNAVAILABLE", "unavailable", "资料空间暂时不可用") }
fn conflict(code: &str, message: &str) -> ToolError { error(code, "conflict", message) }

fn total(db: &Connection, owner: &str) -> Result<(i64, i64), ToolError> {
    db.query_row("SELECT COALESCE(sum(CASE WHEN state='used' THEN bytes ELSE 0 END),0),COALESCE(sum(CASE WHEN state='reserved' THEN bytes ELSE 0 END),0) FROM cloud_storage_objects WHERE owner_user_id=? AND kind='upload'",
        [owner], |r| Ok((r.get(0)?, r.get(1)?))).map_err(storage)
}

fn admit(db: &Connection, owner: &str, bytes: i64, previous: i64, now: i64) -> Result<(), ToolError> {
    let entitlement = cloud_plans::current(db, owner, now)?
        .ok_or_else(|| conflict("STORAGE_PLAN_REQUIRED", "请先开通有效学习套餐，再上传书籍、视频或课件原件"))?;
    if bytes > entitlement.max_file_bytes {
        return Err(conflict("STORAGE_FILE_TOO_LARGE", "这份文件超过套餐的单文件上限"));
    }
    let (used, reserved) = total(db, owner)?;
    if used.saturating_add(reserved).saturating_sub(previous).saturating_add(bytes) > entitlement.storage_bytes {
        return Err(conflict("STORAGE_LIMIT_EXCEEDED", "账号空间不足，请先整理资料或续用更大空间的套餐"));
    }
    Ok(())
}

pub(crate) fn reserve(db: &Connection, owner: &str, workspace: &str, id: &str, name: &str,
    path: &Path, bytes: i64, now: i64) -> Result<(), ToolError> {
    if bytes <= 0 { return Err(invalid()); }
    let old: Option<(String,String,i64,String)> = db.query_row(
        "SELECT workspace_id,filename,bytes,state FROM cloud_storage_objects WHERE owner_user_id=? AND storage_id=?",
        params![owner,id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(storage)?;
    if let Some((old_workspace, old_name, old_bytes, state)) = old {
        if old_workspace != workspace || old_name != name || old_bytes != bytes || state == "deleted" {
            return Err(conflict("STORAGE_UPLOAD_CONFLICT", "上传标识已绑定其他资料或已取消，请重新选择文件"));
        }
        // Resume does not allocate more space, but an expired/downgraded account
        // cannot keep adding file bytes through an old reservation.
        admit(db, owner, bytes, bytes, now)?;
        db.execute("UPDATE cloud_storage_objects SET updated_at=? WHERE owner_user_id=? AND storage_id=?",
            params![now,owner,id]).map_err(storage)?;
        return Ok(());
    }
    admit(db, owner, bytes, 0, now)?;
    db.execute("INSERT INTO cloud_storage_objects(owner_user_id,storage_id,kind,workspace_id,filename,path,bytes,state,created_at,updated_at) VALUES(?,?,'upload',?,?,?,?,'reserved',?,?)",
        params![owner,id,workspace,name,path.to_string_lossy(),bytes,now,now]).map_err(storage)?;
    Ok(())
}

pub(crate) fn uploaded(db: &Connection, owner: &str, id: &str, now: i64) -> Result<(), ToolError> {
    db.execute("UPDATE cloud_storage_objects SET state='used',updated_at=? WHERE owner_user_id=? AND storage_id=? AND state='reserved'",
        params![now,owner,id]).map_err(storage)?;
    Ok(())
}

pub(crate) fn bind_task(db: &Connection, owner: &str, id: &str, task: &str, now: i64) -> Result<(), ToolError> {
    let changed = db.execute("UPDATE cloud_storage_objects SET task_id=?,updated_at=? WHERE owner_user_id=? AND storage_id=? AND state='used' AND (task_id IS NULL OR task_id=?)",
        params![task,now,owner,id,task]).map_err(storage)?;
    if changed == 0 { return Err(conflict("STORAGE_UPLOAD_UNAVAILABLE", "上传已取消或尚未完成，请重新选择资料")); }
    Ok(())
}

fn files(root: &Path) -> Result<Vec<PathBuf>, ToolError> {
    if !root.exists() { return Ok(vec![]); }
    let mut pending = vec![root.to_path_buf()];
    let mut result = Vec::new();
    while let Some(dir) = pending.pop() {
        for item in fs::read_dir(dir).map_err(storage)? {
            let item = item.map_err(storage)?;
            if item.file_type().map_err(storage)?.is_dir() { pending.push(item.path()); }
            else if item.file_type().map_err(storage)?.is_file() { result.push(item.path()); }
        }
    }
    Ok(result)
}

/// Existing account-owned originals are adopted without requiring a purchase.
/// Over-limit old accounts can still read, export and remove their material.
pub(crate) fn adopt(db: &Connection, root: &Path, owner: &str, now: i64) -> Result<(), ToolError> {
    let adopted: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM cloud_storage_adoptions WHERE owner_user_id=?)",
        [owner], |r| r.get(0)).map_err(storage)?;
    if !adopted {
        let private = root.join("personal-preparations").join(owner);
        if private.is_dir() {
            for workspace in fs::read_dir(&private).map_err(storage)? {
                let workspace = workspace.map_err(storage)?;
                let uploads = workspace.path().join("uploads");
                if !uploads.is_dir() { continue; }
                for upload in fs::read_dir(uploads).map_err(storage)? {
                    let upload = upload.map_err(storage)?;
                    let meta_path = upload.path().join("upload.json");
                    if !meta_path.is_file() { continue; }
                    let meta: Value = serde_json::from_slice(&fs::read(meta_path).map_err(storage)?).map_err(storage)?;
                    let Some(extension) = meta["extension"].as_str() else { continue };
                    let Some(name) = meta["filename"].as_str() else { continue };
                    let Some(bytes) = meta["total"].as_i64() else { continue };
                    let path = upload.path().join(meta["source_file"].as_str().map(str::to_owned).unwrap_or_else(||format!("source.{extension}")));
                    let received = fs::metadata(&path).map(|m|m.len()).unwrap_or(0);
                    db.execute("INSERT OR IGNORE INTO cloud_storage_objects(owner_user_id,storage_id,kind,workspace_id,filename,path,bytes,state,created_at,updated_at) VALUES(?,?,'upload',?,?,?,?,?,?,?)",
                        params![owner,upload.file_name().to_string_lossy(),workspace.file_name().to_string_lossy(),name,path.to_string_lossy(),bytes,if received == bytes as u64 {"used"} else {"reserved"},now,now]).map_err(storage)?;
                }
            }
        }
        db.execute("INSERT INTO cloud_storage_adoptions(owner_user_id,adopted_at) VALUES(?,?)",params![owner,now]).map_err(storage)?;
    }
    // The worker attaches publication references after preparation. The original
    // remains the single billable object when that publication is reused.
    db.execute("UPDATE cloud_storage_objects AS s SET task_id=COALESCE(task_id,(SELECT b.task_id FROM operator_builds b,json_each(b.task_json,'$.materials') m WHERE json_extract(b.task_json,'$.user_id')=s.owner_user_id AND json_extract(m.value,'$.target_input')=s.path LIMIT 1)) WHERE owner_user_id=? AND kind='upload' AND state<>'deleted'", [owner]).map_err(storage)?;
    db.execute("UPDATE cloud_storage_objects AS s SET book_id=(SELECT json_extract(task_json,'$.materials[0].published_book_ref.book_id') FROM operator_builds WHERE task_id=s.task_id),publication_id=(SELECT json_extract(task_json,'$.materials[0].published_book_ref.publication_id') FROM operator_builds WHERE task_id=s.task_id) WHERE owner_user_id=? AND kind='upload' AND task_id IS NOT NULL AND state<>'deleted'",[owner]).map_err(storage)?;
    Ok(())
}

pub(crate) fn summary(db: &Connection, owner: &str, now: i64) -> Result<Value, ToolError> {
    let current = cloud_plans::current(db,owner,now)?;
    let latest = cloud_plans::latest(db,owner)?;
    let (used,reserved) = total(db,owner)?;
    let limit = current.as_ref().map(|v|v.storage_bytes);
    let all = used.saturating_add(reserved);
    let mut q = db.prepare("SELECT storage_id,kind,workspace_id,task_id,filename,bytes,state,book_id,publication_id,updated_at FROM cloud_storage_objects WHERE owner_user_id=? AND kind='upload' AND state<>'deleted' ORDER BY created_at DESC,storage_id").map_err(storage)?;
    let items = q.query_map([owner], |r| {
        let id: String = r.get(0)?;
        Ok(json!({"storage_id":id,"kind":r.get::<_,String>(1)?,"workspace_id":r.get::<_,Option<String>>(2)?,
            "task_id":r.get::<_,Option<String>>(3)?,"filename":r.get::<_,String>(4)?,"bytes":r.get::<_,i64>(5)?,
            "state":r.get::<_,String>(6)?,"book_id":r.get::<_,Option<String>>(7)?,"publication_id":r.get::<_,Option<String>>(8)?,
            "updated_at":r.get::<_,i64>(9)?,"download_url":format!("/api/account/storage/download?storage_id={}",url::form_urlencoded::byte_serialize(id.as_bytes()).collect::<String>())}))
    }).map_err(storage)?.collect::<Result<Vec<_>,_>>().map_err(storage)?;
    Ok(json!({"used_bytes":used,"reserved_bytes":reserved,"total_bytes":all,"limit_bytes":limit,
        "max_file_bytes":current.as_ref().map(|v|v.max_file_bytes),"available_bytes":limit.map(|n|(n-all).max(0)).unwrap_or(0),
        "can_upload":limit.is_some_and(|n|all<n),"over_limit":limit.is_some_and(|n|all>n),
        "expires_at":current.as_ref().map(|v|v.expires_at),
        "retained_until":latest.as_ref().map(|v|v.expires_at.saturating_add(v.retention_seconds)),"items":items}))
}

fn remove_tree(path: &Path, allowed: &Path) -> Result<(),ToolError> {
    if !path.exists() { return Ok(()); }
    let resolved = path.canonicalize().map_err(storage)?;
    let boundary = allowed.canonicalize().map_err(storage)?;
    if resolved == boundary || !resolved.starts_with(&boundary) { return Err(invalid()); }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut pending=vec![resolved.clone()];
        while let Some(dir)=pending.pop() {
            fs::set_permissions(&dir,fs::Permissions::from_mode(0o700)).map_err(storage)?;
            for entry in fs::read_dir(&dir).map_err(storage)? {
                let entry=entry.map_err(storage)?;
                if entry.file_type().map_err(storage)?.is_dir() { pending.push(entry.path()); }
            }
        }
    }
    // Immutable publications have read-only files on Windows.
    for file in files(&resolved)? {
        let mut permissions = fs::metadata(&file).map_err(storage)?.permissions();
        if permissions.readonly() {
            #[cfg(windows)]
            { permissions.set_readonly(false); fs::set_permissions(file, permissions).map_err(storage)?; }
        }
    }
    fs::remove_dir_all(resolved).map_err(storage)
}

fn remove_object(db: &Connection, root: &Path, owner: &str, id: &str, cancel: bool, now: i64) -> Result<(), ToolError> {
    let item: Option<(String,String,Option<String>,Option<String>,Option<String>,String)> = db.query_row(
        "SELECT kind,path,task_id,book_id,publication_id,state FROM cloud_storage_objects WHERE owner_user_id=? AND storage_id=?",
        params![owner,id],|r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional().map_err(storage)?;
    let (kind,path,task,book,publication,state) = item.ok_or_else(missing)?;
    if state=="deleted" { return Ok(()); }
    if cancel && task.is_some() { return Err(conflict("STORAGE_IN_USE","这份上传已创建准备任务，请暂停任务后使用删除资料")); }
    if let Some(task) = &task {
        let state: String = db.query_row("SELECT json_extract(task_json,'$.state') FROM operator_builds WHERE task_id=?",[task],|r|r.get(0)).map_err(storage)?;
        if matches!(state.as_str(),"queued_prepare"|"preparing"|"queued"|"running") {
            return Err(conflict("STORAGE_TASK_ACTIVE","请先暂停资料准备，再删除资料"));
        }
    }
    if let (Some(book),Some(publication)) = (&book,&publication) {
        let used: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM learning_workspace_members WHERE book_id=? AND publication_id=?)",
            params![book,publication],|r|r.get(0)).map_err(storage)?;
        if used { return Err(conflict("STORAGE_IN_USE","资料仍被工作区引用，请先从所有工作区移出，再删除原件")); }
        // Keep public releases and another account's grants untouched.
        let shared: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM book_grants WHERE book_id=? AND publication_id=? AND owner_user_id<>?) OR EXISTS(SELECT 1 FROM default_book_grants WHERE book_id=? AND publication_id=?) OR EXISTS(SELECT 1 FROM workspace_releases,json_tree(manifest_json) j WHERE j.key='publication_id' AND j.value=?)",
            params![book,publication,owner,book,publication,publication],|r|r.get(0)).map_err(storage)?;
        db.execute("DELETE FROM book_grants WHERE owner_user_id=? AND book_id=? AND publication_id=?",params![owner,book,publication]).map_err(storage)?;
        if !shared {
            let published: String = db.query_row("SELECT directory FROM book_publications WHERE book_id=? AND publication_id=?",params![book,publication],|r|r.get(0)).map_err(storage)?;
            remove_tree(Path::new(&published),&root.join("published"))?;
        }
    }
    let path = Path::new(&path);
    if kind=="upload" {
        remove_tree(path.parent().ok_or_else(invalid)?,&root.join("personal-preparations").join(owner))?;
        if let Some(task) = &task {
            let raw: String=db.query_row("SELECT task_json FROM operator_builds WHERE task_id=?",[task],|r|r.get(0)).map_err(storage)?;
            let mut value: Value=serde_json::from_str(&raw).map_err(storage)?;
            for material in value["materials"].as_array().into_iter().flatten() {
                if let Some(build) = material["root_dir"].as_str() {
                    let build=Path::new(build);
                    if build.starts_with(root.join("personal-preparations").join(owner)) { remove_tree(build,&root.join("personal-preparations").join(owner))?; }
                }
            }
            value["state"]=json!("deleted");value["revision"]=json!(value["revision"].as_u64().unwrap_or(0)+1);value["updated_at"]=json!(now);
            db.execute("UPDATE operator_builds SET task_json=? WHERE task_id=?",params![value.to_string(),task]).map_err(storage)?;
        }
    } else {
        // Delivered learning artifacts may be referenced by chats and notes.
        // Their existing retention/removal workflow must remove those references.
        return Err(conflict("STORAGE_IN_USE","学习成果由聊天和笔记保留，请先从相应学习记录中整理"));
    }
    db.execute("UPDATE cloud_storage_objects SET state='deleted',updated_at=? WHERE owner_user_id=? AND storage_id=?",params![now,owner,id]).map_err(storage)?;
    Ok(())
}

/// Reclaims abandoned uploads and completed preparation caches. Retention expiry
/// never overrides a workspace reference or deletes charge history.
pub(crate) fn cleanup(db: &Connection, root: &Path, owner: &str, now: i64) -> Result<(),ToolError> {
    let latest=cloud_plans::latest(db,owner)?;
    let expired=cloud_plans::current(db,owner,now)?.is_none() && latest.is_some_and(|v|now>=v.expires_at.saturating_add(v.retention_seconds));
    let mut q=db.prepare("SELECT storage_id FROM cloud_storage_objects WHERE owner_user_id=? AND kind='upload' AND state<>'deleted' AND ((task_id IS NULL AND updated_at<?) OR ?=1)").map_err(storage)?;
    let ids=q.query_map(params![owner,now-UPLOAD_IDLE_SECONDS,expired],|r|r.get::<_,String>(0)).map_err(storage)?.collect::<Result<Vec<_>,_>>().map_err(storage)?;
    for id in ids {
        match remove_object(db,root,owner,&id,false,now) {
            Ok(())=>(),
            Err(e) if matches!(e.error_code.as_str(),"STORAGE_IN_USE"|"STORAGE_TASK_ACTIVE")=>(),
            Err(e)=>return Err(e),
        }
    }
    let mut q=db.prepare("SELECT task_json FROM operator_builds WHERE json_extract(task_json,'$.user_id')=? AND json_extract(task_json,'$.state')='ready' AND json_extract(task_json,'$.updated_at')<?").map_err(storage)?;
    let tasks=q.query_map(params![owner,now-BUILD_CACHE_SECONDS],|r|r.get::<_,String>(0)).map_err(storage)?.collect::<Result<Vec<_>,_>>().map_err(storage)?;
    for raw in tasks {
        let task: Value=serde_json::from_str(&raw).map_err(storage)?;
        for material in task["materials"].as_array().into_iter().flatten() {
            if let Some(path)=material["root_dir"].as_str() {
                let path=Path::new(path);
                if path.starts_with(root.join("personal-preparations").join(owner)) { remove_tree(path,&root.join("personal-preparations").join(owner))?; }
            }
        }
    }
    // Candidates are processing intermediates; delivered versions contain their
    // own complete content. Do not reclaim while the account has a live run.
    let active: bool=db.query_row("SELECT EXISTS(SELECT 1 FROM run_admissions WHERE owner_user_id=? AND dispatch_state IN ('preparing','queued','claimed'))",[owner],|r|r.get(0)).map_err(storage)?;
    if !active {
        let presentations=root.join("users").join(owner).join("memory/agent-history.presentations");
        for directory in ["candidates","method-observations"] {
            for path in files(&presentations.join(directory))? {
                let changed=fs::metadata(&path).map_err(storage)?.modified().ok().and_then(|v|v.duration_since(std::time::UNIX_EPOCH).ok()).map(|v|v.as_secs() as i64);
                if changed.is_some_and(|v|now-v>BUILD_CACHE_SECONDS) { fs::remove_file(path).map_err(storage)?; }
            }
        }
    }
    Ok(())
}

pub(crate) fn request(access: &Arc<Authorization>, owner: &str, method: &str, input: Value, now: i64) -> Result<Value,ToolError> {
    let mut control=access.admin.control.lock().unwrap();
    let root=control.writer.root().to_path_buf();
    let tx=control.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
    adopt(&tx,&root,owner,now)?;
    if method=="POST" {
        match input["action"].as_str().ok_or_else(invalid)? {
            "cancel"|"delete"=>remove_object(&tx,&root,owner,input["storage_id"].as_str().ok_or_else(invalid)?,input["action"]=="cancel",now)?,
            "cleanup"=>cleanup(&tx,&root,owner,now)?,
            _=>return Err(invalid()),
        }
    } else if method!="GET" { return Err(invalid()); }
    cleanup(&tx,&root,owner,now)?;
    tx.execute("UPDATE cloud_storage_adoptions SET maintained_at=? WHERE owner_user_id=?",params![now,owner]).map_err(storage)?;
    let result=summary(&tx,owner,now)?;
    tx.commit().map_err(storage)?;
    Ok(result)
}

/// The host's existing minute maintenance tick rotates at most 16 known
/// accounts. An account is scanned at most once per hour, including accounts
/// that never return to the storage page after abandoning an upload.
pub(crate) fn maintenance(access: &Arc<Authorization>, now: i64) -> Result<(),ToolError> {
    let mut control=access.admin.control.lock().unwrap();
    let root=control.writer.root().to_path_buf();
    let owners={
        let mut q=control.connection.prepare("SELECT owner_user_id FROM cloud_storage_adoptions WHERE maintained_at<=? ORDER BY maintained_at,owner_user_id LIMIT 16").map_err(storage)?;
        let rows=q.query_map([now-3600],|r|r.get::<_,String>(0)).map_err(storage)?.collect::<Result<Vec<_>,_>>().map_err(storage)?;
        rows
    };
    for owner in owners {
        let tx=control.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
        adopt(&tx,&root,&owner,now)?;
        cleanup(&tx,&root,&owner,now)?;
        tx.execute("UPDATE cloud_storage_adoptions SET maintained_at=? WHERE owner_user_id=?",params![now,owner]).map_err(storage)?;
        tx.commit().map_err(storage)?;
    }
    Ok(())
}

pub(crate) fn download(access: &Arc<Authorization>, owner: &str, id: &str) -> Result<(PathBuf,String),ToolError> {
    let control=access.admin.control.lock().unwrap();
    let item: Option<(String,String)>=control.connection.query_row("SELECT path,filename FROM cloud_storage_objects WHERE owner_user_id=? AND storage_id=? AND state='used'",
        params![owner,id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(storage)?;
    let (path,name)=item.ok_or_else(missing)?;
    if !Path::new(&path).is_file() { return Err(missing()); }
    Ok((path.into(),name))
}

#[cfg(test)]
#[path = "tests/cloud_storage_tests.rs"]
pub(crate) mod tests;
