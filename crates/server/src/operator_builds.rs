//! Operator projects coordinate existing Build Engine executions and immutable publication.
use crate::{
    admin_store::{self, AdminStore},
    external_build_costs,
    published_library::{PublishedBookRef, PublishedLibrary},
    user_storage_paths::error,
};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::PathBuf};

fn storage() -> ToolError {
    error(
        "OPERATOR_BUILD_STORAGE_FAILED",
        "unavailable",
        "Operator build could not be saved",
    )
}
fn conflict() -> ToolError {
    error(
        "OPERATOR_BUILD_CONFLICT",
        "conflict",
        "Refresh the task and use its current state and revision",
    )
}
fn invalid() -> ToolError {
    admin_store::invalid()
}
fn capabilities() -> Vec<String> {
    vec!["text".into(), "tutor".into()]
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MaterialInput {
    pub material_id: String,
    pub label: String,
    #[serde(default)]
    pub material_kind: Option<String>,
    #[serde(default)]
    pub build_tutor: bool,
    pub published_book_ref: Option<PublishedBookRef>,
    pub target_input: Option<String>,
    pub root_dir: Option<String>,
    pub pass2: Option<String>,
    pub max_parallel: Option<u32>,
    #[serde(default = "capabilities")]
    pub required_capabilities: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Create {
    pub task_id: String,
    pub name: String,
    pub max_micro_cny: i64,
    pub reserved_per_execution_micro_cny: i64,
    pub materials: Vec<MaterialInput>,
    pub visible_to_users: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Material {
    #[serde(flatten)]
    pub input: MaterialInput,
    pub state: String,
    pub publication_operation_id: String,
    pub plan: Option<Value>,
    pub invocation_ref: Option<String>,
    pub workspace_dir: Option<String>,
    pub observation: Option<Value>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Task {
    #[serde(default)] pub user_id: Option<String>,
    #[serde(default)] pub learning_workspace_id: Option<String>,
    #[serde(default)] pub confirmed_period_id: Option<String>,
    #[serde(default)] pub processing_confirmation: Option<String>,
    pub task_id: String,
    pub name: String,
    pub max_micro_cny: i64,
    pub reserved_per_execution_micro_cny: i64,
    pub materials: Vec<Material>,
    pub visible_to_users: Vec<String>,
    pub state: String,
    pub action: Option<String>,
    pub revision: u32,
    pub host_session_id: Option<String>,
    pub pause_requested: bool,
    pub confirmation: Option<String>,
    pub confirmed_at: Option<i64>,
    pub invocation_max_micro_cny: Option<i64>,
    pub created_by: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub error: Option<String>,
    pub release: Option<Value>,
}
#[derive(Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(crate) enum Mutation {
    Create(Create),
    Transition {
        task_id: String,
        command: String,
        expected_revision: u32,
        max_micro_cny: Option<i64>,
    },
    Publish {
        task_id: String,
        release: crate::workspace_publication::Publish,
    },
}
impl Mutation {
    pub(crate) fn normalize(&mut self) -> Result<(), ToolError> {
        match self {
            Self::Create(input) => {
                admin_store::operation_id(&input.task_id)?;
                if input.name.trim().is_empty()
                    || input.materials.is_empty()
                    || input.visible_to_users.is_empty()
                    || input.max_micro_cny <= 0
                    || input.reserved_per_execution_micro_cny <= 0
                    || input.reserved_per_execution_micro_cny > input.max_micro_cny
                {
                    return Err(invalid());
                }
                input.visible_to_users.sort();
                input.visible_to_users.dedup();
                let mut ids = BTreeSet::new();
                for user in &input.visible_to_users {
                    crate::user_storage_paths::validate_user_id(user)?;
                }
                for material in &mut input.materials {
                    if material.published_book_ref.is_none() && material.pass2.is_none() {
                        material.pass2 = Some("disabled".into());
                    }
                    let video_input = material.target_input.as_ref().is_some_and(|target| {
                        let target = PathBuf::from(target);
                        target.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("mp4"))
                            || target.join("video_import.json").is_file()
                            || target.join("video_source_manifest.json").is_file()
                    });
                    if material.material_kind.is_none() {
                        material.material_kind = Some(if video_input { "video" } else { "book" }.into());
                    }
                    if !matches!(material.material_kind.as_deref(), Some("book" | "video" | "music"))
                        || (video_input && material.material_kind.as_deref() != Some("video"))
                    {
                        return Err(invalid());
                    }
                    if material.material_kind.as_deref() == Some("video") {
                        // Source discourse describes the lecture, not optional Tutor assets.
                        if material.build_tutor {
                            return Err(error("OPERATOR_VIDEO_TUTOR_UNSUPPORTED", "validation",
                                "视频准备包含字幕、画面、知识关系和原讲解结构，尚不生成额外 Tutor 教学材料"));
                        }
                        if material.required_capabilities == capabilities() {
                            material.required_capabilities = ["text", "video", "translation", "visual_index", "graph", "discourse"]
                                .into_iter().map(String::from).collect();
                        }
                    }
                    if material.material_kind.as_deref() == Some("music") {
                        if material.build_tutor {
                            return Err(error("OPERATOR_MUSIC_TUTOR_UNSUPPORTED", "validation",
                                "音乐准备接纳精选谱本、录音、对齐与音乐事件；伴听教学由实际互动生成"));
                        }
                        if material.required_capabilities == capabilities() {
                            material.required_capabilities = ["music", "music_score", "music_events"]
                                .into_iter().map(String::from).collect();
                        }
                    }
                    admin_store::operation_id(&material.material_id)?;
                    if !ids.insert(&material.material_id)
                        || material.label.trim().is_empty()
                        || material.required_capabilities.is_empty()
                    {
                        return Err(invalid());
                    }
                    if material.published_book_ref.is_some() {
                        if material.target_input.is_some() || material.root_dir.is_some() {
                            return Err(invalid());
                        }
                    } else if !material
                        .target_input
                        .as_ref()
                        .is_some_and(|s| PathBuf::from(s).is_absolute())
                        || !material
                            .root_dir
                            .as_ref()
                            .is_some_and(|s| PathBuf::from(s).is_absolute())
                        || !matches!(material.pass2.as_deref(), Some("enabled" | "disabled"))
                        || !matches!(material.max_parallel, Some(1..=3))
                    {
                        return Err(invalid());
                    }
                }
            }
            Self::Transition {
                task_id, command, ..
            } => {
                admin_store::operation_id(task_id)?;
                if !matches!(command.as_str(), "prepare" | "start" | "continue" | "pause") {
                    return Err(invalid());
                }
            }
            Self::Publish { task_id, release } => {
                admin_store::operation_id(task_id)?;
                release.normalize()?;
            }
        }
        Ok(())
    }
    pub(crate) fn apply(
        &self,
        db: &Connection,
        actor: &str,
        operation: &str,
        now: i64,
    ) -> Result<Value, ToolError> {
        let task = match self {
            Self::Create(input) => {
                if db
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM operator_builds WHERE task_id=?)",
                        [&input.task_id],
                        |r| r.get::<_, bool>(0),
                    )
                    .map_err(|_| storage())?
                {
                    return Err(conflict());
                }
                for user in &input.visible_to_users {
                    if !db
                        .query_row(
                            "SELECT EXISTS(SELECT 1 FROM users WHERE user_id=? AND disabled=0)",
                            [user],
                            |r| r.get::<_, bool>(0),
                        )
                        .map_err(|_| storage())?
                    {
                        return Err(invalid());
                    }
                }
                for material in &input.materials {
                    if let Some(reference) = &material.published_book_ref {
                        let value = crate::workspace_publication::material_readiness(
                            db,
                            &crate::workspace_publication::Material {
                                published_book_ref: reference.clone(),
                                required_capabilities: material.required_capabilities.clone(),
                            },
                        )?;
                        if value["publication_available"] != true
                            || !value["missing_capabilities"]
                                .as_array()
                                .is_some_and(Vec::is_empty)
                        {
                            return Err(error(
                                "OPERATOR_MATERIAL_NOT_READY",
                                "conflict",
                                "Select a ready publication or prepare its source",
                            ));
                        }
                    }
                }
                let task = Task {
                    user_id: None, learning_workspace_id: None, confirmed_period_id: None,
                    task_id: input.task_id.clone(),
                    name: input.name.clone(),
                    max_micro_cny: input.max_micro_cny,
                    reserved_per_execution_micro_cny: input.reserved_per_execution_micro_cny,
                    materials: input
                        .materials
                        .iter()
                        .map(|input| Material {
                            state: if input.published_book_ref.is_some() {
                                "reused"
                            } else {
                                "pending"
                            }
                            .into(),
                            input: input.clone(),
                            publication_operation_id: uuid::Uuid::now_v7().to_string(),
                            plan: None,
                            invocation_ref: None,
                            workspace_dir: None,
                            observation: None,
                        })
                        .collect(),
                    visible_to_users: input.visible_to_users.clone(),
                    state: "draft".into(),
                    action: None,
                    revision: 0,
                    host_session_id: None,
                    pause_requested: false,
                    confirmation: None,
                    processing_confirmation: None,
                    confirmed_at: None,
                    invocation_max_micro_cny: None,
                    created_by: actor.into(),
                    created_at: now,
                    updated_at: now,
                    error: None,
                    release: None,
                };
                db.execute(
                    "INSERT INTO operator_builds VALUES(?,?,?)",
                    params![task.task_id, json!(task).to_string(), now],
                )
                .map_err(|_| storage())?;
                task
            }
            Self::Transition {
                task_id,
                command,
                expected_revision,
                max_micro_cny,
            } => {
                let mut task = read(db, task_id)?;
                if task.revision != *expected_revision {
                    return Err(conflict());
                }
                match command.as_str() {
                    "prepare" => {
                        if !matches!(task.state.as_str(), "draft" | "interrupted")
                            || task.confirmation.is_some()
                        {
                            return Err(conflict());
                        }
                        task.state = if task
                            .materials
                            .iter()
                            .all(|m| m.input.published_book_ref.is_some())
                        {
                            "awaiting_confirmation"
                        } else {
                            "queued_prepare"
                        }
                        .into();
                    }
                    "start" | "continue" => {
                        let direct_media_start = command == "start" && task.state == "draft"
                            && task.materials.iter().all(|m| m.input.published_book_ref.is_some()
                                || matches!(m.input.material_kind.as_deref(), Some("video" | "music")));
                        if !matches!(
                            (command.as_str(), task.state.as_str()),
                            ("start", "awaiting_confirmation") | ("continue", "interrupted")
                        ) && !direct_media_start {
                            return Err(conflict());
                        }
                        if task
                            .materials
                            .iter()
                            .any(|m| m.input.published_book_ref.is_none() && m.plan.is_none()
                                && !matches!(m.input.material_kind.as_deref(), Some("video" | "music")))
                        {
                            return Err(conflict());
                        }
                        let requested = max_micro_cny.unwrap_or(task.max_micro_cny);
                        if requested < task.max_micro_cny {
                            return Err(conflict());
                        }
                        let exists: bool = db
                            .query_row(
                                "SELECT EXISTS(SELECT 1 FROM build_task_budgets WHERE task_ref=?)",
                                [task_id],
                                |r| r.get(0),
                            )
                            .map_err(|_| storage())?;
                        let previous = if exists {
                            Some(external_build_costs::summary(db, task_id)?)
                        } else {
                            None
                        };
                        if !exists || requested > task.max_micro_cny {
                            external_build_costs::set_budget(
                                db,
                                actor,
                                task_id,
                                external_build_costs::Budget {
                                    cost_owner: if task.user_id.is_some() { "account" } else { "platform" }.into(),
                                    user_id: task.user_id.clone(),
                                    learning_workspace_id: task.learning_workspace_id.clone(),
                                    max_micro_cny: requested,
                                    confirmation: operation.into(),
                                    expected_revision: previous
                                        .as_ref()
                                        .and_then(|b| b["revision"].as_i64()),
                                },
                                now,
                            )?;
                        }
                        task.max_micro_cny = requested;
                        task.processing_confirmation = Some(operation.into());
                        if task.confirmation.is_none() {
                            task.confirmation = Some(operation.into());
                            task.confirmed_at = Some(now);
                            task.invocation_max_micro_cny = Some(requested);
                        }
                        // This operator confirmation explicitly grants the selected public outputs.
                        for material in &task.materials {
                            if let Some(reference) = &material.input.published_book_ref {
                                grant(db, &task.visible_to_users, reference)?;
                            }
                        }
                        task.state = if task
                            .materials
                            .iter()
                            .all(|m| m.input.published_book_ref.is_some())
                        {
                            "ready"
                        } else {
                            "queued"
                        }
                        .into();
                    }
                    "pause" => {
                        if !matches!(
                            task.state.as_str(),
                            "queued_prepare" | "preparing" | "queued" | "running"
                        ) {
                            return Err(conflict());
                        }
                        task.pause_requested = true;
                        if matches!(task.state.as_str(), "queued_prepare" | "queued") {
                            task.state = "interrupted".into();
                        }
                    }
                    _ => return Err(invalid()),
                }
                if command != "pause" {
                    task.host_session_id = None;
                    task.pause_requested = false;
                    task.error = None;
                }
                task.revision += 1;
                task.updated_at = now;
                attach_personal(db, &task, now)?;
                save(db, &task)?;
                task
            }
            Self::Publish { task_id, release } => {
                let mut task = read(db, task_id)?;
                if task.state != "ready" || task.user_id.is_some() {
                    return Err(conflict());
                }
                let outputs: BTreeSet<_> = task
                    .materials
                    .iter()
                    .filter_map(|m| m.input.published_book_ref.clone())
                    .collect();
                if release
                    .manifest
                    .materials
                    .iter()
                    .any(|m| !outputs.contains(&m.published_book_ref))
                {
                    return Err(invalid());
                }
                let result = release.apply(db, actor, operation, now)?;
                task.release = Some(
                    json!({"release_id":release.release_id,"version":release.expected_version+1}),
                );
                task.revision += 1;
                task.updated_at = now;
                save(db, &task)?;
                return Ok(
                    json!({"operator_build":view(db,&task)?,"workspace_release":result["workspace_release"]}),
                );
            }
        };
        Ok(json!({"operator_build":view(db,&task)?}))
    }
}
pub(crate) fn attach_personal(db: &Connection, task: &Task, now: i64) -> Result<(), ToolError> {
    if let (Some(owner), Some(workspace)) = (&task.user_id, &task.learning_workspace_id) {
        for material in &task.materials {
            if let Some(reference) = &material.input.published_book_ref {
                db.execute("INSERT OR IGNORE INTO learning_workspace_members(owner_user_id,learning_workspace_id,book_id,publication_id,added_at) VALUES(?,?,?,?,?)", params![owner,workspace,reference.book_id,reference.publication_id,now]).map_err(|_|storage())?;
                db.execute("INSERT OR IGNORE INTO workspace_release_instance_private_materials(owner_user_id,learning_workspace_id,book_id,publication_id) SELECT owner_user_id,learning_workspace_id,?,? FROM workspace_release_instances WHERE owner_user_id=? AND learning_workspace_id=?",params![reference.book_id,reference.publication_id,owner,workspace]).map_err(|_|storage())?;
            }
        }
    }
    Ok(())
}
fn grant(db: &Connection, users: &[String], reference: &PublishedBookRef) -> Result<(), ToolError> {
    for user in users {
        db.execute(
            "INSERT OR IGNORE INTO book_grants(owner_user_id,book_id,publication_id) VALUES(?,?,?)",
            params![user, reference.book_id, reference.publication_id],
        )
        .map_err(|_| storage())?;
    }
    Ok(())
}
pub(crate) fn read(db: &Connection, id: &str) -> Result<Task, ToolError> {
    let raw: Option<String> = db
        .query_row(
            "SELECT task_json FROM operator_builds WHERE task_id=?",
            [id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| storage())?;
    serde_json::from_str(&raw.ok_or_else(crate::authorization::missing)?).map_err(|_| storage())
}
pub(crate) fn save(db: &Connection, task: &Task) -> Result<(), ToolError> {
    db.execute(
        "UPDATE operator_builds SET task_json=? WHERE task_id=?",
        params![json!(task).to_string(), task.task_id],
    )
    .map_err(|_| storage())?;
    Ok(())
}
pub(crate) fn view(db: &Connection, task: &Task) -> Result<Value, ToolError> {
    let mut value = json!(task);
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM build_task_budgets WHERE task_ref=?)",
            [&task.task_id],
            |r| r.get(0),
        )
        .map_err(|_| storage())?;
    value["budget"] = if exists {
        external_build_costs::summary(db, &task.task_id)?
    } else {
        Value::Null
    };
    Ok(value)
}
fn host(task: &Task, session: &str) -> Result<(), ToolError> {
    if task.host_session_id.as_deref() != Some(session)
        || !matches!(task.state.as_str(), "preparing" | "running")
    {
        return Err(conflict());
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Progress {
    pub host_session_id: String,
    pub state: String,
    pub plan: Option<Value>,
    pub invocation_ref: Option<String>,
    pub workspace_dir: Option<String>,
    pub observation: Option<Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Import {
    pub host_session_id: String,
    pub invocation_ref: String,
    pub workspace_dir: String,
}
impl AdminStore {
    pub(crate) fn operator_build(&self, id: &str) -> Result<Value, ToolError> {
        let c = self.control.lock().unwrap();
        view(&c.connection, &read(&c.connection, id)?)
    }
    pub(crate) fn operator_builds(&self, limit: u32, offset: u32) -> Result<Value, ToolError> {
        let c = self.control.lock().unwrap();
        let db = &c.connection;
        let total: i64 = db
            .query_row("SELECT count(*) FROM operator_builds", [], |r| r.get(0))
            .map_err(|_| storage())?;
        let mut q=db.prepare("SELECT task_json FROM operator_builds ORDER BY created_at DESC,task_id LIMIT ? OFFSET ?").map_err(|_|storage())?;
        let rows = q
            .query_map(params![limit, offset], |r| r.get::<_, String>(0))
            .map_err(|_| storage())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| storage())?;
        let items = rows
            .iter()
            .map(|raw| {
                let task = serde_json::from_str(raw).map_err(|_| storage())?;
                view(db, &task)
            })
            .collect::<Result<Vec<_>, ToolError>>()?;
        Ok(json!({"items":items,"total":total,"limit":limit,"offset":offset}))
    }
    pub(crate) fn operator_next(&self, session: &str, now: i64) -> Result<Value, ToolError> {
        if session.trim().is_empty() {
            return Err(invalid());
        }
        let mut c = self.control.lock().unwrap();
        let tx = c.connection.transaction().map_err(|_| storage())?;
        let raw:Option<String>=tx.query_row("SELECT task_json FROM operator_builds WHERE json_extract(task_json,'$.state') IN ('queued_prepare','queued') OR (json_extract(task_json,'$.state') IN ('preparing','running') AND json_extract(task_json,'$.host_session_id')=?) ORDER BY CASE WHEN json_extract(task_json,'$.host_session_id')=? THEN 0 ELSE 1 END,created_at,task_id LIMIT 1",params![session,session],|r|r.get(0)).optional().map_err(|_|storage())?;
        let Some(raw) = raw else {
            return Ok(json!({"task":null}));
        };
        let mut task: Task = serde_json::from_str(&raw).map_err(|_| storage())?;
        if matches!(task.state.as_str(), "queued_prepare" | "queued") {
            let prepare = task.state == "queued_prepare";
            task.action = Some(if prepare { "prepare" } else { "run" }.into());
            task.state = if prepare { "preparing" } else { "running" }.into();
            task.host_session_id = Some(session.into());
            task.updated_at = now;
            task.revision += 1;
            save(&tx, &task)?;
        }
        let value = view(&tx, &task)?;
        tx.commit().map_err(|_| storage())?;
        Ok(json!({"task":value}))
    }
    pub(crate) fn operator_progress(
        &self,
        id: &str,
        material: &str,
        input: Progress,
        now: i64,
    ) -> Result<Value, ToolError> {
        let mut c = self.control.lock().unwrap();
        let tx = c.connection.transaction().map_err(|_| storage())?;
        let mut task = read(&tx, id)?;
        host(&task, &input.host_session_id)?;
        let m = task
            .materials
            .iter_mut()
            .find(|m| m.input.material_id == material)
            .ok_or_else(invalid)?;
        if m.input.published_book_ref.is_some() {
            return Err(conflict());
        }
        if !matches!(
            input.state.as_str(),
            "planned" | "running" | "interrupted" | "complete"
        ) {
            return Err(invalid());
        }
        if let Some(plan) = input.plan {
            let video_plan = m.input.material_kind.as_deref() == Some("video")
                && plan["version"] == "operator_video_plan.v1";
            let music_plan = m.input.material_kind.as_deref() == Some("music")
                && plan["version"] == "operator_music_plan.v1";
            let valid_plan = if video_plan || music_plan {
                matches!(task.state.as_str(), "preparing" | "running")
                    && plan["build_tutor"] == false
                    && plan["review_markdown"].as_str().is_some()
                    && plan["workspace_dir"].as_str().is_some_and(|p| PathBuf::from(p).is_absolute())
                    && (!music_plan || plan["target_input"].as_str() == m.input.target_input.as_deref())
            } else {
                !matches!(m.input.material_kind.as_deref(), Some("video" | "music"))
                    && task.state == "preparing"
                    && plan["plan"]["private_artifacts"]
                    .as_array()
                    .is_some_and(Vec::is_empty)
                    && plan["build_plan_path"].as_str().is_some()
                    && plan["workspace_dir"].as_str().is_some()
            };
            if !valid_plan {
                return Err(invalid());
            }
            if (video_plan || music_plan) && m.plan.as_ref().is_some_and(|old| old != &plan) {
                return Err(conflict());
            }
            m.plan = Some(plan);
        }
        if input.state == "planned" && m.plan.is_none() {
            return Err(invalid());
        }
        if let Some(reference) = input.invocation_ref {
            if task.state != "running"
                || m.invocation_ref
                    .as_ref()
                    .is_some_and(|old| old != &reference)
            {
                return Err(conflict());
            }
            m.invocation_ref = Some(reference);
        }
        if let Some(directory) = input.workspace_dir {
            if !PathBuf::from(&directory).is_absolute()
                || m.plan.as_ref().and_then(|p| p["workspace_dir"].as_str())
                    != Some(directory.as_str())
            {
                return Err(invalid());
            }
            m.workspace_dir = Some(directory);
        }
        m.state = input.state;
        m.observation = input.observation;
        task.revision += 1;
        task.updated_at = now;
        save(&tx, &task)?;
        let value = view(&tx, &task)?;
        tx.commit().map_err(|_| storage())?;
        Ok(value)
    }
    pub(crate) fn operator_finish(
        &self,
        id: &str,
        session: &str,
        state: &str,
        failure: Option<String>,
        now: i64,
    ) -> Result<Value, ToolError> {
        let mut c = self.control.lock().unwrap();
        let tx = c.connection.transaction().map_err(|_| storage())?;
        let mut task = read(&tx, id)?;
        host(&task, session)?;
        if !matches!(state, "awaiting_confirmation" | "ready" | "interrupted") {
            return Err(invalid());
        }
        if state == "awaiting_confirmation"
            && (task.state != "preparing"
                || task
                    .materials
                    .iter()
                    .any(|m| m.input.published_book_ref.is_none() && m.plan.is_none()))
        {
            return Err(conflict());
        }
        if state == "ready"
            && task
                .materials
                .iter()
                .any(|m| m.input.published_book_ref.is_none())
        {
            return Err(conflict());
        }
        task.state = state.into();
        task.error = failure;
        task.revision += 1;
        task.updated_at = now;
        save(&tx, &task)?;
        let value = view(&tx, &task)?;
        tx.commit().map_err(|_| storage())?;
        Ok(value)
    }
    pub(crate) fn operator_import(
        &self,
        id: &str,
        material: &str,
        input: Import,
        now: i64,
    ) -> Result<Value, ToolError> {
        let (task, writer) = {
            let c = self.control.lock().unwrap();
            (read(&c.connection, id)?, c.writer.clone())
        };
        host(&task, &input.host_session_id)?;
        let m = task
            .materials
            .iter()
            .find(|m| m.input.material_id == material)
            .ok_or_else(invalid)?;
        if m.invocation_ref.as_deref() != Some(&input.invocation_ref)
            || m.workspace_dir.as_deref() != Some(&input.workspace_dir)
            || !matches!(m.state.as_str(), "complete" | "imported")
        {
            return Err(conflict());
        }
        if m.state == "imported" {
            return self.operator_build(id);
        }
        // Reuse the existing idempotent importer on its own connection, outside the admin mutex.
        let mut library = PublishedLibrary::new(crate::control_store::ControlStore::open(writer)?);
        let manifest = library.publish_online(&crate::online_publication::PublicationRequest {
            operation_id: m.publication_operation_id.clone(),
            input_directory: PathBuf::from(&input.workspace_dir),
            grant_users: task.visible_to_users.clone(),
        })?;
        let reference = manifest.reference;
        let mut c = self.control.lock().unwrap();
        let tx = c.connection.transaction().map_err(|_| storage())?;
        let mut task = read(&tx, id)?;
        host(&task, &input.host_session_id)?;
        let m = task
            .materials
            .iter_mut()
            .find(|m| m.input.material_id == material)
            .ok_or_else(invalid)?;
        m.input.published_book_ref = Some(reference.clone());
        m.state = "imported".into();
        attach_personal(&tx, &task, now)?;
        task.revision += 1;
        task.updated_at = now;
        save(&tx, &task)?;
        let value = view(&tx, &task)?;
        tx.commit().map_err(|_| storage())?;
        Ok(value)
    }
}

pub(crate) fn dispatch(
    access: &std::sync::Arc<crate::authorization::Authorization>,
    actor: &str,
    method: &str,
    parts: &[&str],
    query: &std::collections::HashMap<String, String>,
    input: Value,
    now: i64,
) -> Result<Value, ToolError> {
    fn parse<T: serde::de::DeserializeOwned>(input: Value) -> Result<T, ToolError> {
        serde_json::from_value(input).map_err(|_| invalid())
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Host {
        host_session_id: String,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Finish {
        host_session_id: String,
        state: String,
        error: Option<String>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Transition {
        expected_revision: u32,
        max_micro_cny: Option<i64>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Publish {
        release: crate::workspace_publication::Publish,
    }
    if method == "GET" && parts.is_empty() {
        let (limit, offset) = crate::admin_api::page(query)?;
        return access.admin.operator_builds(limit, offset);
    }
    if !query.is_empty() {
        return Err(invalid());
    }
    match (method, parts) {
        ("GET", [id]) => return access.admin.operator_build(id),
        ("POST", ["next"]) => {
            let host: Host = parse(input)?;
            return access.admin.operator_next(&host.host_session_id, now);
        }
        ("POST", [id, "materials", material, "progress"]) => {
            return access
                .admin
                .operator_progress(id, material, parse(input)?, now)
        }
        ("POST", [id, "materials", material, "import"]) => {
            return access
                .admin
                .operator_import(id, material, parse(input)?, now)
        }
        ("POST", [id, "finish"]) => {
            let finish: Finish = parse(input)?;
            return access.admin.operator_finish(
                id,
                &finish.host_session_id,
                &finish.state,
                finish.error,
                now,
            );
        }
        _ => {}
    }
    if method != "POST" {
        return Err(invalid());
    }
    let mut input = input;
    let operation = input
        .as_object_mut()
        .and_then(|o| o.remove("operation_id"))
        .and_then(|v| v.as_str().map(str::to_owned))
        .ok_or_else(invalid)?;
    let command = match parts {
        [] => Mutation::Create(parse(input)?),
        [id, "publish"] => {
            let body: Publish = parse(input)?;
            Mutation::Publish {
                task_id: (*id).into(),
                release: body.release,
            }
        }
        [id, command] => {
            let body: Transition = parse(input)?;
            Mutation::Transition {
                task_id: (*id).into(),
                command: (*command).into(),
                expected_revision: body.expected_revision,
                max_micro_cny: body.max_micro_cny,
            }
        }
        _ => return Err(invalid()),
    };
    access
        .admin
        .apply(
            actor,
            &operation,
            actor,
            admin_store::Command::OperatorBuild(command),
            now,
        )
        .map(|(result, _)| result)
}
