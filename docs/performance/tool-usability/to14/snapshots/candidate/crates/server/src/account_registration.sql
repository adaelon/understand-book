-- INV1. Emails are normalized before storage; timestamps are Unix seconds.
-- NULL emails keep existing accounts usable until their first email binding.
CREATE UNIQUE INDEX users_email ON users(email);

-- Invitations are issued before the target account exists, so batches have their
-- own operation identity rather than an admin_operations target user.
CREATE TABLE invite_batches (
    actor TEXT NOT NULL REFERENCES users(user_id),
    operation_id TEXT NOT NULL,
    count INTEGER NOT NULL CHECK(count BETWEEN 1 AND 100),
    created_at INTEGER NOT NULL,
    PRIMARY KEY(actor, operation_id)
) STRICT;

CREATE TABLE beta_invites (
    invite_id TEXT PRIMARY KEY NOT NULL,
    code TEXT NOT NULL UNIQUE,
    actor TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('unused','used','disabled')),
    created_at INTEGER NOT NULL,
    used_by TEXT REFERENCES users(user_id),
    used_at INTEGER,
    disabled_at INTEGER,
    FOREIGN KEY(actor, operation_id) REFERENCES invite_batches(actor, operation_id),
    CHECK((state='unused' AND used_by IS NULL AND used_at IS NULL AND disabled_at IS NULL)
       OR (state='used' AND used_by IS NOT NULL AND used_at IS NOT NULL AND disabled_at IS NULL)
       OR (state='disabled' AND used_by IS NULL AND used_at IS NULL AND disabled_at IS NOT NULL))
) STRICT;
CREATE INDEX beta_invites_batch ON beta_invites(actor, operation_id);

-- Pending requests neither reserve an email nor consume an invite. Completion
-- keeps only the receipt and clears both temporary credentials in the same write.
CREATE TABLE registration_requests (
    request_id TEXT PRIMARY KEY NOT NULL,
    email TEXT NOT NULL CHECK(email=lower(trim(email)) AND length(email)>0),
    password_hash TEXT,
    invite_id TEXT NOT NULL REFERENCES beta_invites(invite_id),
    verification_code TEXT,
    expires_at INTEGER NOT NULL,
    failed_attempts INTEGER NOT NULL DEFAULT 0 CHECK(failed_attempts>=0),
    completed_user_id TEXT REFERENCES users(user_id),
    CHECK((completed_user_id IS NULL AND password_hash IS NOT NULL AND verification_code IS NOT NULL)
       OR (completed_user_id IS NOT NULL AND password_hash IS NULL AND verification_code IS NULL))
) STRICT;

CREATE TABLE email_binding_requests (
    request_id TEXT PRIMARY KEY NOT NULL,
    owner_user_id TEXT NOT NULL REFERENCES users(user_id),
    auth_epoch INTEGER NOT NULL,
    email TEXT NOT NULL CHECK(email=lower(trim(email)) AND length(email)>0),
    verification_code TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    failed_attempts INTEGER NOT NULL DEFAULT 0 CHECK(failed_attempts>=0),
    used_at INTEGER
) STRICT;

CREATE TABLE password_reset_requests (
    token TEXT PRIMARY KEY NOT NULL,
    owner_user_id TEXT NOT NULL REFERENCES users(user_id),
    auth_epoch INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    used_at INTEGER
) STRICT;
