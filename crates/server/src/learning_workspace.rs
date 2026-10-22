//! User-owned learning scope. ReaderWorkspace remains the mutable reading scene.
use crate::{control_store::ControlStore, user_storage_paths::error};
use base_schema::PublishedBookRef;
pub use base_schema::{
    LearningWorkspace, LearningWorkspaceMember, WorkspaceGoal, WorkspaceGoalInput,
};
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension};

fn storage() -> ToolError {
    error(
        "LEARNING_WORKSPACE_STORAGE_UNAVAILABLE",
        "unavailable",
        "Learning workspace could not be read or committed",
    )
}

fn missing() -> ToolError {
    error(
        "LEARNING_WORKSPACE_NOT_FOUND",
        "not_found",
        "Learning workspace is unavailable",
    )
}

pub(crate) fn validate_goal(goal: &WorkspaceGoalInput) -> Result<(), ToolError> {
    if goal.text.trim().is_empty() {
        return Err(error(
            "WORKSPACE_GOAL_REQUIRED",
            "validation",
            "A user-accepted learning goal is required",
        ));
    }
    Ok(())
}

pub(crate) fn authorize_material(
    db: &Connection,
    owner: &str,
    reference: &PublishedBookRef,
) -> Result<(), ToolError> {
    let allowed = crate::course_access::material_allowed(db, owner, reference, crate::multi_user_host::now())?;
    if !allowed {
        return Err(error(
            "WORKSPACE_MATERIAL_NOT_AUTHORIZED",
            "permission",
            "Material requires an existing publication grant",
        ));
    }
    Ok(())
}

pub(crate) fn require_active(db: &Connection, owner: &str, id: &str) -> Result<(), ToolError> {
    let archived: Option<bool> = db.query_row(
        "SELECT archived FROM learning_workspaces WHERE owner_user_id=? AND learning_workspace_id=?",
        params![owner, id], |row| row.get(0),
    ).optional().map_err(|_| storage())?;
    match archived {
        None => Err(missing()),
        Some(true) => Err(error(
            "LEARNING_WORKSPACE_ARCHIVED",
            "conflict",
            "Restore the learning workspace before changing it",
        )),
        Some(false) => Ok(()),
    }
}

pub(crate) fn insert_goal(
    db: &Connection,
    owner: &str,
    id: &str,
    revision: u32,
    goal: &WorkspaceGoalInput,
    now: i64,
) -> Result<(), ToolError> {
    let constraints = serde_json::to_string(&goal.constraints).map_err(|_| storage())?;
    db.execute(
        "INSERT INTO learning_workspace_goals(owner_user_id,learning_workspace_id,revision,goal_text,constraints_json,created_at)
         VALUES(?,?,?,?,?,?)",
        params![owner, id, revision, goal.text, constraints, now],
    ).map_err(|_| storage())?;
    Ok(())
}

impl ControlStore {
    pub fn create_learning_workspace(
        &mut self,
        owner: &str,
        goal: WorkspaceGoalInput,
        members: Vec<PublishedBookRef>,
        now: i64,
    ) -> Result<LearningWorkspace, ToolError> {
        self.user_paths(owner)?;
        validate_goal(&goal)?;
        let id = uuid::Uuid::now_v7().to_string();
        let tx = self.connection.transaction().map_err(|_| storage())?;
        // Authorize the complete requested set before creating any private state.
        for member in &members {
            authorize_material(&tx, owner, member)?;
        }
        tx.execute(
            "INSERT INTO learning_workspaces(owner_user_id,learning_workspace_id,created_at,updated_at) VALUES(?,?,?,?)",
            params![owner, id, now, now],
        ).map_err(|_| storage())?;
        insert_goal(&tx, owner, &id, 1, &goal, now)?;
        for member in &members {
            tx.execute(
                "INSERT OR IGNORE INTO learning_workspace_members(owner_user_id,learning_workspace_id,book_id,publication_id,added_at) VALUES(?,?,?,?,?)",
                params![owner, id, member.book_id, member.publication_id, now],
            ).map_err(|_| storage())?;
        }
        tx.commit().map_err(|_| storage())?;
        self.learning_workspace(owner, &id)
    }

