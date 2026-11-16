-- Recovery evidence is scoped to the capability that actually completed.
ALTER TABLE operational_outcomes ADD COLUMN recovery_scope TEXT NOT NULL DEFAULT 'manual';
CREATE TABLE operational_successes (
    scope TEXT PRIMARY KEY,
    finished_at INTEGER NOT NULL
) STRICT;
