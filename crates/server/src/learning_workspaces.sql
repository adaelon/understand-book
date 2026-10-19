-- Learning ownership is separate from the mutable reader_workspaces scene.
CREATE TABLE learning_workspaces (
    owner_user_id TEXT NOT NULL REFERENCES users(user_id),
    learning_workspace_id TEXT NOT NULL,
    archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0,1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(owner_user_id, learning_workspace_id)
);
CREATE TABLE learning_workspace_goals (
    owner_user_id TEXT NOT NULL,
    learning_workspace_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    goal_text TEXT NOT NULL CHECK(length(trim(goal_text)) > 0),
    constraints_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY(owner_user_id, learning_workspace_id, revision),
    FOREIGN KEY(owner_user_id, learning_workspace_id)
        REFERENCES learning_workspaces(owner_user_id, learning_workspace_id)
);
CREATE TABLE learning_workspace_members (
    owner_user_id TEXT NOT NULL,
    learning_workspace_id TEXT NOT NULL,
    book_id TEXT NOT NULL,
    publication_id TEXT NOT NULL,
    added_at INTEGER NOT NULL,
    PRIMARY KEY(owner_user_id, learning_workspace_id, book_id, publication_id),
    FOREIGN KEY(owner_user_id, learning_workspace_id)
        REFERENCES learning_workspaces(owner_user_id, learning_workspace_id),
    FOREIGN KEY(book_id, publication_id)
        REFERENCES book_publications(book_id, publication_id)
);
