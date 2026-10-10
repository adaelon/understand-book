-- Minimal usage facts, independent of private chat/content retention.
CREATE TABLE admin_usage_events (
    user_id TEXT NOT NULL REFERENCES users(user_id),
    kind TEXT NOT NULL CHECK(kind IN ('read','question','completed')),
    event_ref TEXT NOT NULL,
    occurred_at INTEGER NOT NULL,
    PRIMARY KEY(user_id,kind,event_ref)
) STRICT;
CREATE INDEX admin_usage_events_time ON admin_usage_events(occurred_at,user_id,kind);
CREATE INDEX admin_usage_events_user_time ON admin_usage_events(user_id,occurred_at,kind);
