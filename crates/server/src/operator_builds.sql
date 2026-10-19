CREATE TABLE operator_builds (
    task_id TEXT PRIMARY KEY,
    task_json TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
