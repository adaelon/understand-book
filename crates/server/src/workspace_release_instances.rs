//! Private use of public compositions. Updating the reference never rewrites learning evidence.
use crate::{
    control_store::ControlStore,
    learning_workspace::{authorize_material, insert_goal, require_active, validate_goal},
    user_storage_paths::error,
    workspace_publication::{self, Manifest},
};
use base_schema::{PublishedBookRef, WorkspaceGoalInput};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn storage() -> ToolError {
    error(
        "WORKSPACE_INSTANCE_STORAGE_FAILED",
        "unavailable",
        "Shared workspace use could not be read or committed",
    )
}
fn conflict() -> ToolError {
    error(
        "WORKSPACE_INSTANCE_VERSION_CONFLICT",
        "conflict",
        "Review the current selected version before updating",
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UseRelease {
    pub operation_id: String,
    pub goal: WorkspaceGoalInput,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SelectVersion {
    pub operation_id: String,
    pub expected_version: u32,
    pub version: u32,
}

fn replay(
    db: &Connection,
    owner: &str,
    operation: &str,
    parameters: &Value,
) -> Result<Option<String>, ToolError> {
    let old: Option<(String, String)> = db.query_row(
        "SELECT parameters_json,learning_workspace_id FROM workspace_release_instance_operations WHERE owner_user_id=? AND operation_id=?",
        params![owner,operation], |r| Ok((r.get(0)?,r.get(1)?)),
    ).optional().map_err(|_|storage())?;
    if let Some((saved, workspace)) = old {
        if serde_json::from_str::<Value>(&saved).map_err(|_| storage())? != *parameters {
            return Err(error(
                "WORKSPACE_INSTANCE_OPERATION_CONFLICT",
                "conflict",
                "Operation ID has different business parameters",
            ));
        }
        return Ok(Some(workspace));
    }
    Ok(None)
}
fn save_operation(
    db: &Connection,
    owner: &str,
    operation: &str,
    parameters: &Value,
    workspace: &str,
) -> Result<(), ToolError> {
    db.execute("INSERT INTO workspace_release_instance_operations(owner_user_id,operation_id,parameters_json,learning_workspace_id) VALUES(?,?,?,?)",params![owner,operation,parameters.to_string(),workspace]).map_err(|_|storage())?;
    Ok(())
}
fn ready_release(db: &Connection, owner: &str, id: &str, version: u32) -> Result<Value, ToolError> {
    let release = workspace_publication::detail(db, Some(owner), id, version, false)?;
    if release["availability"]["status"] != "ready" {
        return Err(error(
            "WORKSPACE_RELEASE_NOT_READY",
            "conflict",
            "The selected materials, capabilities and grants must be ready before use",
        ));
    }
    Ok(release)
}
fn reference(
    db: &Connection,
    owner: &str,
    workspace: &str,
) -> Result<Option<(String, u32)>, ToolError> {
    db.query_row("SELECT release_id,version FROM workspace_release_instances WHERE owner_user_id=? AND learning_workspace_id=?",params![owner,workspace],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|_|storage())
}
fn refs(release: &Value) -> Result<BTreeSet<PublishedBookRef>, ToolError> {
    let manifest: Manifest =
        serde_json::from_value(release["manifest"].clone()).map_err(|_| storage())?;
    Ok(manifest
        .materials
        .into_iter()
        .map(|m| m.published_book_ref)
        .collect())
}
fn private_members(
    db: &Connection,
    owner: &str,
    workspace: &str,
) -> Result<BTreeSet<PublishedBookRef>, ToolError> {
    let mut query = db.prepare("SELECT book_id,publication_id FROM workspace_release_instance_private_materials WHERE owner_user_id=? AND learning_workspace_id=?").map_err(|_|storage())?;
    let result = query
        .query_map(params![owner, workspace], |r| {
            Ok(PublishedBookRef {
                book_id: r.get(0)?,
                publication_id: r.get(1)?,
            })
        })
        .map_err(|_| storage())?
        .collect::<Result<_, _>>()
        .map_err(|_| storage());
    result
}
fn instance(db: &Connection, owner: &str, workspace: &str) -> Result<Option<Value>, ToolError> {
    let Some((id, version)) = reference(db, owner, workspace)? else {
        return Ok(None);
    };
    let release = workspace_publication::detail(db, Some(owner), &id, version, false)?;
    let latest:u32 = db.query_row("SELECT MAX(version) FROM workspace_releases WHERE release_id=?1 AND EXISTS(SELECT 1 FROM json_each(visible_to_users_json) WHERE value=?2)",params![id,owner],|r|r.get(0)).map_err(|_|storage())?;
    let mut query = db.prepare("SELECT version,selected_at FROM workspace_release_instance_history WHERE owner_user_id=? AND learning_workspace_id=? ORDER BY version").map_err(|_|storage())?;
    let history: Vec<Value> = query
        .query_map(params![owner, workspace], |r| {
            Ok(json!({"version":r.get::<_,u32>(0)?,"selected_at":r.get::<_,i64>(1)?}))
        })
        .map_err(|_| storage())?
        .collect::<Result<_, _>>()
        .map_err(|_| storage())?;
    Ok(Some(
        json!({"release_id":id,"version":version,"latest_version":latest,"release":release,"history":history}),
    ))
}

impl ControlStore {
    fn workspace_instance_result(&self, owner: &str, workspace: &str) -> Result<Value, ToolError> {
        Ok(
            json!({"workspace":self.learning_workspace(owner,workspace)?,"instance":instance(&self.connection,owner,workspace)?}),
        )
    }
    pub(crate) fn use_workspace_release(
        &mut self,
        owner: &str,
        id: &str,
        version: u32,
        input: UseRelease,
        now: i64,
    ) -> Result<Value, ToolError> {
        self.user_paths(owner)?;
        crate::admin_store::operation_id(&input.operation_id)?;
        validate_goal(&input.goal)?;
        let parameters = json!({"kind":"use","release_id":id,"version":version,"goal":input.goal});
        let tx = self.connection.transaction().map_err(|_| storage())?;
        if let Some(workspace) = replay(&tx, owner, &input.operation_id, &parameters)? {
            drop(tx);
            return self.workspace_instance_result(owner, &workspace);
        }
        let release = ready_release(&tx, owner, id, version)?;
        if release["archived"] == true {
            return Err(error(
                "WORKSPACE_RELEASE_ARCHIVED",
                "conflict",
                "This shared workspace no longer accepts new use",
            ));
        }
        let materials = refs(&release)?;
        for material in &materials {
            authorize_material(&tx, owner, material)?;
        }
        let workspace = uuid::Uuid::now_v7().to_string();
        tx.execute("INSERT INTO learning_workspaces(owner_user_id,learning_workspace_id,created_at,updated_at) VALUES(?,?,?,?)",params![owner,workspace,now,now]).map_err(|_|storage())?;
        insert_goal(&tx, owner, &workspace, 1, &input.goal, now)?;
        for material in &materials {
            tx.execute("INSERT INTO learning_workspace_members(owner_user_id,learning_workspace_id,book_id,publication_id,added_at) VALUES(?,?,?,?,?)",params![owner,workspace,material.book_id,material.publication_id,now]).map_err(|_|storage())?;
        }
        tx.execute("INSERT INTO workspace_release_instances(owner_user_id,learning_workspace_id,release_id,version) VALUES(?,?,?,?)",params![owner,workspace,id,version]).map_err(|_|storage())?;
        tx.execute("INSERT INTO workspace_release_instance_history(owner_user_id,learning_workspace_id,version,selected_at) VALUES(?,?,?,?)",params![owner,workspace,version,now]).map_err(|_|storage())?;
        save_operation(&tx, owner, &input.operation_id, &parameters, &workspace)?;
        tx.commit().map_err(|_| storage())?;
        self.workspace_instance_result(owner, &workspace)
    }
    pub(crate) fn workspace_release_instance(
        &self,
        owner: &str,
        workspace: &str,
    ) -> Result<Value, ToolError> {
        self.learning_workspace(owner, workspace)?;
        Ok(json!({"instance":instance(&self.connection,owner,workspace)?}))
    }
    pub(crate) fn preview_workspace_release_update(
        &self,
        owner: &str,
        workspace: &str,
        version: u32,
    ) -> Result<Value, ToolError> {
        self.learning_workspace(owner, workspace)?;
        let current = instance(&self.connection, owner, workspace)?
            .ok_or_else(crate::authorization::missing)?;
        let selected = current["version"].as_u64().ok_or_else(storage)? as u32;
        if version <= selected {
            return Err(conflict());
        }
        let id = current["release_id"].as_str().ok_or_else(storage)?;
        let release =
            workspace_publication::detail(&self.connection, Some(owner), id, version, false)?;
        let old = refs(&current["release"])?;
        let new = refs(&release)?;
        let preserved: Vec<_> = private_members(&self.connection, owner, workspace)?
            .into_iter()
            .collect();
        let added: Vec<_> = new.difference(&old).cloned().collect();
        let removed: Vec<_> = old.difference(&new).cloned().collect();
        let previous = &current["release"]["manifest"];
        let next = &release["manifest"];
        Ok(json!({"instance":current,"release":release,"changes":{
            "added_materials":added,"removed_materials":removed,
            "public_goal_changed":previous["public_goal"] != next["public_goal"],
            "composition_changed":previous["topic_links"] != next["topic_links"] || previous["suggested_route"] != next["suggested_route"],
            "required_capabilities_changed":previous["materials"] != next["materials"],
            "description_changed":previous["name"] != next["name"] || previous["publisher"] != next["publisher"] || previous["applicability"] != next["applicability"] || previous["prerequisites"] != next["prerequisites"]
        },"preserved_private_materials":preserved}))
    }
    pub(crate) fn update_workspace_release_instance(
        &mut self,
        owner: &str,
        workspace: &str,
        input: SelectVersion,
        now: i64,
    ) -> Result<Value, ToolError> {
        self.user_paths(owner)?;
        crate::admin_store::operation_id(&input.operation_id)?;
        let parameters = json!({"kind":"update","learning_workspace_id":workspace,"expected_version":input.expected_version,"version":input.version});
        let tx = self.connection.transaction().map_err(|_| storage())?;
        if let Some(saved) = replay(&tx, owner, &input.operation_id, &parameters)? {
            drop(tx);
            return self.workspace_instance_result(owner, &saved);
        }
        require_active(&tx, owner, workspace)?;
        let (id, current) =
            reference(&tx, owner, workspace)?.ok_or_else(crate::authorization::missing)?;
        if current != input.expected_version || input.version <= current {
            return Err(conflict());
        }
        let previous = workspace_publication::detail(&tx, Some(owner), &id, current, false)?;
        let next = ready_release(&tx, owner, &id, input.version)?;
        let old = refs(&previous)?;
        let new = refs(&next)?;
        let private = private_members(&tx, owner, workspace)?;
        // Only members supplied by the old public composition are replaced. Private additions remain.
        for material in old.difference(&new) {
            if private.contains(material) {
                continue;
            }
            tx.execute("DELETE FROM learning_workspace_members WHERE owner_user_id=? AND learning_workspace_id=? AND book_id=? AND publication_id=?",params![owner,workspace,material.book_id,material.publication_id]).map_err(|_|storage())?;
        }
        for material in &new {
            authorize_material(&tx, owner, material)?;
            tx.execute("INSERT OR IGNORE INTO learning_workspace_members(owner_user_id,learning_workspace_id,book_id,publication_id,added_at) VALUES(?,?,?,?,?)",params![owner,workspace,material.book_id,material.publication_id,now]).map_err(|_|storage())?;
        }
        tx.execute("UPDATE workspace_release_instances SET version=? WHERE owner_user_id=? AND learning_workspace_id=?",params![input.version,owner,workspace]).map_err(|_|storage())?;
        tx.execute("INSERT INTO workspace_release_instance_history(owner_user_id,learning_workspace_id,version,selected_at) VALUES(?,?,?,?)",params![owner,workspace,input.version,now]).map_err(|_|storage())?;
        tx.execute("UPDATE learning_workspaces SET updated_at=? WHERE owner_user_id=? AND learning_workspace_id=?",params![now,owner,workspace]).map_err(|_|storage())?;
        save_operation(&tx, owner, &input.operation_id, &parameters, workspace)?;
        tx.commit().map_err(|_| storage())?;
        self.workspace_instance_result(owner, workspace)
    }
}
