CREATE TABLE mcp_credentials (
    credential_id TEXT PRIMARY KEY NOT NULL,
    owner_user_id TEXT NOT NULL REFERENCES users(user_id),
    auth_epoch INTEGER NOT NULL,
    token_digest BLOB NOT NULL UNIQUE,
    label TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    revoked_at INTEGER
);
CREATE INDEX mcp_credentials_owner ON mcp_credentials(owner_user_id, created_at);
