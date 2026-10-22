//! Personal preparation uses the operator queue, original workspace and shared cost ledger.
use crate::{admin_store::{invalid, operation_id}, operator_builds as builds, authorization::{Authorization, missing}, user_storage_paths::error};
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::{sync::Arc, fs, io::{Read, Seek, SeekFrom, Write}, path::Path};
use base64::Engine;

fn storage<T>(_: T) -> ToolError { error("PREPARATION_STORAGE_FAILED", "unavailable", "资料准备暂时不可用") }
fn conflict(message: &str) -> ToolError { error("PREPARATION_CONFIRMATION_REQUIRED", "conflict", message) }
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, ToolError> { value[key].as_str().filter(|s| !s.is_empty()).ok_or_else(invalid) }
fn supported_formats() -> Vec<&'static str> {
    let mut formats = vec!["epub","md","txt","mp4","pdf"];
    if std::env::var_os("UNDERSTAND_BOOK_LIBREOFFICE").is_some_and(|file| Path::new(&file).is_absolute() && Path::new(&file).is_file()) {
        formats.extend(["pptx","ppt"]);
    }
    formats
}

fn upload(db: &mut rusqlite::Connection, service_root: &Path, root: &Path, owner: &str, workspace: &str, input: &Value, now: i64) -> Result<Value, ToolError> {
    let id = text(input, "upload_id")?; operation_id(id)?;
    let name = text(input,"filename")?;
    let extension = Path::new(name).extension().and_then(|s|s.to_str()).unwrap_or("").to_ascii_lowercase();
    if Path::new(name).file_name().and_then(|s|s.to_str()) != Some(name) { return Err(invalid()); }
    if !supported_formats().contains(&extension.as_str()) {
        return Err(error("PREPARATION_FORMAT_UNAVAILABLE", "invalid_argument", "当前服务器不支持这类资料；PPT/PPTX 未启用时请先导出为带文字层的 PDF。"));
    }
    let total = input["total"].as_u64().filter(|n| *n > 0).ok_or_else(invalid)?;
    let offset = input["offset"].as_u64().ok_or_else(invalid)?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(text(input,"chunk_base64")?).map_err(|_|invalid())?;
    if bytes.is_empty() || bytes.len() > 32 * 1024 || offset.checked_add(bytes.len() as u64).is_none_or(|n| n > total) { return Err(invalid()); }
    let directory = root.join(id);
    let meta = directory.join("upload.json");
    let metadata = if meta.exists() {
        let old: Value = serde_json::from_slice(&fs::read(&meta).map_err(storage)?).map_err(storage)?;
        if old["filename"] != name || old["total"] != total || old["extension"] != extension { return Err(conflict("该上传已绑定另一份资料")); }
        old
    } else {
        // The Build Engine derives the source identity from this filename.
        // Stable upload IDs keep two students' documents distinct without hashes.
        json!({"filename":name,"total":total,"extension":extension,"source_file":format!("source-{id}.{extension}")})
    };
    let source = directory.join(metadata["source_file"].as_str().map(str::to_owned).unwrap_or_else(||format!("source.{extension}")));
    // Reserve before creating files. A write failure keeps this reservation for
    // the same upload to resume, or for explicit cancellation/idle cleanup.
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(storage)?;
    crate::cloud_storage::adopt(&tx,service_root,owner,now)?;
    crate::cloud_storage::cleanup(&tx,service_root,owner,now)?;
    crate::cloud_storage::reserve(&tx,owner,workspace,id,name,&source,i64::try_from(total).map_err(|_|invalid())?,now)?;
    tx.commit().map_err(storage)?;
    fs::create_dir_all(&directory).map_err(storage)?;
    if !meta.exists() { fs::write(&meta, metadata.to_string()).map_err(storage)?; }
    let mut file = fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).open(&source).map_err(storage)?;
    let length = file.metadata().map_err(storage)?.len();
    if offset < length {
        let overlap=usize::try_from((length-offset).min(bytes.len() as u64)).map_err(storage)?;
        let mut saved = vec![0;overlap]; file.seek(SeekFrom::Start(offset)).map_err(storage)?;
        file.read_exact(&mut saved).map_err(storage)?;
        if saved != bytes[..overlap] { return Err(conflict("上传内容不一致，请重新选择资料")); }
        if overlap < bytes.len() {
            file.seek(SeekFrom::End(0)).map_err(storage)?;
            file.write_all(&bytes[overlap..]).map_err(storage)?;
            file.sync_data().map_err(storage)?;
        }
    } else {
        if offset != length { return Err(conflict("请从上次完成的位置继续上传")); }
        file.seek(SeekFrom::End(0)).map_err(storage)?; file.write_all(&bytes).map_err(storage)?; file.sync_data().map_err(storage)?;
    }
    let received = file.metadata().map_err(storage)?.len();
    if received == total { crate::cloud_storage::uploaded(db,owner,id,now)?; }
    Ok(json!({"upload_id":id,"received":received,"total":total,"complete":received==total}))
}

