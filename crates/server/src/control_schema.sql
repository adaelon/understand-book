CREATE TABLE users (
    user_id TEXT PRIMARY KEY NOT NULL,
    disabled INTEGER NOT NULL DEFAULT 0 CHECK(disabled IN (0,1)),
    auth_epoch INTEGER NOT NULL DEFAULT 0,
    password_hash TEXT,
    is_admin INTEGER NOT NULL DEFAULT 0 CHECK(is_admin IN (0,1)),
    email TEXT CHECK(email IS NULL OR (email=lower(trim(email)) AND length(email)>0)),
    email_verified_at INTEGER CHECK((email IS NULL) = (email_verified_at IS NULL))
);
-- MU4 uses Argon2id PHC strings in users.password_hash. NULL cannot sign in.
-- A 32-byte session-token digest is the opaque session identity; epoch and
-- expiration are checked against the user on each authenticated request.
CREATE TABLE auth_sessions (
    token_digest BLOB PRIMARY KEY NOT NULL,
    owner_user_id TEXT NOT NULL REFERENCES users(user_id),
    auth_epoch INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);
CREATE TABLE book_publications (
    book_id TEXT NOT NULL, publication_id TEXT NOT NULL,
    directory TEXT NOT NULL UNIQUE, manifest TEXT NOT NULL,
    PRIMARY KEY(book_id, publication_id)
);
CREATE TABLE book_grants (
    owner_user_id TEXT NOT NULL REFERENCES users(user_id),
    book_id TEXT NOT NULL, publication_id TEXT NOT NULL,
    PRIMARY KEY(owner_user_id, book_id, publication_id),
    FOREIGN KEY(book_id, publication_id) REFERENCES book_publications(book_id, publication_id)
);
CREATE TABLE book_defaults (
    book_id TEXT PRIMARY KEY NOT NULL,
    publication_id TEXT NOT NULL,
    FOREIGN KEY(book_id, publication_id) REFERENCES book_publications(book_id, publication_id)
);
CREATE TABLE reader_workspaces (
    owner_user_id TEXT NOT NULL REFERENCES users(user_id), workspace_id TEXT NOT NULL,
    book_id TEXT, publication_id TEXT, selected_chat TEXT,
    generation INTEGER NOT NULL DEFAULT 0 CHECK(generation>=0),
    revision INTEGER NOT NULL DEFAULT 0 CHECK(revision>=0), checkpoint TEXT,
    checkpoint_seq INTEGER NOT NULL DEFAULT 0,
    CHECK((book_id IS NULL) = (publication_id IS NULL)),
    PRIMARY KEY(owner_user_id, workspace_id),
    FOREIGN KEY(book_id, publication_id) REFERENCES book_publications(book_id, publication_id)
);
CREATE TABLE run_admissions (
    owner_user_id TEXT NOT NULL REFERENCES users(user_id), client_request_id TEXT NOT NULL,
    turn_id TEXT NOT NULL, chat_session_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
    workspace_generation INTEGER NOT NULL, book_id TEXT NOT NULL, publication_id TEXT NOT NULL,
    dispatch_state TEXT NOT NULL CHECK(dispatch_state IN ('preparing','queued','claimed','settled','admission_failed')),
    boot_id TEXT, attempt INTEGER NOT NULL DEFAULT 0,
    cancel_requested INTEGER NOT NULL DEFAULT 0,
    key_closed INTEGER NOT NULL DEFAULT 0,
    unsaved INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(owner_user_id, client_request_id), UNIQUE(owner_user_id, turn_id),
    FOREIGN KEY(owner_user_id, workspace_id) REFERENCES reader_workspaces(owner_user_id, workspace_id),
    FOREIGN KEY(book_id, publication_id) REFERENCES book_publications(book_id, publication_id)
);
CREATE INDEX admission_dispatch ON run_admissions(dispatch_state, owner_user_id);
