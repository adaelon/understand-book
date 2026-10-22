CREATE TABLE workspace_exam_sprints (
    owner_user_id TEXT NOT NULL,
    learning_workspace_id TEXT NOT NULL,
    goal_revision INTEGER NOT NULL,
    settings_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(owner_user_id,learning_workspace_id),
    FOREIGN KEY(owner_user_id,learning_workspace_id) REFERENCES learning_workspaces(owner_user_id,learning_workspace_id)
);
