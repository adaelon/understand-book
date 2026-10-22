-- One-use, short-lived browser authorization for an independent native session.
CREATE TABLE IF NOT EXISTS desktop_authorizations (
    code TEXT PRIMARY KEY,
    owner_user_id TEXT NOT NULL REFERENCES users(user_id),
    auth_epoch INTEGER NOT NULL,
    redirect_uri TEXT NOT NULL,
    code_challenge TEXT NOT NULL,
    expires_at INTEGER NOT NULL
);