    pub fn list_learning_workspaces(
        &self,
        owner: &str,
    ) -> Result<Vec<LearningWorkspace>, ToolError> {
        self.user_paths(owner)?;
        let mut query = self.connection.prepare(
            "SELECT learning_workspace_id FROM learning_workspaces WHERE owner_user_id=? ORDER BY updated_at DESC,learning_workspace_id",
        ).map_err(|_| storage())?;
        let ids: Vec<String> = query
            .query_map([owner], |row| row.get(0))
            .map_err(|_| storage())?
            .collect::<Result<_, _>>()
            .map_err(|_| storage())?;
        ids.iter()
            .map(|id| self.learning_workspace(owner, id))
            .collect()
    }

    pub fn learning_workspace(
        &self,
        owner: &str,
        id: &str,
    ) -> Result<LearningWorkspace, ToolError> {
        self.user_paths(owner)?;
        let (archived, created_at, updated_at) = self.connection.query_row(
            "SELECT archived,created_at,updated_at FROM learning_workspaces WHERE owner_user_id=? AND learning_workspace_id=?",
            params![owner, id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).optional().map_err(|_| storage())?.ok_or_else(missing)?;
        let mut query = self.connection.prepare(
            "SELECT revision,goal_text,constraints_json,created_at FROM learning_workspace_goals
             WHERE owner_user_id=? AND learning_workspace_id=? ORDER BY revision",
        ).map_err(|_| storage())?;
        let rows = query
            .query_map(params![owner, id], |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(|_| storage())?;
        let goal_history: Vec<WorkspaceGoal> = rows
            .map(|row| {
                let (revision, text, constraints, created_at) = row.map_err(|_| storage())?;
                Ok(WorkspaceGoal {
                    revision,
                    text,
                    constraints: serde_json::from_str(&constraints).map_err(|_| storage())?,
                    created_at,
                })
            })
            .collect::<Result<_, ToolError>>()?;
        let goal = goal_history.last().cloned().ok_or_else(storage)?;
        let mut query = self.connection.prepare(
            "SELECT m.book_id,m.publication_id,m.added_at
             FROM learning_workspace_members m WHERE m.owner_user_id=? AND m.learning_workspace_id=?
             ORDER BY m.added_at,m.book_id,m.publication_id",
        ).map_err(|_| storage())?;
        let mut members: Vec<LearningWorkspaceMember> = query
            .query_map(params![owner, id], |row| {
                Ok(LearningWorkspaceMember {
                    published_book_ref: PublishedBookRef {
                        book_id: row.get(0)?,
                        publication_id: row.get(1)?,
                    },
                    added_at: row.get(2)?,
                    readable: false,
                })
            })
            .map_err(|_| storage())?
            .collect::<Result<_, _>>()
            .map_err(|_| storage())?;
        for member in &mut members {
            member.readable = crate::course_access::material_allowed(&self.connection,owner,&member.published_book_ref,crate::multi_user_host::now())?;
        }
        Ok(LearningWorkspace {
            learning_workspace_id: id.into(),
            goal,
            goal_history,
            members,
            archived,
            created_at,
            updated_at,
        })
    }

    pub fn revise_learning_workspace_goal(
        &mut self,
        owner: &str,
        id: &str,
        goal: WorkspaceGoalInput,
        now: i64,
    ) -> Result<LearningWorkspace, ToolError> {
        self.user_paths(owner)?;
        validate_goal(&goal)?;
        let tx = self.connection.transaction().map_err(|_| storage())?;
        require_active(&tx, owner, id)?;
        let revision: u32 = tx.query_row(
            "SELECT COALESCE(MAX(revision),0)+1 FROM learning_workspace_goals WHERE owner_user_id=? AND learning_workspace_id=?",
            params![owner, id], |row| row.get(0),
        ).map_err(|_| storage())?;
        insert_goal(&tx, owner, id, revision, &goal, now)?;
        tx.execute("UPDATE learning_workspaces SET updated_at=? WHERE owner_user_id=? AND learning_workspace_id=?", params![now, owner, id]).map_err(|_| storage())?;
        tx.commit().map_err(|_| storage())?;
        self.learning_workspace(owner, id)
    }

    pub fn add_learning_workspace_member(
        &mut self,
        owner: &str,
        id: &str,
        member: &PublishedBookRef,
        now: i64,
    ) -> Result<LearningWorkspace, ToolError> {
        self.user_paths(owner)?;
        let tx = self.connection.transaction().map_err(|_| storage())?;
        require_active(&tx, owner, id)?;
        authorize_material(&tx, owner, member)?;
        let changed = tx.execute(
            "INSERT OR IGNORE INTO learning_workspace_members(owner_user_id,learning_workspace_id,book_id,publication_id,added_at) VALUES(?,?,?,?,?)",
            params![owner, id, member.book_id, member.publication_id, now],
        ).map_err(|_| storage())?;
        tx.execute(
            "INSERT OR IGNORE INTO workspace_release_instance_private_materials(owner_user_id,learning_workspace_id,book_id,publication_id) SELECT owner_user_id,learning_workspace_id,?,? FROM workspace_release_instances WHERE owner_user_id=? AND learning_workspace_id=?",
            params![member.book_id, member.publication_id, owner, id],
        ).map_err(|_| storage())?;
        if changed > 0 {
            tx.execute("UPDATE learning_workspaces SET updated_at=? WHERE owner_user_id=? AND learning_workspace_id=?", params![now, owner, id]).map_err(|_| storage())?;
        }
        tx.commit().map_err(|_| storage())?;
        self.learning_workspace(owner, id)
    }

    pub fn remove_learning_workspace_member(
        &mut self,
        owner: &str,
        id: &str,
        member: &PublishedBookRef,
        now: i64,
    ) -> Result<LearningWorkspace, ToolError> {
        self.user_paths(owner)?;
        let tx = self.connection.transaction().map_err(|_| storage())?;
        require_active(&tx, owner, id)?;
        let changed = tx.execute(
            "DELETE FROM learning_workspace_members WHERE owner_user_id=? AND learning_workspace_id=? AND book_id=? AND publication_id=?",
            params![owner, id, member.book_id, member.publication_id],
        ).map_err(|_| storage())?;
        tx.execute(
            "DELETE FROM workspace_release_instance_private_materials WHERE owner_user_id=? AND learning_workspace_id=? AND book_id=? AND publication_id=?",
            params![owner, id, member.book_id, member.publication_id],
        ).map_err(|_| storage())?;
        if changed > 0 {
            tx.execute("UPDATE learning_workspaces SET updated_at=? WHERE owner_user_id=? AND learning_workspace_id=?", params![now, owner, id]).map_err(|_| storage())?;
        }
        tx.commit().map_err(|_| storage())?;
        self.learning_workspace(owner, id)
    }

    pub fn set_learning_workspace_archived(
        &mut self,
        owner: &str,
        id: &str,
        archived: bool,
        now: i64,
    ) -> Result<LearningWorkspace, ToolError> {
        self.user_paths(owner)?;
        let changed = self.connection.execute(
            "UPDATE learning_workspaces SET archived=?,updated_at=? WHERE owner_user_id=? AND learning_workspace_id=?",
            params![archived, now, owner, id],
        ).map_err(|_| storage())?;
        if changed == 0 {
            return Err(missing());
        }
        self.learning_workspace(owner, id)
    }
}
