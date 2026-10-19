-- A shared release is referenced by a private workspace; learning records stay private.
CREATE TABLE workspace_release_instances (
    owner_user_id TEXT NOT NULL,
    learning_workspace_id TEXT NOT NULL,
    release_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    PRIMARY KEY(owner_user_id,learning_workspace_id),
    FOREIGN KEY(owner_user_id,learning_workspace_id) REFERENCES learning_workspaces(owner_user_id,learning_workspace_id),
    FOREIGN KEY(release_id,version) REFERENCES workspace_releases(release_id,version)
);
CREATE TABLE workspace_release_instance_history (
    owner_user_id TEXT NOT NULL,
    learning_workspace_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    selected_at INTEGER NOT NULL,
    PRIMARY KEY(owner_user_id,learning_workspace_id,version),
    FOREIGN KEY(owner_user_id,learning_workspace_id) REFERENCES workspace_release_instances(owner_user_id,learning_workspace_id)
);
CREATE TABLE workspace_release_instance_private_materials (
    owner_user_id TEXT NOT NULL,
    learning_workspace_id TEXT NOT NULL,
    book_id TEXT NOT NULL,
    publication_id TEXT NOT NULL,
    PRIMARY KEY(owner_user_id,learning_workspace_id,book_id,publication_id),
    FOREIGN KEY(owner_user_id,learning_workspace_id) REFERENCES workspace_release_instances(owner_user_id,learning_workspace_id),
    FOREIGN KEY(book_id,publication_id) REFERENCES book_publications(book_id,publication_id)
);
CREATE TABLE workspace_release_instance_operations (
    owner_user_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    parameters_json TEXT NOT NULL,
    learning_workspace_id TEXT NOT NULL,
    PRIMARY KEY(owner_user_id,operation_id),
    FOREIGN KEY(owner_user_id,learning_workspace_id) REFERENCES workspace_release_instances(owner_user_id,learning_workspace_id)
);
CREATE TABLE workspace_release_archives (
    release_id TEXT PRIMARY KEY,
    archived INTEGER NOT NULL CHECK(archived IN (0,1)),
    updated_at INTEGER NOT NULL,
    actor TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    FOREIGN KEY(actor,operation_id) REFERENCES admin_operations(actor,operation_id)
);