pub(crate) fn request(access: &Arc<Authorization>, owner: &str, workspace: &str, method: &str, input: Value, now: i64) -> Result<Value, ToolError> {
    let mut c = access.admin.control.lock().unwrap();
    let root = c.writer.root().join("personal-preparations").join(owner).join(workspace);
    let service_root = c.writer.root().to_path_buf();
    if method == "GET" {
        let mut q = c.connection.prepare("SELECT task_json FROM operator_builds WHERE json_extract(task_json,'$.user_id')=? AND json_extract(task_json,'$.learning_workspace_id')=? AND json_extract(task_json,'$.state')<>'deleted' ORDER BY created_at DESC,task_id").map_err(storage)?;
        let tasks = q.query_map(params![owner,workspace], |r|r.get::<_,String>(0)).map_err(storage)?.collect::<Result<Vec<_>,_>>().map_err(storage)?;
        let items = tasks.iter().map(|raw| { let task = serde_json::from_str(raw).map_err(storage)?; view(&c.connection, &task, now, &access.spend) }).collect::<Result<Vec<_>,ToolError>>()?;
        return Ok(json!({"items":items,"supported_formats":supported_formats()}));
    }
    if method != "POST" { return Err(invalid()); }
    let active: bool = c.connection.query_row("SELECT EXISTS(SELECT 1 FROM learning_workspaces WHERE owner_user_id=? AND learning_workspace_id=? AND archived=0)", params![owner,workspace], |r|r.get(0)).map_err(storage)?;
    if !active { return Err(missing()); }
    let action = text(&input,"action")?;
    if action == "upload" {
        return upload(&mut c.connection,&service_root,&root.join("uploads"),owner,workspace,&input,now);
    }
    let id = text(&input,"task_id")?; operation_id(id)?;
    let operation = text(&input,"operation_id")?; operation_id(operation)?;
    let tx = c.connection.transaction().map_err(storage)?;
    crate::cloud_storage::adopt(&tx,&service_root,owner,now)?;
    let prior: Option<(String,String)> = tx.query_row("SELECT parameters_json,result_json FROM admin_operations WHERE actor=? AND operation_id=?",params![owner,operation],|r| Ok((r.get(0)?,r.get(1)?))).optional().map_err(storage)?;
    let parameters = json!({"workspace":workspace,"input":input}).to_string();
    if let Some((request,result)) = prior { return if request == parameters { serde_json::from_str(&result).map_err(storage) } else { Err(conflict("操作标识已经使用")) }; }
    let task = if action == "create" {
        let reference: Option<base_schema::PublishedBookRef> = input.get("published_book_ref").cloned().map(serde_json::from_value).transpose().map_err(|_|invalid())?;
        let (label, target, kind) = if let Some(reference) = &reference {
            crate::learning_workspace::authorize_material(&tx, owner, reference)?;
            let publication=access.library.lock().unwrap().load(owner,reference)?;
            (text(&input,"name")?.to_owned(),None,if publication.directory().join("video_source_manifest.json").is_file(){"video"}else{"book"})
        } else {
            let upload = text(&input,"upload_id")?; operation_id(upload)?;
            let directory = root.join("uploads").join(upload);
            let meta: Value = serde_json::from_slice(&fs::read(directory.join("upload.json")).map_err(storage)?).map_err(storage)?;
            let target = directory.join(meta["source_file"].as_str().map(str::to_owned).unwrap_or_else(||format!("source.{}",meta["extension"].as_str().unwrap_or(""))));
            if fs::metadata(&target).map_err(storage)?.len() != meta["total"].as_u64().ok_or_else(invalid)? { return Err(conflict("资料尚未上传完整")); }
            // One uploaded source has one durable build; retry or goal edits reuse it.
            let existing: Option<String> = tx.query_row("SELECT task_id FROM operator_builds WHERE json_extract(task_json,'$.user_id')=? AND json_extract(task_json,'$.learning_workspace_id')=? AND json_extract(task_json,'$.materials[0].target_input')=?",params![owner,workspace,target.to_string_lossy()],|r|r.get(0)).optional().map_err(storage)?;
            if let Some(existing) = existing { return view(&tx, &builds::read(&tx,&existing)?, now, &access.spend); }
            (text(&meta,"filename")?.to_owned(),Some(target.to_string_lossy().to_string()),if meta["extension"] == "mp4" {"video"} else {"book"})
        };
        fs::create_dir_all(root.join("builds")).map_err(storage)?;
        let mut mutation = builds::Mutation::Create(builds::Create {task_id:id.into(),name:label.clone(),max_micro_cny:1,reserved_per_execution_micro_cny:1,visible_to_users:vec![owner.into()],materials:vec![builds::MaterialInput {
            material_id:"source".into(),label,material_kind:Some(kind.into()),build_tutor:false,published_book_ref:reference,target_input:target,root_dir:Some(root.join("builds").join(id).to_string_lossy().to_string()),pass2:Some("disabled".into()),max_parallel:Some(1),required_capabilities:if kind=="video" {vec!["text".into(),"video".into()]} else {vec!["text".into()]},
        }]});
        // Reused sources do not need a build directory.
        if let builds::Mutation::Create(create) = &mut mutation { if create.materials[0].published_book_ref.is_some() { create.materials[0].root_dir=None; create.materials[0].pass2=None; create.materials[0].max_parallel=None; } }
        mutation.normalize()?; mutation.apply(&tx,owner,operation,now)?;
        let mut task = builds::read(&tx,id)?; task.user_id=Some(owner.into()); task.learning_workspace_id=Some(workspace.into());
        if task.materials.iter().all(|m|m.input.published_book_ref.is_some()) { task.state="ready".into(); builds::attach_personal(&tx,&task,now)?; }
        builds::save(&tx,&task)?;
        if let Some(upload)=input["upload_id"].as_str() { crate::cloud_storage::bind_task(&tx,owner,upload,id,now)?; }
        task
    } else {
        let mut task = builds::read(&tx,id)?;
        if task.user_id.as_deref()!=Some(owner) || task.learning_workspace_id.as_deref()!=Some(workspace) { return Err(missing()); }
        if task.state=="deleted" { return Err(missing()); }
        let revision = input["expected_revision"].as_u64().and_then(|v|u32::try_from(v).ok()).ok_or_else(invalid)?;
        if revision != task.revision { return Err(conflict("资料状态已更新，请刷新后继续")); }
        let maximum = if matches!(action,"start"|"continue") {
            let (period,_) = current_period(&tx,owner,now)?;
            if input["period_id"] != period { return Err(conflict("额度有效期已变化，请确认当前额度后继续")); }
            let basis = crate::account_allowance::basis(&tx,&period)?.ok_or_else(invalid)?;
            let amount = basis.grant(input["max_milli_percent"].as_i64().filter(|v|*v>0).ok_or_else(invalid)?)?.value();
            if crate::account_allowance::balance(&tx,&period)?.available_micro_cny.value() <= 0 { return Err(conflict("当前额度不足，请补充后继续")); }
            for material in &task.materials {
                if material.input.published_book_ref.is_none() && material.input.material_kind.as_deref()==Some("video") {
                    crate::service_usage::rate(&service_root,"video_transcription",now)?;
                }
                if material.input.published_book_ref.is_none() && material.input.material_kind.as_deref()!=Some("video") {
                    let runtime = &material.plan.as_ref().ok_or_else(invalid)?["model_runtime"];
                    spend_rate(&access.spend, runtime, now)?;
                }
            }
            task.confirmed_period_id=Some(period); builds::save(&tx,&task)?; Some(amount)
        } else { None };
        if !matches!(action,"prepare"|"start"|"continue"|"pause") { return Err(invalid()); }
        let mutation = builds::Mutation::Transition {task_id:id.into(),command:action.into(),expected_revision:revision,max_micro_cny:maximum};
        mutation.apply(&tx,owner,operation,now)?; builds::read(&tx,id)?
    };
    let result = view(&tx,&task,now,&access.spend)?;
    tx.execute("INSERT INTO admin_operations(actor,operation_id,user_id,kind,parameters_json,result_json,created_at) VALUES(?,?,?,'personal_preparation',?,?,?)",params![owner,operation,owner,parameters,result.to_string(),now]).map_err(storage)?;
    tx.commit().map_err(storage)?; Ok(result)
}

