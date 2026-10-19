-- Public, explicitly selected workspace compositions. Versions are insert-only.
CREATE TABLE workspace_releases (
    release_id TEXT NOT NULL,
    version INTEGER NOT NULL CHECK(version > 0),
    manifest_json TEXT NOT NULL,
    visible_to_users_json TEXT NOT NULL,
    readiness_json TEXT NOT NULL,
    published_by TEXT NOT NULL REFERENCES users(user_id),
    operation_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY(release_id,version),
    FOREIGN KEY(published_by,operation_id) REFERENCES admin_operations(actor,operation_id)
);
