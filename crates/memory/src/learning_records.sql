CREATE TABLE linked_learning_records (
    learning_workspace_id TEXT NOT NULL,
    source_learning_workspace_id TEXT NOT NULL,
    chat_session_id TEXT NOT NULL,
    selection TEXT NOT NULL,
    linked_at TEXT NOT NULL,
    PRIMARY KEY(learning_workspace_id, source_learning_workspace_id, chat_session_id));
