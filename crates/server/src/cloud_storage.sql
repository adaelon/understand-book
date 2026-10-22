-- Account-owned originals and retained outputs. Internal preparation copies and
-- shared publications are not additional customer storage.
CREATE TABLE cloud_storage_objects (
    owner_user_id TEXT NOT NULL REFERENCES users(user_id),
    storage_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('upload','generated')),
    workspace_id TEXT,
    task_id TEXT,
    filename TEXT NOT NULL,
    path TEXT NOT NULL,
    bytes INTEGER NOT NULL CHECK(bytes >= 0),
    state TEXT NOT NULL CHECK(state IN ('reserved','used','deleted')),
    book_id TEXT,
    publication_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(owner_user_id,storage_id),
    UNIQUE(owner_user_id,path)
) STRICT;
CREATE INDEX cloud_storage_owner_state ON cloud_storage_objects(owner_user_id,state);
CREATE TABLE cloud_storage_adoptions (
    owner_user_id TEXT PRIMARY KEY REFERENCES users(user_id),
    adopted_at INTEGER NOT NULL,
    maintained_at INTEGER NOT NULL DEFAULT 0
) STRICT;