fn current_period(db: &rusqlite::Connection, owner: &str, now: i64) -> Result<(String,i64),ToolError> {
    db.query_row("SELECT period_id,expires_at FROM allowance_periods WHERE user_id=? AND closed_at IS NULL AND starts_at<=? AND ?<expires_at",params![owner,now,now],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(storage)?.ok_or_else(||conflict("当前额度期不可用，请补充额度后继续"))
}
fn view(db: &rusqlite::Connection, task: &builds::Task, now: i64, spend: &crate::model_spend_store::ModelSpendStore) -> Result<Value,ToolError> {
    let period = current_period(db,task.user_id.as_deref().ok_or_else(invalid)?,now).ok();
    let allowance = period.map(|(id,expires)| -> Result<Value,ToolError> {
        let basis = crate::account_allowance::basis(db,&id)?;
        let balance = crate::account_allowance::balance(db,&id)?;
        let budget = builds::view(db,task)?["budget"].clone();
        Ok(json!({"period_id":id,"expires_at":expires,"available":balance.percentages.as_ref().and_then(|v|v.get("available")),
            "maximum":basis.as_ref().map(|b|b.percentage(task.max_micro_cny)),"remaining":basis.as_ref().and_then(|b|budget["remaining_micro_cny"].as_i64().map(|v|b.percentage(v)))}))
    }).transpose()?;
    let mut q=db.prepare("SELECT period_id,COALESCE(sum(account_debit_micro_cny),0),COALESCE(sum(CASE WHEN state='pending' THEN reserved_micro_cny ELSE 0 END),0),COALESCE(sum(CASE WHEN state IN ('reserved','sent') THEN reserved_micro_cny ELSE 0 END),0) FROM billable_usage WHERE parent_task_ref=? AND user_id=? GROUP BY period_id ORDER BY period_id").map_err(storage)?;
    let groups=q.query_map(params![task.task_id,task.user_id], |r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?))).map_err(storage)?.collect::<Result<Vec<_>,_>>().map_err(storage)?;
    let usage=groups.into_iter().map(|(period,used,pending,reserved)| -> Result<Value,ToolError> {
        let basis=crate::account_allowance::basis(db,&period)?;
        let expires:i64=db.query_row("SELECT expires_at FROM allowance_periods WHERE period_id=?",[&period],|r|r.get(0)).map_err(storage)?;
        Ok(json!({"period_id":period,"expires_at":expires,"used":basis.as_ref().map(|b|b.percentage(used)),"pending":basis.as_ref().map(|b|b.percentage(pending)),"reserved":basis.as_ref().map(|b|b.percentage(reserved))}))
    }).collect::<Result<Vec<_>,_>>()?;
    let mut result = json!({"usage_periods":usage,"task_id":task.task_id,"name":task.name,"learning_workspace_id":task.learning_workspace_id,"state":task.state,"revision":task.revision,"error":task.error,"pause_requested":task.pause_requested,
        "allowance":allowance,"estimate":{"status":if task.materials.iter().all(|m|m.input.published_book_ref.is_some()) {"no_preparation_cost"} else {"variable"},"message":if task.materials[0].input.material_kind.as_deref()==Some("video") {"转写按实际处理音频时长扣减服务额度，与模型调用共用本次确认的总上限；已完成片段继续复用。"} else {"准备成本随资料长度和实际模型调用变化，按逐次用量结算，最多扣减已确认的上限。"}},
        "materials":task.materials.iter().map(|m|json!({"label":m.input.label,"kind":m.input.material_kind,"state":m.state,"published_book_ref":m.input.published_book_ref,"progress":m.observation.as_ref().and_then(|o|o.get("progress")),"plan_ready":m.plan.is_some()})).collect::<Vec<_>>() });
    if let Some(plan) = &task.materials[0].plan {
        if let (Ok(rate), Some(period)) = (spend_rate(spend,&plan["model_runtime"],now),result["allowance"]["period_id"].as_str()) {
            if let Some(basis) = crate::account_allowance::basis(db,period)? {
                let cost = |input:u64, output:u64| rate.cost(&runtime::provider_stream::ModelUsage { input_tokens:u32::try_from(input).ok(),output_tokens:u32::try_from(output).ok(),cached_input_tokens:Some(0),..Default::default() }).ok().map(|v|basis.percentage(v.value()));
                let estimate = &plan["plan"]["estimate"];
                result["estimate"]["lower"] = json!(cost(estimate["input_tokens"]["lower"].as_u64().unwrap_or(0),estimate["output_tokens"]["lower"].as_u64().unwrap_or(0)));
                result["estimate"]["upper"] = json!(cost(estimate["input_tokens"]["upper"].as_u64().unwrap_or(0),estimate["output_tokens"]["upper"].as_u64().unwrap_or(0)));
                result["estimate"]["partial"] = json!(estimate["unknown_stages"].as_array().is_none_or(|a|!a.is_empty()));
                result["estimate"]["per_call_maximum"] = json!(cost(plan["model_runtime"]["context_window_tokens"].as_u64().unwrap_or(0),plan["model_runtime"]["max_output_tokens"].as_u64().unwrap_or(0)));
                result["estimate"]["status"] = json!("estimated");
            }
        }
    }
    Ok(result)
}
fn spend_rate(spend: &crate::model_spend_store::ModelSpendStore, runtime: &Value, now: i64) -> Result<crate::model_rates::ModelRateSnapshot,ToolError> {
    spend.rates.as_ref().ok().and_then(|rates|rates.select(runtime["provider"].as_str()?,runtime["model"].as_str()?,now).ok()).ok_or_else(||error("MODEL_RATE_UNAVAILABLE","conflict","当前执行模型尚未配置可用费率，暂不能启动个人付费准备"))
}
